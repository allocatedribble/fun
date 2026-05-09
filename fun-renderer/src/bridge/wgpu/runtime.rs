use std::sync::{Arc, Mutex};

use crate::backend::{BackendCapabilityReport, NativeBackend, WgpuBackendTruthStatus};

use super::{
    WgpuBindingBridgeCache, WgpuBridgeResourceRealizationMap, WgpuNativeBackend,
    WgpuPipelineBridgeCache,
    core::{WgpuCoreBridge, WgpuCoreBridgeSnapshot, WgpuCoreDeviceState, WgpuCoreHookSink},
    device::{WgpuAdapterInfo, WgpuFeatureSummary, WgpuLimitSummary},
    diagnostics::{WgpuBridgeHealthReport, WgpuDescriptorCache},
    hal::{NativeInteropCapabilities, WgpuHalBridgeStatus, hal_bridge_status},
    naga::WgpuNagaBridgeStatus,
};

pub const WGPU_BRIDGE_RUNTIME_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone)]
pub struct WgpuBridgeRuntimeOptions {
    pub power_preference: ::wgpu::PowerPreference,
    pub force_fallback_adapter: bool,
    pub required_features: ::wgpu::Features,
    pub required_limits: ::wgpu::Limits,
    pub label_prefix: &'static str,
}

impl WgpuBridgeRuntimeOptions {
    #[must_use]
    pub fn production_default() -> Self {
        Self {
            power_preference: ::wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            required_features: ::wgpu::Features::empty(),
            required_limits: ::wgpu::Limits::default(),
            label_prefix: "fun_renderer",
        }
    }

    #[must_use]
    pub fn fallback_for_tests() -> Self {
        Self {
            power_preference: ::wgpu::PowerPreference::LowPower,
            force_fallback_adapter: true,
            required_features: ::wgpu::Features::empty(),
            required_limits: ::wgpu::Limits::downlevel_defaults(),
            label_prefix: "fun_renderer.test",
        }
    }
}

impl Default for WgpuBridgeRuntimeOptions {
    fn default() -> Self {
        Self::production_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuBridgeRuntimeFailureReason {
    InstanceCreationFailed,
    AdapterSelectionFailed,
    DeviceCreationFailed,
    BackendTruthMismatch,
    SurfaceCreationFailed,
    SurfaceConfigurationFailed,
}

impl WgpuBridgeRuntimeFailureReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstanceCreationFailed => "instance_creation_failed",
            Self::AdapterSelectionFailed => "adapter_selection_failed",
            Self::DeviceCreationFailed => "device_creation_failed",
            Self::BackendTruthMismatch => "backend_truth_mismatch",
            Self::SurfaceCreationFailed => "surface_creation_failed",
            Self::SurfaceConfigurationFailed => "surface_configuration_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuBridgeRuntimeFailure {
    pub reason: WgpuBridgeRuntimeFailureReason,
    pub requested_backend: NativeBackend,
    pub actual_backend: NativeBackend,
    pub message_hash: u64,
    pub message_len: u32,
}

impl WgpuBridgeRuntimeFailure {
    #[must_use]
    pub const fn new(
        reason: WgpuBridgeRuntimeFailureReason,
        requested_backend: NativeBackend,
        actual_backend: NativeBackend,
    ) -> Self {
        Self {
            reason,
            requested_backend,
            actual_backend,
            message_hash: 0,
            message_len: 0,
        }
    }

    #[must_use]
    pub fn with_message(mut self, message: &str) -> Self {
        self.message_hash = super::core::stable_message_hash(message);
        self.message_len = super::core::capped_len(message);
        self
    }
}

pub struct WgpuBridgeDeviceState<B: WgpuNativeBackend> {
    pub instance: Arc<::wgpu::Instance>,
    pub adapter: Arc<::wgpu::Adapter>,
    pub device: Arc<::wgpu::Device>,
    pub queue: Arc<::wgpu::Queue>,
    pub adapter_features: WgpuFeatureSummary,
    pub adapter_limits: WgpuLimitSummary,
    pub device_features: WgpuFeatureSummary,
    pub device_limits: WgpuLimitSummary,
    pub actual_native_backend: NativeBackend,
    pub hook_sink: WgpuCoreHookSink,
    pub _backend: ::core::marker::PhantomData<fn() -> B>,
}

impl<B: WgpuNativeBackend> ::core::fmt::Debug for WgpuBridgeDeviceState<B> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("WgpuBridgeDeviceState")
            .field("backend", &B::NAME)
            .field("actual_native_backend", &self.actual_native_backend)
            .field("adapter_features", &self.adapter_features)
            .field("adapter_limits", &self.adapter_limits)
            .field("device_features", &self.device_features)
            .field("device_limits", &self.device_limits)
            .finish()
    }
}

pub struct WgpuBridgeSurfaceState {
    pub surface: Arc<::wgpu::Surface<'static>>,
    pub configuration: ::wgpu::SurfaceConfiguration,
}

impl ::core::fmt::Debug for WgpuBridgeSurfaceState {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("WgpuBridgeSurfaceState")
            .field("width", &self.configuration.width)
            .field("height", &self.configuration.height)
            .field("format", &self.configuration.format)
            .field("present_mode", &self.configuration.present_mode)
            .finish()
    }
}

#[must_use]
pub fn create_wgpu_instance<B: WgpuNativeBackend>() -> Arc<::wgpu::Instance> {
    let descriptor = ::wgpu::InstanceDescriptor {
        backends: B::WGPU_BACKENDS,
        flags: ::wgpu::InstanceFlags::from_build_config(),
        memory_budget_thresholds: ::wgpu::MemoryBudgetThresholds::default(),
        backend_options: ::wgpu::BackendOptions::default(),
        display: None,
    };
    Arc::new(::wgpu::Instance::new(descriptor))
}

pub fn select_adapter<B: WgpuNativeBackend>(
    instance: &::wgpu::Instance,
    options: &WgpuBridgeRuntimeOptions,
    compatible_surface: Option<&::wgpu::Surface<'_>>,
) -> Result<::wgpu::Adapter, WgpuBridgeRuntimeFailure> {
    let request = ::wgpu::RequestAdapterOptions {
        power_preference: options.power_preference,
        force_fallback_adapter: options.force_fallback_adapter,
        compatible_surface,
    };
    block_on_init(instance.request_adapter(&request)).map_err(|error| {
        WgpuBridgeRuntimeFailure::new(
            WgpuBridgeRuntimeFailureReason::AdapterSelectionFailed,
            B::NATIVE_BACKEND,
            NativeBackend::Unknown,
        )
        .with_message(error.to_string().as_str())
    })
}

pub fn create_device_and_queue<B: WgpuNativeBackend>(
    adapter: &::wgpu::Adapter,
    options: &WgpuBridgeRuntimeOptions,
) -> Result<(::wgpu::Device, ::wgpu::Queue), WgpuBridgeRuntimeFailure> {
    let descriptor = ::wgpu::DeviceDescriptor {
        label: Some(options.label_prefix),
        required_features: options.required_features,
        required_limits: options.required_limits.clone(),
        experimental_features: ::wgpu::ExperimentalFeatures::disabled(),
        memory_hints: ::wgpu::MemoryHints::Performance,
        trace: ::wgpu::Trace::Off,
    };
    let actual_backend = super::device::native_backend(adapter.get_info().backend);
    block_on_init(adapter.request_device(&descriptor)).map_err(|error| {
        WgpuBridgeRuntimeFailure::new(
            WgpuBridgeRuntimeFailureReason::DeviceCreationFailed,
            B::NATIVE_BACKEND,
            actual_backend,
        )
        .with_message(error.to_string().as_str())
    })
}

pub fn enforce_backend_truth<B: WgpuNativeBackend>(
    actual_backend: NativeBackend,
) -> Result<(), WgpuBridgeRuntimeFailure> {
    let truth = WgpuCoreBridge::<B>::new().backend_truth_from_native_backend(actual_backend);
    match truth.status {
        WgpuBackendTruthStatus::MatchesRequestedBackend => Ok(()),
        _ => Err(WgpuBridgeRuntimeFailure::new(
            WgpuBridgeRuntimeFailureReason::BackendTruthMismatch,
            B::NATIVE_BACKEND,
            actual_backend,
        )),
    }
}

pub fn initialize_wgpu_bridge_runtime<B: WgpuNativeBackend>(
    options: &WgpuBridgeRuntimeOptions,
) -> Result<WgpuBridgeDeviceState<B>, WgpuBridgeRuntimeFailure> {
    let instance = create_wgpu_instance::<B>();
    let adapter = select_adapter::<B>(&instance, options, None)?;
    let actual_backend = super::device::native_backend(adapter.get_info().backend);
    enforce_backend_truth::<B>(actual_backend)?;

    let adapter_features = WgpuFeatureSummary::from_wgpu(adapter.features());
    let adapter_limits = WgpuLimitSummary::from_wgpu(&adapter.limits());

    let (device, queue) = create_device_and_queue::<B>(&adapter, options)?;
    let device_features = WgpuFeatureSummary::from_wgpu(device.features());
    let device_limits = WgpuLimitSummary::from_wgpu(&device.limits());

    let hook_sink: WgpuCoreHookSink = Arc::new(Mutex::new(Default::default()));
    WgpuCoreBridge::<B>::new().install_device_hooks(&device, Arc::clone(&hook_sink));

    Ok(WgpuBridgeDeviceState {
        instance,
        adapter: Arc::new(adapter),
        device: Arc::new(device),
        queue: Arc::new(queue),
        adapter_features,
        adapter_limits,
        device_features,
        device_limits,
        actual_native_backend: actual_backend,
        hook_sink,
        _backend: ::core::marker::PhantomData,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuBridgeHealthArtifact {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub health: OwnedWgpuBridgeHealthReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OwnedWgpuBridgeHealthReport {
    pub schema_version: u16,
    pub selected_wgpu_backend: ::wgpu::Backends,
    pub requested_native_backend: NativeBackend,
    pub actual_native_backend: NativeBackend,
    pub adapter_name_hash: u64,
    pub adapter_name_len: u32,
    pub adapter_vendor: u32,
    pub adapter_device_id: u32,
    pub adapter_device_type: super::device::WgpuAdapterDeviceType,
    pub validation_status: crate::backend::WgpuCoreValidationStatus,
    pub backend_truth_status: WgpuBackendTruthStatus,
    pub feature_summary: WgpuFeatureSummary,
    pub limit_summary: WgpuLimitSummary,
    pub hal_status: WgpuHalBridgeStatus,
    pub interop_capabilities: NativeInteropCapabilities,
    pub naga_status: WgpuNagaBridgeStatus,
    pub pipeline_cache_runtime_failures: u32,
}

impl OwnedWgpuBridgeHealthReport {
    #[must_use]
    pub fn from_health_report(report: &WgpuBridgeHealthReport<'_>) -> Self {
        Self {
            schema_version: report.schema_version,
            selected_wgpu_backend: report.selected_wgpu_backend,
            requested_native_backend: report.core_bridge_status.backend_truth.requested_backend,
            actual_native_backend: report.actual_native_backend,
            adapter_name_hash: super::core::stable_message_hash(report.adapter_info.name),
            adapter_name_len: super::core::capped_len(report.adapter_info.name),
            adapter_vendor: report.adapter_info.vendor,
            adapter_device_id: report.adapter_info.device,
            adapter_device_type: report.adapter_info.device_type,
            validation_status: report.validation_status,
            backend_truth_status: report.core_bridge_status.backend_truth.status,
            feature_summary: report.features,
            limit_summary: report.limits,
            hal_status: WgpuHalBridgeStatus {
                native_handle_support: report.hal_access,
                native_command_encoder: report.command_encoder_availability,
                raw_command_list_available: report
                    .native_interop_capabilities
                    .native_command_list_available_dx12,
                native_interop_capabilities: report.native_interop_capabilities,
            },
            interop_capabilities: report.native_interop_capabilities,
            naga_status: WgpuNagaBridgeStatus {
                status: report.shader_translation_path,
                exposed_to_ecs: false,
            },
            pipeline_cache_runtime_failures: report.pipeline_cache_status.runtime_creation_failures,
        }
    }
}

#[must_use]
pub const fn bridge_health_canonical_artifact_path() -> &'static str {
    "fun-data/renderer/bridge_health.funpb.zst"
}

pub fn build_health_artifact_for_state<B: WgpuNativeBackend>(
    state: &WgpuBridgeDeviceState<B>,
    descriptor_cache: &WgpuDescriptorCache,
) -> WgpuBridgeHealthArtifact {
    let adapter_info = state.adapter.get_info();
    let info = WgpuAdapterInfo::from_wgpu(&adapter_info);
    let report = WgpuBridgeHealthReport::from_parts::<B>(
        info,
        state.device_limits,
        state.device_features,
        descriptor_cache.status(),
    );
    WgpuBridgeHealthArtifact {
        schema_version: WGPU_BRIDGE_RUNTIME_SCHEMA_VERSION,
        canonical_path: bridge_health_canonical_artifact_path(),
        health: OwnedWgpuBridgeHealthReport::from_health_report(&report),
    }
}

#[must_use]
pub fn build_health_artifact_for_unknown_backend<B: WgpuNativeBackend>(
    failure: WgpuBridgeRuntimeFailure,
    descriptor_cache: &WgpuDescriptorCache,
) -> WgpuBridgeHealthArtifact {
    let info = WgpuAdapterInfo::unknown_for(failure.actual_backend);
    let report = WgpuBridgeHealthReport::from_parts::<B>(
        info,
        WgpuLimitSummary::default(),
        WgpuFeatureSummary::default(),
        descriptor_cache.status(),
    );
    WgpuBridgeHealthArtifact {
        schema_version: WGPU_BRIDGE_RUNTIME_SCHEMA_VERSION,
        canonical_path: bridge_health_canonical_artifact_path(),
        health: OwnedWgpuBridgeHealthReport::from_health_report(&report),
    }
}

#[must_use]
pub fn snapshot_core_bridge<B: WgpuNativeBackend>(
    state: &WgpuBridgeDeviceState<B>,
) -> WgpuCoreBridgeSnapshot {
    let info = state.adapter.get_info();
    let identity = super::core::WgpuCoreAdapterIdentity::from_wgpu(&info);
    let device_state = WgpuCoreDeviceState::from_wgpu(&state.device);
    let hook_state = state
        .hook_sink
        .lock()
        .map(|guard| *guard)
        .unwrap_or_default();
    let validation_report = super::core::WgpuCoreValidationReport::new(
        B::CAPABILITY_REPORT.wgpu_core_validation,
        hook_state,
    );
    WgpuCoreBridge::<B>::new().snapshot_from_parts(
        identity,
        state.adapter_features,
        state.adapter_limits,
        device_state,
        validation_report,
    )
}

#[must_use]
pub fn snapshot_hal_status<B: WgpuNativeBackend>() -> WgpuHalBridgeStatus {
    hal_bridge_status(B::CAPABILITY_REPORT)
}

#[must_use]
pub fn build_resource_realization_map() -> WgpuBridgeResourceRealizationMap {
    WgpuBridgeResourceRealizationMap::new()
}

#[must_use]
pub fn build_binding_bridge_cache() -> WgpuBindingBridgeCache {
    WgpuBindingBridgeCache::default()
}

#[must_use]
pub fn build_pipeline_bridge_cache() -> WgpuPipelineBridgeCache {
    WgpuPipelineBridgeCache::default()
}

#[must_use]
pub fn build_descriptor_cache() -> WgpuDescriptorCache {
    WgpuDescriptorCache::default()
}

#[must_use]
pub fn capability_report<B: WgpuNativeBackend>() -> BackendCapabilityReport {
    B::CAPABILITY_REPORT
}

#[doc(hidden)]
fn block_on_init<T>(future: impl ::core::future::Future<Output = T>) -> T {
    use ::core::task::{Context, Poll};
    let mut future = ::core::pin::pin!(future);
    let cx = &mut Context::from_waker(::core::task::Waker::noop());
    loop {
        match future.as_mut().poll(cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => ::core::hint::spin_loop(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::wgpu::{Dx12Native, MetalNative, VulkanNative};

    #[test]
    fn enforce_backend_truth_passes_when_actual_matches_requested() {
        assert!(enforce_backend_truth::<Dx12Native>(NativeBackend::Dx12).is_ok());
        assert!(enforce_backend_truth::<VulkanNative>(NativeBackend::Vulkan).is_ok());
        assert!(enforce_backend_truth::<MetalNative>(NativeBackend::Metal).is_ok());
    }

    #[test]
    fn enforce_backend_truth_fails_when_actual_diverges_from_requested() {
        let err = enforce_backend_truth::<Dx12Native>(NativeBackend::Vulkan)
            .expect_err("DX12 production path must reject Vulkan adapter truth");
        assert_eq!(
            err.reason,
            WgpuBridgeRuntimeFailureReason::BackendTruthMismatch
        );
        assert_eq!(err.requested_backend, NativeBackend::Dx12);
        assert_eq!(err.actual_backend, NativeBackend::Vulkan);
    }

    #[test]
    fn enforce_backend_truth_fails_when_adapter_backend_is_unknown() {
        let err = enforce_backend_truth::<Dx12Native>(NativeBackend::Unknown)
            .expect_err("DX12 production path must reject unknown adapter backend");
        assert_eq!(
            err.reason,
            WgpuBridgeRuntimeFailureReason::BackendTruthMismatch
        );
    }

    #[test]
    fn bridge_health_canonical_artifact_path_uses_funpb_zst_extension() {
        let path = bridge_health_canonical_artifact_path();
        assert!(
            path.ends_with(".funpb.zst"),
            "canonical bridge health artifact must be a compressed protobuf bundle: {}",
            path
        );
        assert!(
            !path.ends_with(".funpb.live.zst"),
            "bridge health is a startup snapshot, not a live stream: {}",
            path
        );
        assert!(
            !path.ends_with(".funpb.sum.zst"),
            "bridge health is a startup snapshot, not a summary aggregate: {}",
            path
        );
    }

    #[test]
    fn unknown_backend_health_artifact_redacts_adapter_identity() {
        let cache = build_descriptor_cache();
        let failure = WgpuBridgeRuntimeFailure::new(
            WgpuBridgeRuntimeFailureReason::AdapterSelectionFailed,
            NativeBackend::Dx12,
            NativeBackend::Unknown,
        );
        let artifact = build_health_artifact_for_unknown_backend::<Dx12Native>(failure, &cache);

        assert_eq!(
            artifact.canonical_path,
            bridge_health_canonical_artifact_path()
        );
        // The artifact must contain the requested backend identity (so consumers can
        // diagnose which static bridge attempted init) and must use the placeholder
        // "unknown" adapter identity rather than any leaked vendor/device ids.
        assert_eq!(
            artifact.health.requested_native_backend,
            NativeBackend::Dx12
        );
        assert_eq!(artifact.health.adapter_vendor, 0);
        assert_eq!(artifact.health.adapter_device_id, 0);
        assert_eq!(
            artifact.health.adapter_device_type,
            super::super::device::WgpuAdapterDeviceType::Other
        );
    }
}
