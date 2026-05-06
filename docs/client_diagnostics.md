# Client Diagnostics

The client now treats diagnostics as structured tracing data first and plain log
text second. The existing `[client perf]` lines stay in place for scripts, but
the same data is emitted with fields that can be filtered by tracing targets.

## Run With Rich Tracing

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -RenderDiagnostics -TraceDiagnostics -RenderBackend dx12 -PresentMode immediate
```

`-TraceDiagnostics` sets both `RUST_LOG` and `BEVY_LOG`. Bevy's log plugin uses
`RUST_LOG`; `BEVY_LOG` is kept as a project-level alias for scripts and future
tools.

```text
RUST_LOG=info,fun=debug,fun::diag=info,fun::perf=info,fun::perf::solari=info,fun::perf::clouds=info,fun::render::clouds=debug,fun::weather=debug,bevy_solari=debug,bevy_solari::realtime=debug,bevy_render::transient=debug,bevy_render::scheduler=trace,bevy_pbr::meshlet::scheduler=trace,bevy_pbr::meshlet::vram=debug
```

You can override `RUST_LOG` manually when you need a narrower view.
For every-frame Solari dispatch tracing, use
`RUST_LOG=info,fun::perf=info,bevy_solari::realtime=trace` for a short capture;
that mode is intentionally not used by comparison scripts because trace-volume
can perturb frame time.

## Frame-Time Profiler

The detailed frame profiler is compile-gated behind both
`game_client/render_diagnostics` and `debug_assertions`. The stack script enables
that feature automatically for debug builds when `-FrameTimeDiagnostics`,
`-RenderDiagnostics`, `-TraceDiagnostics`, or `-RenderProfileVerbose` is used,
and refuses to compile diagnostic features into `--release` runs. Normal and
release client builds do not compile the profiler module, FPS overlay, Bevy
render diagnostics plugin, or client diagnostic systems. That same feature
enables Bevy's `debug` and `track_location` feature flags for debug diagnostic
builds so system/debug metadata is available without leaking that overhead into
release.

Use this for a targeted frame tree without launching any separate benchmark
tool:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -FrameTimeDiagnostics -FrameTimeDiagnosticInterval 60 -FrameTimeDiagnosticMaxDepth 10 -FrameTimeDiagnosticTopChildren 16 -RenderBackend dx12 -PresentMode immediate
```

Set `-FrameTimeDiagnosticMinNs 6944444` to emit only frames that miss the 144 Hz
budget. Use `-FrameTimeDiagnosticTopSpans 40` to widen the slow-span list, and
`-FrameTimeDiagnosticRowEvents` when an external parser wants one structured
event per thread/span/summary row. The report is emitted through `tracing`
target `fun::frame_time` and is shaped for direct frame attribution:

```text
GAME FRAME 330: 3012900 ns
summary: main_profiled_ns=2798600 main_unattributed_ns=214300 gpu_render_ns=2150912 render_cpu_ns=38800 total_profiled_ns=4988312 thread_count=3 span_count=68 marker_count=34
timeline -
@17900ns First:begin
@156700ns First:end
@506100ns RunFixedMainLoop:begin
@545500ns FixedFirst:begin
@687100ns FixedUpdate:begin
@2036200ns RunFixedMainLoop:end
main-thread -
-main-schedule 92.89% 2798600 ns
--RunFixedMainLoop 54.73% 1531600 ns self_ns=295000 start_ns=506100
---FixedUpdate 9.38% 143700 ns count=2 self_ns=113000 start_ns=687100
----apply_kinematic_movement [fn=apply_kinematic_movement @ game_client/src/first_person.rs:660] 21.36% 30700 ns count=2 self_ns=8600 start_ns=719200
-----move_and_slide [fn=move_and_slide @ game_client/src/first_person.rs:605] 71.99% 22100 ns count=2 start_ns=721500
--Update 4.21% 117900 ns self_ns=113000 start_ns=2063600
---receive_world_stream [fn=receive_world_stream @ game_client/src/lib.rs:1576] 1.27% 1500 ns start_ns=2118700
-unattributed 7.11% 214300 ns
gpu-render -
-solari_lighting 89.90% 1933568 ns
--direct_lighting 36.65% 708608 ns
--diffuse_indirect_lighting_spatial 17.82% 344576 ns
render-cpu -
-clustering 29.64% 11500 ns
slow-spans inclusive -
#1 main-thread main-schedule/RunFixedMainLoop 50.83% 1531600 ns start_ns=506100
#2 gpu-render solari_lighting/direct_lighting 23.52% 708608 ns
```

Frame nodes keep explicitly recorded inclusive time separate from child time, so
children do not double-count parent rows. Fixed schedules are nested under
`RunFixedMainLoop`, and manually instrumented systems are nested under their
actual Bevy schedule. `self_ns` is emitted when a parent has recorded time not
explained by profiled children. Source annotations use `#[track_caller]`, giving
the callsite file and line for client systems and manually instrumented
sub-steps.

The `summary:` row is also emitted as structured tracing fields on the same
event: `main_profiled_ns`, `main_unattributed_ns`, `gpu_render_ns`,
`render_cpu_ns`, `total_profiled_ns`, and `thread_count`. This makes it possible
to filter a capture down to the frame ledger before expanding the hierarchy.

Client instrumentation should use the `frame_profile_start!`,
`frame_profile_scope!`, `frame_profile_elapsed!`, and `frame_profile_ns!` macros.
Those macros expand to no-ops unless `game_client/render_diagnostics` and
`debug_assertions` are both active, so callsites can stay close to the measured
code without pulling profiler types or timing work into normal/release builds.

Cross-crate diagnostic tracing should use the shared `game_shared` macros:
`fun_diag_block!`, `fun_diag_block_if!`, `fun_diag_info!`,
`fun_diag_info_if!`, `fun_diag_debug!`, `fun_diag_debug_if!`,
`fun_diag_trace!`, `fun_diag_trace_if!`, `fun_diag_warn!`, and
`fun_diag_warn_if!`. These require the crate's `diagnostics` feature plus
`debug_assertions`, and whole blocks wrapped by them are compiled out otherwise.
Each emitted event automatically includes `diag_file`, `diag_line`, and
`diag_module` fields from the macro call site.

## Targets

- `fun::render`: selected renderer backend, actual renderer backend,
  `FUN_RENDERER_BACKEND` resolution, explicit legacy diagnostic routing,
  present mode, Solari, meshlets, DLSS RR, and denoiser mode.
- `fun::render::clouds`: cloud configuration, render-path cloud signature
  fields, history reset requests, reset generation/counts, and future per-view
  cloud render setup.
- `fun::stream`: streamed world chunks, revisions, ready state, acks, duplicate
  entities, and world resets.
- `fun::solari`: Solari activation and temporal-history resets.
- `fun::weather`: validated cloud/weather profile and pattern state, including
  profile IDs and transition classes without raw user-authored strings.
- `fun::rr`: DLSS Ray Reconstruction availability, activation, and resets.
- `fun::diag`: periodic world/camera/renderable inventory.
- `fun::perf`: FPS, frame ms/ns, Solari total ns, meshlet visibility ns, and
  external RR ns.
- `fun::perf::clouds`: cloud weather update, shape-noise, raymarch, temporal,
  resolve, composite, and total GPU ns, weather-update CPU ns, history
  accept/reset counts, internal dimensions, step counts, quality/profile labels,
  and cloud VRAM bytes.
- `fun::perf::schedule_heatmap`: actual client system costs gathered from the
  running Bevy schedule. It reports networking receive, streamed-world apply,
  movement input, look, first-person physics movement, diagnostics logging,
  render config/window work, Solari runtime-param updates, meshlet extraction,
  and render interpolation as nanoseconds plus share of measured client CPU
  work.
- `fun::perf::non_solari`: meshlet visibility sub-pass GPU timings, meshlet
  extraction/preparation CPU timings, world-stream apply cost, catalog lookup
  cost, physics fixed-update cost, postprocess cost, overlay cost, and present
  wait. It also reports render-path counts and standard-raster GPU cost so the
  meshlet floor can be separated from normal PBR raster work. Meshlet material
  queue diagnostics report the dirty instance count and queue CPU ns, and
  buffer diagnostics report full versus range writes for instance/material
  buffers, view-visibility mask writes, and per-view reset CPU queue writes.
  Together these show whether static or transform-only scenes are avoiding
  whole-buffer uploads, full material scans, and CPU-side reset write spam.
- `fun::perf::cef_ui`: CEF UI transport counters sampled separately from Bevy
  FPS: CPU `OnPaint` cadence, accelerated-paint cadence, CPU upload bytes, GPU
  copy bytes/ns/failures, GPU frame ready/not-ready/reused/blocking-wait counts,
  transport fallback count, published and sampled generations, and stale GPU
  frame count. Startup also emits a parser-stable selected-transport line with
  requested transport, selected transport, backend, bridge readiness, ring
  depth, copy mode, strict mode, debug timings, and fallback reason.
- `fun::perf::render_churn`: engine-level descriptor, bind group layout,
  pipeline layout, PSO, and specialized pipeline cache churn sampled by the
  client when render diagnostics enable the churn counters.
- `fun::perf::render_commands`: command encoder, render pass, compute pass,
  command buffer, queue submit, copy command, and native interop insertion
  counters for DX12/Vulkan submission-shape comparisons.
- `fun::perf::render_shaders`: shader module creation, shader variant request,
  pipeline creation timing, pipeline specialization, and material specialization
  counters for runtime compilation and permutation-pressure checks.
- `fun::render_catalog`: prewarmed render-catalog inventory, per-asset geometry
  class decisions, and runtime catalog usage counts.
- `fun::perf::solari`: Solari pass timings in ns, including direct lighting,
  surface classification, work-queue construction, diffuse GI plus split
  diffuse initial/spatial timings, specular regular/queued/PSR, guide resolve, cheap
  temporal denoise, each à trous denoise pass, and composite. It also reports
  the current world-cache active-cell count so adaptive cache settings can be
  judged against both cost and cell pressure.
- `fun::perf::solari_budget`: architecture, visual target, target FPS, frame
  and Solari GPU budgets, budget pressure, active direct/GI/specular work,
  cache request pressure, visual debt, and reconstruction pixels.
- `fun::perf::solari_queues`: surface-classification-driven queue pressure:
  critical direct pixels, GI tiles, GI repair pixels, specular pixels,
  radiance-cache requests, denoise repair tiles, overflow count, and active
  classified tiles.
- `fun::perf::radiance_cache`: read-only Solari radiance-cache query pressure:
  request count, hits, misses, hit rate, and request overflow. These counters
  are the immediate signal for whether budgeted cache service work is keeping
  up with GI/specular lookups.
- `fun::render::recovery`: device loss, out-of-memory/internal render errors,
  surface-loss/acquire failures, recovery attempts, successful reinitialization,
  and frames skipped while the renderer is unavailable.
- `fun::camera`: camera-count violations.
- `bevy_solari::realtime`: Solari pipeline initialization and per-dispatch trace
  fields such as denoise mode, scene generation, resource generation, reset
  state, view size, and world-cache settings.
- `bevy_solari::vram`: Solari realtime buffer/texture allocation estimates and
  DLSS RR guide texture allocation estimates. DLSS NGX internal allocations are
  opaque to Bevy and are reported as such.
- `bevy_pbr::meshlet::vram`: meshlet persistent GPU buffer allocation changes
  and queued upload counts.
- `bevy_render::transient`: frame-local render scratch resource requests,
  creates, previous-frame reuses, same-frame lifetime aliases, cached slot
  counts, descriptor miss/near-miss reasons, top descriptor-create rows, and
  label-variant rows.
- `bevy_render::scheduler`: render graph budget pressure.
- `bevy_render::capabilities`: one startup capability inventory line with the
  backend capability hash, vendor class, RT/AS support, async queue probe state,
  DLSS capability slots, native opacity/SER/LSS slots, and RT validation slot.
- `fun::render`: one RT gate line with `rt_feature_hash` for the requested
  renderer policy. Benchmark reports keep this separate from
  `backend_capability_hash` so requested features and backend support do not
  collapse into one identifier.
- `bevy_pbr::meshlet::scheduler`: meshlet visibility budget decisions and
  async-compute policy decisions. WGPU currently runs these candidates through
  the graphics-queue fallback unless a backend-specific async path proves a p95
  win.

For volumetric-cloud performance claims, the runtime diagnostic store must
include these GPU paths before a benchmark summary is considered valid:

- `render/clouds/weather_update/elapsed_gpu`
- `render/clouds/shape_noise/elapsed_gpu`
- `render/clouds/raymarch/elapsed_gpu`
- `render/clouds/temporal/elapsed_gpu`
- `render/clouds/resolve/elapsed_gpu`
- `render/clouds/composite/elapsed_gpu`

If these paths are absent, `fun::perf::clouds` may still report cloud
configuration and VRAM, but cloud pass timing is not proven.

## Render Graph Resource Policy

The Bevy fork now exposes a render transient resource arena. Render graph users
declare scratch resource lifetimes by logical pass range. Matching resources can
be reused from prior frames, and same-frame aliasing is allowed only when the
declared lifetimes do not overlap. This is intentionally conservative: it lowers
allocation churn without depending on backend-specific explicit heap aliasing.

Meshlet visibility is the first consumer. Its dummy render target is transient,
while visibility buffers and cull queues stay persistent because they need stable
capacity and bind-group behavior. The benchmark parser recognizes
`bevy_render::transient` logs as `transient_*` metrics when render diagnostics
are enabled. Descriptor audit details are documented in
[`dx12_transient_resource_reuse.md`](dx12_transient_resource_reuse.md).

## GPU Contention

The client cannot share textures, queues, acceleration structures, or DLSS state
with another process such as `cs2.exe`. Windows and the driver share the physical
GPU by scheduling independent process contexts. The client is expected to survive
contention by recreating lost render devices, recreating lost surfaces, preserving
CPU-side meshlet assets for reupload, and reporting every recovery attempt.

When contention is suspected, keep Solari and meshlets enabled and run with:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -RenderDiagnostics -TraceDiagnostics -RenderBackend dx12 -PresentMode immediate
```

Look for `[client render recovery]` lines and `fun::render::recovery` fields.
Those counters are the source of truth for whether the renderer skipped frames,
recreated a device, or only recreated the window surface.

## GPU Budget Knobs

The default visual path stays Solari plus meshlets with the BalancedFast
denoiser. These environment variables exist for controlled captures and stress
testing; engine-side validation clamps them to bounded GPU-safe ranges:

- `FUN_SOLARI_ARCH=legacy|budgeted`
- `FUN_SOLARI_TARGET_FPS=144`
- `FUN_SOLARI_FRAME_BUDGET_NS=6944444`
- `FUN_SOLARI_GPU_BUDGET_NS=3000000`
- `FUN_SOLARI_VISUAL_TARGET=competitive|balanced|cinematic`
- `FUN_RT_SAMPLE_DIRECT=0|1`
- `FUN_RT_SAMPLE_INDIRECT=0|1`
- `FUN_RT_SAMPLE_REFLECTIONS=0|1`
- `FUN_RT_SURFACE_CACHE=0|1`
- `FUN_RT_MEGAGEOM=off|software|native`
- `FUN_RT_OPACITY_MASK=off|baked|native`
- `FUN_RT_HAIR=off|cards|strands|native_lss`
- `FUN_RT_ASYNC_READBACK=0|1`
- `FUN_RT_VALIDATION=0|1`
- `FUN_RENDER_UNKNOWN_VENDOR=1`
- `FUN_RENDER_VENDOR_EMULATION=unknown|nvidia|amd|intel`
- `FUN_SOLARI_WORLD_CACHE_SIZE`
- `FUN_SOLARI_WORLD_CACHE_UPDATES`
- `FUN_SOLARI_WORLD_CACHE_LIGHT_SAMPLES`
- `FUN_SOLARI_WORLD_CACHE_FRAME_SLICES`
- `FUN_SOLARI_WORLD_CACHE_NEAR_METERS`
- `FUN_SOLARI_WORLD_CACHE_MID_METERS`
- `FUN_SOLARI_WORLD_CACHE_FAR_METERS`
- `FUN_SOLARI_LIGHT_TILE_BLOCKS`
- `FUN_SOLARI_LIGHT_TILE_SAMPLES`
- `FUN_SOLARI_DIRECT_INITIAL_SAMPLES`
- `FUN_SOLARI_DIRECT_SPATIAL_SAMPLES`
- `FUN_SOLARI_DIRECT_SPATIAL_BOOST_SAMPLES`
- `FUN_SOLARI_DIRECT_INITIAL_VISIBILITY=none|selected`
- `FUN_SOLARI_DIRECT_BOILING_FILTER_STRENGTH=0.0..1.0`
- `FUN_SOLARI_BLAS_COMPACTION_VERTICES`
- `FUN_SOLARI_INTERNAL_SCALE=1.0|0.75|0.66|0.5`
- `FUN_SOLARI_DEBUG_OVERLAY=surface-classification|work-queues`
- `FUN_DISABLE_CLOUDS=1`
- `FUN_CLOUD_QUALITY=off|cheap|balanced|cinematic`
- `FUN_CLOUD_INTERNAL_SCALE=1.0|0.75|0.5|0.33`
- `FUN_CLOUD_TEMPORAL=0|1`
- `FUN_CLOUD_SHADOWS=0|1`
- `FUN_CLOUD_PROFILE=clear|scattered|overcast|storm_front|cinematic_sunset|custom`
- `FUN_CLOUD_DEBUG_OVERLAY=none|coverage|density|steps|history|weather`

`legacy` preserves the pre-budgeted control path. `budgeted` keeps Solari and
meshlets enabled but routes adaptive runtime controls through a per-view Solari
runtime uniform, so cache-update budgets, ReSTIR reuse radii, temporal
confidence caps, and reconstruction strength can move without rebuilding
pipelines. The stack script defaults to `budgeted`, `competitive`, 144 Hz, a
6,944,444 ns frame budget, and a 3,000,000 ns Solari GPU budget.

The budgeted path now emits a per-frame Solari director plan into the GPU
runtime uniform. It reacts quickly when the previous measured Solari GPU cost
exceeds budget, then restores quality slowly. Surface classification and
work-queue diagnostics are built every frame. Specular lighting uses the queued
critical/glossy/mirror pixel path in budgeted mode, while direct lighting and GI
still keep their full-coverage fallback until their queue consumers can preserve
history and reconstruction quality. Use
`FUN_SOLARI_DEBUG_OVERLAY=surface-classification` or
`FUN_SOLARI_DEBUG_OVERLAY=work-queues` for one-run visual validation.

The radiance cache is now serviced as an explicit producer/consumer path.
GI/specular lighting queries do not initialize cache cells or compare-exchange
against the cache table. They read the camera-centered clipmap page, count
hit/miss pressure, and append compact miss requests. The cache-service pass
then admits the highest-budgeted request subset into resident clipmap pages and
tracks confidence/moments for future reuse.

`FUN_SOLARI_INTERNAL_SCALE` currently scales Solari GI reservoirs only. Direct
lighting stays full resolution so direct shadows remain crisp, and raster
meshlet presentation is not lowered by this control. Bevy's
`MainPassResolutionOverride` path is still respected by Solari resource
preparation when a caller explicitly opts into a lower main-pass resolution, but
the game client does not use that route for the default Solari-only tests.

## Hot-Path Logging Policy

Benchmarks default to metric-only logging. Repeated chunk, entity, control
packet, render catalog, and inventory logs are opt-in so IO does not become a
hidden frame-time variable:

- `FUN_BENCHMARK_LOG_MINIMAL=1` keeps benchmark output focused on metrics.
- `FUN_LOG_STREAM_VERBOSE=1` enables streamed-world chunk/entity details unless
  benchmark-minimal logging is active.
- `FUN_LOG_NET_VERBOSE=1` enables packet/control-flow details unless
  benchmark-minimal logging is active.
- `FUN_LOG_RENDER_VERBOSE=1` enables render-catalog/render-component details
  unless benchmark-minimal logging is active.

The stack script exposes the same controls as `-BenchmarkLogMinimal`,
`-LogStreamVerbose`, `-LogNetVerbose`, and `-LogRenderVerbose`. New diagnostic
output should use tracing macros and targets, not `println!` or duplicate stdout
paths.

## Descriptor And Pipeline Churn

`-RenderDiagnostics` enables engine-level resource churn counters through
`FUN_RENDER_CHURN_COUNTERS=1` and `BEVY_RENDER_CHURN_COUNTERS=1`. The client
emits `[client perf] render churn:` totals and top-ten
`[client perf] render churn top:` rows. Use
[`dx12_descriptor_pipeline_churn.md`](dx12_descriptor_pipeline_churn.md) for the
counter contract, steady-state expectations, and the canonicalization/warmup
decision path.

## Command Submission Shape

The same flag enables command submission counters through
`FUN_RENDER_COMMAND_COUNTERS=1` and `BEVY_RENDER_COMMAND_COUNTERS=1`. The client
emits `[client perf] render commands:` totals and top-ten
`[client perf] render command top:` rows. Use
[`dx12_command_submission_strategy.md`](dx12_command_submission_strategy.md) to
distinguish renderer work from submit fragmentation before changing DX12 queue
strategy.

## Shader Compilation And Quality

`-RenderDiagnostics` also enables shader diagnostics through
`FUN_RENDER_SHADER_DIAGNOSTICS=1` and `BEVY_RENDER_SHADER_DIAGNOSTICS=1`. The
client emits `[client perf] render shaders:` totals and top-ten
`[client perf] render shader top:` rows. Use
[`dx12_shader_quality.md`](dx12_shader_quality.md) for the compilation,
analysis, and permutation-reduction contract.

## Denoiser Comparison

The denoiser matrix can also enable the same tracing:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_denoisers.ps1 -TraceDiagnostics
```

That keeps the summary tables while also producing structured trace context for
why a mode was expensive: guide resolve, PSR specular, external RR, cheap
temporal, spatial à trous passes, or composite.

BalancedFast is the normal Solari denoiser for day-to-day runtime tests. It uses
cheap temporal filtering plus one à trous pass with fused final output. The
regular Balanced preset keeps the second à trous pass for quality comparison,
and Quality is opt-in for screenshots or explicit visual checks. DLSS Ray
Reconstruction is preserved as an explicit `rr`/`dlss-rr` preset, but it is
currently a known-broken path: it can show a large black square/rectangle and
leave Solari shadows broken or missing. Use `-SolariDenoiseMode rr` only for
targeted RR debugging until that issue is fixed.

RR acceptance is stricter than a normal benchmark. The acceptance run must use
`scripts\benchmark_client.ps1 -EnableDx12DlssRr -SolariDenoiseMode rr
-RequireDx12DlssRrAcceptance`. The generated `rr_acceptance` JSON block must
pass and must include `dlss_rr_gpu_ns`,
`solari_pass_dlss_rr_guide_resolve_ns`, `frame_ns.mean`, and `frame_ns.p95`.
