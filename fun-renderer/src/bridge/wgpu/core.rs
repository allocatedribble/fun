use crate::backend::{BackendCapabilityReport, WgpuCoreValidationStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCoreBridgeStatus {
    pub validation: WgpuCoreValidationStatus,
    pub resource_identity_owned_by_wgpu_core: bool,
    pub exposed_to_ecs: bool,
}

#[must_use]
pub const fn core_bridge_status(report: BackendCapabilityReport) -> WgpuCoreBridgeStatus {
    WgpuCoreBridgeStatus {
        validation: report.wgpu_core_validation,
        resource_identity_owned_by_wgpu_core: true,
        exposed_to_ecs: false,
    }
}
