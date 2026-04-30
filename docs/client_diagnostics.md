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
RUST_LOG=info,fun=debug,fun::diag=info,fun::perf=info,fun::perf::solari=info,bevy_solari=debug,bevy_solari::realtime=debug
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
- `fun::perf::solari`: Solari pass timings in ns, including direct lighting,
  diffuse GI, specular regular/PSR, guide resolve, cheap temporal denoise, each
  à trous denoise pass, and composite.
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

The default visual path stays Solari plus meshlets with the Balanced denoiser.
These environment variables exist for controlled captures and stress testing;
engine-side validation clamps them to bounded GPU-safe ranges:

- `FUN_SOLARI_WORLD_CACHE_SIZE`
- `FUN_SOLARI_WORLD_CACHE_UPDATES`
- `FUN_SOLARI_WORLD_CACHE_LIGHT_SAMPLES`
- `FUN_SOLARI_LIGHT_TILE_BLOCKS`
- `FUN_SOLARI_LIGHT_TILE_SAMPLES`
- `FUN_SOLARI_BLAS_COMPACTION_VERTICES`

## Denoiser Comparison

The denoiser matrix can also enable the same tracing:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_denoisers.ps1 -TraceDiagnostics
```

That keeps the summary tables while also producing structured trace context for
why a mode was expensive: guide resolve, PSR specular, external RR, cheap
temporal, spatial à trous passes, or composite.

Balanced is the normal Solari denoiser for day-to-day runtime tests. DLSS Ray
Reconstruction is preserved as an explicit `rr`/`dlss-rr` preset, but it is
currently a known-broken path: it can show a large black square/rectangle and
leave Solari shadows broken or missing. Use `-SolariDenoiseMode rr` only for
targeted RR debugging until that issue is fixed.
