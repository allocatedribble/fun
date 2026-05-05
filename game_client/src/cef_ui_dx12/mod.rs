//! Windows-only CEF accelerated paint interop for `game_client`.
//!
//! This module adapts CEF accelerated paint callbacks to the renderer-owned
//! DX12 transport policy. `fun_render::dx12_native` remains the only approved
//! HAL extraction boundary; `fun_ui_cef` still owns browser lifetime and
//! callbacks.

pub mod bridge;
pub mod diagnostics;
pub mod handles;
pub mod ring;

pub use bridge::{
    Dx12CefBevyImageState, Dx12CefInterop, Dx12CefInteropError, Dx12CefInteropFailure,
    Dx12CefReadyFrameToken, MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK,
};
pub use diagnostics::{Dx12CefInteropDiagnosticSnapshot, Dx12CefInteropDiagnostics};
pub use handles::{Dx12CefNativeHandles, extract_wgpu_dx12_handles};
pub use ring::{
    CEF_GPU_RING_DEPTH_DEFAULT, CEF_GPU_RING_DEPTH_MAX, CEF_GPU_RING_DEPTH_MIN, Dx12CefSlotState,
    Dx12CefTextureRing, Dx12CefTextureSlot, DxgiFormat,
};
