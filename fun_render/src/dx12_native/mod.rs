//! Common DirectX 12 native interop boundary for FUN renderer integrations.
//!
//! This module is the only approved `fun_render` location that may touch wgpu
//! HAL extraction for DX12. CEF accelerated paint, native DLSS, PIX naming, and
//! future debug tooling must call this boundary instead of calling
//! `as_hal::<Dx12>()` or `as_hal_mut::<Dx12>()` directly.

mod command_encoder;
mod diagnostics;
mod handles;
mod labels;
mod states;

pub use command_encoder::{
    Dx12CommandListHandle, with_dx12_command_list, with_dx12_command_list_checked,
};
pub use diagnostics::{
    Dx12NativeInteropError, Dx12NativeInteropFailure, validate_dx12_backend,
    validate_dx12_device_queue, validate_render_device_dx12_backend,
};
pub use handles::{
    Dx12DeviceQueueHandles, Dx12NativeHandles, Dx12TextureHandle, DxgiFormatLike,
    active_backend_is_dx12, extract_dx12_native_handles, extract_dx12_texture_handle,
    with_dx12_device_queue, with_dx12_device_queue_checked, with_dx12_texture,
    with_dx12_texture_checked,
};
pub use labels::{
    Dx12NativeObjectKind, Dx12ObjectLabel, Dx12ObjectNameOutcome, dx12_object_naming_enabled,
    set_dx12_object_name,
};
pub use states::{
    Dx12DlssResourceStatePlan, Dx12DlssResourceStateRules, Dx12NativeResourceState,
    log_dx12_dlss_resource_state_plan_once,
};
