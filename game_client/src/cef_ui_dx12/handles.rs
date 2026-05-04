use bevy::render::renderer::{RenderDevice, RenderQueue};
use fun_render::dx12_native::{
    Dx12NativeInteropError, Dx12NativeInteropFailure, with_dx12_device_queue_checked,
    with_dx12_texture_checked,
};
use windows::{
    Win32::Graphics::Direct3D12::{ID3D12CommandQueue, ID3D12Device, ID3D12Resource},
    core::Interface as _,
};

use super::bridge::{Dx12CefInteropError, Dx12CefInteropFailure};

#[derive(Debug, Clone)]
pub struct Dx12CefNativeHandles {
    pub d3d12_device: ID3D12Device,
    pub d3d12_queue: ID3D12CommandQueue,
}

/// Extracts cloned COM references for the active Bevy/wgpu DX12 device and queue.
///
/// # Safety
///
/// This function crosses the shared `fun_render::dx12_native` boundary. Callers
/// must only use the returned COM objects while the Bevy render device/queue
/// are alive and must not mutate wgpu-owned state outside the explicit
/// D3D11On12 interop path.
pub unsafe fn extract_wgpu_dx12_handles(
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) -> Result<Dx12CefNativeHandles, Dx12CefInteropError> {
    let handles =
        unsafe { with_dx12_device_queue_checked(render_device, render_queue, |handles| handles) }
            .map_err(map_native_error_for_device_queue)?;

    let d3d12_device = unsafe {
        ID3D12Device::from_raw_borrowed(&handles.device)
            .cloned()
            .ok_or_else(|| {
                Dx12CefInteropError::new(
                    Dx12CefInteropFailure::DeviceHalUnavailable,
                    "DX12 native boundary returned no ID3D12Device pointer",
                    None,
                )
            })?
    };
    let d3d12_queue = unsafe {
        ID3D12CommandQueue::from_raw_borrowed(&handles.queue)
            .cloned()
            .ok_or_else(|| {
                Dx12CefInteropError::new(
                    Dx12CefInteropFailure::QueueHalUnavailable,
                    "DX12 native boundary returned no ID3D12CommandQueue pointer",
                    None,
                )
            })?
    };

    Ok(Dx12CefNativeHandles {
        d3d12_device,
        d3d12_queue,
    })
}

pub fn clone_dx12_resource_from_wgpu_texture(
    texture: &wgpu::Texture,
) -> Result<ID3D12Resource, Dx12CefInteropError> {
    let handle = unsafe { with_dx12_texture_checked(texture, |handle| handle) }
        .map_err(map_native_error_for_texture)?;
    unsafe {
        ID3D12Resource::from_raw_borrowed(&handle.resource)
            .cloned()
            .ok_or_else(|| {
                Dx12CefInteropError::new(
                    Dx12CefInteropFailure::BevyTargetTextureHalUnavailable,
                    "DX12 native boundary returned no ID3D12Resource pointer",
                    None,
                )
            })
    }
}

fn map_native_error_for_device_queue(error: Dx12NativeInteropError) -> Dx12CefInteropError {
    let failure = match error.failure {
        Dx12NativeInteropFailure::WrongBackend => Dx12CefInteropFailure::WrongBackend,
        Dx12NativeInteropFailure::DeviceHalUnavailable => {
            Dx12CefInteropFailure::DeviceHalUnavailable
        }
        Dx12NativeInteropFailure::QueueHalUnavailable => Dx12CefInteropFailure::QueueHalUnavailable,
        Dx12NativeInteropFailure::TextureHalUnavailable
        | Dx12NativeInteropFailure::CommandEncoderHalUnavailable
        | Dx12NativeInteropFailure::CommandListUnavailable
        | Dx12NativeInteropFailure::InvalidTextureDimensions
        | Dx12NativeInteropFailure::UnsupportedTextureFormat
        | Dx12NativeInteropFailure::UnsupportedTextureShape
        | Dx12NativeInteropFailure::ObjectNameUnavailable
        | Dx12NativeInteropFailure::ObjectNameFailed => {
            Dx12CefInteropFailure::D3d11On12CreateDeviceFailed
        }
    };
    Dx12CefInteropError::new(failure, error.detail, None)
}

fn map_native_error_for_texture(error: Dx12NativeInteropError) -> Dx12CefInteropError {
    let failure = match error.failure {
        Dx12NativeInteropFailure::WrongBackend => Dx12CefInteropFailure::WrongBackend,
        Dx12NativeInteropFailure::TextureHalUnavailable => {
            Dx12CefInteropFailure::BevyTargetTextureHalUnavailable
        }
        Dx12NativeInteropFailure::InvalidTextureDimensions
        | Dx12NativeInteropFailure::UnsupportedTextureFormat
        | Dx12NativeInteropFailure::UnsupportedTextureShape => {
            Dx12CefInteropFailure::BevyTargetTextureUnsupported
        }
        Dx12NativeInteropFailure::DeviceHalUnavailable
        | Dx12NativeInteropFailure::QueueHalUnavailable
        | Dx12NativeInteropFailure::CommandEncoderHalUnavailable
        | Dx12NativeInteropFailure::CommandListUnavailable
        | Dx12NativeInteropFailure::ObjectNameUnavailable
        | Dx12NativeInteropFailure::ObjectNameFailed => {
            Dx12CefInteropFailure::BevyTextureCopyFailed
        }
    };
    Dx12CefInteropError::new(failure, error.detail, None)
}
