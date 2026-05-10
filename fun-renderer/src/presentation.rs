use crate::component_api::{HdrOutputSettings, UiColorSpace};
use crate::frame_graph::FrameGraphResourceType;

pub const PRESENTATION_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwapchainColorSpace {
    #[default]
    SrgbNonlinear,
    SrgbLinear,
    Hdr10St2084,
    ScRgbExtendedLinear,
}

impl SwapchainColorSpace {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SrgbNonlinear => "srgb_nonlinear",
            Self::SrgbLinear => "srgb_linear",
            Self::Hdr10St2084 => "hdr10_st2084",
            Self::ScRgbExtendedLinear => "scrgb_extended_linear",
        }
    }

    #[must_use]
    pub const fn is_hdr(self) -> bool {
        matches!(self, Self::Hdr10St2084 | Self::ScRgbExtendedLinear)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneLinearTargetFormat {
    #[default]
    Rgba16Float,
    R11G11B10Float,
}

impl SceneLinearTargetFormat {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rgba16Float => "rgba16_float",
            Self::R11G11B10Float => "r11g11b10_float",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdrOutputState {
    #[default]
    Disabled,
    ScaffoldEnabled,
    Active,
    DisplayDoesNotSupportHdr,
    SwapchainCannotProvideHdr,
}

impl HdrOutputState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ScaffoldEnabled => "scaffold_enabled",
            Self::Active => "active",
            Self::DisplayDoesNotSupportHdr => "display_does_not_support_hdr",
            Self::SwapchainCannotProvideHdr => "swapchain_cannot_provide_hdr",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiCompositionPolicy {
    #[default]
    LinearScene,
    HdrScene,
    Hdr10Output,
}

impl UiCompositionPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinearScene => "linear_scene",
            Self::HdrScene => "hdr_scene",
            Self::Hdr10Output => "hdr10_output",
        }
    }

    #[must_use]
    pub const fn requires_color_conversion_for(self, surface_color_space: UiColorSpace) -> bool {
        !matches!(
            (self, surface_color_space),
            (Self::LinearScene, UiColorSpace::Linear) | (Self::Hdr10Output, UiColorSpace::Hdr10)
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SdrFallbackReason {
    #[default]
    NotInFallback,
    HdrDisabled,
    HdrNotAvailable,
    DisplayDoesNotSupportHdr,
    SwapchainCannotProvideHdr,
    PolicyForcedSdr,
}

impl SdrFallbackReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInFallback => "not_in_fallback",
            Self::HdrDisabled => "hdr_disabled",
            Self::HdrNotAvailable => "hdr_not_available",
            Self::DisplayDoesNotSupportHdr => "display_does_not_support_hdr",
            Self::SwapchainCannotProvideHdr => "swapchain_cannot_provide_hdr",
            Self::PolicyForcedSdr => "policy_forced_sdr",
        }
    }

    #[must_use]
    pub const fn is_fallback(self) -> bool {
        !matches!(self, Self::NotInFallback)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DisplayCapabilities {
    pub hdr_supported: bool,
    pub max_luminance_nits: u16,
    pub paper_white_nits: u16,
}

impl DisplayCapabilities {
    pub const SDR: Self = Self {
        hdr_supported: false,
        max_luminance_nits: 80,
        paper_white_nits: 80,
    };

    pub const HDR1000: Self = Self {
        hdr_supported: true,
        max_luminance_nits: 1_000,
        paper_white_nits: 200,
    };
}

impl Default for DisplayCapabilities {
    fn default() -> Self {
        Self::SDR
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwapchainCapabilities {
    pub hdr10_st2084_available: bool,
    pub scrgb_extended_linear_available: bool,
}

impl SwapchainCapabilities {
    pub const SDR_ONLY: Self = Self {
        hdr10_st2084_available: false,
        scrgb_extended_linear_available: false,
    };

    pub const HDR_FULL: Self = Self {
        hdr10_st2084_available: true,
        scrgb_extended_linear_available: true,
    };
}

impl Default for SwapchainCapabilities {
    fn default() -> Self {
        Self::SDR_ONLY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationPolicy {
    pub schema_version: u16,
    pub swapchain_color_space: SwapchainColorSpace,
    pub scene_linear_target_format: SceneLinearTargetFormat,
    pub hdr_output_state: HdrOutputState,
    pub ui_composition_policy: UiCompositionPolicy,
    pub sdr_fallback: SdrFallbackReason,
    pub final_resource: FrameGraphResourceType,
    pub max_luminance_nits: u16,
    pub paper_white_nits: u16,
}

impl PresentationPolicy {
    pub const PRODUCT_SDR_DEFAULT: Self = Self {
        schema_version: PRESENTATION_SCHEMA_VERSION,
        swapchain_color_space: SwapchainColorSpace::SrgbNonlinear,
        scene_linear_target_format: SceneLinearTargetFormat::Rgba16Float,
        hdr_output_state: HdrOutputState::Disabled,
        ui_composition_policy: UiCompositionPolicy::LinearScene,
        sdr_fallback: SdrFallbackReason::HdrDisabled,
        final_resource: FrameGraphResourceType::FinalComposedOutput,
        max_luminance_nits: 80,
        paper_white_nits: 80,
    };

    #[must_use]
    pub const fn product_default() -> Self {
        Self::PRODUCT_SDR_DEFAULT
    }

    #[must_use]
    pub fn from_camera_settings(
        hdr: HdrOutputSettings,
        display: DisplayCapabilities,
        swapchain: SwapchainCapabilities,
    ) -> Self {
        let want_hdr = hdr.enabled;
        let display_supports_hdr = display.hdr_supported;
        let swapchain_supports_hdr10 = swapchain.hdr10_st2084_available;

        let (
            swapchain_color_space,
            hdr_output_state,
            ui_composition_policy,
            sdr_fallback,
            max_luminance_nits,
            paper_white_nits,
        ) = if !want_hdr {
            (
                SwapchainColorSpace::SrgbNonlinear,
                HdrOutputState::Disabled,
                UiCompositionPolicy::LinearScene,
                SdrFallbackReason::HdrDisabled,
                display.max_luminance_nits,
                display.paper_white_nits,
            )
        } else if !display_supports_hdr {
            (
                SwapchainColorSpace::SrgbNonlinear,
                HdrOutputState::DisplayDoesNotSupportHdr,
                UiCompositionPolicy::LinearScene,
                SdrFallbackReason::DisplayDoesNotSupportHdr,
                display.max_luminance_nits,
                display.paper_white_nits,
            )
        } else if !swapchain_supports_hdr10 {
            (
                SwapchainColorSpace::SrgbNonlinear,
                HdrOutputState::SwapchainCannotProvideHdr,
                UiCompositionPolicy::LinearScene,
                SdrFallbackReason::SwapchainCannotProvideHdr,
                display.max_luminance_nits,
                display.paper_white_nits,
            )
        } else {
            (
                SwapchainColorSpace::Hdr10St2084,
                HdrOutputState::ScaffoldEnabled,
                UiCompositionPolicy::Hdr10Output,
                SdrFallbackReason::NotInFallback,
                hdr.max_nits as u16,
                hdr.paper_white_nits as u16,
            )
        };

        Self {
            schema_version: PRESENTATION_SCHEMA_VERSION,
            swapchain_color_space,
            scene_linear_target_format: SceneLinearTargetFormat::Rgba16Float,
            hdr_output_state,
            ui_composition_policy,
            sdr_fallback,
            final_resource: FrameGraphResourceType::FinalComposedOutput,
            max_luminance_nits,
            paper_white_nits,
        }
    }

    #[must_use]
    pub const fn is_hdr_active(self) -> bool {
        matches!(
            self.hdr_output_state,
            HdrOutputState::Active | HdrOutputState::ScaffoldEnabled
        )
    }

    #[must_use]
    pub const fn requires_sdr_fallback(self) -> bool {
        self.sdr_fallback.is_fallback()
    }

    #[must_use]
    pub fn ui_color_conversion_required_for(self, ui: UiColorSpace) -> bool {
        self.ui_composition_policy.requires_color_conversion_for(ui)
    }
}

impl Default for PresentationPolicy {
    fn default() -> Self {
        Self::PRODUCT_SDR_DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationDiagnostics {
    pub schema_version: u16,
    pub swapchain_color_space: SwapchainColorSpace,
    pub hdr_output_state: HdrOutputState,
    pub ui_composition_policy: UiCompositionPolicy,
    pub sdr_fallback: SdrFallbackReason,
    pub max_luminance_nits: u16,
    pub paper_white_nits: u16,
    pub scene_linear_target_format: SceneLinearTargetFormat,
}

impl PresentationDiagnostics {
    #[must_use]
    pub const fn from_policy(policy: PresentationPolicy) -> Self {
        Self {
            schema_version: PRESENTATION_SCHEMA_VERSION,
            swapchain_color_space: policy.swapchain_color_space,
            hdr_output_state: policy.hdr_output_state,
            ui_composition_policy: policy.ui_composition_policy,
            sdr_fallback: policy.sdr_fallback,
            max_luminance_nits: policy.max_luminance_nits,
            paper_white_nits: policy.paper_white_nits,
            scene_linear_target_format: policy.scene_linear_target_format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_api::HdrOutputSettings;

    #[test]
    fn product_default_is_sdr_with_linear_scene_target_and_srgb_swapchain() {
        let policy = PresentationPolicy::product_default();

        assert_eq!(
            policy.swapchain_color_space,
            SwapchainColorSpace::SrgbNonlinear
        );
        assert_eq!(
            policy.scene_linear_target_format,
            SceneLinearTargetFormat::Rgba16Float
        );
        assert_eq!(policy.hdr_output_state, HdrOutputState::Disabled);
        assert_eq!(
            policy.ui_composition_policy,
            UiCompositionPolicy::LinearScene
        );
        assert!(!policy.is_hdr_active());
    }

    #[test]
    fn hdr_request_with_supported_display_and_swapchain_enables_hdr10_scaffold() {
        let policy = PresentationPolicy::from_camera_settings(
            HdrOutputSettings {
                enabled: true,
                max_nits: 1_000.0,
                paper_white_nits: 200.0,
            },
            DisplayCapabilities::HDR1000,
            SwapchainCapabilities::HDR_FULL,
        );

        assert_eq!(
            policy.swapchain_color_space,
            SwapchainColorSpace::Hdr10St2084
        );
        assert_eq!(policy.hdr_output_state, HdrOutputState::ScaffoldEnabled);
        assert_eq!(
            policy.ui_composition_policy,
            UiCompositionPolicy::Hdr10Output
        );
        assert_eq!(policy.sdr_fallback, SdrFallbackReason::NotInFallback);
        assert_eq!(policy.max_luminance_nits, 1_000);
        assert_eq!(policy.paper_white_nits, 200);
    }

    #[test]
    fn hdr_request_falls_back_to_sdr_when_display_lacks_hdr() {
        let policy = PresentationPolicy::from_camera_settings(
            HdrOutputSettings {
                enabled: true,
                max_nits: 1_000.0,
                paper_white_nits: 200.0,
            },
            DisplayCapabilities::SDR,
            SwapchainCapabilities::HDR_FULL,
        );
        assert_eq!(
            policy.hdr_output_state,
            HdrOutputState::DisplayDoesNotSupportHdr
        );
        assert_eq!(
            policy.sdr_fallback,
            SdrFallbackReason::DisplayDoesNotSupportHdr
        );
        assert!(policy.requires_sdr_fallback());
    }

    #[test]
    fn hdr_request_falls_back_to_sdr_when_swapchain_lacks_hdr10() {
        let policy = PresentationPolicy::from_camera_settings(
            HdrOutputSettings {
                enabled: true,
                max_nits: 1_000.0,
                paper_white_nits: 200.0,
            },
            DisplayCapabilities::HDR1000,
            SwapchainCapabilities::SDR_ONLY,
        );
        assert_eq!(
            policy.hdr_output_state,
            HdrOutputState::SwapchainCannotProvideHdr
        );
        assert_eq!(
            policy.sdr_fallback,
            SdrFallbackReason::SwapchainCannotProvideHdr
        );
    }

    #[test]
    fn ui_color_conversion_skipped_when_surface_matches_composition_policy() {
        let policy = PresentationPolicy::product_default();
        assert!(!policy.ui_color_conversion_required_for(UiColorSpace::Linear));
        assert!(policy.ui_color_conversion_required_for(UiColorSpace::Srgb));
    }

    #[test]
    fn diagnostics_summarize_policy_state() {
        let policy = PresentationPolicy::from_camera_settings(
            HdrOutputSettings {
                enabled: true,
                max_nits: 1_500.0,
                paper_white_nits: 250.0,
            },
            DisplayCapabilities::HDR1000,
            SwapchainCapabilities::HDR_FULL,
        );
        let diagnostics = PresentationDiagnostics::from_policy(policy);

        assert_eq!(
            diagnostics.swapchain_color_space,
            SwapchainColorSpace::Hdr10St2084
        );
        assert_eq!(
            diagnostics.hdr_output_state,
            HdrOutputState::ScaffoldEnabled
        );
        assert_eq!(diagnostics.max_luminance_nits, 1_500);
        assert_eq!(diagnostics.paper_white_nits, 250);
    }
}
