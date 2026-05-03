use std::ffi::c_void;

use windows_core::Interface as _;

use super::validation::{Dx12NativeInteropError, Dx12NativeInteropFailure, failure};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12NativeHandles {
    /// Borrowed `ID3D12Device` COM pointer owned by wgpu.
    pub device: *mut c_void,
    /// Borrowed `ID3D12CommandQueue` COM pointer owned by wgpu.
    pub queue: *mut c_void,
}

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

pub fn extract_dx12_native_handles(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<Dx12NativeHandles, Dx12NativeInteropError> {
    super::validation::validate_dx12_backend(device)?;

    // SAFETY: `as_hal` only borrows wgpu-owned backend objects. The returned COM
    // pointers are borrowed and must not be released by callers.
    let device = unsafe {
        let Some(hal_device) = device.as_hal::<wgpu::hal::api::Dx12>() else {
            return Err(failure(
                Dx12NativeInteropFailure::DeviceHalUnavailable,
                "wgpu device did not expose a DX12 HAL device",
            ));
        };
        hal_device.raw_device().as_raw()
    };

    // SAFETY: `as_hal` only borrows the wgpu-owned queue. The returned COM
    // pointer is borrowed and must not be released by callers.
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

    Ok(Dx12NativeHandles { device, queue })
}

/// Extracts a borrowed native texture handle for the native DLSS shim.
///
/// The returned pointer does not AddRef the underlying COM object. The source
/// `wgpu::Texture` must stay alive and must not alias the DLSS output resource
/// through the entire native evaluation call.
pub fn extract_dx12_texture_handle(
    texture: &wgpu::Texture,
) -> Result<Dx12TextureHandle, Dx12NativeInteropError> {
    let width = texture.width();
    let height = texture.height();
    if width == 0 || height == 0 {
        return Err(failure(
            Dx12NativeInteropFailure::InvalidTextureDimensions,
            "DLSS native textures must have non-zero dimensions",
        ));
    }
    if texture.dimension() != wgpu::TextureDimension::D2 || texture.depth_or_array_layers() != 1 {
        return Err(failure(
            Dx12NativeInteropFailure::UnsupportedTextureShape,
            "DLSS native textures must be single-layer 2D textures",
        ));
    }
    if texture.sample_count() != 1 {
        return Err(failure(
            Dx12NativeInteropFailure::UnsupportedTextureShape,
            "DLSS native textures must not be multisampled",
        ));
    }

    let Some(format) = DxgiFormatLike::from_wgpu(texture.format()) else {
        return Err(failure(
            Dx12NativeInteropFailure::UnsupportedTextureFormat,
            "texture format is not mapped for DX12 DLSS native interop",
        ));
    };

    // SAFETY: `as_hal` only borrows the wgpu-owned texture. The native resource
    // pointer is borrowed; callers must keep the source texture alive through the
    // native DLSS evaluation.
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

/// Runs `f` with a borrowed `ID3D12GraphicsCommandList` pointer when available.
///
/// This currently returns `None` after validating DX12 HAL encoder availability
/// because wgpu-hal 29 does not expose the underlying command list. It is kept as
/// the sole future call site so DLSS bring-up does not scatter HAL extraction.
pub fn with_dx12_command_list<R>(
    encoder: &mut wgpu::CommandEncoder,
    f: impl FnOnce(*mut c_void) -> R,
) -> Option<R> {
    with_dx12_command_list_checked(encoder, f).ok()
}

/// Checked variant of [`with_dx12_command_list`].
pub fn with_dx12_command_list_checked<R>(
    encoder: &mut wgpu::CommandEncoder,
    _f: impl FnOnce(*mut c_void) -> R,
) -> Result<R, Dx12NativeInteropError> {
    // SAFETY: This validates that wgpu can open the DX12 HAL command encoder.
    // wgpu-hal 29 does not publicly expose the inner ID3D12GraphicsCommandList,
    // so this function intentionally fails closed until the renderer owns a
    // sanctioned raw-command-list accessor.
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
