# Client Diagnostics

The client now treats diagnostics as structured tracing data first and plain log
text second. The existing `[client perf]` lines stay in place for scripts, but
the same data is emitted with fields that can be filtered by tracing targets.

## Run With Rich Tracing

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -RenderDiagnostics -TraceDiagnostics -RenderBackend vulkan -PresentMode immediate
```

`-TraceDiagnostics` sets both `RUST_LOG` and `BEVY_LOG`. Bevy's log plugin uses
`RUST_LOG`; `BEVY_LOG` is kept as a project-level alias for scripts and future
tools.

```text
RUST_LOG=info,fun=debug,fun::diag=info,fun::perf=info,fun::perf::solari=info,bevy_solari=debug,bevy_solari::realtime=debug,bevy_render::transient=debug,bevy_render::scheduler=trace,bevy_pbr::meshlet::scheduler=trace,bevy_pbr::meshlet::vram=debug
```

You can override `RUST_LOG` manually when you need a narrower view.
For every-frame Solari dispatch tracing, use
`RUST_LOG=info,fun::perf=info,bevy_solari::realtime=trace` for a short capture;
that mode is intentionally not used by comparison scripts because trace-volume
can perturb frame time.

## Targets

- `fun::render`: backend, present mode, Solari, meshlets, DLSS RR, and denoiser
  mode.
- `fun::stream`: streamed world chunks, revisions, ready state, acks, duplicate
  entities, and world resets.
- `fun::solari`: Solari activation and temporal-history resets.
- `fun::rr`: DLSS Ray Reconstruction availability, activation, and resets.
- `fun::diag`: periodic world/camera/renderable inventory.
- `fun::perf`: FPS, frame ms/ns, Solari total ns, meshlet visibility ns, and
  external RR ns.
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
  meshlet floor can be separated from normal PBR raster work.
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
  creates, previous-frame reuses, same-frame lifetime aliases, and cached slot
  counts.
- `bevy_render::scheduler`: render graph budget pressure.
- `bevy_pbr::meshlet::scheduler`: meshlet visibility budget decisions and
  async-compute policy decisions. WGPU currently runs these candidates through
  the graphics-queue fallback unless a backend-specific async path proves a p95
  win.

## Render Graph Resource Policy

The Bevy fork now exposes a render transient resource arena. Render graph users
declare scratch resource lifetimes by logical pass range. Matching resources can
be reused from prior frames, and same-frame aliasing is allowed only when the
declared lifetimes do not overlap. This is intentionally conservative: it lowers
allocation churn without depending on backend-specific explicit heap aliasing.

Meshlet visibility is the first consumer. Its dummy render target is transient,
while visibility buffers and cull queues stay persistent because they need stable
capacity and bind-group behavior. The benchmark parser recognizes
`bevy_render::transient` logs as `transient_*` metrics when trace diagnostics
are enabled.

## GPU Contention

The client cannot share textures, queues, acceleration structures, or DLSS state
with another process such as `cs2.exe`. Windows and the driver share the physical
GPU by scheduling independent process contexts. The client is expected to survive
contention by recreating lost render devices, recreating lost surfaces, preserving
CPU-side meshlet assets for reupload, and reporting every recovery attempt.

When contention is suspected, keep Solari and meshlets enabled and run with:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\run_stack.ps1 -RenderDiagnostics -TraceDiagnostics -RenderBackend vulkan -PresentMode immediate
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
- `FUN_SOLARI_WORLD_CACHE_SIZE`
- `FUN_SOLARI_WORLD_CACHE_UPDATES`
- `FUN_SOLARI_WORLD_CACHE_LIGHT_SAMPLES`
- `FUN_SOLARI_WORLD_CACHE_FRAME_SLICES`
- `FUN_SOLARI_WORLD_CACHE_NEAR_METERS`
- `FUN_SOLARI_WORLD_CACHE_MID_METERS`
- `FUN_SOLARI_WORLD_CACHE_FAR_METERS`
- `FUN_SOLARI_LIGHT_TILE_BLOCKS`
- `FUN_SOLARI_LIGHT_TILE_SAMPLES`
- `FUN_SOLARI_BLAS_COMPACTION_VERTICES`
- `FUN_SOLARI_INTERNAL_SCALE=1.0|0.75|0.66|0.5`
- `FUN_SOLARI_DEBUG_OVERLAY=surface-classification|work-queues`

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
