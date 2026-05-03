use std::{
    cmp,
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

use bevy::render::renderer::{RenderDevice, RenderQueue};
use fun_ui_cef::{
    CefAcceleratedPaintFrame, CefAcceleratedPaintOutcome, CefUiFallbackReason,
    CefUiPaintTransportFallbackReason, render_handler::CefUiFrameGeneration,
};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Graphics::{
            Direct3D11::{
                D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CREATE_DEVICE_DEBUG, D3D11_TEXTURE2D_DESC,
                ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
            },
            Direct3D11on12::{D3D11_RESOURCE_FLAGS, D3D11On12CreateDevice, ID3D11On12Device},
            Direct3D12::{
                D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_FENCE_FLAG_NONE, D3D12_HEAP_FLAG_NONE,
                D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT, D3D12_MEMORY_POOL_UNKNOWN,
                D3D12_RESOURCE_DESC, D3D12_RESOURCE_DIMENSION_TEXTURE2D,
                D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET, D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE, D3D12_TEXTURE_LAYOUT_UNKNOWN,
                ID3D12CommandQueue, ID3D12Device, ID3D12Fence, ID3D12Resource,
            },
            Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
        },
    },
    core::{IUnknown, Interface},
};

use super::{
    diagnostics::{Dx12CefInteropDiagnosticSnapshot, Dx12CefInteropDiagnostics},
    handles::{Dx12CefNativeHandles, extract_wgpu_dx12_handles},
    ring::{
        Dx12CefRingSlotRequest, Dx12CefSlotState, Dx12CefTextureRing, Dx12CefTextureSlot,
        DxgiFormat,
    },
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
    SharedTextureOpenFailed,
    SharedTextureUnsupportedFormat,
    InvalidFrameDimensions,
    DestinationTextureCreateFailed,
    DestinationTextureWrapFailed,
    FenceSignalFailed,
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
            Self::SharedTextureOpenFailed => "shared_texture_open_failed",
            Self::SharedTextureUnsupportedFormat => "shared_texture_unsupported_format",
            Self::InvalidFrameDimensions => "invalid_frame_dimensions",
            Self::DestinationTextureCreateFailed => "destination_texture_create_failed",
            Self::DestinationTextureWrapFailed => "destination_texture_wrap_failed",
            Self::FenceSignalFailed => "fence_signal_failed",
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

const CEF_GPU_TEXTURE_BYTES_PER_PIXEL: u64 = 4;

const CEF_GPU_FORMAT_POLICY: Dx12CefGpuFormatPolicy = Dx12CefGpuFormatPolicy {
    source: DXGI_FORMAT_B8G8R8A8_UNORM,
    target: DXGI_FORMAT_B8G8R8A8_UNORM,
    conversion: Dx12CefColorConversion::None,
    alpha: Dx12CefAlphaMode::Premultiplied,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12CefGpuFormatPolicy {
    pub source: DxgiFormat,
    pub target: DxgiFormat,
    pub conversion: Dx12CefColorConversion,
    pub alpha: Dx12CefAlphaMode,
}

impl Dx12CefGpuFormatPolicy {
    #[must_use]
    pub const fn source_label(self) -> &'static str {
        dxgi_format_label(self.source)
    }

    #[must_use]
    pub const fn target_label(self) -> &'static str {
        dxgi_format_label(self.target)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12CefColorConversion {
    None,
}

impl Dx12CefColorConversion {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12CefAlphaMode {
    Premultiplied,
}

impl Dx12CefAlphaMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Premultiplied => "premultiplied",
        }
    }
}

pub struct Dx12CefInterop {
    d3d12_device: ID3D12Device,
    d3d12_queue: ID3D12CommandQueue,
    d3d11_device: ID3D11Device,
    d3d11_context: ID3D11DeviceContext,
    d3d11_on12: ID3D11On12Device,
    fence: ID3D12Fence,
    next_fence_value: AtomicU64,
    next_frame_generation: AtomicU64,
    slots: Mutex<Dx12CefTextureRing>,
    diagnostics: Dx12CefInteropDiagnostics,
}

impl fmt::Debug for Dx12CefInterop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dx12CefInterop")
            .field("native_pointer_summary", &self.native_pointer_summary())
            .field("next_fence_value", &self.next_fence_value())
            .field("next_frame_generation", &self.next_frame_generation())
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
            next_frame_generation: AtomicU64::new(1),
            slots: Mutex::new(Dx12CefTextureRing::empty()),
            diagnostics: Dx12CefInteropDiagnostics::default(),
        };
        interop.diagnostics.record_init_success();
        interop.log_gpu_format_policy();
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
    pub fn next_frame_generation(&self) -> u64 {
        self.next_frame_generation.load(Ordering::Relaxed)
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
        match self.copy_accelerated_paint(frame) {
            Ok(result) => CefAcceleratedPaintOutcome::Accepted {
                generation: result.generation,
                copied_bytes: result.copied_bytes,
                copy_ns: result.copy_ns,
            },
            Err(error) => {
                self.diagnostics.record_gpu_copy_failure();
                self.diagnostics.record_fallback();
                tracing::debug!(
                    target: "fun::ui",
                    failure = error.failure.as_str(),
                    detail = error.detail,
                    hresult = error.hresult,
                    fallback_reason = Self::fallback_reason_for_copy_error(error).as_wire_str(),
                    "CEF UI accelerated paint GPU copy failed"
                );
                CefAcceleratedPaintOutcome::FallbackRequested {
                    reason: Self::fallback_reason_for_copy_error(error),
                }
            }
        }
    }

    #[must_use]
    pub const fn current_copy_unavailable_reason(&self) -> CefUiFallbackReason {
        CefUiPaintTransportFallbackReason::OutputTextureAllocationUnavailable
    }

    fn copy_accelerated_paint(
        &self,
        frame: CefAcceleratedPaintFrame<'_>,
    ) -> Result<Dx12CefCopyResult, Dx12CefInteropError> {
        let (width, height) = validated_frame_extent(frame.width, frame.height)?;
        let copied_bytes = frame_byte_count(width, height)?;
        let source = self.open_shared_texture(frame.shared_handle)?;
        let source_desc = d3d11_texture_desc(&source);
        validate_source_texture_desc(source_desc, width, height)?;
        let source_resource = source.cast::<ID3D11Resource>().map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::SharedTextureOpenFailed,
                "failed to cast CEF shared texture to ID3D11Resource",
                error,
            )
        })?;

        let mut slots = self.slots.lock().map_err(|_| {
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::OutputTextureRingUnavailable,
                "CEF DX12 texture ring lock is poisoned",
                None,
            )
        })?;
        let slot_index =
            match slots.next_copy_slot_request(width, height, CEF_GPU_FORMAT_POLICY.target) {
                Dx12CefRingSlotRequest::Reuse { index } => index,
                Dx12CefRingSlotRequest::Allocate { index } => {
                    let slot =
                        self.create_destination_slot(width, height, CEF_GPU_FORMAT_POLICY.target)?;
                    slots.install_slot(index, slot);
                    index
                }
                Dx12CefRingSlotRequest::Unavailable => {
                    return Err(Dx12CefInteropError::new(
                        Dx12CefInteropFailure::OutputTextureRingUnavailable,
                        "all CEF DX12 texture ring slots are busy",
                        None,
                    ));
                }
            };
        let slot = slots.slot_mut(slot_index).ok_or_else(|| {
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::OutputTextureRingUnavailable,
                "CEF DX12 texture ring returned no selected slot",
                None,
            )
        })?;
        let previous_state = slot.state;
        slot.state = Dx12CefSlotState::Copying;
        let fence_value = self.next_fence_value.fetch_add(1, Ordering::Relaxed);
        let copy_start = Instant::now();
        if let Err(error) = self.copy_source_into_slot(&source_resource, slot, fence_value) {
            slot.state = previous_state;
            return Err(error);
        }
        let copy_ns = nanos_u64(copy_start.elapsed().as_nanos());
        let generation =
            CefUiFrameGeneration(self.next_frame_generation.fetch_add(1, Ordering::Relaxed));
        slot.generation = generation;
        slot.fence_value = fence_value;
        slot.state = Dx12CefSlotState::Ready;
        self.diagnostics.record_gpu_copy(copied_bytes, copy_ns);
        self.diagnostics
            .record_published_generation(generation.0, fence_value);

        Ok(Dx12CefCopyResult {
            generation,
            copied_bytes,
            copy_ns,
        })
    }

    fn open_shared_texture(
        &self,
        shared_handle: *mut std::ffi::c_void,
    ) -> Result<ID3D11Texture2D, Dx12CefInteropError> {
        if shared_handle.is_null() {
            self.diagnostics.record_shared_texture_open_failure();
            return Err(Dx12CefInteropError::new(
                Dx12CefInteropFailure::SharedTextureHandleMissing,
                "CEF accelerated paint did not provide a shared texture handle",
                None,
            ));
        }

        let mut texture = None;
        unsafe {
            self.d3d11_device
                .OpenSharedResource::<ID3D11Texture2D>(HANDLE(shared_handle), &mut texture)
        }
        .map_err(|error| {
            self.diagnostics.record_shared_texture_open_failure();
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::SharedTextureOpenFailed,
                "ID3D11Device::OpenSharedResource failed for CEF accelerated paint texture",
                error,
            )
        })?;
        let texture = texture.ok_or_else(|| {
            self.diagnostics.record_shared_texture_open_failure();
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::SharedTextureOpenFailed,
                "ID3D11Device::OpenSharedResource returned no texture",
                None,
            )
        })?;
        self.diagnostics.record_shared_texture_open();
        Ok(texture)
    }

    fn create_destination_slot(
        &self,
        width: u32,
        height: u32,
        format: DxgiFormat,
    ) -> Result<Dx12CefTextureSlot, Dx12CefInteropError> {
        let heap_properties = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 0,
            VisibleNodeMask: 0,
        };
        let resource_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: u64::from(width),
            Height: height,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
        };
        let mut d3d12_resource = None;
        unsafe {
            self.d3d12_device.CreateCommittedResource::<ID3D12Resource>(
                &heap_properties,
                D3D12_HEAP_FLAG_NONE,
                &resource_desc,
                D3D12_RESOURCE_STATE_COPY_DEST,
                None,
                &mut d3d12_resource,
            )
        }
        .map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::DestinationTextureCreateFailed,
                "failed to create FUN-owned D3D12 CEF destination texture",
                error,
            )
        })?;
        let d3d12_resource = d3d12_resource.ok_or_else(|| {
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::DestinationTextureCreateFailed,
                "ID3D12Device::CreateCommittedResource returned no texture",
                None,
            )
        })?;
        let wrapped_d3d11_resource = self.wrap_destination_texture(&d3d12_resource)?;

        Ok(Dx12CefTextureSlot {
            generation: CefUiFrameGeneration(0),
            width,
            height,
            format,
            d3d12_resource,
            wrapped_d3d11_resource,
            fence_value: 0,
            state: Dx12CefSlotState::Free,
        })
    }

    fn wrap_destination_texture(
        &self,
        d3d12_resource: &ID3D12Resource,
    ) -> Result<ID3D11Resource, Dx12CefInteropError> {
        let flags = D3D11_RESOURCE_FLAGS {
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32 | D3D11_BIND_SHADER_RESOURCE.0 as u32,
            MiscFlags: 0,
            CPUAccessFlags: 0,
            StructureByteStride: 0,
        };
        let resource_unknown = d3d12_resource.cast::<IUnknown>().map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::DestinationTextureWrapFailed,
                "failed to cast destination ID3D12Resource to IUnknown",
                error,
            )
        })?;
        let mut wrapped_resource = None;
        unsafe {
            self.d3d11_on12.CreateWrappedResource::<_, ID3D11Resource>(
                &resource_unknown,
                &flags,
                D3D12_RESOURCE_STATE_COPY_DEST,
                D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE,
                &mut wrapped_resource,
            )
        }
        .map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::DestinationTextureWrapFailed,
                "ID3D11On12Device::CreateWrappedResource failed for CEF destination texture",
                error,
            )
        })?;
        wrapped_resource.ok_or_else(|| {
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::DestinationTextureWrapFailed,
                "ID3D11On12Device::CreateWrappedResource returned no resource",
                None,
            )
        })
    }

    fn copy_source_into_slot(
        &self,
        source_resource: &ID3D11Resource,
        slot: &Dx12CefTextureSlot,
        fence_value: u64,
    ) -> Result<(), Dx12CefInteropError> {
        let wrapped_resources = [Some(slot.wrapped_d3d11_resource.clone())];
        unsafe {
            self.d3d11_on12.AcquireWrappedResources(&wrapped_resources);
            self.d3d11_context
                .CopyResource(&slot.wrapped_d3d11_resource, source_resource);
            self.d3d11_on12.ReleaseWrappedResources(&wrapped_resources);
            self.d3d11_context.Flush();
            self.d3d12_queue.Signal(&self.fence, fence_value)
        }
        .map_err(|error| {
            Dx12CefInteropError::from_windows(
                Dx12CefInteropFailure::FenceSignalFailed,
                "failed to signal D3D12 fence after CEF GPU copy",
                error,
            )
        })
    }

    #[must_use]
    const fn fallback_reason_for_copy_error(
        error: Dx12CefInteropError,
    ) -> CefUiPaintTransportFallbackReason {
        match error.failure {
            Dx12CefInteropFailure::SharedTextureHandleMissing
            | Dx12CefInteropFailure::SharedTextureOpenFailed
            | Dx12CefInteropFailure::SharedTextureUnsupportedFormat
            | Dx12CefInteropFailure::InvalidFrameDimensions => {
                CefUiPaintTransportFallbackReason::SharedTextureUnsupported
            }
            Dx12CefInteropFailure::DestinationTextureCreateFailed
            | Dx12CefInteropFailure::DestinationTextureWrapFailed
            | Dx12CefInteropFailure::FenceSignalFailed
            | Dx12CefInteropFailure::OutputTextureRingUnavailable => {
                CefUiPaintTransportFallbackReason::OutputTextureAllocationUnavailable
            }
            Dx12CefInteropFailure::WrongBackend => {
                CefUiPaintTransportFallbackReason::RenderBackendNotDx12
            }
            Dx12CefInteropFailure::DeviceHalUnavailable
            | Dx12CefInteropFailure::QueueHalUnavailable => {
                CefUiPaintTransportFallbackReason::DeviceQueueExtractionFailed
            }
            Dx12CefInteropFailure::D3d11On12CreateDeviceFailed
            | Dx12CefInteropFailure::D3d11DeviceMissing
            | Dx12CefInteropFailure::D3d11ImmediateContextMissing
            | Dx12CefInteropFailure::D3d11On12QueryFailed
            | Dx12CefInteropFailure::FenceCreateFailed => {
                CefUiPaintTransportFallbackReason::D3d11On12BridgeUnavailable
            }
        }
    }

    fn log_gpu_format_policy(&self) {
        tracing::info!(
            target: "fun::ui",
            source = CEF_GPU_FORMAT_POLICY.source_label(),
            target = CEF_GPU_FORMAT_POLICY.target_label(),
            conversion = CEF_GPU_FORMAT_POLICY.conversion.as_str(),
            alpha = CEF_GPU_FORMAT_POLICY.alpha.as_str(),
            "CEF UI GPU format"
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Dx12CefCopyResult {
    generation: CefUiFrameGeneration,
    copied_bytes: u64,
    copy_ns: u64,
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

fn validated_frame_extent(width: i32, height: i32) -> Result<(u32, u32), Dx12CefInteropError> {
    if width <= 0 || height <= 0 {
        return Err(Dx12CefInteropError::new(
            Dx12CefInteropFailure::InvalidFrameDimensions,
            "CEF accelerated paint frame dimensions must be positive",
            None,
        ));
    }
    Ok((width as u32, height as u32))
}

fn frame_byte_count(width: u32, height: u32) -> Result<u64, Dx12CefInteropError> {
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(CEF_GPU_TEXTURE_BYTES_PER_PIXEL))
        .ok_or_else(|| {
            Dx12CefInteropError::new(
                Dx12CefInteropFailure::InvalidFrameDimensions,
                "CEF accelerated paint frame byte count overflowed",
                None,
            )
        })
}

fn d3d11_texture_desc(texture: &ID3D11Texture2D) -> D3D11_TEXTURE2D_DESC {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe {
        texture.GetDesc(&mut desc);
    }
    desc
}

fn validate_source_texture_desc(
    desc: D3D11_TEXTURE2D_DESC,
    width: u32,
    height: u32,
) -> Result<(), Dx12CefInteropError> {
    if desc.Width != width || desc.Height != height {
        return Err(Dx12CefInteropError::new(
            Dx12CefInteropFailure::InvalidFrameDimensions,
            "CEF shared texture dimensions did not match accelerated paint metadata",
            None,
        ));
    }
    if desc.Format != CEF_GPU_FORMAT_POLICY.source {
        return Err(Dx12CefInteropError::new(
            Dx12CefInteropFailure::SharedTextureUnsupportedFormat,
            "CEF shared texture format is not the first-pass BGRA8 format",
            None,
        ));
    }
    Ok(())
}

fn nanos_u64(nanos: u128) -> u64 {
    cmp::min(nanos, u128::from(u64::MAX)) as u64
}

const fn dxgi_format_label(format: DxgiFormat) -> &'static str {
    match format.0 {
        87 => "BGRA8",
        _ => "unsupported",
    }
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
        assert_eq!(
            Dx12CefInteropFailure::SharedTextureUnsupportedFormat.as_str(),
            "shared_texture_unsupported_format"
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

    #[test]
    fn dx12_cef_gpu_format_policy_is_bgra_premultiplied_without_conversion() {
        assert_eq!(CEF_GPU_FORMAT_POLICY.source_label(), "BGRA8");
        assert_eq!(CEF_GPU_FORMAT_POLICY.target_label(), "BGRA8");
        assert_eq!(CEF_GPU_FORMAT_POLICY.conversion.as_str(), "none");
        assert_eq!(CEF_GPU_FORMAT_POLICY.alpha.as_str(), "premultiplied");
    }
}
