# DX12 CEF Accelerated Paint Audit

## Current State

`game_client` can run the Bevy/wgpu renderer on DX12. The default CEF UI
transport is still the CPU paint path, but an experimental Windows-only
`cef_ui_dx12_accelerated_paint` feature now builds the D3D11On12 bridge and
per-callback GPU copy boundary.

Evidence in the current code:

- `fun_ui_cef::browser::windowless_window_info` creates a transparent
  windowless browser and enables CEF shared textures only for the selected
  `d3d11_shared_texture_dx12_copy` transport.
- `fun_ui_cef` has tests proving CPU transport disables shared textures and
  accelerated transport enables them.
- `fun_ui_cef::render_handler::on_paint` receives a BGRA buffer from CEF and
  copies it into a Rust-owned frame.
- `game_client::cef_ui` copies CPU frames into a Bevy `Image` and uploads them
  with `RenderQueue::write_texture` on the CPU fallback path.
- `game_client/src/cef_ui_dx12` opens CEF's accelerated D3D11 shared texture
  handle inside `OnAcceleratedPaint`, copies immediately into a FUN-owned
  D3D12-backed texture ring through D3D11On12, signals a D3D12 fence, and
  publishes only a safe frame token to the rest of the UI bridge.
- `game_client::cef_ui` consumes that safe token in the existing Bevy UI texture
  composition path. The main world creates an uninitialized BGRA `Image`, and
  the render world copies the ready DX12 ring slot into the same Bevy GPU image
  that the CPU fallback path uses.

This means DX12 is hardware accelerated for the Bevy renderer. The default CEF
handoff remains the CPU path:

```text
CEF windowless OnPaint
  -> CPU BGRA buffer
  -> fun_ui_cef compositor frame
  -> Bevy Image pixel copy
  -> wgpu queue.write_texture
  -> Bevy/FUN render composition
```

`CEF_UI_WINDOWLESS_FRAME_RATE_HZ` and `CEF_UI_RENDER_RATE_HZ` are both 60, so the
browser is configured to request 60 Hz paints. That value is a target, not proof
that the CEF UI is producing or presenting 60 unique frames under load.

With `cef_ui_dx12_accelerated_paint` and the accelerated transport selected, the
callback-local path is:

```text
CEF windowless OnAcceleratedPaint
  -> per-callback D3D11 shared texture handle
  -> ID3D11Device::OpenSharedResource
  -> FUN-owned D3D12-backed ring slot wrapped by D3D11On12
  -> AcquireWrappedResources
  -> ID3D11DeviceContext::CopyResource
  -> ReleaseWrappedResources
  -> Flush
  -> ID3D12CommandQueue::Signal
  -> publish CefUiFrameGeneration + ring slot token
  -> render-world D3D12 CopyResource into the Bevy UI Image texture
  -> Bevy/FUN render composition
```

The CEF handle, source `ID3D11Texture2D`, dirty-rect slice, and
`CefAcceleratedPaintInfo` are callback-local. They are not cached or enqueued
for later render-world processing.

## User-Visible FPS

Launcher and editor routes now show a compact `UI ... fps` badge driven by the
Svelte page's `requestAnimationFrame` loop. It measures the browser page's UI
frame cadence, not the Bevy render frame rate and not CEF paint callback rate.

This is intentionally separate from the Bevy FPS counter:

- launcher/editor badge: Svelte/CEF page responsiveness.
- Bevy FPS counter: game/render frame cadence, visible only in game or the
  editor preview placement.

## CEF Accelerated Paint Research

CEF's render handler exposes `OnAcceleratedPaint` for windowless rendering when
shared texture mode is enabled. In the current CEF documentation, the callback
receives `CefAcceleratedPaintInfo`; on Windows, the shared handle is a texture
handle that can be opened through D3D11. CEF also documents that the handle can
change per frame, must be reopened each callback, and cannot be cached or used
after the callback returns.

Primary references:

- CEF `CefRenderHandler::OnAcceleratedPaint`:
  https://cef-builds.spotifycdn.com/docs/130.0/classCefRenderHandler.html
- CEF `cef_accelerated_paint_info_t`:
  https://cef-builds.spotifycdn.com/docs/131.2/structcef__accelerated__paint__info__t.html

## DX12 Interop Direction

For the FUN DX12 backend, the accelerated CEF path should not import CEF's frame
as a CPU buffer. The target path is:

```text
CEF windowless OnAcceleratedPaint
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
fun_ui_cef
  browser/shared_texture_enabled config
  render_handler/OnAcceleratedPaint callback surface
  diagnostics for paint transport and accelerated paint failures

game_client or fun_render
  dx12_cef_interop/
    d3d11on12 device creation
    D3D11 shared texture open/copy
    D3D12/wgpu texture ownership
    synchronization and fallback
```

Rules:

- Keep the existing CPU `OnPaint` path as the fallback.
- Enable shared textures only on Windows DX12 after the D3D11on12 bridge is
  ready.
- Do not cache CEF's shared handle outside `OnAcceleratedPaint`.
- Copy into a FUN-owned D3D12/wgpu texture before CEF returns the frame to its
  pool.
- Keep all raw COM pointer and wgpu HAL extraction inside one narrow module.
- Log the selected paint transport on startup: `cpu_paint` or
  `d3d11_shared_texture_dx12_copy`.
- Expose paint callback FPS separately from Svelte `requestAnimationFrame` FPS
  before using it for performance claims.

## Transport Selection Slice

`fun_ui_cef` owns the explicit paint transport contract:

```rust
pub enum CefUiPaintTransport {
    CpuPaint,
    D3d11SharedTextureDx12Copy,
}
```

`BrowserUiConfig` carries the selected transport, the render-backend hint, the
accelerated-paint debug flag, and a typed fallback reason. The production
default remains the CPU lane, but explicit or auto accelerated requests can now
start the shared-texture browser after the render-world D3D11on12 bridge reports
ready. If that bridge is unavailable, the request still fails closed to the CPU
lane with a typed fallback reason.

Environment gates:

- `FUN_CEF_UI_PAINT_TRANSPORT=cpu|auto|d3d11on12`
- `FUN_CEF_UI_ACCELERATED_PAINT=0|1|auto`
- `FUN_CEF_UI_ACCELERATED_PAINT_DEBUG=0|1`

Startup logs include:

- selected transport
- backend hint
- Windows target flag
- CEF shared-texture flag
- D3D11on12 readiness
- fallback reason

The render handler now has an `OnAcceleratedPaint` surface, but it only records
the callback and dispatches a borrowed `CefAcceleratedPaintFrame` to an optional
Windows-only sink. This is intentional: CEF's shared handle is only valid during
the callback, can change every callback, and must not be enqueued for
render-world processing. The `game_client` bridge opens the D3D11 shared texture
and copies it into a FUN-owned GPU resource before the callback returns.

The accelerated callback surface is callback-only:

```rust
#[cfg(windows)]
pub struct CefAcceleratedPaintFrame<'a> {
    pub element: CefPaintElement,
    pub width: i32,
    pub height: i32,
    pub dirty_rects: &'a [CefDirtyRect],
    pub shared_handle: *mut std::ffi::c_void,
    pub timestamp_ns: CefUiFrameTimestampNs,
    pub alpha_mode: CefUiAlphaMode,
}
```

The CPU sink remains unchanged. The optional accelerated sink returns
`Accepted`, `Dropped`, or `FallbackRequested`. `Accepted` carries the published
generation plus copy byte/time counters, but never carries CEF's source handle
or source texture. With the
`cef_ui_dx12_accelerated_paint` feature, `game_client` installs the shared
interop slot as that sink. The sink dispatches to the bridge when initialized;
after the bridge opens and copies the shared texture, it returns `Accepted`.
Failures request CPU fallback with typed reasons such as
`shared_texture_unsupported` or `output_texture_allocation_unavailable`.

## Startup Bridge Gate

CEF shared texture mode is selected at browser creation time, but the D3D11On12
bridge needs Bevy/wgpu's active render device and queue. `game_client` therefore
does not create an accelerated browser from `main` before Bevy render resources
exist.

The current startup model is:

```text
main world
  CefUiStartupState
  CefUiRequestedPaintTransportResource
  SharedDx12CefInteropSlot

render world
  observes RenderDevice + RenderQueue
  initializes or rejects the D3D11On12 bridge
  writes Ready/Error into SharedDx12CefInteropSlot

main world
  starts CPU immediately for cpu requests
  waits briefly for accelerated/auto requests
  starts accelerated only after Ready
  falls back to CPU on Error or timeout
```

When `cef_ui_dx12_accelerated_paint` is not compiled, the render-world slot
records `d3d11on12_bridge_unavailable` after it observes the render device and
queue. When the feature is compiled, `game_client/src/cef_ui_dx12` is the only
module that extracts wgpu DX12 HAL handles. It:

- confirms the active wgpu backend is DX12;
- clones the active `ID3D12Device` and `ID3D12CommandQueue`;
- calls `D3D11On12CreateDevice` with `D3D11_CREATE_DEVICE_BGRA_SUPPORT`;
- queries `ID3D11On12Device`;
- creates an `ID3D12Fence`;
- creates a reusable direct command allocator/list pair for ring-slot to Bevy
  texture copies;
- initializes an empty triple-buffer texture ring.
- logs the first-pass GPU format policy:
  `CEF UI GPU format source=BGRA8 target=BGRA8 conversion=none alpha=premultiplied`.

The main world starts accelerated CEF only after the slot reports `Ready`. If
bridge initialization fails, the client starts a CPU browser with a typed
fallback reason. If an accelerated browser starts and CEF produces CPU `OnPaint`
frames or no accelerated callbacks during the startup observation window, the
client tears down that browser and recreates a CPU paint browser with
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

`copy_latest_cef_gpu_frame_to_bevy_image` runs in
`RenderSystems::PrepareResources` after the CPU upload lane. It:

- reads the extracted `CefUiGpuTextureUpload` token;
- waits until the CEF callback fence has completed;
- validates BGRA8 source and `Bgra8UnormSrgb` target formats;
- uses the active Bevy `GpuImage` texture as the copy target;
- records a native DX12 `CopyResource` through the isolated interop module;
- transitions the target texture to `COPY_DEST` for the copy and back to
  `PIXEL_SHADER_RESOURCE` for Bevy UI sampling;
- updates `CefUiGpuUploadState.last_generation` only after the GPU copy has
  been submitted.

Resource states for this pass:

- CEF source shared texture: D3D11 resource opened and released only during
  `OnAcceleratedPaint`; never cached.
- FUN ring slot: copied through D3D11On12, released in
  `D3D12_RESOURCE_STATE_COPY_SOURCE`, and held until the Bevy copy fence retires.
- Bevy UI target texture: treated as `COMMON` on first allocation and
  `PIXEL_SHADER_RESOURCE` on later copies, transitioned to `COPY_DEST` for the
  native copy, then restored to `PIXEL_SHADER_RESOURCE`.

First successful frames log:

```text
CEF UI GPU copy ready generation=... size=... source=cef_d3d11_shared target_slot=dx12_ring_slot_N
CEF UI GPU frame copied to Bevy image generation=... target_format=Bgra8UnormSrgb
```

## Transport Counters

GPU transport claims must use CEF/FUN counters, not the Svelte
`requestAnimationFrame` badge.

Current counters exposed through `game_client::cef_ui::CefUiFrameStats`:

- `cef_on_paint_fps`
- `cef_on_accelerated_paint_fps`
- `accelerated_paint_count`
- `cef_cpu_upload_bytes`
- `gpu_copied_bytes`
- `gpu_copy_count`
- `gpu_copy_fail_count`
- `cpu_fallback_count`
- `shared_texture_open_fail_count`
- `stale_gpu_frame_count`
- `cef_gpu_copy_count`
- `cef_gpu_copy_bytes`
- `cef_gpu_copy_ns`
- `cef_gpu_copy_failures`
- `cef_transport_fallback_count`
- `cef_published_generation`
- `cef_sampled_generation`

`cef_gpu_copy_*` remains zero on the CPU path. It moves only when the D3D11On12
copy into a FUN-owned D3D12 texture ring succeeds. The current bridge has its
own startup/copy diagnostic counters in `game_client/src/cef_ui_dx12`, including
the last published generation and fence value.

Keep FPS readings separate:

- UI RAF FPS: the Svelte page's `requestAnimationFrame` responsiveness.
- CEF paint FPS: the CPU `OnPaint` callback cadence.
- CEF accelerated paint FPS: the `OnAcceleratedPaint` callback cadence.
- Bevy FPS: the game/render frame cadence.

The UI RAF badge is not evidence of CEF paint callback cadence or GPU transport.

## Fallback Policy

The accelerated path is experimental and must never crash the app. CPU paint
remains the compatibility lane. `game_client/src/cef_ui_dx12` exposes:

```rust
pub const MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK: u32 = 8;
```

The accelerated callback copy records failures but does not request CPU fallback
until the same accelerated transport has failed eight consecutive callback
copies. A successful callback copy resets the budget. The Bevy-image GPU feed
uses the same eight-frame budget for native copy errors and for ready ring-slot
tokens that remain unsampled long enough to imply a stalled fence or stale GPU
frame. When the budget is exhausted, the shared interop slot records a typed
fallback reason and the main-world startup monitor recreates the browser on the
CPU paint path.

Fallback triggers covered by this pass:

- D3D11On12 initialization failure.
- null or missing accelerated shared texture handle.
- repeated `OpenSharedResource` failures.
- unsupported source format.
- invalid accelerated frame dimensions.
- Bevy target texture or native texture extraction failure.
- ring-slot/output texture allocation failure.
- copy fence or ready GPU frame stalling past the frame budget.
- DX12 device/queue extraction failure.
- backend mismatch before accelerated browser creation.
- accelerated callback not observed after browser creation.

## Open Risks

- CEF's accelerated texture is D3D11-facing, while Bevy/wgpu DX12 owns D3D12
  resources.
- D3D11on12 is intended for interop and 2D composition, not heavy 3D work; use it
  only for the browser copy/resolve stage.
- Resource state ownership must be explicit. The D3D11on12 acquire/release calls
  must match the D3D12 states expected by the downstream FUN render path.
- Dirty rectangles should be preserved for future partial-copy optimization, but
  the first implementation should copy the full CEF frame for correctness.
