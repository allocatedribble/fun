# DX12 PIX Barrier Audit Protocol

## Purpose

Use this protocol when DX12 loses to Vulkan, when CEF accelerated paint changes
frame p95, or before adding another native DX12 interop path. The goal is to
identify redundant resource transitions, queue idle bubbles, descriptor churn,
and accidental synchronization around UI, post-process, clouds, Solari, and
future DLSS work.

The audit is evidence-only. Do not change barriers or resource states until the
capture shows which pass owns the cost.

## Capture Command

Use the same scene and timing window for Vulkan and DX12 benchmarking, then
attach PIX only to the DX12 run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 `
  -RenderBackend dx12 `
  -PresentMode immediate `
  -CefPaintTransport d3d11on12 `
  -CefGpuRingDepth 3 `
  -CefDebugTimings `
  -TraceDiagnostics `
  -SampleSeconds 20 `
  -WarmupSeconds 5
```

For a CPU fallback comparison:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 `
  -RenderBackend dx12 `
  -PresentMode immediate `
  -CefPaintTransport cpu `
  -TraceDiagnostics `
  -SampleSeconds 20 `
  -WarmupSeconds 5
```

For a render-only control:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\benchmark_client.ps1 `
  -RenderBackend dx12 `
  -PresentMode immediate `
  -CefPaintTransport disabled `
  -TraceDiagnostics `
  -SampleSeconds 20 `
  -WarmupSeconds 5
```

PIX capture window:

- Warm up for at least five seconds before capturing.
- Capture 180 consecutive frames for p95-sensitive UI tests.
- Capture 60 consecutive frames for a quick barrier-count inspection.
- Keep resolution, present mode, UI route, Solari/cloud/meshlet flags, and
  camera state fixed across captures.

## Marker Vocabulary

Readable markers are required before a capture is considered useful. Current
markers and tracing targets:

- `fun.cef.copy_ring_source_to_bevy_image`: native DX12 copy from the CEF GPU
  ring into the Bevy UI image.
- `fun::perf::cef_ui`: CEF paint/copy/generation counters in client logs.
- `fun::render`: backend, present, DLSS, and renderer policy.
- `fun::perf::clouds`: cloud pass costs and history state.
- `fun::perf::solari`: Solari pass costs and denoiser guide costs.
- `fun::perf::render_churn`: sampled descriptor/layout/pipeline creation and
  specialized pipeline cache counters.
- `bevy_render::transient`: transient texture/buffer reuse, aliasing,
  descriptor miss reasons, and top descriptor-create rows.
- `bevy_render::scheduler`: render graph pressure.

Future marker names should follow `fun.<subsystem>.<pass>` and should be added
to this document before landing native interop or barrier-sensitive work.

## What To Inspect In PIX

Summarize the capture by pass:

- resource barrier count;
- transition pairs that repeat within the same pass;
- transitions to and from `COMMON`;
- copy or resolve operations that force extra transitions;
- command-list count per frame;
- command-allocator reset count per frame;
- fence waits or CPU/GPU synchronization points;
- queue idle intervals;
- descriptor heap switches;
- pipeline state creation or unexpected PSO churn;
- present wait and swapchain queue depth if visible.

Cross-check PIX PSO/root-signature findings against
`render_churn_render_pipeline_creations`,
`render_churn_compute_pipeline_creations`,
`render_churn_bind_group_layout_creations`, and `render_churn_events` in the
benchmark JSON.

If a transition appears redundant, record the resource name, before state, after
state, producing pass, consuming pass, and whether wgpu or a native DX12 interop
module owns the state change.

## Resource-State Ownership Table

No native DX12 path should touch a texture that is missing from this table.

| Resource | Producer | Consumer | Expected before pass | Expected after pass | State owner | Native interop | Promotion/decay | Aliasing |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Swapchain surface | wgpu surface acquire | final present | wgpu-managed | present | wgpu | no | wgpu-managed | no |
| Main HDR color | main scene lighting | post-process and DLSS SR input placeholder | render target | shader resource or render target for next post pass | wgpu/render graph | future DLSS may read | do not assume | no until audited |
| Depth | depth/prepass and main scene | motion vectors, Solari, DLSS placeholder | depth write/read by pass | depth read or shader resource by consumer | wgpu/render graph | future DLSS may read | do not assume | no |
| Motion vectors | motion-vector pass | temporal reconstruction, DLSS placeholder | render target/storage by pass | shader resource | wgpu/render graph | future DLSS may read | do not assume | no |
| CEF UI image | CEF CPU upload or DX12 GPU copy | Bevy UI composition | pixel shader resource after creation, copy dest during native copy | pixel shader resource | CEF interop module for native copy, then wgpu | yes, CEF GPU path | no | no |
| CEF GPU ring slot | `OnAcceleratedPaint` D3D11On12 copy | Bevy UI image native copy | free/copying ring state | copy source until consumed | `game_client::cef_ui_dx12` | yes | no | ring reuse only after fence |
| Post-process intermediates | post-process passes | next post pass or final composition | render target or shader resource per pass | shader resource or render target per pass | wgpu/render graph | no | do not assume | candidate after PIX proof |
| Cloud transmittance/noise/history | cloud passes | cloud resolve/composite | shader/storage/render target per pass | shader resource/history | wgpu/render graph | no | do not assume | history resources no |
| Solari guide surfaces | Solari guide resolve | denoiser/RR diagnostic path | render/storage by guide pass | shader resource | wgpu/render graph | future RR only | no | no |
| DLSS SR input placeholder | main HDR/post lighting | native DLSS SR | shader resource | shader resource | future dx12 native module | yes, future only | no | no |
| DLSS SR output placeholder | native DLSS SR | post-process | unordered access or render target as SDK requires | shader resource/render target for post | future dx12 native module | yes, future only | no | no |
| Readback/capture textures | copy pass | CPU readback/capture tooling | copy dest | map/readback | wgpu | no | no | no |

## CEF Transition Intent

The current accelerated CEF copy is intentionally full-frame:

```text
CEF D3D11 shared texture
  -> D3D11On12 copy into ring slot
  -> ring slot final D3D12 state COPY_SOURCE
  -> native D3D12 CopyResource into Bevy UI image
  -> Bevy UI image restored to PIXEL_SHADER_RESOURCE
```

Expected PIX result for the CEF pass:

- no blocking fence wait in normal frames;
- at most one transition of the Bevy UI image to `COPY_DEST`;
- one transition back to `PIXEL_SHADER_RESOURCE`;
- no transition of the CEF ring slot through `COMMON` unless PIX or validation
  proves the bridge requires it;
- no CPU `write_texture` upload bytes in accelerated lanes.

## Report Shape

Add this summary beside the benchmark JSON when a PIX capture is used:

```text
DX12 PIX barrier audit:
- Benchmark JSON: target\benchmarks\client\<timestamp>\summary.json
- PIX capture: <path>
- GPU/driver/Windows build:
- Backend/present/frame latency:
- CEF transport: requested=... selected=... ring_depth=... copy_mode=...
- Worst pass by barrier count:
- Worst pass by queue idle:
- CEF pass barriers:
- CEF blocking waits:
- Redundant transition candidates:
- State-table updates needed:
- Verdict: present-bound | upload-bound | barrier-bound | pipeline-churn | CEF-sync | shader-bound | unknown
```

Only mark a pass barrier-bound when PIX shows redundant or excessive
transitions and the client benchmark also shows the matching p95 or pass-time
loss.
