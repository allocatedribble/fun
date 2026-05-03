//! Windows-only CEF accelerated paint interop for `game_client`.
//!
//! This module is the single first-pass trapdoor for raw wgpu DX12 handle
//! extraction and D3D11On12 bridge creation. `fun_ui_cef` still owns browser
//! lifetime and callbacks; this module only owns the native transport boundary
//! used by the game presentation host.

pub mod bridge;
pub mod diagnostics;
pub mod handles;
pub mod ring;

pub use bridge::{Dx12CefInterop, Dx12CefInteropError, Dx12CefInteropFailure};
pub use diagnostics::{Dx12CefInteropDiagnosticSnapshot, Dx12CefInteropDiagnostics};
pub use handles::{Dx12CefNativeHandles, extract_wgpu_dx12_handles};
pub use ring::{
    CEF_GPU_RING_LEN, Dx12CefSlotState, Dx12CefTextureRing, Dx12CefTextureSlot, DxgiFormat,
};
