use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use bevy::render::renderer::{RenderDevice, RenderQueue};
use fun_ui_cef::{
    CefAcceleratedPaintFrame, CefAcceleratedPaintOutcome, CefUiFallbackReason,
    CefUiPaintTransportFallbackReason,
};
use windows::{
    Win32::Graphics::{
        Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG, ID3D11Device,
            ID3D11DeviceContext,
        },
        Direct3D11on12::{D3D11On12CreateDevice, ID3D11On12Device},
        Direct3D12::{D3D12_FENCE_FLAG_NONE, ID3D12CommandQueue, ID3D12Device, ID3D12Fence},
    },
    core::{IUnknown, Interface},
};

use super::{
    diagnostics::{Dx12CefInteropDiagnosticSnapshot, Dx12CefInteropDiagnostics},
    handles::{Dx12CefNativeHandles, extract_wgpu_dx12_handles},
    ring::Dx12CefTextureRing,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12CefInteropFailure {
    WrongBackend,
    DeviceHalUnavailable,
    QueueHalUnavailable,
    D3d11On12CreateDeviceFailed,
    D3d11DeviceMissing,
    D3d11ImmediateContextMissing,
    D3d11On12QueryFailed,
    FenceCreateFailed,
    SharedTextureHandleMissing,
    OutputTextureRingUnavailable,
}

impl Dx12CefInteropFailure {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongBackend => "wrong_backend",
            Self::DeviceHalUnavailable => "device_hal_unavailable",
            Self::QueueHalUnavailable => "queue_hal_unavailable",
            Self::D3d11On12CreateDeviceFailed => "d3d11on12_create_device_failed",
            Self::D3d11DeviceMissing => "d3d11_device_missing",
            Self::D3d11ImmediateContextMissing => "d3d11_immediate_context_missing",
            Self::D3d11On12QueryFailed => "d3d11on12_query_failed",
            Self::FenceCreateFailed => "fence_create_failed",
            Self::SharedTextureHandleMissing => "shared_texture_handle_missing",
            Self::OutputTextureRingUnavailable => "output_texture_ring_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12CefInteropError {
    pub failure: Dx12CefInteropFailure,
    pub detail: &'static str,
    pub hresult: Option<i32>,
}

impl Dx12CefInteropError {
    #[must_use]
    pub const fn new(
        failure: Dx12CefInteropFailure,
        detail: &'static str,
        hresult: Option<i32>,
    ) -> Self {
        Self {
            failure,
            detail,
            hresult,
        }
    }

    #[must_use]
    pub fn from_windows(
        failure: Dx12CefInteropFailure,
        detail: &'static str,
        error: windows::core::Error,
    ) -> Self {
        Self::new(failure, detail, Some(error.code().0))
    }
}

impl fmt::Display for Dx12CefInteropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.hresult {
            Some(hresult) => write!(
                f,
                "{}: {} hresult={hresult:#010x}",
                self.failure.as_str(),
                self.detail
            ),
            None => write!(f, "{}: {}", self.failure.as_str(), self.detail),
        }
    }
}

impl Error for Dx12CefInteropError {}

pub struct Dx12CefInterop {
    d3d12_device: ID3D12Device,
    d3d12_queue: ID3D12CommandQueue,
    d3d11_device: ID3D11Device,
    d3d11_context: ID3D11DeviceContext,
    d3d11_on12: ID3D11On12Device,
    fence: ID3D12Fence,
    next_fence_value: AtomicU64,
    slots: Mutex<Dx12CefTextureRing>,
    diagnostics: Dx12CefInteropDiagnostics,
}

impl fmt::Debug for Dx12CefInterop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dx12CefInterop")
            .field("native_pointer_summary", &self.native_pointer_summary())
            .field("next_fence_value", &self.next_fence_value())
            .field("ring_len", &self.ring_len())
            .field("diagnostics", &self.diagnostics_snapshot())
            .finish()
    }
}

impl Dx12CefInterop {
    /// Creates the D3D11On12 bridge from Bevy/wgpu's active DX12 renderer.
    ///
    /// # Safety
    ///
    /// This function extracts native DX12 handles from wgpu. The returned
    /// bridge must not outlive the Bevy render device/queue that provided those
    /// handles, and all future GPU copies through it must keep resource states
    /// synchronized with the owning wgpu renderer.
    pub unsafe fn try_init_from_wgpu(
        render_device: &RenderDevice,
        render_queue: &RenderQueue,
    ) -> Result<Arc<Self>, Dx12CefInteropError> {
        let handles = unsafe { extract_wgpu_dx12_handles(render_device, render_queue)? };
        Self::create(handles).map(Arc::new)
    }

    fn create(handles: Dx12CefNativeHandles) -> Result<Self, Dx12CefInteropError> {
        let mut d3d11_device = None;
        let mut d3d11_context = None;
        let queue_unknown = handles.d3d12_queue.cast::<IUnknown>().map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::QueueHalUnavailable,
                "failed to cast ID3D12CommandQueue to IUnknown",
                error,
            )
        })?;
        let command_queues = [Some(queue_unknown)];
        let mut flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT.0;
        if cef_dx12_debug_layer_requested() {
            flags |= D3D11_CREATE_DEVICE_DEBUG.0;
        }

        unsafe {
            D3D11On12CreateDevice(
                &handles.d3d12_device,
                flags,
                None,
                Some(&command_queues),
                0,
                Some(&mut d3d11_device),
                Some(&mut d3d11_context),
                None,
            )
            .map_err(|error| {
                Dx12CefInteropError::from_windows(
                    Dx12CefInteropFailure::D3d11On12CreateDeviceFailed,
                    "D3D11On12CreateDevice failed for the active Bevy DX12 queue",
                    error,
                )
            })?;
        }

        let d3d11_device = d3d11_device.ok_or_else(|| {
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::D3d11DeviceMissing,
                "D3D11On12CreateDevice returned no ID3D11Device",
                None,
            )
        })?;
        let d3d11_context = d3d11_context.ok_or_else(|| {
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::D3d11ImmediateContextMissing,
                "D3D11On12CreateDevice returned no immediate context",
                None,
            )
        })?;
        let d3d11_on12 = d3d11_device.cast::<ID3D11On12Device>().map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::D3d11On12QueryFailed,
                "ID3D11Device did not expose ID3D11On12Device",
                error,
            )
        })?;
        let fence = unsafe {
            handles
                .d3d12_device
                .CreateFence::<ID3D12Fence>(0, D3D12_FENCE_FLAG_NONE)
        }
        .map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::FenceCreateFailed,
                "failed to create D3D12 fence for CEF interop ring",
                error,
            )
        })?;
        let interop = Self {
            d3d12_device: handles.d3d12_device,
            d3d12_queue: handles.d3d12_queue,
            d3d11_device,
            d3d11_context,
            d3d11_on12,
            fence,
            next_fence_value: AtomicU64::new(1),
            slots: Mutex::new(Dx12CefTextureRing::empty()),
            diagnostics: Dx12CefInteropDiagnostics::default(),
        };
        interop.diagnostics.record_init_success();
        Ok(interop)
    }

    #[must_use]
    pub fn diagnostics_snapshot(&self) -> Dx12CefInteropDiagnosticSnapshot {
        self.diagnostics.snapshot()
    }

    #[must_use]
    pub fn next_fence_value(&self) -> u64 {
        self.next_fence_value.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn ring_len(&self) -> usize {
        self.slots
            .lock()
            .map(|slots| slots.len())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn native_pointer_summary(&self) -> Dx12CefNativePointerSummary {
        Dx12CefNativePointerSummary {
            d3d12_device: self.d3d12_device.as_raw() as usize,
            d3d12_queue: self.d3d12_queue.as_raw() as usize,
            d3d11_device: self.d3d11_device.as_raw() as usize,
            d3d11_context: self.d3d11_context.as_raw() as usize,
            d3d11_on12: self.d3d11_on12.as_raw() as usize,
            fence: self.fence.as_raw() as usize,
        }
    }

    pub fn ingest_accelerated_paint(
        &self,
        frame: CefAcceleratedPaintFrame<'_>,
    ) -> CefAcceleratedPaintOutcome {
        if frame.shared_handle.is_null() {
            self.diagnostics.record_shared_texture_open_failure();
            self.diagnostics.record_fallback();
            return CefAcceleratedPaintOutcome::FallbackRequested {
                reason: CefUiPaintTransportFallbackReason::SharedTextureUnsupported,
            };
        }
        self.diagnostics.record_shared_texture_open();
        self.diagnostics.record_gpu_copy_failure();
        self.diagnostics.record_fallback();
        CefAcceleratedPaintOutcome::FallbackRequested {
            reason: self.current_copy_unavailable_reason(),
        }
    }

    #[must_use]
    pub const fn current_copy_unavailable_reason(&self) -> CefUiFallbackReason {
        CefUiPaintTransportFallbackReason::OutputTextureAllocationUnavailable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12CefNativePointerSummary {
    pub d3d12_device: usize,
    pub d3d12_queue: usize,
    pub d3d11_device: usize,
    pub d3d11_context: usize,
    pub d3d11_on12: usize,
    pub fence: usize,
}

fn cef_dx12_debug_layer_requested() -> bool {
    std::env::var_os("FUN_CEF_UI_ACCELERATED_PAINT_DEBUG").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dx12_cef_interop_failure_labels_are_stable() {
        assert_eq!(
            Dx12CefInteropFailure::D3d11On12CreateDeviceFailed.as_str(),
            "d3d11on12_create_device_failed"
        );
        assert_eq!(
            Dx12CefInteropFailure::OutputTextureRingUnavailable.as_str(),
            "output_texture_ring_unavailable"
        );
    }

    #[test]
    fn dx12_cef_interop_error_formats_without_allocating_context_state() {
        let error = Dx12CefInteropError::new(
            Dx12CefInteropFailure::WrongBackend,
            "active wgpu backend is not DirectX 12",
            Some(-1),
        );

        assert_eq!(
            error.to_string(),
            "wrong_backend: active wgpu backend is not DirectX 12 hresult=0xffffffff"
        );
    }
}
