//! Narrow DirectX 12 native interop boundary for experimental DLSS work.
//!
//! This module is the only `fun_render` location that may touch wgpu HAL
//! extraction for DX12. Callers get opaque raw pointers for the native shim,
//! while all backend checks, failure categories, and resource-state notes stay
//! centralized here.

mod barriers;
mod handles;
mod validation;

pub use barriers::{
    Dx12DlssResourceStatePlan, Dx12DlssResourceStateRules, Dx12NativeResourceState,
    log_dx12_dlss_resource_state_plan_once,
};
pub use handles::{
    Dx12NativeHandles, Dx12TextureHandle, DxgiFormatLike, extract_dx12_native_handles,
    extract_dx12_texture_handle, with_dx12_command_list, with_dx12_command_list_checked,
};
pub use validation::{
    Dx12NativeInteropError, Dx12NativeInteropFailure, validate_dx12_backend,
    validate_dx12_device_queue,
};
