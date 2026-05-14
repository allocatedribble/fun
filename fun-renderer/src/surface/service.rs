use core::fmt;

use fun_window::{
    DpiScale, SurfaceError, SurfaceId, WindowError, WindowHandles, WindowId, WindowSize,
    surface::wgpu::WgpuSurfaceAdapter,
};

use crate::{
    backend::NativeBackend,
    bridge::wgpu::{
        device::native_backend,
        surface::{WgpuNativeSurfaceDesc, WgpuSurfaceClearDesc, native_surface_configuration},
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RendererSurfaceHandle {
    surface: SurfaceId,
}

impl RendererSurfaceHandle {
    pub const fn new(surface: SurfaceId) -> Self {
        Self { surface }
    }

    #[must_use]
    pub const fn surface_id(self) -> SurfaceId {
        self.surface
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.surface.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfaceBackendPreference {
    Primary,
    Vulkan,
    Metal,
    Dx12,
    BrowserWebGpu,
}

impl RendererSurfaceBackendPreference {
    fn wgpu_backends(self) -> ::wgpu::Backends {
        match self {
            Self::Primary => ::wgpu::Backends::PRIMARY,
            Self::Vulkan => ::wgpu::Backends::VULKAN,
            Self::Metal => ::wgpu::Backends::METAL,
            Self::Dx12 => ::wgpu::Backends::DX12,
            Self::BrowserWebGpu => ::wgpu::Backends::BROWSER_WEBGPU,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfacePowerPreference {
    None,
    LowPower,
    HighPerformance,
}

impl RendererSurfacePowerPreference {
    const fn wgpu_power_preference(self) -> ::wgpu::PowerPreference {
        match self {
            Self::None => ::wgpu::PowerPreference::None,
            Self::LowPower => ::wgpu::PowerPreference::LowPower,
            Self::HighPerformance => ::wgpu::PowerPreference::HighPerformance,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfacePresentModePolicy {
    PreferFifo,
    PreferMailbox,
    PreferImmediate,
    FirstSupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendererSurfaceDiagnosticLabels {
    pub frame: &'static str,
    pub adapter: &'static str,
    pub device: &'static str,
    pub surface_configuration: &'static str,
    pub clear: &'static str,
}

impl Default for RendererSurfaceDiagnosticLabels {
    fn default() -> Self {
        Self {
            frame: "fun_renderer.windowed_surface.frame",
            adapter: "fun_renderer.windowed_surface.adapter",
            device: "fun_renderer.windowed_surface.device",
            surface_configuration: "fun_renderer.windowed_surface.configure",
            clear: "fun_renderer.windowed_surface.clear",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RendererSurfaceConfig {
    backend_preference: RendererSurfaceBackendPreference,
    power_preference: RendererSurfacePowerPreference,
    present_mode_policy: RendererSurfacePresentModePolicy,
    force_fallback_adapter: bool,
    required_features: ::wgpu::Features,
    required_limits: ::wgpu::Limits,
    memory_hints: ::wgpu::MemoryHints,
    desired_maximum_frame_latency: u32,
    clear_color: [f64; 4],
    labels: RendererSurfaceDiagnosticLabels,
}

impl RendererSurfaceConfig {
    #[must_use]
    pub fn product_default() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn backend_preference(&self) -> RendererSurfaceBackendPreference {
        self.backend_preference
    }

    #[must_use]
    pub const fn power_preference(&self) -> RendererSurfacePowerPreference {
        self.power_preference
    }

    #[must_use]
    pub const fn present_mode_policy(&self) -> RendererSurfacePresentModePolicy {
        self.present_mode_policy
    }

    #[must_use]
    pub const fn desired_maximum_frame_latency(&self) -> u32 {
        self.desired_maximum_frame_latency
    }

    #[must_use]
    pub const fn clear_color(&self) -> [f64; 4] {
        self.clear_color
    }

    #[must_use]
    pub const fn labels(&self) -> RendererSurfaceDiagnosticLabels {
        self.labels
    }

    #[must_use]
    pub const fn prefer_primary_backend(mut self) -> Self {
        self.backend_preference = RendererSurfaceBackendPreference::Primary;
        self
    }

    #[must_use]
    pub const fn prefer_vulkan_backend(mut self) -> Self {
        self.backend_preference = RendererSurfaceBackendPreference::Vulkan;
        self
    }

    #[must_use]
    pub const fn prefer_metal_backend(mut self) -> Self {
        self.backend_preference = RendererSurfaceBackendPreference::Metal;
        self
    }

    #[must_use]
    pub const fn prefer_dx12_backend(mut self) -> Self {
        self.backend_preference = RendererSurfaceBackendPreference::Dx12;
        self
    }

    #[must_use]
    pub const fn prefer_browser_webgpu_backend(mut self) -> Self {
        self.backend_preference = RendererSurfaceBackendPreference::BrowserWebGpu;
        self
    }

    #[must_use]
    pub const fn ignore_power_preference(mut self) -> Self {
        self.power_preference = RendererSurfacePowerPreference::None;
        self
    }

    #[must_use]
    pub const fn prefer_low_power_adapter(mut self) -> Self {
        self.power_preference = RendererSurfacePowerPreference::LowPower;
        self
    }

    #[must_use]
    pub const fn prefer_high_performance_adapter(mut self) -> Self {
        self.power_preference = RendererSurfacePowerPreference::HighPerformance;
        self
    }

    #[must_use]
    pub const fn force_fallback_adapter(mut self, enabled: bool) -> Self {
        self.force_fallback_adapter = enabled;
        self
    }

    #[must_use]
    pub const fn prefer_fifo_present_mode(mut self) -> Self {
        self.present_mode_policy = RendererSurfacePresentModePolicy::PreferFifo;
        self
    }

    #[must_use]
    pub const fn prefer_mailbox_present_mode(mut self) -> Self {
        self.present_mode_policy = RendererSurfacePresentModePolicy::PreferMailbox;
        self
    }

    #[must_use]
    pub const fn prefer_immediate_present_mode(mut self) -> Self {
        self.present_mode_policy = RendererSurfacePresentModePolicy::PreferImmediate;
        self
    }

    #[must_use]
    pub const fn use_first_supported_present_mode(mut self) -> Self {
        self.present_mode_policy = RendererSurfacePresentModePolicy::FirstSupported;
        self
    }

    #[must_use]
    pub const fn with_desired_maximum_frame_latency(mut self, latency: u32) -> Self {
        self.desired_maximum_frame_latency = if latency == 0 { 1 } else { latency };
        self
    }

    #[must_use]
    pub const fn with_clear_color(mut self, clear_color: [f64; 4]) -> Self {
        self.clear_color = clear_color;
        self
    }

    #[must_use]
    pub const fn with_frame_label(mut self, label: &'static str) -> Self {
        self.labels.frame = label;
        self
    }

    #[must_use]
    pub const fn with_adapter_label(mut self, label: &'static str) -> Self {
        self.labels.adapter = label;
        self
    }

    #[must_use]
    pub const fn with_device_label(mut self, label: &'static str) -> Self {
        self.labels.device = label;
        self
    }

    #[must_use]
    pub const fn with_surface_configuration_label(mut self, label: &'static str) -> Self {
        self.labels.surface_configuration = label;
        self
    }

    #[must_use]
    pub const fn with_clear_label(mut self, label: &'static str) -> Self {
        self.labels.clear = label;
        self
    }

    pub(crate) fn clear_desc(&self) -> WgpuSurfaceClearDesc {
        WgpuSurfaceClearDesc {
            stable_name: self.labels.clear,
            color: self.clear_color,
        }
    }
}

impl Default for RendererSurfaceConfig {
    fn default() -> Self {
        Self {
            backend_preference: RendererSurfaceBackendPreference::Primary,
            power_preference: RendererSurfacePowerPreference::HighPerformance,
            present_mode_policy: RendererSurfacePresentModePolicy::PreferFifo,
            force_fallback_adapter: false,
            required_features: ::wgpu::Features::empty(),
            required_limits: ::wgpu::Limits::default(),
            memory_hints: ::wgpu::MemoryHints::Performance,
            desired_maximum_frame_latency: 2,
            clear_color: [0.02, 0.04, 0.06, 1.0],
            labels: RendererSurfaceDiagnosticLabels::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfaceFormat {
    Unknown,
    Bgra8UnormSrgb,
    Rgba8UnormSrgb,
    Bgra8Unorm,
    Rgba8Unorm,
    Rgba16Float,
    Rgb10a2Unorm,
    Other,
}

impl RendererSurfaceFormat {
    const fn from_wgpu(format: ::wgpu::TextureFormat) -> Self {
        match format {
            ::wgpu::TextureFormat::Bgra8UnormSrgb => Self::Bgra8UnormSrgb,
            ::wgpu::TextureFormat::Rgba8UnormSrgb => Self::Rgba8UnormSrgb,
            ::wgpu::TextureFormat::Bgra8Unorm => Self::Bgra8Unorm,
            ::wgpu::TextureFormat::Rgba8Unorm => Self::Rgba8Unorm,
            ::wgpu::TextureFormat::Rgba16Float => Self::Rgba16Float,
            ::wgpu::TextureFormat::Rgb10a2Unorm => Self::Rgb10a2Unorm,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfacePresentMode {
    Unknown,
    AutoVsync,
    AutoNoVsync,
    Fifo,
    FifoRelaxed,
    Mailbox,
    Immediate,
    Other,
}

impl RendererSurfacePresentMode {
    const fn from_wgpu(mode: ::wgpu::PresentMode) -> Self {
        match mode {
            ::wgpu::PresentMode::AutoVsync => Self::AutoVsync,
            ::wgpu::PresentMode::AutoNoVsync => Self::AutoNoVsync,
            ::wgpu::PresentMode::Fifo => Self::Fifo,
            ::wgpu::PresentMode::FifoRelaxed => Self::FifoRelaxed,
            ::wgpu::PresentMode::Mailbox => Self::Mailbox,
            ::wgpu::PresentMode::Immediate => Self::Immediate,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfaceAlphaMode {
    Unknown,
    Auto,
    Opaque,
    PreMultiplied,
    PostMultiplied,
    Inherit,
}

impl RendererSurfaceAlphaMode {
    const fn from_wgpu(mode: ::wgpu::CompositeAlphaMode) -> Self {
        match mode {
            ::wgpu::CompositeAlphaMode::Auto => Self::Auto,
            ::wgpu::CompositeAlphaMode::Opaque => Self::Opaque,
            ::wgpu::CompositeAlphaMode::PreMultiplied => Self::PreMultiplied,
            ::wgpu::CompositeAlphaMode::PostMultiplied => Self::PostMultiplied,
            ::wgpu::CompositeAlphaMode::Inherit => Self::Inherit,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfaceReconfigureState {
    Clean,
    Dirty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSurfaceLifecycleState {
    Created,
    Configured,
    FrameAcquired,
    FrameRecorded,
    FrameSubmitted,
    Presented,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererPresentStatus {
    AwaitingSurface,
    Presented,
    PresentedSuboptimal,
    Timeout,
    Occluded,
    Outdated,
    Lost,
    ValidationFailed,
    NotRendered,
    Failed,
}

impl RendererPresentStatus {
    #[must_use]
    pub const fn presented(self) -> bool {
        matches!(self, Self::Presented | Self::PresentedSuboptimal)
    }

    #[must_use]
    pub const fn failed(self) -> bool {
        matches!(
            self,
            Self::Timeout
                | Self::Occluded
                | Self::Outdated
                | Self::Lost
                | Self::ValidationFailed
                | Self::NotRendered
                | Self::Failed
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererPresentError {
    WindowHost,
    SurfaceAdapter,
    SurfaceIdExhausted,
    SurfaceNotFound,
    SurfaceRequiresReconfigure,
    InvalidSurfaceState,
    FrameSurfaceMismatch,
    AdapterUnavailable,
    DeviceUnavailable,
    FrameNotRendered,
    NoPresentModes,
    NoSurfaceFormats,
    SurfaceTimeout,
    SurfaceOccluded,
    SurfaceOutdated,
    SurfaceLost,
    SurfaceValidation,
}

impl RendererPresentError {
    #[must_use]
    pub const fn status(self) -> RendererPresentStatus {
        match self {
            Self::SurfaceTimeout => RendererPresentStatus::Timeout,
            Self::SurfaceOccluded => RendererPresentStatus::Occluded,
            Self::SurfaceOutdated => RendererPresentStatus::Outdated,
            Self::SurfaceLost => RendererPresentStatus::Lost,
            Self::SurfaceValidation => RendererPresentStatus::ValidationFailed,
            Self::FrameNotRendered => RendererPresentStatus::NotRendered,
            Self::WindowHost
            | Self::SurfaceAdapter
            | Self::SurfaceIdExhausted
            | Self::SurfaceNotFound
            | Self::SurfaceRequiresReconfigure
            | Self::InvalidSurfaceState
            | Self::FrameSurfaceMismatch
            | Self::AdapterUnavailable
            | Self::DeviceUnavailable
            | Self::NoPresentModes
            | Self::NoSurfaceFormats => RendererPresentStatus::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendererSurfaceFrameReport {
    window: WindowId,
    surface: SurfaceId,
    width: u32,
    height: u32,
    scale_microunits: u32,
    present_status: RendererPresentStatus,
}

impl RendererSurfaceFrameReport {
    #[must_use]
    pub fn new(
        window: WindowId,
        surface: SurfaceId,
        physical_size: WindowSize,
        dpi_scale: DpiScale,
        present_status: RendererPresentStatus,
    ) -> Self {
        Self {
            window,
            surface,
            width: physical_size.width(),
            height: physical_size.height(),
            scale_microunits: dpi_scale_microunits(dpi_scale),
            present_status,
        }
    }

    #[must_use]
    pub fn from_status(status: RendererSurfaceStatus) -> Self {
        Self::new(
            status.window(),
            status.surface_id(),
            status.physical_size(),
            status.dpi_scale(),
            status
                .last_present_status()
                .unwrap_or(RendererPresentStatus::AwaitingSurface),
        )
    }

    #[must_use]
    pub fn from_surface_request(
        request: &fun_window::SurfaceRequest,
        present_status: RendererPresentStatus,
    ) -> Self {
        let size = WindowSize::new(request.size().width(), request.size().height())
            .unwrap_or(WindowSize::DEFAULT);
        Self::new(
            request.window(),
            request.surface(),
            size,
            DpiScale::ONE,
            present_status,
        )
    }

    #[must_use]
    pub const fn window(self) -> WindowId {
        self.window
    }

    #[must_use]
    pub const fn surface(self) -> SurfaceId {
        self.surface
    }

    #[must_use]
    pub const fn surface_id(self) -> SurfaceId {
        self.surface
    }

    #[must_use]
    pub const fn width(self) -> u32 {
        self.width
    }

    #[must_use]
    pub const fn height(self) -> u32 {
        self.height
    }

    #[must_use]
    pub const fn scale_microunits(self) -> u32 {
        self.scale_microunits
    }

    #[must_use]
    pub const fn present_status(self) -> RendererPresentStatus {
        self.present_status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendererFrameOutcome {
    surface_frame: Option<RendererSurfaceFrameReport>,
    present_status: RendererPresentStatus,
    present_error: Option<RendererPresentError>,
}

impl RendererFrameOutcome {
    #[must_use]
    pub const fn presented(surface_frame: RendererSurfaceFrameReport) -> Self {
        Self {
            surface_frame: Some(surface_frame),
            present_status: surface_frame.present_status(),
            present_error: None,
        }
    }

    #[must_use]
    pub const fn failed(present_error: RendererPresentError) -> Self {
        Self {
            surface_frame: None,
            present_status: present_error.status(),
            present_error: Some(present_error),
        }
    }

    #[must_use]
    pub const fn with_surface_frame(mut self, surface_frame: RendererSurfaceFrameReport) -> Self {
        self.surface_frame = Some(surface_frame);
        self
    }

    #[must_use]
    pub const fn surface_frame(self) -> Option<RendererSurfaceFrameReport> {
        self.surface_frame
    }

    #[must_use]
    pub const fn present_status(self) -> RendererPresentStatus {
        self.present_status
    }

    #[must_use]
    pub const fn present_error(self) -> Option<RendererPresentError> {
        self.present_error
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RendererSurfaceStatus {
    handle: RendererSurfaceHandle,
    window: WindowId,
    physical_size: WindowSize,
    dpi_scale: DpiScale,
    selected_backend: NativeBackend,
    selected_format: RendererSurfaceFormat,
    present_mode: RendererSurfacePresentMode,
    alpha_mode: RendererSurfaceAlphaMode,
    reconfigure_state: RendererSurfaceReconfigureState,
    lifecycle_state: RendererSurfaceLifecycleState,
    last_present_status: Option<RendererPresentStatus>,
}

impl RendererSurfaceStatus {
    fn created(
        handle: RendererSurfaceHandle,
        window: WindowId,
        physical_size: WindowSize,
        dpi_scale: DpiScale,
        selected_backend: NativeBackend,
    ) -> Self {
        Self {
            handle,
            window,
            physical_size,
            dpi_scale,
            selected_backend,
            selected_format: RendererSurfaceFormat::Unknown,
            present_mode: RendererSurfacePresentMode::Unknown,
            alpha_mode: RendererSurfaceAlphaMode::Unknown,
            reconfigure_state: RendererSurfaceReconfigureState::Dirty,
            lifecycle_state: RendererSurfaceLifecycleState::Created,
            last_present_status: None,
        }
    }

    #[must_use]
    pub const fn handle(self) -> RendererSurfaceHandle {
        self.handle
    }

    #[must_use]
    pub const fn surface_id(self) -> SurfaceId {
        self.handle.surface_id()
    }

    #[must_use]
    pub const fn window(self) -> WindowId {
        self.window
    }

    #[must_use]
    pub const fn physical_size(self) -> WindowSize {
        self.physical_size
    }

    #[must_use]
    pub const fn dpi_scale(self) -> DpiScale {
        self.dpi_scale
    }

    #[must_use]
    pub const fn selected_backend(self) -> NativeBackend {
        self.selected_backend
    }

    #[must_use]
    pub const fn selected_format(self) -> RendererSurfaceFormat {
        self.selected_format
    }

    #[must_use]
    pub const fn present_mode(self) -> RendererSurfacePresentMode {
        self.present_mode
    }

    #[must_use]
    pub const fn alpha_mode(self) -> RendererSurfaceAlphaMode {
        self.alpha_mode
    }

    #[must_use]
    pub const fn dirty(self) -> bool {
        matches!(
            self.reconfigure_state,
            RendererSurfaceReconfigureState::Dirty
        )
    }

    #[must_use]
    pub const fn reconfigure_state(self) -> RendererSurfaceReconfigureState {
        self.reconfigure_state
    }

    #[must_use]
    pub const fn lifecycle_state(self) -> RendererSurfaceLifecycleState {
        self.lifecycle_state
    }

    #[must_use]
    pub const fn last_present_status(self) -> Option<RendererPresentStatus> {
        self.last_present_status
    }
}

pub struct RendererSurfaceFrame {
    handle: RendererSurfaceHandle,
    surface_texture: ::wgpu::SurfaceTexture,
    suboptimal: bool,
}

impl RendererSurfaceFrame {
    #[must_use]
    pub const fn handle(&self) -> RendererSurfaceHandle {
        self.handle
    }

    #[must_use]
    pub const fn suboptimal(&self) -> bool {
        self.suboptimal
    }
}

pub struct RendererRecordedSurfaceFrame {
    handle: RendererSurfaceHandle,
    command_buffer: ::wgpu::CommandBuffer,
}

impl RendererRecordedSurfaceFrame {
    #[must_use]
    pub const fn handle(&self) -> RendererSurfaceHandle {
        self.handle
    }
}

pub struct RendererSurfaceStore<'window> {
    next_surface_id: u64,
    records: Vec<RendererSurfaceRecord<'window>>,
}

impl<'window> RendererSurfaceStore<'window> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next_surface_id: 1,
            records: Vec::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn status(
        &self,
        handle: RendererSurfaceHandle,
    ) -> Result<RendererSurfaceStatus, RendererSurfaceError> {
        Ok(self.record(handle)?.status)
    }

    fn allocate_handle(&mut self) -> Result<RendererSurfaceHandle, RendererSurfaceError> {
        let surface_id =
            SurfaceId::new(self.next_surface_id).ok_or(RendererSurfaceError::SurfaceIdExhausted)?;
        self.next_surface_id = self
            .next_surface_id
            .checked_add(1)
            .ok_or(RendererSurfaceError::SurfaceIdExhausted)?;
        Ok(RendererSurfaceHandle::new(surface_id))
    }

    fn insert(&mut self, record: RendererSurfaceRecord<'window>) {
        self.records.push(record);
    }

    fn record(
        &self,
        handle: RendererSurfaceHandle,
    ) -> Result<&RendererSurfaceRecord<'window>, RendererSurfaceError> {
        self.records
            .iter()
            .find(|record| record.status.handle == handle)
            .ok_or(RendererSurfaceError::SurfaceNotFound { surface: handle })
    }

    fn record_mut(
        &mut self,
        handle: RendererSurfaceHandle,
    ) -> Result<&mut RendererSurfaceRecord<'window>, RendererSurfaceError> {
        self.records
            .iter_mut()
            .find(|record| record.status.handle == handle)
            .ok_or(RendererSurfaceError::SurfaceNotFound { surface: handle })
    }

    fn remove(
        &mut self,
        handle: RendererSurfaceHandle,
    ) -> Result<RendererSurfaceRecord<'window>, RendererSurfaceError> {
        let Some(index) = self
            .records
            .iter()
            .position(|record| record.status.handle == handle)
        else {
            return Err(RendererSurfaceError::SurfaceNotFound { surface: handle });
        };
        Ok(self.records.remove(index))
    }
}

impl<'window> Default for RendererSurfaceStore<'window> {
    fn default() -> Self {
        Self::new()
    }
}

struct RendererSurfaceRecord<'window> {
    surface: ::wgpu::Surface<'window>,
    adapter: ::wgpu::Adapter,
    device: ::wgpu::Device,
    queue: ::wgpu::Queue,
    config: RendererSurfaceConfig,
    status: RendererSurfaceStatus,
}

pub struct RendererSurfaceService<'window> {
    store: RendererSurfaceStore<'window>,
}

impl<'window> RendererSurfaceService<'window> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            store: RendererSurfaceStore::new(),
        }
    }

    #[must_use]
    pub const fn store(&self) -> &RendererSurfaceStore<'window> {
        &self.store
    }

    pub async fn create_surface_from_window_handles(
        &mut self,
        window: WindowId,
        handles: WindowHandles<'window>,
        physical_size: WindowSize,
        dpi_scale: DpiScale,
        config: RendererSurfaceConfig,
    ) -> Result<RendererSurfaceHandle, RendererSurfaceError> {
        let handle = self.store.allocate_handle()?;
        let instance = ::wgpu::Instance::new(::wgpu::InstanceDescriptor {
            backends: config.backend_preference.wgpu_backends(),
            flags: ::wgpu::InstanceFlags::from_build_config(),
            memory_budget_thresholds: ::wgpu::MemoryBudgetThresholds::default(),
            backend_options: ::wgpu::BackendOptions::default(),
            display: None,
        });
        let surface = WgpuSurfaceAdapter::create_surface(&instance, handles)?;
        let adapter = instance
            .request_adapter(&::wgpu::RequestAdapterOptions {
                power_preference: config.power_preference.wgpu_power_preference(),
                force_fallback_adapter: config.force_fallback_adapter,
                compatible_surface: Some(&surface),
            })
            .await
            .map_err(|_error| RendererSurfaceError::AdapterUnavailable {
                adapter_label: config.labels.adapter,
            })?;
        let adapter_info = adapter.get_info();
        let selected_backend = native_backend(adapter_info.backend);
        let (device, queue) = adapter
            .request_device(&::wgpu::DeviceDescriptor {
                label: Some(config.labels.device),
                required_features: config.required_features,
                required_limits: config.required_limits.clone(),
                experimental_features: ::wgpu::ExperimentalFeatures::disabled(),
                memory_hints: config.memory_hints.clone(),
                trace: ::wgpu::Trace::Off,
            })
            .await
            .map_err(|_error| RendererSurfaceError::DeviceUnavailable {
                device_label: config.labels.device,
            })?;
        let status = RendererSurfaceStatus::created(
            handle,
            window,
            physical_size,
            dpi_scale,
            selected_backend,
        );
        self.store.insert(RendererSurfaceRecord {
            surface,
            adapter,
            device,
            queue,
            config,
            status,
        });
        Ok(handle)
    }

    pub fn configure_surface(
        &mut self,
        handle: RendererSurfaceHandle,
    ) -> Result<RendererSurfaceStatus, RendererSurfaceError> {
        let record = self.store.record_mut(handle)?;
        let capabilities = record.surface.get_capabilities(&record.adapter);
        let format = select_surface_format(
            &capabilities.formats,
            record.config.labels.surface_configuration,
        )?;
        let present_mode = select_present_mode(
            &capabilities.present_modes,
            record.config.present_mode_policy,
            record.config.labels.surface_configuration,
        )?;
        let alpha_mode = capabilities
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(::wgpu::CompositeAlphaMode::Auto);
        let desc = WgpuNativeSurfaceDesc {
            stable_name: record.config.labels.surface_configuration,
            width: record.status.physical_size.width(),
            height: record.status.physical_size.height(),
            format,
            usage: ::wgpu::TextureUsages::RENDER_ATTACHMENT,
            present_mode,
            alpha_mode,
            desired_maximum_frame_latency: record.config.desired_maximum_frame_latency,
        };
        let configuration = native_surface_configuration(desc, vec![format]);
        record.surface.configure(&record.device, &configuration);
        record.status.selected_format = RendererSurfaceFormat::from_wgpu(format);
        record.status.present_mode = RendererSurfacePresentMode::from_wgpu(present_mode);
        record.status.alpha_mode = RendererSurfaceAlphaMode::from_wgpu(alpha_mode);
        record.status.reconfigure_state = RendererSurfaceReconfigureState::Clean;
        record.status.lifecycle_state = RendererSurfaceLifecycleState::Configured;
        Ok(record.status)
    }

    pub fn reconfigure_surface(
        &mut self,
        handle: RendererSurfaceHandle,
        physical_size: WindowSize,
        dpi_scale: DpiScale,
    ) -> Result<RendererSurfaceStatus, RendererSurfaceError> {
        {
            let record = self.store.record_mut(handle)?;
            record.status.physical_size = physical_size;
            record.status.dpi_scale = dpi_scale;
            record.status.reconfigure_state = RendererSurfaceReconfigureState::Dirty;
        }
        self.configure_surface(handle)
    }

    pub fn acquire_frame(
        &mut self,
        handle: RendererSurfaceHandle,
    ) -> Result<RendererSurfaceFrame, RendererSurfaceError> {
        let record = self.store.record_mut(handle)?;
        if record.status.dirty() {
            return Err(RendererSurfaceError::SurfaceRequiresReconfigure { surface: handle });
        }
        ensure_state(record.status, handle, "configured or presented", |state| {
            matches!(
                state,
                RendererSurfaceLifecycleState::Configured
                    | RendererSurfaceLifecycleState::Presented
            )
        })?;
        let (surface_texture, suboptimal) = match record.surface.get_current_texture() {
            ::wgpu::CurrentSurfaceTexture::Success(texture) => (texture, false),
            ::wgpu::CurrentSurfaceTexture::Suboptimal(texture) => (texture, true),
            ::wgpu::CurrentSurfaceTexture::Timeout => {
                record_surface_failure(record, RendererPresentStatus::Timeout);
                return Err(RendererSurfaceError::SurfaceTimeout {
                    surface: handle,
                    surface_label: record.config.labels.surface_configuration,
                });
            }
            ::wgpu::CurrentSurfaceTexture::Occluded => {
                record_surface_failure(record, RendererPresentStatus::Occluded);
                return Err(RendererSurfaceError::SurfaceOccluded {
                    surface: handle,
                    surface_label: record.config.labels.surface_configuration,
                });
            }
            ::wgpu::CurrentSurfaceTexture::Outdated => {
                record_surface_failure(record, RendererPresentStatus::Outdated);
                return Err(RendererSurfaceError::SurfaceOutdated {
                    surface: handle,
                    surface_label: record.config.labels.surface_configuration,
                });
            }
            ::wgpu::CurrentSurfaceTexture::Lost => {
                record_surface_failure(record, RendererPresentStatus::Lost);
                return Err(RendererSurfaceError::SurfaceLost {
                    surface: handle,
                    surface_label: record.config.labels.surface_configuration,
                });
            }
            ::wgpu::CurrentSurfaceTexture::Validation => {
                record_surface_failure(record, RendererPresentStatus::ValidationFailed);
                return Err(RendererSurfaceError::SurfaceValidation {
                    surface: handle,
                    surface_label: record.config.labels.surface_configuration,
                });
            }
        };
        if suboptimal {
            record.status.reconfigure_state = RendererSurfaceReconfigureState::Dirty;
        }
        record.status.lifecycle_state = RendererSurfaceLifecycleState::FrameAcquired;
        Ok(RendererSurfaceFrame {
            handle,
            surface_texture,
            suboptimal,
        })
    }

    pub fn record_clear_frame(
        &mut self,
        handle: RendererSurfaceHandle,
        frame: &RendererSurfaceFrame,
    ) -> Result<RendererRecordedSurfaceFrame, RendererSurfaceError> {
        if frame.handle != handle {
            return Err(RendererSurfaceError::FrameSurfaceMismatch {
                expected: handle,
                actual: frame.handle,
            });
        }
        let record = self.store.record_mut(handle)?;
        ensure_state(record.status, handle, "frame acquired", |state| {
            matches!(state, RendererSurfaceLifecycleState::FrameAcquired)
        })?;
        let clear = record.config.clear_desc();
        let surface_view = frame
            .surface_texture
            .texture
            .create_view(&::wgpu::TextureViewDescriptor::default());
        let mut encoder = record
            .device
            .create_command_encoder(&::wgpu::CommandEncoderDescriptor {
                label: Some(clear.stable_name),
            });
        {
            let _pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
                label: Some(clear.stable_name),
                color_attachments: &[Some(::wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: ::wgpu::Operations {
                        load: ::wgpu::LoadOp::Clear(::wgpu::Color {
                            r: clear.color[0],
                            g: clear.color[1],
                            b: clear.color[2],
                            a: clear.color[3],
                        }),
                        store: ::wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        record.status.lifecycle_state = RendererSurfaceLifecycleState::FrameRecorded;
        Ok(RendererRecordedSurfaceFrame {
            handle,
            command_buffer: encoder.finish(),
        })
    }

    pub fn submit_frame(
        &mut self,
        recorded: RendererRecordedSurfaceFrame,
    ) -> Result<(), RendererSurfaceError> {
        let record = self.store.record_mut(recorded.handle)?;
        ensure_state(record.status, recorded.handle, "frame recorded", |state| {
            matches!(state, RendererSurfaceLifecycleState::FrameRecorded)
        })?;
        record.queue.submit([recorded.command_buffer]);
        record.status.lifecycle_state = RendererSurfaceLifecycleState::FrameSubmitted;
        Ok(())
    }

    pub fn present_frame(
        &mut self,
        frame: RendererSurfaceFrame,
    ) -> Result<RendererSurfaceStatus, RendererSurfaceError> {
        let record = self.store.record_mut(frame.handle)?;
        ensure_state(record.status, frame.handle, "frame submitted", |state| {
            matches!(state, RendererSurfaceLifecycleState::FrameSubmitted)
        })?;
        frame.surface_texture.present();
        record.status.last_present_status = Some(if frame.suboptimal {
            RendererPresentStatus::PresentedSuboptimal
        } else {
            RendererPresentStatus::Presented
        });
        record.status.lifecycle_state = RendererSurfaceLifecycleState::Presented;
        Ok(record.status)
    }

    pub fn release_surface(
        &mut self,
        handle: RendererSurfaceHandle,
    ) -> Result<RendererSurfaceStatus, RendererSurfaceError> {
        let mut record = self.store.remove(handle)?;
        record.status.lifecycle_state = RendererSurfaceLifecycleState::Released;
        Ok(record.status)
    }

    pub fn status(
        &self,
        handle: RendererSurfaceHandle,
    ) -> Result<RendererSurfaceStatus, RendererSurfaceError> {
        self.store.status(handle)
    }
}

impl RendererSurfaceService<'static> {
    pub fn create_persistent_surface_from_window_handles(
        &mut self,
        window: WindowId,
        handles: WindowHandles<'_>,
        physical_size: WindowSize,
        dpi_scale: DpiScale,
        config: RendererSurfaceConfig,
    ) -> Result<RendererSurfaceHandle, RendererSurfaceError> {
        pollster::block_on(async {
            let handle = self.store.allocate_handle()?;
            let instance = ::wgpu::Instance::new(::wgpu::InstanceDescriptor {
                backends: config.backend_preference.wgpu_backends(),
                flags: ::wgpu::InstanceFlags::from_build_config(),
                memory_budget_thresholds: ::wgpu::MemoryBudgetThresholds::default(),
                backend_options: ::wgpu::BackendOptions::default(),
                display: None,
            });
            let surface = WgpuSurfaceAdapter::create_surface_for_host_lifetime(&instance, handles)?;
            let adapter = instance
                .request_adapter(&::wgpu::RequestAdapterOptions {
                    power_preference: config.power_preference.wgpu_power_preference(),
                    force_fallback_adapter: config.force_fallback_adapter,
                    compatible_surface: Some(&surface),
                })
                .await
                .map_err(|_error| RendererSurfaceError::AdapterUnavailable {
                    adapter_label: config.labels.adapter,
                })?;
            let adapter_info = adapter.get_info();
            let selected_backend = native_backend(adapter_info.backend);
            let (device, queue) = adapter
                .request_device(&::wgpu::DeviceDescriptor {
                    label: Some(config.labels.device),
                    required_features: config.required_features,
                    required_limits: config.required_limits.clone(),
                    experimental_features: ::wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: config.memory_hints.clone(),
                    trace: ::wgpu::Trace::Off,
                })
                .await
                .map_err(|_error| RendererSurfaceError::DeviceUnavailable {
                    device_label: config.labels.device,
                })?;
            let status = RendererSurfaceStatus::created(
                handle,
                window,
                physical_size,
                dpi_scale,
                selected_backend,
            );
            self.store.insert(RendererSurfaceRecord {
                surface,
                adapter,
                device,
                queue,
                config,
                status,
            });
            Ok(handle)
        })
    }

    pub fn record_submit_present_clear_frame(
        &mut self,
        handle: RendererSurfaceHandle,
    ) -> Result<RendererSurfaceFrameReport, RendererSurfaceError> {
        let surface_frame = self.acquire_frame(handle)?;
        let recorded_frame = self.record_clear_frame(handle, &surface_frame)?;
        self.submit_frame(recorded_frame)?;
        let status = self.present_frame(surface_frame)?;
        Ok(RendererSurfaceFrameReport::from_status(status))
    }
}

impl<'window> Default for RendererSurfaceService<'window> {
    fn default() -> Self {
        Self::new()
    }
}

fn ensure_state(
    status: RendererSurfaceStatus,
    surface: RendererSurfaceHandle,
    expected: &'static str,
    accepts: impl FnOnce(RendererSurfaceLifecycleState) -> bool,
) -> Result<(), RendererSurfaceError> {
    if accepts(status.lifecycle_state) {
        return Ok(());
    }
    Err(RendererSurfaceError::InvalidSurfaceState {
        surface,
        expected,
        actual: status.lifecycle_state,
    })
}

fn record_surface_failure(record: &mut RendererSurfaceRecord<'_>, result: RendererPresentStatus) {
    record.status.last_present_status = Some(result);
    match result {
        RendererPresentStatus::Outdated | RendererPresentStatus::Lost => {
            record.status.reconfigure_state = RendererSurfaceReconfigureState::Dirty;
        }
        RendererPresentStatus::AwaitingSurface
        | RendererPresentStatus::Timeout
        | RendererPresentStatus::Occluded
        | RendererPresentStatus::ValidationFailed
        | RendererPresentStatus::Presented
        | RendererPresentStatus::PresentedSuboptimal
        | RendererPresentStatus::NotRendered
        | RendererPresentStatus::Failed => {}
    }
}

fn dpi_scale_microunits(scale: DpiScale) -> u32 {
    let value = scale.get();
    if value.is_finite() && value > 0.0 {
        (value * 1_000_000.0).round() as u32
    } else {
        1_000_000
    }
}

pub(crate) fn select_surface_format(
    formats: &[::wgpu::TextureFormat],
    surface_label: &'static str,
) -> Result<::wgpu::TextureFormat, RendererSurfaceError> {
    formats
        .iter()
        .copied()
        .find(::wgpu::TextureFormat::is_srgb)
        .or_else(|| formats.first().copied())
        .ok_or(RendererSurfaceError::NoSurfaceFormats { surface_label })
}

pub(crate) fn select_present_mode(
    present_modes: &[::wgpu::PresentMode],
    policy: RendererSurfacePresentModePolicy,
    surface_label: &'static str,
) -> Result<::wgpu::PresentMode, RendererSurfaceError> {
    let preferred = match policy {
        RendererSurfacePresentModePolicy::PreferFifo => Some(::wgpu::PresentMode::Fifo),
        RendererSurfacePresentModePolicy::PreferMailbox => Some(::wgpu::PresentMode::Mailbox),
        RendererSurfacePresentModePolicy::PreferImmediate => Some(::wgpu::PresentMode::Immediate),
        RendererSurfacePresentModePolicy::FirstSupported => None,
    };
    preferred
        .filter(|mode| present_modes.contains(mode))
        .or_else(|| present_modes.first().copied())
        .ok_or(RendererSurfaceError::NoPresentModes { surface_label })
}

#[cfg(test)]
fn map_acquire_failure(
    error: crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure,
    surface: RendererSurfaceHandle,
    surface_label: &'static str,
) -> RendererSurfaceError {
    use crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure;

    match error {
        WgpuSurfaceAcquireFailure::Timeout => RendererSurfaceError::SurfaceTimeout {
            surface,
            surface_label,
        },
        WgpuSurfaceAcquireFailure::Occluded => RendererSurfaceError::SurfaceOccluded {
            surface,
            surface_label,
        },
        WgpuSurfaceAcquireFailure::Outdated => RendererSurfaceError::SurfaceOutdated {
            surface,
            surface_label,
        },
        WgpuSurfaceAcquireFailure::Lost => RendererSurfaceError::SurfaceLost {
            surface,
            surface_label,
        },
        WgpuSurfaceAcquireFailure::Validation => RendererSurfaceError::SurfaceValidation {
            surface,
            surface_label,
        },
    }
}

#[derive(Debug)]
pub enum RendererSurfaceError {
    Window(WindowError),
    SurfaceAdapter(SurfaceError),
    SurfaceIdExhausted,
    SurfaceNotFound {
        surface: RendererSurfaceHandle,
    },
    SurfaceRequiresReconfigure {
        surface: RendererSurfaceHandle,
    },
    InvalidSurfaceState {
        surface: RendererSurfaceHandle,
        expected: &'static str,
        actual: RendererSurfaceLifecycleState,
    },
    FrameSurfaceMismatch {
        expected: RendererSurfaceHandle,
        actual: RendererSurfaceHandle,
    },
    AdapterUnavailable {
        adapter_label: &'static str,
    },
    DeviceUnavailable {
        device_label: &'static str,
    },
    FrameNotRendered {
        frame_label: &'static str,
    },
    NoPresentModes {
        surface_label: &'static str,
    },
    NoSurfaceFormats {
        surface_label: &'static str,
    },
    SurfaceTimeout {
        surface: RendererSurfaceHandle,
        surface_label: &'static str,
    },
    SurfaceOccluded {
        surface: RendererSurfaceHandle,
        surface_label: &'static str,
    },
    SurfaceOutdated {
        surface: RendererSurfaceHandle,
        surface_label: &'static str,
    },
    SurfaceLost {
        surface: RendererSurfaceHandle,
        surface_label: &'static str,
    },
    SurfaceValidation {
        surface: RendererSurfaceHandle,
        surface_label: &'static str,
    },
}

impl RendererSurfaceError {
    #[must_use]
    pub const fn present_error(&self) -> RendererPresentError {
        match self {
            Self::Window(_) => RendererPresentError::WindowHost,
            Self::SurfaceAdapter(_) => RendererPresentError::SurfaceAdapter,
            Self::SurfaceIdExhausted => RendererPresentError::SurfaceIdExhausted,
            Self::SurfaceNotFound { .. } => RendererPresentError::SurfaceNotFound,
            Self::SurfaceRequiresReconfigure { .. } => {
                RendererPresentError::SurfaceRequiresReconfigure
            }
            Self::InvalidSurfaceState { .. } => RendererPresentError::InvalidSurfaceState,
            Self::FrameSurfaceMismatch { .. } => RendererPresentError::FrameSurfaceMismatch,
            Self::AdapterUnavailable { .. } => RendererPresentError::AdapterUnavailable,
            Self::DeviceUnavailable { .. } => RendererPresentError::DeviceUnavailable,
            Self::FrameNotRendered { .. } => RendererPresentError::FrameNotRendered,
            Self::NoPresentModes { .. } => RendererPresentError::NoPresentModes,
            Self::NoSurfaceFormats { .. } => RendererPresentError::NoSurfaceFormats,
            Self::SurfaceTimeout { .. } => RendererPresentError::SurfaceTimeout,
            Self::SurfaceOccluded { .. } => RendererPresentError::SurfaceOccluded,
            Self::SurfaceOutdated { .. } => RendererPresentError::SurfaceOutdated,
            Self::SurfaceLost { .. } => RendererPresentError::SurfaceLost,
            Self::SurfaceValidation { .. } => RendererPresentError::SurfaceValidation,
        }
    }
}

impl fmt::Display for RendererSurfaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Window(error) => write!(formatter, "window surface host failed: {error}"),
            Self::SurfaceAdapter(error) => {
                write!(formatter, "window surface adapter failed: {error}")
            }
            Self::SurfaceIdExhausted => formatter.write_str("renderer surface id space exhausted"),
            Self::SurfaceNotFound { surface } => {
                write!(
                    formatter,
                    "renderer surface {} was not found",
                    surface.get()
                )
            }
            Self::SurfaceRequiresReconfigure { surface } => write!(
                formatter,
                "renderer surface {} requires reconfiguration",
                surface.get()
            ),
            Self::InvalidSurfaceState {
                surface,
                expected,
                actual,
            } => write!(
                formatter,
                "renderer surface {} state invalid: expected {expected}, got {actual:?}",
                surface.get()
            ),
            Self::FrameSurfaceMismatch { expected, actual } => write!(
                formatter,
                "renderer surface frame mismatch: expected {}, got {}",
                expected.get(),
                actual.get()
            ),
            Self::AdapterUnavailable { adapter_label } => {
                write!(
                    formatter,
                    "graphics adapter unavailable for {adapter_label}"
                )
            }
            Self::DeviceUnavailable { device_label } => {
                write!(formatter, "graphics device unavailable for {device_label}")
            }
            Self::FrameNotRendered { frame_label } => {
                write!(
                    formatter,
                    "surface frame was not rendered for {frame_label}"
                )
            }
            Self::NoPresentModes { surface_label } => {
                write!(
                    formatter,
                    "surface reports no present modes for {surface_label}"
                )
            }
            Self::NoSurfaceFormats { surface_label } => {
                write!(formatter, "surface reports no formats for {surface_label}")
            }
            Self::SurfaceTimeout { surface_label, .. } => {
                write!(
                    formatter,
                    "surface acquisition timed out for {surface_label}"
                )
            }
            Self::SurfaceOccluded { surface_label, .. } => {
                write!(formatter, "surface is occluded for {surface_label}")
            }
            Self::SurfaceOutdated { surface_label, .. } => {
                write!(formatter, "surface is outdated for {surface_label}")
            }
            Self::SurfaceLost { surface_label, .. } => {
                write!(formatter, "surface was lost for {surface_label}")
            }
            Self::SurfaceValidation { surface_label, .. } => {
                write!(formatter, "surface validation failed for {surface_label}")
            }
        }
    }
}

impl std::error::Error for RendererSurfaceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Window(error) => Some(error),
            Self::SurfaceAdapter(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WindowError> for RendererSurfaceError {
    fn from(error: WindowError) -> Self {
        Self::Window(error)
    }
}

impl From<SurfaceError> for RendererSurfaceError {
    fn from(error: SurfaceError) -> Self {
        Self::SurfaceAdapter(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::wgpu::surface::WgpuSurfaceAcquireFailure;

    #[test]
    fn renderer_surface_config_defaults_to_renderer_owned_policy() {
        let config = RendererSurfaceConfig::default();

        assert_eq!(
            config.backend_preference(),
            RendererSurfaceBackendPreference::Primary
        );
        assert_eq!(
            config.power_preference(),
            RendererSurfacePowerPreference::HighPerformance
        );
        assert_eq!(
            config.present_mode_policy(),
            RendererSurfacePresentModePolicy::PreferFifo
        );
        assert_eq!(config.desired_maximum_frame_latency(), 2);
        assert_eq!(
            config.clear_desc().stable_name,
            "fun_renderer.windowed_surface.clear"
        );
    }

    #[test]
    fn renderer_surface_store_allocates_surface_ids() -> Result<(), RendererSurfaceError> {
        let mut store = RendererSurfaceStore::new();
        let first = store.allocate_handle()?;
        let second = store.allocate_handle()?;

        assert_eq!(first.get(), 1);
        assert_eq!(second.get(), 2);
        assert_ne!(first, second);
        Ok(())
    }

    #[test]
    fn renderer_surface_select_surface_format_prefers_srgb() -> Result<(), RendererSurfaceError> {
        assert_eq!(
            select_surface_format(
                &[
                    ::wgpu::TextureFormat::Rgba8Unorm,
                    ::wgpu::TextureFormat::Bgra8UnormSrgb,
                ],
                "fun_renderer.test.surface",
            )?,
            ::wgpu::TextureFormat::Bgra8UnormSrgb
        );
        Ok(())
    }

    #[test]
    fn renderer_surface_select_surface_format_falls_back_to_first()
    -> Result<(), RendererSurfaceError> {
        assert_eq!(
            select_surface_format(
                &[
                    ::wgpu::TextureFormat::Rgba16Float,
                    ::wgpu::TextureFormat::Rgba8Unorm,
                ],
                "fun_renderer.test.surface",
            )?,
            ::wgpu::TextureFormat::Rgba16Float
        );
        Ok(())
    }

    #[test]
    fn renderer_surface_present_policy_prefers_fifo() -> Result<(), RendererSurfaceError> {
        assert_eq!(
            select_present_mode(
                &[::wgpu::PresentMode::Immediate, ::wgpu::PresentMode::Fifo],
                RendererSurfacePresentModePolicy::PreferFifo,
                "fun_renderer.test.surface",
            )?,
            ::wgpu::PresentMode::Fifo
        );
        Ok(())
    }

    #[test]
    fn renderer_surface_present_policy_uses_first_supported_when_preference_is_missing()
    -> Result<(), RendererSurfaceError> {
        assert_eq!(
            select_present_mode(
                &[::wgpu::PresentMode::Fifo],
                RendererSurfacePresentModePolicy::PreferImmediate,
                "fun_renderer.test.surface",
            )?,
            ::wgpu::PresentMode::Fifo
        );
        Ok(())
    }

    #[test]
    fn renderer_surface_acquire_failures_keep_specific_error_kinds() {
        let handle = RendererSurfaceHandle::new(SurfaceId::PRIMARY);
        let label = "fun_renderer.test.surface";

        assert!(matches!(
            map_acquire_failure(WgpuSurfaceAcquireFailure::Timeout, handle, label),
            RendererSurfaceError::SurfaceTimeout { surface, surface_label }
                if surface == handle && surface_label == label
        ));
        assert!(matches!(
            map_acquire_failure(WgpuSurfaceAcquireFailure::Occluded, handle, label),
            RendererSurfaceError::SurfaceOccluded { surface, surface_label }
                if surface == handle && surface_label == label
        ));
        assert!(matches!(
            map_acquire_failure(WgpuSurfaceAcquireFailure::Outdated, handle, label),
            RendererSurfaceError::SurfaceOutdated { surface, surface_label }
                if surface == handle && surface_label == label
        ));
        assert!(matches!(
            map_acquire_failure(WgpuSurfaceAcquireFailure::Lost, handle, label),
            RendererSurfaceError::SurfaceLost { surface, surface_label }
                if surface == handle && surface_label == label
        ));
        assert!(matches!(
            map_acquire_failure(WgpuSurfaceAcquireFailure::Validation, handle, label),
            RendererSurfaceError::SurfaceValidation { surface, surface_label }
                if surface == handle && surface_label == label
        ));
    }

    #[test]
    fn renderer_present_errors_classify_present_status() {
        assert_eq!(
            RendererPresentError::SurfaceTimeout.status(),
            RendererPresentStatus::Timeout
        );
        assert_eq!(
            RendererPresentError::SurfaceOccluded.status(),
            RendererPresentStatus::Occluded
        );
        assert_eq!(
            RendererPresentError::SurfaceOutdated.status(),
            RendererPresentStatus::Outdated
        );
        assert_eq!(
            RendererPresentError::SurfaceLost.status(),
            RendererPresentStatus::Lost
        );
        assert_eq!(
            RendererPresentError::SurfaceValidation.status(),
            RendererPresentStatus::ValidationFailed
        );
        assert!(RendererPresentStatus::Presented.presented());
        assert!(RendererPresentStatus::Lost.failed());
    }

    #[test]
    fn renderer_surface_status_tracks_dirty_reconfigure_state() {
        let handle = RendererSurfaceHandle::new(SurfaceId::PRIMARY);
        let status = RendererSurfaceStatus::created(
            handle,
            WindowId::PRIMARY,
            WindowSize::new(640, 480).expect("valid size"),
            DpiScale::new(1.25).expect("valid scale"),
            NativeBackend::Dx12,
        );

        assert_eq!(status.surface_id(), SurfaceId::PRIMARY);
        assert_eq!(status.window(), WindowId::PRIMARY);
        assert_eq!(status.physical_size().width(), 640);
        assert_eq!(status.dpi_scale().get(), 1.25);
        assert_eq!(status.selected_backend(), NativeBackend::Dx12);
        assert!(status.dirty());
        assert_eq!(
            status.reconfigure_state(),
            RendererSurfaceReconfigureState::Dirty
        );
        assert_eq!(
            status.lifecycle_state(),
            RendererSurfaceLifecycleState::Created
        );
    }

    #[test]
    fn renderer_surface_frame_report_is_renderer_owned() {
        let report = RendererSurfaceFrameReport::new(
            WindowId::PRIMARY,
            SurfaceId::PRIMARY,
            WindowSize::new(1920, 1080).expect("valid size"),
            DpiScale::new(1.5).expect("valid scale"),
            RendererPresentStatus::Presented,
        );

        assert_eq!(report.window(), WindowId::PRIMARY);
        assert_eq!(report.surface_id(), SurfaceId::PRIMARY);
        assert_eq!(report.width(), 1920);
        assert_eq!(report.height(), 1080);
        assert_eq!(report.scale_microunits(), 1_500_000);
        assert_eq!(report.present_status(), RendererPresentStatus::Presented);
        assert_eq!(
            RendererFrameOutcome::presented(report).surface_frame(),
            Some(report)
        );
    }
}
