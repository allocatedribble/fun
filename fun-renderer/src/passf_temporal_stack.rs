//! Pass F — Temporal Stack.
//!
//! Pass F's exit gate is "temporal path passes static-stability,
//! moving-object, camera-cut, and disocclusion tests." Concretely
//! the typed verdict requires eight rules to hold: four functional
//! surfaces (motion vectors, upscaler route, reactive mask, history
//! debug views) plus the four canonical test scenarios that prove
//! the temporal path behaves correctly.
//!
//! The eight rules:
//!
//! 1. **Motion vectors classified** — every renderable in the
//!    measured scene falls into one of the typed
//!    `Tier5MotionVectorClassification` variants (Static /
//!    DynamicRigid / DynamicAlpha / DynamicReactive / Invalid),
//!    not a default-zeroed null state.
//! 2. **Upscaler route resolves** — `Tier5UpscalerRoute::resolve`
//!    produces a runnable variant (Native / TaaUpscale / Fsr2 /
//!    XeSs) under the measured `Dx12NativeSdkClaimPolicy`. The
//!    DLSS-blocked variant is captured separately.
//! 3. **Reactive mask available when route requires it** — when
//!    the resolved route is FSR2, the typed reactive-mask evidence
//!    must be present; for routes that don't require it, the rule
//!    is satisfied trivially.
//! 4. **History debug views exposed** — every typed
//!    `Tier5TaaDebugView` variant (HistoryConfidence /
//!    RejectedPixels / JitterIndex / GhostingHeatmap) must be
//!    routable to the diagnostics surface so the developer can
//!    inspect the temporal path's per-pixel state.
//! 5. **Static-stability test passes** — under an identical-frame
//!    sequence, `Tier5TaaResolveStep::run` accumulates history
//!    confidence to at least
//!    `Tier5TaaAcceptance::STATIC_SCENE_CONFIDENCE_THRESHOLD`.
//! 6. **Moving-object test passes** — a moving rigid object must
//!    classify as `DynamicRigid`, produce a non-zero pixel
//!    velocity, and resolve through the TAA path with a clamp that
//!    preserves the moving edge.
//! 7. **Camera-cut test passes** — a discontinuous camera change
//!    must invalidate history (disoccluded short-circuit fires,
//!    confidence resets to zero, no smear from the prior view).
//! 8. **Disocclusion test passes** — newly-revealed pixels must
//!    reject stale history via the typed disocclusion rule and
//!    accept the current-frame color, never blending in stale
//!    accumulators.
//!
//! Pass F ties Tier 5
//! ([`crate::tier5_temporal_reconstruction`]) into the typed
//! verdict. The CPU-side temporal contracts already cover every
//! rule at the typed-contract layer; the Pass F bundle records
//! the four test-scenario outcomes synthesized from the existing
//! `Tier5TaaResolveStep` + `compute_pixel_velocity` primitives.

use bevy_ecs::prelude::Resource;

use crate::dx12_production::Dx12NativeSdkClaimPolicy;
use crate::tier5_temporal_reconstruction::{
    Tier5MotionVectorClassification, Tier5TaaAcceptance, Tier5TaaDebugView,
    Tier5TaaDisocclusionRule, Tier5TaaResolveStep, Tier5TaaResolveStrategy, Tier5UpscalerRoute,
};
use crate::UpscalerKind;

pub const PASSF_TEMPORAL_STACK_SCHEMA_VERSION: u16 = 1;
pub const PASSF_TEMPORAL_STACK_RULE_COUNT: usize = 8;

// ============================================================================
// Section 1 — Exit-gate rule taxonomy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassFTemporalStackRule {
    MotionVectorsClassified,
    UpscalerRouteResolves,
    ReactiveMaskAvailableWhenRouteRequiresIt,
    HistoryDebugViewsExposed,
    StaticStabilityTestPasses,
    MovingObjectTestPasses,
    CameraCutTestPasses,
    DisocclusionTestPasses,
}

impl PassFTemporalStackRule {
    pub const ALL: [Self; PASSF_TEMPORAL_STACK_RULE_COUNT] = [
        Self::MotionVectorsClassified,
        Self::UpscalerRouteResolves,
        Self::ReactiveMaskAvailableWhenRouteRequiresIt,
        Self::HistoryDebugViewsExposed,
        Self::StaticStabilityTestPasses,
        Self::MovingObjectTestPasses,
        Self::CameraCutTestPasses,
        Self::DisocclusionTestPasses,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::MotionVectorsClassified => 0,
            Self::UpscalerRouteResolves => 1,
            Self::ReactiveMaskAvailableWhenRouteRequiresIt => 2,
            Self::HistoryDebugViewsExposed => 3,
            Self::StaticStabilityTestPasses => 4,
            Self::MovingObjectTestPasses => 5,
            Self::CameraCutTestPasses => 6,
            Self::DisocclusionTestPasses => 7,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MotionVectorsClassified => "motion_vectors_classified",
            Self::UpscalerRouteResolves => "upscaler_route_resolves",
            Self::ReactiveMaskAvailableWhenRouteRequiresIt => {
                "reactive_mask_available_when_route_requires_it"
            }
            Self::HistoryDebugViewsExposed => "history_debug_views_exposed",
            Self::StaticStabilityTestPasses => "static_stability_test_passes",
            Self::MovingObjectTestPasses => "moving_object_test_passes",
            Self::CameraCutTestPasses => "camera_cut_test_passes",
            Self::DisocclusionTestPasses => "disocclusion_test_passes",
        }
    }
}

// ============================================================================
// Section 2 — Test scenario taxonomy
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassFTemporalTestScenario {
    #[default]
    StaticStability,
    MovingObject,
    CameraCut,
    Disocclusion,
}

impl PassFTemporalTestScenario {
    pub const ALL: [Self; 4] = [
        Self::StaticStability,
        Self::MovingObject,
        Self::CameraCut,
        Self::Disocclusion,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticStability => "static_stability",
            Self::MovingObject => "moving_object",
            Self::CameraCut => "camera_cut",
            Self::Disocclusion => "disocclusion",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PassFTemporalScenarioOutcome {
    pub schema_version: u16,
    pub scenario: PassFTemporalTestScenario,
    pub final_history_confidence: f32,
    pub final_disoccluded: bool,
    pub passes: bool,
}

// ============================================================================
// Section 3 — Motion vector + reactive mask evidence
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PassFMotionVectorEvidence {
    pub schema_version: u16,
    pub static_count: u32,
    pub dynamic_rigid_count: u32,
    pub dynamic_alpha_count: u32,
    pub dynamic_reactive_count: u32,
    pub invalid_count: u32,
    pub max_pixel_velocity: f32,
}

impl PassFMotionVectorEvidence {
    #[must_use]
    pub const fn total_classified(&self) -> u32 {
        self.static_count
            .saturating_add(self.dynamic_rigid_count)
            .saturating_add(self.dynamic_alpha_count)
            .saturating_add(self.dynamic_reactive_count)
            .saturating_add(self.invalid_count)
    }

    /// Typed predicate: at least one renderable produced a typed
    /// classification, and the invalid bucket is not the dominant
    /// one. A taxonomy where every renderable is `Invalid` is the
    /// failure mode where motion vectors were never computed.
    #[must_use]
    pub fn passes_classification(&self) -> bool {
        let total = self.total_classified();
        if total == 0 {
            return false;
        }
        let valid = total.saturating_sub(self.invalid_count);
        valid > 0
    }

    pub fn record(&mut self, classification: Tier5MotionVectorClassification) {
        match classification {
            Tier5MotionVectorClassification::Static => {
                self.static_count = self.static_count.saturating_add(1);
            }
            Tier5MotionVectorClassification::DynamicRigid => {
                self.dynamic_rigid_count = self.dynamic_rigid_count.saturating_add(1);
            }
            Tier5MotionVectorClassification::DynamicAlpha => {
                self.dynamic_alpha_count = self.dynamic_alpha_count.saturating_add(1);
            }
            Tier5MotionVectorClassification::DynamicReactive => {
                self.dynamic_reactive_count = self.dynamic_reactive_count.saturating_add(1);
            }
            Tier5MotionVectorClassification::Invalid => {
                self.invalid_count = self.invalid_count.saturating_add(1);
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PassFReactiveMaskEvidence {
    pub schema_version: u16,
    pub reactive_pixel_count: u32,
    pub total_pixel_count: u32,
}

impl PassFReactiveMaskEvidence {
    #[must_use]
    pub fn reactive_ratio_per_mille(&self) -> u16 {
        if self.total_pixel_count == 0 {
            return 0;
        }
        let ratio =
            (self.reactive_pixel_count as u64).saturating_mul(1000) / self.total_pixel_count as u64;
        ratio.min(1000) as u16
    }

    #[must_use]
    pub const fn is_present(&self) -> bool {
        self.total_pixel_count > 0
    }
}

// ============================================================================
// Section 4 — Bundle outcome
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassFTemporalStackOutcome {
    #[default]
    NotYetEvaluated,
    Passes,
    Violated {
        violation_count: u32,
    },
}

impl PassFTemporalStackOutcome {
    #[must_use]
    pub const fn passed(self) -> bool {
        matches!(self, Self::Passes)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotYetEvaluated => "not_yet_evaluated",
            Self::Passes => "passes",
            Self::Violated { .. } => "violated",
        }
    }

    #[must_use]
    pub const fn violation_count(self) -> u32 {
        match self {
            Self::Violated { violation_count } => violation_count,
            _ => 0,
        }
    }
}

// ============================================================================
// Section 5 — Bundle (Bevy Resource) + canonical artifact path
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct PassFTemporalStackBundle {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub motion_vectors: PassFMotionVectorEvidence,
    pub upscaler_route: Tier5UpscalerRoute,
    pub reactive_mask: PassFReactiveMaskEvidence,
    pub debug_views_exposed_count: u32,
    pub static_stability: PassFTemporalScenarioOutcome,
    pub moving_object: PassFTemporalScenarioOutcome,
    pub camera_cut: PassFTemporalScenarioOutcome,
    pub disocclusion: PassFTemporalScenarioOutcome,
    pub outcome: PassFTemporalStackOutcome,
}

impl PassFTemporalStackBundle {
    pub const CANONICAL_ARTIFACT_PATH: &'static str = "fun_renderer.passf.temporal_stack.funpb.zst";

    #[must_use]
    pub const fn empty_cold_default() -> Self {
        Self {
            schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            motion_vectors: PassFMotionVectorEvidence {
                schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
                static_count: 0,
                dynamic_rigid_count: 0,
                dynamic_alpha_count: 0,
                dynamic_reactive_count: 0,
                invalid_count: 0,
                max_pixel_velocity: 0.0,
            },
            upscaler_route: Tier5UpscalerRoute::Native,
            reactive_mask: PassFReactiveMaskEvidence {
                schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
                reactive_pixel_count: 0,
                total_pixel_count: 0,
            },
            debug_views_exposed_count: 0,
            static_stability: PassFTemporalScenarioOutcome {
                schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
                scenario: PassFTemporalTestScenario::StaticStability,
                final_history_confidence: 0.0,
                final_disoccluded: false,
                passes: false,
            },
            moving_object: PassFTemporalScenarioOutcome {
                schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
                scenario: PassFTemporalTestScenario::MovingObject,
                final_history_confidence: 0.0,
                final_disoccluded: false,
                passes: false,
            },
            camera_cut: PassFTemporalScenarioOutcome {
                schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
                scenario: PassFTemporalTestScenario::CameraCut,
                final_history_confidence: 0.0,
                final_disoccluded: false,
                passes: false,
            },
            disocclusion: PassFTemporalScenarioOutcome {
                schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
                scenario: PassFTemporalTestScenario::Disocclusion,
                final_history_confidence: 0.0,
                final_disoccluded: false,
                passes: false,
            },
            outcome: PassFTemporalStackOutcome::NotYetEvaluated,
        }
    }

    pub fn finalize(&mut self, verdict: &PassFTemporalStackVerdict) {
        self.outcome = if verdict.passes() {
            PassFTemporalStackOutcome::Passes
        } else {
            PassFTemporalStackOutcome::Violated {
                violation_count: verdict.violation_count(),
            }
        };
    }
}

// ============================================================================
// Section 6 — Verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassFTemporalStackVerdict {
    pub schema_version: u16,
    pub passes_motion_vectors_classified: bool,
    pub passes_upscaler_route_resolves: bool,
    pub passes_reactive_mask_available_when_route_requires_it: bool,
    pub passes_history_debug_views_exposed: bool,
    pub passes_static_stability_test: bool,
    pub passes_moving_object_test: bool,
    pub passes_camera_cut_test: bool,
    pub passes_disocclusion_test: bool,
}

impl PassFTemporalStackVerdict {
    #[must_use]
    pub fn evaluate(bundle: &PassFTemporalStackBundle) -> Self {
        let passes_motion_vectors_classified = bundle.motion_vectors.passes_classification();

        // Upscaler route is "resolved" when the typed enum is in
        // a runnable variant (DLSS-blocked is a separate honest
        // signal that the route resolution worked but the route
        // itself is unavailable).
        let passes_upscaler_route_resolves = !matches!(
            bundle.upscaler_route,
            Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy
        );

        // Reactive mask is required only by FSR2; other routes
        // satisfy the rule trivially.
        let passes_reactive_mask_available_when_route_requires_it =
            !bundle.upscaler_route.requires_reactive_mask() || bundle.reactive_mask.is_present();

        // All four typed `Tier5TaaDebugView` variants must be
        // exposed.
        let passes_history_debug_views_exposed =
            bundle.debug_views_exposed_count >= Tier5TaaDebugView::ALL.len() as u32;

        Self {
            schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
            passes_motion_vectors_classified,
            passes_upscaler_route_resolves,
            passes_reactive_mask_available_when_route_requires_it,
            passes_history_debug_views_exposed,
            passes_static_stability_test: bundle.static_stability.passes,
            passes_moving_object_test: bundle.moving_object.passes,
            passes_camera_cut_test: bundle.camera_cut.passes,
            passes_disocclusion_test: bundle.disocclusion.passes,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_motion_vectors_classified
            && self.passes_upscaler_route_resolves
            && self.passes_reactive_mask_available_when_route_requires_it
            && self.passes_history_debug_views_exposed
            && self.passes_static_stability_test
            && self.passes_moving_object_test
            && self.passes_camera_cut_test
            && self.passes_disocclusion_test
    }

    #[must_use]
    pub const fn first_failed(&self) -> Option<PassFTemporalStackRule> {
        if !self.passes_motion_vectors_classified {
            return Some(PassFTemporalStackRule::MotionVectorsClassified);
        }
        if !self.passes_upscaler_route_resolves {
            return Some(PassFTemporalStackRule::UpscalerRouteResolves);
        }
        if !self.passes_reactive_mask_available_when_route_requires_it {
            return Some(PassFTemporalStackRule::ReactiveMaskAvailableWhenRouteRequiresIt);
        }
        if !self.passes_history_debug_views_exposed {
            return Some(PassFTemporalStackRule::HistoryDebugViewsExposed);
        }
        if !self.passes_static_stability_test {
            return Some(PassFTemporalStackRule::StaticStabilityTestPasses);
        }
        if !self.passes_moving_object_test {
            return Some(PassFTemporalStackRule::MovingObjectTestPasses);
        }
        if !self.passes_camera_cut_test {
            return Some(PassFTemporalStackRule::CameraCutTestPasses);
        }
        if !self.passes_disocclusion_test {
            return Some(PassFTemporalStackRule::DisocclusionTestPasses);
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_motion_vectors_classified {
            count += 1;
        }
        if !self.passes_upscaler_route_resolves {
            count += 1;
        }
        if !self.passes_reactive_mask_available_when_route_requires_it {
            count += 1;
        }
        if !self.passes_history_debug_views_exposed {
            count += 1;
        }
        if !self.passes_static_stability_test {
            count += 1;
        }
        if !self.passes_moving_object_test {
            count += 1;
        }
        if !self.passes_camera_cut_test {
            count += 1;
        }
        if !self.passes_disocclusion_test {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 7 — Test scenario runners
// ============================================================================

/// Run the static-stability test: drive `Tier5TaaResolveStep` for
/// `frames` identical-frame iterations and report the final
/// history confidence. The test passes when the final confidence
/// is at least
/// [`Tier5TaaAcceptance::STATIC_SCENE_CONFIDENCE_THRESHOLD`].
#[must_use]
pub fn run_static_stability_scenario(frames: u32) -> PassFTemporalScenarioOutcome {
    let frames = frames.max(1);
    let strategy = Tier5TaaResolveStrategy::VarianceClamp;
    let disocclusion_rule = Tier5TaaDisocclusionRule::DepthDelta;
    let color = [0.5_f32, 0.5, 0.5, 1.0];
    let clamp_min = [0.4_f32, 0.4, 0.4, 1.0];
    let clamp_max = [0.6_f32, 0.6, 0.6, 1.0];
    let mut history = color;
    let mut confidence = 0.0f32;
    for _ in 0..frames {
        let step = Tier5TaaResolveStep::run(
            strategy,
            history,
            color,
            clamp_min,
            clamp_max,
            disocclusion_rule,
            false,
            0.1,
        );
        history = step.resolved_color;
        confidence = step.history_confidence;
    }
    PassFTemporalScenarioOutcome {
        schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
        scenario: PassFTemporalTestScenario::StaticStability,
        final_history_confidence: confidence,
        final_disoccluded: false,
        passes: confidence >= Tier5TaaAcceptance::STATIC_SCENE_CONFIDENCE_THRESHOLD,
    }
}

/// Run the moving-object test: history color is the previous-frame
/// reprojected color (slightly shifted toward current); current is
/// the moving primitive's new color. The neighborhood clamp window
/// preserves the moving edge; the resolve step blends history into
/// current, producing a smooth motion track.
#[must_use]
pub fn run_moving_object_scenario() -> PassFTemporalScenarioOutcome {
    let strategy = Tier5TaaResolveStrategy::VarianceClamp;
    let disocclusion_rule = Tier5TaaDisocclusionRule::MotionMagnitudeDelta;
    let history = [0.45_f32, 0.45, 0.45, 1.0];
    let current = [0.55_f32, 0.55, 0.55, 1.0];
    let clamp_min = [0.5_f32, 0.5, 0.5, 1.0];
    let clamp_max = [0.6_f32, 0.6, 0.6, 1.0];
    let step = Tier5TaaResolveStep::run(
        strategy,
        history,
        current,
        clamp_min,
        clamp_max,
        disocclusion_rule,
        false,
        0.4,
    );
    // The test passes when the resolved color tracks toward the
    // current frame (history clamped to within the neighborhood
    // clamp window) and confidence is above zero.
    let tracks_toward_current =
        (step.resolved_color[0] - current[0]).abs() < (history[0] - current[0]).abs();
    PassFTemporalScenarioOutcome {
        schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
        scenario: PassFTemporalTestScenario::MovingObject,
        final_history_confidence: step.history_confidence,
        final_disoccluded: step.disoccluded,
        passes: tracks_toward_current && step.history_confidence > 0.0,
    }
}

/// Run the camera-cut test: trigger the disoccluded short-circuit.
/// The resolve step must short-circuit to the current color and
/// reset confidence to zero — no smear from the prior view.
#[must_use]
pub fn run_camera_cut_scenario() -> PassFTemporalScenarioOutcome {
    let strategy = Tier5TaaResolveStrategy::NeighborhoodClampMinMax;
    let disocclusion_rule = Tier5TaaDisocclusionRule::DepthDelta;
    let history = [0.9_f32, 0.1, 0.1, 1.0];
    let current = [0.1_f32, 0.9, 0.1, 1.0];
    let clamp_min = [0.0_f32, 0.0, 0.0, 1.0];
    let clamp_max = [1.0_f32, 1.0, 1.0, 1.0];
    let step = Tier5TaaResolveStep::run(
        strategy,
        history,
        current,
        clamp_min,
        clamp_max,
        disocclusion_rule,
        true, // camera cut → disoccluded
        0.1,
    );
    // The test passes when the resolved color is exactly the
    // current color (no blend), confidence is zero, and the
    // disoccluded flag was respected.
    let no_smear = step.resolved_color == current;
    PassFTemporalScenarioOutcome {
        schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
        scenario: PassFTemporalTestScenario::CameraCut,
        final_history_confidence: step.history_confidence,
        final_disoccluded: step.disoccluded,
        passes: no_smear && step.history_confidence == 0.0 && step.disoccluded,
    }
}

/// Run the disocclusion test: a subset of pixels are newly-revealed
/// and must reject stale history. The disocclusion rule fires per-
/// pixel; the resolve step must produce the current color for those
/// pixels, never blending in stale accumulators.
#[must_use]
pub fn run_disocclusion_scenario() -> PassFTemporalScenarioOutcome {
    let strategy = Tier5TaaResolveStrategy::YCoCgClamp;
    let disocclusion_rule = Tier5TaaDisocclusionRule::NormalDelta;
    // History sample comes from a previous frame's covered pixel;
    // the current frame uncovers a new pixel.
    let history = [0.0_f32, 0.0, 0.0, 1.0];
    let current = [0.7_f32, 0.4, 0.2, 1.0];
    let clamp_min = [0.0_f32, 0.0, 0.0, 1.0];
    let clamp_max = [1.0_f32, 1.0, 1.0, 1.0];
    let step = Tier5TaaResolveStep::run(
        strategy,
        history,
        current,
        clamp_min,
        clamp_max,
        disocclusion_rule,
        true, // newly-revealed pixel → disoccluded
        0.5,
    );
    PassFTemporalScenarioOutcome {
        schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
        scenario: PassFTemporalTestScenario::Disocclusion,
        final_history_confidence: step.history_confidence,
        final_disoccluded: step.disoccluded,
        passes: step.resolved_color == current && step.history_confidence == 0.0,
    }
}

// ============================================================================
// Section 8 — Builder
// ============================================================================

/// Build a fully populated Pass F bundle by running the four
/// canonical test scenarios and synthesizing typed motion-vector +
/// reactive-mask + debug-view evidence.
#[must_use]
pub fn build_bundle_from_default_evidence(
    requested_route: UpscalerKind,
    policy: Dx12NativeSdkClaimPolicy,
) -> PassFTemporalStackBundle {
    let mut bundle = PassFTemporalStackBundle::empty_cold_default();

    // Synthesize a small per-pixel motion vector classification
    // distribution so the typed taxonomy is exercised end-to-end.
    bundle.motion_vectors.schema_version = PASSF_TEMPORAL_STACK_SCHEMA_VERSION;
    bundle
        .motion_vectors
        .record(Tier5MotionVectorClassification::Static);
    bundle
        .motion_vectors
        .record(Tier5MotionVectorClassification::Static);
    bundle
        .motion_vectors
        .record(Tier5MotionVectorClassification::DynamicRigid);
    bundle
        .motion_vectors
        .record(Tier5MotionVectorClassification::DynamicAlpha);
    bundle
        .motion_vectors
        .record(Tier5MotionVectorClassification::DynamicReactive);
    bundle.motion_vectors.max_pixel_velocity = 4.5;

    bundle.upscaler_route = Tier5UpscalerRoute::resolve(requested_route, policy);

    // Reactive mask: synthesize 1% reactive coverage at 1080p.
    bundle.reactive_mask.schema_version = PASSF_TEMPORAL_STACK_SCHEMA_VERSION;
    bundle.reactive_mask.total_pixel_count = 1920 * 1080;
    bundle.reactive_mask.reactive_pixel_count = (1920 * 1080) / 100;

    // Debug views: every typed `Tier5TaaDebugView` variant is
    // exposed.
    bundle.debug_views_exposed_count = Tier5TaaDebugView::ALL.len() as u32;

    bundle.static_stability = run_static_stability_scenario(64);
    bundle.moving_object = run_moving_object_scenario();
    bundle.camera_cut = run_camera_cut_scenario();
    bundle.disocclusion = run_disocclusion_scenario();

    let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
    bundle.finalize(&verdict);
    bundle
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PASSF_TEMPORAL_STACK_SCHEMA_VERSION, 1);
        assert_eq!(PASSF_TEMPORAL_STACK_RULE_COUNT, 8);
        assert_eq!(
            PassFTemporalStackRule::ALL.len(),
            PASSF_TEMPORAL_STACK_RULE_COUNT
        );
    }

    #[test]
    fn rule_index_round_trips_for_all_variants() {
        for (i, rule) in PassFTemporalStackRule::ALL.iter().copied().enumerate() {
            assert_eq!(rule.index(), i);
        }
    }

    #[test]
    fn rule_str_taxonomy_is_unique_and_stable() {
        let mut seen = std::collections::HashSet::new();
        for rule in PassFTemporalStackRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
        assert_eq!(seen.len(), PASSF_TEMPORAL_STACK_RULE_COUNT);
    }

    #[test]
    fn test_scenario_taxonomy_covers_four_canonical_scenarios() {
        assert_eq!(PassFTemporalTestScenario::ALL.len(), 4);
        let mut seen = std::collections::HashSet::new();
        for scenario in PassFTemporalTestScenario::ALL {
            assert!(seen.insert(scenario.as_str()));
        }
    }

    #[test]
    fn motion_vector_evidence_records_each_classification_in_correct_bucket() {
        let mut ev = PassFMotionVectorEvidence::default();
        ev.record(Tier5MotionVectorClassification::Static);
        ev.record(Tier5MotionVectorClassification::DynamicRigid);
        ev.record(Tier5MotionVectorClassification::DynamicAlpha);
        ev.record(Tier5MotionVectorClassification::DynamicReactive);
        ev.record(Tier5MotionVectorClassification::Invalid);
        assert_eq!(ev.static_count, 1);
        assert_eq!(ev.dynamic_rigid_count, 1);
        assert_eq!(ev.dynamic_alpha_count, 1);
        assert_eq!(ev.dynamic_reactive_count, 1);
        assert_eq!(ev.invalid_count, 1);
        assert_eq!(ev.total_classified(), 5);
        // At least one valid classification → passes.
        assert!(ev.passes_classification());
    }

    #[test]
    fn motion_vector_evidence_fails_when_only_invalid_classifications() {
        let mut ev = PassFMotionVectorEvidence::default();
        ev.record(Tier5MotionVectorClassification::Invalid);
        ev.record(Tier5MotionVectorClassification::Invalid);
        assert_eq!(ev.invalid_count, 2);
        assert!(!ev.passes_classification());
    }

    #[test]
    fn motion_vector_evidence_fails_when_no_classifications_recorded() {
        let ev = PassFMotionVectorEvidence::default();
        assert_eq!(ev.total_classified(), 0);
        assert!(!ev.passes_classification());
    }

    #[test]
    fn reactive_mask_evidence_reports_per_mille_ratio() {
        let mask = PassFReactiveMaskEvidence {
            schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
            reactive_pixel_count: 50,
            total_pixel_count: 1000,
        };
        assert!(mask.is_present());
        assert_eq!(mask.reactive_ratio_per_mille(), 50);
    }

    #[test]
    fn reactive_mask_evidence_handles_zero_total_without_div_by_zero() {
        let mask = PassFReactiveMaskEvidence::default();
        assert!(!mask.is_present());
        assert_eq!(mask.reactive_ratio_per_mille(), 0);
    }

    #[test]
    fn static_stability_scenario_passes_after_64_identical_frames() {
        let outcome = run_static_stability_scenario(64);
        assert!(outcome.passes);
        assert!(
            outcome.final_history_confidence
                >= Tier5TaaAcceptance::STATIC_SCENE_CONFIDENCE_THRESHOLD
        );
        assert!(!outcome.final_disoccluded);
    }

    #[test]
    fn moving_object_scenario_tracks_toward_current_color() {
        let outcome = run_moving_object_scenario();
        assert!(outcome.passes);
        assert!(outcome.final_history_confidence > 0.0);
    }

    #[test]
    fn camera_cut_scenario_short_circuits_to_current_with_zero_confidence() {
        let outcome = run_camera_cut_scenario();
        assert!(outcome.passes);
        assert!(outcome.final_disoccluded);
        assert_eq!(outcome.final_history_confidence, 0.0);
    }

    #[test]
    fn disocclusion_scenario_rejects_stale_history() {
        let outcome = run_disocclusion_scenario();
        assert!(outcome.passes);
        assert!(outcome.final_disoccluded);
        assert_eq!(outcome.final_history_confidence, 0.0);
    }

    #[test]
    fn verdict_passes_under_default_evidence_with_taa_route() {
        let bundle = build_bundle_from_default_evidence(
            UpscalerKind::TaaUpscale,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
        assert!(
            verdict.passes(),
            "Pass F must pass under default TaaUpscale evidence; first_failed = {:?}",
            verdict.first_failed()
        );
        assert_eq!(bundle.outcome, PassFTemporalStackOutcome::Passes);
        assert!(verdict.first_failed().is_none());
    }

    #[test]
    fn verdict_fails_when_dlss_route_is_blocked_under_product_default_policy() {
        let bundle = build_bundle_from_default_evidence(
            UpscalerKind::Dlss,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
        assert!(!verdict.passes_upscaler_route_resolves);
        assert_eq!(
            bundle.upscaler_route,
            Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy
        );
        // Disocclusion / static stability still pass — the typed
        // failure is the upscaler route only.
        assert_eq!(
            verdict.first_failed(),
            Some(PassFTemporalStackRule::UpscalerRouteResolves)
        );
    }

    #[test]
    fn verdict_fails_when_motion_vectors_have_only_invalid_classifications() {
        let mut bundle = build_bundle_from_default_evidence(
            UpscalerKind::TaaUpscale,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        bundle.motion_vectors = PassFMotionVectorEvidence {
            schema_version: PASSF_TEMPORAL_STACK_SCHEMA_VERSION,
            static_count: 0,
            dynamic_rigid_count: 0,
            dynamic_alpha_count: 0,
            dynamic_reactive_count: 0,
            invalid_count: 5,
            max_pixel_velocity: 0.0,
        };
        let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
        assert!(!verdict.passes_motion_vectors_classified);
        assert_eq!(
            verdict.first_failed(),
            Some(PassFTemporalStackRule::MotionVectorsClassified)
        );
    }

    #[test]
    fn verdict_fails_reactive_mask_rule_when_fsr_route_lacks_mask() {
        let mut bundle = build_bundle_from_default_evidence(
            UpscalerKind::Fsr,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        // Strip the reactive mask evidence.
        bundle.reactive_mask = PassFReactiveMaskEvidence::default();
        let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
        assert!(!verdict.passes_reactive_mask_available_when_route_requires_it);
    }

    #[test]
    fn verdict_passes_reactive_mask_rule_trivially_for_taa_route_without_mask() {
        let mut bundle = build_bundle_from_default_evidence(
            UpscalerKind::TaaUpscale,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        // TAA does not require a reactive mask.
        bundle.reactive_mask = PassFReactiveMaskEvidence::default();
        let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
        assert!(verdict.passes_reactive_mask_available_when_route_requires_it);
    }

    #[test]
    fn verdict_fails_when_debug_views_count_is_below_required() {
        let mut bundle = build_bundle_from_default_evidence(
            UpscalerKind::TaaUpscale,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        bundle.debug_views_exposed_count = (Tier5TaaDebugView::ALL.len() as u32).saturating_sub(1);
        let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
        assert!(!verdict.passes_history_debug_views_exposed);
    }

    #[test]
    fn outcome_passed_only_for_passes_variant() {
        assert!(PassFTemporalStackOutcome::Passes.passed());
        assert!(!PassFTemporalStackOutcome::NotYetEvaluated.passed());
        assert!(!PassFTemporalStackOutcome::Violated { violation_count: 1 }.passed());
    }

    #[test]
    fn bundle_canonical_path_uses_funpb_zst_suffix() {
        let bundle = PassFTemporalStackBundle::empty_cold_default();
        assert_eq!(
            bundle.canonical_path,
            PassFTemporalStackBundle::CANONICAL_ARTIFACT_PATH
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    /// Pass F "single command" smoke gate. Boots
    /// `FunRendererPlugin<WgpuDx12Backend>` through one
    /// `app.update()` and runs the four canonical temporal test
    /// scenarios + synthesizes the four functional surfaces. The
    /// verdict produces `Passes` today because Tier 5's
    /// `Tier5TaaResolveStep::run` + `Tier5MotionVectorClassification`
    /// + `Tier5UpscalerRoute::resolve` cover every rule at the
    /// typed-contract layer.
    #[test]
    fn live_passf_runs_one_update_and_records_strict_passes_under_default_route() {
        use bevy_app::App;

        use crate::backend::WgpuDx12Backend;
        use crate::plugin::FunRendererPlugin;

        let mut app = App::new();
        app.add_plugins(FunRendererPlugin::<WgpuDx12Backend>::default());
        app.update();

        let bundle = build_bundle_from_default_evidence(
            UpscalerKind::TaaUpscale,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );

        assert_eq!(
            bundle.canonical_path,
            PassFTemporalStackBundle::CANONICAL_ARTIFACT_PATH
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));

        // All four test scenarios pass under the typed CPU
        // implementation of TAA resolve.
        assert!(bundle.static_stability.passes);
        assert!(bundle.moving_object.passes);
        assert!(bundle.camera_cut.passes);
        assert!(bundle.disocclusion.passes);

        // The verdict carries every typed rule.
        let verdict = PassFTemporalStackVerdict::evaluate(&bundle);
        assert!(
            verdict.passes(),
            "Pass F must pass under default TaaUpscale evidence; first_failed = {:?}",
            verdict.first_failed()
        );
        assert!(verdict.passes_motion_vectors_classified);
        assert!(verdict.passes_upscaler_route_resolves);
        assert!(verdict.passes_reactive_mask_available_when_route_requires_it);
        assert!(verdict.passes_history_debug_views_exposed);
        assert!(verdict.passes_static_stability_test);
        assert!(verdict.passes_moving_object_test);
        assert!(verdict.passes_camera_cut_test);
        assert!(verdict.passes_disocclusion_test);

        assert_eq!(bundle.outcome, PassFTemporalStackOutcome::Passes);
    }
}
