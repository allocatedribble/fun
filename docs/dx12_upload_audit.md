# DX12 Upload Path Audit

Status: measurement slice, upload cleanup tier 3.1.

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
