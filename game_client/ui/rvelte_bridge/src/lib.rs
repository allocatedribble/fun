//! Pass-72 product native UI adapter skeleton.
//!
//! This crate bridges rvelte's FUN-native pipeline to the game-client
//! process. It is the first crate inside `fun/**` to consume rvelte's
//! typed contracts; the boundary is pinned by
//! `rvelte/docs/project-fun-native-adapter-design.md` (pass 71).
//!
//! Pass 72 ships only the skeleton: a typed
//! [`ProductRvelteAdapter`] that mounts native routes, ingests host
//! snapshots / patches, translates product input events, emits the
//! pass-50 `fun_ui_render_packet_v1` frame packets, and submits them
//! through any [`FunUiFramePacketConsumer`] implementation. The adapter
//! is generic over the renderer sink so pass-72 tests use the
//! rvelte-side fake renderer and pass-73 will plug in fun-renderer's
//! native UI adapter without touching this surface.
//!
//! Forbidden surface (enforced by `Cargo.toml`):
//!
//! - no CEF, no browser subprocess, no Vite bundle;
//! - no browser globals (`window`, `document`, `navigator`);
//! - no JS bridge;
//! - no concrete renderer API import (D3D12 / Metal / Vulkan /
//!   wgpu types) — only the typed [`FunUiFramePacketConsumer`] trait;
//! - no graphics-backend crate dependency in the production tree.
//!
//! Schema label: [`PRODUCT_RVELTE_ADAPTER_SCHEMA`].

#![doc(html_root_url = "https://docs.rs/fun-rvelte-bridge/0.1.0")]

pub mod diagnostics;
#[cfg(feature = "fun_renderer_backend")]
pub mod fun_renderer_backend;
pub mod host_transport;
pub mod input_translator;
pub mod route_registry;
pub mod runtime;

use std::collections::BTreeMap;

use fun_native_app::{FunNativeApp, FunNativeAppFixtureResolver, NativeRouteKind};
use rvelte_fun_input::FunUiNativeInputEvent;
use rvelte_fun_native_codegen::host_bridge::{
    FunNativeHostCommandIntent, FunNativeHostPatch, FunNativeHostSnapshot,
};
use rvelte_fun_render_adapter::{
    FunRenderUiAdapter, FunRenderUiSubmitResult, FunUiFramePacketConsumer,
};
use rvelte_fun_ui_core::{FunUiAccessibilityPacket, FunUiFramePacket, FunUiHitRegionPacket};

pub use diagnostics::{
    PRODUCT_RVELTE_ADAPTER_SCHEMA, PRODUCT_RVELTE_ADAPTER_SCHEMA_VERSION, ProductRvelteDiagnostic,
};
pub use input_translator::{ProductInputEvent, translate_product_input};
pub use route_registry::{ProductRouteKind, ProductRouteRegistry};
pub use runtime::FunNativeUiRuntime;

/// Per-tick frame output handed back to the game-client scheduler.
#[derive(Clone, Debug)]
pub struct FrameOutput {
    /// Active route at emission time.
    pub route: ProductRouteKind,
    /// Frame ID stamped by the runtime.
    pub frame_id: u64,
    /// Tally returned by the rvelte fun-render adapter.
    pub submit: FunRenderUiSubmitResult,
    /// Cloned frame packet for inspection (the renderer already
    /// consumed it).
    pub frame: FunUiFramePacket,
    /// Routed input events emitted during this tick.
    pub routed_input_events: Vec<rvelte_fun_input::FunUiRoutedInputEvent>,
    /// Pending command intents drained from the runners.
    pub command_intents: Vec<FunNativeHostCommandIntent>,
    /// Typed diagnostics accumulated during this tick.
    pub diagnostics: Vec<ProductRvelteDiagnostic>,
}

/// Top-level adapter handle. One instance per game-client process.
///
/// The adapter is generic over the renderer sink so pass-72 tests
/// use the rvelte-side fake renderer and pass-73 plugs in
/// fun-renderer's native UI adapter without touching this surface.
pub struct ProductRvelteAdapter<S, R = fun_native_app::DefaultFixtureResolver>
where
    S: FunUiFramePacketConsumer,
    R: FunNativeAppFixtureResolver,
{
    /// Stable lookup of mountable routes.
    registry: ProductRouteRegistry,
    /// Underlying app shell that owns the FUN-native runtime.
    app: FunNativeApp<R>,
    /// Owned renderer sink. Stored as `Option<S>` so the adapter
    /// can lift the sink into the rvelte fun-render adapter
    /// wrapper for the duration of one `tick` and put it back
    /// without unsafe code.
    renderer_sink: Option<S>,
    /// Pending product input events buffered until the next tick.
    pending_input: Vec<ProductInputEvent>,
    /// Pending command intents harvested from the active route's
    /// runner. Drained on each tick.
    pending_command_intents: Vec<FunNativeHostCommandIntent>,
    /// Most recent hit-region slice cached for the product input
    /// router.
    last_hit_regions: Vec<FunUiHitRegionPacket>,
    /// Most recent accessibility slice cached for the product
    /// accessibility layer.
    last_accessibility: Vec<FunUiAccessibilityPacket>,
    /// Diagnostics accumulated since the last tick.
    diagnostics: Vec<ProductRvelteDiagnostic>,
    /// Mounted-route map (registry-side identity keyed off the
    /// underlying `NativeRouteKind`).
    mounted: BTreeMap<NativeRouteKind, ()>,
}

impl<S> ProductRvelteAdapter<S, fun_native_app::DefaultFixtureResolver>
where
    S: FunUiFramePacketConsumer,
{
    /// Creates the adapter using the default fixture resolver
    /// rooted at the rvelte examples directory. Production builds
    /// can replace the resolver via [`Self::with_resolver`].
    pub fn new(registry: ProductRouteRegistry, renderer_sink: S) -> Self {
        let crate_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Walk from `fun/game_client/ui/rvelte_bridge` up four
        // levels to reach the project root, then descend into
        // `rvelte/examples/fun-native-app` so the resolver picks up
        // the route fixtures the design references.
        let rvelte_app_root = crate_root
            .join("..")
            .join("..")
            .join("..")
            .join("..")
            .join("rvelte")
            .join("examples")
            .join("fun-native-app");
        let resolver = fun_native_app::DefaultFixtureResolver::from_crate_root(&rvelte_app_root);
        Self::with_resolver(registry, renderer_sink, resolver)
    }
}

impl<S, R> ProductRvelteAdapter<S, R>
where
    S: FunUiFramePacketConsumer,
    R: FunNativeAppFixtureResolver,
{
    /// Creates the adapter with a custom resolver. Tests use this
    /// to point at fixture roots they own.
    pub fn with_resolver(registry: ProductRouteRegistry, renderer_sink: S, resolver: R) -> Self {
        Self {
            registry,
            app: FunNativeApp::with_resolver(resolver),
            renderer_sink: Some(renderer_sink),
            pending_input: Vec::new(),
            pending_command_intents: Vec::new(),
            last_hit_regions: Vec::new(),
            last_accessibility: Vec::new(),
            diagnostics: Vec::new(),
            mounted: BTreeMap::new(),
        }
    }

    /// Returns the currently active route, when any.
    #[must_use]
    pub fn active_route(&self) -> Option<ProductRouteKind> {
        self.app
            .active_route()
            .and_then(|kind| self.registry.product_kind_of(kind))
    }

    /// Returns true when the named route has been mounted.
    #[must_use]
    pub fn has_route(&self, route: ProductRouteKind) -> bool {
        let Some(kind) = self.registry.native_kind_of(route) else {
            return false;
        };
        self.mounted.contains_key(&kind) && self.app.has_route(kind)
    }

    /// Returns the most recent hit-region packet slice. Pass-72
    /// hands this to the product input router.
    #[must_use]
    pub fn hit_regions(&self) -> &[FunUiHitRegionPacket] {
        &self.last_hit_regions
    }

    /// Returns the most recent accessibility packet slice. Pass-72
    /// hands this to the product accessibility layer.
    #[must_use]
    pub fn accessibility(&self) -> &[FunUiAccessibilityPacket] {
        &self.last_accessibility
    }

    /// Returns a shared reference to the renderer sink. Tests use
    /// this to inspect recorded events. Returns `None` only during
    /// the brief window inside `tick` while the sink is on loan to
    /// the rvelte adapter wrapper; outside `tick` the sink is
    /// always `Some`.
    #[must_use]
    pub fn renderer_sink(&self) -> Option<&S> {
        self.renderer_sink.as_ref()
    }

    /// Drains the typed diagnostics accumulated since the last
    /// tick.
    pub fn drain_diagnostics(&mut self) -> Vec<ProductRvelteDiagnostic> {
        std::mem::take(&mut self.diagnostics)
    }

    /// Drains pending command intents.
    pub fn drain_command_intents(&mut self) -> Vec<FunNativeHostCommandIntent> {
        std::mem::take(&mut self.pending_command_intents)
    }

    /// Mounts a route. Idempotent: re-mounting refreshes the
    /// underlying runner from the on-disk manifest.
    pub fn mount_route(&mut self, route: ProductRouteKind) -> Result<(), ProductRvelteDiagnostic> {
        let kind = self.resolve_kind(route)?;
        match self.app.mount_route(kind) {
            Ok(()) => {
                self.mounted.insert(kind, ());
                Ok(())
            }
            Err(error) => Err(ProductRvelteDiagnostic::AppShell(error)),
        }
    }

    /// Switches the active route. The underlying app shell tracks
    /// the active route on each [`mount_route`](Self::mount_route)
    /// call, so this method re-mounts to make `route` active again.
    pub fn set_active_route(
        &mut self,
        route: ProductRouteKind,
    ) -> Result<(), ProductRvelteDiagnostic> {
        self.mount_route(route)
    }

    /// Receives a host snapshot for `route`.
    ///
    /// Pass 72 ingests the snapshot through the app shell's
    /// disk-backed scenario loader. A future pass (74) will accept
    /// the snapshot directly when `fun_host` wires the real host
    /// transport.
    pub fn ingest_host_snapshot(
        &mut self,
        route: ProductRouteKind,
        _snapshot: FunNativeHostSnapshot,
    ) -> Result<(), ProductRvelteDiagnostic> {
        let kind = self.resolve_kind(route)?;
        self.app
            .ingest_host_snapshot(kind)
            .map(|_| ())
            .map_err(ProductRvelteDiagnostic::AppShell)
    }

    /// Receives a host patch for `route`.
    pub fn apply_host_patch(
        &mut self,
        route: ProductRouteKind,
        patch: FunNativeHostPatch,
    ) -> Result<(), ProductRvelteDiagnostic> {
        let kind = self.resolve_kind(route)?;
        self.app
            .apply_host_patch(kind, patch)
            .map_err(ProductRvelteDiagnostic::AppShell)
    }

    /// Receives a product input event. The event is buffered until
    /// the next [`tick`](Self::tick); at tick time the adapter
    /// translates it into a typed [`FunUiNativeInputEvent`] and
    /// replays it through the active route's input router.
    pub fn submit_product_input(
        &mut self,
        event: ProductInputEvent,
    ) -> Result<(), ProductRvelteDiagnostic> {
        self.pending_input.push(event);
        Ok(())
    }

    /// Disposes a route. The active flag clears if it matched.
    pub fn dispose(&mut self, route: ProductRouteKind) -> Result<(), ProductRvelteDiagnostic> {
        let kind = self.resolve_kind(route)?;
        self.app.dispose(kind);
        self.mounted.remove(&kind);
        Ok(())
    }

    /// Runs one tick of the runtime loop. The order matches the
    /// design's runtime-loop contract:
    ///
    /// 1. translate + replay pending product input events;
    /// 2. emit the next frame packet;
    /// 3. submit through the renderer sink;
    /// 4. cache hit-region / accessibility slices;
    /// 5. drain command intents and diagnostics;
    /// 6. return a [`FrameOutput`].
    ///
    /// Pass-72 emits a typed
    /// [`ProductRvelteDiagnostic::NoActiveRoute`] when no route is
    /// mounted (the safe blank/error UI state).
    pub fn tick(&mut self) -> Result<FrameOutput, ProductRvelteDiagnostic> {
        let active = match self.app.active_route() {
            Some(kind) => kind,
            None => {
                let diagnostic = ProductRvelteDiagnostic::NoActiveRoute;
                self.diagnostics.push(diagnostic.clone());
                return Err(diagnostic);
            }
        };
        let product_route = self
            .registry
            .product_kind_of(active)
            .ok_or(ProductRvelteDiagnostic::UnknownRoute { route_id: 0 })?;

        // Phase 1: input replay.
        let mut routed_input_events = Vec::with_capacity(self.pending_input.len());
        if !self.pending_input.is_empty() {
            let mut translated: Vec<FunUiNativeInputEvent> =
                Vec::with_capacity(self.pending_input.len());
            for event in self.pending_input.drain(..) {
                match translate_product_input(event) {
                    Ok(typed) => translated.push(typed),
                    Err(reason) => {
                        self.diagnostics
                            .push(ProductRvelteDiagnostic::InputTranslation { reason });
                    }
                }
            }
            if !translated.is_empty() {
                match self.app.replay_input(translated) {
                    Ok(replay) => routed_input_events.extend(replay.routed_events),
                    Err(error) => {
                        return Err(ProductRvelteDiagnostic::AppShell(error));
                    }
                }
            }
        }

        // Phase 2 + 3: emit the frame packet through the runtime,
        // then submit it through the rvelte fun-render adapter
        // into the product-owned renderer sink. The sink is on
        // loan to the wrapper for the duration of this call; the
        // adapter recovers it via `into_backend()`.
        let app_frame = match self.app.emit_frame() {
            Ok(frame) => frame,
            Err(error) => {
                return Err(ProductRvelteDiagnostic::AppShell(error));
            }
        };
        // Pass-73 packet-consumer API: the adapter borrows the sink
        // mutably for the duration of the submit call. No ownership
        // transfer is needed.
        let sink = match self.renderer_sink.as_mut() {
            Some(sink) => sink,
            None => {
                return Err(ProductRvelteDiagnostic::RendererSinkUnavailable);
            }
        };
        let adapter = FunRenderUiAdapter::new();
        let submit_result = adapter
            .submit(&app_frame.frame, sink)
            .map_err(ProductRvelteDiagnostic::Adapter)?;

        // Phase 4: cache hit / accessibility slices.
        self.last_hit_regions = app_frame.frame.hit_regions.clone();
        self.last_accessibility = app_frame.frame.accessibility.clone();

        // Phase 5: command intents (none harvested yet — pass 74
        // will wire the real bridge transport). Diagnostics drain
        // through the FrameOutput.
        let diagnostics = std::mem::take(&mut self.diagnostics);
        let command_intents = std::mem::take(&mut self.pending_command_intents);

        Ok(FrameOutput {
            route: product_route,
            frame_id: app_frame.frame_id,
            submit: submit_result,
            frame: app_frame.frame,
            routed_input_events,
            command_intents,
            diagnostics,
        })
    }

    fn resolve_kind(
        &self,
        route: ProductRouteKind,
    ) -> Result<NativeRouteKind, ProductRvelteDiagnostic> {
        self.registry
            .native_kind_of(route)
            .ok_or(ProductRvelteDiagnostic::UnknownRoute {
                route_id: route.code(),
            })
    }
}
