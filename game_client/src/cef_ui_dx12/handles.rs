use bevy::render::renderer::{RenderDevice, RenderQueue};
use windows::Win32::Graphics::Direct3D12::{ID3D12CommandQueue, ID3D12Device};

use super::bridge::{Dx12CefInteropError, Dx12CefInteropFailure};

#[derive(Debug, Clone)]
pub struct Dx12CefNativeHandles {
    pub d3d12_device: ID3D12Device,
    pub d3d12_queue: ID3D12CommandQueue,
}

/// Extracts cloned COM references for the active Bevy/wgpu DX12 device and
/// queue.
///
/// # Safety
///
/// This function crosses the wgpu HAL boundary. Callers must only use the
/// returned COM objects while the Bevy render device/queue are alive and must
/// not mutate wgpu-owned state outside the explicit D3D11On12 interop path.
pub unsafe fn extract_wgpu_dx12_handles(
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) -> Result<Dx12CefNativeHandles, Dx12CefInteropError> {
    let wgpu_device = render_device.wgpu_device();
    if wgpu_device.adapter_info().backend != wgpu::Backend::Dx12 {
        return Err(Dx12CefInteropError::new(
            Dx12CefInteropFailure::WrongBackend,
            "active wgpu backend is not DirectX 12",
            None,
        ));
    }

    let d3d12_device = unsafe {
        let Some(hal_device) = wgpu_device.as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(Dx12CefInteropError::new(
                Dx12CefInteropFailure::DeviceHalUnavailable,
                "wgpu device did not expose a DX12 HAL device",
                None,
            ));
        };
        hal_device.raw_device().clone()
    };

    let d3d12_queue = unsafe {
        let Some(hal_queue) = render_queue.as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(Dx12CefInteropError::new(
                Dx12CefInteropFailure::QueueHalUnavailable,
                "wgpu queue did not expose a DX12 HAL queue",
                None,
            ));
        };
        hal_queue.as_raw().clone()
    };

    Ok(Dx12CefNativeHandles {
        d3d12_device,
        d3d12_queue,
    })
}
