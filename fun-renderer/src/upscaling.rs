use std::{fmt::Write as _, io, path::Path};

use crate::frame_graph::{
    FrameGraphPassRole, FrameGraphResourceType, RendererFrameDescription, RendererFrameGraph,
};

pub const UPSCALING_SCHEMA_VERSION: u16 = 1;
pub const UPSCALING_BENCHMARK_ARTIFACT_ENV: &str = "FUN_RENDERER_UPSCALING_BENCHMARK_ARTIFACT";
pub const UPSCALING_INPUT_RESOURCE_COUNT: usize = 7;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderExtent {
    pub width: u32,
    pub height: u32,
}

impl RenderExtent {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.width > 0 && self.height > 0
    }

    #[must_use]
    pub const fn pixel_count(self) -> u64 {
        (self.width as u64).saturating_mul(self.height as u64)
    }

    #[must_use]
    pub fn scale_ratio_per_mille(self, display: Self) -> u32 {
        if !self.is_valid() {
            return 0;
        }
        let render_pixels = self.pixel_count();
        if render_pixels == 0 {
            return 0;
        }
        let display_pixels = display.pixel_count();
        display_pixels
            .saturating_mul(1_000)
            .checked_div(render_pixels)
            .unwrap_or(0)
            .try_into()
            .unwrap_or(u32::MAX)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerMode {
    #[default]
    Disabled,
    NativeNearest,
    NativeBilinear,
    NativeDebug,
    DlssSuperResolution,
    Fsr2,
    Fsr3,
}

impl UpscalerMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NativeNearest => "native_nearest",
            Self::NativeBilinear => "native_bilinear",
            Self::NativeDebug => "native_debug",
            Self::DlssSuperResolution => "dlss_super_resolution",
            Self::Fsr2 => "fsr2",
            Self::Fsr3 => "fsr3",
        }
    }

    #[must_use]
    pub const fn vendor(self) -> UpscalerVendor {
        match self {
            Self::Disabled | Self::NativeNearest | Self::NativeBilinear | Self::NativeDebug => {
                UpscalerVendor::Native
            }
            Self::DlssSuperResolution => UpscalerVendor::NvidiaDlss,
            Self::Fsr2 | Self::Fsr3 => UpscalerVendor::AmdFsr,
        }
    }

    #[must_use]
    pub const fn is_native(self) -> bool {
        matches!(
            self,
            Self::NativeNearest | Self::NativeBilinear | Self::NativeDebug
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerVendor {
    #[default]
    Native,
    NvidiaDlss,
    AmdFsr,
}

impl UpscalerVendor {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::NvidiaDlss => "nvidia_dlss",
            Self::AmdFsr => "amd_fsr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerCapabilities {
    pub native: bool,
    pub dlss_sr: bool,
    pub fsr2: bool,
    pub fsr3: bool,
    pub hdr: bool,
    pub reactive_mask: bool,
    pub transparency_mask: bool,
}

impl UpscalerCapabilities {
    pub const COMPILED: Self = Self {
        native: true,
        dlss_sr: cfg!(feature = "dlss"),
        fsr2: cfg!(feature = "fsr"),
        fsr3: cfg!(feature = "fsr"),
        hdr: true,
        reactive_mask: cfg!(feature = "fsr"),
        transparency_mask: cfg!(feature = "fsr"),
    };

    #[must_use]
    pub const fn compiled() -> Self {
        Self::COMPILED
    }

    #[must_use]
    pub const fn software_only() -> Self {
        Self {
            native: true,
            dlss_sr: false,
            fsr2: false,
            fsr3: false,
            hdr: true,
            reactive_mask: true,
            transparency_mask: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerMipBiasPolicy {
    #[default]
    None,
    Automatic,
    FixedMilli(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerConfig {
    pub requested_mode: UpscalerMode,
    pub allow_native_fallback: bool,
    pub mip_bias_policy: UpscalerMipBiasPolicy,
}

impl UpscalerConfig {
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            requested_mode: UpscalerMode::Disabled,
            allow_native_fallback: true,
            mip_bias_policy: UpscalerMipBiasPolicy::None,
        }
    }

    #[must_use]
    pub const fn native_debug() -> Self {
        Self {
            requested_mode: UpscalerMode::NativeDebug,
            allow_native_fallback: true,
            mip_bias_policy: UpscalerMipBiasPolicy::Automatic,
        }
    }

    #[must_use]
    pub const fn request(requested_mode: UpscalerMode) -> Self {
        Self {
            requested_mode,
            allow_native_fallback: true,
            mip_bias_policy: UpscalerMipBiasPolicy::Automatic,
        }
    }
}

impl Default for UpscalerConfig {
    fn default() -> Self {
        Self::native_debug()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerJitter {
    pub x_milli: i16,
    pub y_milli: i16,
    pub sequence_index: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerExposure {
    pub exposure_ev100_milli: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerHdrMetadata {
    pub hdr_enabled: bool,
    pub max_luminance_nits: u16,
    pub paper_white_nits: u16,
}

impl Default for UpscalerHdrMetadata {
    fn default() -> Self {
        Self {
            hdr_enabled: false,
            max_luminance_nits: 1_000,
            paper_white_nits: 200,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionVectorSpace {
    #[default]
    RenderPixels,
    DisplayPixels,
    Ndc,
}

impl MotionVectorSpace {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RenderPixels => "render_pixels",
            Self::DisplayPixels => "display_pixels",
            Self::Ndc => "ndc",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionVectorInvalidReason {
    #[default]
    None,
    Missing,
    WrongSpace,
    CoverageTooLow,
    PreviousMatricesMissing,
    JitterNotApplied,
    SceneCutRequiresReset,
}

impl MotionVectorInvalidReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Missing => "missing",
            Self::WrongSpace => "wrong_space",
            Self::CoverageTooLow => "coverage_too_low",
            Self::PreviousMatricesMissing => "previous_matrices_missing",
            Self::JitterNotApplied => "jitter_not_applied",
            Self::SceneCutRequiresReset => "scene_cut_requires_reset",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MotionVectorValidationInput {
    pub present: bool,
    pub space: MotionVectorSpace,
    pub coverage_per_mille: u16,
    pub previous_matrices_available: bool,
    pub jitter_compensated: bool,
    pub scene_cut: bool,
}

impl MotionVectorValidationInput {
    #[must_use]
    pub const fn valid_fullscreen() -> Self {
        Self {
            present: true,
            space: MotionVectorSpace::RenderPixels,
            coverage_per_mille: 1_000,
            previous_matrices_available: true,
            jitter_compensated: true,
            scene_cut: false,
        }
    }
}

impl Default for MotionVectorValidationInput {
    fn default() -> Self {
        Self::valid_fullscreen()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MotionVectorValidationResult {
    pub valid: bool,
    pub reason: MotionVectorInvalidReason,
    pub coverage_per_mille: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerInputResources {
    pub scene_color: FrameGraphResourceType,
    pub depth: FrameGraphResourceType,
    pub motion_vectors: FrameGraphResourceType,
    pub exposure: FrameGraphResourceType,
    pub reactive_mask: FrameGraphResourceType,
    pub transparency_mask: FrameGraphResourceType,
    pub hdr_metadata: FrameGraphResourceType,
    pub scene_color_contains_ui: bool,
}

impl UpscalerInputResources {
    pub const HUDLESS_SCENE_COLOR: Self = Self {
        scene_color: FrameGraphResourceType::RenderResolutionSceneColor,
        depth: FrameGraphResourceType::Depth,
        motion_vectors: FrameGraphResourceType::MotionVectors,
        exposure: FrameGraphResourceType::Exposure,
        reactive_mask: FrameGraphResourceType::ReactiveMask,
        transparency_mask: FrameGraphResourceType::TransparencyMask,
        hdr_metadata: FrameGraphResourceType::HdrMetadata,
        scene_color_contains_ui: false,
    };

    #[must_use]
    pub const fn is_contract_complete(self) -> bool {
        matches!(
            self.scene_color,
            FrameGraphResourceType::RenderResolutionSceneColor
        ) && matches!(self.depth, FrameGraphResourceType::Depth)
            && matches!(self.motion_vectors, FrameGraphResourceType::MotionVectors)
            && matches!(self.exposure, FrameGraphResourceType::Exposure)
            && matches!(self.reactive_mask, FrameGraphResourceType::ReactiveMask)
            && matches!(
                self.transparency_mask,
                FrameGraphResourceType::TransparencyMask
            )
            && matches!(self.hdr_metadata, FrameGraphResourceType::HdrMetadata)
            && !self.scene_color_contains_ui
    }
}

impl Default for UpscalerInputResources {
    fn default() -> Self {
        Self::HUDLESS_SCENE_COLOR
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerFrameInputs {
    pub resources: UpscalerInputResources,
    pub render_size: RenderExtent,
    pub display_size: RenderExtent,
    pub jitter: UpscalerJitter,
    pub exposure: UpscalerExposure,
    pub hdr_metadata: UpscalerHdrMetadata,
    pub motion_vectors: MotionVectorValidationInput,
    pub reactive_mask_coverage_per_mille: u16,
    pub transparency_mask_coverage_per_mille: u16,
}

impl UpscalerFrameInputs {
    #[must_use]
    pub const fn test_scene() -> Self {
        Self {
            resources: UpscalerInputResources::HUDLESS_SCENE_COLOR,
            render_size: RenderExtent::new(1_280, 720),
            display_size: RenderExtent::new(1_920, 1_080),
            jitter: UpscalerJitter {
                x_milli: 125,
                y_milli: -375,
                sequence_index: 3,
            },
            exposure: UpscalerExposure {
                exposure_ev100_milli: 0,
            },
            hdr_metadata: UpscalerHdrMetadata {
                hdr_enabled: true,
                max_luminance_nits: 1_000,
                paper_white_nits: 200,
            },
            motion_vectors: MotionVectorValidationInput::valid_fullscreen(),
            reactive_mask_coverage_per_mille: 43,
            transparency_mask_coverage_per_mille: 17,
        }
    }
}

impl Default for UpscalerFrameInputs {
    fn default() -> Self {
        Self::test_scene()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerHistoryStatus {
    #[default]
    Uninitialized,
    Valid,
    Reset,
    Disabled,
}

impl UpscalerHistoryStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::Valid => "valid",
            Self::Reset => "reset",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerDebugStatus {
    Ready,
    #[default]
    Disabled,
    NativeFallback,
    CapabilityMissing,
    InvalidInput,
    MotionVectorsInvalid,
    UiSeparationViolation,
}

impl UpscalerDebugStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Disabled => "disabled",
            Self::NativeFallback => "native_fallback",
            Self::CapabilityMissing => "capability_missing",
            Self::InvalidInput => "invalid_input",
            Self::MotionVectorsInvalid => "motion_vectors_invalid",
            Self::UiSeparationViolation => "ui_separation_violation",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerSelectionReason {
    #[default]
    DisabledByConfig,
    NativeSelected,
    DlssSelected,
    FsrSelected,
    CapabilityMissingNativeFallback,
    CapabilityMissingDisabled,
    MotionInvalidNativeFallback,
    MotionInvalidDisabled,
    UiSeparationFailed,
    InvalidInput,
}

impl UpscalerSelectionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DisabledByConfig => "disabled_by_config",
            Self::NativeSelected => "native_selected",
            Self::DlssSelected => "dlss_selected",
            Self::FsrSelected => "fsr_selected",
            Self::CapabilityMissingNativeFallback => "capability_missing_native_fallback",
            Self::CapabilityMissingDisabled => "capability_missing_disabled",
            Self::MotionInvalidNativeFallback => "motion_invalid_native_fallback",
            Self::MotionInvalidDisabled => "motion_invalid_disabled",
            Self::UiSeparationFailed => "ui_separation_failed",
            Self::InvalidInput => "invalid_input",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerSelection {
    pub requested_mode: UpscalerMode,
    pub selected_mode: UpscalerMode,
    pub reason: UpscalerSelectionReason,
    pub vendor: UpscalerVendor,
    pub debug_status: UpscalerDebugStatus,
    pub dlss_boundary_ready: bool,
    pub fsr_boundary_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerFrameDiagnostics {
    pub schema_version: u16,
    pub mode: UpscalerMode,
    pub render_width: u32,
    pub render_height: u32,
    pub display_width: u32,
    pub display_height: u32,
    pub jitter_x_milli: i16,
    pub jitter_y_milli: i16,
    pub motion_vector_valid: bool,
    pub motion_vector_invalid_reason: MotionVectorInvalidReason,
    pub reactive_mask_coverage_per_mille: u16,
    pub transparency_mask_coverage_per_mille: u16,
    pub history_reset_count: u32,
    pub upscaler_pass_time_ns: u64,
    pub mip_bias_milli: i16,
    pub ui_separation_ok: bool,
    pub dlss_boundary_ready: bool,
    pub fsr_boundary_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UpscalerFrameOutput {
    pub display_scene_color: FrameGraphResourceType,
    pub history_status: UpscalerHistoryStatus,
    pub debug_status: UpscalerDebugStatus,
    pub diagnostics: UpscalerFrameDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpscalerDebugArtifact {
    pub schema_version: u16,
    pub content: String,
}

#[must_use]
pub fn validate_motion_vectors(input: MotionVectorValidationInput) -> MotionVectorValidationResult {
    let reason = if !input.present {
        MotionVectorInvalidReason::Missing
    } else if input.scene_cut {
        MotionVectorInvalidReason::SceneCutRequiresReset
    } else if input.space != MotionVectorSpace::RenderPixels {
        MotionVectorInvalidReason::WrongSpace
    } else if input.coverage_per_mille < 950 {
        MotionVectorInvalidReason::CoverageTooLow
    } else if !input.previous_matrices_available {
        MotionVectorInvalidReason::PreviousMatricesMissing
    } else if !input.jitter_compensated {
        MotionVectorInvalidReason::JitterNotApplied
    } else {
        MotionVectorInvalidReason::None
    };

    MotionVectorValidationResult {
        valid: reason == MotionVectorInvalidReason::None,
        reason,
        coverage_per_mille: input.coverage_per_mille,
    }
}

#[must_use]
pub fn select_upscaler(
    config: UpscalerConfig,
    capabilities: UpscalerCapabilities,
    inputs: UpscalerFrameInputs,
) -> UpscalerSelection {
    let motion = validate_motion_vectors(inputs.motion_vectors);
    if !inputs.resources.is_contract_complete() {
        return UpscalerSelection {
            requested_mode: config.requested_mode,
            selected_mode: UpscalerMode::Disabled,
            reason: if inputs.resources.scene_color_contains_ui {
                UpscalerSelectionReason::UiSeparationFailed
            } else {
                UpscalerSelectionReason::InvalidInput
            },
            vendor: UpscalerVendor::Native,
            debug_status: if inputs.resources.scene_color_contains_ui {
                UpscalerDebugStatus::UiSeparationViolation
            } else {
                UpscalerDebugStatus::InvalidInput
            },
            dlss_boundary_ready: false,
            fsr_boundary_ready: false,
        };
    }
    if config.requested_mode == UpscalerMode::Disabled {
        return UpscalerSelection {
            requested_mode: config.requested_mode,
            selected_mode: UpscalerMode::Disabled,
            reason: UpscalerSelectionReason::DisabledByConfig,
            vendor: UpscalerVendor::Native,
            debug_status: UpscalerDebugStatus::Disabled,
            dlss_boundary_ready: false,
            fsr_boundary_ready: false,
        };
    }
    if config.requested_mode.is_native() {
        return select_native(config.requested_mode, capabilities.native);
    }

    match config.requested_mode {
        UpscalerMode::DlssSuperResolution => {
            select_vendor_upscaler(config, capabilities.dlss_sr, motion, false)
        }
        UpscalerMode::Fsr2 => {
            select_vendor_upscaler(config, fsr_ready(capabilities, inputs, false), motion, true)
        }
        UpscalerMode::Fsr3 => {
            select_vendor_upscaler(config, fsr_ready(capabilities, inputs, true), motion, true)
        }
        UpscalerMode::Disabled
        | UpscalerMode::NativeNearest
        | UpscalerMode::NativeBilinear
        | UpscalerMode::NativeDebug => unreachable!("handled above"),
    }
}

fn select_native(requested_mode: UpscalerMode, native_available: bool) -> UpscalerSelection {
    if native_available {
        UpscalerSelection {
            requested_mode,
            selected_mode: requested_mode,
            reason: UpscalerSelectionReason::NativeSelected,
            vendor: UpscalerVendor::Native,
            debug_status: UpscalerDebugStatus::Ready,
            dlss_boundary_ready: false,
            fsr_boundary_ready: false,
        }
    } else {
        UpscalerSelection {
            requested_mode,
            selected_mode: UpscalerMode::Disabled,
            reason: UpscalerSelectionReason::CapabilityMissingDisabled,
            vendor: UpscalerVendor::Native,
            debug_status: UpscalerDebugStatus::CapabilityMissing,
            dlss_boundary_ready: false,
            fsr_boundary_ready: false,
        }
    }
}

fn select_vendor_upscaler(
    config: UpscalerConfig,
    capability_ready: bool,
    motion: MotionVectorValidationResult,
    fsr: bool,
) -> UpscalerSelection {
    if !capability_ready {
        return fallback_or_disable(
            config,
            UpscalerSelectionReason::CapabilityMissingNativeFallback,
            UpscalerSelectionReason::CapabilityMissingDisabled,
            UpscalerDebugStatus::CapabilityMissing,
        );
    }
    if !motion.valid {
        return fallback_or_disable(
            config,
            UpscalerSelectionReason::MotionInvalidNativeFallback,
            UpscalerSelectionReason::MotionInvalidDisabled,
            UpscalerDebugStatus::MotionVectorsInvalid,
        );
    }

    UpscalerSelection {
        requested_mode: config.requested_mode,
        selected_mode: config.requested_mode,
        reason: if fsr {
            UpscalerSelectionReason::FsrSelected
        } else {
            UpscalerSelectionReason::DlssSelected
        },
        vendor: config.requested_mode.vendor(),
        debug_status: UpscalerDebugStatus::Ready,
        dlss_boundary_ready: !fsr,
        fsr_boundary_ready: fsr,
    }
}

fn fallback_or_disable(
    config: UpscalerConfig,
    fallback_reason: UpscalerSelectionReason,
    disabled_reason: UpscalerSelectionReason,
    disabled_debug_status: UpscalerDebugStatus,
) -> UpscalerSelection {
    if config.allow_native_fallback {
        UpscalerSelection {
            requested_mode: config.requested_mode,
            selected_mode: UpscalerMode::NativeBilinear,
            reason: fallback_reason,
            vendor: UpscalerVendor::Native,
            debug_status: UpscalerDebugStatus::NativeFallback,
            dlss_boundary_ready: false,
            fsr_boundary_ready: false,
        }
    } else {
        UpscalerSelection {
            requested_mode: config.requested_mode,
            selected_mode: UpscalerMode::Disabled,
            reason: disabled_reason,
            vendor: UpscalerVendor::Native,
            debug_status: disabled_debug_status,
            dlss_boundary_ready: false,
            fsr_boundary_ready: false,
        }
    }
}

fn fsr_ready(
    capabilities: UpscalerCapabilities,
    inputs: UpscalerFrameInputs,
    require_fsr3: bool,
) -> bool {
    let mode_ready = if require_fsr3 {
        capabilities.fsr3
    } else {
        capabilities.fsr2
    };
    mode_ready
        && capabilities.reactive_mask
        && capabilities.transparency_mask
        && (!inputs.hdr_metadata.hdr_enabled || capabilities.hdr)
}

#[must_use]
pub fn evaluate_upscaler_frame(
    config: UpscalerConfig,
    capabilities: UpscalerCapabilities,
    inputs: UpscalerFrameInputs,
    previous_history_valid: bool,
    history_reset_count: u32,
    pass_time_ns: u64,
) -> UpscalerFrameOutput {
    let selection = select_upscaler(config, capabilities, inputs);
    let motion = validate_motion_vectors(inputs.motion_vectors);
    let history_status = if selection.selected_mode == UpscalerMode::Disabled {
        UpscalerHistoryStatus::Disabled
    } else if !previous_history_valid || !motion.valid {
        UpscalerHistoryStatus::Reset
    } else {
        UpscalerHistoryStatus::Valid
    };
    let history_reset_count = history_reset_count
        .saturating_add(u32::from(history_status == UpscalerHistoryStatus::Reset));
    let mip_bias_milli = compute_mip_bias_milli(
        inputs.render_size,
        inputs.display_size,
        config.mip_bias_policy,
    );

    UpscalerFrameOutput {
        display_scene_color: FrameGraphResourceType::DisplayResolutionSceneColor,
        history_status,
        debug_status: selection.debug_status,
        diagnostics: UpscalerFrameDiagnostics {
            schema_version: UPSCALING_SCHEMA_VERSION,
            mode: selection.selected_mode,
            render_width: inputs.render_size.width,
            render_height: inputs.render_size.height,
            display_width: inputs.display_size.width,
            display_height: inputs.display_size.height,
            jitter_x_milli: inputs.jitter.x_milli,
            jitter_y_milli: inputs.jitter.y_milli,
            motion_vector_valid: motion.valid,
            motion_vector_invalid_reason: motion.reason,
            reactive_mask_coverage_per_mille: inputs.reactive_mask_coverage_per_mille,
            transparency_mask_coverage_per_mille: inputs.transparency_mask_coverage_per_mille,
            history_reset_count,
            upscaler_pass_time_ns: pass_time_ns,
            mip_bias_milli,
            ui_separation_ok: inputs.resources.is_contract_complete(),
            dlss_boundary_ready: selection.dlss_boundary_ready,
            fsr_boundary_ready: selection.fsr_boundary_ready,
        },
    }
}

#[must_use]
pub fn frame_graph_upscaling_contract_valid(graph: &RendererFrameGraph) -> bool {
    if !graph.execute().graph_valid() {
        return false;
    }
    let Some(upscale) = graph.pass_handle_for_role(FrameGraphPassRole::UpscaleBoundary) else {
        return false;
    };
    for resource_type in [
        FrameGraphResourceType::RenderResolutionSceneColor,
        FrameGraphResourceType::Depth,
        FrameGraphResourceType::MotionVectors,
        FrameGraphResourceType::Exposure,
        FrameGraphResourceType::ReactiveMask,
        FrameGraphResourceType::TransparencyMask,
        FrameGraphResourceType::HdrMetadata,
    ] {
        if !graph.pass_reads_resource_type(upscale, resource_type) {
            return false;
        }
    }
    if graph.pass_reads_resource_type(upscale, FrameGraphResourceType::UiColorAlpha)
        || !graph
            .pass_writes_resource_type(upscale, FrameGraphResourceType::DisplayResolutionSceneColor)
    {
        return false;
    }

    let Some(compose) = graph.pass_handle_for_role(FrameGraphPassRole::Compose) else {
        return false;
    };
    if !graph.pass_reads_resource_type(compose, FrameGraphResourceType::DisplayResolutionSceneColor)
        || !graph.pass_reads_resource_type(compose, FrameGraphResourceType::UiColorAlpha)
    {
        return false;
    }

    let Some(upscale_order) = pass_order(graph, FrameGraphPassRole::UpscaleBoundary) else {
        return false;
    };
    if let Some(frame_generation_order) =
        pass_order(graph, FrameGraphPassRole::FrameGenerationBoundary)
        && upscale_order >= frame_generation_order
    {
        return false;
    }
    let Some(compose_order) = pass_order(graph, FrameGraphPassRole::Compose) else {
        return false;
    };
    upscale_order < compose_order
}

fn pass_order(graph: &RendererFrameGraph, role: FrameGraphPassRole) -> Option<usize> {
    graph
        .passes()
        .iter()
        .filter(|pass| pass.descriptor.enabled)
        .position(|pass| pass.descriptor.role == role)
}

#[must_use]
pub fn compute_mip_bias_milli(
    render_size: RenderExtent,
    display_size: RenderExtent,
    policy: UpscalerMipBiasPolicy,
) -> i16 {
    match policy {
        UpscalerMipBiasPolicy::None => 0,
        UpscalerMipBiasPolicy::FixedMilli(value) => value,
        UpscalerMipBiasPolicy::Automatic => {
            if !render_size.is_valid() || !display_size.is_valid() {
                return 0;
            }
            let ratio = render_size.scale_ratio_per_mille(display_size);
            match ratio {
                0..=1_000 => 0,
                1_001..=1_499 => -500,
                1_500..=2_249 => -750,
                2_250..=3_999 => -1_000,
                _ => -1_250,
            }
        }
    }
}

#[must_use]
pub fn estimate_upscaler_pass_time_ns(
    mode: UpscalerMode,
    render_size: RenderExtent,
    display_size: RenderExtent,
) -> u64 {
    if mode == UpscalerMode::Disabled {
        return 0;
    }
    let render_pixels = render_size.pixel_count();
    let display_pixels = display_size.pixel_count();
    let cost_per_mega_pixel_ns = match mode {
        UpscalerMode::Disabled => 0,
        UpscalerMode::NativeNearest => 90_000,
        UpscalerMode::NativeBilinear => 120_000,
        UpscalerMode::NativeDebug => 150_000,
        UpscalerMode::DlssSuperResolution => 280_000,
        UpscalerMode::Fsr2 => 260_000,
        UpscalerMode::Fsr3 => 300_000,
    };
    render_pixels
        .saturating_add(display_pixels)
        .saturating_mul(cost_per_mega_pixel_ns)
        / 1_000_000
}

#[must_use]
pub fn upscaling_debug_artifact(
    output: &UpscalerFrameOutput,
    selection: UpscalerSelection,
) -> UpscalerDebugArtifact {
    let diagnostics = output.diagnostics;
    let mut content = String::new();
    let _ = writeln!(
        content,
        "schema_version={} mode={} requested_mode={} selected_mode={} reason={} vendor={}",
        UPSCALING_SCHEMA_VERSION,
        diagnostics.mode.as_str(),
        selection.requested_mode.as_str(),
        selection.selected_mode.as_str(),
        selection.reason.as_str(),
        selection.vendor.as_str(),
    );
    let _ = writeln!(
        content,
        "render_size={}x{} display_size={}x{} jitter=({}, {}) motion_vector_valid={} motion_vector_invalid_reason={}",
        diagnostics.render_width,
        diagnostics.render_height,
        diagnostics.display_width,
        diagnostics.display_height,
        diagnostics.jitter_x_milli,
        diagnostics.jitter_y_milli,
        diagnostics.motion_vector_valid,
        diagnostics.motion_vector_invalid_reason.as_str(),
    );
    let _ = writeln!(
        content,
        "reactive_mask_coverage_per_mille={} transparency_mask_coverage_per_mille={} history_reset_count={} upscaler_pass_time_ns={} mip_bias_milli={}",
        diagnostics.reactive_mask_coverage_per_mille,
        diagnostics.transparency_mask_coverage_per_mille,
        diagnostics.history_reset_count,
        diagnostics.upscaler_pass_time_ns,
        diagnostics.mip_bias_milli,
    );
    let _ = writeln!(
        content,
        "ui_separation_ok={} dlss_boundary_ready={} fsr_boundary_ready={} output_resource={} history_status={} debug_status={}",
        diagnostics.ui_separation_ok,
        diagnostics.dlss_boundary_ready,
        diagnostics.fsr_boundary_ready,
        output.display_scene_color.as_str(),
        output.history_status.as_str(),
        output.debug_status.as_str(),
    );
    UpscalerDebugArtifact {
        schema_version: UPSCALING_SCHEMA_VERSION,
        content,
    }
}

pub fn write_upscaling_artifact(
    path: impl AsRef<Path>,
    artifact: &UpscalerDebugArtifact,
) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, artifact.content.as_bytes())
}

#[must_use]
pub fn upscaling_test_frame_graph() -> RendererFrameGraph {
    RendererFrameGraph::from_frame_description(
        RendererFrameDescription::static_scene_with_ui(16)
            .with_upscaling(true)
            .with_frame_generation(true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vendor_capabilities() -> UpscalerCapabilities {
        UpscalerCapabilities {
            native: true,
            dlss_sr: true,
            fsr2: true,
            fsr3: true,
            hdr: true,
            reactive_mask: true,
            transparency_mask: true,
        }
    }

    #[test]
    fn native_fallback_upscaler_outputs_display_resolution_hudless_scene_color() {
        let inputs = UpscalerFrameInputs::test_scene();
        let output = evaluate_upscaler_frame(
            UpscalerConfig::native_debug(),
            UpscalerCapabilities::software_only(),
            inputs,
            true,
            2,
            estimate_upscaler_pass_time_ns(
                UpscalerMode::NativeDebug,
                inputs.render_size,
                inputs.display_size,
            ),
        );

        assert_eq!(
            output.display_scene_color,
            FrameGraphResourceType::DisplayResolutionSceneColor
        );
        assert_eq!(output.debug_status, UpscalerDebugStatus::Ready);
        assert_eq!(output.history_status, UpscalerHistoryStatus::Valid);
        assert_eq!(output.diagnostics.mode, UpscalerMode::NativeDebug);
        assert!(output.diagnostics.ui_separation_ok);
    }

    #[test]
    fn dlss_boundary_requires_capability_and_valid_motion_vectors() {
        let mut config = UpscalerConfig::request(UpscalerMode::DlssSuperResolution);
        config.allow_native_fallback = false;
        let mut inputs = UpscalerFrameInputs::test_scene();
        inputs.motion_vectors.coverage_per_mille = 400;

        let output = evaluate_upscaler_frame(config, vendor_capabilities(), inputs, true, 0, 0);
        assert_eq!(output.diagnostics.mode, UpscalerMode::Disabled);
        assert_eq!(
            output.debug_status,
            UpscalerDebugStatus::MotionVectorsInvalid
        );
        assert_eq!(
            output.diagnostics.motion_vector_invalid_reason,
            MotionVectorInvalidReason::CoverageTooLow
        );
        assert!(!output.diagnostics.dlss_boundary_ready);

        inputs.motion_vectors = MotionVectorValidationInput::valid_fullscreen();
        let output = evaluate_upscaler_frame(config, vendor_capabilities(), inputs, true, 0, 0);
        assert_eq!(output.diagnostics.mode, UpscalerMode::DlssSuperResolution);
        assert!(output.diagnostics.dlss_boundary_ready);
        assert!(output.diagnostics.motion_vector_valid);
    }

    #[test]
    fn fsr_boundary_requires_masks_hdr_and_exposure_contract() {
        let mut capabilities = vendor_capabilities();
        capabilities.reactive_mask = false;
        let inputs = UpscalerFrameInputs::test_scene();
        let selection = select_upscaler(
            UpscalerConfig::request(UpscalerMode::Fsr2),
            capabilities,
            inputs,
        );

        assert_eq!(selection.selected_mode, UpscalerMode::NativeBilinear);
        assert_eq!(
            selection.reason,
            UpscalerSelectionReason::CapabilityMissingNativeFallback
        );
        assert!(!selection.fsr_boundary_ready);

        let selection = select_upscaler(
            UpscalerConfig::request(UpscalerMode::Fsr2),
            vendor_capabilities(),
            inputs,
        );
        assert_eq!(selection.selected_mode, UpscalerMode::Fsr2);
        assert!(selection.fsr_boundary_ready);
    }

    #[test]
    fn frame_graph_places_upscaling_before_frame_generation_and_late_ui_composition() {
        let graph = upscaling_test_frame_graph();

        assert!(frame_graph_upscaling_contract_valid(&graph));
        let upscale = graph
            .pass_handle_for_role(FrameGraphPassRole::UpscaleBoundary)
            .expect("upscale pass");
        assert!(
            !graph.pass_reads_resource_type(upscale, FrameGraphResourceType::UiColorAlpha),
            "upscale must consume HUD-less scene color only"
        );
        for resource_type in [
            FrameGraphResourceType::RenderResolutionSceneColor,
            FrameGraphResourceType::Depth,
            FrameGraphResourceType::MotionVectors,
            FrameGraphResourceType::Exposure,
            FrameGraphResourceType::ReactiveMask,
            FrameGraphResourceType::TransparencyMask,
            FrameGraphResourceType::HdrMetadata,
        ] {
            assert!(
                graph.pass_reads_resource_type(upscale, resource_type),
                "{}",
                resource_type.as_str()
            );
        }
    }

    #[test]
    fn upscaler_rejects_scene_color_that_already_contains_ui() {
        let mut inputs = UpscalerFrameInputs::test_scene();
        inputs.resources.scene_color_contains_ui = true;

        let selection = select_upscaler(
            UpscalerConfig::request(UpscalerMode::DlssSuperResolution),
            vendor_capabilities(),
            inputs,
        );

        assert_eq!(selection.selected_mode, UpscalerMode::Disabled);
        assert_eq!(
            selection.reason,
            UpscalerSelectionReason::UiSeparationFailed
        );
        assert_eq!(
            selection.debug_status,
            UpscalerDebugStatus::UiSeparationViolation
        );
    }

    #[test]
    fn upscaling_benchmark_artifact_records_diagnostics() {
        let inputs = UpscalerFrameInputs::test_scene();
        let config = UpscalerConfig::request(UpscalerMode::Fsr2);
        let selection = select_upscaler(config, vendor_capabilities(), inputs);
        let output = evaluate_upscaler_frame(
            config,
            vendor_capabilities(),
            inputs,
            false,
            0,
            estimate_upscaler_pass_time_ns(
                selection.selected_mode,
                inputs.render_size,
                inputs.display_size,
            ),
        );
        let artifact = upscaling_debug_artifact(&output, selection);

        assert!(artifact.content.contains("mode=fsr2"));
        assert!(artifact.content.contains("ui_separation_ok=true"));
        assert!(artifact.content.contains("dlss_boundary_ready=false"));
        assert!(artifact.content.contains("fsr_boundary_ready=true"));

        if let Some(path) = std::env::var_os(UPSCALING_BENCHMARK_ARTIFACT_ENV) {
            write_upscaling_artifact(path, &artifact)
                .expect("upscaling benchmark artifact should be writable when requested");
        }
    }
}
