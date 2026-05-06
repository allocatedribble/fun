# fun_render Migration Inventory

status: inventory + pass3-diagnostics-facts
owner_repo: fun
captured_on: 2026-05-06
scope: fun_render, fun-renderer, fun-lux, fun-scene, game_client, fun_host, fun_ui_cef

This pass pins the renderer state before another architecture change. It is not
a proposal to keep the current product path. Current liabilities are recorded as
first-order migration blockers.

## Source Contract Read

- Root docs: `README.md`, `AGENTS.md`, `docs/workspace-map.md`,
  `docs/agent-protocol.md`, `docs/reversibility.md`,
  `docs/rust-standard.md`, `docs/diagnostics.md`,
  `docs/machine-workability.md`, and `docs/security-privacy.md`.
- FUN docs: `README.md`, `docs/client_benchmarking.md`,
  `docs/client_diagnostics.md`, `docs/renderer_ownership.md`,
  `docs/fun_scene_migration.md`, `docs/dx12_upload_audit.md`,
  `docs/dx12_implementation_doctrine.md`,
  `docs/dx12_parity_decision_pass.md`,
  `docs/dx12_cef_accelerated_paint.md`,
  `docs/dx12_descriptor_pipeline_churn.md`,
  `docs/dx12_shader_quality.md`, and
  `docs/dx12_dlss_boundary_gate.md`.
- Local evidence: `target/run-stack/cef-ui-transport.json` last written
  `2026-05-05 22:52:31`, `target/dx12-pix/pipeline_cardinality_report.md`,
  `target/dx12-parity/current/dx12_parity_report.{md,json}`, and the
  2026-05-04 CEF accelerated benchmark summary at
  `target/benchmarks/client/20260504-005500-247/summary.json`.

## Direct Answers

| question | current answer |
| --- | --- |
| What does `fun_render` own today? | The Bevy-facing product renderer plugin, Winit presentation, config/env parsing, Solari/cloud/meshlet integration, DX12 native interop gate, CEF texture composition bridge, diagnostics/benchmark counters, upload arena experiments, DLSS correctness/native-SR scaffolding, and the bridge re-export surface for `fun-renderer`, `fun-lux`, and `fun-scene`. |
| What does `fun-renderer` own today? | Real crate and compile-checked ownership contracts, ECS data/layout policy, render-world resources, no-op renderer-core API, no-op clear-color presentation interface, frame-graph model, GPU scene DB skeleton, and heuristic scheduler. It does not yet own the product swapchain or visible frame execution. |
| Does `fun-renderer` exist as code? | Yes. It is `fun/fun-renderer` with crate name `fun_renderer`; default features are now `bevy_ecs` and `fun_renderer_core`. DX12 native interop is an explicit boundary flag, not an implied default shipping capability. |
| Which path presents frames today? | `game_client` builds `FunRenderWinitPresentationPlugin` plus `FunRenderCorePlugin`. `FunRenderWinitPresentationPlugin` installs Bevy `DefaultPlugins`, `WindowPlugin`, selected DX12/Vulkan `RenderPlugin`, Winit, and render recovery. Product-visible presentation is still Bevy/wgpu through `fun_render`; `FUN_RENDERER_BACKEND=fun` initializes the no-op `fun-renderer`/`fun-lux` path but does not own the swapchain yet. |
| Can product UI run GPU-only today? | Not proven. The experimental D3D11On12 path exists, but the latest strict local status selected `disabled` with `fallback_reason=render_backend_not_dx12`; the latest non-strict live lane selected CPU fallback. Current product CEF composition still uses a Bevy UI `ImageNode` target. |
| Which runtime pipeline gates still fail? | The local pipeline cardinality report records runtime creation p95 maxima: render pipeline `22`, compute pipeline `82`, shader pipeline `104`. The DX12 perf gate treats runtime render, compute, and shader pipeline creation after warmup as hard failures. |
| Is DX12 real DX12 or fallback? | The Bevy/wgpu renderer can select DX12 through the stack profiles and `RenderPlugin`, and diagnostics report the requested backend. The CEF accelerated bridge is not ready: both local CEF artifacts report `backend=dx12` but `bridge_ready=false` and `fallback_reason=render_backend_not_dx12`. |
| What must migrate before DLSS/FSR/FG? | CEF GPU-only health, hot upload cleanup, runtime PSO/shader creation, PIX/barrier evidence, presentation matrix evidence, and renderer-owned scene/UI/upscale separation. `docs/dx12_dlss_boundary_gate.md` still says `DX12 baseline ready for DLSS SR bring-up: no`. |

## Crate Ownership Inventory

| crate/module | current owner | intended owner | migration status | current feature flags | validation commands |
| --- | --- | --- | --- | --- | --- |
| `fun_render` | Bevy/game renderer bridge, but still the product renderer brain for Winit presentation, Solari, meshlets, clouds, CEF texture composition, DX12 interop, DLSS scaffolding, diagnostics, and benchmark parsing. | Bevy/app bridge only: plugin registration, extraction, app integration, migration flags, diagnostics, benchmark hooks, legacy compatibility during transition. | Runtime selector is installed by `FunRenderCorePlugin`; `FUN_RENDERER_BACKEND=fun` boots the no-op core/lux path; product presentation still here. | `default=[offscreen,volumetric_clouds]`, `winit_presentation`, `render_diagnostics`, `diagnostics`, `dlss`, `dx12_dlss_native`, `dx12_native_interop`, `dx12_native_object_names`, `dx12_mesh_shader_experiment`, canonical renderer flags plus compatibility `fun_renderer_*`/`fun_lux_*` aliases. | `cargo check -p fun_render`; `cargo test -p fun_render --lib`; `cargo check -p fun_render --features dx12_native_interop`; targeted `cargo test -p fun_render pipeline_warmup --locked`. |
| `fun-renderer` / `fun_renderer` | Compile-checked renderer-core contracts, ECS schedule/data layout, GPU scene DB skeleton, frame graph skeleton, heuristic scheduler, no-op clear-color boot path, backend selector types, pass diagnostics, upload/CEF/upscale seams. | Default renderer core: GPU scene DB, frame graph execution, virtual geometry/shadows, page scheduler, CEF compositor, upscale/FG boundary, DX12/Vulkan backend abstraction. | Exists as real crate; product swapchain and visible backend execution not wired. | `default=[bevy_ecs,fun_renderer_core]`, `bevy_ecs`, `legacy_renderer`, `fun_renderer_core`, `dx12_native_interop`, `vulkan_backend`, `cef_gpu_only`, `upscaling`, `dlss`, `fsr`, `frame_generation`, `many_light`, `virtual_geometry`, `virtual_shadows`, `hybrid_gi`, `experimental_renderer_ml`; old names are aliases. | `cargo check -p fun-renderer`; `cargo test -p fun-renderer --lib`. |
| `fun-lux` / `fun_lux` | Lighting/GI policy descriptors, light DB API, no-op Lux core, ECS resources/systems for light/emissive/GI/shadow participant extraction and diagnostics. | Direct lighting, many-light sampling, virtual shadow policy, GI, reflections, denoising/reconstruction, radiance/surface/probe caches. | Exists as real crate; now exposes baseline no-op frame and shutdown reports for bridge boot. | `many_light`, `virtual_shadows`, `hybrid_gi`; no default features; old `fun_lux_*` names are aliases. | `cargo check -p fun-lux`; `cargo test -p fun-lux --lib`. |
| `fun-scene` / `fun_scene` | FUN-owned `fun!`/`fun_list!` authoring wrappers, scene manifests, stable identity, networking/streaming primitives, renderer/lux/UI/upscale authoring components, editor operations. | First-party scene DSL, deterministic scene authority, streaming manifests, renderer/lux declarations, editor/server/client shared scene substrate. | Exists and migrated into `game_scene`; scene component names are domain names without product prefixes. | No feature flags. Depends on Bevy ECS/scene/transform/camera/color and `fun-scene-macros`. | `cargo check -p fun-scene`; `cargo test -p fun-scene --lib`; `tools/check_fun_scene_migration.ps1`. |
| `game_client` | Runtime app, Winit client, server connection, game/editor host modes, CEF bridge, CEF DX12 accelerated interop module, Svelte host page integration, benchmark/runtime diagnostics. | Product client using `fun_render` bridge, CEF/Svelte product UI, and later `fun-renderer` visible path. | Current frame path is legacy Bevy/wgpu through `fun_render`; CEF UI can be compiled but uses Bevy UI image composition. | `default=[dlss,volumetric_clouds]`, `cef_ui`, `cef_ui_dx12_accelerated_paint`, `dx12_native_object_names`, `render_diagnostics`, `diagnostics`, `benchmarks`, `dlss`, `dx12_dlss_native`, `force_disable_dlss`. | `cargo check -p game_client`; `cargo check -p game_client --no-default-features --features cef_ui --locked`; `cargo check -p game_client --no-default-features --features cef_ui_dx12_accelerated_paint --locked`; `scripts/run_stack.ps1 -RenderBackend dx12 -PresentMode immediate`. |
| `fun_host` | Rust-owned host/launcher/editor command authority, current-client preview state, command routing to CEF/Svelte. | Rust authoritative host bridge for Svelte/CEF, not a renderer. | Active; editor preview is current-client metadata, not child process or HWND path. | No explicit feature flags. | `cargo check -p fun_host`; `cargo test -p fun_host --lib`. |
| `fun_ui_cef` | CEF runtime, browser lifetime, offscreen browser settings, JavaScript bridge, security validation, CPU paint compositor, accelerated callback surface and counters. | Browser subsystem only: callbacks/transport policy/events, while `fun-renderer` owns final GPU compositor contract. | Active; accelerated callback surface exists; CPU `OnPaint` path remains compiled as compatibility lane. | `debug_remote`; no default features. | `cargo check -p fun_ui_cef`; `cargo test -p fun_ui_cef --lib`. |
| `game_scene` | Game-specific scene catalog over `fun_scene`, default arenas, deterministic manifests/chunks. | Game-specific catalog only; generic scene authority stays in `fun-scene`. | Migrated off direct `bsn!` usage. | No explicit feature flags. | `cargo test -p game_scene --locked`; `tools/check_fun_scene_migration.ps1`. |
| `fun_dx12_dlss` | Fail-closed native Windows DLSS C ABI scaffold and runtime discovery surface. | Native bridge crate consumed by `fun-renderer`/`fun_render` after DX12 baseline is ready. | Scaffolded; Streamline/NGX evaluation not linked. | No listed package-level feature flags in this inventory pass. | `cargo check -p fun_dx12_dlss`; DLSS SR acceptance remains blocked by boundary gate. |

## Pass 2 Runtime Selector

`FUN_RENDERER_BACKEND` is now parsed into
`fun_renderer::FunRendererBackendSelection` and installed by
`fun_render::RendererBridgeSettings::from_env`.

| env value | requested | current resolved path | diagnostic policy | notes |
| --- | --- | --- | --- | --- |
| unset | `auto` | `legacy` | loud | Preserves the current Bevy/wgpu product presentation path for this pass. |
| `auto` | `auto` | `legacy` | loud | Explicitly asks for transition policy selection. |
| `legacy` | `legacy` | `legacy` | loud | Temporary escape hatch for one migration cycle. |
| `fun` | `fun` | `fun` | normal info | Boots `NoopRendererCore`, produces and presents a clear-color no-op frame, calls `NoopLuxCore`, records diagnostics, and shuts down cleanly. |
| invalid value | `auto` | `legacy` | loud | Invalid values are treated as accidental fallback and must be visible in diagnostics. |

The exact future default flip point is
`fun_render::bridge::RendererBridgeSettings::from_env`. Changing that function
from current `auto -> legacy` policy to default `fun` is the handoff point.

The no-op `fun` path is intentionally not product-visible yet. It proves the
bridge/core/lux crate boundary and lifecycle without taking the swapchain away
from the current known-booting Bevy/wgpu path.

## Pass 2 Feature Map

Canonical feature names:

| feature | owner | current meaning |
| --- | --- | --- |
| `legacy_renderer` | `fun_render` + `fun-renderer` | Temporary legacy backend routing capability. |
| `fun_renderer_core` | `fun-renderer` | Compile the renderer-core boundary and no-op boot path. |
| `dx12_native_interop` | `fun_render` + `fun-renderer` | DX12 native handle/interop boundary, explicit opt-in only. |
| `vulkan_backend` | `fun-renderer` | Vulkan backend abstraction hook. |
| `cef_gpu_only` | `fun-renderer` | CEF GPU-only compositor policy hook. |
| `upscaling` | `fun-renderer` | Presentation/upscale boundary hook. |
| `dlss` | `fun_render` + `fun-renderer` | DLSS wiring flag; still blocked by boundary gate for product claims. |
| `fsr` | `fun-renderer` | FSR wiring hook. |
| `frame_generation` | `fun-renderer` | Frame-generation boundary hook. |
| `many_light` | `fun-lux` | Many-light policy hook. |
| `virtual_geometry` | `fun-renderer` | Virtual geometry policy hook. |
| `virtual_shadows` | `fun-renderer` + `fun-lux` | Virtual shadow policy hook. |
| `hybrid_gi` | `fun-lux` | Hybrid GI policy hook. |
| `experimental_renderer_ml` | `fun-renderer` | Renderer-side model request hook only; reusable model runtime remains in `fun-ai`. |

Compatibility aliases retained for one transition cycle:
`fun_renderer_legacy`, `fun_renderer_new_core`, `fun_renderer_dx12`,
`fun_renderer_vulkan`, `fun_renderer_cef_gpu_only`, `fun_renderer_upscale`,
`fun_renderer_dlss`, `fun_renderer_fsr`, `fun_renderer_frame_generation`,
`fun_renderer_experimental_ml`, `fun_lux_many_light`,
`fun_lux_virtual_shadows`, and `fun_lux_hybrid_gi`.

## Pass 3 Renderer Facts

`fun_render` now emits a startup capability report with schema
`fun.renderer.capability_report.v1`. The report is a render-world resource and
can also be written to JSON by setting
`FUN_RENDERER_CAPABILITY_REPORT_PATH`. `scripts/run_stack.ps1` wires this to
`target/run-stack/renderer-capabilities.json` for client runs, and
`scripts/benchmark_client.ps1` embeds the report under
`renderer_capability_report` in benchmark summaries.

The report records:

- selected and actual renderer lane;
- selected and actual graphics backend plus mismatch reason;
- adapter vendor/device/type with adapter and driver names hashed;
- Bevy backend capability hash and capability matrix;
- DX12 native handle support, Vulkan support, D3D11On12 fallback state, CEF
  shared-texture support, bindless/descriptor-indexing support, mesh shader,
  ray tracing, HDR/swapchain placeholders, DLSS, FSR, frame-generation, and
  pipeline warmup state;
- active bridge feature flags and non-empty renderer/CEF/DLSS environment
  overrides.

CEF product transport now fails closed by default. Accelerated `auto` or
`d3d11on12` requests select `disabled` when the GPU transport prerequisites are
not ready. CPU fallback is only re-enabled by the explicit diagnostic/test
override `FUN_CEF_UI_ALLOW_CPU_FALLBACK=1`; stack scripts clear that override by
default. This keeps product lanes from silently treating CPU `OnPaint` uploads
as a working accelerated UI path.

`tools/dx12_parity_report.py` reads the embedded capability report and adds a
`Renderer Capability Report` table to the dashboard. Accelerated CEF lanes now
fail if the capability report says CPU runtime fallback is allowed, if CPU
upload bytes are nonzero, or if the transport health decision is `disabled` or
`fallback`. Backend summaries prefer explicit selected/actual backend facts
from the capability report, so a report cannot claim DX12 while the runtime
adapter says Vulkan.

### Pass 3 Runtime Evidence

Validation produced a current Vulkan startup capability artifact at
`target/run-stack/renderer-capabilities.json` with
`selected_graphics_backend=vulkan`, `actual_graphics_backend=vulkan`,
`d3d11on12_fallback_state=cpu_fallback_forbidden`, and
`cef_accelerated_shared_texture_support=fallback_reason=render_backend_not_dx12`.

Fresh DX12/Vulkan benchmark matrix artifacts could not be completed in the
current local repo state:

- the Vulkan runtime smoke repeatedly hit existing compute-culling pipeline
  validation failures for `fun_compute_culling_*` pipelines before benchmark
  samples were produced;
- the strict DX12 CEF stack launch was blocked during build by the dirty sibling
  `fun-warden` checkout: `WardenDateBucket` is missing `Hash`, and
  `WardenAccountVariantRoot` is now a record struct while one call still uses
  tuple-struct construction.

These blockers match the migration doctrine that runtime pipeline creation and
CEF transport readiness are first-order constraints, not distant cleanup.

## Renderer Module Inventory

| area | current files/tools | current owner | intended owner | status |
| --- | --- | --- | --- | --- |
| Product presentation | `game_client/src/lib.rs`, `fun_render/src/winit.rs`, `fun_render/src/core.rs` | `game_client` + `fun_render` | `fun-renderer` core through `fun_render` bridge | Current visible frame path. |
| Renderer-core seams | `fun-renderer/src/{lib.rs,api.rs,ecs.rs,heuristics.rs}` | `fun-renderer` | `fun-renderer` | Buildable substrate, not product presentation. |
| Lighting/Lux seams | `fun-lux/src/{lib.rs,api.rs}` | `fun-lux` | `fun-lux` | Buildable substrate and ECS extraction/update hooks. |
| Scene substrate | `fun-scene/src/*`, `fun-scene-macros/src/*`, `game_scene/src/*` | `fun-scene` + `game_scene` | same split | Active and first-party. |
| CEF/Svelte product UI | `game_client/ui/main`, `game_client/src/cef_ui.rs`, `fun_ui_cef/src/*`, `fun_host/src/lib.rs` | `game_client`, `fun_ui_cef`, `fun_host` | CEF/Svelte UI with renderer-owned GPU compositor | UI is active, but final composition still uses Bevy UI image node. |
| CEF DX12 accelerated transport | `game_client/src/cef_ui_dx12/*`, `fun_render/src/dx12_native/cef.rs`, `docs/dx12_cef_accelerated_paint.md` | `game_client` interop over `fun_render::dx12_native` | `fun-renderer` compositor/backend boundary with `fun_ui_cef` callbacks | Partially implemented; latest local status blocked. |
| DX12 parity/benchmark tools | `scripts/benchmark_dx12_parity.ps1`, `scripts/benchmark_client.ps1`, `tools/dx12_*`, `tools/check_dx12_*` | `fun` tooling | remains benchmark/governance tooling | Active. |
| CEF parity/visual checks | `tools/compare_cef_ui_screenshots.ps1`, `scripts/stack/profiles/cef.*.json`, `scripts/benchmark_dx12_parity.ps1 -MatrixSize cef_transport` | `fun` tooling | remains benchmark/governance tooling | Plan and health parsing exist; full healthy matrix blocked. |
| Upload arena and upload reports | `fun_render/src/upload_{arena,budget,labels,ranges,report}.rs`, `fun_render/src/instance_tables.rs`, `docs/dx12_upload_audit.md` | `fun_render` | small-buffer policy should move toward renderer core once measured owners are known | Boundary exists; top offenders still generic Bevy write helpers. |
| Pipeline diagnostics | `fun_render/src/pipeline_warmup.rs`, `docs/dx12_descriptor_pipeline_churn.md`, `docs/dx12_shader_quality.md`, `tools/dx12_pipeline_cardinality_report.py` | `fun_render` + Bevy diagnostics | renderer diagnostics through `fun-renderer`/engine hooks | Runtime creation still measured after warmup. |
| Native interop | `fun_render/src/dx12_native/*`, `game_client/src/cef_ui_dx12/*` | `fun_render` interop gate, `game_client` CEF bridge | `fun-renderer` backend abstraction after migration | Centralized gate exists; raw command-list accessor still intentionally limited. |

## Known Blockers

| blocker | current evidence | owner now | migration impact | next proof |
| --- | --- | --- | --- | --- |
| `FunUploadArena` boundary | `fun_render::FunUploadArena` wraps `wgpu::util::StagingBelt` and tracks/budgets aligned buffer writes. `docs/dx12_upload_audit.md` says not to force Bevy prepare-stage `RenderQueue` helpers through ad hoc encoders. | `fun_render` | Required before moving hot small-buffer upload ownership into `fun-renderer`. | Top semantic owners for `DynamicUniformBuffer`/`RawBufferVec` rows, then measured before/after with no submit regression. |
| DX12 upload kill-list | Current parity dashboard lists top offenders under Bevy generic rows, including `uniform_buffer.rs:311`, `buffer_vec.rs:183`, `gpu_image.rs:84`, and Solari constant rows. | Bevy + `fun_render` diagnostics | Prevents speculative upload rewrites. | Label split or owning-system attribution for top rows. |
| CEF accelerated lane classified `cef_transport_bound` | Root ledger and CEF docs record the failed accelerated lane; `target/benchmarks/client/20260504-005500-247/summary.json` selected CPU fallback with `fallback_reason=render_backend_not_dx12`, `bridge_ready=false`, zero GPU copy bytes, and zero accelerated paint FPS. | `game_client` CEF DX12 bridge over `fun_render::dx12_native` | Blocks GPU-only product UI and DLSS/upscale boundary proof. | Strict D3D11On12 lane with `selected=d3d11on12`, `bridge_ready=true`, `cef_cpu_upload_bytes=0`, nonzero `cef_gpu_copy_bytes`, no normal-frame blocking waits. |
| Strict D3D11On12 lane disabled | `target/run-stack/cef-ui-transport.json` last written 2026-05-05 reports `requested=d3d11on12`, `selected=disabled`, `backend=dx12`, `bridge_ready=false`, `cpu_fallback_enabled=false`, `strict=true`, `fallback_reason=render_backend_not_dx12`. | `game_client` startup/interop | Confirms strict GPU-only UI currently fails closed instead of presenting UI. | Fix backend/bridge readiness detection, then rerun strict stack lane. |
| Runtime render pipeline creation | `target/dx12-pix/pipeline_cardinality_report.md` reports render pipeline p95 `22`. | Bevy pipeline cache + `fun_render` warmup | DX12 perf gate hard failure; blocks DLSS/FG claims. | `FUN_RENDER_PIPELINE_WARMUP=observed` lane with render pipeline creations at steady-state zero or justified scene-transition rows. |
| Runtime compute pipeline creation | Same report records compute pipeline p95 `82`. | Bevy/Solari/meshlet pipelines + `fun_render` warmup | DX12 perf gate hard failure; indicates PSO churn or late warmup. | Creation-focused events plus warmup comparison; reduce late compute variants. |
| Runtime shader pipeline creation | Same report records shader pipeline create p95 `104`, led by PBR, Solari, meshlet, and UI families. | Bevy/Solari/meshlet/UI pipeline families | DX12 perf gate hard failure; UI family also keeps Bevy UI cost visible. | `render_shader_*` counters zero after warmup or explicitly bound to scene load. |
| DLSS boundary gate not ready | `docs/dx12_dlss_boundary_gate.md` says `DX12 baseline ready for DLSS SR bring-up: no`. CEF health, uploads, barriers, and pipeline creation are not ready. | `fun_render` DLSS scaffold today; intended `fun-renderer` presentation boundary | DLSS/FSR/FG cannot be used for performance claims. | Gate flips to `yes` with attached parity, CEF GPU, upload, PIX/barrier, present, and pipeline evidence. |
| Runtime Bevy UI product dependency | Current CEF and FPS paths spawn Bevy UI `Node`, `ImageNode`, `Text`, and `FpsOverlayPlugin`. | `game_client` + `fun_render` | Violates long-term product UI rule; keeps UI pipeline creation in frame path. | Replace product UI composition with renderer-owned CEF compositor and renderer debug primitives/CEF diagnostics. |

## Bevy UI Usage Map

| path | usage | classification | reason | action |
| --- | --- | --- | --- | --- |
| `game_client/src/cef_ui.rs` `sync_cef_ui_image_node` | Spawns full-window `Node` + `ImageNode` named `CEF UI Render Texture`. | product-forbidden | This is the current product CEF presentation surface and is compiled with `game_client/cef_ui`. | Replace with renderer-owned CEF compositor pass outside Bevy UI. |
| `game_client/src/cef_ui.rs` `upload_cef_ui_frame_to_fun_texture` | Updates Bevy `Image` handle and writes CPU paint fallback texture uploads; GPU token path still targets same Bevy image. | product-forbidden | UI pixels still flow through Bevy image/UI composition. CPU fallback remains a runtime lane unless strict mode fails closed. | Move CPU fallback to test-only/golden fixtures; require GPU transport in product. |
| `game_client/src/cef_ui.rs` `update_fun_client_fps_counter` | Spawns `Node`, `BackgroundColor`, `Text`, `TextFont`, `TextColor`. | must migrate to CEF/Svelte or renderer debug primitive | Runtime/product debug overlay uses Bevy UI. | Surface FPS through CEF/Svelte diagnostics or renderer debug text. |
| `fun_render/src/core.rs` | Adds Bevy `FpsOverlayPlugin` under `debug_assertions` when overlay is enabled and not editor preview. | debug-only transition | Not release/product by default, but still Bevy UI. | Keep only during transition; replace with CEF/Svelte or renderer diagnostics. |
| Workspace `Cargo.toml` patch table | Patches `bevy_ui`, `bevy_ui_render`, and `bevy_ui_widgets` for local Bevy fork. | not a product usage by itself | Patch table exposes local path crates; usage depends on Bevy default plugins and product code. | Do not use this as acceptance proof; block product imports/usages separately. |

No Bevy UI dependency was found in `fun_host` or `fun_ui_cef` Rust code during
this pass. `fun_ui_cef` owns browser surfaces, not Bevy UI.

## CEF/Svelte Product Path

| layer | current status |
| --- | --- |
| Svelte app | Active under `game_client/ui/main`; package name `fun-client-ui`; scripts are `dev`, `build`, `preview`, and `check`; UI dependencies are Svelte/Vite/TypeScript/Bulma. |
| Rust host authority | `fun_host` exposes launcher/editor/preview command descriptors and current-client preview status; CEF command surface readiness is part of host status. |
| CEF runtime | `fun_ui_cef` owns CEF browser bootstrap/runtime, windowless browser settings, JS bridge, input validation, CPU paint compositor, accelerated callback surface, diagnostics, and security checks. |
| Game integration | `game_client/src/cef_ui.rs` installs `GameCefUiPlugin`, starts CEF, routes input/model patches/host commands, tracks transport counters, and creates the render texture. |
| Current composition | Bevy UI full-window `ImageNode` sampling a Bevy `Image`; CPU path uses `RenderQueue::write_texture`; experimental GPU path copies into the same Bevy image. |
| Accelerated transport | Partially implemented. `game_client/src/cef_ui_dx12` opens CEF D3D11 shared textures, copies through D3D11On12 into a FUN-owned D3D12 ring, and publishes safe tokens when bridge initialization succeeds. Latest artifacts show bridge readiness failing. |
| Runtime CPU fallback | Present today in non-strict lanes. Long-term product rule rejects this; only test-only golden-image fixtures should retain CPU `OnPaint` uploads. |
| GPU-only product readiness | Blocked until strict accelerated lane presents UI with zero CPU upload bytes and the composition target is not Bevy UI. |

## DX12 And Vulkan Status

| item | DX12 current status | Vulkan current status |
| --- | --- | --- |
| Backend selection | Stack profiles and scripts default to `dx12`/`immediate`; `fun_render/src/winit.rs` selects Bevy render plugin backend from env/profile. | Supported by parity scripts and `default.vulkan.immediate.json`. |
| Product visible renderer | Bevy/wgpu through `fun_render`, not `fun-renderer` backend abstraction. | Same product path, different backend. |
| Native interop | Centralized in `fun_render::dx12_native`; CEF/DLSS use this gate. | No equivalent native feature gate in this pass. |
| CEF accelerated transport | Blocked: bridge readiness currently fails with `render_backend_not_dx12` even when the run requests DX12. | Not applicable; accelerated path is Windows/D3D11On12/DX12-specific. |
| Parity evidence | Selected local matrix exists; current JSON recommendation is `runtime_pipeline_creation_bound` with high confidence. | Used as control lane in parity matrix. |
| Present decision | Do not change defaults until full present matrix and latency evidence are complete. | Control lane required before changing DX12 defaults. |
| Barrier evidence | Blocked on PIX CSV/capture rows. | Not the target of PIX DX12 barrier audit. |

## Validation Command Inventory

| purpose | command |
| --- | --- |
| Renderer core compile | `cargo check -p fun-renderer` |
| Renderer core tests | `cargo test -p fun-renderer --lib` |
| Lux compile/tests | `cargo check -p fun-lux`; `cargo test -p fun-lux --lib` |
| Bridge compile/tests | `cargo check -p fun_render`; `cargo test -p fun_render --lib` |
| Client compile | `cargo check -p game_client` |
| CEF CPU lane compile | `cargo check -p game_client --no-default-features --features cef_ui --locked` |
| CEF accelerated lane compile | `cargo check -p game_client --no-default-features --features cef_ui_dx12_accelerated_paint --locked` |
| Scene migration check | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_fun_scene_migration.ps1 -SelfTest`; then without `-SelfTest` |
| DX12 doctrine check | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_dx12_doctrine.ps1 -SelfTest`; then without `-SelfTest` |
| DX12 perf gate parser | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_dx12_perf_regression.ps1 -SelfTest` |
| DX12 parity plan | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/benchmark_dx12_parity.ps1 -PlanOnly` |
| CEF transport plan | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/benchmark_dx12_parity.ps1 -MatrixSize cef_transport -PlanOnly` |
| Runtime smoke | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run_stack.ps1 -RenderBackend dx12 -PresentMode immediate` |
| Strict CEF GPU proof | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/run_stack.ps1 -RenderBackend dx12 -PresentMode immediate -CefUi -CefPaintTransport d3d11on12 -CefAcceleratedStrict` |
| Client benchmark | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/benchmark_client.ps1 -RenderBackend dx12 -PresentMode immediate` |
| Required performance lanes | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/benchmark_required_lanes.ps1 -RenderBackend dx12 -PresentMode immediate` |
| Doc-only whitespace | `git diff --check` |

## First Rollback Strategy

Pass 2 adds typed selector and no-op boot behavior. Rollback is the nested
`fun` commit that changes `fun-renderer`, `fun-lux`, `fun_render`, and this
document, followed by the umbrella root commit that advances the `fun` gitlink
and appends the ledger entry.

For future behavior passes, the first rollback path remains:

1. Set `FUN_RENDERER_BACKEND=legacy` while the transition flag exists.
2. Disable CEF product UI with `-CefPaintTransport disabled` for render-only
   diagnosis, or use CPU CEF only in explicit compatibility/debug lanes.
3. Revert the nested `fun` behavior commit before the umbrella root gitlink
   commit.
4. Re-run the narrow validation lane that originally proved the change.

## Immediate Migration Implications

- Do not build DLSS/FSR/frame generation on the current baseline. The gate is
  explicitly not ready.
- Do not claim GPU-only product UI until strict D3D11On12 CEF presents through
  a non-Bevy-UI compositor with zero CPU upload bytes.
- Do not move generic Bevy upload helpers into `FunUploadArena` until the
  semantic owner and render-schedule insertion point are known.
- Do not treat `fun-renderer` as product-visible just because it compiles. It is
  the intended default renderer core, but the visible swapchain is still Bevy
  Winit/wgpu through `fun_render`.
- Bevy ECS is already the intended data substrate and is already compiled into
  `fun-renderer`; the next runtime migration should connect visible frame
  ownership without breaking that ECS-first shape.
