# fun-rvelte-bridge — fun-renderer-backed packet consumer

contract_id: fun_product_rvelte_bridge_fun_renderer_backend_v1
status: pass_73_fun_renderer_backed_consumer
owner: project-fun-ui-platform
schema_label: fun.product.rvelte_bridge.fun_renderer_backend.v1
related_design: `../../rvelte/docs/project-fun-native-adapter-design.md`
related_pass_72: `rvelte_bridge.md`
related_rvelte_adapter: `../../rvelte/docs/fun-render-adapter.md`

## Decision

Pass 73 wires fun-renderer's experimental `native_ui_adapter`
feature into the rvelte bridge as a real
[`FunUiFramePacketConsumer`]. The integration is feature-gated
(`fun_renderer_backend`), default-off so the bridge's production
dependency tree stays free of bevy / wgpu / dx12 / vulkan / metal /
native_ui / swapchain crates. With the feature on the bridge gains the
`fun_renderer_backend::FunRendererPacketConsumer` consumer, which
lifts a `FunUiFramePacket` through fun-renderer's
`NativeUiRendererDescriptors::from_packet` in one call — no
per-call walker, no fake renderer.

## Architectural pivot

Pass 72 made `ProductRvelteAdapter` generic over any
`Renderer2DCommandSink`, treating the per-call 14-method surface as
the renderer contract. fun-renderer's `native_ui_adapter` does not
expose 14 separate hooks; its public API is the whole-packet
function `NativeUiRendererDescriptors::from_packet(packet,
&resource_table, composite_into)`. Wrapping that in a per-call
walker would mean re-flattening the packet immediately, then
re-aggregating it — wasted work for the only backend pass-73 ships.

Pass 73 split the rvelte adapter's external contract from its
implementation strategy:

- The new primary trait `FunUiFramePacketConsumer` is the contract
  every backend implements: declare a capability report, accept a
  `FunUiFramePacket`, return a typed `FunRenderUiSubmitResult` or
  reject with a typed `FunRenderUiAdapterError`.
- The 14-method `Renderer2DCommandSink` surface remains as the
  *implementation strategy* the fake renderer continues to use; a
  blanket `impl<T: Renderer2DCommandSink> FunUiFramePacketConsumer
  for T` keeps the existing fake-renderer test channel unchanged.
- `FunRenderUiAdapter::submit` now takes `&mut C:
  FunUiFramePacketConsumer` directly; the previous
  `FunRenderUiPacketSink` wrapper is removed. The pre-walk
  schema/validate/capability-limit/primitive-capability gates run
  the same way for every consumer.

The `ProductRvelteAdapter` generic bound is now
`S: FunUiFramePacketConsumer`, so swapping the rvelte fake renderer
for `FunRendererPacketConsumer` is a one-line change at
construction time.

## Artifacts

`fun/game_client/ui/rvelte_bridge/`:

- `Cargo.toml` — adds an opt-in `fun_renderer_backend` feature that
  pulls in `fun_renderer` with its `native_ui_adapter` feature.
  Default features remain empty.
- `src/lib.rs` — re-exports `fun_renderer_backend` behind
  `#[cfg(feature = "fun_renderer_backend")]`. The unfeatured
  `ProductRvelteAdapter` API is unchanged.
- `src/fun_renderer_backend.rs` — `FunRendererPacketConsumer` struct
  owning a `NativeUiResourceTable`, a `NativeUiCompositeLayer`
  policy, an advertised `FunRenderUiCapabilityReport`, the most
  recent `NativeUiRendererDescriptors`, and a `submit_count`.
  Implements `FunUiFramePacketConsumer::submit_frame` by calling
  `NativeUiRendererDescriptors::from_packet(frame,
  &self.resource_table, self.composite_into_layer)` and lifting
  fun-renderer's validation diagnostics into the rvelte typed-error
  vocabulary (`MissingResource { kind: "glyph" | "image" }`,
  `UnsupportedPrimitive { operation: "draw_path_lite" }`,
  `InvalidClipStack`, `InvalidTransformStack`,
  `InvalidOpacityStack`, `BackendRejected` fallback).
- `tests/fun_renderer_backend.rs` — eight tests gated by
  `#![cfg(feature = "fun_renderer_backend")]`.

## Consumer surface

```rust
pub struct FunRendererPacketConsumer { /* … */ }

impl FunRendererPacketConsumer {
    pub fn new() -> Self;
    pub fn with_capability(self, FunRenderUiCapabilityReport) -> Self;
    pub const fn with_composite_layer(self, NativeUiCompositeLayer) -> Self;
    pub const fn resource_table(&self) -> &NativeUiResourceTable;
    pub const fn resource_table_mut(&mut self) -> &mut NativeUiResourceTable;
    pub fn last_descriptors(&self) -> Option<&NativeUiRendererDescriptors>;
    pub const fn submit_count(&self) -> u32;
    pub const fn composite_into_layer(&self) -> NativeUiCompositeLayer;
}

impl FunUiFramePacketConsumer for FunRendererPacketConsumer { /* … */ }
```

The owner records renderer-allocated resources via
`resource_table_mut()` whenever the resource registry hands back a
freshly-allocated texture; the consumer itself never allocates GPU
resources.

## Diagnostic lifting

fun-renderer surfaces validation diagnostics as fields on
`NativeUiRendererDescriptors::validation`:

| fun-renderer field | rvelte adapter error |
|---|---|
| `schema_mismatch` | `SchemaMismatch { expected, observed }` |
| `missing_glyph_resources > 0` | `MissingResource { kind: "glyph", id }` |
| `missing_image_resources > 0` | `MissingResource { kind: "image", id }` |
| `unsupported_path_lite_count > 0` | `UnsupportedPrimitive { operation: "draw_path_lite" }` |
| `clip_stack_imbalance` | `InvalidClipStack { detail: "fun_renderer_clip_stack_imbalance", clip_id: None }` |
| `transform_stack_imbalance` | `InvalidTransformStack { detail: "fun_renderer_transform_stack_imbalance" }` |
| `opacity_stack_imbalance` | `InvalidOpacityStack { detail: "fun_renderer_opacity_stack_imbalance" }` |
| any other `passed_validation() == false` | `BackendRejected { code: "rvt.fun_render.adapter.fun_renderer_validation_failed" }` |

Helper functions `first_missing_glyph` / `first_missing_image` walk
the frame to recover the first offending logical ID for the typed
diagnostic. Capability-flag rejections (e.g.
`supports_text_run_reference = false`) fire upstream in the rvelte
adapter's `check_primitive_capabilities` gate before the consumer
ever sees the packet, so the consumer never has to enumerate
unsupported draws on its own.

## Tests

| test | what it proves |
|---|---|
| `schema_label_is_stable` | `FUN_RENDERER_PACKET_CONSUMER_SCHEMA` and version pinned. |
| `launcher_shell_lifts_through_fun_renderer_backend` | pass-58 LauncherShell frame lifts cleanly; consumer records non-zero layers + draws and stores a passed-validation descriptor. |
| `hud_overlay_lifts_through_fun_renderer_backend` | pass-59 HudOverlay frame lifts cleanly with paired layers. |
| `pause_menu_lifts_through_fun_renderer_backend` | pass-60 PauseMenuRoute frame lifts cleanly. |
| `missing_glyph_resource_surfaces_typed_rejection` | dropping the glyph entries before submit yields `MissingResource { kind: "glyph", … }`, not a stringly-typed diagnostic. |
| `submit_count_advances_per_frame` | three sequential lifts advance `submit_count` to 3. |
| `capability_report_drives_adapter_pre_validation` | turning off `supports_text_run_reference` in the consumer's advertised capability gets the frame rejected in the adapter's pre-walk gate (code `rvt.fun_render.adapter.unsupported_primitive`) before fun-renderer ever sees it. |
| `composite_layer_policy_is_threaded_into_descriptors` | constructing the consumer with `NativeUiCompositeLayer::OverlayOnly` writes that policy into `NativeUiRendererDescriptors::graph_pass.composite_into_layer`. |

All eight tests pass:
`cargo test -p fun-rvelte-bridge --features fun_renderer_backend
--test fun_renderer_backend`.

## Validation

Default features (production tree, no graphics-API crates):

```text
cargo fmt -p fun-rvelte-bridge -- --check                 # clean
cargo check -p fun-rvelte-bridge --all-targets            # clean
cargo clippy -p fun-rvelte-bridge --all-targets --
  -D warnings                                              # clean
cargo test -p fun-rvelte-bridge                           # 27/27 passing
```

With the `fun_renderer_backend` feature on:

```text
cargo fmt -p fun-rvelte-bridge -- --check                       # clean
cargo check -p fun-rvelte-bridge --all-targets
  --features fun_renderer_backend                                # clean
cargo clippy -p fun-rvelte-bridge --all-targets
  --features fun_renderer_backend -- -D warnings                 # clean
cargo test -p fun-rvelte-bridge --features fun_renderer_backend # 35/35 passing
```

Regression sweep (rvelte): `cargo test --workspace` from `rvelte/`
reports 497 tests passing, zero failures across the consumer-trait
refactor.

## Forbidden surface (default features)

The crate's default-feature dependency tree must stay free of
graphics-API crates. With the `fun_renderer_backend` feature off:

- `Cargo.toml` lists `fun_renderer` only as `optional = true`.
- The `fun_renderer_backend` module is gated by
  `#[cfg(feature = "fun_renderer_backend")]` and inert by default.
- `cargo tree -p fun-rvelte-bridge -e normal | grep -ciE
  "wgpu|vulkan|metal|dx12|d3d12|native_ui|swapchain|fun-renderer|
  fun_renderer"` returns 0.

With the feature on, the production tree gains `fun_renderer` and
its transitive bevy/wgpu surface — the explicit price of opting
into the experimental backend. Pass 73 does not change the
rvelte-side adapter's forbidden-surface guarantee; the rvelte crate
remains backend-free.

## Forward Pointers

Per the pass-71 design's migration plan:

- **Pass 74** — replace the disk-backed scenario loader inside
  `ingest_host_snapshot` with a real `fun_host` transport.
- **Pass 75** — wire product input events from the existing
  game-client input pipeline.
- **Pass 76** — add `--rvelte-bridge=<route>` to the game-client
  binary so a single product binary can boot any of the six routes
  through the adapter, switching between the fake renderer and the
  fun-renderer-backed consumer via a feature-flag-driven runtime
  selection.
- **Pass 77+** — staged removal of legacy NATIVE_UI / browser UI, gated
  on the fun-renderer-backed path being proven on every product
  route.

Pass 73 explicitly does **not** authorize any change under
`fun/game_client/ui/main`, `fun/fun_ui_native_ui`, or `fun/fun_host`.

## Exit Criteria

- Bridge owns a feature-gated fun-renderer-backed consumer that
  implements `FunUiFramePacketConsumer` directly via
  `NativeUiRendererDescriptors::from_packet`.
- The consumer lifts the launcher / HUD / pause-menu route fixtures
  through fun-renderer with passed validation, advertises a
  configurable capability report, and surfaces fun-renderer
  validation diagnostics through the rvelte typed-error vocabulary.
- Default-feature production tree retains zero graphics-API
  dependencies.
- The `fun_renderer_backend` feature gate exists, compiles, lints
  clean, and is exercised by eight integration tests.
