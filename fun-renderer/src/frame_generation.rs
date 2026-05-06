use std::{fmt::Write as _, io, path::Path};

use crate::{
    frame_graph::{
        FrameGraphPassRole, FrameGraphResourceType, RendererFrameDescription, RendererFrameGraph,
    },
    upscaling::{
        MotionVectorInvalidReason, MotionVectorValidationInput, RenderExtent,
        validate_motion_vectors,
    },
};

pub const FRAME_GENERATION_SCHEMA_VERSION: u16 = 1;
pub const FRAME_GENERATION_BENCHMARK_ARTIFACT_ENV: &str =
    "FUN_RENDERER_FRAME_GENERATION_BENCHMARK_ARTIFACT";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGenerationMode {
    #[default]
    Disabled,
    NativePassThrough,
    DlssFrameGeneration,
    FsrFrameGeneration,
}

impl FrameGenerationMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NativePassThrough => "native_pass_through",
            Self::DlssFrameGeneration => "dlss_frame_generation",
            Self::FsrFrameGeneration => "fsr_frame_generation",
        }
    }

    #[must_use]
    pub const fn vendor(self) -> FrameGenerationVendor {
        match self {
            Self::Disabled | Self::NativePassThrough => FrameGenerationVendor::Native,
            Self::DlssFrameGeneration => FrameGenerationVendor::NvidiaDlss,
            Self::FsrFrameGeneration => FrameGenerationVendor::AmdFsr,
        }
    }

    #[must_use]
    pub const fn generates_frames(self) -> bool {
        matches!(self, Self::DlssFrameGeneration | Self::FsrFrameGeneration)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGenerationVendor {
    #[default]
    Native,
    NvidiaDlss,
    AmdFsr,
}

impl FrameGenerationVendor {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::NvidiaDlss => "nvidia_dlss",
            Self::AmdFsr => "amd_fsr",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGenerationViewportPolicy {
    #[default]
    GameRuntime,
    EditorDocked,
    EditorPlayInEditor,
    EditorImmersiveViewport,
    CinematicPreview,
}

impl FrameGenerationViewportPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GameRuntime => "game_runtime",
            Self::EditorDocked => "editor_docked",
            Self::EditorPlayInEditor => "editor_play_in_editor",
            Self::EditorImmersiveViewport => "editor_immersive_viewport",
            Self::CinematicPreview => "cinematic_preview",
        }
    }

    #[must_use]
    pub const fn allows_frame_generation(self) -> bool {
        matches!(
            self,
            Self::GameRuntime
                | Self::EditorPlayInEditor
                | Self::EditorImmersiveViewport
                | Self::CinematicPreview
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGenerationDisableReason {
    #[default]
    None,
    DisabledByConfig,
    CapabilityMissing,
    BackendTruthNotReady,
    Paused,
    Loading,
    Menu,
    ResolutionChange,
    SwapchainRecreation,
    InvalidMotionVectors,
    InvalidUiBuffer,
    EditorDockedHeavyManipulation,
    UnstableFramePacing,
    PresentResourceLifetimeInvalid,
    FsrPresentationBridgeMissing,
    UiReadabilityRisk,
    UpscalerHistoryReset,
    InvalidInputResources,
}

impl FrameGenerationDisableReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DisabledByConfig => "disabled_by_config",
            Self::CapabilityMissing => "capability_missing",
            Self::BackendTruthNotReady => "backend_truth_not_ready",
            Self::Paused => "paused",
            Self::Loading => "loading",
            Self::Menu => "menu",
            Self::ResolutionChange => "resolution_change",
            Self::SwapchainRecreation => "swapchain_recreation",
            Self::InvalidMotionVectors => "invalid_motion_vectors",
            Self::InvalidUiBuffer => "invalid_ui_buffer",
            Self::EditorDockedHeavyManipulation => "editor_docked_heavy_manipulation",
            Self::UnstableFramePacing => "unstable_frame_pacing",
            Self::PresentResourceLifetimeInvalid => "present_resource_lifetime_invalid",
            Self::FsrPresentationBridgeMissing => "fsr_presentation_bridge_missing",
            Self::UiReadabilityRisk => "ui_readability_risk",
            Self::UpscalerHistoryReset => "upscaler_history_reset",
            Self::InvalidInputResources => "invalid_input_resources",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGenerationDebugStatus {
    Ready,
    #[default]
    Disabled,
    PolicyBlocked,
    CapabilityBlocked,
    InputInvalid,
    PacingBlocked,
    ResourceLifetimeBlocked,
}

impl FrameGenerationDebugStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Disabled => "disabled",
            Self::PolicyBlocked => "policy_blocked",
            Self::CapabilityBlocked => "capability_blocked",
            Self::InputInvalid => "input_invalid",
            Self::PacingBlocked => "pacing_blocked",
            Self::ResourceLifetimeBlocked => "resource_lifetime_blocked",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiBufferInvalidReason {
    #[default]
    None,
    Missing,
    ExtentMismatch,
    InvalidAlpha,
    NotStableUntilPresent,
    TextReadabilityRisk,
}

impl UiBufferInvalidReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Missing => "missing",
            Self::ExtentMismatch => "extent_mismatch",
            Self::InvalidAlpha => "invalid_alpha",
            Self::NotStableUntilPresent => "not_stable_until_present",
            Self::TextReadabilityRisk => "text_readability_risk",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiBufferValidationInput {
    pub present: bool,
    pub extent_matches_display: bool,
    pub alpha_mode_known: bool,
    pub stable_until_present: bool,
    pub text_readability_risk: bool,
}

impl UiBufferValidationInput {
    #[must_use]
    pub const fn valid() -> Self {
        Self {
            present: true,
            extent_matches_display: true,
            alpha_mode_known: true,
            stable_until_present: true,
            text_readability_risk: false,
        }
    }
}

impl Default for UiBufferValidationInput {
    fn default() -> Self {
        Self::valid()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiBufferValidationResult {
    pub valid: bool,
    pub reason: UiBufferInvalidReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentResourceLifetime {
    pub valid_until_present: bool,
    pub swapchain_generation_valid: bool,
    pub scene_color_valid: bool,
    pub ui_color_valid: bool,
    pub depth_valid: bool,
    pub motion_vectors_valid: bool,
    pub sync_fence_valid: bool,
}

impl PresentResourceLifetime {
    #[must_use]
    pub const fn valid() -> Self {
        Self {
            valid_until_present: true,
            swapchain_generation_valid: true,
            scene_color_valid: true,
            ui_color_valid: true,
            depth_valid: true,
            motion_vectors_valid: true,
            sync_fence_valid: true,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.valid_until_present
            && self.swapchain_generation_valid
            && self.scene_color_valid
            && self.ui_color_valid
            && self.depth_valid
            && self.motion_vectors_valid
            && self.sync_fence_valid
    }
}

impl Default for PresentResourceLifetime {
    fn default() -> Self {
        Self::valid()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationFrameTiming {
    pub frame_index: u64,
    pub delta_time_us: u32,
    pub present_interval_us: u32,
    pub pacing_jitter_us: u32,
    pub unstable_frame_pacing: bool,
}

impl FrameGenerationFrameTiming {
    #[must_use]
    pub const fn stable_60hz(frame_index: u64) -> Self {
        Self {
            frame_index,
            delta_time_us: 16_667,
            present_interval_us: 16_667,
            pacing_jitter_us: 120,
            unstable_frame_pacing: false,
        }
    }

    #[must_use]
    pub const fn is_stable(self) -> bool {
        !self.unstable_frame_pacing && self.pacing_jitter_us <= 1_500
    }
}

impl Default for FrameGenerationFrameTiming {
    fn default() -> Self {
        Self::stable_60hz(0)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationResetFlags {
    pub paused: bool,
    pub loading: bool,
    pub menu_active: bool,
    pub resolution_changed: bool,
    pub swapchain_recreated: bool,
    pub editor_docked_heavy_manipulation: bool,
    pub upscaler_history_reset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationCapabilities {
    pub backend_truth_ready: bool,
    pub native_passthrough: bool,
    pub dlss_fg: bool,
    pub fsr_fg: bool,
    pub fsr_presentation_bridge: bool,
    pub valid_until_present_lifetimes: bool,
}

impl FrameGenerationCapabilities {
    pub const COMPILED: Self = Self {
        backend_truth_ready: false,
        native_passthrough: true,
        dlss_fg: cfg!(all(feature = "frame_generation", feature = "dlss")),
        fsr_fg: cfg!(all(feature = "frame_generation", feature = "fsr")),
        fsr_presentation_bridge: cfg!(all(feature = "frame_generation", feature = "fsr")),
        valid_until_present_lifetimes: false,
    };

    #[must_use]
    pub const fn compiled() -> Self {
        Self::COMPILED
    }

    #[must_use]
    pub const fn test_ready() -> Self {
        Self {
            backend_truth_ready: true,
            native_passthrough: true,
            dlss_fg: true,
            fsr_fg: true,
            fsr_presentation_bridge: true,
            valid_until_present_lifetimes: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationPolicy {
    pub requested_mode: FrameGenerationMode,
    pub explicitly_enabled: bool,
    pub viewport_policy: FrameGenerationViewportPolicy,
    pub disable_in_menus: bool,
}

impl FrameGenerationPolicy {
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            requested_mode: FrameGenerationMode::Disabled,
            explicitly_enabled: false,
            viewport_policy: FrameGenerationViewportPolicy::GameRuntime,
            disable_in_menus: true,
        }
    }

    #[must_use]
    pub const fn game_runtime(requested_mode: FrameGenerationMode) -> Self {
        Self {
            requested_mode,
            explicitly_enabled: true,
            viewport_policy: FrameGenerationViewportPolicy::GameRuntime,
            disable_in_menus: true,
        }
    }

    #[must_use]
    pub const fn editor_docked_default(requested_mode: FrameGenerationMode) -> Self {
        Self {
            requested_mode,
            explicitly_enabled: false,
            viewport_policy: FrameGenerationViewportPolicy::EditorDocked,
            disable_in_menus: true,
        }
    }

    #[must_use]
    pub const fn editor_immersive(requested_mode: FrameGenerationMode) -> Self {
        Self {
            requested_mode,
            explicitly_enabled: true,
            viewport_policy: FrameGenerationViewportPolicy::EditorImmersiveViewport,
            disable_in_menus: true,
        }
    }
}

impl Default for FrameGenerationPolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationInputResources {
    pub display_scene_color: FrameGraphResourceType,
    pub ui_color_alpha: FrameGraphResourceType,
    pub depth: FrameGraphResourceType,
    pub motion_vectors: FrameGraphResourceType,
    pub frame_timing: FrameGraphResourceType,
    pub present_resources: FrameGraphResourceType,
    pub reset_flags: FrameGraphResourceType,
    pub final_composed_already_flattened: bool,
}

impl FrameGenerationInputResources {
    pub const SPLIT_PRESENT_BOUNDARY: Self = Self {
        display_scene_color: FrameGraphResourceType::DisplayResolutionSceneColor,
        ui_color_alpha: FrameGraphResourceType::UiColorAlpha,
        depth: FrameGraphResourceType::Depth,
        motion_vectors: FrameGraphResourceType::MotionVectors,
        frame_timing: FrameGraphResourceType::FrameTiming,
        present_resources: FrameGraphResourceType::PresentResources,
        reset_flags: FrameGraphResourceType::FrameGenerationResetFlags,
        final_composed_already_flattened: false,
    };

    #[must_use]
    pub const fn is_contract_complete(self) -> bool {
        matches!(
            self.display_scene_color,
            FrameGraphResourceType::DisplayResolutionSceneColor
        ) && matches!(self.ui_color_alpha, FrameGraphResourceType::UiColorAlpha)
            && matches!(self.depth, FrameGraphResourceType::Depth)
            && matches!(self.motion_vectors, FrameGraphResourceType::MotionVectors)
            && matches!(self.frame_timing, FrameGraphResourceType::FrameTiming)
            && matches!(
                self.present_resources,
                FrameGraphResourceType::PresentResources
            )
            && matches!(
                self.reset_flags,
                FrameGraphResourceType::FrameGenerationResetFlags
            )
            && !self.final_composed_already_flattened
    }
}

impl Default for FrameGenerationInputResources {
    fn default() -> Self {
        Self::SPLIT_PRESENT_BOUNDARY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationFrameInputs {
    pub resources: FrameGenerationInputResources,
    pub display_size: RenderExtent,
    pub frame_timing: FrameGenerationFrameTiming,
    pub motion_vectors: MotionVectorValidationInput,
    pub ui_buffer: UiBufferValidationInput,
    pub present_lifetime: PresentResourceLifetime,
    pub reset_flags: FrameGenerationResetFlags,
}

impl FrameGenerationFrameInputs {
    #[must_use]
    pub const fn test_scene() -> Self {
        Self {
            resources: FrameGenerationInputResources::SPLIT_PRESENT_BOUNDARY,
            display_size: RenderExtent::new(1_920, 1_080),
            frame_timing: FrameGenerationFrameTiming::stable_60hz(17),
            motion_vectors: MotionVectorValidationInput::valid_fullscreen(),
            ui_buffer: UiBufferValidationInput::valid(),
            present_lifetime: PresentResourceLifetime::valid(),
            reset_flags: FrameGenerationResetFlags {
                paused: false,
                loading: false,
                menu_active: false,
                resolution_changed: false,
                swapchain_recreated: false,
                editor_docked_heavy_manipulation: false,
                upscaler_history_reset: false,
            },
        }
    }
}

impl Default for FrameGenerationFrameInputs {
    fn default() -> Self {
        Self::test_scene()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationSelection {
    pub requested_mode: FrameGenerationMode,
    pub selected_mode: FrameGenerationMode,
    pub vendor: FrameGenerationVendor,
    pub disabled_reason: FrameGenerationDisableReason,
    pub debug_status: FrameGenerationDebugStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationPacingDiagnostics {
    pub frame_index: u64,
    pub delta_time_us: u32,
    pub present_interval_us: u32,
    pub pacing_jitter_us: u32,
    pub unstable_frame_pacing: bool,
    pub generated_frame_count: u32,
    pub presented_frame_count: u32,
    pub estimated_pass_time_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationDiagnostics {
    pub schema_version: u16,
    pub mode: FrameGenerationMode,
    pub vendor: FrameGenerationVendor,
    pub display_width: u32,
    pub display_height: u32,
    pub motion_vector_valid: bool,
    pub motion_vector_invalid_reason: MotionVectorInvalidReason,
    pub ui_buffer_valid: bool,
    pub ui_buffer_invalid_reason: UiBufferInvalidReason,
    pub present_lifetime_valid: bool,
    pub disable_reason: FrameGenerationDisableReason,
    pub pacing: FrameGenerationPacingDiagnostics,
    pub ui_stability_protected: bool,
    pub valid_until_present: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGenerationOutput {
    pub presentable_frames: FrameGraphResourceType,
    pub generated_frame_count: u32,
    pub presented_frame_count: u32,
    pub disable_reason: FrameGenerationDisableReason,
    pub debug_status: FrameGenerationDebugStatus,
    pub diagnostics: FrameGenerationDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameGenerationDebugArtifact {
    pub schema_version: u16,
    pub content: String,
}

#[must_use]
pub fn validate_ui_buffer(input: UiBufferValidationInput) -> UiBufferValidationResult {
    let reason = if !input.present {
        UiBufferInvalidReason::Missing
    } else if !input.extent_matches_display {
        UiBufferInvalidReason::ExtentMismatch
    } else if !input.alpha_mode_known {
        UiBufferInvalidReason::InvalidAlpha
    } else if !input.stable_until_present {
        UiBufferInvalidReason::NotStableUntilPresent
    } else if input.text_readability_risk {
        UiBufferInvalidReason::TextReadabilityRisk
    } else {
        UiBufferInvalidReason::None
    };
    UiBufferValidationResult {
        valid: reason == UiBufferInvalidReason::None,
        reason,
    }
}

#[must_use]
pub fn select_frame_generation(
    policy: FrameGenerationPolicy,
    capabilities: FrameGenerationCapabilities,
    inputs: FrameGenerationFrameInputs,
) -> FrameGenerationSelection {
    let disabled_reason = disable_reason(policy, capabilities, inputs);
    if disabled_reason != FrameGenerationDisableReason::None {
        return FrameGenerationSelection {
            requested_mode: policy.requested_mode,
            selected_mode: FrameGenerationMode::Disabled,
            vendor: FrameGenerationVendor::Native,
            disabled_reason,
            debug_status: debug_status_for_disable(disabled_reason),
        };
    }
    FrameGenerationSelection {
        requested_mode: policy.requested_mode,
        selected_mode: policy.requested_mode,
        vendor: policy.requested_mode.vendor(),
        disabled_reason: FrameGenerationDisableReason::None,
        debug_status: FrameGenerationDebugStatus::Ready,
    }
}

fn disable_reason(
    policy: FrameGenerationPolicy,
    capabilities: FrameGenerationCapabilities,
    inputs: FrameGenerationFrameInputs,
) -> FrameGenerationDisableReason {
    if !inputs.resources.is_contract_complete() {
        return FrameGenerationDisableReason::InvalidInputResources;
    }
    if policy.requested_mode == FrameGenerationMode::Disabled || !policy.explicitly_enabled {
        return FrameGenerationDisableReason::DisabledByConfig;
    }
    if !policy.viewport_policy.allows_frame_generation() {
        return FrameGenerationDisableReason::EditorDockedHeavyManipulation;
    }
    if inputs.reset_flags.paused {
        return FrameGenerationDisableReason::Paused;
    }
    if inputs.reset_flags.loading {
        return FrameGenerationDisableReason::Loading;
    }
    if policy.disable_in_menus && inputs.reset_flags.menu_active {
        return FrameGenerationDisableReason::Menu;
    }
    if inputs.reset_flags.resolution_changed {
        return FrameGenerationDisableReason::ResolutionChange;
    }
    if inputs.reset_flags.swapchain_recreated {
        return FrameGenerationDisableReason::SwapchainRecreation;
    }
    if inputs.reset_flags.editor_docked_heavy_manipulation {
        return FrameGenerationDisableReason::EditorDockedHeavyManipulation;
    }
    if inputs.reset_flags.upscaler_history_reset {
        return FrameGenerationDisableReason::UpscalerHistoryReset;
    }
    if !inputs.frame_timing.is_stable() {
        return FrameGenerationDisableReason::UnstableFramePacing;
    }
    if !validate_motion_vectors(inputs.motion_vectors).valid {
        return FrameGenerationDisableReason::InvalidMotionVectors;
    }
    let ui = validate_ui_buffer(inputs.ui_buffer);
    if !ui.valid {
        return if ui.reason == UiBufferInvalidReason::TextReadabilityRisk {
            FrameGenerationDisableReason::UiReadabilityRisk
        } else {
            FrameGenerationDisableReason::InvalidUiBuffer
        };
    }
    if !inputs.present_lifetime.is_valid() || !capabilities.valid_until_present_lifetimes {
        return FrameGenerationDisableReason::PresentResourceLifetimeInvalid;
    }
    if !capabilities.backend_truth_ready {
        return FrameGenerationDisableReason::BackendTruthNotReady;
    }
    match policy.requested_mode {
        FrameGenerationMode::Disabled => FrameGenerationDisableReason::DisabledByConfig,
        FrameGenerationMode::NativePassThrough => {
            if capabilities.native_passthrough {
                FrameGenerationDisableReason::None
            } else {
                FrameGenerationDisableReason::CapabilityMissing
            }
        }
        FrameGenerationMode::DlssFrameGeneration => {
            if capabilities.dlss_fg {
                FrameGenerationDisableReason::None
            } else {
                FrameGenerationDisableReason::CapabilityMissing
            }
        }
        FrameGenerationMode::FsrFrameGeneration => {
            if !capabilities.fsr_fg {
                FrameGenerationDisableReason::CapabilityMissing
            } else if !capabilities.fsr_presentation_bridge {
                FrameGenerationDisableReason::FsrPresentationBridgeMissing
            } else {
                FrameGenerationDisableReason::None
            }
        }
    }
}

fn debug_status_for_disable(reason: FrameGenerationDisableReason) -> FrameGenerationDebugStatus {
    match reason {
        FrameGenerationDisableReason::None => FrameGenerationDebugStatus::Ready,
        FrameGenerationDisableReason::DisabledByConfig => FrameGenerationDebugStatus::Disabled,
        FrameGenerationDisableReason::CapabilityMissing
        | FrameGenerationDisableReason::BackendTruthNotReady
        | FrameGenerationDisableReason::FsrPresentationBridgeMissing => {
            FrameGenerationDebugStatus::CapabilityBlocked
        }
        FrameGenerationDisableReason::InvalidInputResources
        | FrameGenerationDisableReason::InvalidMotionVectors
        | FrameGenerationDisableReason::InvalidUiBuffer
        | FrameGenerationDisableReason::UiReadabilityRisk => {
            FrameGenerationDebugStatus::InputInvalid
        }
        FrameGenerationDisableReason::UnstableFramePacing => {
            FrameGenerationDebugStatus::PacingBlocked
        }
        FrameGenerationDisableReason::PresentResourceLifetimeInvalid => {
            FrameGenerationDebugStatus::ResourceLifetimeBlocked
        }
        FrameGenerationDisableReason::Paused
        | FrameGenerationDisableReason::Loading
        | FrameGenerationDisableReason::Menu
        | FrameGenerationDisableReason::ResolutionChange
        | FrameGenerationDisableReason::SwapchainRecreation
        | FrameGenerationDisableReason::EditorDockedHeavyManipulation
        | FrameGenerationDisableReason::UpscalerHistoryReset => {
            FrameGenerationDebugStatus::PolicyBlocked
        }
    }
}

#[must_use]
pub fn evaluate_frame_generation(
    policy: FrameGenerationPolicy,
    capabilities: FrameGenerationCapabilities,
    inputs: FrameGenerationFrameInputs,
) -> FrameGenerationOutput {
    let selection = select_frame_generation(policy, capabilities, inputs);
    let motion = validate_motion_vectors(inputs.motion_vectors);
    let ui = validate_ui_buffer(inputs.ui_buffer);
    let generated_frame_count = if selection.selected_mode.generates_frames() {
        1
    } else {
        0
    };
    let presented_frame_count = 1 + generated_frame_count;
    let estimated_pass_time_ns =
        estimate_frame_generation_pass_time_ns(selection.selected_mode, inputs.display_size);

    FrameGenerationOutput {
        presentable_frames: FrameGraphResourceType::PresentableFrames,
        generated_frame_count,
        presented_frame_count,
        disable_reason: selection.disabled_reason,
        debug_status: selection.debug_status,
        diagnostics: FrameGenerationDiagnostics {
            schema_version: FRAME_GENERATION_SCHEMA_VERSION,
            mode: selection.selected_mode,
            vendor: selection.vendor,
            display_width: inputs.display_size.width,
            display_height: inputs.display_size.height,
            motion_vector_valid: motion.valid,
            motion_vector_invalid_reason: motion.reason,
            ui_buffer_valid: ui.valid,
            ui_buffer_invalid_reason: ui.reason,
            present_lifetime_valid: inputs.present_lifetime.is_valid()
                && capabilities.valid_until_present_lifetimes,
            disable_reason: selection.disabled_reason,
            pacing: FrameGenerationPacingDiagnostics {
                frame_index: inputs.frame_timing.frame_index,
                delta_time_us: inputs.frame_timing.delta_time_us,
                present_interval_us: inputs.frame_timing.present_interval_us,
                pacing_jitter_us: inputs.frame_timing.pacing_jitter_us,
                unstable_frame_pacing: !inputs.frame_timing.is_stable(),
                generated_frame_count,
                presented_frame_count,
                estimated_pass_time_ns,
            },
            ui_stability_protected: ui.valid && inputs.resources.is_contract_complete(),
            valid_until_present: inputs.present_lifetime.is_valid(),
        },
    }
}

#[must_use]
pub fn frame_graph_frame_generation_contract_valid(graph: &RendererFrameGraph) -> bool {
    if !graph.execute().graph_valid() {
        return false;
    }
    let Some(upscale_order) = pass_order(graph, FrameGraphPassRole::UpscaleBoundary) else {
        return false;
    };
    let Some(frame_generation) =
        graph.pass_handle_for_role(FrameGraphPassRole::FrameGenerationBoundary)
    else {
        return false;
    };
    for resource_type in [
        FrameGraphResourceType::DisplayResolutionSceneColor,
        FrameGraphResourceType::UiColorAlpha,
        FrameGraphResourceType::Depth,
        FrameGraphResourceType::MotionVectors,
        FrameGraphResourceType::FrameTiming,
        FrameGraphResourceType::PresentResources,
        FrameGraphResourceType::FrameGenerationResetFlags,
    ] {
        if !graph.pass_reads_resource_type(frame_generation, resource_type) {
            return false;
        }
    }
    for resource_type in [
        FrameGraphResourceType::HistoryBuffer,
        FrameGraphResourceType::PresentableFrames,
        FrameGraphResourceType::PacingDiagnostics,
    ] {
        if !graph.pass_writes_resource_type(frame_generation, resource_type) {
            return false;
        }
    }
    if graph.pass_reads_resource_type(
        frame_generation,
        FrameGraphResourceType::FinalComposedOutput,
    ) {
        return false;
    }
    let Some(frame_generation_order) =
        pass_order(graph, FrameGraphPassRole::FrameGenerationBoundary)
    else {
        return false;
    };
    let Some(compose_order) = pass_order(graph, FrameGraphPassRole::Compose) else {
        return false;
    };
    let Some(present_order) = pass_order(graph, FrameGraphPassRole::Present) else {
        return false;
    };
    let Some(compose) = graph.pass_handle_for_role(FrameGraphPassRole::Compose) else {
        return false;
    };
    let Some(present) = graph.pass_handle_for_role(FrameGraphPassRole::Present) else {
        return false;
    };
    upscale_order < frame_generation_order
        && frame_generation_order < compose_order
        && compose_order < present_order
        && graph.pass_reads_resource_type(compose, FrameGraphResourceType::UiColorAlpha)
        && graph.pass_reads_resource_type(compose, FrameGraphResourceType::PresentableFrames)
        && graph.pass_reads_resource_type(present, FrameGraphResourceType::PresentResources)
}

fn pass_order(graph: &RendererFrameGraph, role: FrameGraphPassRole) -> Option<usize> {
    graph
        .passes()
        .iter()
        .filter(|pass| pass.descriptor.enabled)
        .position(|pass| pass.descriptor.role == role)
}

#[must_use]
pub fn estimate_frame_generation_pass_time_ns(
    mode: FrameGenerationMode,
    display_size: RenderExtent,
) -> u64 {
    let cost_per_mega_pixel_ns = match mode {
        FrameGenerationMode::Disabled => 0,
        FrameGenerationMode::NativePassThrough => 40_000,
        FrameGenerationMode::DlssFrameGeneration => 420_000,
        FrameGenerationMode::FsrFrameGeneration => 460_000,
    };
    display_size
        .pixel_count()
        .saturating_mul(cost_per_mega_pixel_ns)
        / 1_000_000
}

#[must_use]
pub fn frame_generation_debug_artifact(
    output: &FrameGenerationOutput,
    selection: FrameGenerationSelection,
) -> FrameGenerationDebugArtifact {
    let diagnostics = output.diagnostics;
    let mut content = String::new();
    let _ = writeln!(
        content,
        "schema_version={} mode={} requested_mode={} selected_mode={} vendor={} disable_reason={} debug_status={}",
        FRAME_GENERATION_SCHEMA_VERSION,
        diagnostics.mode.as_str(),
        selection.requested_mode.as_str(),
        selection.selected_mode.as_str(),
        diagnostics.vendor.as_str(),
        diagnostics.disable_reason.as_str(),
        output.debug_status.as_str(),
    );
    let _ = writeln!(
        content,
        "display_size={}x{} generated_frame_count={} presented_frame_count={} estimated_pass_time_ns={}",
        diagnostics.display_width,
        diagnostics.display_height,
        output.generated_frame_count,
        output.presented_frame_count,
        diagnostics.pacing.estimated_pass_time_ns,
    );
    let _ = writeln!(
        content,
        "motion_vector_valid={} motion_vector_invalid_reason={} ui_buffer_valid={} ui_buffer_invalid_reason={} present_lifetime_valid={} valid_until_present={}",
        diagnostics.motion_vector_valid,
        diagnostics.motion_vector_invalid_reason.as_str(),
        diagnostics.ui_buffer_valid,
        diagnostics.ui_buffer_invalid_reason.as_str(),
        diagnostics.present_lifetime_valid,
        diagnostics.valid_until_present,
    );
    let _ = writeln!(
        content,
        "frame_index={} delta_time_us={} present_interval_us={} pacing_jitter_us={} unstable_frame_pacing={} ui_stability_protected={} output_resource={}",
        diagnostics.pacing.frame_index,
        diagnostics.pacing.delta_time_us,
        diagnostics.pacing.present_interval_us,
        diagnostics.pacing.pacing_jitter_us,
        diagnostics.pacing.unstable_frame_pacing,
        diagnostics.ui_stability_protected,
        output.presentable_frames.as_str(),
    );
    FrameGenerationDebugArtifact {
        schema_version: FRAME_GENERATION_SCHEMA_VERSION,
        content,
    }
}

pub fn write_frame_generation_artifact(
    path: impl AsRef<Path>,
    artifact: &FrameGenerationDebugArtifact,
) -> io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, artifact.content.as_bytes())
}

#[must_use]
pub fn frame_generation_test_frame_graph() -> RendererFrameGraph {
    RendererFrameGraph::from_frame_description(
        RendererFrameDescription::static_scene_with_ui(17)
            .with_upscaling(true)
            .with_frame_generation(true),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_generation_eligibility_requires_policy_capability_and_valid_inputs() {
        let inputs = FrameGenerationFrameInputs::test_scene();
        let policy = FrameGenerationPolicy::game_runtime(FrameGenerationMode::DlssFrameGeneration);
        let output =
            evaluate_frame_generation(policy, FrameGenerationCapabilities::test_ready(), inputs);

        assert_eq!(
            output.diagnostics.mode,
            FrameGenerationMode::DlssFrameGeneration
        );
        assert_eq!(output.generated_frame_count, 1);
        assert_eq!(output.presented_frame_count, 2);
        assert_eq!(output.disable_reason, FrameGenerationDisableReason::None);
        assert!(output.diagnostics.ui_stability_protected);
    }

    #[test]
    fn frame_generation_disables_cleanly_for_invalid_motion_vectors() {
        let mut inputs = FrameGenerationFrameInputs::test_scene();
        inputs.motion_vectors.coverage_per_mille = 250;
        let output = evaluate_frame_generation(
            FrameGenerationPolicy::game_runtime(FrameGenerationMode::DlssFrameGeneration),
            FrameGenerationCapabilities::test_ready(),
            inputs,
        );

        assert_eq!(output.diagnostics.mode, FrameGenerationMode::Disabled);
        assert_eq!(
            output.disable_reason,
            FrameGenerationDisableReason::InvalidMotionVectors
        );
        assert_eq!(output.generated_frame_count, 0);
        assert_eq!(
            output.diagnostics.motion_vector_invalid_reason,
            MotionVectorInvalidReason::CoverageTooLow
        );
    }

    #[test]
    fn resource_lifetime_validation_requires_present_valid_resources() {
        let mut inputs = FrameGenerationFrameInputs::test_scene();
        inputs.present_lifetime.valid_until_present = false;
        let output = evaluate_frame_generation(
            FrameGenerationPolicy::game_runtime(FrameGenerationMode::FsrFrameGeneration),
            FrameGenerationCapabilities::test_ready(),
            inputs,
        );

        assert_eq!(
            output.disable_reason,
            FrameGenerationDisableReason::PresentResourceLifetimeInvalid
        );
        assert_eq!(
            output.debug_status,
            FrameGenerationDebugStatus::ResourceLifetimeBlocked
        );
        assert!(!output.diagnostics.present_lifetime_valid);
    }

    #[test]
    fn ui_readability_and_editor_policy_protect_text_stability() {
        let docked = evaluate_frame_generation(
            FrameGenerationPolicy::editor_docked_default(FrameGenerationMode::DlssFrameGeneration),
            FrameGenerationCapabilities::test_ready(),
            FrameGenerationFrameInputs::test_scene(),
        );
        assert_eq!(
            docked.disable_reason,
            FrameGenerationDisableReason::DisabledByConfig
        );

        let mut inputs = FrameGenerationFrameInputs::test_scene();
        inputs.ui_buffer.text_readability_risk = true;
        let immersive = evaluate_frame_generation(
            FrameGenerationPolicy::editor_immersive(FrameGenerationMode::DlssFrameGeneration),
            FrameGenerationCapabilities::test_ready(),
            inputs,
        );
        assert_eq!(
            immersive.disable_reason,
            FrameGenerationDisableReason::UiReadabilityRisk
        );
        assert_eq!(immersive.generated_frame_count, 0);
    }

    #[test]
    fn fsr_frame_generation_requires_dedicated_presentation_bridge() {
        let mut capabilities = FrameGenerationCapabilities::test_ready();
        capabilities.fsr_presentation_bridge = false;
        let output = evaluate_frame_generation(
            FrameGenerationPolicy::game_runtime(FrameGenerationMode::FsrFrameGeneration),
            capabilities,
            FrameGenerationFrameInputs::test_scene(),
        );

        assert_eq!(
            output.disable_reason,
            FrameGenerationDisableReason::FsrPresentationBridgeMissing
        );
        assert_eq!(
            output.debug_status,
            FrameGenerationDebugStatus::CapabilityBlocked
        );
    }

    #[test]
    fn frame_graph_keeps_fg_after_upscaling_and_before_late_ui_composition() {
        let graph = frame_generation_test_frame_graph();

        assert!(frame_graph_frame_generation_contract_valid(&graph));
        let frame_generation = graph
            .pass_handle_for_role(FrameGraphPassRole::FrameGenerationBoundary)
            .expect("frame-generation pass");
        assert!(!graph.pass_reads_resource_type(
            frame_generation,
            FrameGraphResourceType::FinalComposedOutput
        ));
        for resource_type in [
            FrameGraphResourceType::DisplayResolutionSceneColor,
            FrameGraphResourceType::UiColorAlpha,
            FrameGraphResourceType::Depth,
            FrameGraphResourceType::MotionVectors,
            FrameGraphResourceType::FrameTiming,
            FrameGraphResourceType::PresentResources,
            FrameGraphResourceType::FrameGenerationResetFlags,
        ] {
            assert!(
                graph.pass_reads_resource_type(frame_generation, resource_type),
                "{}",
                resource_type.as_str()
            );
        }
    }

    #[test]
    fn benchmark_artifact_records_generated_and_presented_counts() {
        let inputs = FrameGenerationFrameInputs::test_scene();
        let policy = FrameGenerationPolicy::game_runtime(FrameGenerationMode::FsrFrameGeneration);
        let capabilities = FrameGenerationCapabilities::test_ready();
        let selection = select_frame_generation(policy, capabilities, inputs);
        let output = evaluate_frame_generation(policy, capabilities, inputs);
        let artifact = frame_generation_debug_artifact(&output, selection);

        assert!(artifact.content.contains("mode=fsr_frame_generation"));
        assert!(artifact.content.contains("generated_frame_count=1"));
        assert!(artifact.content.contains("presented_frame_count=2"));
        assert!(artifact.content.contains("ui_stability_protected=true"));

        if let Some(path) = std::env::var_os(FRAME_GENERATION_BENCHMARK_ARTIFACT_ENV) {
            write_frame_generation_artifact(path, &artifact)
                .expect("frame-generation artifact should be writable when requested");
        }
    }
}
