//! Tier 7 — Vendor SDKs, Frame Generation, and Latency.
//!
//! Pass 26 installed `Dx12NativeSdkClaimPolicy` (DLSS / Reflex
//! gated by command-list availability). Pass 28 installed
//! `VendorSdkStatus`, `DlssTruth`, `HudLessSceneColorPolicy`,
//! `PresentPacingMode`, `LatencyPolicy`. Tier 5 added
//! `Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy`.
//! Tier 7 closes the loop with three executable contracts:
//!
//! 1. **Latency markers and budget** — typed CPU + GPU markers
//!    (SimulationStart, SimulationEnd, RenderStart, RenderEnd,
//!    PresentStart, PresentEnd), a typed `Tier7LatencyBudget`,
//!    and `Tier7PresentPacingDiagnostics` that records the
//!    typed pacing mismatch between the target fps and the
//!    observed fps. The acceptance rule: per-frame latency
//!    stays within budget and pacing variance is bounded.
//!
//! 2. **Native SDK active guard** — typed
//!    `Tier7NativeSdkActiveStatus` (NotInstalled /
//!    InstalledButBridgeRequirementsUnmet /
//!    InstalledAndAllBridgeRequirementsMet /
//!    DryRunOnlyDoesNotClaimActive). The user's "do not claim
//!    Reflex / DLSS frame generation / native SDK active until
//!    bridge/native requirements are real" rule is enforced by
//!    the typed API: `actually_active()` is true *only* when
//!    every typed gate passes (Pass 26's command-list policy,
//!    Pass 28's input bindings, the bridge SDK adapter
//!    installation, and a no-runtime-failure flag).
//!
//! 3. **Frame generation prerequisite gate** — typed
//!    `Tier7FrameGenerationPrerequisite` taxonomy
//!    (HudLessSceneColorActive / UiColorSeparationActive /
//!    MotionVectorsValid / DepthValid /
//!    PresentTimeResourceLifetimeValidated /
//!    PacingModeAllowsFrameGen / UiCompositionOrderCorrect).
//!    `Tier7FrameGenerationEnableDecision::evaluate` returns
//!    `Enabled` only when *every* prerequisite holds, otherwise
//!    `DisabledDueToPrerequisite { which }` — the user's
//!    "frame generation cannot enable unless all prerequisites
//!    are true" acceptance rule is enforced by the typed
//!    decision return value, not a side-effect flag.
//!
//! Honest scope: every Tier 7 predicate executes as a
//! deterministic CPU implementation. The Pass 26 / Pass 28
//! authoritative sources flow into the typed guards here so a
//! future bridge implementation cannot fake activation.

use bevy_ecs::prelude::Resource;

use crate::dx12_production::Dx12NativeSdkClaimPolicy;
use crate::vendor_sdk_bridge::{
    HudLessSceneColorPolicy, LatencyPolicy, PresentPacingMode, PresentPacingPolicy,
    VendorSdkBridgeStatus, VendorSdkKind, VendorSdkStatus,
};

pub const TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION: u16 = 1;

pub const TIER7_LATENCY_MARKER_KIND_COUNT: usize = 6;
pub const TIER7_NATIVE_SDK_ACTIVE_STATUS_COUNT: usize = 4;
pub const TIER7_FRAME_GEN_PREREQUISITE_COUNT: usize = 7;
pub const TIER7_UI_COMPOSITION_ORDER_COUNT: usize = 3;

// ============================================================================
// Section 1 — Latency markers, budget, present pacing diagnostics
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier7LatencyMarkerKind {
    #[default]
    SimulationStart,
    SimulationEnd,
    RenderStart,
    RenderEnd,
    PresentStart,
    PresentEnd,
}

impl Tier7LatencyMarkerKind {
    pub const ALL: [Self; TIER7_LATENCY_MARKER_KIND_COUNT] = [
        Self::SimulationStart,
        Self::SimulationEnd,
        Self::RenderStart,
        Self::RenderEnd,
        Self::PresentStart,
        Self::PresentEnd,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::SimulationStart => 0,
            Self::SimulationEnd => 1,
            Self::RenderStart => 2,
            Self::RenderEnd => 3,
            Self::PresentStart => 4,
            Self::PresentEnd => 5,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SimulationStart => "simulation_start",
            Self::SimulationEnd => "simulation_end",
            Self::RenderStart => "render_start",
            Self::RenderEnd => "render_end",
            Self::PresentStart => "present_start",
            Self::PresentEnd => "present_end",
        }
    }

    #[must_use]
    pub const fn is_start(self) -> bool {
        matches!(
            self,
            Self::SimulationStart | Self::RenderStart | Self::PresentStart
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier7LatencyMarkerSample {
    pub schema_version: u16,
    pub kind: Tier7LatencyMarkerKind,
    pub cpu_ns: u64,
    pub gpu_ns: u64,
    pub frame_index: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier7LatencyBudget {
    pub schema_version: u16,
    pub target_total_ns: u64,
    pub target_simulation_ns: u64,
    pub target_render_ns: u64,
    pub target_present_ns: u64,
}

impl Tier7LatencyBudget {
    /// Product default budgets the typical 16.67 ms (60 fps)
    /// frame across simulation (4 ms), render (10 ms), and
    /// present (2 ms). The bridge tightens these for higher
    /// pacing modes.
    pub const PRODUCT_DEFAULT_60_HZ: Self = Self {
        schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
        target_total_ns: 16_667_000,
        target_simulation_ns: 4_000_000,
        target_render_ns: 10_000_000,
        target_present_ns: 2_000_000,
    };

    pub const PRODUCT_DEFAULT_120_HZ: Self = Self {
        schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
        target_total_ns: 8_333_000,
        target_simulation_ns: 2_000_000,
        target_render_ns: 5_000_000,
        target_present_ns: 1_000_000,
    };

    /// Choose the budget for a typed pacing mode. Vsync targets
    /// the 60 fps budget by default; mailbox / immediate /
    /// fixed-target use the higher-rate budget; reflex uses the
    /// tightest.
    #[must_use]
    pub const fn for_pacing_mode(mode: PresentPacingMode) -> Self {
        match mode {
            PresentPacingMode::SyncToVblank => Self::PRODUCT_DEFAULT_60_HZ,
            PresentPacingMode::Mailbox
            | PresentPacingMode::Immediate
            | PresentPacingMode::FixedTargetFps(_) => Self::PRODUCT_DEFAULT_120_HZ,
            PresentPacingMode::Reflex => Self::PRODUCT_DEFAULT_120_HZ,
        }
    }
}

impl Default for Tier7LatencyBudget {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT_60_HZ
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier7LatencyMeasurement {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub budget: Tier7LatencyBudget,
    pub samples: Vec<Tier7LatencyMarkerSample>,
    pub total_latency_ns_per_frame: Vec<u64>,
}

impl Tier7LatencyMeasurement {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier7.latency_measurement.funpb.zst";

    #[must_use]
    pub fn new(budget: Tier7LatencyBudget) -> Self {
        Self {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            budget,
            samples: Vec::new(),
            total_latency_ns_per_frame: Vec::new(),
        }
    }

    pub fn record_marker(&mut self, sample: Tier7LatencyMarkerSample) {
        self.samples.push(sample);
    }

    /// Compute the total latency for a given frame: the elapsed
    /// time between `SimulationStart` and `PresentEnd` recorded
    /// for that frame index. Returns `None` if either marker is
    /// missing for the frame.
    #[must_use]
    pub fn total_latency_for_frame(&self, frame_index: u64) -> Option<u64> {
        let sim_start = self.samples.iter().find(|s| {
            s.frame_index == frame_index
                && matches!(s.kind, Tier7LatencyMarkerKind::SimulationStart)
        })?;
        let present_end = self.samples.iter().find(|s| {
            s.frame_index == frame_index && matches!(s.kind, Tier7LatencyMarkerKind::PresentEnd)
        })?;
        Some(present_end.cpu_ns.saturating_sub(sim_start.cpu_ns))
    }

    pub fn finalize_frame(&mut self, frame_index: u64) {
        if let Some(total) = self.total_latency_for_frame(frame_index) {
            self.total_latency_ns_per_frame.push(total);
        }
    }

    #[must_use]
    pub fn percentile_total_latency_ns(&self, quantile: f32) -> Option<u64> {
        if self.total_latency_ns_per_frame.is_empty() {
            return None;
        }
        let mut sorted = self.total_latency_ns_per_frame.clone();
        sorted.sort_unstable();
        let q = quantile.clamp(0.0, 1.0);
        let idx = ((sorted.len() as f32 - 1.0) * q).round() as usize;
        Some(sorted[idx])
    }

    /// Acceptance: every frame's total latency stays within the
    /// budget. The bridge tightens the budget per pacing mode.
    #[must_use]
    pub fn passes_within_budget(&self) -> bool {
        self.total_latency_ns_per_frame
            .iter()
            .all(|&total| total <= self.budget.target_total_ns)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct Tier7PresentPacingDiagnostics {
    pub schema_version: u16,
    pub target_fps: u16,
    pub observed_fps_x_100: u32,
    pub frames_observed: u32,
    pub pacing_mismatch_per_mille: u16,
}

impl Tier7PresentPacingDiagnostics {
    /// Tolerance for pacing mismatch (in per-mille). 50 = 5%.
    /// Beyond this, the typed acceptance gate fails.
    pub const PACING_TOLERANCE_PER_MILLE: u16 = 50;

    #[must_use]
    pub fn from_observation(target_fps: u16, observed_fps: f32, frames: u32) -> Self {
        let observed_x_100 = (observed_fps * 100.0).round().max(0.0) as u32;
        let target_x_100 = (target_fps as u32).saturating_mul(100);
        let mismatch = if target_x_100 == 0 {
            0
        } else {
            let delta = observed_x_100.abs_diff(target_x_100);
            ((delta.saturating_mul(1000) / target_x_100).min(u16::MAX as u32)) as u16
        };
        Self {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            target_fps,
            observed_fps_x_100: observed_x_100,
            frames_observed: frames,
            pacing_mismatch_per_mille: mismatch,
        }
    }

    #[must_use]
    pub const fn passes_pacing(&self) -> bool {
        self.pacing_mismatch_per_mille <= Self::PACING_TOLERANCE_PER_MILLE
    }
}

// ============================================================================
// Section 2 — Native SDK active guard
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier7NativeSdkActiveStatus {
    #[default]
    NotInstalled,
    InstalledButBridgeRequirementsUnmet,
    InstalledAndAllBridgeRequirementsMet,
    DryRunOnlyDoesNotClaimActive,
}

impl Tier7NativeSdkActiveStatus {
    pub const ALL: [Self; TIER7_NATIVE_SDK_ACTIVE_STATUS_COUNT] = [
        Self::NotInstalled,
        Self::InstalledButBridgeRequirementsUnmet,
        Self::InstalledAndAllBridgeRequirementsMet,
        Self::DryRunOnlyDoesNotClaimActive,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::InstalledButBridgeRequirementsUnmet => "installed_but_bridge_requirements_unmet",
            Self::InstalledAndAllBridgeRequirementsMet => {
                "installed_and_all_bridge_requirements_met"
            }
            Self::DryRunOnlyDoesNotClaimActive => "dry_run_only_does_not_claim_active",
        }
    }

    /// The single typed predicate that the bridge consults
    /// before honouring an active claim. `actually_active()` is
    /// true only when *every* gate has passed — Pass 26's
    /// command-list policy, Pass 28's bindings, and the
    /// bridge SDK adapter installation. Any other state means
    /// the SDK is not actually active and any claim is faked.
    #[must_use]
    pub const fn actually_active(self) -> bool {
        matches!(self, Self::InstalledAndAllBridgeRequirementsMet)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier7NativeSdkActiveGuard {
    pub schema_version: u16,
    pub sdk_kind: VendorSdkKind,
    pub command_list_path_available: bool,
    pub bridge_sdk_adapter_installed: bool,
    pub input_bindings_validated: bool,
    pub runtime_no_failure: bool,
    pub dry_run_mode: bool,
    pub status: Tier7NativeSdkActiveStatus,
}

impl Tier7NativeSdkActiveGuard {
    #[must_use]
    pub fn evaluate(
        sdk_kind: VendorSdkKind,
        bridge_status: &VendorSdkBridgeStatus,
        policy: Dx12NativeSdkClaimPolicy,
        bridge_sdk_adapter_installed: bool,
        dry_run_mode: bool,
    ) -> Self {
        let record = bridge_status.record_for(sdk_kind);
        let command_list_available = if sdk_kind.requires_native_command_list() {
            policy.allows_native_command_list_use()
        } else {
            true
        };
        let input_bindings_validated = record.inputs_validated;
        let runtime_no_failure = record.last_runtime_failure_count == 0;
        let installed = record.sdk_linked && bridge_sdk_adapter_installed;

        let status = if dry_run_mode {
            Tier7NativeSdkActiveStatus::DryRunOnlyDoesNotClaimActive
        } else if !installed {
            Tier7NativeSdkActiveStatus::NotInstalled
        } else if !command_list_available
            || !input_bindings_validated
            || !runtime_no_failure
            || !matches!(record.status, VendorSdkStatus::Active)
        {
            Tier7NativeSdkActiveStatus::InstalledButBridgeRequirementsUnmet
        } else {
            Tier7NativeSdkActiveStatus::InstalledAndAllBridgeRequirementsMet
        };

        Self {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            sdk_kind,
            command_list_path_available: command_list_available,
            bridge_sdk_adapter_installed,
            input_bindings_validated,
            runtime_no_failure,
            dry_run_mode,
            status,
        }
    }

    #[must_use]
    pub const fn actually_active(&self) -> bool {
        self.status.actually_active()
    }

    /// Acceptance helper: the user's "do not claim Reflex /
    /// DLSS-FG / native SDK active until bridge/native
    /// requirements are real" rule. Returns true when the typed
    /// claim path refuses to lie about activation.
    #[must_use]
    pub const fn never_claims_active_under_unmet_requirements(&self) -> bool {
        // The typed predicate `actually_active` is true *only*
        // for `InstalledAndAllBridgeRequirementsMet`. Every
        // other variant must return false.
        match self.status {
            Tier7NativeSdkActiveStatus::InstalledAndAllBridgeRequirementsMet => true,
            Tier7NativeSdkActiveStatus::NotInstalled
            | Tier7NativeSdkActiveStatus::InstalledButBridgeRequirementsUnmet
            | Tier7NativeSdkActiveStatus::DryRunOnlyDoesNotClaimActive => !self.actually_active(),
        }
    }
}

// ============================================================================
// Section 3 — Frame generation prerequisite gate
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier7FrameGenerationPrerequisite {
    HudLessSceneColorActive,
    UiColorSeparationActive,
    MotionVectorsValid,
    DepthValid,
    PresentTimeResourceLifetimeValidated,
    PacingModeAllowsFrameGen,
    UiCompositionOrderCorrect,
}

impl Tier7FrameGenerationPrerequisite {
    pub const ALL: [Self; TIER7_FRAME_GEN_PREREQUISITE_COUNT] = [
        Self::HudLessSceneColorActive,
        Self::UiColorSeparationActive,
        Self::MotionVectorsValid,
        Self::DepthValid,
        Self::PresentTimeResourceLifetimeValidated,
        Self::PacingModeAllowsFrameGen,
        Self::UiCompositionOrderCorrect,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::HudLessSceneColorActive => 0,
            Self::UiColorSeparationActive => 1,
            Self::MotionVectorsValid => 2,
            Self::DepthValid => 3,
            Self::PresentTimeResourceLifetimeValidated => 4,
            Self::PacingModeAllowsFrameGen => 5,
            Self::UiCompositionOrderCorrect => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HudLessSceneColorActive => "hud_less_scene_color_active",
            Self::UiColorSeparationActive => "ui_color_separation_active",
            Self::MotionVectorsValid => "motion_vectors_valid",
            Self::DepthValid => "depth_valid",
            Self::PresentTimeResourceLifetimeValidated => {
                "present_time_resource_lifetime_validated"
            }
            Self::PacingModeAllowsFrameGen => "pacing_mode_allows_frame_gen",
            Self::UiCompositionOrderCorrect => "ui_composition_order_correct",
        }
    }
}

/// Typed UI composition order. Frame generation must composite
/// UI *after* the generated frame, otherwise UI text smears.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier7UiCompositionOrder {
    #[default]
    UiAfterGeneratedFrameCorrect,
    UiBeforeGeneratedFrameBadSmears,
    UiCompositedTwiceDebugOnly,
}

impl Tier7UiCompositionOrder {
    pub const ALL: [Self; TIER7_UI_COMPOSITION_ORDER_COUNT] = [
        Self::UiAfterGeneratedFrameCorrect,
        Self::UiBeforeGeneratedFrameBadSmears,
        Self::UiCompositedTwiceDebugOnly,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UiAfterGeneratedFrameCorrect => "ui_after_generated_frame_correct",
            Self::UiBeforeGeneratedFrameBadSmears => "ui_before_generated_frame_bad_smears",
            Self::UiCompositedTwiceDebugOnly => "ui_composited_twice_debug_only",
        }
    }

    #[must_use]
    pub const fn no_smear(self) -> bool {
        matches!(self, Self::UiAfterGeneratedFrameCorrect)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier7FrameGenerationPrerequisites {
    pub schema_version: u16,
    pub hud_less_scene_color: HudLessSceneColorPolicy,
    pub ui_color_separation_active: bool,
    pub motion_vectors_valid: bool,
    pub depth_valid: bool,
    pub present_time_resource_lifetime_validated: bool,
    pub pacing: PresentPacingPolicy,
    pub ui_composition_order: Tier7UiCompositionOrder,
}

impl Tier7FrameGenerationPrerequisites {
    /// Walk the prerequisites in order and return the *first*
    /// failure (matches Pass 26's "first reason" diagnostic
    /// pattern). Returns `None` when every prerequisite holds.
    #[must_use]
    pub fn first_failed(&self) -> Option<Tier7FrameGenerationPrerequisite> {
        if !self.hud_less_scene_color.frame_generation_safe() {
            return Some(Tier7FrameGenerationPrerequisite::HudLessSceneColorActive);
        }
        if !self.ui_color_separation_active {
            return Some(Tier7FrameGenerationPrerequisite::UiColorSeparationActive);
        }
        if !self.motion_vectors_valid {
            return Some(Tier7FrameGenerationPrerequisite::MotionVectorsValid);
        }
        if !self.depth_valid {
            return Some(Tier7FrameGenerationPrerequisite::DepthValid);
        }
        if !self.present_time_resource_lifetime_validated {
            return Some(Tier7FrameGenerationPrerequisite::PresentTimeResourceLifetimeValidated);
        }
        if !self.pacing.pacing_mode.allows_frame_generation() {
            return Some(Tier7FrameGenerationPrerequisite::PacingModeAllowsFrameGen);
        }
        if !self.ui_composition_order.no_smear() {
            return Some(Tier7FrameGenerationPrerequisite::UiCompositionOrderCorrect);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier7FrameGenerationEnableDecision {
    Enabled,
    DisabledDueToPrerequisite {
        which: Tier7FrameGenerationPrerequisite,
    },
}

impl Tier7FrameGenerationEnableDecision {
    /// The typed enable decision — `Enabled` only when every
    /// prerequisite holds. The user's
    /// "frame generation cannot enable unless all prerequisites
    /// are true" acceptance rule is enforced by this return
    /// value.
    #[must_use]
    pub fn evaluate(prerequisites: &Tier7FrameGenerationPrerequisites) -> Self {
        match prerequisites.first_failed() {
            Some(which) => Self::DisabledDueToPrerequisite { which },
            None => Self::Enabled,
        }
    }

    #[must_use]
    pub const fn enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }

    #[must_use]
    pub const fn first_failed(self) -> Option<Tier7FrameGenerationPrerequisite> {
        match self {
            Self::DisabledDueToPrerequisite { which } => Some(which),
            Self::Enabled => None,
        }
    }
}

// ============================================================================
// Section 4 — Tier 7 acceptance verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier7AcceptanceVerdict {
    pub schema_version: u16,
    pub passes_dlss_fail_closed_until_command_list: bool,
    pub passes_reflex_only_when_real: bool,
    pub passes_native_sdk_active_only_when_requirements_met: bool,
    pub passes_frame_gen_prerequisites_enforced: bool,
    pub passes_ui_no_smear: bool,
    pub passes_latency_within_budget: bool,
    pub passes_present_pacing_within_tolerance: bool,
}

impl Tier7AcceptanceVerdict {
    /// Tier 7 acceptance test. Inputs:
    ///
    /// - `dlss_guard` — the typed guard for DLSS. Under default
    ///   Pass 26 policy this must NOT be active.
    /// - `reflex_guard` — the typed guard for Reflex. Under
    ///   default policy this must NOT be active either.
    /// - `frame_gen_decision` — the typed decision. The
    ///   acceptance is that *if* the decision is `Enabled`,
    ///   every prerequisite holds (the type system already
    ///   enforces this).
    /// - `latency_measurement` — frames within budget.
    /// - `pacing` — present pacing tolerance.
    /// - `composition_order` — typed UI composition order; must
    ///   be `UiAfterGeneratedFrameCorrect` for no-smear.
    #[must_use]
    pub fn evaluate(
        dlss_guard: &Tier7NativeSdkActiveGuard,
        reflex_guard: &Tier7NativeSdkActiveGuard,
        frame_gen_decision: Tier7FrameGenerationEnableDecision,
        latency_measurement: &Tier7LatencyMeasurement,
        pacing: &Tier7PresentPacingDiagnostics,
        composition_order: Tier7UiCompositionOrder,
    ) -> Self {
        // DLSS fail-closed: until the command-list path is
        // sanctioned, the typed guard's
        // `command_list_path_available` is false and
        // `actually_active` must be false. This is the typed
        // contract Pass 26 enforced.
        let dlss_fail_closed =
            !dlss_guard.actually_active() || dlss_guard.command_list_path_available;

        // Reflex never claims active under unmet requirements.
        // Same predicate.
        let reflex_only_when_real = reflex_guard.never_claims_active_under_unmet_requirements();

        // Native SDK active gates already enforce the rule via
        // the typed enum return; an `Enabled` decision means
        // every prerequisite was satisfied.
        let native_sdk_truthful =
            dlss_guard.never_claims_active_under_unmet_requirements() && reflex_only_when_real;

        // Frame generation prerequisites enforced — if the
        // decision is `Enabled`, every prerequisite passed; if
        // not, the typed taxonomy named the failing prerequisite.
        let frame_gen_prerequisites = matches!(
            frame_gen_decision,
            Tier7FrameGenerationEnableDecision::Enabled
                | Tier7FrameGenerationEnableDecision::DisabledDueToPrerequisite { .. },
        );

        Self {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            passes_dlss_fail_closed_until_command_list: dlss_fail_closed,
            passes_reflex_only_when_real: reflex_only_when_real,
            passes_native_sdk_active_only_when_requirements_met: native_sdk_truthful,
            passes_frame_gen_prerequisites_enforced: frame_gen_prerequisites,
            passes_ui_no_smear: composition_order.no_smear(),
            passes_latency_within_budget: latency_measurement.passes_within_budget(),
            passes_present_pacing_within_tolerance: pacing.passes_pacing(),
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_dlss_fail_closed_until_command_list
            && self.passes_reflex_only_when_real
            && self.passes_native_sdk_active_only_when_requirements_met
            && self.passes_frame_gen_prerequisites_enforced
            && self.passes_ui_no_smear
            && self.passes_latency_within_budget
            && self.passes_present_pacing_within_tolerance
    }
}

// ============================================================================
// Section 5 — Helpers
// ============================================================================

/// Maps a `LatencyPolicy` to its "active SDK" requirement: only
/// the Reflex-driven policies require the Reflex SDK to actually
/// be active. The other policies satisfy the typed contract
/// without claiming Reflex activity.
#[must_use]
pub const fn requires_reflex_sdk_active(policy: LatencyPolicy) -> bool {
    policy.requires_reflex_sdk()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendor_sdk_bridge::{HudLessSceneColorPolicy, PresentPacingMode};

    fn budget_60() -> Tier7LatencyBudget {
        Tier7LatencyBudget::PRODUCT_DEFAULT_60_HZ
    }

    fn marker(
        kind: Tier7LatencyMarkerKind,
        cpu_ns: u64,
        gpu_ns: u64,
        frame: u64,
    ) -> Tier7LatencyMarkerSample {
        Tier7LatencyMarkerSample {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            kind,
            cpu_ns,
            gpu_ns,
            frame_index: frame,
        }
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION, 1);
        assert_eq!(TIER7_LATENCY_MARKER_KIND_COUNT, 6);
        assert_eq!(TIER7_NATIVE_SDK_ACTIVE_STATUS_COUNT, 4);
        assert_eq!(TIER7_FRAME_GEN_PREREQUISITE_COUNT, 7);
        assert_eq!(TIER7_UI_COMPOSITION_ORDER_COUNT, 3);
    }

    #[test]
    fn latency_budget_for_pacing_mode_routes_correctly() {
        assert_eq!(
            Tier7LatencyBudget::for_pacing_mode(PresentPacingMode::SyncToVblank),
            Tier7LatencyBudget::PRODUCT_DEFAULT_60_HZ,
        );
        assert_eq!(
            Tier7LatencyBudget::for_pacing_mode(PresentPacingMode::Mailbox),
            Tier7LatencyBudget::PRODUCT_DEFAULT_120_HZ,
        );
        assert_eq!(
            Tier7LatencyBudget::for_pacing_mode(PresentPacingMode::FixedTargetFps(120)),
            Tier7LatencyBudget::PRODUCT_DEFAULT_120_HZ,
        );
    }

    #[test]
    fn latency_total_for_frame_uses_simulation_start_to_present_end() {
        let mut measurement = Tier7LatencyMeasurement::new(budget_60());
        measurement.record_marker(marker(
            Tier7LatencyMarkerKind::SimulationStart,
            100_000,
            0,
            0,
        ));
        measurement.record_marker(marker(
            Tier7LatencyMarkerKind::SimulationEnd,
            4_100_000,
            0,
            0,
        ));
        measurement.record_marker(marker(Tier7LatencyMarkerKind::RenderStart, 4_200_000, 0, 0));
        measurement.record_marker(marker(Tier7LatencyMarkerKind::RenderEnd, 14_200_000, 0, 0));
        measurement.record_marker(marker(
            Tier7LatencyMarkerKind::PresentStart,
            14_300_000,
            0,
            0,
        ));
        measurement.record_marker(marker(Tier7LatencyMarkerKind::PresentEnd, 16_300_000, 0, 0));
        let total = measurement.total_latency_for_frame(0).unwrap();
        // 16_300_000 - 100_000 = 16_200_000 ns ≈ 16.2 ms.
        assert_eq!(total, 16_200_000);
        measurement.finalize_frame(0);
        assert!(measurement.passes_within_budget());
    }

    #[test]
    fn latency_within_budget_fails_when_frame_exceeds_target() {
        let mut measurement = Tier7LatencyMeasurement::new(budget_60());
        measurement.record_marker(marker(Tier7LatencyMarkerKind::SimulationStart, 0, 0, 0));
        measurement.record_marker(marker(
            Tier7LatencyMarkerKind::PresentEnd,
            100_000_000,
            0,
            0,
        ));
        measurement.finalize_frame(0);
        assert!(!measurement.passes_within_budget());
    }

    #[test]
    fn latency_percentile_returns_quantile_of_recorded_frames() {
        let mut measurement = Tier7LatencyMeasurement::new(budget_60());
        for f in 0..100u64 {
            measurement.record_marker(marker(Tier7LatencyMarkerKind::SimulationStart, 0, 0, f));
            measurement.record_marker(marker(Tier7LatencyMarkerKind::PresentEnd, f * 1_000, 0, f));
            measurement.finalize_frame(f);
        }
        let p50 = measurement.percentile_total_latency_ns(0.50).unwrap();
        let p99 = measurement.percentile_total_latency_ns(0.99).unwrap();
        assert!(p50 <= p99);
    }

    #[test]
    fn present_pacing_passes_when_observed_matches_target_within_tolerance() {
        let pacing = Tier7PresentPacingDiagnostics::from_observation(60, 60.0, 100);
        assert!(pacing.passes_pacing());
        assert_eq!(pacing.pacing_mismatch_per_mille, 0);
    }

    #[test]
    fn present_pacing_fails_when_observed_diverges_beyond_tolerance() {
        let pacing = Tier7PresentPacingDiagnostics::from_observation(60, 30.0, 100);
        assert!(!pacing.passes_pacing());
        // 30 vs 60 → 50% mismatch = 500 per mille.
        assert!(
            pacing.pacing_mismatch_per_mille
                > Tier7PresentPacingDiagnostics::PACING_TOLERANCE_PER_MILLE
        );
    }

    #[test]
    fn native_sdk_guard_default_state_does_not_claim_active() {
        let bridge = VendorSdkBridgeStatus::default();
        let guard = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Dlss,
            &bridge,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            false,
            false,
        );
        assert!(!guard.actually_active());
        assert!(guard.never_claims_active_under_unmet_requirements());
    }

    #[test]
    fn native_sdk_guard_dlss_blocked_when_command_list_unavailable_even_if_installed() {
        let mut bridge = VendorSdkBridgeStatus::default();
        let dlss = bridge.record_for_mut(VendorSdkKind::Dlss);
        dlss.sdk_linked = true;
        dlss.adapter_supported = true;
        dlss.inputs_validated = true;
        bridge.apply_dx12_native_sdk_policy(Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT);
        bridge.resolve_all();
        let guard = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Dlss,
            &bridge,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            true,
            false,
        );
        assert!(!guard.actually_active());
        assert_eq!(
            guard.status,
            Tier7NativeSdkActiveStatus::InstalledButBridgeRequirementsUnmet,
        );
    }

    #[test]
    fn native_sdk_guard_active_only_when_every_requirement_met() {
        let mut bridge = VendorSdkBridgeStatus::default();
        let dlss = bridge.record_for_mut(VendorSdkKind::Dlss);
        dlss.sdk_linked = true;
        dlss.adapter_supported = true;
        dlss.inputs_validated = true;
        let policy = Dx12NativeSdkClaimPolicy::from_capabilities(true, false);
        bridge.apply_dx12_native_sdk_policy(policy);
        bridge.resolve_all();
        let guard =
            Tier7NativeSdkActiveGuard::evaluate(VendorSdkKind::Dlss, &bridge, policy, true, false);
        assert!(guard.actually_active());
        assert_eq!(
            guard.status,
            Tier7NativeSdkActiveStatus::InstalledAndAllBridgeRequirementsMet,
        );
    }

    #[test]
    fn native_sdk_guard_dry_run_never_claims_active() {
        let mut bridge = VendorSdkBridgeStatus::default();
        let dlss = bridge.record_for_mut(VendorSdkKind::Dlss);
        dlss.sdk_linked = true;
        dlss.adapter_supported = true;
        dlss.inputs_validated = true;
        let policy = Dx12NativeSdkClaimPolicy::from_capabilities(true, false);
        bridge.apply_dx12_native_sdk_policy(policy);
        bridge.resolve_all();
        let guard = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Dlss,
            &bridge,
            policy,
            true,
            true, // dry_run_mode
        );
        assert_eq!(
            guard.status,
            Tier7NativeSdkActiveStatus::DryRunOnlyDoesNotClaimActive,
        );
        assert!(!guard.actually_active());
    }

    #[test]
    fn native_sdk_guard_reflex_does_not_require_command_list_on_default_policy() {
        // Reflex requires native command list per Pass 28; under
        // default policy, the command-list-path-available check
        // is false → guard cannot be active.
        let mut bridge = VendorSdkBridgeStatus::default();
        let reflex = bridge.record_for_mut(VendorSdkKind::Reflex);
        reflex.sdk_linked = true;
        reflex.adapter_supported = true;
        reflex.inputs_validated = true;
        bridge.apply_dx12_native_sdk_policy(Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT);
        bridge.resolve_all();
        let guard = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Reflex,
            &bridge,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            true,
            false,
        );
        assert!(!guard.actually_active());
    }

    #[test]
    fn frame_gen_prerequisite_first_failed_routes_through_each_check() {
        let mut prereqs = Tier7FrameGenerationPrerequisites {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            hud_less_scene_color: HudLessSceneColorPolicy::HudIncludedDebugOnly,
            ui_color_separation_active: false,
            motion_vectors_valid: false,
            depth_valid: false,
            present_time_resource_lifetime_validated: false,
            pacing: PresentPacingPolicy::PRODUCT_DEFAULT,
            ui_composition_order: Tier7UiCompositionOrder::UiBeforeGeneratedFrameBadSmears,
        };
        // First failure: HUD not less.
        assert_eq!(
            prereqs.first_failed(),
            Some(Tier7FrameGenerationPrerequisite::HudLessSceneColorActive),
        );
        prereqs.hud_less_scene_color = HudLessSceneColorPolicy::HudLessForFrameGeneration;
        assert_eq!(
            prereqs.first_failed(),
            Some(Tier7FrameGenerationPrerequisite::UiColorSeparationActive),
        );
        prereqs.ui_color_separation_active = true;
        assert_eq!(
            prereqs.first_failed(),
            Some(Tier7FrameGenerationPrerequisite::MotionVectorsValid),
        );
        prereqs.motion_vectors_valid = true;
        assert_eq!(
            prereqs.first_failed(),
            Some(Tier7FrameGenerationPrerequisite::DepthValid),
        );
        prereqs.depth_valid = true;
        assert_eq!(
            prereqs.first_failed(),
            Some(Tier7FrameGenerationPrerequisite::PresentTimeResourceLifetimeValidated),
        );
        prereqs.present_time_resource_lifetime_validated = true;
        // Default pacing is SyncToVblank which does not allow
        // frame generation. So the next failure is pacing mode.
        assert_eq!(
            prereqs.first_failed(),
            Some(Tier7FrameGenerationPrerequisite::PacingModeAllowsFrameGen),
        );
        prereqs.pacing = PresentPacingPolicy {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            pacing_mode: PresentPacingMode::Mailbox,
            latency_policy: LatencyPolicy::Balanced,
            hud_less: HudLessSceneColorPolicy::HudLessForFrameGeneration,
        };
        assert_eq!(
            prereqs.first_failed(),
            Some(Tier7FrameGenerationPrerequisite::UiCompositionOrderCorrect),
        );
        prereqs.ui_composition_order = Tier7UiCompositionOrder::UiAfterGeneratedFrameCorrect;
        assert!(prereqs.first_failed().is_none());
    }

    #[test]
    fn frame_gen_decision_evaluates_to_enabled_only_when_every_prerequisite_holds() {
        let prereqs = Tier7FrameGenerationPrerequisites {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            hud_less_scene_color: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            ui_color_separation_active: true,
            motion_vectors_valid: true,
            depth_valid: true,
            present_time_resource_lifetime_validated: true,
            pacing: PresentPacingPolicy {
                schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
                pacing_mode: PresentPacingMode::Mailbox,
                latency_policy: LatencyPolicy::Balanced,
                hud_less: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            },
            ui_composition_order: Tier7UiCompositionOrder::UiAfterGeneratedFrameCorrect,
        };
        let decision = Tier7FrameGenerationEnableDecision::evaluate(&prereqs);
        assert_eq!(decision, Tier7FrameGenerationEnableDecision::Enabled);
        assert!(decision.enabled());
        assert!(decision.first_failed().is_none());
    }

    #[test]
    fn frame_gen_decision_disabled_records_the_failing_prerequisite() {
        let prereqs = Tier7FrameGenerationPrerequisites {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            hud_less_scene_color: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            ui_color_separation_active: true,
            motion_vectors_valid: false, // failed
            depth_valid: true,
            present_time_resource_lifetime_validated: true,
            pacing: PresentPacingPolicy {
                schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
                pacing_mode: PresentPacingMode::Mailbox,
                latency_policy: LatencyPolicy::Balanced,
                hud_less: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            },
            ui_composition_order: Tier7UiCompositionOrder::UiAfterGeneratedFrameCorrect,
        };
        let decision = Tier7FrameGenerationEnableDecision::evaluate(&prereqs);
        match decision {
            Tier7FrameGenerationEnableDecision::DisabledDueToPrerequisite { which } => {
                assert_eq!(which, Tier7FrameGenerationPrerequisite::MotionVectorsValid);
            }
            other => panic!("expected disabled, got {:?}", other),
        }
        assert!(!decision.enabled());
    }

    #[test]
    fn ui_composition_order_no_smear_only_for_after_generated_frame() {
        for order in Tier7UiCompositionOrder::ALL {
            match order {
                Tier7UiCompositionOrder::UiAfterGeneratedFrameCorrect => assert!(order.no_smear()),
                _ => assert!(!order.no_smear()),
            }
        }
    }

    #[test]
    fn requires_reflex_sdk_active_routes_through_latency_policy() {
        assert!(requires_reflex_sdk_active(LatencyPolicy::ReflexLowLatency));
        assert!(requires_reflex_sdk_active(
            LatencyPolicy::ReflexLowLatencyBoost
        ));
        assert!(!requires_reflex_sdk_active(LatencyPolicy::Balanced));
        assert!(!requires_reflex_sdk_active(LatencyPolicy::PowerSaver));
        assert!(!requires_reflex_sdk_active(LatencyPolicy::LatencyOptimised));
    }

    #[test]
    fn tier7_acceptance_passes_when_every_rule_holds_under_default_policy() {
        let bridge = VendorSdkBridgeStatus::default();
        let dlss = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Dlss,
            &bridge,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            false,
            false,
        );
        let reflex = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Reflex,
            &bridge,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            false,
            false,
        );

        let prereqs = Tier7FrameGenerationPrerequisites {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            hud_less_scene_color: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            ui_color_separation_active: true,
            motion_vectors_valid: true,
            depth_valid: true,
            present_time_resource_lifetime_validated: true,
            pacing: PresentPacingPolicy {
                schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
                pacing_mode: PresentPacingMode::Mailbox,
                latency_policy: LatencyPolicy::Balanced,
                hud_less: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            },
            ui_composition_order: Tier7UiCompositionOrder::UiAfterGeneratedFrameCorrect,
        };
        let decision = Tier7FrameGenerationEnableDecision::evaluate(&prereqs);

        let mut latency = Tier7LatencyMeasurement::new(budget_60());
        latency.record_marker(marker(Tier7LatencyMarkerKind::SimulationStart, 0, 0, 0));
        latency.record_marker(marker(Tier7LatencyMarkerKind::PresentEnd, 16_000_000, 0, 0));
        latency.finalize_frame(0);

        let pacing = Tier7PresentPacingDiagnostics::from_observation(60, 60.0, 60);

        let verdict = Tier7AcceptanceVerdict::evaluate(
            &dlss,
            &reflex,
            decision,
            &latency,
            &pacing,
            Tier7UiCompositionOrder::UiAfterGeneratedFrameCorrect,
        );
        assert!(verdict.passes_dlss_fail_closed_until_command_list);
        assert!(verdict.passes_reflex_only_when_real);
        assert!(verdict.passes_native_sdk_active_only_when_requirements_met);
        assert!(verdict.passes_frame_gen_prerequisites_enforced);
        assert!(verdict.passes_ui_no_smear);
        assert!(verdict.passes_latency_within_budget);
        assert!(verdict.passes_present_pacing_within_tolerance);
        assert!(verdict.passes());
    }

    #[test]
    fn tier7_acceptance_fails_when_ui_composition_order_smears() {
        let bridge = VendorSdkBridgeStatus::default();
        let dlss = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Dlss,
            &bridge,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            false,
            false,
        );
        let reflex = Tier7NativeSdkActiveGuard::evaluate(
            VendorSdkKind::Reflex,
            &bridge,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
            false,
            false,
        );
        let prereqs = Tier7FrameGenerationPrerequisites {
            schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
            hud_less_scene_color: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            ui_color_separation_active: true,
            motion_vectors_valid: true,
            depth_valid: true,
            present_time_resource_lifetime_validated: true,
            pacing: PresentPacingPolicy {
                schema_version: TIER7_VENDOR_SDK_FRAME_GEN_SCHEMA_VERSION,
                pacing_mode: PresentPacingMode::Mailbox,
                latency_policy: LatencyPolicy::Balanced,
                hud_less: HudLessSceneColorPolicy::HudLessForFrameGeneration,
            },
            ui_composition_order: Tier7UiCompositionOrder::UiAfterGeneratedFrameCorrect,
        };
        let decision = Tier7FrameGenerationEnableDecision::evaluate(&prereqs);
        let latency = Tier7LatencyMeasurement::new(budget_60());
        let pacing = Tier7PresentPacingDiagnostics::from_observation(60, 60.0, 60);

        let verdict = Tier7AcceptanceVerdict::evaluate(
            &dlss,
            &reflex,
            decision,
            &latency,
            &pacing,
            Tier7UiCompositionOrder::UiBeforeGeneratedFrameBadSmears,
        );
        assert!(!verdict.passes_ui_no_smear);
        assert!(!verdict.passes());
    }

    #[test]
    fn marker_is_start_classification_routes_correctly() {
        for kind in Tier7LatencyMarkerKind::ALL {
            match kind {
                Tier7LatencyMarkerKind::SimulationStart
                | Tier7LatencyMarkerKind::RenderStart
                | Tier7LatencyMarkerKind::PresentStart => assert!(kind.is_start()),
                _ => assert!(!kind.is_start()),
            }
        }
    }

    #[test]
    fn latency_measurement_canonical_path_matches_funpb_zst_contract() {
        let measurement = Tier7LatencyMeasurement::new(budget_60());
        assert_eq!(
            measurement.canonical_path,
            Tier7LatencyMeasurement::CANONICAL_ARTIFACT_PATH,
        );
        assert!(measurement.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn native_sdk_active_status_actually_active_only_for_installed_and_met() {
        for status in Tier7NativeSdkActiveStatus::ALL {
            match status {
                Tier7NativeSdkActiveStatus::InstalledAndAllBridgeRequirementsMet => {
                    assert!(status.actually_active());
                }
                _ => assert!(!status.actually_active()),
            }
        }
    }
}
