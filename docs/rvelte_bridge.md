# fun-rvelte-bridge

contract_id: fun_product_rvelte_bridge_v1
status: pass_72_product_native_adapter_skeleton
owner: project-fun-ui-platform
schema_label: fun.product.rvelte_bridge.adapter.v1
proposed_owner_path: `fun/game_client/ui/rvelte_bridge/`
related_design: `../../rvelte/docs/project-fun-native-adapter-design.md`

## Decision

Pass 72 ships the product native UI adapter skeleton at
`fun/game_client/ui/rvelte_bridge/`. The crate is the first
`fun/**` consumer of rvelte's typed contracts; the boundary itself
was accepted by pass-71's design gate. Pass 72 adds no concrete
graphics-API dependency — the adapter is generic over any
[`Renderer2DCommandSink`] implementation, and tests exercise it
through the rvelte-side fake renderer.

## Artifacts

`fun/game_client/ui/rvelte_bridge/`:

- `Cargo.toml` — production deps limited to `rvelte-fun-ui-core`,
  `rvelte-fun-input`, `rvelte-fun-native-codegen`,
  `rvelte-fun-render-adapter`, `fun-native-app`, and `serde`.
  No `fun-renderer`, no graphics-API crate.
- `src/lib.rs` — `ProductRvelteAdapter`, `FrameOutput`, the
  Option<S>-based renderer-sink ownership transfer used by
  `tick`, schema label `fun.product.rvelte_bridge.adapter.v1`.
- `src/runtime.rs` — `FunNativeUiRuntime` type alias.
- `src/route_registry.rs` — `ProductRouteKind` (6 well-known
  variants + `ProductReserved` for product-only future routes),
  `ProductRouteRegistry::with_well_known()`.
- `src/input_translator.rs` — `ProductInputEvent`, `ProductKey`,
  `ProductPointerButton`, `translate_product_input`. Bounded text
  payloads (max 64 ASCII chars).
- `src/diagnostics.rs` — `ProductRvelteDiagnostic` (7 variants
  with stable `fun.product.rvelte_bridge.*` codes).
- `tests/adapter_smoke.rs` — 13 tests covering the seven spec
  scenarios + structural and safe-blank-state cases.

`fun/Cargo.toml` registers the new crate as a workspace member at
`game_client/ui/rvelte_bridge`.

## Pass-72 spec coverage

| spec item | covered by |
|---|---|
| native route registry loader | `ProductRouteRegistry::with_well_known` + `ProductRouteKind` |
| runtime owner | `FunNativeUiRuntime` (re-export of `fun_native_app::FunNativeApp`) wrapped inside `ProductRvelteAdapter` |
| host bridge adapter | `ProductRvelteAdapter::ingest_host_snapshot` + `apply_host_patch` |
| input adapter | `ProductInputEvent` + `translate_product_input` |
| frame packet output | `FrameOutput.frame` (`FunUiFramePacket`) |
| fake renderer test channel | tests use `FakeRenderer2DCommandSink`; the adapter is generic over any `Renderer2DCommandSink` |
| diagnostics channel | `ProductRvelteDiagnostic` + `FrameOutput.diagnostics` + `drain_diagnostics` |
| safe blank/error UI state | `tick` returns `Err(ProductRvelteDiagnostic::NoActiveRoute)` when no route is mounted; resolver / route / sink rejections all surface typed |

## Tests

| spec test | name | what it asserts |
|---|---|---|
| 1. adapter creates runtime | `scenario_adapter_creates_runtime` | construction succeeds, no active route, sink is `Some` |
| 2. adapter mounts launcher route | `scenario_adapter_mounts_launcher_route` | mount succeeds, route reports active |
| 3. adapter receives mock host snapshot | `scenario_adapter_receives_mock_host_snapshot` | snapshot ingestion succeeds |
| 4. adapter replays input | `scenario_adapter_replays_input` | pointer-down + pointer-up on launch button surfaces `ButtonClicked` |
| 5. adapter emits frame packet | `scenario_adapter_emits_frame_packet` | `FrameOutput` carries non-zero layers, hit/a11y caches populated |
| 6. adapter submits to fake renderer | `scenario_adapter_submits_to_fake_renderer` | fake renderer recorded `BeginFrame` and `FinishFrame` |
| 7. adapter disposes cleanly | `scenario_adapter_disposes_cleanly` | dispose clears active route + mount flag |

Plus six structural / safe-state tests:

- schema label / version stable;
- registry maps six well-known routes;
- registry returns `None` for `ProductReserved`;
- `tick` without active route returns
  `ProductRvelteDiagnostic::NoActiveRoute`;
- dispose of a `ProductReserved` route returns
  `ProductRvelteDiagnostic::UnknownRoute { route_id }`;
- input translation rejects oversized text payload as a typed
  diagnostic (no panic, no silent truncation).

All 13 tests pass.

## Validation

```text
cargo fmt -p fun-rvelte-bridge --check          # exit 0
cargo check -p fun-rvelte-bridge                 # exit 0
cargo clippy -p fun-rvelte-bridge --all-targets -- -D warnings   # exit 0
cargo test -p fun-rvelte-bridge                  # 13/13 passing
cargo tree -p fun-rvelte-bridge -e normal | grep -ciE "wgpu|vulkan|metal|dx12|d3d12|cef|swapchain|fun-renderer|fun_renderer"   # 0
```

Plus regression: `cargo test --workspace` from `rvelte/` reports
74 test groups, zero failures.

## Forbidden Surface

Pass-72 invariant: zero concrete renderer dependency.

- `Cargo.toml` does not list `fun-renderer`, `wgpu`, `ash`, `metal`,
  `windows`, `cef`, `web-sys`, or `wasm-bindgen` as a dependency.
- `cargo tree -p fun-rvelte-bridge -e normal | grep -ciE
  "wgpu|vulkan|metal|dx12|d3d12|cef|swapchain|fun-renderer|fun_renderer"`
  returns 0.
- The adapter is generic over `S: Renderer2DCommandSink`; only
  pass-73 will plug in a concrete `S`.

## Forward Pointers

Per the pass-71 design's migration plan:

- **Pass 73** — wire fun-renderer's `native_ui_adapter` feature as
  the concrete `Renderer2DCommandSink`. The adapter's public API
  does not change.
- **Pass 74** — replace the disk-backed scenario loader inside
  `ingest_host_snapshot` with a real `fun_host` transport.
- **Pass 75** — wire product input events from the existing
  game-client input pipeline.
- **Pass 76** — add `--rvelte-bridge=<route>` to the game-client
  binary so a single product binary can boot any of the six
  routes through the adapter.
- **Pass 77+** — staged removal of legacy CEF / browser UI.

Pass 72 explicitly does **not** authorize any change under
`fun/game_client/ui/main`, `fun/fun_ui_cef`, or `fun/fun_host`.

## Exit Criteria

- Product path has a native UI adapter skeleton:
  `fun/game_client/ui/rvelte_bridge/` is a registered workspace
  member with the public surface from the pass-71 design.
- Still no concrete renderer dependency: production tree contains
  zero matches for `wgpu|vulkan|metal|dx12|d3d12|cef|swapchain|
  fun-renderer|fun_renderer`.
