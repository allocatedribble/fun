use core::marker::PhantomData;

use crate::backend::{
    BackendCapabilityReport, NativeBackend, NativeCommandEncoderAvailability,
    WgpuHalNativeHandleSupport,
};

use super::{Dx12Native, MetalNative, VulkanNative, WgpuNativeBackend};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuHalBridgeStatus {
    pub native_handle_support: WgpuHalNativeHandleSupport,
    pub native_command_encoder: NativeCommandEncoderAvailability,
    pub raw_command_list_available: bool,
    pub native_interop_capabilities: NativeInteropCapabilities,
}

#[must_use]
pub const fn hal_bridge_status(report: BackendCapabilityReport) -> WgpuHalBridgeStatus {
    let native_interop_capabilities = NativeInteropCapabilities::from_report(report);
    WgpuHalBridgeStatus {
        native_handle_support: report.wgpu_hal_native_handle_support,
        native_command_encoder: report.command_encoder_availability,
        raw_command_list_available: native_interop_capabilities.native_command_list_available_dx12,
        native_interop_capabilities,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeInteropCapabilityBit {
    NativeDeviceAvailable,
    NativeQueueAvailable,
    NativeCommandEncoderAvailable,
    NativeCommandListAvailableDx12,
    NativeTextureHandleAvailable,
    NativeExternalTextureImportAvailable,
    NativeFenceInteropAvailable,
    NativeDebugMarkerAvailable,
}

impl NativeInteropCapabilityBit {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeDeviceAvailable => "native_device_available",
            Self::NativeQueueAvailable => "native_queue_available",
            Self::NativeCommandEncoderAvailable => "native_command_encoder_available",
            Self::NativeCommandListAvailableDx12 => "native_command_list_available_dx12",
            Self::NativeTextureHandleAvailable => "native_texture_handle_available",
            Self::NativeExternalTextureImportAvailable => {
                "native_external_texture_import_available"
            }
            Self::NativeFenceInteropAvailable => "native_fence_interop_available",
            Self::NativeDebugMarkerAvailable => "native_debug_marker_available",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeInteropCapabilities {
    pub native_device_available: bool,
    pub native_queue_available: bool,
    pub native_command_encoder_available: bool,
    pub native_command_list_available_dx12: bool,
    pub native_texture_handle_available: bool,
    pub native_external_texture_import_available: bool,
    pub native_fence_interop_available: bool,
    pub native_debug_marker_available: bool,
}

impl NativeInteropCapabilities {
    pub const NONE: Self = Self {
        native_device_available: false,
        native_queue_available: false,
        native_command_encoder_available: false,
        native_command_list_available_dx12: false,
        native_texture_handle_available: false,
        native_external_texture_import_available: false,
        native_fence_interop_available: false,
        native_debug_marker_available: false,
    };

    pub const WGPU_HAL_PARTIAL: Self = Self {
        native_device_available: true,
        native_queue_available: true,
        native_command_encoder_available: false,
        native_command_list_available_dx12: false,
        native_texture_handle_available: true,
        native_external_texture_import_available: false,
        native_fence_interop_available: false,
        native_debug_marker_available: false,
    };

    #[must_use]
    pub const fn from_report(report: BackendCapabilityReport) -> Self {
        let hal_handles_available = matches!(
            report.wgpu_hal_native_handle_support,
            WgpuHalNativeHandleSupport::Available | WgpuHalNativeHandleSupport::Partial
        );
        let command_encoder_available = matches!(
            report.command_encoder_availability,
            NativeCommandEncoderAvailability::Available
        );

        Self {
            native_device_available: hal_handles_available,
            native_queue_available: hal_handles_available,
            native_command_encoder_available: command_encoder_available,
            native_command_list_available_dx12: command_encoder_available
                && matches!(report.actual_native_backend, NativeBackend::Dx12),
            native_texture_handle_available: hal_handles_available,
            native_external_texture_import_available: false,
            native_fence_interop_available: false,
            native_debug_marker_available: command_encoder_available,
        }
    }

    #[must_use]
    pub const fn has(self, capability: NativeInteropCapabilityBit) -> bool {
        match capability {
            NativeInteropCapabilityBit::NativeDeviceAvailable => self.native_device_available,
            NativeInteropCapabilityBit::NativeQueueAvailable => self.native_queue_available,
            NativeInteropCapabilityBit::NativeCommandEncoderAvailable => {
                self.native_command_encoder_available
            }
            NativeInteropCapabilityBit::NativeCommandListAvailableDx12 => {
                self.native_command_list_available_dx12
            }
            NativeInteropCapabilityBit::NativeTextureHandleAvailable => {
                self.native_texture_handle_available
            }
            NativeInteropCapabilityBit::NativeExternalTextureImportAvailable => {
                self.native_external_texture_import_available
            }
            NativeInteropCapabilityBit::NativeFenceInteropAvailable => {
                self.native_fence_interop_available
            }
            NativeInteropCapabilityBit::NativeDebugMarkerAvailable => {
                self.native_debug_marker_available
            }
        }
    }

    pub fn require(
        self,
        capability: NativeInteropCapabilityBit,
        requested_backend: NativeBackend,
        actual_backend: NativeBackend,
        use_case: NativeInteropUseCase,
    ) -> Result<(), HalInteropFailure> {
        if self.has(capability) {
            Ok(())
        } else {
            Err(HalInteropFailure::for_capability(
                capability,
                requested_backend,
                actual_backend,
                use_case,
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeInteropUseCase {
    NativeUiD3d11On12Copy,
    DlssStreamlineNgx,
    VendorSdk,
    PixNativeMarkers,
    ExternalTextureImportExport,
    ArbitraryGameplaySystem,
    BypassFrameGraphResourceStates,
    HiddenCommandSubmission,
    UndeclaredGraphResourceWrite,
}

impl NativeInteropUseCase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeUiD3d11On12Copy => "native_ui_d3d11on12_copy",
            Self::DlssStreamlineNgx => "dlss_streamline_ngx",
            Self::VendorSdk => "vendor_sdk",
            Self::PixNativeMarkers => "pix_native_markers",
            Self::ExternalTextureImportExport => "external_texture_import_export",
            Self::ArbitraryGameplaySystem => "arbitrary_gameplay_system",
            Self::BypassFrameGraphResourceStates => "bypass_frame_graph_resource_states",
            Self::HiddenCommandSubmission => "hidden_command_submission",
            Self::UndeclaredGraphResourceWrite => "undeclared_graph_resource_write",
        }
    }

    #[must_use]
    pub const fn is_allowed(self) -> bool {
        matches!(
            self,
            Self::NativeUiD3d11On12Copy
                | Self::DlssStreamlineNgx
                | Self::VendorSdk
                | Self::PixNativeMarkers
                | Self::ExternalTextureImportExport
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeInteropPolicyDecision {
    Allowed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeInteropPolicy;

impl NativeInteropPolicy {
    #[must_use]
    pub const fn decision(use_case: NativeInteropUseCase) -> NativeInteropPolicyDecision {
        if use_case.is_allowed() {
            NativeInteropPolicyDecision::Allowed
        } else {
            NativeInteropPolicyDecision::Rejected
        }
    }

    pub fn ensure_use_case_allowed(
        use_case: NativeInteropUseCase,
        requested_backend: NativeBackend,
        actual_backend: NativeBackend,
    ) -> Result<(), HalInteropFailure> {
        match Self::decision(use_case) {
            NativeInteropPolicyDecision::Allowed => Ok(()),
            NativeInteropPolicyDecision::Rejected => Err(HalInteropFailure::new(
                HalInteropFailureReason::UnsafeInteropRejected,
                requested_backend,
                actual_backend,
                None,
                use_case,
            )),
        }
    }

    pub fn ensure_requirements(
        use_case: NativeInteropUseCase,
        requested_backend: NativeBackend,
        actual_backend: NativeBackend,
        capabilities: NativeInteropCapabilities,
    ) -> Result<(), HalInteropFailure> {
        Self::ensure_use_case_allowed(use_case, requested_backend, actual_backend)?;
        match use_case {
            NativeInteropUseCase::NativeUiD3d11On12Copy => {
                capabilities.require(
                    NativeInteropCapabilityBit::NativeDeviceAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeQueueAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeTextureHandleAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeExternalTextureImportAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )
            }
            NativeInteropUseCase::DlssStreamlineNgx => {
                capabilities.require(
                    NativeInteropCapabilityBit::NativeDeviceAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeQueueAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                if matches!(actual_backend, NativeBackend::Dx12) {
                    capabilities.require(
                        NativeInteropCapabilityBit::NativeCommandListAvailableDx12,
                        NativeBackend::Dx12,
                        actual_backend,
                        use_case,
                    )
                } else {
                    capabilities.require(
                        NativeInteropCapabilityBit::NativeCommandEncoderAvailable,
                        requested_backend,
                        actual_backend,
                        use_case,
                    )
                }
            }
            NativeInteropUseCase::VendorSdk => {
                capabilities.require(
                    NativeInteropCapabilityBit::NativeDeviceAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeCommandEncoderAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )
            }
            NativeInteropUseCase::PixNativeMarkers => {
                capabilities.require(
                    NativeInteropCapabilityBit::NativeCommandEncoderAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeDebugMarkerAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )
            }
            NativeInteropUseCase::ExternalTextureImportExport => {
                capabilities.require(
                    NativeInteropCapabilityBit::NativeDeviceAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeTextureHandleAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )?;
                capabilities.require(
                    NativeInteropCapabilityBit::NativeExternalTextureImportAvailable,
                    requested_backend,
                    actual_backend,
                    use_case,
                )
            }
            NativeInteropUseCase::ArbitraryGameplaySystem
            | NativeInteropUseCase::BypassFrameGraphResourceStates
            | NativeInteropUseCase::HiddenCommandSubmission
            | NativeInteropUseCase::UndeclaredGraphResourceWrite => {
                Self::ensure_use_case_allowed(use_case, requested_backend, actual_backend)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HalInteropFailureReason {
    WrongActualBackend,
    HalAccessUnavailable,
    NativeDeviceUnavailable,
    NativeQueueUnavailable,
    NativeCommandEncoderUnavailable,
    NativeCommandListUnavailable,
    NativeTextureHandleUnavailable,
    InteropScopeExpired,
    InteropFeatureNotEnabled,
    UnsafeInteropRejected,
}

impl HalInteropFailureReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WrongActualBackend => "wrong_actual_backend",
            Self::HalAccessUnavailable => "hal_access_unavailable",
            Self::NativeDeviceUnavailable => "native_device_unavailable",
            Self::NativeQueueUnavailable => "native_queue_unavailable",
            Self::NativeCommandEncoderUnavailable => "native_command_encoder_unavailable",
            Self::NativeCommandListUnavailable => "native_command_list_unavailable",
            Self::NativeTextureHandleUnavailable => "native_texture_handle_unavailable",
            Self::InteropScopeExpired => "interop_scope_expired",
            Self::InteropFeatureNotEnabled => "interop_feature_not_enabled",
            Self::UnsafeInteropRejected => "unsafe_interop_rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HalInteropFailure {
    pub reason: HalInteropFailureReason,
    pub requested_backend: NativeBackend,
    pub actual_backend: NativeBackend,
    pub capability: Option<NativeInteropCapabilityBit>,
    pub use_case: NativeInteropUseCase,
}

impl HalInteropFailure {
    #[must_use]
    pub const fn new(
        reason: HalInteropFailureReason,
        requested_backend: NativeBackend,
        actual_backend: NativeBackend,
        capability: Option<NativeInteropCapabilityBit>,
        use_case: NativeInteropUseCase,
    ) -> Self {
        Self {
            reason,
            requested_backend,
            actual_backend,
            capability,
            use_case,
        }
    }

    #[must_use]
    pub const fn for_capability(
        capability: NativeInteropCapabilityBit,
        requested_backend: NativeBackend,
        actual_backend: NativeBackend,
        use_case: NativeInteropUseCase,
    ) -> Self {
        Self::new(
            failure_reason_for_capability(capability),
            requested_backend,
            actual_backend,
            Some(capability),
            use_case,
        )
    }
}

#[must_use]
pub const fn failure_reason_for_capability(
    capability: NativeInteropCapabilityBit,
) -> HalInteropFailureReason {
    match capability {
        NativeInteropCapabilityBit::NativeDeviceAvailable => {
            HalInteropFailureReason::NativeDeviceUnavailable
        }
        NativeInteropCapabilityBit::NativeQueueAvailable => {
            HalInteropFailureReason::NativeQueueUnavailable
        }
        NativeInteropCapabilityBit::NativeCommandEncoderAvailable => {
            HalInteropFailureReason::NativeCommandEncoderUnavailable
        }
        NativeInteropCapabilityBit::NativeCommandListAvailableDx12 => {
            HalInteropFailureReason::NativeCommandListUnavailable
        }
        NativeInteropCapabilityBit::NativeTextureHandleAvailable => {
            HalInteropFailureReason::NativeTextureHandleUnavailable
        }
        NativeInteropCapabilityBit::NativeExternalTextureImportAvailable
        | NativeInteropCapabilityBit::NativeFenceInteropAvailable
        | NativeInteropCapabilityBit::NativeDebugMarkerAvailable => {
            HalInteropFailureReason::InteropFeatureNotEnabled
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ScopedNativeHandleBits {
    native_command_encoder: Option<u64>,
    dx12_command_list: Option<u64>,
}

impl ScopedNativeHandleBits {
    const NONE: Self = Self {
        native_command_encoder: None,
        dx12_command_list: None,
    };
}

#[derive(Debug)]
pub struct HalInteropScope<'scope, B: WgpuNativeBackend> {
    pub actual_backend: NativeBackend,
    pub capabilities: NativeInteropCapabilities,
    handles: ScopedNativeHandleBits,
    active: bool,
    _scope: PhantomData<&'scope mut ()>,
    _backend: PhantomData<fn() -> B>,
}

impl<'scope, B: WgpuNativeBackend> HalInteropScope<'scope, B> {
    #[must_use]
    pub const fn pending_from_report(report: BackendCapabilityReport) -> Self {
        Self {
            actual_backend: report.actual_native_backend,
            capabilities: NativeInteropCapabilities::from_report(report),
            handles: ScopedNativeHandleBits::NONE,
            active: true,
            _scope: PhantomData,
            _backend: PhantomData,
        }
    }

    #[must_use]
    pub const fn pending(
        actual_backend: NativeBackend,
        capabilities: NativeInteropCapabilities,
    ) -> Self {
        Self {
            actual_backend,
            capabilities,
            handles: ScopedNativeHandleBits::NONE,
            active: true,
            _scope: PhantomData,
            _backend: PhantomData,
        }
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_scoped_handles(
        actual_backend: NativeBackend,
        capabilities: NativeInteropCapabilities,
        native_command_encoder: Option<u64>,
        dx12_command_list: Option<u64>,
    ) -> Self {
        Self {
            actual_backend,
            capabilities,
            handles: ScopedNativeHandleBits {
                native_command_encoder,
                dx12_command_list,
            },
            active: true,
            _scope: PhantomData,
            _backend: PhantomData,
        }
    }

    #[cfg(test)]
    pub(crate) fn expire_for_test(&mut self) {
        self.active = false;
    }

    fn ensure_active(&self, use_case: NativeInteropUseCase) -> Result<(), HalInteropFailure> {
        if self.active {
            Ok(())
        } else {
            Err(HalInteropFailure::new(
                HalInteropFailureReason::InteropScopeExpired,
                B::NATIVE_BACKEND,
                self.actual_backend,
                None,
                use_case,
            ))
        }
    }

    fn ensure_expected_backend(
        &self,
        expected_backend: NativeBackend,
        use_case: NativeInteropUseCase,
    ) -> Result<(), HalInteropFailure> {
        if self.actual_backend == expected_backend {
            Ok(())
        } else {
            Err(HalInteropFailure::new(
                HalInteropFailureReason::WrongActualBackend,
                expected_backend,
                self.actual_backend,
                None,
                use_case,
            ))
        }
    }
}

#[derive(Debug)]
pub struct NativeDeviceHandleKind;
#[derive(Debug)]
pub struct NativeQueueHandleKind;
#[derive(Debug)]
pub struct NativeCommandEncoderHandleKind;
#[derive(Debug)]
pub struct Dx12CommandListHandleKind;
#[derive(Debug)]
pub struct NativeTextureHandleKind;
#[derive(Debug)]
pub struct NativeFenceHandleKind;
#[derive(Debug)]
pub struct NativeDebugMarkerHandleKind;

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ScopedNativeHandle<'scope, B: WgpuNativeBackend, Kind> {
    raw: u64,
    _scope: PhantomData<&'scope mut ()>,
    _backend: PhantomData<fn() -> B>,
    _kind: PhantomData<fn() -> Kind>,
}

impl<'scope, B: WgpuNativeBackend, Kind> ScopedNativeHandle<'scope, B, Kind> {
    #[must_use]
    const fn new(raw: u64) -> Self {
        Self {
            raw,
            _scope: PhantomData,
            _backend: PhantomData,
            _kind: PhantomData,
        }
    }

    #[must_use]
    pub const fn raw(&self) -> u64 {
        self.raw
    }
}

pub type ScopedNativeDevice<'scope, B> = ScopedNativeHandle<'scope, B, NativeDeviceHandleKind>;
pub type ScopedNativeQueue<'scope, B> = ScopedNativeHandle<'scope, B, NativeQueueHandleKind>;
pub type ScopedNativeCommandEncoder<'scope, B> =
    ScopedNativeHandle<'scope, B, NativeCommandEncoderHandleKind>;
pub type ScopedDx12CommandList<'scope, B> =
    ScopedNativeHandle<'scope, B, Dx12CommandListHandleKind>;
pub type ScopedNativeTexture<'scope, B> = ScopedNativeHandle<'scope, B, NativeTextureHandleKind>;
pub type ScopedNativeFence<'scope, B> = ScopedNativeHandle<'scope, B, NativeFenceHandleKind>;
pub type ScopedNativeDebugMarker<'scope, B> =
    ScopedNativeHandle<'scope, B, NativeDebugMarkerHandleKind>;

pub trait HalInteropBridge: Sized {
    type Native: WgpuNativeBackend;

    #[must_use]
    fn capability_report() -> BackendCapabilityReport {
        <Self::Native as WgpuNativeBackend>::CAPABILITY_REPORT
    }

    #[must_use]
    fn capabilities() -> NativeInteropCapabilities {
        NativeInteropCapabilities::from_report(Self::capability_report())
    }

    fn with_native_command_encoder<R: 'static>(
        scope: &mut HalInteropScope<'_, Self::Native>,
        use_case: NativeInteropUseCase,
        callback: impl FnOnce(ScopedNativeCommandEncoder<'_, Self::Native>) -> R,
    ) -> Result<R, HalInteropFailure> {
        scope.ensure_active(use_case)?;
        scope.ensure_expected_backend(Self::Native::NATIVE_BACKEND, use_case)?;
        NativeInteropPolicy::ensure_requirements(
            use_case,
            Self::Native::NATIVE_BACKEND,
            scope.actual_backend,
            scope.capabilities,
        )?;
        scope.capabilities.require(
            NativeInteropCapabilityBit::NativeCommandEncoderAvailable,
            Self::Native::NATIVE_BACKEND,
            scope.actual_backend,
            use_case,
        )?;
        let Some(raw) = scope.handles.native_command_encoder else {
            return Err(HalInteropFailure::for_capability(
                NativeInteropCapabilityBit::NativeCommandEncoderAvailable,
                Self::Native::NATIVE_BACKEND,
                scope.actual_backend,
                use_case,
            ));
        };

        Ok(callback(ScopedNativeCommandEncoder::new(raw)))
    }

    fn with_dx12_command_list<R: 'static>(
        scope: &mut HalInteropScope<'_, Self::Native>,
        use_case: NativeInteropUseCase,
        callback: impl FnOnce(ScopedDx12CommandList<'_, Self::Native>) -> R,
    ) -> Result<R, HalInteropFailure> {
        scope.ensure_active(use_case)?;
        scope.ensure_expected_backend(NativeBackend::Dx12, use_case)?;
        NativeInteropPolicy::ensure_requirements(
            use_case,
            NativeBackend::Dx12,
            scope.actual_backend,
            scope.capabilities,
        )?;
        scope.capabilities.require(
            NativeInteropCapabilityBit::NativeCommandListAvailableDx12,
            NativeBackend::Dx12,
            scope.actual_backend,
            use_case,
        )?;
        let Some(raw) = scope.handles.dx12_command_list else {
            return Err(HalInteropFailure::for_capability(
                NativeInteropCapabilityBit::NativeCommandListAvailableDx12,
                NativeBackend::Dx12,
                scope.actual_backend,
                use_case,
            ));
        };

        Ok(callback(ScopedDx12CommandList::new(raw)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12HalInterop;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VulkanHalInterop;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetalHalInterop;

impl HalInteropBridge for Dx12HalInterop {
    type Native = Dx12Native;
}

impl HalInteropBridge for VulkanHalInterop {
    type Native = VulkanNative;
}

impl HalInteropBridge for MetalHalInterop {
    type Native = MetalNative;
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CAPABILITIES: NativeInteropCapabilities = NativeInteropCapabilities {
        native_device_available: true,
        native_queue_available: true,
        native_command_encoder_available: true,
        native_command_list_available_dx12: true,
        native_texture_handle_available: true,
        native_external_texture_import_available: true,
        native_fence_interop_available: true,
        native_debug_marker_available: true,
    };

    #[test]
    fn dx12_wgpu_hal_status_reports_fail_closed_command_list() {
        let status = hal_bridge_status(BackendCapabilityReport::WGPU_DX12);

        assert_eq!(
            status.native_handle_support,
            WgpuHalNativeHandleSupport::Partial
        );
        assert_eq!(
            status.native_command_encoder,
            NativeCommandEncoderAvailability::BridgeDoesNotExpose
        );
        assert!(status.native_interop_capabilities.native_device_available);
        assert!(status.native_interop_capabilities.native_queue_available);
        assert!(
            status
                .native_interop_capabilities
                .native_texture_handle_available
        );
        assert!(!status.raw_command_list_available);
        assert!(
            !status
                .native_interop_capabilities
                .native_command_list_available_dx12
        );
    }

    #[test]
    fn dx12_command_list_callback_fails_loudly_when_wgpu_does_not_expose_it() {
        let mut scope =
            HalInteropScope::<Dx12Native>::pending_from_report(BackendCapabilityReport::WGPU_DX12);
        let mut callback_ran = false;

        let err = Dx12HalInterop::with_dx12_command_list(
            &mut scope,
            NativeInteropUseCase::DlssStreamlineNgx,
            |_| {
                callback_ran = true;
            },
        )
        .expect_err("current wgpu-hal bridge must not pretend to expose ID3D12GraphicsCommandList");

        assert!(!callback_ran);
        assert_eq!(
            err.reason,
            HalInteropFailureReason::NativeCommandListUnavailable
        );
        assert_eq!(err.requested_backend, NativeBackend::Dx12);
        assert_eq!(err.actual_backend, NativeBackend::Dx12);
        assert_eq!(
            err.capability,
            Some(NativeInteropCapabilityBit::NativeCommandListAvailableDx12)
        );
    }

    #[test]
    fn dx12_command_list_callback_rejects_wrong_actual_backend() {
        let mut scope = HalInteropScope::<VulkanNative>::pending_from_report(
            BackendCapabilityReport::WGPU_VULKAN,
        );

        let err = VulkanHalInterop::with_dx12_command_list(
            &mut scope,
            NativeInteropUseCase::DlssStreamlineNgx,
            |_| (),
        )
        .expect_err("DX12 command-list access must assert actual backend truth");

        assert_eq!(err.reason, HalInteropFailureReason::WrongActualBackend);
        assert_eq!(err.requested_backend, NativeBackend::Dx12);
        assert_eq!(err.actual_backend, NativeBackend::Vulkan);
    }

    #[test]
    fn native_interop_policy_rejects_hidden_or_gameplay_bypass_uses() {
        for use_case in [
            NativeInteropUseCase::ArbitraryGameplaySystem,
            NativeInteropUseCase::BypassFrameGraphResourceStates,
            NativeInteropUseCase::HiddenCommandSubmission,
            NativeInteropUseCase::UndeclaredGraphResourceWrite,
        ] {
            let err = NativeInteropPolicy::ensure_requirements(
                use_case,
                NativeBackend::Dx12,
                NativeBackend::Dx12,
                ALL_CAPABILITIES,
            )
            .expect_err("forbidden native interop use case must be rejected");

            assert_eq!(err.reason, HalInteropFailureReason::UnsafeInteropRejected);
            assert_eq!(err.use_case, use_case);
        }
    }

    #[test]
    fn scoped_callback_receives_typed_command_list_only_inside_active_scope() {
        let mut scope = HalInteropScope::<Dx12Native>::with_scoped_handles(
            NativeBackend::Dx12,
            ALL_CAPABILITIES,
            Some(0x10),
            Some(0x20),
        );

        let raw = Dx12HalInterop::with_dx12_command_list(
            &mut scope,
            NativeInteropUseCase::DlssStreamlineNgx,
            |command_list| command_list.raw(),
        )
        .expect("test scope supplies a sanctioned DX12 command list");

        assert_eq!(raw, 0x20);
    }

    #[test]
    fn expired_interop_scope_blocks_callbacks_before_capability_checks() {
        let mut scope = HalInteropScope::<Dx12Native>::with_scoped_handles(
            NativeBackend::Dx12,
            ALL_CAPABILITIES,
            Some(0x10),
            Some(0x20),
        );
        scope.expire_for_test();

        let err = Dx12HalInterop::with_native_command_encoder(
            &mut scope,
            NativeInteropUseCase::VendorSdk,
            |_| (),
        )
        .expect_err("expired scopes must not expose native handles");

        assert_eq!(err.reason, HalInteropFailureReason::InteropScopeExpired);
    }
}
