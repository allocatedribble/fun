# DX12 Descriptor And Pipeline Churn

## Purpose

DX12 can lose to Vulkan when descriptor/root-layout variety or runtime PSO
creation leaks into steady-state frames. This document defines the first
measurement slice for that problem. It intentionally measures before
canonicalizing layouts or moving warmup work, because incorrect layout merging
can break shader contracts.

## Runtime Counters

`bevy_render` owns the engine-level resource churn counters. They are disabled
unless one of these environment variables is enabled:

- `FUN_RENDER_CHURN_COUNTERS=1`
- `BEVY_RENDER_CHURN_COUNTERS=1`
- `FUN_RENDER_PIPELINE_COUNTERS=1`

`scripts/run_stack.ps1 -RenderDiagnostics` enables them together with render
upload counters. `game_client` samples and resets the counters on the existing
client performance interval.

The parser-stable summary line is:

```text
[client perf] render churn: bind_group_creations=... bind_group_layout_creations=... bind_group_layout_cache_hits=... bind_group_layout_cache_misses=... pipeline_layout_creations=... render_pipeline_queued=... compute_pipeline_queued=... render_pipeline_creations=... compute_pipeline_creations=... render_pipeline_ready=... compute_pipeline_ready=... render_pipeline_errors=... compute_pipeline_errors=... pipeline_cache_hits=... pipeline_cache_misses=... material_pipeline_key_count=... post_process_pipeline_key_count=... cloud_pipeline_key_count=... solari_pipeline_key_count=... meshlet_pipeline_key_count=... ui_pipeline_key_count=... debug_overlay_pipeline_key_count=... event_count=...
```

Top event rows are emitted as:

```text
[client perf] render churn top: rank=1 operation=render_pipeline_queued category=cloud label=fun_cloud_raymarch_pipeline calls=1
```

Benchmark JSON stores the top rows under `render_churn_events`.

Creation-focused rows are emitted separately so cache hits do not bury the
labels that matter for warmup and layout decisions:

```text
[client perf] render churn creation top: rank=1 operation=render_pipeline_created category=solari label=bevy_solari::realtime::diffuse calls=1
```

Benchmark JSON stores those rows under `render_churn_creation_events`.

## Counted Events

Engine-level counters currently cover:

- bind group creation through `RenderDevice::create_bind_group`;
- bind group layout creation through `RenderDevice::create_bind_group_layout`;
- bind group layout cache hits and misses through `PipelineCache`;
- pipeline layout creation through `RenderDevice::create_pipeline_layout`;
- render and compute pipelines queued through `PipelineCache`;
- render and compute pipeline GPU object creation through `RenderDevice`;
- render and compute pipelines becoming ready or failing through `PipelineCache`;
- specialized pipeline cache hits and misses through Bevy's specialized pipeline
  caches.

The first pass does not count every `set_bind_group` call. Bevy exposes those as
raw pass methods today, and wrapping every pass command would be a larger engine
surface change. Use PIX for bind-group set calls until a safe render-pass wrapper
exists.

## Labels And Categories

Labels are taken from actual descriptor labels or specializer type names, then
classified into:

- `material`
- `post_process`
- `cloud`
- `solari`
- `meshlet`
- `ui`
- `debug_overlay`
- `other`

The category counts are named as pipeline-key cardinality metrics, for example
`render_churn_material_pipeline_key_count` and
`render_churn_cloud_pipeline_key_count`. They count queued runtime pipeline
variants in the sample window, not a static shader inventory.

## Benchmark Commands

Collect a DX12 lane with churn counters:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 `
  -RenderBackend dx12 `
  -PresentMode immediate `
  -TraceDiagnostics `
  -SampleSeconds 30 `
  -WarmupSeconds 10
```

Compare against Vulkan with the same UI/feature lane:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_dx12_parity.ps1 -MatrixSize quick -ContinueOnFailure
```

The report should show `render_churn_render_pipeline_creations`,
`render_churn_compute_pipeline_creations`, bind group layout creation/miss
counts, pipeline cache hit/miss counts, and top churn events.

Generate the focused cardinality report from an existing matrix:

```powershell
python tools\dx12_pipeline_cardinality_report.py `
  --matrix target\dx12-parity\current\matrix.json `
  --output-dir target\dx12-pix
```

Outputs:

- `target\dx12-pix\pipeline_cardinality_report.md`
- `target\dx12-pix\pipeline_cardinality_report.json`

## Steady-State Rule

During steady-state gameplay, these should be zero or explicitly justified:

- `render_churn_render_pipeline_creations`
- `render_churn_compute_pipeline_creations`
- `render_churn_pipeline_layout_creations`
- `render_churn_bind_group_layout_creations`

Nonzero values are acceptable during startup, shader reload, scene load, asset
streaming that introduces new material variants, or explicit debug overlay
activation. If a lane reports nonzero values after warmup, inspect
`render_churn_events` first, then confirm root-signature/PSO behavior in PIX.

## Canonicalization Plan

Do not canonicalize layouts from labels alone. Use the top churn events and PIX
captures to identify layouts with matching binding structure but different
creation sites.

Canonicalization candidates should move into a small registry only after the
structure match is proven:

```rust
pub struct CanonicalBindGroupLayouts {
    pub camera: BindGroupLayout,
    pub material_standard: BindGroupLayout,
    pub material_extended: BindGroupLayout,
    pub post_process_common: BindGroupLayout,
    pub cloud_common: BindGroupLayout,
    pub solari_common: BindGroupLayout,
}
```

Rules:

- Preserve shader binding compatibility first.
- Prefer dummy/null resources over layout variants only when the resource type,
  format, binding visibility, and access mode remain valid.
- Do not collapse variants that exist for measured performance reasons.
- Validate in PIX that root signature or descriptor layout switches decrease.

## Pipeline Key Reduction Plan

Use the category metrics to split key fields into:

- `must_specialize`: shader entry point, binding layout, target format, topology,
  MSAA when enabled, and real shader-def choices.
- `can_uniform`: quality levels, thresholds, debug visualization modes, and
  runtime feature strength where shaders can branch cheaply.
- `debug_only`: overlays and validation modes that should not appear in normal
  benchmark lanes.
- `dead_or_obsolete`: fields that no longer affect descriptors.

Do not remove a key bit without a visual comparison and a pipeline-count delta.

## Warmup Discipline

`fun_render` exposes an opt-in warmup hook:

- `FUN_RENDER_PIPELINE_WARMUP=off|basic|observed|scene|exhaustive`
- `FUN_RENDER_PIPELINE_WARMUP_BUDGET_MS=1|2|4|8`

Current behavior:

- `off`: do nothing.
- `basic`: process the queued Bevy `PipelineCache` once in the render
  `Prepare` set before the render graph consumes it.
- `observed`: process the queued Bevy `PipelineCache` while runtime pipeline
  work is still being observed, then stop after 30 consecutive idle render
  frames or after 240 render frames. This is the default next experiment when
  the cardinality report shows runtime-created pipelines but does not yet
  justify an exhaustive or scene-wide warmup.
- `scene`: process queued pipelines for the first 120 render frames.
- `exhaustive`: process queued pipelines every render frame.

The budget is measured and reported; `PipelineCache::process_queue` is not yet
preemptible, so the hook cannot stop mid-pipeline when a driver compile exceeds
the budget. The hook logs `waiting_before`, `waiting_after`, `elapsed_ns`,
`budget_ns`, and `budget_exceeded`.

This is a warmup control surface, not a full variant enumerator. Scene-aware
material enumeration should come after the churn counters identify which
runtime pipelines are created after loading.

Layout canonicalization remains blocked until `render_churn_creation_events`
or PIX descriptor rows identify a specific family and a binding-structure
comparison proves the layouts are compatible.
