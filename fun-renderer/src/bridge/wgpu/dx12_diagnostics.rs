use crate::backend::{BackendCapabilityReport, NativeBackend, WgpuBackendTruthStatus};

use super::{
    WgpuNativeBackend,
    core::{WgpuCoreBridge, stable_message_hash},
    device::{WgpuAdapterDeviceType, WgpuAdapterInfo, WgpuFeatureSummary, WgpuLimitSummary},
    hal::{HalInteropFailureReason, NativeInteropCapabilities, hal_bridge_status},
};

pub const DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION: u16 = 1;
pub const DX12_BRIDGE_DIAGNOSTICS_CANONICAL_ARTIFACT_PATH: &str =
    "fun-data/renderer/bridge_dx12_native_health.funpb.zst";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12DebugLayerStatus {
    Available,
    Disabled,
    NotInstalled,
    NotSupportedByBridge,
    Unknown,
}

impl Dx12DebugLayerStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Disabled => "disabled",
            Self::NotInstalled => "not_installed",
            Self::NotSupportedByBridge => "not_supported_by_bridge",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12GpuValidationStatus {
    Available,
    Disabled,
    NotInstalled,
    NotSupportedByBridge,
    Unknown,
}

impl Dx12GpuValidationStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Disabled => "disabled",
            Self::NotInstalled => "not_installed",
            Self::NotSupportedByBridge => "not_supported_by_bridge",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12PixMarkerStatus {
    Available,
    NotSupportedByBridge,
    PixNotAttached,
    Unknown,
}

impl Dx12PixMarkerStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NotSupportedByBridge => "not_supported_by_bridge",
            Self::PixNotAttached => "pix_not_attached",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12DredStatus {
    Available,
    Disabled,
    NotSupportedByBridge,
    DeviceRemoved,
    Unknown,
}

impl Dx12DredStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Disabled => "disabled",
            Self::NotSupportedByBridge => "not_supported_by_bridge",
            Self::DeviceRemoved => "device_removed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12CommandListBridgeStatus {
    Available,
    NativeCommandListUnavailable,
    BackendNotDx12,
    Unknown,
}

impl Dx12CommandListBridgeStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::NativeCommandListUnavailable => "native_command_list_unavailable",
            Self::BackendNotDx12 => "backend_not_dx12",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn from_capabilities(
        actual_backend: NativeBackend,
        capabilities: NativeInteropCapabilities,
    ) -> Self {
        if !matches!(actual_backend, NativeBackend::Dx12) {
            return Self::BackendNotDx12;
        }
        if capabilities.native_command_list_available_dx12 {
            Self::Available
        } else {
            Self::NativeCommandListUnavailable
        }
    }

    #[must_use]
    pub const fn failure_reason(self) -> Option<HalInteropFailureReason> {
        match self {
            Self::Available => None,
            Self::NativeCommandListUnavailable => {
                Some(HalInteropFailureReason::NativeCommandListUnavailable)
            }
            Self::BackendNotDx12 => Some(HalInteropFailureReason::WrongActualBackend),
            Self::Unknown => Some(HalInteropFailureReason::HalAccessUnavailable),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12CommandListBridgeProbe {
    pub schema_version: u16,
    pub status: Dx12CommandListBridgeStatus,
    pub native_device_available: bool,
    pub native_queue_available: bool,
    pub native_command_encoder_available: bool,
    pub native_texture_handle_available: bool,
    pub native_command_list_available_dx12: bool,
}

impl Dx12CommandListBridgeProbe {
    #[must_use]
    pub const fn from_capabilities(
        actual_backend: NativeBackend,
        capabilities: NativeInteropCapabilities,
    ) -> Self {
        Self {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            status: Dx12CommandListBridgeStatus::from_capabilities(actual_backend, capabilities),
            native_device_available: capabilities.native_device_available,
            native_queue_available: capabilities.native_queue_available,
            native_command_encoder_available: capabilities.native_command_encoder_available,
            native_texture_handle_available: capabilities.native_texture_handle_available,
            native_command_list_available_dx12: capabilities.native_command_list_available_dx12,
        }
    }

    #[must_use]
    pub const fn from_report(report: BackendCapabilityReport) -> Self {
        Self::from_capabilities(
            report.actual_native_backend,
            NativeInteropCapabilities::from_report(report),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12DriverReport {
    pub schema_version: u16,
    pub adapter_name_hash: u64,
    pub adapter_name_len: u32,
    pub adapter_vendor: u32,
    pub adapter_device_id: u32,
    pub adapter_device_type: WgpuAdapterDeviceType,
    pub driver_string_hash: u64,
    pub driver_info_hash: u64,
}

impl Dx12DriverReport {
    #[must_use]
    pub fn from_wgpu_adapter_info(info: &::wgpu::AdapterInfo) -> Self {
        Self {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            adapter_name_hash: stable_message_hash(info.name.as_str()),
            adapter_name_len: info.name.len().min(u32::MAX as usize) as u32,
            adapter_vendor: info.vendor,
            adapter_device_id: info.device,
            adapter_device_type: super::device::device_type(info.device_type),
            driver_string_hash: stable_message_hash(info.driver.as_str()),
            driver_info_hash: stable_message_hash(info.driver_info.as_str()),
        }
    }

    #[must_use]
    pub fn from_redacted_adapter_info(info: WgpuAdapterInfo<'_>) -> Self {
        Self {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            adapter_name_hash: stable_message_hash(info.name),
            adapter_name_len: info.name.len().min(u32::MAX as usize) as u32,
            adapter_vendor: info.vendor,
            adapter_device_id: info.device,
            adapter_device_type: info.device_type,
            driver_string_hash: 0,
            driver_info_hash: 0,
        }
    }

    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            adapter_name_hash: 0,
            adapter_name_len: 0,
            adapter_vendor: 0,
            adapter_device_id: 0,
            adapter_device_type: WgpuAdapterDeviceType::Other,
            driver_string_hash: 0,
            driver_info_hash: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12DiagnosticHooks {
    pub schema_version: u16,
    pub debug_layer: Dx12DebugLayerStatus,
    pub gpu_based_validation: Dx12GpuValidationStatus,
    pub pix_markers: Dx12PixMarkerStatus,
    pub dred: Dx12DredStatus,
}

impl Dx12DiagnosticHooks {
    /// Conservative defaults: the public wgpu-hal bridge does not expose
    /// the D3D12 debug layer, GBV, PIX user markers, or DRED. Pass 16 lands
    /// the typed surfaces; later passes can flip these to `Available` once
    /// the bridge plumbing actually proves them at runtime.
    pub const PUBLIC_WGPU_BRIDGE_DEFAULTS: Self = Self {
        schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
        debug_layer: Dx12DebugLayerStatus::NotSupportedByBridge,
        gpu_based_validation: Dx12GpuValidationStatus::NotSupportedByBridge,
        pix_markers: Dx12PixMarkerStatus::NotSupportedByBridge,
        dred: Dx12DredStatus::NotSupportedByBridge,
    };

    #[must_use]
    pub const fn from_instance_flags(flags: ::wgpu::InstanceFlags) -> Self {
        let validation = flags.contains(::wgpu::InstanceFlags::VALIDATION);
        let gpu_based = flags.contains(::wgpu::InstanceFlags::GPU_BASED_VALIDATION);
        let debug_layer = if validation {
            Dx12DebugLayerStatus::Available
        } else {
            Dx12DebugLayerStatus::Disabled
        };
        let gpu_validation = if gpu_based {
            Dx12GpuValidationStatus::Available
        } else if validation {
            Dx12GpuValidationStatus::Disabled
        } else {
            Dx12GpuValidationStatus::NotInstalled
        };
        Self {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            debug_layer,
            gpu_based_validation: gpu_validation,
            pix_markers: Dx12PixMarkerStatus::NotSupportedByBridge,
            dred: Dx12DredStatus::NotSupportedByBridge,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12NativeBackendHealth {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub requested_backend: NativeBackend,
    pub actual_backend: NativeBackend,
    pub backend_truth_status: WgpuBackendTruthStatus,
    pub driver_report: Dx12DriverReport,
    pub feature_summary: WgpuFeatureSummary,
    pub limit_summary: WgpuLimitSummary,
    pub interop_capabilities: NativeInteropCapabilities,
    pub command_list_bridge: Dx12CommandListBridgeProbe,
    pub diagnostic_hooks: Dx12DiagnosticHooks,
}

impl Dx12NativeBackendHealth {
    #[must_use]
    pub fn from_runtime<B: WgpuNativeBackend>(
        adapter_info: &::wgpu::AdapterInfo,
        feature_summary: WgpuFeatureSummary,
        limit_summary: WgpuLimitSummary,
        instance_flags: ::wgpu::InstanceFlags,
    ) -> Self {
        let actual_backend = super::device::native_backend(adapter_info.backend);
        let truth = WgpuCoreBridge::<B>::new().backend_truth_from_native_backend(actual_backend);
        let report = if matches!(B::NATIVE_BACKEND, NativeBackend::Dx12) {
            BackendCapabilityReport::WGPU_DX12
        } else {
            B::CAPABILITY_REPORT
        };
        let capabilities = NativeInteropCapabilities::from_report(report);
        Self {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            canonical_path: DX12_BRIDGE_DIAGNOSTICS_CANONICAL_ARTIFACT_PATH,
            requested_backend: B::NATIVE_BACKEND,
            actual_backend,
            backend_truth_status: truth.status,
            driver_report: Dx12DriverReport::from_wgpu_adapter_info(adapter_info),
            feature_summary,
            limit_summary,
            interop_capabilities: capabilities,
            command_list_bridge: Dx12CommandListBridgeProbe::from_capabilities(
                actual_backend,
                capabilities,
            ),
            diagnostic_hooks: Dx12DiagnosticHooks::from_instance_flags(instance_flags),
        }
    }

    #[must_use]
    pub fn from_failure<B: WgpuNativeBackend>(
        actual_backend: NativeBackend,
        adapter_info: WgpuAdapterInfo<'_>,
    ) -> Self {
        let report = B::CAPABILITY_REPORT;
        let capabilities = NativeInteropCapabilities::from_report(report);
        let truth = WgpuCoreBridge::<B>::new().backend_truth_from_native_backend(actual_backend);
        Self {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            canonical_path: DX12_BRIDGE_DIAGNOSTICS_CANONICAL_ARTIFACT_PATH,
            requested_backend: B::NATIVE_BACKEND,
            actual_backend,
            backend_truth_status: truth.status,
            driver_report: Dx12DriverReport::from_redacted_adapter_info(adapter_info),
            feature_summary: WgpuFeatureSummary::default(),
            limit_summary: WgpuLimitSummary::default(),
            interop_capabilities: capabilities,
            command_list_bridge: Dx12CommandListBridgeProbe::from_capabilities(
                actual_backend,
                capabilities,
            ),
            diagnostic_hooks: Dx12DiagnosticHooks::PUBLIC_WGPU_BRIDGE_DEFAULTS,
        }
    }

    #[must_use]
    pub const fn product_dx12_truth_holds(&self) -> bool {
        matches!(self.requested_backend, NativeBackend::Dx12)
            && matches!(self.actual_backend, NativeBackend::Dx12)
            && matches!(
                self.backend_truth_status,
                WgpuBackendTruthStatus::MatchesRequestedBackend
            )
    }

    #[must_use]
    pub const fn command_list_status(&self) -> Dx12CommandListBridgeStatus {
        self.command_list_bridge.status
    }

    #[must_use]
    pub fn truthful_native_interop_summary(&self) -> NativeInteropCapabilities {
        if matches!(self.actual_backend, NativeBackend::Dx12) {
            self.interop_capabilities
        } else {
            NativeInteropCapabilities::NONE
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dx12ProductionGateError {
    ActualBackendMismatch {
        requested: NativeBackend,
        actual: NativeBackend,
    },
    NotDx12Bridge {
        requested: NativeBackend,
    },
}

impl Dx12ProductionGateError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ActualBackendMismatch { .. } => "actual_backend_mismatch",
            Self::NotDx12Bridge { .. } => "not_dx12_bridge",
        }
    }

    #[must_use]
    pub const fn requested(self) -> NativeBackend {
        match self {
            Self::ActualBackendMismatch { requested, .. } | Self::NotDx12Bridge { requested } => {
                requested
            }
        }
    }

    #[must_use]
    pub const fn actual(self) -> NativeBackend {
        match self {
            Self::ActualBackendMismatch { actual, .. } => actual,
            Self::NotDx12Bridge { .. } => NativeBackend::Unknown,
        }
    }
}

pub const fn enforce_dx12_production_gate<B: WgpuNativeBackend>(
    actual_backend: NativeBackend,
) -> Result<(), Dx12ProductionGateError> {
    if !matches!(B::NATIVE_BACKEND, NativeBackend::Dx12) {
        return Err(Dx12ProductionGateError::NotDx12Bridge {
            requested: B::NATIVE_BACKEND,
        });
    }
    if !matches!(actual_backend, NativeBackend::Dx12) {
        return Err(Dx12ProductionGateError::ActualBackendMismatch {
            requested: B::NATIVE_BACKEND,
            actual: actual_backend,
        });
    }
    Ok(())
}

#[must_use]
pub fn dx12_command_list_probe_from_capability_report(
    report: BackendCapabilityReport,
) -> Dx12CommandListBridgeProbe {
    Dx12CommandListBridgeProbe::from_report(report)
}

#[must_use]
pub const fn dx12_native_backend_health_default() -> Dx12NativeBackendHealth {
    Dx12NativeBackendHealth {
        schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
        canonical_path: DX12_BRIDGE_DIAGNOSTICS_CANONICAL_ARTIFACT_PATH,
        requested_backend: NativeBackend::Dx12,
        actual_backend: NativeBackend::Unknown,
        backend_truth_status: WgpuBackendTruthStatus::Unknown,
        driver_report: Dx12DriverReport::unknown(),
        feature_summary: WgpuFeatureSummary {
            bits_low: 0,
            bits_high: 0,
            timestamp_query: false,
            pipeline_cache: false,
            shader_f16: false,
            ray_tracing_acceleration_structure: false,
            mesh_shader: false,
        },
        limit_summary: WgpuLimitSummary {
            max_texture_dimension_2d: 0,
            max_bind_groups: 0,
            max_bindings_per_bind_group: 0,
            max_buffer_size: 0,
            max_color_attachments: 0,
            max_compute_workgroups_per_dimension: 0,
        },
        interop_capabilities: NativeInteropCapabilities::NONE,
        command_list_bridge: Dx12CommandListBridgeProbe {
            schema_version: DX12_BRIDGE_DIAGNOSTICS_SCHEMA_VERSION,
            status: Dx12CommandListBridgeStatus::Unknown,
            native_device_available: false,
            native_queue_available: false,
            native_command_encoder_available: false,
            native_texture_handle_available: false,
            native_command_list_available_dx12: false,
        },
        diagnostic_hooks: Dx12DiagnosticHooks::PUBLIC_WGPU_BRIDGE_DEFAULTS,
    }
}

#[must_use]
pub fn dx12_command_list_capabilities_for(
    report: BackendCapabilityReport,
) -> NativeInteropCapabilities {
    let _ = hal_bridge_status; // Re-export check; ensures the hal layer is wired.
    NativeInteropCapabilities::from_report(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::wgpu::{Dx12Native, MetalNative, VulkanNative};

    #[test]
    fn enforce_dx12_production_gate_passes_for_dx12_actual_backend() {
        assert_eq!(
            enforce_dx12_production_gate::<Dx12Native>(NativeBackend::Dx12),
            Ok(())
        );
    }

    #[test]
    fn enforce_dx12_production_gate_fails_when_actual_backend_is_not_dx12() {
        let err = enforce_dx12_production_gate::<Dx12Native>(NativeBackend::Vulkan)
            .expect_err("Vulkan adapter must not pose as DX12");
        assert_eq!(
            err,
            Dx12ProductionGateError::ActualBackendMismatch {
                requested: NativeBackend::Dx12,
                actual: NativeBackend::Vulkan,
            }
        );
        assert_eq!(err.as_str(), "actual_backend_mismatch");
        assert_eq!(err.requested(), NativeBackend::Dx12);
        assert_eq!(err.actual(), NativeBackend::Vulkan);
    }

    #[test]
    fn enforce_dx12_production_gate_rejects_non_dx12_bridge() {
        let err_vulkan = enforce_dx12_production_gate::<VulkanNative>(NativeBackend::Vulkan)
            .expect_err("Vulkan bridge must not satisfy DX12 production gate");
        assert_eq!(
            err_vulkan,
            Dx12ProductionGateError::NotDx12Bridge {
                requested: NativeBackend::Vulkan,
            }
        );
        let err_metal = enforce_dx12_production_gate::<MetalNative>(NativeBackend::Metal)
            .expect_err("Metal bridge must not satisfy DX12 production gate");
        assert!(matches!(
            err_metal,
            Dx12ProductionGateError::NotDx12Bridge { .. }
        ));
    }

    #[test]
    fn dx12_command_list_probe_reports_unavailable_through_public_wgpu_hal_bridge() {
        let probe = Dx12CommandListBridgeProbe::from_report(BackendCapabilityReport::WGPU_DX12);

        assert_eq!(
            probe.status,
            Dx12CommandListBridgeStatus::NativeCommandListUnavailable
        );
        assert!(probe.native_device_available);
        assert!(probe.native_queue_available);
        assert!(probe.native_texture_handle_available);
        assert!(!probe.native_command_list_available_dx12);
        assert_eq!(
            probe.status.failure_reason(),
            Some(HalInteropFailureReason::NativeCommandListUnavailable)
        );
    }

    #[test]
    fn dx12_command_list_probe_reports_backend_mismatch_when_running_on_vulkan_actual() {
        let probe = Dx12CommandListBridgeProbe::from_capabilities(
            NativeBackend::Vulkan,
            NativeInteropCapabilities::from_report(BackendCapabilityReport::WGPU_DX12),
        );

        assert_eq!(probe.status, Dx12CommandListBridgeStatus::BackendNotDx12);
        assert_eq!(
            probe.status.failure_reason(),
            Some(HalInteropFailureReason::WrongActualBackend)
        );
    }

    #[test]
    fn dx12_diagnostic_hooks_default_reports_all_capabilities_unsupported_by_public_bridge() {
        let hooks = Dx12DiagnosticHooks::PUBLIC_WGPU_BRIDGE_DEFAULTS;
        assert_eq!(
            hooks.debug_layer,
            Dx12DebugLayerStatus::NotSupportedByBridge
        );
        assert_eq!(
            hooks.gpu_based_validation,
            Dx12GpuValidationStatus::NotSupportedByBridge
        );
        assert_eq!(hooks.pix_markers, Dx12PixMarkerStatus::NotSupportedByBridge);
        assert_eq!(hooks.dred, Dx12DredStatus::NotSupportedByBridge);
    }

    #[test]
    fn dx12_diagnostic_hooks_promote_debug_layer_when_validation_flag_set() {
        let hooks = Dx12DiagnosticHooks::from_instance_flags(::wgpu::InstanceFlags::VALIDATION);

        assert_eq!(hooks.debug_layer, Dx12DebugLayerStatus::Available);
        assert_eq!(
            hooks.gpu_based_validation,
            Dx12GpuValidationStatus::Disabled
        );
        // PIX markers + DRED stay unsupported until the bridge plumbing proves them.
        assert_eq!(hooks.pix_markers, Dx12PixMarkerStatus::NotSupportedByBridge);
        assert_eq!(hooks.dred, Dx12DredStatus::NotSupportedByBridge);
    }

    #[test]
    fn dx12_diagnostic_hooks_promote_gbv_when_gpu_validation_flag_set() {
        let hooks = Dx12DiagnosticHooks::from_instance_flags(
            ::wgpu::InstanceFlags::VALIDATION | ::wgpu::InstanceFlags::GPU_BASED_VALIDATION,
        );

        assert_eq!(hooks.debug_layer, Dx12DebugLayerStatus::Available);
        assert_eq!(
            hooks.gpu_based_validation,
            Dx12GpuValidationStatus::Available
        );
    }

    #[test]
    fn dx12_native_backend_health_canonical_artifact_path_uses_funpb_zst() {
        let path = DX12_BRIDGE_DIAGNOSTICS_CANONICAL_ARTIFACT_PATH;
        assert!(path.ends_with(".funpb.zst"));
        assert!(!path.ends_with(".funpb.live.zst"));
        assert!(!path.ends_with(".funpb.sum.zst"));
    }

    #[test]
    fn dx12_native_backend_health_failure_path_redacts_adapter_identity() {
        let info = WgpuAdapterInfo::unknown_for(NativeBackend::Unknown);
        let health =
            Dx12NativeBackendHealth::from_failure::<Dx12Native>(NativeBackend::Unknown, info);

        assert!(!health.product_dx12_truth_holds());
        assert_eq!(
            health.command_list_status(),
            Dx12CommandListBridgeStatus::BackendNotDx12
        );
        let truthful = health.truthful_native_interop_summary();
        assert!(!truthful.native_device_available);
        assert!(!truthful.native_command_list_available_dx12);
        assert_eq!(health.driver_report.driver_string_hash, 0);
        assert_eq!(health.driver_report.driver_info_hash, 0);
    }

    #[test]
    fn dx12_default_health_starts_unknown_until_runtime_probes() {
        let health = dx12_native_backend_health_default();
        assert_eq!(health.requested_backend, NativeBackend::Dx12);
        assert_eq!(health.actual_backend, NativeBackend::Unknown);
        assert_eq!(health.backend_truth_status, WgpuBackendTruthStatus::Unknown);
        assert!(!health.product_dx12_truth_holds());
    }

    #[test]
    fn dx12_command_list_capability_smoke_keeps_hal_bridge_status_function_wired() {
        let _capabilities = dx12_command_list_capabilities_for(BackendCapabilityReport::WGPU_DX12);
        // touching Dx12Native keeps the trait import live in this module.
        assert_eq!(Dx12Native::NATIVE_BACKEND, NativeBackend::Dx12);
    }
}
