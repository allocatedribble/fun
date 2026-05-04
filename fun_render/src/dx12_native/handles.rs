use core::ffi::c_void;

use bevy::render::renderer::{RenderDevice, RenderQueue};
use windows_core::Interface as _;

use super::diagnostics::{
    Dx12NativeInteropError, Dx12NativeInteropFailure, failure, validate_render_device_dx12_backend,
    validate_wgpu_dx12_backend,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12DeviceQueueHandles {
    /// Borrowed `ID3D12Device` COM pointer owned by wgpu.
    pub device: *mut c_void,
    /// Borrowed `ID3D12CommandQueue` COM pointer owned by wgpu.
    pub queue: *mut c_void,
}

pub type Dx12NativeHandles = Dx12DeviceQueueHandles;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12TextureHandle {
    /// Borrowed `ID3D12Resource` COM pointer owned by the source wgpu texture.
    pub resource: *mut c_void,
    pub format: DxgiFormatLike,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxgiFormatLike {
    Rgba8Unorm,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgba16Float,
    R32Float,
    Rg16Float,
    Rg32Float,
    Depth32Float,
    Depth24Plus,
    Depth24PlusStencil8,
}

impl DxgiFormatLike {
    pub const fn from_wgpu(format: wgpu::TextureFormat) -> Option<Self> {
        match format {
            wgpu::TextureFormat::Rgba8Unorm => Some(Self::Rgba8Unorm),
            wgpu::TextureFormat::Rgba8UnormSrgb => Some(Self::Rgba8UnormSrgb),
            wgpu::TextureFormat::Bgra8Unorm => Some(Self::Bgra8Unorm),
            wgpu::TextureFormat::Bgra8UnormSrgb => Some(Self::Bgra8UnormSrgb),
            wgpu::TextureFormat::Rgba16Float => Some(Self::Rgba16Float),
            wgpu::TextureFormat::R32Float => Some(Self::R32Float),
            wgpu::TextureFormat::Rg16Float => Some(Self::Rg16Float),
            wgpu::TextureFormat::Rg32Float => Some(Self::Rg32Float),
            wgpu::TextureFormat::Depth32Float => Some(Self::Depth32Float),
            wgpu::TextureFormat::Depth24Plus => Some(Self::Depth24Plus),
            wgpu::TextureFormat::Depth24PlusStencil8 => Some(Self::Depth24PlusStencil8),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba8Unorm => "rgba8_unorm",
            Self::Rgba8UnormSrgb => "rgba8_unorm_srgb",
            Self::Bgra8Unorm => "bgra8_unorm",
            Self::Bgra8UnormSrgb => "bgra8_unorm_srgb",
            Self::Rgba16Float => "rgba16_float",
            Self::R32Float => "r32_float",
            Self::Rg16Float => "rg16_float",
            Self::Rg32Float => "rg32_float",
            Self::Depth32Float => "depth32_float",
            Self::Depth24Plus => "depth24_plus",
            Self::Depth24PlusStencil8 => "depth24_plus_stencil8",
        }
    }
}

#[must_use]
pub fn active_backend_is_dx12(render_device: &RenderDevice) -> bool {
    render_device.wgpu_device().adapter_info().backend == wgpu::Backend::Dx12
}

/// Runs `f` with borrowed native DX12 device/queue pointers from Bevy's active renderer.
///
/// # Safety
///
/// The returned pointers are borrowed from wgpu. The closure must not release
/// them, must not store them without taking its own COM reference, and must only
/// use them while the source `RenderDevice` and `RenderQueue` remain alive.
pub unsafe fn with_dx12_device_queue<R>(
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
    f: impl FnOnce(Dx12DeviceQueueHandles) -> R,
) -> Option<R> {
    unsafe { with_dx12_device_queue_checked(render_device, render_queue, f).ok() }
}

/// Checked variant of [`with_dx12_device_queue`].
///
/// # Safety
///
/// See [`with_dx12_device_queue`]. The closure receives borrowed COM pointers
/// owned by wgpu and must not release them.
pub unsafe fn with_dx12_device_queue_checked<R>(
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
    f: impl FnOnce(Dx12DeviceQueueHandles) -> R,
) -> Result<R, Dx12NativeInteropError> {
    validate_render_device_dx12_backend(render_device)?;

    let device = unsafe {
        let Some(hal_device) = render_device.wgpu_device().as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(failure(
                Dx12NativeInteropFailure::DeviceHalUnavailable,
                "wgpu device did not expose a DX12 HAL device",
            ));
        };
        hal_device.raw_device().as_raw()
    };

    let queue = unsafe {
        let Some(hal_queue) = render_queue.as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(failure(
                Dx12NativeInteropFailure::QueueHalUnavailable,
                "wgpu queue did not expose a DX12 HAL queue",
            ));
        };
        hal_queue.as_raw().as_raw()
    };

    if device.is_null() {
        return Err(failure(
            Dx12NativeInteropFailure::DeviceHalUnavailable,
            "DX12 HAL device returned a null ID3D12Device pointer",
        ));
    }
    if queue.is_null() {
        return Err(failure(
            Dx12NativeInteropFailure::QueueHalUnavailable,
            "DX12 HAL queue returned a null ID3D12CommandQueue pointer",
        ));
    }

    Ok(f(Dx12DeviceQueueHandles { device, queue }))
}

/// Extracts borrowed native DX12 device/queue pointers from raw wgpu objects.
///
/// # Safety
///
/// The returned pointers are borrowed from wgpu. Callers must not release them
/// and must keep the source `wgpu::Device` and `wgpu::Queue` alive for the
/// entire native call that consumes the handles.
pub unsafe fn extract_dx12_native_handles(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<Dx12NativeHandles, Dx12NativeInteropError> {
    validate_wgpu_dx12_backend(device)?;

    let device = unsafe {
        let Some(hal_device) = device.as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(failure(
                Dx12NativeInteropFailure::DeviceHalUnavailable,
                "wgpu device did not expose a DX12 HAL device",
            ));
        };
        hal_device.raw_device().as_raw()
    };

    let queue = unsafe {
        let Some(hal_queue) = queue.as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(failure(
                Dx12NativeInteropFailure::QueueHalUnavailable,
                "wgpu queue did not expose a DX12 HAL queue",
            ));
        };
        hal_queue.as_raw().as_raw()
    };

    if device.is_null() {
        return Err(failure(
            Dx12NativeInteropFailure::DeviceHalUnavailable,
            "DX12 HAL device returned a null ID3D12Device pointer",
        ));
    }
    if queue.is_null() {
        return Err(failure(
            Dx12NativeInteropFailure::QueueHalUnavailable,
            "DX12 HAL queue returned a null ID3D12CommandQueue pointer",
        ));
    }

    Ok(Dx12DeviceQueueHandles { device, queue })
}

/// Runs `f` with a borrowed native D3D12 texture resource pointer.
///
/// # Safety
///
/// The returned pointer is borrowed from wgpu. The closure must not release it,
/// must not use it after `texture` is dropped, and must respect wgpu's resource
/// state ownership unless a higher-level native interop path explicitly owns
/// the transition plan.
pub unsafe fn with_dx12_texture<R>(
    texture: &wgpu::Texture,
    f: impl FnOnce(Dx12TextureHandle) -> R,
) -> Option<R> {
    unsafe { with_dx12_texture_checked(texture, f).ok() }
}

/// Checked variant of [`with_dx12_texture`].
///
/// # Safety
///
/// See [`with_dx12_texture`]. The closure receives a borrowed
/// `ID3D12Resource` pointer owned by wgpu.
pub unsafe fn with_dx12_texture_checked<R>(
    texture: &wgpu::Texture,
    f: impl FnOnce(Dx12TextureHandle) -> R,
) -> Result<R, Dx12NativeInteropError> {
    let handle = unsafe { extract_dx12_texture_handle(texture)? };
    Ok(f(handle))
}

/// Extracts a borrowed native texture handle.
///
/// # Safety
///
/// The returned pointer does not AddRef the underlying COM object. The source
/// `wgpu::Texture` must stay alive through the entire native use, and callers
/// must not assume wgpu's resource-state tracker knows about external D3D12
/// transitions.
pub unsafe fn extract_dx12_texture_handle(
    texture: &wgpu::Texture,
) -> Result<Dx12TextureHandle, Dx12NativeInteropError> {
    let width = texture.width();
    let height = texture.height();
    if width == 0 || height == 0 {
        return Err(failure(
            Dx12NativeInteropFailure::InvalidTextureDimensions,
            "DX12 native textures must have non-zero dimensions",
        ));
    }
    if texture.dimension() != wgpu::TextureDimension::D2 || texture.depth_or_array_layers() != 1 {
        return Err(failure(
            Dx12NativeInteropFailure::UnsupportedTextureShape,
            "DX12 native textures must be single-layer 2D textures",
        ));
    }
    if texture.sample_count() != 1 {
        return Err(failure(
            Dx12NativeInteropFailure::UnsupportedTextureShape,
            "DX12 native textures must not be multisampled",
        ));
    }

    let Some(format) = DxgiFormatLike::from_wgpu(texture.format()) else {
        return Err(failure(
            Dx12NativeInteropFailure::UnsupportedTextureFormat,
            "texture format is not mapped for DX12 native interop",
        ));
    };

    let resource = unsafe {
        let Some(hal_texture) = texture.as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(failure(
                Dx12NativeInteropFailure::TextureHalUnavailable,
                "wgpu texture did not expose a DX12 HAL texture",
            ));
        };
        hal_texture.raw_resource().as_raw()
    };

    if resource.is_null() {
        return Err(failure(
            Dx12NativeInteropFailure::TextureHalUnavailable,
            "DX12 HAL texture returned a null ID3D12Resource pointer",
        ));
    }

    Ok(Dx12TextureHandle {
        resource,
        format,
        width,
        height,
    })
}
