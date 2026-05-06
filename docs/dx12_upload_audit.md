# DX12 Upload Path Audit

Status: measurement slice, upload cleanup tier 3.1 + pass5 resource ownership boundary.

## Instrumentation

- `FUN_RENDER_UPLOAD_COUNTERS=1` or `BEVY_RENDER_UPLOAD_COUNTERS=1` enables
  Bevy `RenderQueue` counters for `write_texture`, `write_buffer`, and
  `write_buffer_with`.
- `scripts/run_stack.ps1 -RenderDiagnostics` enables the counters for benchmark
  runs.
- `game_client` emits `[client perf] render uploads:` totals and top-ten
  `[client perf] render upload top:` callsites.
- `scripts/benchmark_client.ps1` records upload totals in `metrics` and
  aggregates top callsites under `render_upload_callsites`.
- `tools/dx12_parity_report.py` turns the DX12 summary's
  `render_upload_callsites` into a top-callsite kill list with calls/frame,
  bytes/frame, calls/sec, bytes/sec, p95 impact guess, and a recommended fix.

## Inventory Summary

Search command:

```powershell
rg -n "\.write_texture\(|\.write_buffer\(|\.write_buffer_with\(|RenderQueue::write_texture|RenderQueue::write_buffer|Queue::write_texture|Queue::write_buffer|write_buffer_range" fun bevy avian bevy_quinnet thunder -g "*.rs"
```

Categories:

| category | callsites | notes |
|---|---|---|
| CEF/UI large texture | `fun/game_client/src/cef_ui.rs` | CPU paint fallback writes full or dirty BGRA texture rects. Semantic labels are `cef_ui.cpu_paint.full_frame` and `cef_ui.cpu_paint.dirty_rect`. |
| asset load / dynamic image | `bevy_render/src/texture/gpu_image.rs`, `bevy_pbr/src/render/mesh.rs` | Image asset upload and fallback texture setup. Often startup or asset-load, but dynamic image replacement can be hot. |
| per-frame generic buffer helpers | `bevy_render/src/render_resource/{buffer_vec,uniform_buffer,storage_buffer,sparse_buffer_vec,gpu_array_buffer,batched_uniform_buffer}.rs` | Main abstraction layer for many engine uploads. Counter callsites identify concrete helper source locations when active. |
| meshlet / visibility | `bevy_pbr/src/meshlet/resource_manager.rs`, `bevy_render/src/batching/gpu_preprocessing.rs`, `bevy_render/src/batching/no_gpu_preprocessing.rs` | Per-frame visibility, instance, indirect, and meshlet table uploads. Existing diagnostics already count full/range meshlet writes; upload counters add byte totals. |
| clustered lighting / decals / probes | `bevy_pbr/src/cluster`, `bevy_pbr/src/render/light.rs`, `bevy_pbr/src/decal/clustered.rs`, `bevy_pbr/src/light_probe/generate.rs` | Mostly per-frame small/medium buffers and probe generation constants. |
| Solari scene / realtime | `bevy_solari/src/scene/binder.rs`, `bevy_solari/src/realtime/{prepare,node}.rs` | Scene bind data, realtime pass constants, and denoiser/guide parameters. |
| UI render buffers | `bevy_ui_render/src/{lib,gradient,box_shadow,ui_material_pipeline,ui_texture_slice_pipeline}.rs` | Per-frame Bevy UI vertex/index buffers; should be near-zero for the CEF/Svelte UI path except diagnostics/FPS. |
| post-process / atmosphere / SMAA | `bevy_post_process`, `bevy_pbr/src/atmosphere`, `bevy_anti_alias/src/smaa` | Mostly constants and lookup buffers, usually small. |
| debug/dev tools | `bevy_dev_tools`, examples, screenshots/readback | Should not dominate product benchmark lanes unless diagnostic overlays are enabled. |

## Cleanup Targets

First optimization targets should come from measured top-ten callsites, not from
the static inventory alone.

Current local DX12 auto-no-vsync evidence points at Bevy generic
`DynamicUniformBuffer` and `RawBufferVec` helper rows before any `fun_render`
semantic owner is known. Those rows need semantic label splitting or a Bevy
prepare/upload scheduling change before a hot owner can be moved safely.

Priority order:

1. CEF CPU texture uploads when the accelerated DX12 lane is unavailable or
   unhealthy.
2. Per-frame small buffer helpers that appear repeatedly in top-ten byte or call
   counts.
3. Meshlet/visibility buffer range writes if byte totals or call counts dominate
   p95 lanes.
4. Dynamic image uploads that appear outside startup/resize windows.

## Deferred Work

- Add a `fun_render` staging-belt resource only after the top callsites prove
  which small buffers should move first.
- Add a persistent texture-upload ring for unavoidable CPU texture uploads after
  the CEF accelerated path and dirty-rect behavior are measured in the same
  benchmark lane.

## Upload Arena Boundary

`fun_render::FunUploadArena` is available as the narrow staging-belt boundary
for measured hot small-buffer writes that already have a render command encoder.
Do not force Bevy prepare-stage `RenderQueue::write_buffer` helpers through it
by creating ad hoc per-callsite encoders or submits; that would trade upload
allocation pressure for submit and synchronization pressure. Move a hot owner
only after the report's kill list and a render-schedule insertion point agree.

## Pass 5 Resource Ownership Boundary

`fun-renderer/src/resource.rs` is now the renderer-owned resource model. It
defines the resource classes that future passes must allocate through:

| class | current kinds |
|---|---|
| `upload` | staging buffer pages, ring allocations, transient upload batches |
| `transient` | frame-lifetime textures, frame-lifetime buffers, pass-local scratch |
| `persistent` | material tables, mesh tables, page pools, shadow page pools, GI/radiance caches, texture residency pools |
| `imported` | CEF shared textures, swapchain resources, vendor SDK resources |
| `readback_debug` | diagnostics readback, screenshots, benchmark captures |

The current policy is explicit:

| field | value |
|---|---|
| policy owner | `fun-renderer` |
| compatibility shim owner | `fun_render` |
| hidden transient allocations in major passes | forbidden |
| force Bevy prepare-stage helpers through upload arena | false |
| semantic owner required before generic helper migration | true |

`fun_render::FunUploadArena` remains a compatibility shim under that policy. It
is not the final allocator. The shim records that it uses staging-buffer pages,
requires an existing encoder, creates no ad hoc encoder, and must not absorb
generic Bevy prepare-stage writes until their semantic owners are split.

## Current FunUploadArena Audit

| audit field | current status |
|---|---|
| owner module | `fun_render/src/upload_arena.rs` |
| compatibility contract | `FUN_UPLOAD_ARENA_RESOURCE_SHIM` |
| backing primitive | `wgpu::util::StagingBelt` with 1 MiB chunks |
| public API | `new`, `write_buffer_tracked`, `write_buffer_budgeted`, `finish`, `recall_completed`, `stats`, `frame_report` |
| validation | alignment tests plus `upload_arena_is_renderer_resource_compatibility_shim` |
| metrics | per-frame write calls/bytes, raw write fallbacks, label stats, frame report top labels |
| DX12/native assumptions | none directly; callers pass Bevy/wgpu buffers and an existing command encoder |
| current call sites | `fun_render/src/instance_tables.rs` dynamic instance dirty-range uploads |
| call-site restraint | `InstanceUploadPlan::creates_ad_hoc_encoder=false` and `requires_existing_encoder=true` for arena writes |

## Pass 5 Hot Upload Kill List

These entries are now represented by `fun_renderer::resource::HOT_UPLOAD_KILL_LIST`
and tied to `target/dx12-parity/current/dx12_parity_report.json`.

| rank | owner row | current owner | action gate |
|---:|---|---|---|
| 1 | `DynamicUniformBuffer` at `uniform_buffer.rs:311` | Bevy generic render resource | split semantic owner before upload-arena migration |
| 2 | `RawBufferVec` at `buffer_vec.rs:183` | Bevy generic render resource | batch or split semantic owner before upload-arena migration |
| 3 | `DynamicUniformBuffer` at `uniform_buffer.rs:140` | Bevy generic render resource | split semantic owner before upload-arena migration |
| 4 | `RawBufferVec` at `buffer_vec.rs:442` | Bevy generic render resource | batch or split semantic owner before upload-arena migration |

The local before artifact for the new boundary is
`target/benchmarks/client/20260506-005505-031/summary.json`. The Pass 5
parser-only candidate artifact is
`target/benchmarks/client/20260506-011124-520/summary.json`, compared by
`target/dx12-upload/pass5-resource-ownership/upload_perf_report.{md,json}`. A
live after run is not expected to move performance yet because this pass
introduces ownership and diagnostics contracts, not a runtime upload rewrite.
