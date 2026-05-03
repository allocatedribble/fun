# DX12 CEF Accelerated Paint Audit

## Current State

`game_client` can run the Bevy/wgpu renderer on DX12, but the CEF UI transport is
still the CPU paint path.

Evidence in the current code:

- `fun_ui_cef::browser::windowless_window_info` creates a transparent
  windowless browser, but does not enable CEF shared textures.
- `fun_ui_cef` has a test asserting `shared_texture_enabled == 0` for the
  windowless browser.
- `fun_ui_cef::render_handler::on_paint` receives a BGRA buffer from CEF and
  copies it into a Rust-owned frame.
- `game_client::cef_ui` copies that frame into a Bevy `Image` and uploads it
  with `RenderQueue::write_texture`.

This means DX12 is hardware accelerated for the Bevy renderer, but the CEF to
Bevy handoff is not. The current handoff is:

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

The first accelerated-paint implementation should add one Windows-only unsafe
interop module, not scattered renderer calls.

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
accelerated-paint debug flag, and a typed fallback reason. The current production
selection still resolves to `cpu_paint` because the D3D11on12 bridge is not
implemented yet. Explicit accelerated requests therefore fail closed to the CPU
lane and record `d3d11on12_bridge_unavailable`.

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
the callback and rejects it until the GPU bridge exists. This is intentional:
CEF's shared handle is only valid during the callback, can change every callback,
and must not be enqueued for render-world processing. The future bridge must open
the D3D11 shared texture and copy it into a FUN-owned GPU resource before the
callback returns.

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

Until the bridge exists, the render-world slot records
`d3d11on12_bridge_unavailable` after it observes the render device and queue.
This keeps the browser on the CPU path without enabling CEF shared textures
blindly. If a future accelerated browser starts and CEF produces CPU `OnPaint`
frames or no accelerated callbacks during the startup observation window, the
client tears down that browser and recreates a CPU paint browser with
`accelerated_paint_not_observed`.

## Transport Counters

GPU transport claims must use CEF/FUN counters, not the Svelte
`requestAnimationFrame` badge.

Current counters exposed through `game_client::cef_ui::CefUiFrameStats`:

- `cef_on_paint_fps`
- `cef_on_accelerated_paint_fps`
- `cef_cpu_upload_bytes`
- `cef_gpu_copy_bytes`
- `cef_gpu_copy_ns`
- `cef_gpu_copy_failures`
- `cef_transport_fallback_count`
- `cef_published_generation`
- `cef_sampled_generation`

`cef_gpu_copy_*` remains zero on the CPU path. It should only move once the
D3D11On12 copy into a FUN-owned D3D12 texture ring exists.

## Open Risks

- CEF's accelerated texture is D3D11-facing, while Bevy/wgpu DX12 owns D3D12
  resources.
- D3D11on12 is intended for interop and 2D composition, not heavy 3D work; use it
  only for the browser copy/resolve stage.
- Resource state ownership must be explicit. The D3D11on12 acquire/release calls
  must match the D3D12 states expected by the downstream FUN render path.
- Dirty rectangles should be preserved for future partial-copy optimization, but
  the first implementation should copy the full CEF frame for correctness.
