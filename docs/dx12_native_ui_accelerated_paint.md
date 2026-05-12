# DX12 NATIVE_UI Accelerated Paint Audit

## Current State

`game_client` can run the Bevy/wgpu renderer on DX12. The default NATIVE_UI UI
transport is still the CPU paint path, but an experimental Windows-only
`native_ui_dx12_accelerated_paint` feature now builds the D3D11On12 bridge and
per-callback GPU copy boundary.

The May 4, 2026 NATIVE_UI transport decision pass does not promote accelerated NATIVE_UI to
default-on. A 1280x720 animated `d3d11on12` request selected CPU fallback with
`fallback_reason=render_backend_not_dx12`, `bridge_ready=false`, nonzero CPU
upload bytes, zero GPU copy bytes, and zero accelerated paint FPS. The health
badge and NATIVE_UI transport matrix are now wired, but the runtime bridge readiness
issue must be fixed before `auto` can prefer D3D11On12 by default.

Evidence in the current code:

- `fun_ui_native_ui::browser::windowless_window_info` creates a transparent
  windowless browser and enables NATIVE_UI shared textures only for the selected
  `d3d11on12` transport.
- `fun_ui_native_ui` has tests proving CPU transport disables shared textures and
  accelerated transport enables them.
- `fun_ui_native_ui::render_handler::on_paint` receives a BGRA buffer from NATIVE_UI and
  copies it into a Rust-owned frame.
- `game_client::native_ui` copies CPU frames into a Bevy `Image` and uploads them
  with `RenderQueue::write_texture` on the CPU fallback path.
- `game_client/src/native_ui_dx12` opens NATIVE_UI's accelerated D3D11 shared texture
  handle inside `OnAcceleratedPaint`, copies immediately into a FUN-owned
  D3D12-backed texture ring through D3D11On12, signals a D3D12 fence, and
  publishes only a safe frame token to the rest of the UI bridge.
- `game_client::native_ui` consumes that safe token in the existing Bevy UI texture
  composition path. The main world creates an uninitialized BGRA `Image`, and
  the render world copies the ready DX12 ring slot into the same Bevy GPU image
  that the CPU fallback path uses.

This means DX12 is hardware accelerated for the Bevy renderer. The default NATIVE_UI
handoff remains the CPU path:

```text
NATIVE_UI windowless OnPaint
  -> CPU BGRA buffer
  -> fun_ui_native_ui compositor frame
  -> Bevy Image pixel copy
  -> wgpu queue.write_texture
  -> Bevy/FUN render composition
```

The browser paint request and the client-side texture consumer share one
cadence constant. That value is a request to NATIVE_UI, not proof that the UI is
producing or presenting a unique frame on every eligible tick under load.

With `native_ui_dx12_accelerated_paint` and the accelerated transport selected, the
callback-local path is:

```text
NATIVE_UI windowless OnAcceleratedPaint
  -> per-callback D3D11 shared texture handle
  -> ID3D11Device::OpenSharedResource
  -> FUN-owned D3D12-backed ring slot wrapped by D3D11On12
  -> AcquireWrappedResources
  -> ID3D11DeviceContext::CopyResource
  -> ReleaseWrappedResources
  -> Flush
  -> ID3D12CommandQueue::Signal
  -> publish NativeUiFrameGeneration + ring slot token
  -> render-world D3D12 CopyResource into the Bevy UI Image texture
  -> Bevy/FUN render composition
```

The NATIVE_UI handle, source `ID3D11Texture2D`, dirty-rect slice, and
`NativeUiAcceleratedPaintInfo` are callback-local. They are not cached or enqueued
for later render-world processing.

## User-Visible FPS

Launcher and editor routes now show a compact `UI ... fps` badge driven by the
Svelte page's `requestAnimationFrame` loop. It measures the browser page's UI
frame cadence, not the Bevy render frame rate and not NATIVE_UI paint callback rate.

This is intentionally separate from the Bevy FPS counter:

- launcher/editor badge: Svelte/NATIVE_UI page responsiveness.
- Bevy FPS counter: game/render frame cadence, visible only in game or the
  editor preview placement.

## NATIVE_UI Accelerated Paint Research

NATIVE_UI's render handler exposes `OnAcceleratedPaint` for windowless rendering when
shared texture mode is enabled. In the current NATIVE_UI documentation, the callback
receives `NativeUiAcceleratedPaintInfo`; on Windows, the shared handle is a texture
handle that can be opened through D3D11. NATIVE_UI also documents that the handle can
change per frame, must be reopened each callback, and cannot be cached or used
after the callback returns.

Primary references:

- NATIVE_UI `NativeUiRenderHandler::OnAcceleratedPaint`:
  https://native_ui-builds.spotifycdn.com/docs/130.0/classNativeUiRenderHandler.html
- NATIVE_UI `native_ui_accelerated_paint_info_t`:
  https://native_ui-builds.spotifycdn.com/docs/131.2/structnative_ui__accelerated__paint__info__t.html

## DX12 Interop Direction

For the FUN DX12 backend, the accelerated NATIVE_UI path should not import NATIVE_UI's frame
as a CPU buffer. The target path is:

```text
NATIVE_UI windowless OnAcceleratedPaint
  -> per-callback D3D11 shared texture handle
  -> D3D11 OpenSharedResource
  -> D3D11on12 bridge tied to the active D3D12 device and 3D queue
  -> copy/resolve into a FUN-owned D3D12/wgpu texture
  -> Bevy/FUN render composition
```

Microsoft's D3D11on12 documentation describes creating a D3D11 device over an
existing D3D12 device and command queue, then using wrapped resources with
`AcquireWrappedResources`, `ReleaseWrappedResources`, and an immediate-context
`Flush` for synchronization and state tracking.

Primary references:

- Direct3D 11 on 12 overview:
  https://learn.microsoft.com/en-us/windows/win32/direct3d12/direct3d-11-on-12
- `D3D11On12CreateDevice`:
  https://learn.microsoft.com/en-us/windows/win32/api/d3d11on12/nf-d3d11on12-d3d11on12createdevice
- `ID3D11On12Device::AcquireWrappedResources`:
  https://learn.microsoft.com/en-us/windows/win32/api/d3d11on12/nf-d3d11on12-id3d11on12device-acquirewrappedresources
- `ID3D11On12Device::ReleaseWrappedResources`:
  https://learn.microsoft.com/en-us/windows/win32/api/d3d11on12/nf-d3d11on12-id3d11on12device-releasewrappedresources

## Required Implementation Boundary

The first accelerated-paint implementation adds one Windows-only unsafe interop
module, not scattered renderer calls.

Suggested shape:

```text
fun_ui_native_ui
  browser/shared_texture_enabled config
  render_handler/OnAcceleratedPaint callback surface
  diagnostics for paint transport and accelerated paint failures

game_client or fun_render
  dx12_native_ui_interop/
    d3d11on12 device creation
    D3D11 shared texture open/copy
    D3D12/wgpu texture ownership
    synchronization and fallback
```

Rules:

- Keep the existing CPU `OnPaint` path as the fallback.
- Enable shared textures only on Windows DX12 after the D3D11on12 bridge is
  ready.
- Do not cache NATIVE_UI's shared handle outside `OnAcceleratedPaint`.
- Copy into a FUN-owned D3D12/wgpu texture before NATIVE_UI returns the frame to its
  pool.
- Keep all wgpu HAL extraction inside `fun_render::dx12_native`; NATIVE_UI-specific
  D3D11On12 COM work stays inside `game_client/src/native_ui_dx12`.
- Log the requested and selected paint transport on startup, including
  `disabled`, `cpu`, `d3d11on12`, and `auto` decisions.
- Expose paint callback FPS separately from Svelte `requestAnimationFrame` FPS
  before using it for performance claims.

## Transport Selection Slice

`fun_ui_native_ui` owns the explicit paint transport contract:

```rust
pub enum NativeUiPaintTransport {
    Disabled,
    Auto,
    CpuPaint,
    D3d11On12Accelerated,
}
```

`BrowserUiConfig` carries the selected transport, the render-backend hint, the
strict fallback policy, the GPU ring depth, the copy mode, timing-debug state,
and a typed fallback reason. The compatibility lane remains CPU paint, while
`auto` can select accelerated paint after the render-world D3D11On12 bridge
reports ready. `disabled` starts no NATIVE_UI browser and is intended for render-only
DX12 comparison lanes.

Environment gates:

- `FUN_NATIVE_UI_PAINT_TRANSPORT=disabled|cpu|d3d11on12|auto`
- `FUN_NATIVE_UI_ACCELERATED_STRICT=0|1`
- `FUN_NATIVE_UI_GPU_RING_DEPTH=2|3|4|5`
- `FUN_NATIVE_UI_COPY_DIRTY_RECTS=0|1`
- `FUN_NATIVE_UI_DEBUG_TIMINGS=0|1`

`FUN_NATIVE_UI_ACCELERATED_PAINT=0|1|auto` is still accepted as a compatibility
alias for older scripts, but new tooling should prefer
`FUN_NATIVE_UI_PAINT_TRANSPORT`.

Startup logs include:

- requested transport
- selected transport
- backend hint
- Windows target flag
- NATIVE_UI shared-texture flag
- D3D11On12 readiness
- strict/fallback policy
- GPU ring depth
- copy mode (`full_frame` for the current implementation)
- fallback reason

The parser-stable line is:

```text
[client perf] native_ui transport selected: requested=auto selected=d3d11on12 backend=dx12 bridge_ready=true cpu_fallback_enabled=true ring_depth=3 copy_mode=full_frame strict=false debug_timings=false fallback_reason=none
```

Benchmark summaries record this as `native_ui_transport_selection` so DX12 parity
reports can distinguish a real accelerated lane from a CPU fallback lane.

Every benchmark sample also emits a separate health badge line. This is the
developer-facing transport badge; it is independent of the Svelte RAF FPS badge:

```text
[client perf] native_ui transport health: transport=d3d11on12 status=healthy accel_paint_fps=<rate> paint_fps=0 gpu_copy_ms=0.180 gpu_copy_ns_per_copy=180000 cpu_upload_bytes_per_frame=0 reused_frames=2 not_ready_frames=0 blocking_waits=0 fallback_count=0 ring_depth=3
```

`fun-bench client` records the latest line as `native_ui_transport_health` and
also folds numeric fields into `native_ui_health_*` metrics. The DX12 parity dashboard
renders a compact NATIVE_UI Transport Health section in the form:

```text
NATIVE_UI transport: d3d11on12 | accelerated paint active | gpu copy 0.18 ms | CPU upload 0 B/frame | reused 2 frames | fallback 0
```

Use that badge, not the Svelte UI FPS badge, when deciding whether the NATIVE_UI path
is truly accelerated.

The render handler now has an `OnAcceleratedPaint` surface, but it only records
the callback and dispatches a borrowed `NativeUiAcceleratedPaintFrame` to an optional
Windows-only sink. This is intentional: NATIVE_UI's shared handle is only valid during
the callback, can change every callback, and must not be enqueued for
render-world processing. The `game_client` bridge opens the D3D11 shared texture
and copies it into a FUN-owned GPU resource before the callback returns.

The accelerated callback surface is callback-only:

```rust
#[cfg(windows)]
pub struct NativeUiAcceleratedPaintFrame<'a> {
    pub element: NativeUiPaintElement,
    pub width: i32,
    pub height: i32,
    pub dirty_rects: &'a [NativeUiDirtyRect],
    pub shared_handle: *mut std::ffi::c_void,
    pub timestamp_ns: NativeUiFrameTimestampNs,
    pub alpha_mode: NativeUiAlphaMode,
}
```

The CPU sink remains unchanged. The optional accelerated sink returns
`Accepted`, `Dropped`, or `FallbackRequested`. `Accepted` carries the published
generation plus copy byte/time counters, but never carries NATIVE_UI's source handle
or source texture. With the
`native_ui_dx12_accelerated_paint` feature, `game_client` installs the shared
interop slot as that sink. The sink dispatches to the bridge when initialized;
after the bridge opens and copies the shared texture, it returns `Accepted`.
Failures request CPU fallback with typed reasons such as
`shared_texture_unsupported` or `output_texture_allocation_unavailable`.

## Startup Bridge Gate

NATIVE_UI shared texture mode is selected at browser creation time, but the D3D11On12
bridge needs Bevy/wgpu's active render device and queue. `game_client` therefore
does not create an accelerated browser from `main` before Bevy render resources
exist.

The current startup model is:

```text
main world
  NativeUiStartupState
  NativeUiRequestedPaintTransportResource
  SharedDx12NativeUiInteropSlot

render world
  observes RenderDevice + RenderQueue
  initializes or rejects the D3D11On12 bridge
  writes Ready/Error into SharedDx12NativeUiInteropSlot

main world
  starts CPU immediately for cpu requests
  waits briefly for accelerated/auto requests
  starts accelerated only after Ready
  falls back to CPU on Error or timeout
```

When `native_ui_dx12_accelerated_paint` is not compiled, the render-world slot
records `d3d11on12_bridge_unavailable` after it observes the render device and
queue. When the feature is compiled, `game_client/src/native_ui_dx12` builds the
NATIVE_UI-specific D3D11On12 bridge, but all wgpu DX12 HAL extraction goes through
`fun_render::dx12_native` as documented in
`docs/dx12_native_interop_governance.md`. The bridge:

- confirms the active wgpu backend is DX12;
- clones the active `ID3D12Device` and `ID3D12CommandQueue`;
- calls `D3D11On12CreateDevice` with `D3D11_CREATE_DEVICE_BGRA_SUPPORT`;
- queries `ID3D11On12Device`;
- creates an `ID3D12Fence`;
- creates a reusable direct command allocator/list pair for ring-slot to Bevy
  texture copies;
- initializes an empty texture ring with depth clamped to the supported
  `2..=5` slot range.
- logs the first-pass GPU format policy:
  `NATIVE_UI UI GPU format source=BGRA8 target=BGRA8 conversion=none alpha=premultiplied`.

The main world starts accelerated NATIVE_UI only after the slot reports `Ready`. If
bridge initialization fails, the client starts a CPU browser with a typed
fallback reason unless strict accelerated mode is enabled. Strict mode marks the
startup state as failed and logs the exact fallback reason instead of silently
creating the CPU browser. If an accelerated browser starts and NATIVE_UI produces CPU
`OnPaint` frames or no accelerated callbacks during the startup observation
window, the same strict-vs-fallback policy applies with
`accelerated_paint_not_observed`.

## Render-World Bevy Texture Feed

The first accelerated feed keeps the existing Bevy UI image composition. It only
replaces the transport into that image:

```text
old:
  CPU pixels -> RenderQueue::write_texture -> Bevy Image

new:
  D3D12 ring slot -> native D3D12 copy -> Bevy Image
```

`copy_latest_native_ui_gpu_frame_to_bevy_image` runs in
`RenderSystems::PrepareResources` after the CPU upload lane. It:

- reads the extracted `NativeUiGpuTextureUpload` token;
- consumes only frames whose NATIVE_UI callback fence has completed;
- reuses the last sampled frame when the newest accelerated frame is not ready
  instead of blocking the render frame;
- validates BGRA8 source and `Bgra8UnormSrgb` target formats;
- uses the active Bevy `GpuImage` texture as the copy target;
- records a native DX12 `CopyResource` through the isolated interop module;
- transitions the target texture to `COPY_DEST` for the copy and back to
  `PIXEL_SHADER_RESOURCE` for Bevy UI sampling;
- updates `NativeUiGpuUploadState.last_generation` only after the GPU copy has
  been submitted.

Normal rendering must not wait on NATIVE_UI fences. Blocking waits are reserved for
shutdown, resize/device teardown, or strict debug recovery. The GPU path records
ready, not-ready, reused, and blocking-wait counters so p95 regressions can be
attributed to interop synchronization instead of blended into generic frame
time.

The first GPU transport deliberately copies the full NATIVE_UI frame for correctness.
Dirty rectangles are still retained as metadata on the safe generation token,
not as callback-local NATIVE_UI resources. The token carries the dirty rect count,
dirty rect union, dirty rect explosion count, and full-frame reason. For normal
accelerated frames the reason is `GpuFullFrameFirstPass`; empty dirty rect lists
and dirty rect explosions keep the same full-frame reasons used by the CPU
compositor.

NATIVE_UI is composed as HUD/UI after the world image is finished. The combined order
is:

```text
World render
  -> depth/motion vectors
  -> Solari / lighting
  -> DLSS SR / RR if active
  -> post-processing
  -> NATIVE_UI UI composition
  -> debug overlays / FPS counters
  -> present
```

The NATIVE_UI texture is not a DLSS input or guide surface. It must not feed input
color, depth, motion vectors, or Ray Reconstruction guide buffers because it has
no world-space motion-vector convention and should not enter temporal
reconstruction history.

Resource states for this pass:

- NATIVE_UI source shared texture: D3D11 resource opened and released only during
  `OnAcceleratedPaint`; never cached.
- FUN ring slot: copied through D3D11On12, released in
  `D3D12_RESOURCE_STATE_COPY_SOURCE`, and held until the Bevy copy fence retires.
- Bevy UI target texture: treated as `COMMON` on first allocation and
  `PIXEL_SHADER_RESOURCE` on later copies, transitioned to `COPY_DEST` for the
  native copy, then restored to `PIXEL_SHADER_RESOURCE`.

First successful frames log:

```text
NATIVE_UI UI GPU copy ready generation=... size=... source=native_ui_d3d11_shared target_slot=dx12_ring_slot_N
NATIVE_UI UI GPU frame copied to Bevy image generation=... target_format=Bgra8UnormSrgb
```

## Transport Counters

GPU transport claims must use NATIVE_UI/FUN counters, not the Svelte
`requestAnimationFrame` badge.

Current counters exposed through `game_client::native_ui::NativeUiFrameStats`:

- `native_ui_on_paint_fps`
- `native_ui_on_accelerated_paint_fps`
- `accelerated_paint_count`
- `dirty_rect_count`
- `dirty_rect_explosion_count`
- `last_dirty_rect_union`
- `full_frame_upload_count`
- `last_full_frame_reason`
- `native_ui_cpu_upload_bytes`
- `gpu_copied_bytes`
- `gpu_copy_count`
- `gpu_copy_fail_count`
- `cpu_fallback_count`
- `shared_texture_open_fail_count`
- `stale_gpu_frame_count`
- `native_ui_gpu_copy_count`
- `native_ui_gpu_copy_bytes`
- `native_ui_gpu_copy_ns`
- `native_ui_gpu_copy_failures`
- `native_ui_gpu_frame_ready_count`
- `native_ui_gpu_frame_not_ready_count`
- `native_ui_gpu_frame_reused_count`
- `native_ui_gpu_frame_blocking_wait_count`
- `native_ui_transport_fallback_count`
- `native_ui_published_generation`
- `native_ui_sampled_generation`

`native_ui_gpu_copy_*` remains zero on the CPU path. It moves only when the D3D11On12
copy into a FUN-owned D3D12 texture ring succeeds. The current bridge has its
own startup/copy diagnostic counters in `game_client/src/native_ui_dx12`, including
the last published generation, fence value, ring slot count, and nonblocking
frame-consumption state.

Keep FPS readings separate:

- UI RAF FPS: the Svelte page's `requestAnimationFrame` responsiveness.
- NATIVE_UI paint FPS: the CPU `OnPaint` callback cadence.
- NATIVE_UI accelerated paint FPS: the `OnAcceleratedPaint` callback cadence.
- Bevy FPS: the game/render frame cadence.

The UI RAF badge is not evidence of NATIVE_UI paint callback cadence or GPU transport.

## NATIVE_UI Transport Matrix

`fun-bench dx12-parity --matrix-size native_ui_transport` expands the NATIVE_UI
decision lanes:

- hidden control;
- CPU paint and D3D11On12 lanes for static and animated Svelte pages;
- current window, 1080p, and 1440p surface sizes;
- D3D11On12 ring-depth tuning for depths 2, 3, 4, and 5.

The matrix summary includes selected transport, ring depth, health status,
accelerated paint FPS, CPU upload bytes, GPU copy time, blocking waits, and
fallback count per lane. Resize, alt-tab, editor route, launcher route, and
static screenshot-diff checks remain explicit manual evidence rows in the
matrix artifact because they require window interaction or screenshot capture.

The ring-depth decision rule is unchanged: prefer the smallest depth with zero
normal-frame blocking waits, no callback/render contention, no stale-frame
bursts, and acceptable latency. Until a complete healthy matrix says otherwise,
the documented default remains `3`.

## Static Visual Match

When validating the accelerated path, capture the same static UI route once with
`--native_ui-paint-transport cpu` and once with `--native_ui-paint-transport d3d11on12`. Compare
the screenshots with:

```text
fun-data report dx12-parity `
  -CpuReference target\captures\native_ui_cpu.png `
  -GpuCandidate target\captures\native_ui_gpu.png `
  -JsonOut target\captures\native_ui_screenshot_diff.json
```

The script checks dimensions, max per-channel difference, mean channel
difference, and changed-pixel ratio. Use this as the static UI guard for
swapped channels, bad transparency, and obvious gamma/color-space drift. Runtime
benchmark lanes still own paint cadence, GPU-copy timing, and fallback behavior.

## Fallback Policy

The accelerated path is experimental and must never crash the app. CPU paint
remains the compatibility lane. `game_client/src/native_ui_dx12` exposes:

```rust
pub const MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK: u32 = 8;
```

The accelerated callback copy records failures but does not request CPU fallback
until the same accelerated transport has failed eight consecutive callback
copies. A successful callback copy resets the budget. The Bevy-image GPU feed
uses the same eight-frame budget for native copy errors. Ready ring-slot tokens
that have not completed their fence are counted as not-ready/reused frames and
do not block or trigger CPU fallback during normal rendering. Strict mode can
promote repeated not-ready GPU frames to a loud fallback/error signal for
debugging. When a real fallback budget is exhausted, the shared interop slot
records a typed fallback reason and the main-world startup monitor recreates the
browser on the CPU paint path unless strict mode is enabled.

Fallback triggers covered by this pass:

- D3D11On12 initialization failure.
- null or missing accelerated shared texture handle.
- repeated `OpenSharedResource` failures.
- unsupported source format.
- invalid accelerated frame dimensions.
- Bevy target texture or native texture extraction failure.
- ring-slot/output texture allocation failure.
- native Bevy image GPU copy failures.
- copy fence or ready GPU frame stalling past the frame budget in strict mode.
- DX12 device/queue extraction failure.
- backend mismatch before accelerated browser creation.
- accelerated callback not observed after browser creation.

## Open Risks

- NATIVE_UI's accelerated texture is D3D11-facing, while Bevy/wgpu DX12 owns D3D12
  resources.
- D3D11on12 is intended for interop and 2D composition, not heavy 3D work; use it
  only for the browser copy/resolve stage.
- Resource state ownership must be explicit. The D3D11on12 acquire/release calls
  must match the D3D12 states expected by the downstream FUN render path.
- Dirty rectangles should be preserved for future partial-copy optimization, but
  the first implementation should copy the full NATIVE_UI frame for correctness.

## Dirty Rectangles After Correctness

The GPU path currently preserves dirty rect metadata but does not do partial GPU
copies. The callback thread opens the NATIVE_UI shared texture, copies the full frame
into a FUN-owned D3D12 ring slot, records the dirty rect metadata on that slot,
and publishes only the generation token. This keeps the correctness path simple
and avoids retaining NATIVE_UI handles, source textures, or borrowed dirty rect slices
after `OnAcceleratedPaint` returns.

The later optimization path is intentionally narrow:

```text
NATIVE_UI dirty rects
  -> D3D11 CopySubresourceRegion per rect, or D3D12 CopyTextureRegion per rect
  -> coalesce when rect count exceeds the threshold
  -> full copy on resize, scale change, empty dirty rects, or dirty rect explosion
```
