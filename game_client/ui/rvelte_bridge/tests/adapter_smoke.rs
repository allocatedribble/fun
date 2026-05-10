//! Pass-72 product native UI adapter smoke tests.
//!
//! Drives the [`ProductRvelteAdapter`] through the seven scenarios
//! the pass-72 spec lists:
//!
//! 1. adapter creates runtime;
//! 2. adapter mounts launcher route;
//! 3. adapter receives mock host snapshot;
//! 4. adapter replays input;
//! 5. adapter emits frame packet;
//! 6. adapter submits to fake renderer;
//! 7. adapter disposes cleanly.
//!
//! The renderer sink is the rvelte-side
//! [`FakeRenderer2DCommandSink`]. Pass 72 explicitly does **not**
//! depend on any concrete graphics backend.

use std::path::PathBuf;

use fun_native_app::{DefaultFixtureResolver, NativeRouteKind};
use fun_rvelte_bridge::{
    PRODUCT_RVELTE_ADAPTER_SCHEMA, PRODUCT_RVELTE_ADAPTER_SCHEMA_VERSION, ProductInputEvent,
    ProductRouteKind, ProductRouteRegistry, ProductRvelteAdapter, ProductRvelteDiagnostic,
};
use rvelte_fun_native_codegen::host_bridge::{
    FUN_NATIVE_HOST_BRIDGE_SCHEMA, FunNativeHostSnapshot, HostAuthorizationContext,
};
use rvelte_fun_render_adapter::fake::{FakeRenderer2DCommandSink, FakeRendererEvent};

fn rvelte_app_root() -> PathBuf {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_root
        .join("..")
        .join("..")
        .join("..")
        .join("..")
        .join("rvelte")
        .join("examples")
        .join("fun-native-app")
}

fn make_adapter() -> ProductRvelteAdapter<FakeRenderer2DCommandSink, DefaultFixtureResolver> {
    let resolver = DefaultFixtureResolver::from_crate_root(&rvelte_app_root());
    ProductRvelteAdapter::with_resolver(
        ProductRouteRegistry::with_well_known(),
        FakeRenderer2DCommandSink::new(),
        resolver,
    )
}

fn must_ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("expected Ok, got {error:?}"),
    }
}

fn must_err<T: std::fmt::Debug, E>(result: Result<T, E>) -> E {
    match result {
        Ok(value) => panic!("expected Err, got Ok({value:?})"),
        Err(error) => error,
    }
}

// --------------------------------------------------------------------
// Structural tests.
// --------------------------------------------------------------------

#[test]
fn schema_label_and_version_are_stable() {
    assert_eq!(
        PRODUCT_RVELTE_ADAPTER_SCHEMA,
        "fun.product.rvelte_bridge.adapter.v1"
    );
    assert_eq!(PRODUCT_RVELTE_ADAPTER_SCHEMA_VERSION, 1);
}

#[test]
fn registry_maps_six_well_known_routes() {
    let registry = ProductRouteRegistry::with_well_known();
    assert_eq!(registry.len(), 6);
    assert_eq!(
        registry.native_kind_of(ProductRouteKind::LauncherShell),
        Some(NativeRouteKind::LauncherShell)
    );
    assert_eq!(
        registry.product_kind_of(NativeRouteKind::DiagnosticsList),
        Some(ProductRouteKind::DiagnosticsList)
    );
}

#[test]
fn registry_returns_none_for_product_reserved_route() {
    let registry = ProductRouteRegistry::with_well_known();
    let reserved = ProductRouteKind::ProductReserved { code: 999 };
    assert!(registry.native_kind_of(reserved).is_none());
}

// --------------------------------------------------------------------
// Spec scenarios.
// --------------------------------------------------------------------

#[test]
fn scenario_adapter_creates_runtime() {
    let adapter = make_adapter();
    // No active route at construction.
    assert!(adapter.active_route().is_none());
    // Renderer sink is owned by the adapter and accessible for
    // inspection.
    assert!(adapter.renderer_sink().is_some());
    // No diagnostics accumulated.
    assert!(adapter.hit_regions().is_empty());
    assert!(adapter.accessibility().is_empty());
}

#[test]
fn scenario_adapter_mounts_launcher_route() {
    let mut adapter = make_adapter();
    must_ok(adapter.mount_route(ProductRouteKind::LauncherShell));
    assert!(adapter.has_route(ProductRouteKind::LauncherShell));
    assert_eq!(
        adapter.active_route(),
        Some(ProductRouteKind::LauncherShell)
    );
}

#[test]
fn scenario_adapter_receives_mock_host_snapshot() {
    let mut adapter = make_adapter();
    must_ok(adapter.mount_route(ProductRouteKind::LauncherShell));
    // Pass-72 ingests the snapshot through the disk-backed scenario
    // loader (the design's pass-74 will accept the snapshot
    // directly). Construct a placeholder snapshot the call site
    // does not consume yet — ingest_host_snapshot simply forwards
    // to the app shell's loader.
    let placeholder = FunNativeHostSnapshot {
        schema_version: String::from(FUN_NATIVE_HOST_BRIDGE_SCHEMA),
        component_id: 1,
        revision: 1,
        freshness_token: 1,
        authorization_context: HostAuthorizationContext {
            scope_label: String::from("launcher"),
            session_revision: 1,
            capability_mask: 0xFF,
        },
        payload_bytes: 16,
        state: std::collections::BTreeMap::new(),
    };
    must_ok(adapter.ingest_host_snapshot(ProductRouteKind::LauncherShell, placeholder));
}

#[test]
fn scenario_adapter_replays_input() {
    let mut adapter = make_adapter();
    must_ok(adapter.mount_route(ProductRouteKind::LauncherShell));
    must_ok(adapter.ingest_host_snapshot(ProductRouteKind::LauncherShell, placeholder_snapshot()));
    // Run one tick to populate the runner's region table.
    let _ = must_ok(adapter.tick());
    // Click the primary launch button at (116, 94) — same coords
    // as the rvelte-side launcher scenario.
    must_ok(
        adapter.submit_product_input(ProductInputEvent::PointerDown {
            pointer_id: 1,
            x: 116,
            y: 94,
            button: fun_rvelte_bridge::input_translator::ProductPointerButton::Primary,
        }),
    );
    must_ok(adapter.submit_product_input(ProductInputEvent::PointerUp {
        pointer_id: 1,
        x: 116,
        y: 94,
        button: fun_rvelte_bridge::input_translator::ProductPointerButton::Primary,
    }));
    let frame = must_ok(adapter.tick());
    let clicked = frame.routed_input_events.iter().any(|event| {
        matches!(
            event.semantic_route,
            rvelte_fun_input::FunUiSemanticInputRoute::ButtonClicked
        )
    });
    assert!(
        clicked,
        "expected ButtonClicked from product input replay, got {:?}",
        frame.routed_input_events
    );
}

#[test]
fn scenario_adapter_emits_frame_packet() {
    let mut adapter = make_adapter();
    must_ok(adapter.mount_route(ProductRouteKind::HudOverlay));
    must_ok(adapter.ingest_host_snapshot(ProductRouteKind::HudOverlay, placeholder_snapshot()));
    let frame = must_ok(adapter.tick());
    assert_eq!(frame.route, ProductRouteKind::HudOverlay);
    assert!(frame.submit.layers_submitted >= 1);
    assert!(!frame.frame.layers.is_empty());
    // Hit-region and accessibility slices are cached for the
    // product input router and accessibility layer.
    assert!(!adapter.hit_regions().is_empty());
    assert!(!adapter.accessibility().is_empty());
}

#[test]
fn scenario_adapter_submits_to_fake_renderer() {
    let mut adapter = make_adapter();
    must_ok(adapter.mount_route(ProductRouteKind::PauseMenu));
    must_ok(adapter.ingest_host_snapshot(ProductRouteKind::PauseMenu, placeholder_snapshot()));
    let _ = must_ok(adapter.tick());
    // The fake renderer recorded BeginFrame + CreateLayer +
    // FinishLayer + FinishFrame events at minimum.
    let sink = match adapter.renderer_sink() {
        Some(sink) => sink,
        None => panic!("renderer sink unexpectedly absent"),
    };
    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, FakeRendererEvent::BeginFrame { .. })),
        "fake renderer recorded no BeginFrame: {:?}",
        events
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, FakeRendererEvent::FinishFrame)),
        "fake renderer recorded no FinishFrame"
    );
}

#[test]
fn scenario_adapter_disposes_cleanly() {
    let mut adapter = make_adapter();
    must_ok(adapter.mount_route(ProductRouteKind::SettingsShell));
    must_ok(adapter.ingest_host_snapshot(ProductRouteKind::SettingsShell, placeholder_snapshot()));
    let _ = must_ok(adapter.tick());
    must_ok(adapter.dispose(ProductRouteKind::SettingsShell));
    assert!(adapter.active_route().is_none());
    assert!(!adapter.has_route(ProductRouteKind::SettingsShell));
}

// --------------------------------------------------------------------
// Safe blank/error UI state tests.
// --------------------------------------------------------------------

#[test]
fn tick_without_active_route_returns_typed_diagnostic() {
    let mut adapter = make_adapter();
    let diagnostic = must_err(adapter.tick());
    assert!(matches!(diagnostic, ProductRvelteDiagnostic::NoActiveRoute));
    assert_eq!(
        diagnostic.code(),
        "fun.product.rvelte_bridge.no_active_route"
    );
}

#[test]
fn dispose_unknown_route_returns_typed_diagnostic() {
    let mut adapter = make_adapter();
    // Dispose a route that was never mounted; the call should
    // succeed (idempotent) but not the unknown variant.
    let result = adapter.dispose(ProductRouteKind::ProductReserved { code: 999 });
    let diagnostic = must_err(result);
    assert!(matches!(
        diagnostic,
        ProductRvelteDiagnostic::UnknownRoute { route_id: 999 }
    ));
}

#[test]
fn input_translation_rejects_oversized_text_payload() {
    let mut adapter = make_adapter();
    must_ok(adapter.mount_route(ProductRouteKind::CommandBar));
    must_ok(adapter.ingest_host_snapshot(ProductRouteKind::CommandBar, placeholder_snapshot()));
    let _ = must_ok(adapter.tick());
    must_ok(adapter.submit_product_input(ProductInputEvent::TextInput {
        text: "x".repeat(128),
    }));
    let frame = must_ok(adapter.tick());
    assert!(
        frame
            .diagnostics
            .iter()
            .any(|d| matches!(d, ProductRvelteDiagnostic::InputTranslation { .. })),
        "expected typed InputTranslation diagnostic"
    );
}

// --------------------------------------------------------------------
// Helpers.
// --------------------------------------------------------------------

fn placeholder_snapshot() -> FunNativeHostSnapshot {
    FunNativeHostSnapshot {
        schema_version: String::from(FUN_NATIVE_HOST_BRIDGE_SCHEMA),
        component_id: 1,
        revision: 1,
        freshness_token: 1,
        authorization_context: HostAuthorizationContext {
            scope_label: String::from("launcher"),
            session_revision: 1,
            capability_mask: 0xFF,
        },
        payload_bytes: 16,
        state: std::collections::BTreeMap::new(),
    }
}
