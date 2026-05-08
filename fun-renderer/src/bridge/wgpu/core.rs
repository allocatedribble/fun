use core::marker::PhantomData;
use std::sync::{Arc, Mutex};

use crate::backend::{
    BackendCapabilityReport, NativeBackend, WgpuBackendTruth, WgpuCoreValidationStatus,
};

use super::{
    WgpuNativeBackend,
    device::{WgpuAdapterDeviceType, WgpuFeatureSummary, WgpuLimitSummary, native_backend},
};

pub const WGPU_CORE_BRIDGE_SCHEMA_VERSION: u16 = 1;
pub const WGPU_EXPECTED_VERSION: &str = "29.0.3";
pub const WGPU_CORE_EXPECTED_VERSION: &str = "29.0.1";
pub const WGPU_HAL_EXPECTED_VERSION: &str = "29.0.1";
pub const NAGA_EXPECTED_VERSION: &str = "29.0.3";
pub const WGPU_UPGRADE_CONTRACT_DOC: &str = "docs/renderer/v4_wgpu_upgrade_contract.md";
const OUT_OF_MEMORY_MESSAGE: &str = "out_of_memory";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuCoreAccessMode {
    PublicWgpuApiOnly,
    PrivateInternalsGuarded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuCoreResourceIdentityTracking {
    WgpuCoreOwned,
    DirectBackendOwned,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuCoreValidationFailureKind {
    Validation,
    OutOfMemory,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreValidationFailureRecord {
    pub kind: WgpuCoreValidationFailureKind,
    pub message_hash: u64,
    pub message_len: u32,
}

impl WgpuCoreValidationFailureRecord {
    #[must_use]
    pub fn from_wgpu_error(error: &::wgpu::Error) -> Self {
        match error {
            ::wgpu::Error::OutOfMemory { .. } => Self::from_message(
                WgpuCoreValidationFailureKind::OutOfMemory,
                OUT_OF_MEMORY_MESSAGE,
            ),
            ::wgpu::Error::Validation { description, .. } => {
                Self::from_message(WgpuCoreValidationFailureKind::Validation, description)
            }
            ::wgpu::Error::Internal { description, .. } => {
                Self::from_message(WgpuCoreValidationFailureKind::Internal, description)
            }
        }
    }

    #[must_use]
    pub fn from_message(kind: WgpuCoreValidationFailureKind, message: &str) -> Self {
        Self {
            kind,
            message_hash: stable_message_hash(message),
            message_len: capped_len(message),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuCoreDeviceLostReason {
    Unknown,
    Destroyed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreDeviceLostRecord {
    pub reason: WgpuCoreDeviceLostReason,
    pub message_hash: u64,
    pub message_len: u32,
}

impl WgpuCoreDeviceLostRecord {
    #[must_use]
    pub fn from_wgpu(reason: ::wgpu::DeviceLostReason, message: &str) -> Self {
        Self {
            reason: device_lost_reason(reason),
            message_hash: stable_message_hash(message),
            message_len: capped_len(message),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreHookState {
    pub validation_failure_count: u32,
    pub out_of_memory_count: u32,
    pub internal_failure_count: u32,
    pub device_lost_count: u32,
    pub latest_validation_failure: Option<WgpuCoreValidationFailureRecord>,
    pub latest_device_lost: Option<WgpuCoreDeviceLostRecord>,
}

impl WgpuCoreHookState {
    pub fn record_validation_failure(&mut self, record: WgpuCoreValidationFailureRecord) {
        match record.kind {
            WgpuCoreValidationFailureKind::Validation => {
                self.validation_failure_count = self.validation_failure_count.saturating_add(1);
            }
            WgpuCoreValidationFailureKind::OutOfMemory => {
                self.out_of_memory_count = self.out_of_memory_count.saturating_add(1);
            }
            WgpuCoreValidationFailureKind::Internal => {
                self.internal_failure_count = self.internal_failure_count.saturating_add(1);
            }
        }
        self.latest_validation_failure = Some(record);
    }

    pub fn record_device_lost(&mut self, record: WgpuCoreDeviceLostRecord) {
        self.device_lost_count = self.device_lost_count.saturating_add(1);
        self.latest_device_lost = Some(record);
    }
}

pub type WgpuCoreHookSink = Arc<Mutex<WgpuCoreHookState>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreDebugHookAvailability {
    pub error_scope_available: bool,
    pub uncaptured_error_hook_available: bool,
    pub device_lost_hook_available: bool,
    pub internal_counters_available: bool,
    pub allocator_report_available: bool,
    pub hub_report_direct_access: bool,
}

impl WgpuCoreDebugHookAvailability {
    pub const PUBLIC_WGPU_API: Self = Self {
        error_scope_available: true,
        uncaptured_error_hook_available: true,
        device_lost_hook_available: true,
        internal_counters_available: true,
        allocator_report_available: true,
        hub_report_direct_access: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreValidationReport {
    pub status: WgpuCoreValidationStatus,
    pub hooks: WgpuCoreDebugHookAvailability,
    pub hook_state: WgpuCoreHookState,
}

impl WgpuCoreValidationReport {
    #[must_use]
    pub const fn new(status: WgpuCoreValidationStatus, hook_state: WgpuCoreHookState) -> Self {
        Self {
            status,
            hooks: WgpuCoreDebugHookAvailability::PUBLIC_WGPU_API,
            hook_state,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreResourceCounters {
    pub buffers: u64,
    pub textures: u64,
    pub texture_views: u64,
    pub bind_groups: u64,
    pub bind_group_layouts: u64,
    pub render_pipelines: u64,
    pub compute_pipelines: u64,
    pub pipeline_layouts: u64,
    pub samplers: u64,
    pub command_encoders: u64,
    pub shader_modules: u64,
    pub query_sets: u64,
    pub fences: u64,
    pub buffer_memory_bytes: u64,
    pub texture_memory_bytes: u64,
    pub acceleration_structure_memory_bytes: u64,
    pub memory_allocations: u64,
}

impl WgpuCoreResourceCounters {
    #[must_use]
    pub fn from_wgpu_internal_counters(counters: &::wgpu::InternalCounters) -> Self {
        let hal = &counters.hal;
        Self {
            buffers: counter_value(hal.buffers.read()),
            textures: counter_value(hal.textures.read()),
            texture_views: counter_value(hal.texture_views.read()),
            bind_groups: counter_value(hal.bind_groups.read()),
            bind_group_layouts: counter_value(hal.bind_group_layouts.read()),
            render_pipelines: counter_value(hal.render_pipelines.read()),
            compute_pipelines: counter_value(hal.compute_pipelines.read()),
            pipeline_layouts: counter_value(hal.pipeline_layouts.read()),
            samplers: counter_value(hal.samplers.read()),
            command_encoders: counter_value(hal.command_encoders.read()),
            shader_modules: counter_value(hal.shader_modules.read()),
            query_sets: counter_value(hal.query_sets.read()),
            fences: counter_value(hal.fences.read()),
            buffer_memory_bytes: counter_value(hal.buffer_memory.read()),
            texture_memory_bytes: counter_value(hal.texture_memory.read()),
            acceleration_structure_memory_bytes: counter_value(
                hal.acceleration_structure_memory.read(),
            ),
            memory_allocations: counter_value(hal.memory_allocations.read()),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreAllocatorSummary {
    pub available: bool,
    pub allocation_count: u32,
    pub block_count: u32,
    pub total_allocated_bytes: u64,
    pub total_reserved_bytes: u64,
}

impl WgpuCoreAllocatorSummary {
    #[must_use]
    pub fn from_wgpu_allocator_report(report: Option<&::wgpu::AllocatorReport>) -> Self {
        match report {
            Some(report) => Self {
                available: true,
                allocation_count: report.allocations.len().min(u32::MAX as usize) as u32,
                block_count: report.blocks.len().min(u32::MAX as usize) as u32,
                total_allocated_bytes: report.total_allocated_bytes,
                total_reserved_bytes: report.total_reserved_bytes,
            },
            None => Self::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreLifecycleReport {
    pub resource_identity_tracking: WgpuCoreResourceIdentityTracking,
    pub resource_counters: WgpuCoreResourceCounters,
    pub allocator_summary: WgpuCoreAllocatorSummary,
}

impl Default for WgpuCoreLifecycleReport {
    fn default() -> Self {
        Self {
            resource_identity_tracking: WgpuCoreResourceIdentityTracking::WgpuCoreOwned,
            resource_counters: WgpuCoreResourceCounters::default(),
            allocator_summary: WgpuCoreAllocatorSummary::default(),
        }
    }
}

impl WgpuCoreLifecycleReport {
    #[must_use]
    pub fn from_wgpu_device(device: &::wgpu::Device) -> Self {
        let counters = device.get_internal_counters();
        let allocator_report = device.generate_allocator_report();
        Self {
            resource_identity_tracking: WgpuCoreResourceIdentityTracking::WgpuCoreOwned,
            resource_counters: WgpuCoreResourceCounters::from_wgpu_internal_counters(&counters),
            allocator_summary: WgpuCoreAllocatorSummary::from_wgpu_allocator_report(
                allocator_report.as_ref(),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreAdapterIdentity {
    pub name_hash: u64,
    pub name_len: u32,
    pub vendor: u32,
    pub device: u32,
    pub device_type: WgpuAdapterDeviceType,
    pub backend: NativeBackend,
}

impl WgpuCoreAdapterIdentity {
    #[must_use]
    pub fn from_wgpu(info: &::wgpu::AdapterInfo) -> Self {
        Self {
            name_hash: stable_message_hash(info.name.as_str()),
            name_len: capped_len(info.name.as_str()),
            vendor: info.vendor,
            device: info.device,
            device_type: super::device::device_type(info.device_type),
            backend: native_backend(info.backend),
        }
    }

    #[must_use]
    pub const fn unknown_for(backend: NativeBackend) -> Self {
        Self {
            name_hash: 0,
            name_len: 0,
            vendor: 0,
            device: 0,
            device_type: WgpuAdapterDeviceType::Other,
            backend,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreDeviceState {
    pub features: WgpuFeatureSummary,
    pub limits: WgpuLimitSummary,
    pub lifecycle: WgpuCoreLifecycleReport,
}

impl WgpuCoreDeviceState {
    #[must_use]
    pub fn from_wgpu(device: &::wgpu::Device) -> Self {
        let limits = device.limits();
        Self {
            features: WgpuFeatureSummary::from_wgpu(device.features()),
            limits: WgpuLimitSummary::from_wgpu(&limits),
            lifecycle: WgpuCoreLifecycleReport::from_wgpu_device(device),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreCompatibilityReport {
    pub expected_wgpu_version: &'static str,
    pub expected_wgpu_core_version: &'static str,
    pub expected_wgpu_hal_version: &'static str,
    pub expected_naga_version: &'static str,
    pub access_mode: WgpuCoreAccessMode,
    pub private_internals_used: bool,
    pub private_internals_contained: bool,
    pub upgrade_contract_doc: &'static str,
}

impl WgpuCoreCompatibilityReport {
    pub const PUBLIC_WGPU_API_ONLY: Self = Self {
        expected_wgpu_version: WGPU_EXPECTED_VERSION,
        expected_wgpu_core_version: WGPU_CORE_EXPECTED_VERSION,
        expected_wgpu_hal_version: WGPU_HAL_EXPECTED_VERSION,
        expected_naga_version: NAGA_EXPECTED_VERSION,
        access_mode: WgpuCoreAccessMode::PublicWgpuApiOnly,
        private_internals_used: false,
        private_internals_contained: true,
        upgrade_contract_doc: WGPU_UPGRADE_CONTRACT_DOC,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreBridgeStatus {
    pub validation: WgpuCoreValidationStatus,
    pub backend_truth: WgpuBackendTruth,
    pub resource_identity_tracking: WgpuCoreResourceIdentityTracking,
    pub debug_hooks: WgpuCoreDebugHookAvailability,
    pub access_mode: WgpuCoreAccessMode,
    pub exposed_to_ecs: bool,
    pub hot_draw_loop_use_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreBridgeSnapshot {
    pub schema_version: u16,
    pub status: WgpuCoreBridgeStatus,
    pub adapter_identity: WgpuCoreAdapterIdentity,
    pub adapter_features: WgpuFeatureSummary,
    pub adapter_limits: WgpuLimitSummary,
    pub device_state: WgpuCoreDeviceState,
    pub validation_report: WgpuCoreValidationReport,
    pub compatibility: WgpuCoreCompatibilityReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreBridge<B: WgpuNativeBackend> {
    marker: PhantomData<fn() -> B>,
}

impl<B: WgpuNativeBackend> WgpuCoreBridge<B> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn expected_native_backend(&self) -> NativeBackend {
        B::NATIVE_BACKEND
    }

    #[must_use]
    pub const fn compatibility_report(&self) -> WgpuCoreCompatibilityReport {
        WgpuCoreCompatibilityReport::PUBLIC_WGPU_API_ONLY
    }

    #[must_use]
    pub fn backend_truth_from_adapter(&self, adapter: &::wgpu::Adapter) -> WgpuBackendTruth {
        let info = adapter.get_info();
        self.backend_truth_from_native_backend(native_backend(info.backend))
    }

    #[must_use]
    pub const fn backend_truth_from_native_backend(
        &self,
        adapter_backend: NativeBackend,
    ) -> WgpuBackendTruth {
        if matches!(adapter_backend, NativeBackend::Unknown) {
            WgpuBackendTruth::UNKNOWN
        } else if matches_backend(B::NATIVE_BACKEND, adapter_backend) {
            WgpuBackendTruth::matched(B::NATIVE_BACKEND)
        } else {
            WgpuBackendTruth::mismatch(B::NATIVE_BACKEND, adapter_backend)
        }
    }

    #[must_use]
    pub fn status_from_adapter_backend(
        &self,
        adapter_backend: NativeBackend,
    ) -> WgpuCoreBridgeStatus {
        core_bridge_status_with_truth(
            B::CAPABILITY_REPORT,
            self.backend_truth_from_native_backend(adapter_backend),
        )
    }

    #[must_use]
    pub fn snapshot_from_parts(
        &self,
        adapter_identity: WgpuCoreAdapterIdentity,
        adapter_features: WgpuFeatureSummary,
        adapter_limits: WgpuLimitSummary,
        device_state: WgpuCoreDeviceState,
        validation_report: WgpuCoreValidationReport,
    ) -> WgpuCoreBridgeSnapshot {
        let backend_truth = self.backend_truth_from_native_backend(adapter_identity.backend);
        WgpuCoreBridgeSnapshot {
            schema_version: WGPU_CORE_BRIDGE_SCHEMA_VERSION,
            status: core_bridge_status_with_truth(B::CAPABILITY_REPORT, backend_truth),
            adapter_identity,
            adapter_features,
            adapter_limits,
            device_state,
            validation_report,
            compatibility: self.compatibility_report(),
        }
    }

    pub fn install_device_hooks(&self, device: &::wgpu::Device, sink: WgpuCoreHookSink) {
        let validation_sink = Arc::clone(&sink);
        device.on_uncaptured_error(Arc::new(move |error| {
            let record = WgpuCoreValidationFailureRecord::from_wgpu_error(&error);
            if let Ok(mut state) = validation_sink.lock() {
                state.record_validation_failure(record);
            }
        }));

        let lost_sink = Arc::clone(&sink);
        device.set_device_lost_callback(move |reason, message| {
            let record = WgpuCoreDeviceLostRecord::from_wgpu(reason, message.as_str());
            if let Ok(mut state) = lost_sink.lock() {
                state.record_device_lost(record);
            }
        });
    }
}

impl<B: WgpuNativeBackend> Default for WgpuCoreBridge<B> {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub const fn core_bridge_status(report: BackendCapabilityReport) -> WgpuCoreBridgeStatus {
    core_bridge_status_with_truth(report, report.wgpu_backend_truth)
}

#[must_use]
pub const fn core_bridge_status_with_truth(
    report: BackendCapabilityReport,
    backend_truth: WgpuBackendTruth,
) -> WgpuCoreBridgeStatus {
    WgpuCoreBridgeStatus {
        validation: report.wgpu_core_validation,
        backend_truth,
        resource_identity_tracking: WgpuCoreResourceIdentityTracking::WgpuCoreOwned,
        debug_hooks: WgpuCoreDebugHookAvailability::PUBLIC_WGPU_API,
        access_mode: WgpuCoreAccessMode::PublicWgpuApiOnly,
        exposed_to_ecs: false,
        hot_draw_loop_use_allowed: false,
    }
}

#[must_use]
pub const fn device_lost_reason(reason: ::wgpu::DeviceLostReason) -> WgpuCoreDeviceLostReason {
    match reason {
        ::wgpu::DeviceLostReason::Unknown => WgpuCoreDeviceLostReason::Unknown,
        ::wgpu::DeviceLostReason::Destroyed => WgpuCoreDeviceLostReason::Destroyed,
    }
}

#[must_use]
pub const fn matches_backend(expected: NativeBackend, actual: NativeBackend) -> bool {
    matches!(
        (expected, actual),
        (NativeBackend::Dx12, NativeBackend::Dx12)
            | (NativeBackend::Vulkan, NativeBackend::Vulkan)
            | (NativeBackend::Metal, NativeBackend::Metal)
    )
}

#[must_use]
pub fn stable_message_hash(message: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    message.as_bytes().iter().fold(FNV_OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
    })
}

#[must_use]
pub fn capped_len(message: &str) -> u32 {
    message.len().min(u32::MAX as usize) as u32
}

#[must_use]
pub fn counter_value(value: isize) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backend::{WgpuBackendTruthStatus, WgpuCoreValidationStatus},
        bridge::wgpu::{Dx12Native, VulkanNative},
    };

    #[test]
    fn core_bridge_reports_adapter_backend_mismatch_from_actual_adapter_truth() {
        let status =
            WgpuCoreBridge::<Dx12Native>::new().status_from_adapter_backend(NativeBackend::Vulkan);

        assert_eq!(
            status.backend_truth.status,
            WgpuBackendTruthStatus::Mismatch
        );
        assert_eq!(status.backend_truth.requested_backend, NativeBackend::Dx12);
        assert_eq!(status.backend_truth.adapter_backend, NativeBackend::Vulkan);
        assert!(!status.exposed_to_ecs);
        assert!(!status.hot_draw_loop_use_allowed);
    }

    #[test]
    fn core_bridge_snapshot_keeps_identity_redacted_and_version_pinned() {
        let identity = WgpuCoreAdapterIdentity {
            name_hash: stable_message_hash("local-gpu-name"),
            name_len: "local-gpu-name".len() as u32,
            vendor: 0x1414,
            device: 0x008c,
            device_type: WgpuAdapterDeviceType::DiscreteGpu,
            backend: NativeBackend::Vulkan,
        };
        let validation = WgpuCoreValidationReport::new(
            WgpuCoreValidationStatus::Enabled,
            WgpuCoreHookState::default(),
        );
        let snapshot = WgpuCoreBridge::<VulkanNative>::new().snapshot_from_parts(
            identity,
            WgpuFeatureSummary::default(),
            WgpuLimitSummary::default(),
            WgpuCoreDeviceState::default(),
            validation,
        );

        assert_eq!(snapshot.schema_version, WGPU_CORE_BRIDGE_SCHEMA_VERSION);
        assert_eq!(
            snapshot.status.backend_truth.status,
            WgpuBackendTruthStatus::MatchesRequestedBackend
        );
        assert_eq!(
            snapshot.compatibility.expected_wgpu_core_version,
            WGPU_CORE_EXPECTED_VERSION
        );
        assert!(!snapshot.compatibility.private_internals_used);
        assert_eq!(snapshot.adapter_identity.name_hash, identity.name_hash);
    }

    #[test]
    fn core_error_records_store_redacted_message_stats_only() {
        let record = WgpuCoreValidationFailureRecord::from_message(
            WgpuCoreValidationFailureKind::Validation,
            "resource label scene.color failed validation",
        );

        assert_eq!(record.kind, WgpuCoreValidationFailureKind::Validation);
        assert_eq!(
            record.message_hash,
            stable_message_hash("resource label scene.color failed validation")
        );
        assert_eq!(
            record.message_len,
            "resource label scene.color failed validation".len() as u32
        );
    }

    #[test]
    fn core_hook_state_counts_validation_and_device_lost_by_reason() {
        let mut state = WgpuCoreHookState::default();

        state.record_validation_failure(WgpuCoreValidationFailureRecord::from_message(
            WgpuCoreValidationFailureKind::OutOfMemory,
            OUT_OF_MEMORY_MESSAGE,
        ));
        state.record_device_lost(WgpuCoreDeviceLostRecord::from_wgpu(
            ::wgpu::DeviceLostReason::Destroyed,
            "device destroyed by shutdown",
        ));

        assert_eq!(state.out_of_memory_count, 1);
        assert_eq!(state.device_lost_count, 1);
        assert_eq!(
            state.latest_device_lost.map(|record| record.reason),
            Some(WgpuCoreDeviceLostReason::Destroyed)
        );
    }

    #[test]
    fn core_bridge_is_zero_sized_static_bridge_state() {
        assert_eq!(core::mem::size_of::<WgpuCoreBridge<Dx12Native>>(), 0);
    }
}
