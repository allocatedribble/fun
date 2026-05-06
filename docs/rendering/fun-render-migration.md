# fun_render Migration Inventory

status: inventory + pass20-quality-settings-ladder
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
  `target/dx12-parity/current/dx12_parity_report.{md,json}`,
  `target/benchmarks/client/20260506-005505-031/summary.json`,
  `target/benchmarks/client/20260506-011124-520/summary.json`,
  `target/dx12-upload/pass5-resource-ownership/upload_perf_report.{md,json}`,
  `target/frame-graph/pass6/renderer_frame_graph_debug.txt`,
  and the
  2026-05-04 CEF accelerated benchmark summary at
  `target/benchmarks/client/20260504-005500-247/summary.json`.

## Direct Answers

| question | current answer |
| --- | --- |
| What does `fun_render` own today? | The Bevy-facing product renderer plugin, Winit presentation, config/env parsing, Solari/cloud/meshlet integration, DX12 native interop gate, CEF texture composition bridge, diagnostics/benchmark counters, upload arena experiments, DLSS correctness/native-SR scaffolding, and the bridge re-export surface for `fun-renderer`, `fun-lux`, and `fun-scene`. |
| What does `fun-renderer` own today? | Real crate and compile-checked ownership contracts, ECS data/layout policy, render-world resources, no-op renderer-core API, no-op clear-color presentation interface, renderer-owned frame graph with pass/resource declarations and validation, renderer-owned GPU scene database records with generation-checked handles and dirty uploads, resource ownership policy, shared page scheduler, static virtual geometry asset/runtime substrate, dynamic geometry/procedural submission substrate, page-based virtual shadow storage, shared heuristic scheduler, quality/settings schema, capability-aware default selector, renderer-owned upscaling/frame-generation diagnostics, and renderer-facing ML request/fallback interfaces. It does not yet own the product swapchain or visible frame execution. |
| Does `fun-renderer` exist as code? | Yes. It is `fun/fun-renderer` with crate name `fun_renderer`; default features are now `bevy_ecs` and `fun_renderer_core`. DX12 native interop is an explicit boundary flag, not an implied default shipping capability. |
| Which path presents frames today? | `game_client` builds `FunRenderWinitPresentationPlugin` plus `FunRenderCorePlugin`. `FunRenderWinitPresentationPlugin` installs Bevy `DefaultPlugins`, `WindowPlugin`, selected DX12/Vulkan `RenderPlugin`, Winit, and render recovery. Product-visible presentation is still Bevy/wgpu through `fun_render`; `FUN_RENDERER_BACKEND=fun` initializes the no-op `fun-renderer`/`fun-lux` path but does not own the swapchain yet. |
| Can product UI run GPU-only today? | The product code path is now GPU-only/fail-closed: CEF CPU `OnPaint` frames are rejected, Bevy UI image composition has been removed from `game_client`, and `fun-renderer::RendererCefCompositor` owns the late UI layer contract. Runtime proof is still blocked until the strict D3D11On12 lane reports `bridge_ready=true` with nonzero GPU copy bytes. |
| Which runtime pipeline gates still fail? | The older local pipeline cardinality report records runtime creation p95 maxima: render pipeline `22`, compute pipeline `82`, shader pipeline `104`. Pass 4 adds a `fun-renderer` pipeline registry plus a bridge warmup plan, fixes the compute-culling read-only storage binding mismatch, and records a new selected-DX12/actual-Vulkan smoke artifact with render, compute, and shader pipeline creation p95 all `0` after warmup. A true DX12 artifact is still required because capability diagnostics report `actual_backend=vulkan`. |
| Is DX12 real DX12 or fallback? | Pass 9 makes this a runtime truth contract instead of a requested-backend guess. The latest local smoke requested and selected DX12, but the renderer capability report says `actual_graphics_backend=vulkan`, `fallback_graphics_backend=vulkan`, and `graphics_backend_truth_state=actual_backend_mismatch`. This blocks CEF GPU transport and all premium rendering gates. |
| What must migrate before DLSS/FSR/FG? | CEF GPU-only health, hot upload cleanup, runtime PSO/shader creation, PIX/barrier evidence, presentation matrix evidence, backend truth, and live renderer-owned presentation. Pass 16 adds the renderer-owned scene/UI/upscale contract and Pass 17 adds the FG eligibility contract, but `docs/dx12_dlss_boundary_gate.md` still says `DX12 baseline ready for DLSS SR bring-up: no`. |

## Crate Ownership Inventory

| crate/module | current owner | intended owner | migration status | current feature flags | validation commands |
| --- | --- | --- | --- | --- | --- |
| `fun_render` | Bevy/game renderer bridge, but still the product renderer brain for Winit presentation, Solari, meshlets, clouds, CEF texture composition, DX12 interop, DLSS scaffolding, diagnostics, and benchmark parsing. | Bevy/app bridge only: plugin registration, extraction, app integration, migration flags, diagnostics, benchmark hooks, legacy compatibility during transition. | Runtime selector is installed by `FunRenderCorePlugin`; `FUN_RENDERER_BACKEND=fun` boots the no-op core/lux path; product presentation still here. | `default=[offscreen,volumetric_clouds]`, `winit_presentation`, `render_diagnostics`, `diagnostics`, `dlss`, `dx12_dlss_native`, `dx12_native_interop`, `dx12_native_object_names`, `dx12_mesh_shader_experiment`, canonical renderer flags plus compatibility `fun_renderer_*`/`fun_lux_*` aliases. | `cargo check -p fun_render`; `cargo test -p fun_render --lib`; `cargo check -p fun_render --features dx12_native_interop`; targeted `cargo test -p fun_render pipeline_warmup --locked`. |
| `fun-renderer` / `fun_renderer` | Compile-checked renderer-core contracts, ECS schedule/data layout, GPU scene DB records, frame graph skeleton, ECS heuristic systems, shared scheduler priority language, quality/settings schema, capability-aware defaults, no-op clear-color boot path, backend selector types, pass diagnostics, renderer resource ownership policy, shared page scheduler, static virtual geometry format/runtime, dynamic geometry/procedural submission substrate, page-based virtual shadow storage, renderer-owned CEF compositor, upload/upscale seams, `upscaling::Upscaler*` selection/evaluation diagnostics, `frame_generation::FrameGeneration*` eligibility/disable diagnostics, and `ml::RendererPredictionClient` request/fallback scaffolding. | Default renderer core: GPU scene DB, frame graph execution, virtual geometry/shadows, dynamic/procedural geometry submission, shared heuristic scheduler, quality/settings policy, page scheduler, CEF compositor, resource allocation ownership, upscale/FG boundary, DX12/Vulkan backend abstraction. | Exists as real crate; product swapchain and visible backend execution not wired. Pass 11 adds static virtual geometry format/runtime. Pass 12 adds `DynamicGeometrySubmission`, `DynamicGeometryDatabase`, classic mesh packets, optional dynamic cluster packets for selected classes, procedural chunk revision invalidation, destruction-fragment stress accounting, and upload/dirty-record diagnostics. Pass 13 adds `VirtualShadowStorage`, directional clipmap/local-light page pools, shadow page-table records, invalidation reasons, refresh prioritization, cache/miss diagnostics, and a stable shadow-page artifact. Pass 16 adds the neutral upscaler interface, native/debug fallback mode, DLSS SR and FSR readiness hooks, motion-vector validation, UI separation rejection, and a stable upscaling benchmark artifact. Pass 17 adds the neutral FG interface, DLSS/FSR FG readiness hooks, disable reasons, present-resource lifetime checks, UI readability policy, editor/game policy, and a stable FG benchmark artifact. Pass 18 adds optional `experimental_renderer_ml` shadow-page-prior request/evaluation hooks while keeping reusable model runtime ownership in `fun-ai`. Pass 19 adds `scheduler::SharedHeuristicScheduler`, normalized priority inputs, system integration hooks, shared budget recommendations, diagnostics heatmaps, debug overlays, and p95/p99 benchmark artifacts. Pass 20 adds `settings::RendererSettingsRequest`, the Baseline/Hybrid/High/RT-assisted/Vendor-enhanced/Experimental ladder, capability-aware default/rejection selection, reproducible settings artifacts, and a typed CEF/Svelte UI model via `fun_render`. | `default=[bevy_ecs,fun_renderer_core]`, `bevy_ecs`, `legacy_renderer`, `fun_renderer_core`, `dx12_native_interop`, `vulkan_backend`, `cef_gpu_only`, `upscaling`, `dlss`, `fsr`, `frame_generation`, `many_light`, `virtual_geometry`, `virtual_shadows`, `hybrid_gi`, `experimental_renderer_ml`; old names are aliases. | `cargo check -p fun-renderer`; `cargo check -p fun-renderer --features experimental_renderer_ml`; `cargo check -p fun-renderer --features upscaling,dlss,fsr,frame_generation,cef_gpu_only`; `cargo test -p fun-renderer --lib settings`; `cargo test -p fun-renderer --lib scheduler`; `cargo test -p fun-renderer --features experimental_renderer_ml --lib ml`; `cargo test -p fun-renderer --lib`; `cargo test -p fun-renderer --lib frame_generation`; `cargo test -p fun-renderer --lib upscaling`; `cargo test -p fun-renderer --lib frame_graph`; `cargo test -p fun-renderer --lib scene`; `cargo test -p fun-renderer --lib virtual_geometry`; `cargo test -p fun-renderer --lib dynamic_geometry`; `cargo test -p fun-renderer --lib virtual_shadow`; `cargo test -p fun-renderer --lib cef`; `cargo test -p fun_render --no-default-features --lib settings_bridge`; `cargo test -p fun_render --lib extraction`. |
| `fun-lux` / `fun_lux` | Lighting/GI policy descriptors, light DB API, no-op Lux core, ECS resources/systems for light/emissive/GI/shadow participant extraction and diagnostics, many-light candidate/reservoir policy, hybrid GI/reflection policy, and shadow policy decisions separate from renderer storage. | Direct lighting, many-light sampling, virtual shadow policy, GI, reflections, denoising/reconstruction, radiance/surface/probe caches. | Exists as real crate; now exposes baseline no-op frame and shutdown reports for bridge boot. Pass 13 adds `shadow::ShadowPolicyEngine` for caster selection, quality tier, soft-shadow mode, reconstruction mode, directional clipmap budget, local-light shadow budget, and policy diagnostics. Pass 14 adds `many_light::GpuLightDatabase`, clustered/Forward+ candidate lists, temporal/spatial reservoirs, emissive promotion, budgeted shadow request intents, and benchmark/shadow/overlay artifacts. Pass 15 adds `gi::SurfaceCache`, GI quality tiers, persistent cache invalidation/update lifecycle, reflection source selection, and GI/reflection debug artifacts. | `many_light`, `virtual_shadows`, `hybrid_gi`; no default features; old `fun_lux_*` names are aliases. | `cargo check -p fun-lux`; `cargo test -p fun-lux --lib`; `cargo test -p fun-lux --lib shadow`; `cargo test -p fun-lux --lib many_light`; `cargo test -p fun-lux --lib gi`. |
| `fun-scene` / `fun_scene` | FUN-owned `fun!`/`fun_list!` authoring wrappers, scene manifests, stable identity, networking/streaming primitives, renderer/lux/UI/upscale authoring components, editor operations. | First-party scene DSL, deterministic scene authority, streaming manifests, renderer/lux declarations, editor/server/client shared scene substrate. | Exists and migrated into `game_scene`; scene component names are domain names without product prefixes. | No feature flags. Depends on Bevy ECS/scene/transform/camera/color and `fun-scene-macros`. | `cargo check -p fun-scene`; `cargo test -p fun-scene --lib`; `tools/check_fun_scene_migration.ps1`. |
| `game_client` | Runtime app, Winit client, server connection, game/editor host modes, CEF bridge, CEF DX12 accelerated interop module, Svelte host page integration, benchmark/runtime diagnostics. | Product client using `fun_render` bridge, CEF/Svelte product UI, and later `fun-renderer` visible path. | Current frame path is legacy Bevy/wgpu through `fun_render`; CEF UI now publishes accelerated ready-frame tokens into `RendererCefCompositor` and rejects CPU `OnPaint` product fallback. | `default=[dlss,volumetric_clouds]`, `cef_ui`, `cef_ui_dx12_accelerated_paint`, `dx12_native_object_names`, `render_diagnostics`, `diagnostics`, `benchmarks`, `dlss`, `dx12_dlss_native`, `force_disable_dlss`. | `cargo check -p game_client`; `cargo check -p game_client --no-default-features --features cef_ui --locked`; `cargo check -p game_client --no-default-features --features cef_ui_dx12_accelerated_paint --locked`; `tools/check_product_ui_policy.ps1`; `scripts/run_stack.ps1 -RenderBackend dx12 -PresentMode immediate`. |
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

## Pass 4 Pipeline Registry

`fun-renderer` now exposes `pipeline::PipelineRegistry` as the renderer-core
pipeline inventory. Each registered pipeline has:

- stable ID;
- static label;
- render or compute kind;
- shader path and entry point;
- shader variant ID;
- feature, backend, and quality-tier masks;
- warmup boundary policy;
- pass dependencies;
- benchmark runtime-creation policy.

The registry includes the current compute-culling pipeline labels queued by
`fun_render`, cloud compute/view-composite labels, and first renderer-owned
placeholders for CEF composition, virtual geometry, virtual shadows, Lux direct
lighting, upscaling, and frame generation. Shader variants are constrained to
the explicit axes allowed by the renderer migration plan: backend, HDR/LDR,
MSAA, skinning, alpha mode, lighting tier, shadow tier, upscaler mode, and
virtual geometry.

`fun_render::pipeline_warmup` now builds a renderer-initialization warmup plan
from that registry even when the Bevy `PipelineCache` warmup executor is
disabled. `FUN_RENDER_PIPELINE_WARMUP=observed` still controls the current Bevy
queue processing path, but the bridge now logs the registry counts and first
eligible warmup label so warmup coverage can be compared against runtime
creation events.

Pass 4 also fixes the concrete validation loop recorded during Pass 3. The
compute-culling shader declares bindings 1, 2, and 7 as read-only storage; the
pipeline layout now uses Bevy's `storage_buffer_read_only` helper for those
bindings instead of writable storage buffers. The focused unit test
`shader_read_only_storage_inputs_match_pipeline_layout_contract` locks that
contract.

### Pass 4 Runtime Evidence

Runtime smoke was attempted with:

```powershell
$env:FUN_RENDERER_BACKEND='fun'
$env:FUN_RENDER_PIPELINE_WARMUP='observed'
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 `
  -BenchmarkLane cpu_floor `
  -BenchmarkProfile pass4_pipeline `
  -BenchmarkScenario runtime_creation_zero_smoke `
  -BenchmarkMatrixLane dx12_pipeline_smoke `
  -RenderBackend dx12 `
  -PresentMode immediate `
  -WarmupSeconds 2 `
  -SampleSeconds 3 `
  -DisableClouds `
  -DisableSolari `
  -DisableMeshlets `
  -DisableFpsOverlay `
  -WindowWidth 320 `
  -WindowHeight 180
```

The live wrapper did not exit before the local command timeout, so the child
stack was stopped and the captured client log was parsed through
`scripts/benchmark_client.ps1 -InputLog`. The resulting artifact is:

- `target/benchmarks/client/20260506-005505-031/summary.json`
- `target/benchmarks/client/20260506-005505-031/summary.md`
- `target/dx12-perf-gate/pass4-pipeline/report.{md,json}`
- `target/dx12-perf-gate/pass4-pipeline/pipeline_cardinality_report.{md,json}`

The pipeline cardinality report records:

| metric | p95 max |
| --- | ---: |
| render pipeline creation | 0 |
| compute pipeline creation | 0 |
| shader pipeline creation | 0 |

This is evidence that the pass has a remediation path for hot-path pipeline
creation and that the compute-culling validation loop no longer dominates the
captured steady-state sample. It is not proof of a healthy DX12 lane: startup
capability diagnostics for this local run still reported
`selected_backend=dx12`, `actual_backend=vulkan`, and
`fallback_reason=actual_backend_mismatch`.

## Pass 5 Resource Ownership

`fun-renderer` now owns the renderer resource policy in
`fun-renderer/src/resource.rs`. The model is compile-checked and intentionally
small: upload resources, transient resources, persistent resources, imported
resources, and readback/debug resources each have typed sub-kinds and
allocation diagnostics.

The first allocator contract is still conservative. The renderer policy says:

| contract | value |
|---|---|
| policy owner | `fun-renderer` |
| compatibility shim owner | `fun_render` |
| hidden transient allocations in major passes | forbidden |
| force Bevy prepare-stage helpers through upload arena | false |
| semantic owner required before generic helper migration | true |

`fun_render::FunUploadArena` now exposes `FUN_UPLOAD_ARENA_RESOURCE_SHIM`, which
marks the existing staging-belt arena as a bridge compatibility shim under the
renderer-owned resource model. It still requires an existing command encoder
and creates no ad hoc encoder. This preserves the previous upload-arena
restraint: generic Bevy `DynamicUniformBuffer` and `RawBufferVec` helper rows
remain observe-only until their owning render systems are labeled and a
schedule insertion point is known.

`fun_renderer::resource::HOT_UPLOAD_KILL_LIST` ties the current small-buffer
offenders to `target/dx12-parity/current/dx12_parity_report.json`:

| owner row | current status | migration gate |
|---|---|---|
| `DynamicUniformBuffer` `uniform_buffer.rs:311` | Bevy generic helper, observe-only | semantic owner split |
| `RawBufferVec` `buffer_vec.rs:183` | Bevy generic helper, observe-only | batch or semantic owner split |
| `DynamicUniformBuffer` `uniform_buffer.rs:140` | Bevy generic helper, observe-only | semantic owner split |
| `RawBufferVec` `buffer_vec.rs:442` | Bevy generic helper, observe-only | batch or semantic owner split |

The allocation diagnostic payload is
`ResourceFrameAllocationDiagnostics`: per frame upload bytes/count, transient
bytes/count, persistent bytes/count, imported-resource count, readback
bytes/count, top allocation sites, and high-water marks.

## Pass 6 Renderer-Owned Frame Graph

`fun-renderer` now owns the typed frame graph in
`fun-renderer/src/frame_graph.rs`. The graph has explicit pass types for render,
compute, copy/import, readback, presentation, and vendor SDK work. It also has
explicit resource types for render-resolution scene color, display-resolution
scene color, depth, motion vectors, normals/material IDs, UI color/alpha, final
composed output, transient scratch, and history buffers.

The initial graph built from `RendererFrameDescription::static_scene_with_ui`
contains:

| order | pass | type | role |
|---:|---|---|---|
| 0 | `fun_renderer.pass.clear` | render | clear |
| 1 | `fun_renderer.pass.static_scene_placeholder` | render | HUD-less scene placeholder |
| 2 | `fun_renderer.pass.cef_gpu_import` | copy/import | CEF GPU UI color/alpha import |
| 3 | `fun_renderer.pass.compose` | render | late scene/UI compose |
| 4 | `fun_renderer.pass.present` | presentation | final present |

Optional frame-description flags add graph slots without moving ownership out of
`fun-renderer`:

| flag | added pass | required inputs |
|---|---|---|
| `include_virtual_resource_slot` | `virtual_resource_feedback` compute pass | depth, motion vectors |
| `include_upscaling_slot` | `upscale_boundary` vendor SDK pass | HUD-less scene color, depth, motion vectors |
| `include_frame_generation_slot` | `frame_generation_boundary` vendor SDK pass | HUD-less display scene color, UI color/alpha, depth, motion vectors |
| `include_diagnostics_readback` | `diagnostics_readback` readback pass | final composed output |

`RendererFrameGraph::execute` currently emits zero-cost placeholder pass timings,
stable pass order, resource lifetimes, and validation failures. Validation locks
the day-one presentation contract:

- scene and UI color remain separate;
- frame generation receives HUD-less scene color, UI color/alpha, depth, and
  motion vectors;
- compose reads display scene color plus UI color/alpha and writes final output;
- present reads final output and remains the final pass.

`fun_render` now derives a `RendererFrameDescription` from bridge settings and
submits it through `FrameGraphSubmission` on `NoopRendererCore`. The bridge
records `RendererBridgeFrameGraphReport`, including graph diagnostics and a
debug artifact, but it does not own pass order or graph execution policy.

Current debug artifact:
`target/frame-graph/pass6/renderer_frame_graph_debug.txt`.

## Pass 7 GPU Scene Database

`fun-renderer` now owns `GpuSceneDatabase` in `fun-renderer/src/scene.rs`.
This is the renderer-facing record store; it is not a private scene authoring
dialect. It consumes `fun-scene` declaration components such as `Renderable`,
`GeometryRef`, `MaterialRef`, `VirtualGeometryAuthoring`, `LuxLight`, and
`SceneStableIdentity`.

Record families:

| record | key/handle | core fields |
|---|---|---|
| view | `GpuViewHandle` | camera transform, projection, viewport, jitter, previous/current view-projection, exposure/HDR metadata |
| instance | `GpuInstanceHandle` | stable instance ID, current/previous transform, mesh handle, material handle, flags, visibility, page metadata |
| mesh | `GpuMeshHandle` | `fun-scene` geometry ref, classic mesh buffer ref, virtual geometry ref, fallback mesh ref |
| material | `GpuMaterialHandle` | `fun-scene` material ref, material class, texture table indices, bindless base index, alpha/specular/emissive flags |
| light | `GpuLightHandle` | stable light ID, type, transform, color/intensity, radius/cone, shadow policy, importance |
| page metadata | `GpuPageMetadataHandle` | geometry page ID, shadow page ID, texture residency ID, GI cache ID |

Update protocol:

- creates and updates go through upsert APIs;
- handles carry `index + generation` and stale-generation operations are
  rejected and counted;
- instances are looked up by `StableInstanceId`, not transient Bevy `Entity`;
- transform updates retain `previous_transform` for motion history;
- removals mark records dirty and invalidate the stale handle;
- compaction is explicit through `compact_removed_records`;
- dirty ranges are tracked per record family.

Upload protocol:

- `GpuSceneDatabase::upload_dirty_records` drains dirty ranges into
  `ResourceFrameAllocationDiagnostics`;
- upload bytes and uploaded record counts are recorded in
  `GpuSceneDatabaseDiagnostics`;
- `fun_render` bridges those upload totals into `FunRendererUploadArena` while
  the database and allocation policy remain in `fun-renderer`.

`fun_render/src/extraction.rs` adds `FunRenderSceneExtractionBridge` and
`FunRenderSceneExtractionReport`. The bridge maps Bevy entities to
`StableInstanceId`, prefers `fun-scene` stable identities when present, creates
synthetic IDs only as a transition fallback, and extracts renderable/light ECS
components into `GpuSceneDatabase`. This keeps Bevy as orchestration/extraction
and leaves renderer-core storage in `fun-renderer`.

## Pass 8 CEF GPU Compositor

`fun-renderer/src/ui/cef.rs` now owns the renderer-side CEF compositor contract.
The active product CEF lane publishes accelerated ready-frame tokens into
`RendererCefCompositor` instead of presenting through a Bevy UI image.

The compositor records only renderer-owned texture state:

- `RendererCefFrameId`
- `RendererCefTextureId`
- `RendererCefOwnedTexture`
- `RendererCefUiLayer`
- `RendererCefCompositorDiagnostics`

Callback-lifetime shared handles are not stored in the renderer API. The DX12
bridge copies CEF's D3D11 shared texture into the FUN-owned D3D12 ring during
the callback path, then exposes a plain `Dx12CefReadyFrameToken` containing
generation, extent, DXGI format, dirty-rect metadata, callback/import timing,
and copied bytes. `game_client` converts that token into
`RendererCefImportedFrame` and immediately imports it into the renderer
compositor resource.

Product CPU fallback now fails closed at three points:

- explicit `CpuPaint` startup requests select `disabled` and mark startup
  failed;
- accelerated bridge failure no longer recreates a CPU browser;
- any CPU `OnPaint` frame observed by the runtime compositor is rejected and
  counted by `RendererCefCompositorDiagnostics`.

The frame graph names the late UI import pass
`fun_renderer.pass.cef_gpu_import`; scene color and UI color remain separate
until the compose pass. This keeps super-resolution ahead of UI composition and
leaves a clean future FG boundary: HUD-less scene color, UI color/alpha, depth,
motion vectors, and present-time resource lifetime facts.

Diagnostics now cover callback-to-import latency, import/copy duration, UI
composite pass duration, dropped/stale UI frames, invalid frame metadata,
copied bytes, CPU fallback attempts, and fail-closed count. The product policy
checker is `tools/check_product_ui_policy.ps1`; it rejects Bevy UI product
components, CEF CPU upload/write paths, and the removed CEF-to-Bevy-image copy
bridge.

## Pass 9 Backend Truth Gate

Pass 9 turns backend selection into a testable contract. The renderer
capability report now records:

- requested, selected, actual, and fallback graphics backend;
- graphics backend selection reason and truth state;
- DX12 native interop support;
- premium rendering gate status;
- backend parity scene IDs.

The required backend parity scene catalog now lives in
`fun-renderer/src/parity.rs` and covers:

| scene ID | purpose |
| --- | --- |
| `backend_parity.clear_present` | clear/present sanity lane |
| `backend_parity.static_mesh` | static mesh parity lane |
| `backend_parity.material` | material parity lane |
| `backend_parity.depth_motion_vector` | depth and motion-vector parity lane |
| `backend_parity.cef_ui_composite` | CEF UI composite parity lane |
| `backend_parity.post_upscale_placeholder` | post/upscale placeholder lane |
| `backend_parity.benchmark_capture` | benchmark capture lane |

`game_client` now reads the renderer capability report before writing
`target/run-stack/cef-ui-transport.json`, and CEF transport logs/status include
the same backend truth fields. `scripts/benchmark_client.ps1` merges renderer
backend truth into `cef_ui_transport_selection`, and
`tools/dx12_parity_report.py` fails a DX12 lane when the report says the actual
backend is not DX12.

Current local evidence:

| artifact | status | key result |
| --- | --- | --- |
| `target/run-stack/renderer-capabilities.json` | measured | requested DX12, actual Vulkan |
| `target/benchmarks/client/20260506-032407-482/summary.json` | measured | `requested_graphics_backend=dx12`, `selected_graphics_backend=dx12`, `actual_graphics_backend=vulkan`, `fallback_graphics_backend=vulkan`, `graphics_backend_truth_state=actual_backend_mismatch`, `premium_rendering_gate=blocked_backend_mismatch` |
| `target/dx12-parity/current/dx12_parity_report.md` | measured_fail | dashboard now names the requested/selected/actual/fallback mismatch |
| `target/benchmarks/dx12_parity/20260506-025401-303/matrix.json` | planned | CEF transport parity plan generated |

The short live smoke reached runtime and produced capability/log artifacts, but
the benchmark wrapper timed out before a normal sample-window closeout. The log
was harvested through the parser-only path, which is enough for the Pass 9
backend-truth finding. The finding is implementation/configuration, not
hardware absence: the NVIDIA adapter is visible, but the Bevy/wgpu actual
backend is Vulkan when the lane asks for DX12.

Current premium-rendering rule: do not attempt DLSS, FSR, frame generation, or
CEF GPU transport product claims until `graphics_backend_truth_state` is
`trusted_dx12` or `auto_resolved_dx12` and `dx12_native_interop_support` is
`supported`.

## Pass 10 Shared Page Scheduler

`fun-renderer/src/page.rs` now owns the shared virtual-resource page scheduler.
It is the residency spine for virtual geometry, virtual shadows, streamed
textures, GI/radiance cache pages, material cache pages, and future neural cache
data. Virtual geometry and virtual shadows must use this API instead of
inventing separate residency systems.

Page identity and state are explicit:

- `LogicalPageId { owner, value }`
- `PhysicalPageSlot { index, generation }`
- `PageGeneration`
- `PageResidencyState`
- `PageUploadState`
- `PageEvictionState`

The residency states are `missing`, `requested`, `uploading`, `resident`,
`pinned`, `eviction_candidate`, `evicting`, and `invalidated`. Physical slots
are generation-checked and return to the free list with a bumped generation
after eviction or invalidation.

The first priority heuristic is deterministic and inspectable. It scores:

- projected area;
- camera proximity;
- visibility confidence;
- temporal instability;
- motion magnitude;
- luminance/contrast importance;
- shadow receiver demand;
- gameplay salience;
- editor focus.

`PagePriorityScore` stores both the final score and a
`PagePriorityBreakdown`, so later learned predictors can imitate or augment the
heuristic without hiding why a page was requested.

Diagnostics now cover:

- page faults per frame;
- evictions per frame;
- upload bytes by owner;
- resident pages by owner;
- page age heatmap;
- priority heatmap;
- pinned page count;
- fault storm detector;
- high-water marks.

`PageOwnerFrameRequests` is the shared owner-facing submission API. The focused
tests exercise virtual geometry and virtual shadows as independent mock owners
using the same scheduler path. The debug artifact is controlled by
`FUN_RENDERER_PAGE_SCHEDULER_BENCHMARK_ARTIFACT`.

## Pass 11 Static Virtual Geometry

`fun-renderer/src/virtual_geometry.rs` now owns the static virtual geometry
asset and runtime substrate. `tools/virtual_geometry_bake` is a thin tooling
front-end over that renderer-owned API instead of depending on the legacy
`fun_render` bridge.

The renderer-owned asset format is `StaticVirtualGeometryAsset`. It contains:

- `StaticVirtualGeometryAssetHeader` with schema version, asset ID, root node,
  counts, checksum, and deterministic tool metadata;
- `VirtualGeometryClusterHeader` records with local/world bounds, triangle and
  index ranges, material ranges, normal cones, child ranges, page references,
  screen-error fields, cull hints, and HZB occluder hints;
- `VirtualGeometryPage` records with byte offsets, compressed byte counts,
  first cluster, cluster count, checksum, and conversion to shared
  `LogicalPageId`;
- compact `VirtualGeometryMaterialRange` records;
- hierarchy nodes for refinement;
- `VirtualGeometryFallbackMesh` for graceful missing-root-page behavior;
- debug metadata for deterministic bake validation.

Runtime selection uses `select_static_virtual_geometry_frame` and
`StaticVirtualGeometryExecutionPolicy`. It performs:

- static asset validation and checksum verification;
- hierarchical screen-error refinement;
- frustum and HZB culling through cull hints and view context;
- visible cluster compaction;
- page requests through `PageScheduler` with `PageOwner::VirtualGeometry`;
- compute-indirect draw packet construction as the baseline path;
- optional mesh-shader draw packets only when supported and requested;
- fallback mesh packets when the root page is missing.

The current path is still a CPU-side renderer-core substrate and diagnostic
model, not final GPU command execution. The important ownership boundary is now
correct: static virtual geometry assets, runtime selection, page-fault
diagnostics, and bake determinism live in `fun-renderer`; `fun_render` remains
the bridge.

Metrics now cover input clusters, resident clusters, visible clusters, drawn
clusters, culled clusters, missing/fallback clusters, geometry page faults,
bytes streamed, HZB timing placeholder, and mesh-shader timing placeholder. The
large-scene page-fault artifact is controlled by
`FUN_RENDERER_STATIC_VIRTUAL_GEOMETRY_BENCHMARK_ARTIFACT`.

## Pass 12 Dynamic Geometry And Procedural Bridge

`fun-scene/src/renderer_components.rs` now declares dynamic geometry semantics
with domain names, not product-prefixed component names:

- `SceneGeometryKind` separates static, streamable, dynamic, and procedural
  chunk declarations;
- `DynamicGeometryClass` classifies skinned players, NPCs, vehicles, weapons,
  destruction fragments, procedural terrain chunks, temporary FX geometry,
  editor gizmos/debug shapes, and generic dynamic actors;
- `DynamicGeometryLifetime` separates persistent actors, streamed chunks,
  destruction debris, transient frames, editor-only geometry, and normal
  runtime dynamic records;
- `DynamicGeometryUpdateHint` records transform, skinning, vertex/index,
  material, procedural chunk, destruction, lifetime, editor gizmo, and unknown
  update reasons;
- `ProceduralChunkOwner` carries chunk ID and revision;
- `GeometryDeclaration` and `DynamicGeometryAuthoring` make this scene-authored
  ECS data instead of private renderer folklore.

`fun-renderer/src/dynamic_geometry.rs` now owns the renderer-core substrate for
dynamic submissions. `DynamicGeometrySubmission` accepts the scene-authored mesh
and material handles, dynamic payload sizes, bounds, current and previous
transforms, update reason, lifetime class, priority hint, procedural owner, and
dynamic cluster mode. `DynamicGeometryDatabase` tracks live records, dirty
flags, generation increments, upload bytes, CPU/GPU update cost placeholders,
skinning cost, procedural invalidation cost, draw counts, dispatch counts, and
class counts.

The execution path is intentionally split from static virtual geometry:

- classic mesh draw packets are the default dynamic path;
- optional dynamic cluster packets are allowed for selected classes such as
  destruction fragments, procedural chunks, and vehicles when payloads provide a
  dynamic cluster budget;
- procedural chunk edits call `apply_procedural_update` to update matching
  chunk revisions and dirty only the affected dynamic records;
- the procedural update report keeps `full_static_virtual_geometry_rebuilds=0`
  so minor procedural edits do not imply a whole `.funvg` rebuild.

Synthetic tests cover a skinned dynamic actor scene, procedural invalidation
scene, destruction-fragment stress artifact, coexistence with static virtual
geometry, and fun-scene declaration-to-submission conversion. The upload/dirty
artifact is controlled by `FUN_RENDERER_DYNAMIC_GEOMETRY_BENCHMARK_ARTIFACT`.


## Pass 13 Virtual Shadows And Lux Policy

`fun-renderer/src/virtual_shadow.rs` now owns the virtual-shadow storage
substrate. The core model is page-based:

- `ShadowVirtualPageId`, `ShadowPageTableId`, and `ShadowPageRecord` form the
  renderer-owned shadow page table;
- `ShadowPageKind` separates directional clipmap pages from sparse local-light
  pages;
- `ShadowPhysicalPageSlot` and `ShadowStorageConfig` model directional and
  local physical page pools plus per-frame refresh budgets;
- `ShadowPageState` tracks invalidated, requested, refreshing, and resident
  states;
- `ShadowInvalidationReason` covers camera, receiver, occluder, light,
  intensity, time-of-day, geometry-page, and procedural-chunk invalidation;
- `ShadowPagePriorityInputs` scores visible receiver demand, screen coverage,
  contrast, temporal instability, gameplay salience, editor focus, and light
  importance.

Refreshes are driven by submitted page requests and sorted by receiver demand,
not by the mere existence of lights. Directional and local-light refresh budgets
are separate, so local lights can be sparse and budget-managed instead of
linearly expanding a global atlas. The current implementation is still a
renderer-core storage and diagnostic substrate, not final GPU command
execution.

`fun-lux/src/shadow.rs` now owns policy decisions through
`shadow::ShadowPolicyEngine`. It selects directional clipmap shadows versus
local paged shadows, applies quality tier, soft-shadow mode, reconstruction
mode, minimum intensity, maximum caster count, and page-budget policy. It
returns decisions and diagnostics only; allocator state, resident pages,
invalidated pages, and refresh ownership remain in `fun-renderer`.

Diagnostics cover refreshed pages, resident pages, invalidated pages, page
misses, cache hits/misses, cache hit ratio, per-light page budget,
directional-clipmap pressure, and local-light pressure. The stress tests cover
directional receiver-demand refresh, sparse local-light budget behavior, typed
invalidation reasons, cache accounting, Lux policy selection, Lux local budget
capping, and the virtual-shadow artifact controlled by
`FUN_RENDERER_VIRTUAL_SHADOW_BENCHMARK_ARTIFACT`.

## Pass 14 Many-Light Direct Illumination

`fun-lux/src/many_light.rs` now owns the many-light direct illumination
substrate. It is budget-managed rather than `lights * pixels`:

- `GpuLightDatabase` stores stable light IDs, type, view transform,
  color/intensity, radius/cone shape, shadow policy, update stamp, importance
  hints, and optional emissive-source reference;
- `ClusterGridConfig`, `LightClusterKey`, and `ClusteredCandidateSet` partition
  view space into bounded clustered candidate lists and preserve a
  `ForwardPlusFallback` mode;
- `LightCandidate` records candidate source, selected light, score, update
  stamp, shadow capability, and page-budget hint;
- `ReservoirStorage` tracks selected lights, temporal reuse, spatial reuse,
  reuse validity, rejection reasons, and shadowed candidate count;
- `EmissiveSourceRecord` promotes important emissive geometry into direct-light
  candidates while low-importance emissives can remain GI/cache contributors;
- `ManyLightShadowRequest` emits budgeted virtual-shadow request intents for
  winning candidates without making `fun-lux` own renderer page allocation.

The current implementation is a deterministic CPU-side Lux policy/data
substrate and diagnostic model. It establishes the GPU-facing records and
budgeting shape before final GPU command execution lands in `fun-renderer`.
Adding more lights now increases candidate pressure, overflow counters,
reservoir work, and shadow request pressure instead of implying a naive
per-pixel loop over every light.

Artifacts are controlled by `FUN_LUX_MANY_LIGHT_BENCHMARK_ARTIFACT`,
`FUN_LUX_MANY_LIGHT_SHADOW_ARTIFACT`, and
`FUN_LUX_MANY_LIGHT_DEBUG_OVERLAY_ARTIFACT`. They record candidate pressure,
selected lights, reuse counts, shadowed candidate counts, requested shadow
pages, and compact overlay rows that explain which light each cluster selected.

## Pass 15 Hybrid GI And Reflections

`fun-lux/src/gi.rs` now owns the hybrid GI/reflection substrate. The production
world representation is intentionally singular: a persistent surface cache. The
quality ladder is explicit:

- tier 0: ambient/probe fallback;
- tier 1: screen-space GI/reflections;
- tier 2: screen-space plus surface cache;
- tier 3: selective hardware RT assist where the backend supports it;
- tier 4: experimental neural/radiance-cache assistance through future
  `fun-ai` handoff points.

The surface cache tracks occupancy, age, validity, update cost, history
rejection, invalidation reason, frame index, and scene revision. Invalidation is
typed for procedural edits, destruction, time-of-day jumps, weather changes,
material changes, camera cuts, and major scene-streaming events. Update
requests are priority sorted, deterministic, and budget-limited so dynamic
scene mutation has explicit behavior instead of hidden cache resets.

Reflection source selection is centralized in `fun-lux` and follows the ordered
ladder: screen trace, surface cache, optional hardware RT assist, then
denoise/reconstruct. Hardware RT improves quality when available but tier 2
remains a software path. Debug artifacts controlled by
`FUN_LUX_GI_REFLECTION_SCENE_ARTIFACT`,
`FUN_LUX_GI_PROCEDURAL_INVALIDATION_ARTIFACT`, and
`FUN_LUX_GI_CACHE_STABILITY_ARTIFACT` report source mix, cache invalidation, and
stability score.

## Pass 16 Upscaling Boundary

`fun-renderer/src/upscaling.rs` now owns the renderer presentation upscaler
contract. The neutral interface is independent of vendor SDKs and records the
expected inputs explicitly:

- render-resolution HUD-less scene color;
- depth;
- motion vectors;
- exposure;
- jitter;
- reactive mask;
- transparency mask;
- HDR metadata;
- render and display extents.

The frame graph declares exposure, reactive-mask, transparency-mask, and HDR
metadata resources and validates that `upscale_boundary` reads those resources
plus HUD-less scene color, depth, and motion vectors. It also validates that the
upscaler never reads `ui_color_alpha`; late CEF/Svelte composition still happens
in `compose` after the display-resolution HUD-less scene color is produced.
When frame generation is present, the upscaler must remain before that vendor
handoff and before UI composition.

The current implementation includes native/debug fallback modes plus DLSS SR
and FSR hooks. DLSS SR requires capability support and valid render-pixel motion
vectors. FSR requires capability support, reactive and transparency mask
support, and HDR support whenever the frame is HDR. Both vendor paths fall back
to native bilinear only when configuration allows it; otherwise they disable
with a diagnostic reason. Scene color that already contains UI is rejected
instead of being upscaled.

Diagnostics record mode, render/display resolution, jitter, motion-vector
validity and reason, reactive/transparency mask coverage, history reset count,
estimated pass time, mip-bias policy result, UI separation, and DLSS/FSR
boundary readiness. The benchmark artifact is written by setting
`FUN_RENDERER_UPSCALING_BENCHMARK_ARTIFACT`.

## Pass 17 Frame-Generation Boundary

`fun-renderer/src/frame_generation.rs` now owns the renderer frame-generation
contract. It is neutral across vendor paths and records the expected inputs:

- display-resolution HUD-less scene color;
- UI color/alpha;
- depth;
- motion vectors;
- frame timing;
- present resources;
- reset flags.

The frame graph declares frame timing, present resources, FG reset flags,
presentable frame candidates, and pacing diagnostics. `frame_generation_boundary`
must run after `upscale_boundary`, read split scene/UI inputs plus timing,
present resources, depth, motion vectors, and reset flags, then write presentable
frames, history, and pacing diagnostics. It must not read final composed output.
`compose` remains after FG and still consumes UI color/alpha, so FG cannot force
early UI flattening. `present` reads present resources, keeping present-time
resource lifetime visible in graph diagnostics.

The current implementation disables cleanly for pause, loading, menu state,
resolution changes, swapchain recreation, invalid motion vectors, invalid UI
buffer, UI readability risk, editor docked-heavy manipulation, unstable pacing,
upscaler history reset, backend-truth failure, missing capability, missing FSR
presentation bridge, or invalid present-time resource lifetime.

Policy is explicit: FG is off by default for docked editor viewports and is
eligible only for game runtime, play-in-editor, immersive editor viewport, or
cinematic preview when explicitly enabled and all validation gates pass. DLSS FG
and FSR FG are only readiness hooks until backend truth and visible
renderer-owned presentation are proven. The benchmark artifact is written by
setting `FUN_RENDERER_FRAME_GENERATION_BENCHMARK_ARTIFACT`.

## Pass 18 Renderer-Facing Model Scaffold

`fun-ai` owns the reusable runtime side: model manifests, routing/backend
selection, inference queues, model trust/versioning, evals, and offline
training/evaluation hooks. `fun-renderer` owns only renderer-side request
metadata, tensor/resource contract validation, GPU handle references, and
deterministic heuristic fallback.

The first narrow model-assisted surface is `shadow_page_priority_prior`.
`fun_ai_core::renderer` describes the hook schema, bounded input/output tensors,
required metadata, GPU resource handle classes, and fallback policies.
`fun-renderer/src/ml.rs` is compiled only behind `experimental_renderer_ml` and
exposes `RendererPredictionClient`, `ShadowPagePriorityPriorRequest`, and
`select_shadow_page_priority_priors`. The selection path falls back
deterministically when policy disables ML, model runtime is unavailable, model
load fails, the queue is unavailable, inference is late, output is invalid, the
hook kind mismatches, or required GPU resource handles are missing.

Promotion remains evidence-gated. The fun-ai eval contract compares
heuristic-only, model-assisted, and model-disabled fallback lanes for latency,
p95/p99 frame time, page faults, evictions, shadow misses, visual instability
proxy, and false positives/negatives. The renderer benchmark artifact is written
by setting `FUN_RENDERER_ML_BENCHMARK_ARTIFACT`.


## Pass 19 Shared Heuristic Scheduler

`fun-renderer/src/scheduler.rs` now owns the shared renderer work-priority
language. `SharedPriorityInputs` normalizes projected area, motion magnitude,
temporal history error, luminance variance, material risk, alpha/specular risk,
occlusion confidence, gameplay salience, editor focus, and camera proximity into
a `NormalizedPriorityScore` with per-term contributions.

`SHARED_SCHEDULER_INTEGRATION_HOOKS` routes the same score into geometry page
requests, shadow page refreshes, light candidate budgets, GI cache updates,
reflection ray budgets, texture residency, shading-rate decisions, and optional
ML inference density. The scheduler exposes quality presets, developer
overrides, per-scene profiles, benchmark profiles, and debug freeze/lock
controls.

Diagnostics include a priority heatmap, per-system budget allocations,
dropped/deferred work, high-priority misses, a stability score, p95/p99 frame
impact, and a top-row debug overlay for CEF/Svelte diagnostics. The benchmark
artifact is written by setting `FUN_RENDERER_SHARED_SCHEDULER_BENCHMARK_ARTIFACT`.

## Pass 20 Quality Ladder And Settings

`fun-renderer/src/settings.rs` now owns the renderer settings schema
`fun.renderer.settings.v1`. The quality ladder is typed as Baseline, Hybrid,
High, RT-assisted, Vendor-enhanced, and Experimental, with descriptor rows that
state which tiers require virtual geometry, virtual shadows, many-light direct
illumination, hybrid GI, larger page budgets, RT assist, vendor SR/FG, or
experimental model/work-graph features.

`RendererSettingsRequest` separates user settings from internal settings. User
settings cover runtime/backend selection, quality preset, upscaler, frame
generation, shadow quality, GI quality, reflection quality, virtual geometry
budget, texture streaming budget, and diagnostics visibility. Internal settings
cover page pool size, shadow page budget, light candidate budget, GI cache
update budget, pipeline warmup policy, runtime pipeline creation policy, and
fallback strictness.

`RendererCapabilityFacts` records the actual backend, vendor class, VRAM,
display mode, product/editor/benchmark mode, CEF GPU transport state, backend
feature support, upscaler/FG support, and compiled feature gates. The selector
chooses safe defaults and emits explicit `RendererSettingsAdjustment` rows when
a requested feature is not supported. Product CEF GPU transport remains
fail-closed, vendor SR/FG is never enabled by default, and frame generation is
disabled for docked editor mode even when a vendor path exists.

`fun_render::settings_bridge` converts the typed selection into
`RendererSettingsUiModel`, a CEF/Svelte-safe model with selected labels, enabled
options, disabled reasons, diagnostics visibility, and a benchmark reproduction
label. It deliberately avoids raw adapter names, driver strings, or other
avoidable identifying fields. The capability artifact is written by setting
`FUN_RENDERER_SETTINGS_CAPABILITY_ARTIFACT`.

## Renderer Module Inventory

| area | current files/tools | current owner | intended owner | status |
| --- | --- | --- | --- | --- |
| Product presentation | `game_client/src/lib.rs`, `fun_render/src/winit.rs`, `fun_render/src/core.rs` | `game_client` + `fun_render` | `fun-renderer` core through `fun_render` bridge | Current visible frame path. |
| Renderer-core seams | `fun-renderer/src/{lib.rs,api.rs,dynamic_geometry.rs,ecs.rs,frame_generation.rs,frame_graph.rs,heuristics.rs,ml.rs,page.rs,resource.rs,scheduler.rs,settings.rs,scene.rs,ui/cef.rs,upscaling.rs,virtual_geometry.rs,virtual_shadow.rs}` | `fun-renderer` | `fun-renderer` | Buildable substrate, frame-graph skeleton, GPU scene DB records, shared page scheduler, static virtual geometry format/runtime, dynamic geometry/procedural bridge, virtual-shadow page storage, shared heuristic scheduler, quality/settings ladder, resource model, renderer-owned CEF compositor, upscaling/frame-generation diagnostics, renderer-facing ML request/fallback scaffolding, not product swapchain ownership. |
| Renderer asset tooling | `tools/virtual_geometry_bake/src/*` | `fun-renderer` API through `virtual_geometry_bake` tool | `fun-renderer` + tooling front-end | Deterministic `.funvg.json` emitter uses renderer-owned static virtual geometry format and scheduler-compatible page IDs. |
| Lighting/Lux seams | `fun-lux/src/{lib.rs,api.rs,gi.rs,many_light.rs,shadow.rs}` | `fun-lux` | `fun-lux` | Buildable substrate, ECS extraction/update hooks, virtual-shadow policy decisions, many-light GPU records, clustered candidates, reservoirs, emissive promotion, GI quality ladder, surface-cache lifecycle, reflection source mix, and shadow request intents separate from renderer page storage. |
| Scene substrate | `fun-scene/src/*`, `fun-scene-macros/src/*`, `game_scene/src/*` | `fun-scene` + `game_scene` | same split | Active and first-party. |
| CEF/Svelte product UI | `game_client/ui/main`, `game_client/src/cef_ui.rs`, `fun_ui_cef/src/*`, `fun_host/src/lib.rs`, `fun-renderer/src/ui/cef.rs` | browser lifetime in `fun_ui_cef`, host/app bridge in `game_client`, compositor contract in `fun-renderer` | CEF/Svelte UI with renderer-owned GPU compositor | Active product lane now imports GPU tokens into `RendererCefCompositor`; visible swapchain composition still awaits renderer present handoff. |
| CEF DX12 accelerated transport | `game_client/src/cef_ui_dx12/*`, `fun_render/src/dx12_native/cef.rs`, `docs/dx12_cef_accelerated_paint.md` | `game_client` interop over `fun_render::dx12_native` | `fun-renderer` compositor/backend boundary with `fun_ui_cef` callbacks | D3D11 shared texture is copied into a FUN-owned D3D12 ring and exposed as safe ready-frame tokens; latest local runtime status is still bridge-blocked. |
| DX12 parity/benchmark tools | `scripts/benchmark_dx12_parity.ps1`, `scripts/benchmark_client.ps1`, `tools/dx12_*`, `tools/check_dx12_*` | `fun` tooling | remains benchmark/governance tooling | Active. |
| CEF parity/visual checks | `tools/compare_cef_ui_screenshots.ps1`, `scripts/stack/profiles/cef.*.json`, `scripts/benchmark_dx12_parity.ps1 -MatrixSize cef_transport` | `fun` tooling | remains benchmark/governance tooling | Plan and health parsing exist; full healthy matrix blocked. |
| Upload arena and upload reports | `fun-renderer/src/resource.rs`, `fun_render/src/upload_{arena,budget,labels,ranges,report}.rs`, `fun_render/src/instance_tables.rs`, `docs/dx12_upload_audit.md` | policy in `fun-renderer`, compatibility shim in `fun_render` | renderer-owned resource model with bridge shims until measured owners are migrated | Boundary exists; resource classes and allocation diagnostics are typed; top offenders still generic Bevy write helpers. |
| Pipeline diagnostics | `fun_render/src/pipeline_warmup.rs`, `docs/dx12_descriptor_pipeline_churn.md`, `docs/dx12_shader_quality.md`, `tools/dx12_pipeline_cardinality_report.py` | `fun_render` + Bevy diagnostics | renderer diagnostics through `fun-renderer`/engine hooks | Runtime creation still measured after warmup. |
| Native interop | `fun_render/src/dx12_native/*`, `game_client/src/cef_ui_dx12/*` | `fun_render` interop gate, `game_client` CEF bridge | `fun-renderer` backend abstraction after migration | Centralized gate exists; raw command-list accessor still intentionally limited. |

## Known Blockers

| blocker | current evidence | owner now | migration impact | next proof |
| --- | --- | --- | --- | --- |
| DX12 backend truth mismatch | Pass 9 smoke artifact `target/benchmarks/client/20260506-032407-482/summary.json` records `requested_graphics_backend=dx12`, `selected_graphics_backend=dx12`, `actual_graphics_backend=vulkan`, `fallback_graphics_backend=vulkan`, and `graphics_backend_truth_state=actual_backend_mismatch`. | `fun_render` startup/backend selection over Bevy/wgpu | Blocks CEF GPU transport, DX12 native interop, DLSS, FSR, frame generation, and renderer-owned presentation claims. | A live artifact with actual DX12, no fallback backend, `dx12_native_interop_support=supported`, and matching CEF bridge readiness. |
| `FunUploadArena` boundary | `fun_render::FunUploadArena` wraps `wgpu::util::StagingBelt` and tracks/budgets aligned buffer writes. `FUN_UPLOAD_ARENA_RESOURCE_SHIM` records it as a bridge compatibility shim for `fun_renderer::resource` staging-buffer pages. `docs/dx12_upload_audit.md` says not to force Bevy prepare-stage `RenderQueue` helpers through ad hoc encoders. | policy in `fun-renderer`, shim in `fun_render` | Foundation for moving measured small-buffer owners into renderer resource ownership without increasing submit pressure. | Top semantic owners for `DynamicUniformBuffer`/`RawBufferVec` rows, then measured before/after with no submit regression. |
| DX12 upload kill-list | Current parity dashboard lists top offenders under Bevy generic rows, including `uniform_buffer.rs:311`, `buffer_vec.rs:183`, `gpu_image.rs:84`, and Solari constant rows. | Bevy + `fun_render` diagnostics | Prevents speculative upload rewrites. | Label split or owning-system attribution for top rows. |
| CEF accelerated lane classified `cef_transport_bound` | Root ledger and CEF docs record the failed accelerated lane; `target/benchmarks/client/20260504-005500-247/summary.json` selected CPU fallback with `fallback_reason=render_backend_not_dx12`, `bridge_ready=false`, zero GPU copy bytes, and zero accelerated paint FPS. | `game_client` CEF DX12 bridge over `fun_render::dx12_native` | Blocks GPU-only product UI and DLSS/upscale boundary proof. | Strict D3D11On12 lane with `selected=d3d11on12`, `bridge_ready=true`, `cef_cpu_upload_bytes=0`, nonzero `cef_gpu_copy_bytes`, no normal-frame blocking waits. |
| Strict D3D11On12 lane disabled | `target/run-stack/cef-ui-transport.json` last written 2026-05-05 reports `requested=d3d11on12`, `selected=disabled`, `backend=dx12`, `bridge_ready=false`, `cpu_fallback_enabled=false`, `strict=true`, `fallback_reason=render_backend_not_dx12`. | `game_client` startup/interop | Confirms strict GPU-only UI currently fails closed instead of presenting UI. | Fix backend/bridge readiness detection, then rerun strict stack lane. |
| Runtime render pipeline creation | Older `target/dx12-pix/pipeline_cardinality_report.md` reports render pipeline p95 `22`; Pass 4 smoke parse reports render pipeline p95 `0` after observed warmup. | Bevy pipeline cache + `fun_render` warmup, with registry metadata in `fun-renderer` | DX12 perf gate hard failure when nonzero after warmup; blocks DLSS/FG claims until a real DX12 artifact is clean. | Rerun a true DX12 lane after backend mismatch is resolved; preserve zero render pipeline p95 and name any nonzero pipeline label. |
| Runtime compute pipeline creation | Older report records compute pipeline p95 `82`; Pass 4 smoke parse reports compute pipeline p95 `0` after observed warmup. | Bevy/Solari/meshlet pipelines + `fun_render` warmup, with registry metadata in `fun-renderer` | Hard failure if nonzero after warmup; compute-culling read-only binding mismatch is fixed. | Keep creation-focused events and verify warmup coverage against registry labels. |
| Runtime shader pipeline creation | Older report records shader pipeline create p95 `104`; Pass 4 smoke parse reports shader pipeline p95 `0` after observed warmup. | Bevy/Solari/meshlet families plus `fun-renderer` variant axes | Hard failure if nonzero after warmup; product Bevy UI paths have been removed from the CEF lane. | Keep shader variants on explicit registry axes and block ad hoc stringly permutations. |
| DLSS boundary gate not ready | `docs/dx12_dlss_boundary_gate.md` says `DX12 baseline ready for DLSS SR bring-up: no`. CEF health, uploads, barriers, and pipeline creation are not ready. | `fun_render` DLSS scaffold today; intended `fun-renderer` presentation boundary | DLSS/FSR/FG cannot be used for performance claims. | Gate flips to `yes` with attached parity, CEF GPU, upload, PIX/barrier, present, and pipeline evidence. |
| Runtime Bevy UI product dependency | Pass 8 removes `game_client` CEF `Node`/`ImageNode` presentation, CPU texture upload helpers, client FPS Bevy UI, and `fun_render` `FpsOverlayPlugin` registration. `tools/check_product_ui_policy.ps1` blocks regression. | `game_client` + `fun_render` + `fun-renderer` policy | Product UI is now CEF/Svelte through the renderer compositor contract; test-only/compat labels remain classified separately. | Keep checker in validation and route future overlays through CEF/Svelte or renderer debug primitives. |

## Bevy UI Usage Map

| path | usage | classification | reason | action |
| --- | --- | --- | --- | --- |
| `game_client/src/cef_ui.rs` former `sync_cef_ui_image_node` | Removed. | migrated | The CEF product lane no longer spawns Bevy UI `Node`/`ImageNode` for presentation. | Keep blocked by `tools/check_product_ui_policy.ps1`. |
| `game_client/src/cef_ui.rs` former CPU upload path | Removed active GPU writes. CPU `OnPaint` observations are rejected and counted. | migrated/fail-closed | Product CEF pixels no longer use `RenderQueue::write_texture` or a Bevy `Image`. | Keep CPU paint only as diagnostic/test fixture logic until fully deleted. |
| `game_client/src/cef_ui.rs` former client FPS counter | Removed. | migrated | Runtime/product debug overlay no longer uses Bevy UI text. | Surface FPS through CEF/Svelte diagnostics or renderer debug primitives. |
| `fun_render/src/core.rs` former `FpsOverlayPlugin` registration | Removed. | migrated | The bridge no longer installs Bevy's FPS overlay plugin. | Keep `FUN_ENABLE_FPS_OVERLAY` as inert transition config until config cleanup. |
| Workspace `Cargo.toml` patch table | Patches `bevy_ui`, `bevy_ui_render`, and `bevy_ui_widgets` for local Bevy fork. | not a product usage by itself | Patch table exposes local path crates; usage depends on Bevy default plugins and product code. | Do not use this as acceptance proof; block product imports/usages separately. |

No Bevy UI product usage is allowed in `game_client`, `fun_render`,
`fun-renderer`, or `fun_host` after Pass 8. The checker allows
`fun-renderer` policy constants that document the prohibition.

## CEF/Svelte Product Path

| layer | current status |
| --- | --- |
| Svelte app | Active under `game_client/ui/main`; package name `fun-client-ui`; scripts are `dev`, `build`, `preview`, and `check`; UI dependencies are Svelte/Vite/TypeScript/Bulma. |
| Rust host authority | `fun_host` exposes launcher/editor/preview command descriptors and current-client preview status; CEF command surface readiness is part of host status. |
| CEF runtime | `fun_ui_cef` owns CEF browser bootstrap/runtime, windowless browser settings, JS bridge, input validation, CPU paint compositor, accelerated callback surface, diagnostics, and security checks. |
| Game integration | `game_client/src/cef_ui.rs` installs `GameCefUiPlugin`, starts CEF, routes input/model patches/host commands, tracks transport counters, and publishes GPU-ready CEF frames to `RendererCefCompositor`. |
| Current composition | Renderer-owned CEF compositor resource plus frame-graph `cef_gpu_import` slot; visible final swapchain composition still awaits the future renderer-present handoff. |
| Accelerated transport | `game_client/src/cef_ui_dx12` opens CEF D3D11 shared textures, copies through D3D11On12 into a FUN-owned D3D12 ring during the callback path, and publishes safe tokens with timing/copy metadata. Latest artifacts show bridge readiness failing. |
| Runtime CPU fallback | Product runtime is fail-closed. CPU `OnPaint` startup requests select `disabled`, accelerated failures do not recreate a CPU browser, and observed CPU frames are rejected by the renderer compositor. |
| GPU-only product readiness | Code path is GPU-only/fail-closed; runtime proof remains blocked until strict accelerated lane reports `bridge_ready=true`, `selected=d3d11on12`, zero CPU upload bytes, and nonzero GPU copy bytes. |

## DX12 And Vulkan Status

| item | DX12 current status | Vulkan current status |
| --- | --- | --- |
| Backend selection | Stack profiles and scripts can request `dx12`/`immediate`, but Pass 9 evidence currently reports requested/selected DX12 with actual Vulkan and fallback Vulkan. DX12 success now requires the actual backend to be DX12, not just the requested lane. | Supported by parity scripts and `default.vulkan.immediate.json`; Vulkan is also the observed actual backend in the latest requested-DX12 smoke. |
| Product visible renderer | Bevy/wgpu through `fun_render`, not `fun-renderer` backend abstraction. | Same product path, different backend. |
| Native interop | Centralized in `fun_render::dx12_native`; CEF/DLSS use this gate. Current truth state blocks it because actual backend is Vulkan. | No equivalent native feature gate in this pass. |
| CEF accelerated transport | Blocked: bridge readiness fails with `render_backend_not_dx12`; Pass 9 shows the deeper foundation issue is actual Vulkan under a requested DX12 lane. | Not applicable; accelerated path is Windows/D3D11On12/DX12-specific. |
| Parity evidence | Selected local matrix exists; Pass 9 dashboard now fails a requested-DX12 lane when capability JSON reports actual Vulkan. | Used as control lane in parity matrix. |
| Present decision | Do not change defaults until full present matrix and latency evidence are complete. | Control lane required before changing DX12 defaults. |
| Barrier evidence | Blocked on PIX CSV/capture rows. | Not the target of PIX DX12 barrier audit. |

## Validation Command Inventory

| purpose | command |
| --- | --- |
| Renderer core compile | `cargo check -p fun-renderer` |
| Renderer core tests | `cargo test -p fun-renderer --lib` |
| Renderer page scheduler tests | `cargo test -p fun-renderer --lib page` |
| Renderer page scheduler artifact | `$env:FUN_RENDERER_PAGE_SCHEDULER_BENCHMARK_ARTIFACT='target\page-scheduler\pass10-fault-eviction.txt'; cargo test -p fun-renderer --lib fault_eviction_benchmark_artifact_records_shared_owner_metrics` |
| Static virtual geometry tests | `cargo test -p fun-renderer --lib virtual_geometry` |
| Static virtual geometry bake tool | `cargo test -p virtual_geometry_bake` |
| Static virtual geometry page-fault artifact | `$env:FUN_RENDERER_STATIC_VIRTUAL_GEOMETRY_BENCHMARK_ARTIFACT='target\virtual-geometry\pass11-static-vg-page-faults.txt'; cargo test -p fun-renderer --lib virtual_geometry::tests::large_static_scene_benchmark_artifact_records_page_faults -- --exact --nocapture` |
| Dynamic geometry tests | `cargo test -p fun-renderer --lib dynamic_geometry` |
| Dynamic geometry upload/dirty artifact | `$env:FUN_RENDERER_DYNAMIC_GEOMETRY_BENCHMARK_ARTIFACT='target\dynamic-geometry\pass12-upload-dirty-records.txt'; cargo test -p fun-renderer --lib dynamic_geometry::tests::destruction_fragment_stress_test_writes_upload_artifact -- --exact --nocapture` |
| Renderer virtual shadow tests | `cargo test -p fun-renderer --lib virtual_shadow` |
| Renderer virtual shadow artifact | `$env:FUN_RENDERER_VIRTUAL_SHADOW_BENCHMARK_ARTIFACT='target\virtual-shadow\pass13-shadow-pages.txt'; cargo test -p fun-renderer --lib virtual_shadow::tests::shadow_page_artifact_records_directional_and_local_pressure -- --exact --nocapture` |
| Lux shadow policy tests | `cargo test -p fun-lux --lib shadow` |
| Lux many-light tests | `cargo test -p fun-lux --lib many_light` |
| Lux many-light artifacts | `$env:FUN_LUX_MANY_LIGHT_BENCHMARK_ARTIFACT='target\many-light\pass14-many-light-benchmark.txt'; $env:FUN_LUX_MANY_LIGHT_SHADOW_ARTIFACT='target\many-light\pass14-shadow-interaction.txt'; $env:FUN_LUX_MANY_LIGHT_DEBUG_OVERLAY_ARTIFACT='target\many-light\pass14-debug-overlay.txt'; cargo test -p fun-lux --lib many_light::tests::many_light_benchmark_shadow_and_overlay_artifacts_are_stable -- --exact --nocapture` |
| Lux GI/reflection tests | `cargo test -p fun-lux --lib gi` |
| Lux GI/reflection artifacts | `$root=(Get-Location).Path; $env:FUN_LUX_GI_REFLECTION_SCENE_ARTIFACT=Join-Path $root 'target\gi\pass15-reflection-scene.txt'; $env:FUN_LUX_GI_PROCEDURAL_INVALIDATION_ARTIFACT=Join-Path $root 'target\gi\pass15-procedural-invalidation.txt'; $env:FUN_LUX_GI_CACHE_STABILITY_ARTIFACT=Join-Path $root 'target\gi\pass15-cache-stability.txt'; cargo test -p fun-lux --lib gi::tests::gi_reflection_invalidation_and_stability_artifacts_are_stable -- --exact --nocapture` |
| Renderer upscaling tests | `cargo test -p fun-renderer --lib upscaling` |
| Renderer upscaling artifact | `$root=(Get-Location).Path; $env:FUN_RENDERER_UPSCALING_BENCHMARK_ARTIFACT=Join-Path $root 'target\upscaling\pass16-upscaling-boundary.txt'; cargo test -p fun-renderer --lib upscaling::tests::upscaling_benchmark_artifact_records_diagnostics -- --exact --nocapture` |
| Renderer frame-generation tests | `cargo test -p fun-renderer --lib frame_generation` |
| Renderer frame-generation artifact | `$root=(Get-Location).Path; $env:FUN_RENDERER_FRAME_GENERATION_BENCHMARK_ARTIFACT=Join-Path $root 'target\frame-generation\pass17-frame-generation-boundary.txt'; cargo test -p fun-renderer --lib frame_generation::tests::benchmark_artifact_records_generated_and_presented_counts -- --exact --nocapture` |
| Renderer upscaling feature compile | `cargo check -p fun-renderer --features upscaling,dlss,fsr,frame_generation,cef_gpu_only` |
| Renderer ML scaffold tests | `cargo test -p fun-renderer --features experimental_renderer_ml --lib ml` |
| Renderer ML scaffold artifact | `$root=(Get-Location).Path; $env:FUN_RENDERER_ML_BENCHMARK_ARTIFACT=Join-Path $root 'target\ml\pass18-shadow-page-prior.txt'; cargo test -p fun-renderer --features experimental_renderer_ml --lib ml::tests::benchmark_artifact_compares_required_lanes -- --exact --nocapture` |
| Renderer shared scheduler tests | `cargo test -p fun-renderer --lib scheduler` |
| Renderer shared scheduler artifact | `$root=(Get-Location).Path; $env:FUN_RENDERER_SHARED_SCHEDULER_BENCHMARK_ARTIFACT=Join-Path $root 'target\scheduler\pass19-shared-heuristic-scheduler.txt'; cargo test -p fun-renderer --lib scheduler::tests::p95_p99_frame_time_artifact_records_scheduler_diagnostics -- --exact --nocapture` |
| Renderer settings tests | `cargo test -p fun-renderer --lib settings` |
| Renderer settings capability artifact | `$root=(Get-Location).Path; $env:FUN_RENDERER_SETTINGS_CAPABILITY_ARTIFACT=Join-Path $root 'target\settings\pass20-renderer-settings-capability.txt'; cargo test -p fun-renderer --lib settings::tests::settings_capability_artifact_records_reproducible_settings -- --exact --nocapture` |
| Renderer CEF/Svelte settings bridge tests | `cargo test -p fun_render --no-default-features --lib settings_bridge` |
| Renderer frame graph tests | `cargo test -p fun-renderer --lib frame_graph` |
| Renderer frame graph debug artifact | `$env:FUN_RENDERER_FRAME_GRAPH_DEBUG_ARTIFACT='target\frame-graph\pass6\renderer_frame_graph_debug.txt'; cargo test -p fun-renderer --lib frame_graph_debug_artifact_names_passes_resources_and_markers` |
| Renderer scene DB tests | `cargo test -p fun-renderer --lib scene` |
| Renderer CEF compositor tests | `cargo test -p fun-renderer --lib cef` |
| Renderer CEF compositor benchmark artifact | `$env:FUN_RENDERER_CEF_COMPOSITOR_BENCHMARK_ARTIFACT='target\benchmarks\cef_compositor\pass8-ui-composite.json'; cargo test -p fun-renderer --lib cef_compositor_records_ui_composite_benchmark_payload` |
| Bridge extraction tests | `cargo test -p fun_render --lib extraction` |
| Lux compile/tests | `cargo check -p fun-lux`; `cargo test -p fun-lux --lib` |
| Bridge compile/tests | `cargo check -p fun_render`; `cargo test -p fun_render --lib` |
| Client compile | `cargo check -p game_client` |
| CEF CPU lane compile | `cargo check -p game_client --no-default-features --features cef_ui --locked` |
| CEF accelerated lane compile | `cargo check -p game_client --no-default-features --features cef_ui_dx12_accelerated_paint --locked` |
| Scene migration check | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_fun_scene_migration.ps1 -SelfTest`; then without `-SelfTest` |
| Product UI policy check | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_product_ui_policy.ps1 -SelfTest`; then `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_product_ui_policy.ps1 -EmitArtifact target\cef-parity\pass8-product-ui-policy.json` |
| DX12 doctrine check | `powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_dx12_doctrine.ps1 -SelfTest`; then without `-SelfTest` |
| Renderer backend truth smoke | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/benchmark_client.ps1 -BenchmarkLane presentation_floor -BenchmarkProfile pass9_backend_truth -BenchmarkScenario dx12_truth_smoke -BenchmarkMatrixLane dx12_backend_truth_smoke -RenderBackend dx12 -PresentMode immediate -WarmupSeconds 1 -SampleSeconds 2 -DisableClouds -DisableSolari -DisableMeshlets -DisableFpsOverlay -WindowWidth 320 -WindowHeight 180` |
| Renderer backend truth parse-only artifact | `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/benchmark_client.ps1 -InputLog target\run-stack\logs\game_client.err.log -BenchmarkProfile pass9_backend_truth -BenchmarkScenario dx12_truth_smoke -BenchmarkMatrixLane dx12_backend_truth_smoke -RenderBackend dx12 -PresentMode immediate -WarmupSeconds 0 -SampleSeconds 0 -DisableClouds -DisableSolari -DisableMeshlets -DisableFpsOverlay -WindowWidth 320 -WindowHeight 180` |
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
   diagnosis. Do not re-enable CPU `OnPaint` upload as a product fallback.
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
- Route virtual geometry, virtual shadow, streamed texture, GI/radiance,
  material-cache, and neural-cache residency through the shared page scheduler
  before adding owner-specific GPU implementations.
- Keep static virtual geometry on the compute-indirect baseline. Mesh shaders
  are an optional `VirtualGeometryMeshShaderMode::Optional` packet path and must
  stay removable without changing correctness or fallback behavior.
- Build future static geometry GPU execution from `StaticVirtualGeometryAsset`
  and `select_static_virtual_geometry_frame` instead of extending the older
  `fun_render` virtual-geometry prototype.
- Keep dynamic gameplay geometry on the dynamic submission path. Skinned
  players, vehicles, weapons, destruction fragments, procedural chunks, FX
  geometry, and editor debug shapes should submit `DynamicGeometrySubmission`
  records instead of forcing whole static virtual geometry asset rebuilds.
