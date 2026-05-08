use crate::backend::{
    BackendCapabilityReport, NativeCommandEncoderAvailability, WgpuHalNativeHandleSupport,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuHalBridgeStatus {
    pub native_handle_support: WgpuHalNativeHandleSupport,
    pub native_command_encoder: NativeCommandEncoderAvailability,
    pub raw_command_list_available: bool,
}

#[must_use]
pub const fn hal_bridge_status(report: BackendCapabilityReport) -> WgpuHalBridgeStatus {
    WgpuHalBridgeStatus {
        native_handle_support: report.wgpu_hal_native_handle_support,
        native_command_encoder: report.command_encoder_availability,
        raw_command_list_available: matches!(
            report.command_encoder_availability,
            NativeCommandEncoderAvailability::Available
        ),
    }
}
