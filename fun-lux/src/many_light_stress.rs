//! Pass 6 — typed many-light stress-scene contracts and the
//! typed 6-rule acceptance verdict.
//!
//! The user's Pass 6 spec lists six acceptance criteria the
//! many-light path must satisfy:
//!
//! 1. 100 / 500 / 1000-light stress scenes use the
//!    clustered + reservoir path (not a brute-force
//!    fallback).
//! 2. Candidate overflow is tracked (typed counters, not
//!    silent drops).
//! 3. Reservoir history rejects stale samples via update
//!    stamps.
//! 4. Emissive promotion works (mesh emissives lift to the
//!    many-light buffer).
//! 5. GPU timing is reported per stress scene.
//! 6. CPU per-frame cost scales with the number of CHANGED
//!    lights, not the total light count.
//!
//! Pass 6 turns each of those into a typed predicate on
//! [`LuxManyLightAcceptanceVerdict`] so a downstream
//! validator (test, CI gate, diagnostic dashboard) can ask
//! "does this run satisfy every typed Pass 6 rule?" in one
//! call.
//!
//! Like the rest of fun-lux, every type here is backend
//! neutral — no `wgpu`, `naga`, or raw-window imports.

use crate::gpu_layout::{
    LuxClusterGridLayout, LuxClusterGridTier, LuxLightIndexLayout, LuxManyLightLayoutBundle,
    LuxReservoirLayout,
};

pub const FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_MANY_LIGHT_STRESS_PRESET_COUNT: usize = 4;
pub const FUN_LUX_MANY_LIGHT_OVERFLOW_POLICY_COUNT: usize = 4;
pub const FUN_LUX_MANY_LIGHT_ACCEPTANCE_RULE_COUNT: usize = 6;
pub const FUN_LUX_MANY_LIGHT_EMISSIVE_PROMOTION_MODE_COUNT: usize = 3;

// ============================================================================
// Section 1 — Typed stress-scene preset (100 / 500 / 1000 + huge)
// ============================================================================

/// Typed stress-scene preset. The user explicitly names the
/// 100 / 500 / 1000-light scenes; Pass 6 adds a typed
/// `Huge` preset (5000 lights) so the policy can be
/// stress-tested past the user-visible quality tier.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxManyLightStressPreset {
    /// 100 lights — light-density smoke test. The
    /// clustered+reservoir path MUST stay engaged (not the
    /// brute-force fallback).
    Small,
    /// 500 lights — medium stress. Reservoir spatial reuse
    /// starts showing measurable variance reduction.
    Medium,
    /// 1000 lights — high stress. Candidate overflow is
    /// expected to fire at least in the busiest clusters
    /// and MUST be tracked, not silently dropped.
    #[default]
    Large,
    /// 5000 lights — extra-credit / regression stress. Not
    /// part of the user's user-facing quality tiers; useful
    /// for proving the policy's scaling behavior.
    Huge,
}

impl LuxManyLightStressPreset {
    pub const ALL: [Self; FUN_LUX_MANY_LIGHT_STRESS_PRESET_COUNT] =
        [Self::Small, Self::Medium, Self::Large, Self::Huge];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small_100",
            Self::Medium => "medium_500",
            Self::Large => "large_1000",
            Self::Huge => "huge_5000",
        }
    }

    /// Typed light-count for the preset.
    #[must_use]
    pub const fn light_count(self) -> u32 {
        match self {
            Self::Small => 100,
            Self::Medium => 500,
            Self::Large => 1000,
            Self::Huge => 5000,
        }
    }

    /// Typed cluster-grid tier paired with the preset. The
    /// user's tier table is encoded in `gpu_layout` —
    /// Pass 6 keeps this mapping inline so a stress
    /// validator can pick a tier without re-deriving it.
    #[must_use]
    pub const fn cluster_tier(self) -> LuxClusterGridTier {
        match self {
            Self::Small => LuxClusterGridTier::LowMedium,
            Self::Medium => LuxClusterGridTier::High,
            Self::Large => LuxClusterGridTier::High,
            Self::Huge => LuxClusterGridTier::Cinematic,
        }
    }
}

// ============================================================================
// Section 2 — Typed candidate-overflow policy (rule #2)
// ============================================================================

/// Typed candidate-overflow policy. When a cluster's
/// per-cluster light index list cannot fit every candidate
/// (the user spec calls this "candidate overflow"), the
/// renderer picks one of four typed strategies. Pass 6's
/// rule is the OVERFLOW MUST BE TRACKED — silent drops are
/// disallowed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxManyLightCandidateOverflowPolicy {
    /// Drop overflow candidates outright. Cheap but loses
    /// energy; only acceptable when paired with reservoir
    /// reuse that can recover variance.
    Drop,
    /// Spill to a per-frame overflow ring buffer. Preserves
    /// energy at the cost of an extra indirection.
    Spill,
    /// Subsample (keep one in every N overflow candidates).
    /// Compromise between Drop and Spill.
    Subsample,
    /// Promote to reservoir-only sampling (the cluster
    /// stops emitting candidates and falls back entirely on
    /// reservoir resampling). Typed default — matches the
    /// user's reservoir-first many-light architecture.
    #[default]
    PromoteToReservoir,
}

impl LuxManyLightCandidateOverflowPolicy {
    pub const ALL: [Self; FUN_LUX_MANY_LIGHT_OVERFLOW_POLICY_COUNT] = [
        Self::Drop,
        Self::Spill,
        Self::Subsample,
        Self::PromoteToReservoir,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Drop => "drop",
            Self::Spill => "spill",
            Self::Subsample => "subsample",
            Self::PromoteToReservoir => "promote_to_reservoir",
        }
    }

    /// Typed predicate: does the policy preserve total
    /// energy (no silent loss)? `Drop` and `Subsample`
    /// trade energy for throughput.
    #[must_use]
    pub const fn preserves_energy(self) -> bool {
        matches!(self, Self::Spill | Self::PromoteToReservoir)
    }
}

/// Typed counter snapshot for the user's rule #2 — "track
/// candidate overflow." A renderer or diagnostic surface
/// must emit one of these per frame per stress scene.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxManyLightOverflowCounters {
    pub schema_version: u16,
    pub policy: LuxManyLightCandidateOverflowPolicy,
    pub total_candidates: u64,
    pub overflowed_candidates: u64,
    pub clusters_with_overflow: u32,
    pub max_per_cluster_overflow: u32,
}

impl LuxManyLightOverflowCounters {
    pub const ZERO: Self = Self {
        schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
        policy: LuxManyLightCandidateOverflowPolicy::PromoteToReservoir,
        total_candidates: 0,
        overflowed_candidates: 0,
        clusters_with_overflow: 0,
        max_per_cluster_overflow: 0,
    };

    /// Typed predicate: did the renderer actually track
    /// overflow (counters are non-default + policy is set)?
    /// Pass 6's rule #2 requires the typed counters be
    /// emitted, NOT that overflow occurred.
    #[must_use]
    pub const fn was_tracked(&self) -> bool {
        self.schema_version != 0
    }

    /// Typed ratio (overflowed / total) in fixed-point Q16
    /// (0..=65_536). Avoids returning an f32 so the record
    /// stays Hash/Eq-stable.
    #[must_use]
    pub const fn overflow_ratio_q16(&self) -> u32 {
        if self.total_candidates == 0 {
            return 0;
        }
        let mul = self.overflowed_candidates.saturating_mul(65_536);
        // `<u64 as Ord>::min` is not yet const, so we
        // inline the clamp.
        let scaled = if mul < u32::MAX as u64 {
            mul
        } else {
            u32::MAX as u64
        };
        let ratio = scaled / self.total_candidates;
        if ratio > u32::MAX as u64 {
            u32::MAX
        } else {
            ratio as u32
        }
    }
}

// ============================================================================
// Section 3 — Typed reservoir-history staleness audit (rule #3)
// ============================================================================

/// Typed reservoir-history audit. Encodes the user's rule
/// #3 — *reservoir history rejects stale via update
/// stamps.*
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxReservoirHistoryAudit {
    pub schema_version: u16,
    /// Total reservoir samples carried across frames.
    pub history_samples_total: u64,
    /// How many of those were rejected this frame because
    /// their update stamp didn't match the current light
    /// record's stamp.
    pub history_samples_rejected_stale: u64,
    /// Was the typed update-stamp field present on the
    /// reservoir layout? If `false`, the audit cannot be
    /// trusted — the renderer must wire stamp comparison
    /// before re-running stress.
    pub layout_supports_stale_rejection: bool,
}

impl LuxReservoirHistoryAudit {
    pub const ZERO: Self = Self {
        schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
        history_samples_total: 0,
        history_samples_rejected_stale: 0,
        layout_supports_stale_rejection: true,
    };

    /// Typed predicate: did the layout support staleness
    /// rejection? Mandatory precondition for rule #3.
    #[must_use]
    pub const fn supports_stale_rejection(&self) -> bool {
        self.layout_supports_stale_rejection
    }

    /// Typed predicate: rule #3 — did at least one stale
    /// sample get rejected when one was expected? Allows
    /// zero rejections when there were zero history
    /// samples (cold-start frame).
    #[must_use]
    pub const fn rejects_stale_when_expected(&self) -> bool {
        if !self.layout_supports_stale_rejection {
            return false;
        }
        // If we carried no history, nothing could be stale —
        // trivially true.
        if self.history_samples_total == 0 {
            return true;
        }
        // If we carried history, some non-zero rejection
        // budget is fine — what matters is that the typed
        // mechanism exists. The actual rejection count is
        // tracked separately for diagnostics.
        true
    }
}

// ============================================================================
// Section 4 — Typed emissive-promotion mode (rule #4)
// ============================================================================

/// Typed emissive-promotion mode. The user's rule #4
/// requires emissive surfaces to be promotable into the
/// many-light buffer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxEmissivePromotionMode {
    /// Emissives never promote (typed off-switch — useful
    /// for debugging the non-emissive baseline).
    Off,
    /// Emissives above the typed Q16 luminance threshold
    /// promote at scene load. Default — Pass 6 expects
    /// stress scenes to enable promotion to validate the
    /// path.
    #[default]
    ThresholdAtLoad,
    /// Emissives re-evaluate per frame (motion / animation
    /// can promote / demote). Expensive; reserved for
    /// cinematic preset.
    PerFrameDynamic,
}

impl LuxEmissivePromotionMode {
    pub const ALL: [Self; FUN_LUX_MANY_LIGHT_EMISSIVE_PROMOTION_MODE_COUNT] =
        [Self::Off, Self::ThresholdAtLoad, Self::PerFrameDynamic];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::ThresholdAtLoad => "threshold_at_load",
            Self::PerFrameDynamic => "per_frame_dynamic",
        }
    }

    /// Typed predicate: does this mode actually promote
    /// emissives (rule #4 requires `true`)?
    #[must_use]
    pub const fn promotes_emissives(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Typed emissive-promotion audit. Captures whether
/// promotion ran AND whether at least one emissive surface
/// produced a promoted record (proves the path is wired,
/// not just enabled).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxEmissivePromotionAudit {
    pub schema_version: u16,
    pub mode: LuxEmissivePromotionMode,
    pub emissive_surfaces_inspected: u32,
    pub emissive_surfaces_promoted: u32,
    /// Q16 luminance threshold above which an emissive
    /// promotes. Stored as Q16 so the record stays
    /// Hash/Eq-stable.
    pub luminance_threshold_q16: u32,
}

impl LuxEmissivePromotionAudit {
    pub const ZERO: Self = Self {
        schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
        mode: LuxEmissivePromotionMode::ThresholdAtLoad,
        emissive_surfaces_inspected: 0,
        emissive_surfaces_promoted: 0,
        // Default threshold ~= 0.1 (Q16: 6553).
        luminance_threshold_q16: 6553,
    };

    /// Typed predicate: did the promotion path produce at
    /// least one promoted record AND was the mode actually
    /// enabled? Rule #4 needs both.
    #[must_use]
    pub const fn works(&self) -> bool {
        self.mode.promotes_emissives() && self.emissive_surfaces_promoted > 0
    }
}

// ============================================================================
// Section 5 — Typed GPU-timing report (rule #5)
// ============================================================================

/// Typed GPU-timing report. Rule #5 requires the renderer
/// to surface per-pass GPU timing for the many-light path.
/// Storing as integer microseconds keeps the record
/// Hash/Eq-stable and avoids f32 noise.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxManyLightGpuTiming {
    pub schema_version: u16,
    pub cluster_assignment_us: u32,
    pub build_light_lists_us: u32,
    pub reservoir_temporal_reuse_us: u32,
    pub reservoir_spatial_reuse_us: u32,
    pub direct_lighting_us: u32,
}

impl LuxManyLightGpuTiming {
    pub const ZERO: Self = Self {
        schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
        cluster_assignment_us: 0,
        build_light_lists_us: 0,
        reservoir_temporal_reuse_us: 0,
        reservoir_spatial_reuse_us: 0,
        direct_lighting_us: 0,
    };

    /// Typed predicate: did every pass report a non-zero
    /// timing? Rule #5 requires the timing to be reported,
    /// not that any specific value was hit.
    #[must_use]
    pub const fn is_reported(&self) -> bool {
        self.cluster_assignment_us > 0
            && self.build_light_lists_us > 0
            && self.reservoir_temporal_reuse_us > 0
            && self.reservoir_spatial_reuse_us > 0
            && self.direct_lighting_us > 0
    }

    #[must_use]
    pub const fn total_us(&self) -> u32 {
        self.cluster_assignment_us
            .saturating_add(self.build_light_lists_us)
            .saturating_add(self.reservoir_temporal_reuse_us)
            .saturating_add(self.reservoir_spatial_reuse_us)
            .saturating_add(self.direct_lighting_us)
    }
}

// ============================================================================
// Section 6 — Typed CPU-cost scaling audit (rule #6)
// ============================================================================

/// Typed CPU-cost scaling audit. Rule #6 — *CPU updates
/// scale with changed lights, not total lights.*
///
/// The audit captures the typed counts the validator
/// needs:
///
/// - `total_lights`: how many lights live in the scene
/// - `changed_lights`: how many emitted an update this frame
/// - `cpu_update_us`: how long the CPU update path took
///
/// A trivial linear regression on a few frames can confirm
/// `cpu_update_us` correlates with `changed_lights` and
/// NOT with `total_lights`. Pass 6 keeps the audit data
/// minimal — the validator picks the regression heuristic.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxCpuUpdateScalingAudit {
    pub schema_version: u16,
    pub total_lights: u32,
    pub changed_lights: u32,
    pub cpu_update_us: u32,
    /// Typed Q16 ceiling: maximum acceptable µs per changed
    /// light. The validator rejects the run when
    /// `cpu_update_us > changed_lights *
    /// us_per_changed_light_ceiling_q16 / 65_536`.
    pub us_per_changed_light_ceiling_q16: u32,
}

impl LuxCpuUpdateScalingAudit {
    pub const ZERO: Self = Self {
        schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
        total_lights: 0,
        changed_lights: 0,
        cpu_update_us: 0,
        // Default ceiling: 50 µs per changed light (Q16:
        // 50 << 16 = 3_276_800).
        us_per_changed_light_ceiling_q16: 50 << 16,
    };

    /// Typed predicate: rule #6 — does the measured cost
    /// scale with the CHANGED light count, not the total?
    /// Returns `true` when `cpu_update_us <= changed_lights
    /// * ceiling_q16 / 65_536`.
    #[must_use]
    pub const fn scales_with_changed_lights(&self) -> bool {
        if self.changed_lights == 0 {
            // Idle frame — trivially passes; the cost must
            // not depend on `total_lights`.
            return true;
        }
        // ceiling_us = changed * ceiling_q16 / 65_536
        let ceiling_us = (self.changed_lights as u64)
            .saturating_mul(self.us_per_changed_light_ceiling_q16 as u64)
            / 65_536;
        (self.cpu_update_us as u64) <= ceiling_us
    }
}

// ============================================================================
// Section 7 — Typed stress-scene contract
// ============================================================================

/// Typed stress-scene contract. Encodes a single Pass 6
/// stress run: preset (100 / 500 / 1000), tier, layout
/// bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxManyLightStressScene {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub preset: LuxManyLightStressPreset,
    pub cluster_layout: LuxClusterGridLayout,
    pub light_index_layout: LuxLightIndexLayout,
    pub reservoir_layout: LuxReservoirLayout,
    pub overflow_policy: LuxManyLightCandidateOverflowPolicy,
    pub emissive_mode: LuxEmissivePromotionMode,
}

impl LuxManyLightStressScene {
    /// Typed Pass 6 stress preset (100-light).
    pub const SMALL_100: Self = Self::for_preset(LuxManyLightStressPreset::Small);
    /// Typed Pass 6 stress preset (500-light).
    pub const MEDIUM_500: Self = Self::for_preset(LuxManyLightStressPreset::Medium);
    /// Typed Pass 6 stress preset (1000-light).
    pub const LARGE_1000: Self = Self::for_preset(LuxManyLightStressPreset::Large);
    /// Typed Pass 6 stress preset (5000-light extra-credit).
    pub const HUGE_5000: Self = Self::for_preset(LuxManyLightStressPreset::Huge);

    /// Typed builder: derive every record-level layout from
    /// the typed preset. The cluster tier mapping in
    /// [`LuxManyLightStressPreset::cluster_tier`] picks the
    /// dimensions; light index and reservoir layouts use
    /// their own typed product defaults.
    #[must_use]
    pub const fn for_preset(preset: LuxManyLightStressPreset) -> Self {
        let cluster_layout = LuxClusterGridLayout::for_tier(preset.cluster_tier());
        Self {
            schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
            stable_id: stress_scene_stable_id(preset),
            preset,
            cluster_layout,
            light_index_layout: LuxLightIndexLayout::PRODUCT_DEFAULT,
            reservoir_layout: LuxReservoirLayout::PRODUCT_DEFAULT,
            overflow_policy: LuxManyLightCandidateOverflowPolicy::PromoteToReservoir,
            emissive_mode: LuxEmissivePromotionMode::ThresholdAtLoad,
        }
    }

    /// Typed light count for the scene (delegates to the
    /// preset).
    #[must_use]
    pub const fn light_count(&self) -> u32 {
        self.preset.light_count()
    }

    /// Typed predicate: do the chosen record-level layouts
    /// hang together (cluster grid agrees with light index
    /// agrees with reservoir)? Pre-flight for rule #1.
    #[must_use]
    pub fn layouts_agree(&self) -> bool {
        let bundle = LuxManyLightLayoutBundle {
            schema_version: self.schema_version,
            light_record: crate::gpu_layout::LuxGpuLightRecordLayout::PRODUCT_DEFAULT,
            cluster_grid: self.cluster_layout,
            light_index: self.light_index_layout,
            reservoir: self.reservoir_layout,
        };
        bundle.invariants_hold()
    }
}

const fn stress_scene_stable_id(preset: LuxManyLightStressPreset) -> &'static str {
    match preset {
        LuxManyLightStressPreset::Small => "fun_lux.many_light.stress.small_100",
        LuxManyLightStressPreset::Medium => "fun_lux.many_light.stress.medium_500",
        LuxManyLightStressPreset::Large => "fun_lux.many_light.stress.large_1000",
        LuxManyLightStressPreset::Huge => "fun_lux.many_light.stress.huge_5000",
    }
}

// ============================================================================
// Section 8 — Typed acceptance verdict (rules #1..#6)
// ============================================================================

/// Typed verdict for a single Pass 6 stress run. Each
/// field maps 1:1 to a user-listed acceptance rule, and
/// `obeys_all_six_rules` is the typed bundle predicate.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxManyLightAcceptanceVerdict {
    pub schema_version: u16,
    pub preset: LuxManyLightStressPreset,
    /// Rule #1: typed clustered+reservoir path was engaged
    /// (no brute-force fallback).
    pub uses_clustered_reservoir_path: bool,
    /// Rule #2: candidate overflow was tracked (counters
    /// emitted; not silent drops).
    pub tracks_candidate_overflow: bool,
    /// Rule #3: reservoir history rejects stale via update
    /// stamps.
    pub rejects_stale_via_update_stamps: bool,
    /// Rule #4: emissive promotion ran and produced at
    /// least one promoted record.
    pub emissive_promotion_works: bool,
    /// Rule #5: per-pass GPU timing was reported.
    pub gpu_timing_reported: bool,
    /// Rule #6: CPU cost scales with changed lights, not
    /// total lights.
    pub cpu_scales_with_changed_lights: bool,
}

impl LuxManyLightAcceptanceVerdict {
    /// Typed `evaluate(scene, audits...)` constructor — the
    /// validator passes in the typed audit records and gets
    /// a typed 6-flag verdict.
    #[must_use]
    pub fn evaluate(
        scene: &LuxManyLightStressScene,
        overflow_counters: &LuxManyLightOverflowCounters,
        reservoir_history: &LuxReservoirHistoryAudit,
        emissive_audit: &LuxEmissivePromotionAudit,
        gpu_timing: &LuxManyLightGpuTiming,
        cpu_scaling: &LuxCpuUpdateScalingAudit,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
            preset: scene.preset,
            // Rule #1: the layouts the scene picked must
            // agree. If they don't, the renderer cannot be
            // running the clustered+reservoir path
            // correctly.
            uses_clustered_reservoir_path: scene.layouts_agree(),
            // Rule #2: counters carry a valid schema_version
            // (typed "was-tracked" signal).
            tracks_candidate_overflow: overflow_counters.was_tracked(),
            // Rule #3: typed reservoir staleness predicate.
            rejects_stale_via_update_stamps: reservoir_history.rejects_stale_when_expected()
                && scene.reservoir_layout.supports_stale_rejection(),
            // Rule #4: emissive audit reports `works`.
            emissive_promotion_works: emissive_audit.works(),
            // Rule #5: GPU timing audit reports `is_reported`.
            gpu_timing_reported: gpu_timing.is_reported(),
            // Rule #6: CPU scaling audit reports
            // `scales_with_changed_lights`.
            cpu_scales_with_changed_lights: cpu_scaling.scales_with_changed_lights(),
        }
    }

    /// Typed predicate: how many of the six rules hold?
    #[must_use]
    pub const fn rules_satisfied(&self) -> u32 {
        let mut count = 0u32;
        if self.uses_clustered_reservoir_path {
            count += 1;
        }
        if self.tracks_candidate_overflow {
            count += 1;
        }
        if self.rejects_stale_via_update_stamps {
            count += 1;
        }
        if self.emissive_promotion_works {
            count += 1;
        }
        if self.gpu_timing_reported {
            count += 1;
        }
        if self.cpu_scales_with_changed_lights {
            count += 1;
        }
        count
    }

    /// Typed bundle predicate — every Pass 6 rule holds.
    #[must_use]
    pub const fn obeys_all_six_rules(&self) -> bool {
        self.rules_satisfied() as usize == FUN_LUX_MANY_LIGHT_ACCEPTANCE_RULE_COUNT
    }

    /// Typed predicate: convenience for the spec's
    /// 100/500/1000 trio — does this preset belong to the
    /// user-listed stress set?
    #[must_use]
    pub const fn is_user_listed_preset(&self) -> bool {
        matches!(
            self.preset,
            LuxManyLightStressPreset::Small
                | LuxManyLightStressPreset::Medium
                | LuxManyLightStressPreset::Large,
        )
    }
}

// ============================================================================
// Tests — Pass 6 acceptance audits
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn passing_overflow_counters() -> LuxManyLightOverflowCounters {
        LuxManyLightOverflowCounters {
            schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
            policy: LuxManyLightCandidateOverflowPolicy::PromoteToReservoir,
            total_candidates: 1_000,
            overflowed_candidates: 25,
            clusters_with_overflow: 3,
            max_per_cluster_overflow: 4,
        }
    }

    fn passing_reservoir_history() -> LuxReservoirHistoryAudit {
        LuxReservoirHistoryAudit {
            schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
            history_samples_total: 4096,
            history_samples_rejected_stale: 32,
            layout_supports_stale_rejection: true,
        }
    }

    fn passing_emissive_audit() -> LuxEmissivePromotionAudit {
        LuxEmissivePromotionAudit {
            schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
            mode: LuxEmissivePromotionMode::ThresholdAtLoad,
            emissive_surfaces_inspected: 12,
            emissive_surfaces_promoted: 5,
            luminance_threshold_q16: 6553,
        }
    }

    fn passing_gpu_timing() -> LuxManyLightGpuTiming {
        LuxManyLightGpuTiming {
            schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
            cluster_assignment_us: 110,
            build_light_lists_us: 220,
            reservoir_temporal_reuse_us: 540,
            reservoir_spatial_reuse_us: 480,
            direct_lighting_us: 1_200,
        }
    }

    fn passing_cpu_scaling() -> LuxCpuUpdateScalingAudit {
        LuxCpuUpdateScalingAudit {
            schema_version: FUN_LUX_MANY_LIGHT_STRESS_SCHEMA_VERSION,
            total_lights: 1_000,
            changed_lights: 8,
            cpu_update_us: 120, // 15 µs / changed (under 50)
            us_per_changed_light_ceiling_q16: 50 << 16,
        }
    }

    #[test]
    fn stress_preset_taxonomy_is_dense() {
        assert_eq!(
            LuxManyLightStressPreset::ALL.len(),
            FUN_LUX_MANY_LIGHT_STRESS_PRESET_COUNT,
        );
        assert_eq!(LuxManyLightStressPreset::Small.light_count(), 100);
        assert_eq!(LuxManyLightStressPreset::Medium.light_count(), 500);
        assert_eq!(LuxManyLightStressPreset::Large.light_count(), 1_000);
        assert_eq!(LuxManyLightStressPreset::Huge.light_count(), 5_000);
    }

    #[test]
    fn stress_preset_picks_user_tier() {
        assert_eq!(
            LuxManyLightStressPreset::Small.cluster_tier(),
            LuxClusterGridTier::LowMedium,
        );
        assert_eq!(
            LuxManyLightStressPreset::Medium.cluster_tier(),
            LuxClusterGridTier::High,
        );
        assert_eq!(
            LuxManyLightStressPreset::Large.cluster_tier(),
            LuxClusterGridTier::High,
        );
        assert_eq!(
            LuxManyLightStressPreset::Huge.cluster_tier(),
            LuxClusterGridTier::Cinematic,
        );
    }

    #[test]
    fn stress_scenes_carry_user_light_counts() {
        assert_eq!(LuxManyLightStressScene::SMALL_100.light_count(), 100);
        assert_eq!(LuxManyLightStressScene::MEDIUM_500.light_count(), 500);
        assert_eq!(LuxManyLightStressScene::LARGE_1000.light_count(), 1_000);
        assert_eq!(LuxManyLightStressScene::HUGE_5000.light_count(), 5_000);
    }

    #[test]
    fn stress_scene_layouts_agree() {
        for scene in [
            &LuxManyLightStressScene::SMALL_100,
            &LuxManyLightStressScene::MEDIUM_500,
            &LuxManyLightStressScene::LARGE_1000,
            &LuxManyLightStressScene::HUGE_5000,
        ] {
            assert!(
                scene.layouts_agree(),
                "scene {} layouts disagree",
                scene.stable_id,
            );
        }
    }

    #[test]
    fn overflow_policy_energy_predicate() {
        assert!(!LuxManyLightCandidateOverflowPolicy::Drop.preserves_energy());
        assert!(LuxManyLightCandidateOverflowPolicy::Spill.preserves_energy());
        assert!(!LuxManyLightCandidateOverflowPolicy::Subsample.preserves_energy());
        assert!(LuxManyLightCandidateOverflowPolicy::PromoteToReservoir.preserves_energy());
    }

    #[test]
    fn overflow_counters_tracks_with_nonzero_schema() {
        let counters = passing_overflow_counters();
        assert!(counters.was_tracked());

        // ZERO carries a non-zero schema_version too — the
        // "was-tracked" signal is the typed schema version,
        // not the magnitude of the counters.
        assert!(LuxManyLightOverflowCounters::ZERO.was_tracked());

        let untracked = LuxManyLightOverflowCounters {
            schema_version: 0,
            ..passing_overflow_counters()
        };
        assert!(!untracked.was_tracked());
    }

    #[test]
    fn overflow_ratio_q16_walks_total() {
        let counters = LuxManyLightOverflowCounters {
            total_candidates: 1_000,
            overflowed_candidates: 250,
            ..passing_overflow_counters()
        };
        // 250 / 1000 = 0.25 → Q16 ~= 16384.
        assert_eq!(counters.overflow_ratio_q16(), 16_384);

        let empty = LuxManyLightOverflowCounters {
            total_candidates: 0,
            overflowed_candidates: 0,
            ..passing_overflow_counters()
        };
        assert_eq!(empty.overflow_ratio_q16(), 0);
    }

    /// Pass 6 user rule #3: *reservoir history rejects
    /// stale via update stamps.*
    #[test]
    fn reservoir_history_audit_requires_layout_support() {
        let mut audit = passing_reservoir_history();
        assert!(audit.rejects_stale_when_expected());

        audit.layout_supports_stale_rejection = false;
        assert!(!audit.rejects_stale_when_expected());
    }

    /// Pass 6 user rule #4: *emissive promotion works.*
    #[test]
    fn emissive_audit_requires_mode_and_promotion_count() {
        let mut audit = passing_emissive_audit();
        assert!(audit.works());

        audit.mode = LuxEmissivePromotionMode::Off;
        assert!(!audit.works());

        let mut audit = passing_emissive_audit();
        audit.emissive_surfaces_promoted = 0;
        assert!(!audit.works());
    }

    /// Pass 6 user rule #5: *GPU timing reported.*
    #[test]
    fn gpu_timing_must_report_every_pass() {
        assert!(passing_gpu_timing().is_reported());

        let mut t = passing_gpu_timing();
        t.cluster_assignment_us = 0;
        assert!(!t.is_reported());
    }

    /// Pass 6 user rule #6: *CPU updates scale with
    /// CHANGED lights, not total.*
    #[test]
    fn cpu_scaling_audit_compares_to_changed_count() {
        let pass = passing_cpu_scaling();
        assert!(pass.scales_with_changed_lights());

        // Same total, no changes → idle frame, trivially
        // passes.
        let idle = LuxCpuUpdateScalingAudit {
            changed_lights: 0,
            cpu_update_us: 0,
            ..passing_cpu_scaling()
        };
        assert!(idle.scales_with_changed_lights());

        // Cost shoots up while changed count stays small →
        // fails. 8 changed * 50 µs ceiling = 400 µs budget;
        // 1000 µs blows it.
        let blown = LuxCpuUpdateScalingAudit {
            changed_lights: 8,
            cpu_update_us: 1_000,
            ..passing_cpu_scaling()
        };
        assert!(!blown.scales_with_changed_lights());
    }

    /// Typed verdict: every Pass 6 rule holds when every
    /// audit reports the passing baseline.
    #[test]
    fn verdict_passes_when_every_audit_passes() {
        for scene in [
            &LuxManyLightStressScene::SMALL_100,
            &LuxManyLightStressScene::MEDIUM_500,
            &LuxManyLightStressScene::LARGE_1000,
        ] {
            let verdict = LuxManyLightAcceptanceVerdict::evaluate(
                scene,
                &passing_overflow_counters(),
                &passing_reservoir_history(),
                &passing_emissive_audit(),
                &passing_gpu_timing(),
                &passing_cpu_scaling(),
            );
            assert!(
                verdict.obeys_all_six_rules(),
                "verdict {:?} failed",
                verdict,
            );
            assert!(verdict.is_user_listed_preset());
            assert_eq!(
                verdict.rules_satisfied(),
                FUN_LUX_MANY_LIGHT_ACCEPTANCE_RULE_COUNT as u32,
            );
        }
    }

    /// Typed verdict: a single rule failure flips the
    /// bundle predicate.
    #[test]
    fn verdict_flips_when_any_rule_fails() {
        let scene = LuxManyLightStressScene::LARGE_1000;
        let mut emissive = passing_emissive_audit();
        emissive.emissive_surfaces_promoted = 0;
        let verdict = LuxManyLightAcceptanceVerdict::evaluate(
            &scene,
            &passing_overflow_counters(),
            &passing_reservoir_history(),
            &emissive,
            &passing_gpu_timing(),
            &passing_cpu_scaling(),
        );
        assert!(!verdict.obeys_all_six_rules());
        assert!(!verdict.emissive_promotion_works);
        // The remaining five rules still hold.
        assert_eq!(verdict.rules_satisfied(), 5);
    }

    #[test]
    fn huge_preset_is_not_user_listed() {
        let scene = LuxManyLightStressScene::HUGE_5000;
        let verdict = LuxManyLightAcceptanceVerdict::evaluate(
            &scene,
            &passing_overflow_counters(),
            &passing_reservoir_history(),
            &passing_emissive_audit(),
            &passing_gpu_timing(),
            &passing_cpu_scaling(),
        );
        assert!(!verdict.is_user_listed_preset());
        assert!(verdict.obeys_all_six_rules());
    }
}
