use crate::backend::NativeBackend;

use super::composite::{UiCompositeColorPolicy, UiCompositePassDiagnostics};
use super::dx12_transport::Dx12NativeUiTransportConfig;
use super::native_ui::{
    RendererNativeUiCompositorDiagnostics, RendererNativeUiFailClosedReason,
    RendererNativeUiImportSyncStatus, RendererNativeUiTransportMode,
};
use super::producer::{NativeUiProducerEvents, NativeUiProducerSubmissionState};

pub const UI_TRANSPORT_TELEMETRY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiTransportTelemetryMode {
    Disabled,
    Dx12GpuD3d11On12,
    Dx12GpuDirect,
    VulkanExternalMemory,
    CpuUploadDebug,
}

impl UiTransportTelemetryMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Dx12GpuD3d11On12 => "dx12_gpu_d3d11on12",
            Self::Dx12GpuDirect => "dx12_gpu_direct",
            Self::VulkanExternalMemory => "vulkan_external_memory",
            Self::CpuUploadDebug => "cpu_upload_debug",
        }
    }

    #[must_use]
    pub const fn is_gpu_transport(self) -> bool {
        matches!(
            self,
            Self::Dx12GpuD3d11On12 | Self::Dx12GpuDirect | Self::VulkanExternalMemory
        )
    }

    #[must_use]
    pub const fn is_default_allowed_for_dx12_production(self) -> bool {
        // CPU upload is not allowed as a default Windows production path.
        // Only GPU shared-texture transports may be the default.
        matches!(self, Self::Dx12GpuD3d11On12 | Self::Dx12GpuDirect)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiCpuUploadDebugMode {
    pub enabled: bool,
    pub upload_bytes: u64,
    pub fallback_reason: Option<RendererNativeUiFailClosedReason>,
}

impl UiCpuUploadDebugMode {
    pub const DISABLED: Self = Self {
        enabled: false,
        upload_bytes: 0,
        fallback_reason: None,
    };

    #[must_use]
    pub const fn enabled_for_diagnostics(
        upload_bytes: u64,
        fallback_reason: RendererNativeUiFailClosedReason,
    ) -> Self {
        Self {
            enabled: true,
            upload_bytes,
            fallback_reason: Some(fallback_reason),
        }
    }

    pub const fn validate_for_production(
        self,
        native_backend: NativeBackend,
    ) -> Result<(), UiTransportTelemetryError> {
        if self.enabled && matches!(native_backend, NativeBackend::Dx12) {
            Err(UiTransportTelemetryError::CpuUploadCannotBeDefaultOnDx12)
        } else {
            Ok(())
        }
    }
}

impl Default for UiCpuUploadDebugMode {
    fn default() -> Self {
        Self::DISABLED
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiTransportTelemetryError {
    CpuUploadCannotBeDefaultOnDx12,
    DisabledOnDx12Production,
    GpuTransportUnavailable,
    SchemaMismatch,
}

impl UiTransportTelemetryError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CpuUploadCannotBeDefaultOnDx12 => "cpu_upload_cannot_be_default_on_dx12",
            Self::DisabledOnDx12Production => "disabled_on_dx12_production",
            Self::GpuTransportUnavailable => "gpu_transport_unavailable",
            Self::SchemaMismatch => "schema_mismatch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiTransportTelemetry {
    pub schema_version: u16,
    pub mode: UiTransportTelemetryMode,
    pub native_backend: NativeBackend,
    pub gpu_transport_default: bool,
    pub cpu_upload_debug: UiCpuUploadDebugMode,
    pub last_sync_status: RendererNativeUiImportSyncStatus,
    pub last_fail_closed_reason: Option<RendererNativeUiFailClosedReason>,
    pub callback_to_import_latency_ns: u64,
    pub import_copy_duration_ns: u64,
    pub ui_composite_pass_duration_ns: u64,
    pub copied_bytes: u64,
    pub fail_closed_count: u64,
    pub cpu_fallback_attempts: u64,
    pub dropped_ui_frames: u64,
    pub stale_ui_frame_age_ns: u64,
    pub imported_frame_count: u64,
    pub producer_submitted_frames: u64,
    pub producer_replaced_frames: u64,
    pub producer_aged_frames: u64,
    pub producer_rejected_submissions: u64,
    pub last_producer_submission_state: Option<NativeUiProducerSubmissionState>,
    pub composite_layers_composited: u32,
    pub composite_debug_border_layers: u32,
    pub composite_color_conversion_count: u32,
    pub composite_uses_premultiplied_alpha: bool,
    pub output_color_policy: Option<UiCompositeColorPolicy>,
}

impl UiTransportTelemetry {
    #[must_use]
    pub fn from_inputs(
        mode: UiTransportTelemetryMode,
        native_backend: NativeBackend,
        cpu_upload_debug: UiCpuUploadDebugMode,
        compositor: &RendererNativeUiCompositorDiagnostics,
        producer: &NativeUiProducerEvents,
        composite: UiCompositePassDiagnostics,
    ) -> Self {
        Self {
            schema_version: UI_TRANSPORT_TELEMETRY_SCHEMA_VERSION,
            mode,
            native_backend,
            gpu_transport_default: matches!(native_backend, NativeBackend::Dx12)
                && mode.is_default_allowed_for_dx12_production(),
            cpu_upload_debug,
            last_sync_status: compositor.last_sync_status,
            last_fail_closed_reason: compositor.last_fail_closed_reason,
            callback_to_import_latency_ns: compositor.callback_to_import_latency_ns,
            import_copy_duration_ns: compositor.import_copy_duration_ns,
            ui_composite_pass_duration_ns: compositor.ui_composite_pass_duration_ns,
            copied_bytes: compositor.copied_bytes,
            fail_closed_count: compositor.fail_closed_count,
            cpu_fallback_attempts: compositor.cpu_fallback_attempts,
            dropped_ui_frames: compositor.dropped_ui_frames,
            stale_ui_frame_age_ns: compositor.stale_ui_frame_age_ns,
            imported_frame_count: compositor.imported_frame_count,
            producer_submitted_frames: producer.submitted_frames(),
            producer_replaced_frames: producer.replaced_frames(),
            producer_aged_frames: producer.aged_frames(),
            producer_rejected_submissions: producer.rejected_submissions(),
            last_producer_submission_state: producer.last_submission_state(),
            composite_layers_composited: composite.layers_composited,
            composite_debug_border_layers: composite.debug_border_layers,
            composite_color_conversion_count: composite.color_conversion_count,
            composite_uses_premultiplied_alpha: composite.uses_premultiplied_alpha,
            output_color_policy: composite.output_policy,
        }
    }

    pub const fn validate_dx12_production(self) -> Result<(), UiTransportTelemetryError> {
        if !matches!(self.native_backend, NativeBackend::Dx12) {
            return Ok(());
        }
        if matches!(self.mode, UiTransportTelemetryMode::CpuUploadDebug)
            && !self.cpu_upload_debug.enabled
        {
            return Err(UiTransportTelemetryError::CpuUploadCannotBeDefaultOnDx12);
        }
        if self.cpu_upload_debug.enabled && self.gpu_transport_default {
            return Err(UiTransportTelemetryError::CpuUploadCannotBeDefaultOnDx12);
        }
        if !self.mode.is_gpu_transport() && !self.cpu_upload_debug.enabled {
            return Err(UiTransportTelemetryError::DisabledOnDx12Production);
        }
        Ok(())
    }

    #[must_use]
    pub const fn cpu_upload_debug_disclosed(self) -> bool {
        self.cpu_upload_debug.enabled
            && self.cpu_upload_debug.fallback_reason.is_some()
            && self.cpu_upload_debug.upload_bytes > 0
            && matches!(self.mode, UiTransportTelemetryMode::CpuUploadDebug)
    }
}

#[must_use]
pub fn telemetry_mode_from_compositor(
    transport: RendererNativeUiTransportMode,
    bridge_config: Option<Dx12NativeUiTransportConfig>,
) -> UiTransportTelemetryMode {
    match transport {
        RendererNativeUiTransportMode::Disabled => UiTransportTelemetryMode::Disabled,
        RendererNativeUiTransportMode::CpuOnPaint => UiTransportTelemetryMode::CpuUploadDebug,
        RendererNativeUiTransportMode::D3d11On12SharedTexture => match bridge_config {
            Some(config)
                if matches!(
                    config.bridge_mode,
                    super::dx12_transport::Dx12NativeUiBridgeMode::D3d12Direct
                ) =>
            {
                UiTransportTelemetryMode::Dx12GpuDirect
            }
            _ => UiTransportTelemetryMode::Dx12GpuD3d11On12,
        },
        RendererNativeUiTransportMode::VulkanExternalMemory => {
            UiTransportTelemetryMode::VulkanExternalMemory
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::composite::{
        UiCompositeContract, UiCompositePassDiagnostics, UiCompositePlan,
    };
    use super::super::dx12_transport::Dx12NativeUiBridgeMode;
    use super::super::native_ui::{RendererNativeUiAlphaMode, RendererNativeUiDirtyRect};
    use super::super::producer::{
        GpuSubmissionDirtyRects, GpuSubmissionTimings, NativeUiProducerSubmission,
        build_gpu_submission,
    };
    use super::*;
    use crate::component_api::RenderStableId;

    fn empty_compositor_diag() -> RendererNativeUiCompositorDiagnostics {
        RendererNativeUiCompositorDiagnostics::default()
    }

    fn empty_composite_plan() -> UiCompositePlan {
        UiCompositePlan::empty(
            UiCompositeContract::default(),
            UiCompositeColorPolicy::SdrLinearScene,
        )
    }

    fn submission(producer: u64, frame_id: u64) -> NativeUiProducerSubmission {
        build_gpu_submission(
            RenderStableId::new(producer),
            frame_id,
            super::super::native_ui::RendererNativeUiExtent::new(1280, 720),
            RendererNativeUiAlphaMode::Premultiplied,
            GpuSubmissionTimings {
                callback_timestamp_ns: 1_000,
                import_begin_timestamp_ns: 1_500,
                import_complete_timestamp_ns: 2_000,
                copied_bytes: 1280 * 720 * 4,
            },
            GpuSubmissionDirtyRects {
                count: 1,
                union: Some(RendererNativeUiDirtyRect::new(0, 0, 1280, 720)),
            },
        )
    }

    #[test]
    fn dx12_gpu_transport_is_default_when_d3d11on12_is_active() {
        let mode = telemetry_mode_from_compositor(
            RendererNativeUiTransportMode::D3d11On12SharedTexture,
            Some(Dx12NativeUiTransportConfig::default()),
        );
        assert_eq!(mode, UiTransportTelemetryMode::Dx12GpuD3d11On12);
        assert!(mode.is_gpu_transport());
        assert!(mode.is_default_allowed_for_dx12_production());
    }

    #[test]
    fn cpu_upload_is_never_default_on_dx12_production() {
        let cpu = UiCpuUploadDebugMode::enabled_for_diagnostics(
            1024,
            RendererNativeUiFailClosedReason::CpuOnPaintRuntimeFallback,
        );
        let err = cpu
            .validate_for_production(NativeBackend::Dx12)
            .expect_err("cpu upload must not be default on dx12 production");
        assert_eq!(
            err,
            UiTransportTelemetryError::CpuUploadCannotBeDefaultOnDx12
        );

        let telemetry = UiTransportTelemetry::from_inputs(
            UiTransportTelemetryMode::Dx12GpuD3d11On12,
            NativeBackend::Dx12,
            cpu,
            &empty_compositor_diag(),
            &NativeUiProducerEvents::new(),
            UiCompositePassDiagnostics::from_plan(&empty_composite_plan()),
        );
        assert_eq!(
            telemetry.validate_dx12_production(),
            Err(UiTransportTelemetryError::CpuUploadCannotBeDefaultOnDx12)
        );
    }

    #[test]
    fn cpu_upload_debug_mode_must_disclose_bytes_and_fallback_reason() {
        let cpu = UiCpuUploadDebugMode::enabled_for_diagnostics(
            512,
            RendererNativeUiFailClosedReason::CpuOnPaintRuntimeFallback,
        );
        let telemetry = UiTransportTelemetry {
            schema_version: UI_TRANSPORT_TELEMETRY_SCHEMA_VERSION,
            mode: UiTransportTelemetryMode::CpuUploadDebug,
            native_backend: NativeBackend::Vulkan,
            gpu_transport_default: false,
            cpu_upload_debug: cpu,
            last_sync_status: RendererNativeUiImportSyncStatus::NotImported,
            last_fail_closed_reason: Some(
                RendererNativeUiFailClosedReason::CpuOnPaintRuntimeFallback,
            ),
            callback_to_import_latency_ns: 0,
            import_copy_duration_ns: 0,
            ui_composite_pass_duration_ns: 0,
            copied_bytes: 0,
            fail_closed_count: 0,
            cpu_fallback_attempts: 0,
            dropped_ui_frames: 0,
            stale_ui_frame_age_ns: 0,
            imported_frame_count: 0,
            producer_submitted_frames: 0,
            producer_replaced_frames: 0,
            producer_aged_frames: 0,
            producer_rejected_submissions: 0,
            last_producer_submission_state: None,
            composite_layers_composited: 0,
            composite_debug_border_layers: 0,
            composite_color_conversion_count: 0,
            composite_uses_premultiplied_alpha: false,
            output_color_policy: None,
        };

        assert!(telemetry.cpu_upload_debug_disclosed());
        // Vulkan-backed test build allows cpu upload debug since the rule is dx12 production.
        assert_eq!(telemetry.validate_dx12_production(), Ok(()));
    }

    #[test]
    fn dx12_production_disabled_transport_is_a_loud_failure() {
        let telemetry = UiTransportTelemetry::from_inputs(
            UiTransportTelemetryMode::Disabled,
            NativeBackend::Dx12,
            UiCpuUploadDebugMode::DISABLED,
            &empty_compositor_diag(),
            &NativeUiProducerEvents::new(),
            UiCompositePassDiagnostics::from_plan(&empty_composite_plan()),
        );
        assert_eq!(
            telemetry.validate_dx12_production(),
            Err(UiTransportTelemetryError::DisabledOnDx12Production)
        );
    }

    #[test]
    fn telemetry_aggregates_compositor_and_producer_metrics() {
        let mut producer = NativeUiProducerEvents::new();
        producer.submit(submission(1, 1)).expect("submit");
        producer.submit(submission(1, 2)).expect("submit");
        let mut compositor = empty_compositor_diag();
        compositor.imported_frame_count = 5;
        compositor.copied_bytes = 1024;
        compositor.last_sync_status = RendererNativeUiImportSyncStatus::CopiedIntoRendererTexture;

        let telemetry = UiTransportTelemetry::from_inputs(
            UiTransportTelemetryMode::Dx12GpuD3d11On12,
            NativeBackend::Dx12,
            UiCpuUploadDebugMode::DISABLED,
            &compositor,
            &producer,
            UiCompositePassDiagnostics::from_plan(&empty_composite_plan()),
        );

        assert_eq!(telemetry.imported_frame_count, 5);
        assert_eq!(telemetry.copied_bytes, 1024);
        assert_eq!(telemetry.producer_submitted_frames, 2);
        assert_eq!(telemetry.producer_replaced_frames, 1);
        assert!(telemetry.gpu_transport_default);
        assert_eq!(telemetry.validate_dx12_production(), Ok(()));
    }

    #[test]
    fn d3d12_direct_bridge_reports_direct_dx12_telemetry_mode() {
        let mode = telemetry_mode_from_compositor(
            RendererNativeUiTransportMode::D3d11On12SharedTexture,
            Some(Dx12NativeUiTransportConfig {
                bridge_mode: Dx12NativeUiBridgeMode::D3d12Direct,
                ..Dx12NativeUiTransportConfig::default()
            }),
        );
        assert_eq!(mode, UiTransportTelemetryMode::Dx12GpuDirect);
    }
}
