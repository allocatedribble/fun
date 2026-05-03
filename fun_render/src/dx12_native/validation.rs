use std::{
    error::Error,
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12NativeInteropFailure {
    WrongBackend,
    DeviceHalUnavailable,
    QueueHalUnavailable,
    TextureHalUnavailable,
    CommandEncoderHalUnavailable,
    CommandListUnavailable,
    InvalidTextureDimensions,
    UnsupportedTextureFormat,
    UnsupportedTextureShape,
}

impl Dx12NativeInteropFailure {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongBackend => "wrong_backend",
            Self::DeviceHalUnavailable => "device_hal_unavailable",
            Self::QueueHalUnavailable => "queue_hal_unavailable",
            Self::TextureHalUnavailable => "texture_hal_unavailable",
            Self::CommandEncoderHalUnavailable => "command_encoder_hal_unavailable",
            Self::CommandListUnavailable => "command_list_unavailable",
            Self::InvalidTextureDimensions => "invalid_texture_dimensions",
            Self::UnsupportedTextureFormat => "unsupported_texture_format",
            Self::UnsupportedTextureShape => "unsupported_texture_shape",
        }
    }

    const fn log_index(self) -> usize {
        match self {
            Self::WrongBackend => 0,
            Self::DeviceHalUnavailable => 1,
            Self::QueueHalUnavailable => 2,
            Self::TextureHalUnavailable => 3,
            Self::CommandEncoderHalUnavailable => 4,
            Self::CommandListUnavailable => 5,
            Self::InvalidTextureDimensions => 6,
            Self::UnsupportedTextureFormat => 7,
            Self::UnsupportedTextureShape => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12NativeInteropError {
    pub failure: Dx12NativeInteropFailure,
    pub detail: &'static str,
}

impl Dx12NativeInteropError {
    pub const fn new(failure: Dx12NativeInteropFailure, detail: &'static str) -> Self {
        Self { failure, detail }
    }
}

impl fmt::Display for Dx12NativeInteropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.failure.as_str(), self.detail)
    }
}

impl Error for Dx12NativeInteropError {}

static LOGGED_FAILURES: [AtomicBool; 9] = [
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
];

pub(crate) fn failure(
    failure: Dx12NativeInteropFailure,
    detail: &'static str,
) -> Dx12NativeInteropError {
    log_failure_once(failure, detail);
    Dx12NativeInteropError::new(failure, detail)
}

pub(crate) fn log_failure_once(failure: Dx12NativeInteropFailure, detail: &'static str) {
    if !LOGGED_FAILURES[failure.log_index()].swap(true, Ordering::Relaxed) {
        warn!(
            target: "fun::render::dx12_native",
            failure = failure.as_str(),
            detail,
            "FUN DX12 native interop disabled"
        );
    }
}

pub fn validate_dx12_backend(device: &wgpu::Device) -> Result<(), Dx12NativeInteropError> {
    let backend = device.adapter_info().backend;
    if backend == wgpu::Backend::Dx12 {
        Ok(())
    } else {
        Err(failure(
            Dx12NativeInteropFailure::WrongBackend,
            "active wgpu backend is not DirectX 12",
        ))
    }
}

pub fn validate_dx12_device_queue(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(), Dx12NativeInteropError> {
    super::handles::extract_dx12_native_handles(device, queue).map(|_| ())
}
