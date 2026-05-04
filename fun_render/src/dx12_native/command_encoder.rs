use core::ffi::c_void;

use super::diagnostics::{Dx12NativeInteropError, Dx12NativeInteropFailure, failure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12CommandListHandle {
    /// Borrowed `ID3D12GraphicsCommandList` pointer owned by the active wgpu encoder.
    pub command_list: *mut c_void,
}

/// Runs `f` with a borrowed `ID3D12GraphicsCommandList` pointer when available.
///
/// # Safety
///
/// The command list is owned by wgpu, must be used only during the callback,
/// must not be closed or reset by callers, and external commands must preserve
/// the resource-state contract expected by wgpu's render graph.
pub unsafe fn with_dx12_command_list<R>(
    encoder: &mut wgpu::CommandEncoder,
    f: impl FnOnce(Dx12CommandListHandle) -> R,
) -> Option<R> {
    unsafe { with_dx12_command_list_checked(encoder, f).ok() }
}

/// Checked variant of [`with_dx12_command_list`].
///
/// # Safety
///
/// See [`with_dx12_command_list`]. wgpu 29 currently does not expose the inner
/// command-list pointer, so this fails closed after proving the DX12 HAL encoder
/// path is present.
pub unsafe fn with_dx12_command_list_checked<R>(
    encoder: &mut wgpu::CommandEncoder,
    _f: impl FnOnce(Dx12CommandListHandle) -> R,
) -> Result<R, Dx12NativeInteropError> {
    unsafe {
        encoder.as_hal_mut::<wgpu::hal::api::Dx12, _, _>(|encoder| {
            let Some(_encoder) = encoder else {
                return Err(failure(
                    Dx12NativeInteropFailure::CommandEncoderHalUnavailable,
                    "wgpu command encoder did not expose a DX12 HAL command encoder",
                ));
            };

            Err(failure(
                Dx12NativeInteropFailure::CommandListUnavailable,
                "wgpu-hal DX12 command encoder does not expose ID3D12GraphicsCommandList",
            ))
        })
    }
}
