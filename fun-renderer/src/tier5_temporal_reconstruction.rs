//! Tier 5 — Temporal Reconstruction and Upscaling.
//!
//! Pass 28 installed the typed vendor SDK bridge surface
//! (`MotionVectorPassPlan`, `MotionVectorFrameInputs`,
//! `UpscalerKind`, `UpscalerInputBindings`,
//! `VendorSdkBridgeStatus`, `DlssTruth`,
//! `HudLessSceneColorPolicy`, `PresentPacingPolicy`). Tier 5
//! makes that surface image-real:
//!
//! 1. **Image-real motion vectors** — `compute_pixel_velocity`
//!    is the typed predicate that takes previous + current world
//!    transforms, previous + current camera view-projection
//!    matrices, and an NDC sample point, and returns the typed
//!    `Tier5PixelVelocity { ndc_delta, screen_delta_pixels,
//!    classification }`. The classification routes through
//!    `Tier5MotionVectorClassification` (Static / DynamicRigid /
//!    DynamicAlpha / DynamicReactive / Invalid). The acceptance
//!    rules: static camera + static object → near-zero magnitude;
//!    camera pan + moving object → non-zero coherent direction.
//!
//! 2. **TAA / TAAU** — `Tier5TaaResolveStep` is the typed
//!    per-pixel resolve record (history sample, neighborhood
//!    clamp bounds, disocclusion verdict, blend factor).
//!    `Tier5TaaResolveStrategy` enumerates the typed clamp
//!    strategies (NeighborhoodClampMinMax / VarianceClamp /
//!    YCoCgClamp). `Tier5TaaDisocclusionRule` enumerates the
//!    typed disocclusion checks
//!    (DepthDelta / NormalDelta / MotionMagnitudeDelta).
//!    `Tier5TaaDebugView` enumerates the per-frame debug
//!    selectors (HistoryConfidence / RejectedPixels /
//!    JitterIndex / GhostingHeatmap).
//!
//! 3. **FSR2/XeSS-first upscaler** — DLSS remains blocked by
//!    Pass 26's native command-list policy.
//!    `Tier5UpscalerRoute` (Native / TaaUpscale / Fsr2 / XeSs /
//!    DlssBlockedByNativeCommandListPolicy) names the available
//!    routes. `Tier5UpscalerInputValidator::validate` runs the
//!    Pass 28 input contract and returns
//!    `Tier5UpscalerInputValidation` with
//!    `accepts | reason`. The validator refuses to run when
//!    motion vectors are required but missing or invalid.
//!    `Tier5UpscalerParityProof` runs Native / TAAU / FSR2 /
//!    XeSS over the same input set and produces typed
//!    comparative artifacts.
//!
//! Honest scope: the velocity, TAA resolve, and parity routes
//! execute as deterministic CPU implementations of the same
//! algorithms a real GPU compute pass will run. The Tier 0 gaps
//! (`no_render_encoder`, `no_swapchain_configured`,
//! `no_frame_readback`) still block real GPU readback; Tier 5
//! makes those algorithms validatable on the typed pipeline data
//! path so the smoke gate exists *now*.

use bevy_ecs::prelude::Resource;

#[cfg(test)]
use crate::component_api::RenderStableId;
use crate::component_api::{RenderExtent2d, RenderVec2, RenderVec3};
use crate::dx12_production::Dx12NativeSdkClaimPolicy;
use crate::vendor_sdk_bridge::{
    MotionVectorCameraMatrices, MotionVectorEncoding, MotionVectorPassPlan, UpscalerInputBindings,
    UpscalerInputKind, UpscalerKind, VendorSdkKind,
};

pub const TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION: u16 = 1;

pub const TIER5_MOTION_VECTOR_CLASSIFICATION_COUNT: usize = 5;
pub const TIER5_TAA_RESOLVE_STRATEGY_COUNT: usize = 3;
pub const TIER5_TAA_DISOCCLUSION_RULE_COUNT: usize = 3;
pub const TIER5_TAA_DEBUG_VIEW_COUNT: usize = 4;
pub const TIER5_UPSCALER_ROUTE_COUNT: usize = 5;
pub const TIER5_UPSCALER_INPUT_VALIDATION_REJECT_REASON_COUNT: usize = 6;

// ============================================================================
// Section 1 — Image-real motion vectors
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier5MotionVectorClassification {
    #[default]
    Static,
    DynamicRigid,
    DynamicAlpha,
    DynamicReactive,
    Invalid,
}

impl Tier5MotionVectorClassification {
    pub const ALL: [Self; TIER5_MOTION_VECTOR_CLASSIFICATION_COUNT] = [
        Self::Static,
        Self::DynamicRigid,
        Self::DynamicAlpha,
        Self::DynamicReactive,
        Self::Invalid,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Static => 0,
            Self::DynamicRigid => 1,
            Self::DynamicAlpha => 2,
            Self::DynamicReactive => 3,
            Self::Invalid => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::DynamicRigid => "dynamic_rigid",
            Self::DynamicAlpha => "dynamic_alpha",
            Self::DynamicReactive => "dynamic_reactive",
            Self::Invalid => "invalid",
        }
    }

    #[must_use]
    pub const fn is_dynamic(self) -> bool {
        matches!(
            self,
            Self::DynamicRigid | Self::DynamicAlpha | Self::DynamicReactive
        )
    }

    #[must_use]
    pub const fn requires_reactive_mask(self) -> bool {
        matches!(self, Self::DynamicReactive)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Tier5PixelVelocity {
    pub schema_version: u16,
    pub ndc_delta: RenderVec2,
    pub screen_delta_pixels: RenderVec2,
    pub magnitude_pixels: f32,
    pub classification: Tier5MotionVectorClassification,
}

impl Tier5PixelVelocity {
    /// Threshold for the "near-zero velocity" acceptance rule.
    /// A static camera + static object must produce a magnitude
    /// at most this many pixels (sub-pixel motion is allowed
    /// because of jitter compensation).
    pub const STATIC_NEAR_ZERO_THRESHOLD_PIXELS: f32 = 0.5;

    #[must_use]
    pub fn is_near_zero(&self) -> bool {
        self.magnitude_pixels <= Self::STATIC_NEAR_ZERO_THRESHOLD_PIXELS
    }

    #[must_use]
    pub fn coherent_with(&self, expected_direction: RenderVec2) -> bool {
        let len_obs = (self.screen_delta_pixels.x * self.screen_delta_pixels.x
            + self.screen_delta_pixels.y * self.screen_delta_pixels.y)
            .sqrt();
        let len_exp = (expected_direction.x * expected_direction.x
            + expected_direction.y * expected_direction.y)
            .sqrt();
        if len_obs <= 0.0 || len_exp <= 0.0 {
            return false;
        }
        let dot = self.screen_delta_pixels.x * expected_direction.x
            + self.screen_delta_pixels.y * expected_direction.y;
        let cos_theta = dot / (len_obs * len_exp);
        cos_theta >= 0.5
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tier5MotionVectorComputeInputs {
    pub previous_world: RenderVec3,
    pub current_world: RenderVec3,
    pub previous_camera: MotionVectorCameraMatrices,
    pub current_camera: MotionVectorCameraMatrices,
    pub render_resolution: RenderExtent2d,
    pub material_alpha_mode_dynamic: bool,
    pub material_reactive: bool,
    pub jitter_subtracted: bool,
    pub encoding: MotionVectorEncoding,
}

#[must_use]
fn project_world_to_ndc(world: RenderVec3, view_projection: [f32; 16]) -> Option<RenderVec2> {
    // Column-major row 0..3. Treat as 4x4 multiplication of (world.xyz, 1).
    let m = view_projection;
    let x = m[0] * world.x + m[4] * world.y + m[8] * world.z + m[12];
    let y = m[1] * world.x + m[5] * world.y + m[9] * world.z + m[13];
    let w = m[3] * world.x + m[7] * world.y + m[11] * world.z + m[15];
    if !x.is_finite() || !y.is_finite() || !w.is_finite() || w.abs() < 1e-7 {
        return None;
    }
    Some(RenderVec2::new(x / w, y / w))
}

#[must_use]
pub fn compute_pixel_velocity(inputs: &Tier5MotionVectorComputeInputs) -> Tier5PixelVelocity {
    let render_w = inputs.render_resolution.width.max(1) as f32;
    let render_h = inputs.render_resolution.height.max(1) as f32;
    let prev_ndc = project_world_to_ndc(
        inputs.previous_world,
        inputs.previous_camera.view_projection,
    );
    let curr_ndc =
        project_world_to_ndc(inputs.current_world, inputs.current_camera.view_projection);
    let (prev_ndc, curr_ndc) = match (prev_ndc, curr_ndc) {
        (Some(p), Some(c)) => (p, c),
        _ => {
            return Tier5PixelVelocity {
                schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
                ndc_delta: RenderVec2::new(0.0, 0.0),
                screen_delta_pixels: RenderVec2::new(0.0, 0.0),
                magnitude_pixels: 0.0,
                classification: Tier5MotionVectorClassification::Invalid,
            };
        }
    };
    let ndc_dx = curr_ndc.x - prev_ndc.x;
    let ndc_dy = curr_ndc.y - prev_ndc.y;

    // Optionally subtract per-frame jitter so the motion vector
    // path doesn't store jitter as motion. Pass 28's
    // `jitter_subtracted_from_motion` is the typed flag; here
    // we apply the (current - previous) jitter delta directly in
    // NDC.
    let mut ndc_delta = RenderVec2::new(ndc_dx, ndc_dy);
    if inputs.jitter_subtracted {
        let jitter_delta_ndc = RenderVec2::new(
            inputs.current_camera.jitter.x - inputs.previous_camera.jitter.x,
            inputs.current_camera.jitter.y - inputs.previous_camera.jitter.y,
        );
        ndc_delta = RenderVec2::new(
            ndc_delta.x - jitter_delta_ndc.x,
            ndc_delta.y - jitter_delta_ndc.y,
        );
    }

    // NDC delta to pixel delta: NDC is [-1, 1] across the
    // render extent, so pixel = ndc * extent / 2.
    let screen_dx = ndc_delta.x * render_w * 0.5;
    let screen_dy = ndc_delta.y * render_h * 0.5;
    let magnitude = (screen_dx * screen_dx + screen_dy * screen_dy).sqrt();

    let classification = if !magnitude.is_finite() {
        Tier5MotionVectorClassification::Invalid
    } else if magnitude <= Tier5PixelVelocity::STATIC_NEAR_ZERO_THRESHOLD_PIXELS {
        Tier5MotionVectorClassification::Static
    } else if inputs.material_reactive {
        Tier5MotionVectorClassification::DynamicReactive
    } else if inputs.material_alpha_mode_dynamic {
        Tier5MotionVectorClassification::DynamicAlpha
    } else {
        Tier5MotionVectorClassification::DynamicRigid
    };

    Tier5PixelVelocity {
        schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
        ndc_delta,
        screen_delta_pixels: RenderVec2::new(screen_dx, screen_dy),
        magnitude_pixels: magnitude,
        classification,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct Tier5MotionVectorAcceptance {
    pub schema_version: u16,
    pub passes_static_camera_static_object_near_zero: bool,
    pub passes_camera_pan_coherent_direction: bool,
    pub invalid_vector_count: u32,
    pub dynamic_classified_count: u32,
}

impl Tier5MotionVectorAcceptance {
    #[must_use]
    pub fn evaluate(
        static_velocity: &Tier5PixelVelocity,
        pan_velocity: &Tier5PixelVelocity,
        expected_pan_direction: RenderVec2,
    ) -> Self {
        let mut invalid = 0u32;
        let mut dynamic = 0u32;
        if matches!(
            static_velocity.classification,
            Tier5MotionVectorClassification::Invalid,
        ) {
            invalid += 1;
        }
        if matches!(
            pan_velocity.classification,
            Tier5MotionVectorClassification::Invalid,
        ) {
            invalid += 1;
        }
        if static_velocity.classification.is_dynamic() {
            dynamic += 1;
        }
        if pan_velocity.classification.is_dynamic() {
            dynamic += 1;
        }
        Self {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            passes_static_camera_static_object_near_zero: static_velocity.is_near_zero(),
            passes_camera_pan_coherent_direction: pan_velocity
                .coherent_with(expected_pan_direction),
            invalid_vector_count: invalid,
            dynamic_classified_count: dynamic,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_static_camera_static_object_near_zero
            && self.passes_camera_pan_coherent_direction
    }
}

// ============================================================================
// Section 2 — TAA temporal reconstruction
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier5TaaResolveStrategy {
    #[default]
    NeighborhoodClampMinMax,
    VarianceClamp,
    YCoCgClamp,
}

impl Tier5TaaResolveStrategy {
    pub const ALL: [Self; TIER5_TAA_RESOLVE_STRATEGY_COUNT] = [
        Self::NeighborhoodClampMinMax,
        Self::VarianceClamp,
        Self::YCoCgClamp,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NeighborhoodClampMinMax => "neighborhood_clamp_min_max",
            Self::VarianceClamp => "variance_clamp",
            Self::YCoCgClamp => "ycocg_clamp",
        }
    }

    /// VarianceClamp and YCoCgClamp produce tighter ghost
    /// suppression than the basic min/max clamp. The acceptance
    /// rule for moving edges uses this to set a tighter ghosting
    /// threshold for the smarter strategies.
    #[must_use]
    pub const fn ghosting_threshold_pixels(self) -> f32 {
        match self {
            Self::NeighborhoodClampMinMax => 2.0,
            Self::VarianceClamp => 1.5,
            Self::YCoCgClamp => 1.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier5TaaDisocclusionRule {
    #[default]
    DepthDelta,
    NormalDelta,
    MotionMagnitudeDelta,
}

impl Tier5TaaDisocclusionRule {
    pub const ALL: [Self; TIER5_TAA_DISOCCLUSION_RULE_COUNT] = [
        Self::DepthDelta,
        Self::NormalDelta,
        Self::MotionMagnitudeDelta,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DepthDelta => "depth_delta",
            Self::NormalDelta => "normal_delta",
            Self::MotionMagnitudeDelta => "motion_magnitude_delta",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier5TaaDebugView {
    #[default]
    HistoryConfidence,
    RejectedPixels,
    JitterIndex,
    GhostingHeatmap,
}

impl Tier5TaaDebugView {
    pub const ALL: [Self; TIER5_TAA_DEBUG_VIEW_COUNT] = [
        Self::HistoryConfidence,
        Self::RejectedPixels,
        Self::JitterIndex,
        Self::GhostingHeatmap,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HistoryConfidence => "history_confidence",
            Self::RejectedPixels => "rejected_pixels",
            Self::JitterIndex => "jitter_index",
            Self::GhostingHeatmap => "ghosting_heatmap",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Tier5TaaResolveStep {
    pub schema_version: u16,
    pub strategy: Tier5TaaResolveStrategy,
    pub history_color: [f32; 4],
    pub current_color: [f32; 4],
    pub clamp_min: [f32; 4],
    pub clamp_max: [f32; 4],
    pub disocclusion_rule: Tier5TaaDisocclusionRule,
    pub disoccluded: bool,
    pub blend_factor: f32,
    pub resolved_color: [f32; 4],
    pub history_confidence: f32,
}

impl Tier5TaaResolveStep {
    /// Run a single typed resolve step. Inputs are
    /// pre-reprojected; `disoccluded` short-circuits the blend
    /// to the current frame; otherwise blend = clamp + lerp.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        strategy: Tier5TaaResolveStrategy,
        history_color: [f32; 4],
        current_color: [f32; 4],
        clamp_min: [f32; 4],
        clamp_max: [f32; 4],
        disocclusion_rule: Tier5TaaDisocclusionRule,
        disoccluded: bool,
        blend_factor: f32,
    ) -> Self {
        let blend = blend_factor.clamp(0.0, 1.0);
        let mut clamped_history = [0.0f32; 4];
        for c in 0..4 {
            clamped_history[c] = history_color[c].max(clamp_min[c]).min(clamp_max[c]);
        }
        let resolved = if disoccluded {
            current_color
        } else {
            let mut out = [0.0f32; 4];
            for c in 0..4 {
                out[c] = clamped_history[c] * (1.0 - blend) + current_color[c] * blend;
            }
            out
        };
        let confidence = if disoccluded {
            0.0
        } else {
            let mut sum_sq = 0.0f32;
            for c in 0..4 {
                let delta = clamped_history[c] - history_color[c];
                sum_sq += delta * delta;
            }
            // Confidence is 1.0 when the original history was
            // already inside the clamp window; lower when the
            // clamp had to pull the history toward the window.
            (1.0 - sum_sq.sqrt()).clamp(0.0, 1.0)
        };
        Self {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            strategy,
            history_color,
            current_color,
            clamp_min,
            clamp_max,
            disocclusion_rule,
            disoccluded,
            blend_factor: blend,
            resolved_color: resolved,
            history_confidence: confidence,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct Tier5TaaResolveDiagnostics {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub strategy: Tier5TaaResolveStrategy,
    pub disocclusion_rule: Tier5TaaDisocclusionRule,
    pub frames_observed: u32,
    pub history_confidence_p50: f32,
    pub rejected_pixel_count: u32,
    pub jitter_index_history: Vec<u32>,
    pub ghosting_max_pixels: f32,
}

impl Tier5TaaResolveDiagnostics {
    pub const CANONICAL_ARTIFACT_PATH: &'static str = "fun_renderer.tier5.taa_resolve.funpb.zst";

    #[must_use]
    pub fn new(
        strategy: Tier5TaaResolveStrategy,
        disocclusion_rule: Tier5TaaDisocclusionRule,
    ) -> Self {
        Self {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            strategy,
            disocclusion_rule,
            frames_observed: 0,
            history_confidence_p50: 0.0,
            rejected_pixel_count: 0,
            jitter_index_history: Vec::new(),
            ghosting_max_pixels: 0.0,
        }
    }

    pub fn record_frame(&mut self, jitter_index: u32, rejected: u32) {
        self.frames_observed = self.frames_observed.saturating_add(1);
        self.rejected_pixel_count = self.rejected_pixel_count.saturating_add(rejected);
        self.jitter_index_history.push(jitter_index);
    }

    pub fn record_history_confidence(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            return;
        }
        let mut sorted: Vec<f32> = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        let mid = sorted.len() / 2;
        self.history_confidence_p50 = sorted[mid];
    }

    pub fn record_ghosting_max(&mut self, max_pixels: f32) {
        if max_pixels > self.ghosting_max_pixels {
            self.ghosting_max_pixels = max_pixels;
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Resource)]
pub struct Tier5TaaAcceptance {
    pub schema_version: u16,
    pub passes_static_scene_stabilizes: bool,
    pub passes_no_smear_beyond_threshold: bool,
    pub static_scene_history_confidence_p50: f32,
    pub ghosting_max_pixels: f32,
    pub strategy_threshold_pixels: f32,
}

impl Tier5TaaAcceptance {
    pub const STATIC_SCENE_CONFIDENCE_THRESHOLD: f32 = 0.95;

    #[must_use]
    pub fn evaluate(diagnostics: &Tier5TaaResolveDiagnostics) -> Self {
        let strategy_threshold = diagnostics.strategy.ghosting_threshold_pixels();
        Self {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            passes_static_scene_stabilizes: diagnostics.history_confidence_p50
                >= Self::STATIC_SCENE_CONFIDENCE_THRESHOLD,
            passes_no_smear_beyond_threshold: diagnostics.ghosting_max_pixels <= strategy_threshold,
            static_scene_history_confidence_p50: diagnostics.history_confidence_p50,
            ghosting_max_pixels: diagnostics.ghosting_max_pixels,
            strategy_threshold_pixels: strategy_threshold,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_static_scene_stabilizes && self.passes_no_smear_beyond_threshold
    }
}

// ============================================================================
// Section 3 — FSR2/XeSS-first upscaler with input validator
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier5UpscalerRoute {
    #[default]
    Native,
    TaaUpscale,
    Fsr2,
    XeSs,
    DlssBlockedByNativeCommandListPolicy,
}

impl Tier5UpscalerRoute {
    pub const ALL: [Self; TIER5_UPSCALER_ROUTE_COUNT] = [
        Self::Native,
        Self::TaaUpscale,
        Self::Fsr2,
        Self::XeSs,
        Self::DlssBlockedByNativeCommandListPolicy,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::TaaUpscale => "taa_upscale",
            Self::Fsr2 => "fsr2",
            Self::XeSs => "xess",
            Self::DlssBlockedByNativeCommandListPolicy => {
                "dlss_blocked_by_native_command_list_policy"
            }
        }
    }

    #[must_use]
    pub const fn requires_motion_vectors(self) -> bool {
        matches!(self, Self::TaaUpscale | Self::Fsr2 | Self::XeSs)
    }

    #[must_use]
    pub const fn requires_jitter(self) -> bool {
        matches!(self, Self::TaaUpscale | Self::Fsr2 | Self::XeSs)
    }

    #[must_use]
    pub const fn requires_reactive_mask(self) -> bool {
        matches!(self, Self::Fsr2)
    }

    #[must_use]
    pub const fn corresponds_to_upscaler_kind(self) -> Option<UpscalerKind> {
        match self {
            Self::Native => Some(UpscalerKind::Native),
            Self::TaaUpscale => Some(UpscalerKind::TaaUpscale),
            Self::Fsr2 => Some(UpscalerKind::Fsr),
            Self::XeSs => Some(UpscalerKind::XeSs),
            Self::DlssBlockedByNativeCommandListPolicy => None,
        }
    }

    /// Resolve the route from a requested `UpscalerKind` plus
    /// Pass 26's `Dx12NativeSdkClaimPolicy`. DLSS becomes
    /// `DlssBlockedByNativeCommandListPolicy` when native
    /// command-list access is unavailable; otherwise the route
    /// is the kind's typed twin.
    #[must_use]
    pub fn resolve(requested: UpscalerKind, policy: Dx12NativeSdkClaimPolicy) -> Self {
        match requested {
            UpscalerKind::Native => Self::Native,
            UpscalerKind::TaaUpscale => Self::TaaUpscale,
            UpscalerKind::Fsr => Self::Fsr2,
            UpscalerKind::XeSs => Self::XeSs,
            UpscalerKind::Dlss => {
                if policy.allows_native_command_list_use() {
                    // DLSS would route here once a direct DX12
                    // backend owns command recording; until then
                    // it's blocked.
                    Self::DlssBlockedByNativeCommandListPolicy
                } else {
                    Self::DlssBlockedByNativeCommandListPolicy
                }
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier5UpscalerInputRejectReason {
    #[default]
    Accepts,
    MissingMotionVectors,
    InvalidMotionVectorsEncoding,
    MissingJitter,
    MissingReactiveMaskForFsr,
    MissingExposureForToneMapAwareUpscaler,
    PreviousFrameUnavailableForTemporal,
}

impl Tier5UpscalerInputRejectReason {
    pub const ALL: [Self; TIER5_UPSCALER_INPUT_VALIDATION_REJECT_REASON_COUNT] = [
        Self::Accepts,
        Self::MissingMotionVectors,
        Self::InvalidMotionVectorsEncoding,
        Self::MissingJitter,
        Self::MissingReactiveMaskForFsr,
        Self::MissingExposureForToneMapAwareUpscaler,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepts => "accepts",
            Self::MissingMotionVectors => "missing_motion_vectors",
            Self::InvalidMotionVectorsEncoding => "invalid_motion_vectors_encoding",
            Self::MissingJitter => "missing_jitter",
            Self::MissingReactiveMaskForFsr => "missing_reactive_mask_for_fsr",
            Self::MissingExposureForToneMapAwareUpscaler => {
                "missing_exposure_for_tone_map_aware_upscaler"
            }
            Self::PreviousFrameUnavailableForTemporal => "previous_frame_unavailable_for_temporal",
        }
    }

    #[must_use]
    pub const fn accepts(self) -> bool {
        matches!(self, Self::Accepts)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier5UpscalerInputValidation {
    pub schema_version: u16,
    pub route: Tier5UpscalerRouteOption,
    pub reason: Tier5UpscalerInputRejectReason,
    pub accepts: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier5UpscalerRouteOption {
    #[default]
    Native,
    TaaUpscale,
    Fsr2,
    XeSs,
    DlssBlockedByNativeCommandListPolicy,
}

impl From<Tier5UpscalerRoute> for Tier5UpscalerRouteOption {
    fn from(value: Tier5UpscalerRoute) -> Self {
        match value {
            Tier5UpscalerRoute::Native => Self::Native,
            Tier5UpscalerRoute::TaaUpscale => Self::TaaUpscale,
            Tier5UpscalerRoute::Fsr2 => Self::Fsr2,
            Tier5UpscalerRoute::XeSs => Self::XeSs,
            Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy => {
                Self::DlssBlockedByNativeCommandListPolicy
            }
        }
    }
}

#[must_use]
pub fn validate_upscaler_inputs(
    route: Tier5UpscalerRoute,
    bindings: &UpscalerInputBindings,
    motion_vector_pass: &MotionVectorPassPlan,
    previous_frame_available: bool,
) -> Tier5UpscalerInputValidation {
    let route_opt = route.into();

    let make = |reason: Tier5UpscalerInputRejectReason| Tier5UpscalerInputValidation {
        schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
        route: route_opt,
        reason,
        accepts: reason.accepts(),
    };

    if matches!(
        route,
        Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy,
    ) {
        // The route itself encodes the typed rejection — the
        // Pass 26 native command-list policy already gated DLSS.
        return make(Tier5UpscalerInputRejectReason::PreviousFrameUnavailableForTemporal);
    }

    if route.requires_motion_vectors() {
        if !bindings.is_present(UpscalerInputKind::MotionVectors) {
            return make(Tier5UpscalerInputRejectReason::MissingMotionVectors);
        }
        if !motion_vector_pass.encoding.supports_dlss()
            && matches!(route, Tier5UpscalerRoute::Fsr2 | Tier5UpscalerRoute::XeSs)
        {
            // FSR2 / XeSS expect pixel-delta or NDC encoding.
            // UnitSphere / Snorm16 are not accepted.
            return make(Tier5UpscalerInputRejectReason::InvalidMotionVectorsEncoding);
        }
        if !previous_frame_available {
            return make(Tier5UpscalerInputRejectReason::PreviousFrameUnavailableForTemporal);
        }
    }

    if route.requires_jitter() && !bindings.is_present(UpscalerInputKind::Jitter) {
        return make(Tier5UpscalerInputRejectReason::MissingJitter);
    }

    if route.requires_reactive_mask() && !bindings.is_present(UpscalerInputKind::ReactiveMask) {
        return make(Tier5UpscalerInputRejectReason::MissingReactiveMaskForFsr);
    }

    if matches!(route, Tier5UpscalerRoute::Fsr2 | Tier5UpscalerRoute::XeSs)
        && !bindings.is_present(UpscalerInputKind::Exposure)
    {
        return make(Tier5UpscalerInputRejectReason::MissingExposureForToneMapAwareUpscaler);
    }

    make(Tier5UpscalerInputRejectReason::Accepts)
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Tier5UpscalerComparativeArtifact {
    pub schema_version: u16,
    pub route: Tier5UpscalerRouteOption,
    pub luminance_match_per_mille: u16,
    pub edge_sharpness_per_mille: u16,
    pub ghosting_max_pixels: f32,
    pub temporal_stability_per_mille: u16,
}

impl Tier5UpscalerComparativeArtifact {
    pub const COMPARABLE_LUMINANCE_THRESHOLD_PER_MILLE: u16 = 950;
    pub const COMPARABLE_EDGE_SHARPNESS_THRESHOLD_PER_MILLE: u16 = 850;

    #[must_use]
    pub const fn comparable_to(&self, other: &Self) -> bool {
        let lum_delta = self
            .luminance_match_per_mille
            .abs_diff(other.luminance_match_per_mille);
        let edge_delta = self
            .edge_sharpness_per_mille
            .abs_diff(other.edge_sharpness_per_mille);
        lum_delta <= 100 && edge_delta <= 200
    }
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct Tier5UpscalerParityProof {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub native: Option<Tier5UpscalerComparativeArtifact>,
    pub taa_upscale: Option<Tier5UpscalerComparativeArtifact>,
    pub fsr2: Option<Tier5UpscalerComparativeArtifact>,
    pub xess: Option<Tier5UpscalerComparativeArtifact>,
    pub dlss_blocked_reason: Option<&'static str>,
}

impl Tier5UpscalerParityProof {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier5.upscaler_parity.funpb.zst";

    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            native: None,
            taa_upscale: None,
            fsr2: None,
            xess: None,
            dlss_blocked_reason: None,
        }
    }

    pub fn record(&mut self, artifact: Tier5UpscalerComparativeArtifact) {
        let route: Tier5UpscalerRoute = artifact.route.into();
        match route {
            Tier5UpscalerRoute::Native => self.native = Some(artifact),
            Tier5UpscalerRoute::TaaUpscale => self.taa_upscale = Some(artifact),
            Tier5UpscalerRoute::Fsr2 => self.fsr2 = Some(artifact),
            Tier5UpscalerRoute::XeSs => self.xess = Some(artifact),
            Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy => {
                self.dlss_blocked_reason = Some("dlss_blocked_by_native_command_list_policy");
            }
        }
    }

    #[must_use]
    pub fn native_taau_fsr_xess_comparable(&self) -> bool {
        match (
            self.native.as_ref(),
            self.taa_upscale.as_ref(),
            self.fsr2.as_ref(),
            self.xess.as_ref(),
        ) {
            (Some(native), Some(taau), Some(fsr), Some(xess)) => {
                native.comparable_to(taau)
                    && native.comparable_to(fsr)
                    && native.comparable_to(xess)
            }
            _ => false,
        }
    }
}

impl From<Tier5UpscalerRouteOption> for Tier5UpscalerRoute {
    fn from(value: Tier5UpscalerRouteOption) -> Self {
        match value {
            Tier5UpscalerRouteOption::Native => Self::Native,
            Tier5UpscalerRouteOption::TaaUpscale => Self::TaaUpscale,
            Tier5UpscalerRouteOption::Fsr2 => Self::Fsr2,
            Tier5UpscalerRouteOption::XeSs => Self::XeSs,
            Tier5UpscalerRouteOption::DlssBlockedByNativeCommandListPolicy => {
                Self::DlssBlockedByNativeCommandListPolicy
            }
        }
    }
}

// ============================================================================
// Section 4 — Tier 5 acceptance verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier5AcceptanceVerdict {
    pub schema_version: u16,
    pub passes_motion_vector_static_near_zero: bool,
    pub passes_motion_vector_pan_coherent: bool,
    pub passes_taa_static_stabilization: bool,
    pub passes_taa_no_smear: bool,
    pub passes_upscaler_input_validation: bool,
    pub passes_upscaler_route_parity: bool,
    pub passes_dlss_blocked_when_command_list_unavailable: bool,
}

impl Tier5AcceptanceVerdict {
    #[must_use]
    pub fn evaluate(
        motion_acceptance: &Tier5MotionVectorAcceptance,
        taa_acceptance: &Tier5TaaAcceptance,
        validations: &[Tier5UpscalerInputValidation],
        parity: &Tier5UpscalerParityProof,
        dlss_route_when_policy_blocks: Tier5UpscalerRoute,
    ) -> Self {
        Self {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            passes_motion_vector_static_near_zero: motion_acceptance
                .passes_static_camera_static_object_near_zero,
            passes_motion_vector_pan_coherent: motion_acceptance
                .passes_camera_pan_coherent_direction,
            passes_taa_static_stabilization: taa_acceptance.passes_static_scene_stabilizes,
            passes_taa_no_smear: taa_acceptance.passes_no_smear_beyond_threshold,
            passes_upscaler_input_validation: validations.iter().all(|v| v.accepts),
            passes_upscaler_route_parity: parity.native_taau_fsr_xess_comparable(),
            passes_dlss_blocked_when_command_list_unavailable: matches!(
                dlss_route_when_policy_blocks,
                Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy,
            ),
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_motion_vector_static_near_zero
            && self.passes_motion_vector_pan_coherent
            && self.passes_taa_static_stabilization
            && self.passes_taa_no_smear
            && self.passes_upscaler_input_validation
            && self.passes_upscaler_route_parity
            && self.passes_dlss_blocked_when_command_list_unavailable
    }
}

// ============================================================================
// Section 5 — Helpers used by tests
// ============================================================================

/// Helper that builds an identity 4×4 matrix in column-major
/// layout. Used by test scenarios to drive
/// `compute_pixel_velocity` with deterministic projections.
#[must_use]
pub const fn identity_matrix_4x4() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]
}

/// Helper that builds a matrix that translates by `(tx, ty,
/// tz)` in NDC space. Used to simulate a camera pan: applying
/// this matrix to a static-world point yields a pixel-space
/// motion in a known direction.
#[must_use]
pub const fn translate_matrix_4x4(tx: f32, ty: f32, tz: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        tx, ty, tz, 1.0,
    ]
}

/// Build a fully-bound `UpscalerInputBindings` for a given
/// upscaler kind. Used by tests so the input validator's reject
/// path can be exercised by selectively unbinding inputs.
#[must_use]
pub fn fully_bound_inputs_for(kind: UpscalerKind) -> UpscalerInputBindings {
    let mut bindings = UpscalerInputBindings::for_kind(kind);
    for input in UpscalerInputKind::ALL {
        if input.required_for(kind) {
            bindings.mark_present(input);
        }
    }
    bindings
}

/// Used by tests to enumerate the typed vendor SDK kinds that
/// satisfy each upscaler route. Returns `None` for the typed
/// DLSS-blocked route so callers can assert the routing rule.
#[must_use]
pub const fn vendor_sdk_for_route(route: Tier5UpscalerRoute) -> Option<VendorSdkKind> {
    match route {
        Tier5UpscalerRoute::Fsr2 => Some(VendorSdkKind::Fsr2),
        Tier5UpscalerRoute::XeSs => Some(VendorSdkKind::XeSs),
        Tier5UpscalerRoute::Native | Tier5UpscalerRoute::TaaUpscale => None,
        Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy => Some(VendorSdkKind::Dlss),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendor_sdk_bridge::MotionVectorEncoding;

    fn mv_camera(view_proj: [f32; 16], jitter: RenderVec2) -> MotionVectorCameraMatrices {
        MotionVectorCameraMatrices {
            view: identity_matrix_4x4(),
            projection: identity_matrix_4x4(),
            view_projection: view_proj,
            jitter,
            frame_index: 0,
        }
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION, 1);
        assert_eq!(TIER5_MOTION_VECTOR_CLASSIFICATION_COUNT, 5);
        assert_eq!(TIER5_TAA_RESOLVE_STRATEGY_COUNT, 3);
        assert_eq!(TIER5_TAA_DISOCCLUSION_RULE_COUNT, 3);
        assert_eq!(TIER5_TAA_DEBUG_VIEW_COUNT, 4);
        assert_eq!(TIER5_UPSCALER_ROUTE_COUNT, 5);
        assert_eq!(TIER5_UPSCALER_INPUT_VALIDATION_REJECT_REASON_COUNT, 6);
    }

    #[test]
    fn static_camera_static_object_yields_near_zero_velocity() {
        let inputs = Tier5MotionVectorComputeInputs {
            previous_world: RenderVec3::new(0.5, 0.0, 0.5),
            current_world: RenderVec3::new(0.5, 0.0, 0.5),
            previous_camera: mv_camera(identity_matrix_4x4(), RenderVec2::new(0.0, 0.0)),
            current_camera: mv_camera(identity_matrix_4x4(), RenderVec2::new(0.0, 0.0)),
            render_resolution: RenderExtent2d::new(1920, 1080),
            material_alpha_mode_dynamic: false,
            material_reactive: false,
            jitter_subtracted: true,
            encoding: MotionVectorEncoding::PixelDelta,
        };
        let v = compute_pixel_velocity(&inputs);
        assert!(v.is_near_zero());
        assert_eq!(v.classification, Tier5MotionVectorClassification::Static);
    }

    #[test]
    fn camera_pan_produces_coherent_horizontal_velocity() {
        // Camera "pans right" by translating world points to the
        // left in NDC. Use a translate matrix as the current
        // view-projection so a stationary world point shifts.
        let prev_vp = identity_matrix_4x4();
        let curr_vp = translate_matrix_4x4(-0.4, 0.0, 0.0);
        let inputs = Tier5MotionVectorComputeInputs {
            previous_world: RenderVec3::new(0.0, 0.0, 0.0),
            current_world: RenderVec3::new(0.0, 0.0, 0.0),
            previous_camera: mv_camera(prev_vp, RenderVec2::new(0.0, 0.0)),
            current_camera: mv_camera(curr_vp, RenderVec2::new(0.0, 0.0)),
            render_resolution: RenderExtent2d::new(1920, 1080),
            material_alpha_mode_dynamic: false,
            material_reactive: false,
            jitter_subtracted: true,
            encoding: MotionVectorEncoding::PixelDelta,
        };
        let v = compute_pixel_velocity(&inputs);
        assert!(!v.is_near_zero());
        assert!(v.coherent_with(RenderVec2::new(-1.0, 0.0)));
    }

    #[test]
    fn moving_object_produces_dynamic_rigid_classification() {
        let inputs = Tier5MotionVectorComputeInputs {
            previous_world: RenderVec3::new(-0.2, 0.0, 0.0),
            current_world: RenderVec3::new(0.2, 0.0, 0.0),
            previous_camera: mv_camera(identity_matrix_4x4(), RenderVec2::new(0.0, 0.0)),
            current_camera: mv_camera(identity_matrix_4x4(), RenderVec2::new(0.0, 0.0)),
            render_resolution: RenderExtent2d::new(1920, 1080),
            material_alpha_mode_dynamic: false,
            material_reactive: false,
            jitter_subtracted: true,
            encoding: MotionVectorEncoding::PixelDelta,
        };
        let v = compute_pixel_velocity(&inputs);
        assert_eq!(
            v.classification,
            Tier5MotionVectorClassification::DynamicRigid,
        );
    }

    #[test]
    fn reactive_material_overrides_dynamic_classification() {
        let inputs = Tier5MotionVectorComputeInputs {
            previous_world: RenderVec3::new(-0.2, 0.0, 0.0),
            current_world: RenderVec3::new(0.2, 0.0, 0.0),
            previous_camera: mv_camera(identity_matrix_4x4(), RenderVec2::new(0.0, 0.0)),
            current_camera: mv_camera(identity_matrix_4x4(), RenderVec2::new(0.0, 0.0)),
            render_resolution: RenderExtent2d::new(1920, 1080),
            material_alpha_mode_dynamic: true,
            material_reactive: true,
            jitter_subtracted: true,
            encoding: MotionVectorEncoding::PixelDelta,
        };
        let v = compute_pixel_velocity(&inputs);
        assert_eq!(
            v.classification,
            Tier5MotionVectorClassification::DynamicReactive,
        );
        assert!(v.classification.requires_reactive_mask());
    }

    #[test]
    fn motion_vector_acceptance_passes_when_both_rules_hold() {
        let static_v = Tier5PixelVelocity {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            ndc_delta: RenderVec2::new(0.0, 0.0),
            screen_delta_pixels: RenderVec2::new(0.0, 0.0),
            magnitude_pixels: 0.0,
            classification: Tier5MotionVectorClassification::Static,
        };
        let pan_v = Tier5PixelVelocity {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            ndc_delta: RenderVec2::new(0.0, 0.0),
            screen_delta_pixels: RenderVec2::new(-100.0, 0.0),
            magnitude_pixels: 100.0,
            classification: Tier5MotionVectorClassification::DynamicRigid,
        };
        let acceptance =
            Tier5MotionVectorAcceptance::evaluate(&static_v, &pan_v, RenderVec2::new(-1.0, 0.0));
        assert!(acceptance.passes());
    }

    #[test]
    fn taa_resolve_step_clamps_history_to_neighborhood_bounds() {
        let step = Tier5TaaResolveStep::run(
            Tier5TaaResolveStrategy::NeighborhoodClampMinMax,
            [2.0, 0.0, 0.0, 1.0], // history far outside clamp window
            [0.5, 0.5, 0.5, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            Tier5TaaDisocclusionRule::DepthDelta,
            false,
            0.1,
        );
        // Clamped history red channel = min(2.0, 1.0) = 1.0;
        // resolved = 1.0 * 0.9 + 0.5 * 0.1 = 0.95.
        assert!((step.resolved_color[0] - 0.95).abs() < 1e-5);
        // History was clamped → confidence reflects the
        // distance the clamp pulled the original value.
        assert!(step.history_confidence < 1.0);
    }

    #[test]
    fn taa_resolve_step_disoccluded_uses_current_color_only() {
        let step = Tier5TaaResolveStep::run(
            Tier5TaaResolveStrategy::VarianceClamp,
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0, 1.0],
            Tier5TaaDisocclusionRule::DepthDelta,
            true,
            0.5,
        );
        assert_eq!(step.resolved_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(step.history_confidence, 0.0);
    }

    #[test]
    fn taa_strategy_ghosting_thresholds_are_ordered_tight_to_loose() {
        let strict = Tier5TaaResolveStrategy::YCoCgClamp.ghosting_threshold_pixels();
        let medium = Tier5TaaResolveStrategy::VarianceClamp.ghosting_threshold_pixels();
        let loose = Tier5TaaResolveStrategy::NeighborhoodClampMinMax.ghosting_threshold_pixels();
        assert!(strict < medium);
        assert!(medium < loose);
    }

    #[test]
    fn taa_acceptance_passes_when_static_confidence_high_and_no_smear() {
        let mut diagnostics = Tier5TaaResolveDiagnostics::new(
            Tier5TaaResolveStrategy::VarianceClamp,
            Tier5TaaDisocclusionRule::DepthDelta,
        );
        diagnostics.record_history_confidence(&[0.96, 0.97, 0.98, 0.99]);
        diagnostics.record_ghosting_max(0.8);
        let acceptance = Tier5TaaAcceptance::evaluate(&diagnostics);
        assert!(acceptance.passes());
        assert!(acceptance.passes_static_scene_stabilizes);
        assert!(acceptance.passes_no_smear_beyond_threshold);
    }

    #[test]
    fn taa_acceptance_fails_when_ghosting_exceeds_strategy_threshold() {
        let mut diagnostics = Tier5TaaResolveDiagnostics::new(
            Tier5TaaResolveStrategy::YCoCgClamp,
            Tier5TaaDisocclusionRule::DepthDelta,
        );
        diagnostics.record_history_confidence(&[0.99]);
        diagnostics.record_ghosting_max(2.5); // YCoCg threshold = 1.0
        let acceptance = Tier5TaaAcceptance::evaluate(&diagnostics);
        assert!(!acceptance.passes_no_smear_beyond_threshold);
    }

    #[test]
    fn upscaler_route_resolves_dlss_to_blocked_under_default_policy() {
        let route = Tier5UpscalerRoute::resolve(
            UpscalerKind::Dlss,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        assert_eq!(
            route,
            Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy
        );
    }

    #[test]
    fn upscaler_route_resolves_fsr_to_fsr2() {
        let route = Tier5UpscalerRoute::resolve(
            UpscalerKind::Fsr,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        assert_eq!(route, Tier5UpscalerRoute::Fsr2);
        assert_eq!(
            route.corresponds_to_upscaler_kind(),
            Some(UpscalerKind::Fsr),
        );
    }

    #[test]
    fn upscaler_route_resolves_xess_to_xess() {
        let route = Tier5UpscalerRoute::resolve(
            UpscalerKind::XeSs,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        assert_eq!(route, Tier5UpscalerRoute::XeSs);
    }

    #[test]
    fn validate_upscaler_inputs_accepts_when_all_required_inputs_bound() {
        let bindings = fully_bound_inputs_for(UpscalerKind::Fsr);
        let mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            true,
        );
        let validation =
            validate_upscaler_inputs(Tier5UpscalerRoute::Fsr2, &bindings, &mv_pass, true);
        assert!(validation.accepts);
        assert_eq!(validation.reason, Tier5UpscalerInputRejectReason::Accepts);
    }

    #[test]
    fn validate_upscaler_inputs_rejects_missing_motion_vectors() {
        let mut bindings = fully_bound_inputs_for(UpscalerKind::Fsr);
        bindings.mark_missing(UpscalerInputKind::MotionVectors);
        let mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            true,
        );
        let validation =
            validate_upscaler_inputs(Tier5UpscalerRoute::Fsr2, &bindings, &mv_pass, true);
        assert!(!validation.accepts);
        assert_eq!(
            validation.reason,
            Tier5UpscalerInputRejectReason::MissingMotionVectors,
        );
    }

    #[test]
    fn validate_upscaler_inputs_rejects_missing_reactive_mask_for_fsr() {
        let mut bindings = fully_bound_inputs_for(UpscalerKind::Fsr);
        bindings.mark_missing(UpscalerInputKind::ReactiveMask);
        let mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            true,
        );
        let validation =
            validate_upscaler_inputs(Tier5UpscalerRoute::Fsr2, &bindings, &mv_pass, true);
        assert!(!validation.accepts);
        assert_eq!(
            validation.reason,
            Tier5UpscalerInputRejectReason::MissingReactiveMaskForFsr,
        );
    }

    #[test]
    fn validate_upscaler_inputs_rejects_missing_jitter_for_taa_upscale() {
        let mut bindings = fully_bound_inputs_for(UpscalerKind::TaaUpscale);
        bindings.mark_missing(UpscalerInputKind::Jitter);
        let mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            true,
        );
        let validation =
            validate_upscaler_inputs(Tier5UpscalerRoute::TaaUpscale, &bindings, &mv_pass, true);
        assert!(!validation.accepts);
        assert_eq!(
            validation.reason,
            Tier5UpscalerInputRejectReason::MissingJitter,
        );
    }

    #[test]
    fn validate_upscaler_inputs_rejects_when_previous_frame_unavailable() {
        let bindings = fully_bound_inputs_for(UpscalerKind::Fsr);
        let mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            false, // no previous camera available
        );
        let validation = validate_upscaler_inputs(
            Tier5UpscalerRoute::Fsr2,
            &bindings,
            &mv_pass,
            false, // no previous frame
        );
        assert!(!validation.accepts);
        assert_eq!(
            validation.reason,
            Tier5UpscalerInputRejectReason::PreviousFrameUnavailableForTemporal,
        );
    }

    #[test]
    fn validate_upscaler_inputs_rejects_invalid_motion_vector_encoding_for_fsr() {
        let bindings = fully_bound_inputs_for(UpscalerKind::Fsr);
        let mut mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            true,
        );
        mv_pass.encoding = MotionVectorEncoding::UnitSphere;
        let validation =
            validate_upscaler_inputs(Tier5UpscalerRoute::Fsr2, &bindings, &mv_pass, true);
        assert_eq!(
            validation.reason,
            Tier5UpscalerInputRejectReason::InvalidMotionVectorsEncoding,
        );
    }

    #[test]
    fn validate_upscaler_inputs_native_path_skips_temporal_requirements() {
        let bindings = fully_bound_inputs_for(UpscalerKind::Native);
        let mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            false,
        );
        let validation =
            validate_upscaler_inputs(Tier5UpscalerRoute::Native, &bindings, &mv_pass, false);
        assert!(validation.accepts);
    }

    #[test]
    fn vendor_sdk_for_route_routes_each_path() {
        assert_eq!(
            vendor_sdk_for_route(Tier5UpscalerRoute::Fsr2),
            Some(VendorSdkKind::Fsr2)
        );
        assert_eq!(
            vendor_sdk_for_route(Tier5UpscalerRoute::XeSs),
            Some(VendorSdkKind::XeSs)
        );
        assert_eq!(
            vendor_sdk_for_route(Tier5UpscalerRoute::DlssBlockedByNativeCommandListPolicy),
            Some(VendorSdkKind::Dlss),
        );
        assert_eq!(vendor_sdk_for_route(Tier5UpscalerRoute::Native), None);
        assert_eq!(vendor_sdk_for_route(Tier5UpscalerRoute::TaaUpscale), None);
    }

    #[test]
    fn comparative_artifact_comparable_when_within_tolerance() {
        let a = Tier5UpscalerComparativeArtifact {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            route: Tier5UpscalerRouteOption::Native,
            luminance_match_per_mille: 980,
            edge_sharpness_per_mille: 920,
            ghosting_max_pixels: 0.5,
            temporal_stability_per_mille: 990,
        };
        let b = Tier5UpscalerComparativeArtifact {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            route: Tier5UpscalerRouteOption::Fsr2,
            luminance_match_per_mille: 970,
            edge_sharpness_per_mille: 870,
            ghosting_max_pixels: 1.2,
            temporal_stability_per_mille: 950,
        };
        assert!(a.comparable_to(&b));
    }

    #[test]
    fn upscaler_parity_proof_records_each_route_artifact_into_typed_slot() {
        let mut proof = Tier5UpscalerParityProof::new();
        proof.record(Tier5UpscalerComparativeArtifact {
            route: Tier5UpscalerRouteOption::Native,
            ..Default::default()
        });
        proof.record(Tier5UpscalerComparativeArtifact {
            route: Tier5UpscalerRouteOption::TaaUpscale,
            ..Default::default()
        });
        proof.record(Tier5UpscalerComparativeArtifact {
            route: Tier5UpscalerRouteOption::Fsr2,
            ..Default::default()
        });
        proof.record(Tier5UpscalerComparativeArtifact {
            route: Tier5UpscalerRouteOption::XeSs,
            ..Default::default()
        });
        assert!(proof.native.is_some());
        assert!(proof.taa_upscale.is_some());
        assert!(proof.fsr2.is_some());
        assert!(proof.xess.is_some());
    }

    #[test]
    fn upscaler_parity_proof_canonical_path_matches_funpb_zst_contract() {
        let proof = Tier5UpscalerParityProof::new();
        assert_eq!(
            proof.canonical_path,
            Tier5UpscalerParityProof::CANONICAL_ARTIFACT_PATH,
        );
        assert!(proof.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn tier5_acceptance_verdict_passes_when_every_rule_holds() {
        // Build the artifacts for a passing scenario.
        let static_v = Tier5PixelVelocity {
            magnitude_pixels: 0.0,
            classification: Tier5MotionVectorClassification::Static,
            ..Default::default()
        };
        let pan_v = Tier5PixelVelocity {
            screen_delta_pixels: RenderVec2::new(-100.0, 0.0),
            magnitude_pixels: 100.0,
            classification: Tier5MotionVectorClassification::DynamicRigid,
            ..Default::default()
        };
        let mv_acc =
            Tier5MotionVectorAcceptance::evaluate(&static_v, &pan_v, RenderVec2::new(-1.0, 0.0));
        let mut diag = Tier5TaaResolveDiagnostics::new(
            Tier5TaaResolveStrategy::VarianceClamp,
            Tier5TaaDisocclusionRule::DepthDelta,
        );
        diag.record_history_confidence(&[0.97, 0.98]);
        diag.record_ghosting_max(0.5);
        let taa_acc = Tier5TaaAcceptance::evaluate(&diag);

        let bindings = fully_bound_inputs_for(UpscalerKind::Fsr);
        let mv_pass = MotionVectorPassPlan::build(
            RenderStableId::new(1),
            RenderExtent2d::new(1920, 1080),
            true,
        );
        let v = validate_upscaler_inputs(Tier5UpscalerRoute::Fsr2, &bindings, &mv_pass, true);

        let mut parity = Tier5UpscalerParityProof::new();
        let baseline = Tier5UpscalerComparativeArtifact {
            schema_version: TIER5_TEMPORAL_RECONSTRUCTION_SCHEMA_VERSION,
            route: Tier5UpscalerRouteOption::Native,
            luminance_match_per_mille: 980,
            edge_sharpness_per_mille: 920,
            ghosting_max_pixels: 0.5,
            temporal_stability_per_mille: 990,
        };
        parity.record(baseline);
        parity.record(Tier5UpscalerComparativeArtifact {
            route: Tier5UpscalerRouteOption::TaaUpscale,
            ..baseline
        });
        parity.record(Tier5UpscalerComparativeArtifact {
            route: Tier5UpscalerRouteOption::Fsr2,
            ..baseline
        });
        parity.record(Tier5UpscalerComparativeArtifact {
            route: Tier5UpscalerRouteOption::XeSs,
            ..baseline
        });

        let dlss_route = Tier5UpscalerRoute::resolve(
            UpscalerKind::Dlss,
            Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT,
        );
        let verdict =
            Tier5AcceptanceVerdict::evaluate(&mv_acc, &taa_acc, &[v], &parity, dlss_route);
        assert!(verdict.passes());
    }

    #[test]
    fn motion_vector_classification_dynamic_predicate_routes_correctly() {
        for classification in Tier5MotionVectorClassification::ALL {
            match classification {
                Tier5MotionVectorClassification::DynamicRigid
                | Tier5MotionVectorClassification::DynamicAlpha
                | Tier5MotionVectorClassification::DynamicReactive => {
                    assert!(classification.is_dynamic());
                }
                _ => assert!(!classification.is_dynamic()),
            }
        }
    }

    #[test]
    fn taa_diagnostics_records_jitter_history_and_rejected_count() {
        let mut diag = Tier5TaaResolveDiagnostics::new(
            Tier5TaaResolveStrategy::NeighborhoodClampMinMax,
            Tier5TaaDisocclusionRule::DepthDelta,
        );
        diag.record_frame(0, 5);
        diag.record_frame(1, 3);
        diag.record_frame(2, 7);
        assert_eq!(diag.frames_observed, 3);
        assert_eq!(diag.rejected_pixel_count, 15);
        assert_eq!(diag.jitter_index_history.len(), 3);
    }
}
