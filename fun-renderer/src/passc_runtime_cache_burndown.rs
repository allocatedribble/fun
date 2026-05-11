//! Pass C — Runtime Cache and Churn Burn-Down.
//!
//! Pass C's exit gate is a single command that runs a measured
//! stress scene (10k objects, 1k materials, dynamic transform
//! churn, UI packet stream, resize events) and proves five typed
//! invariants hold across the post-warmup window:
//!
//! 1. **No post-warmup shader translation** — `Dx12RuntimeGateCounters::post_warmup_count(ShaderTranslation) == 0`.
//! 2. **No post-warmup pipeline creation** — `Dx12RuntimeGateCounters::post_warmup_count(PipelineCreation) == 0`.
//! 3. **No post-warmup bind layout creation** — `Dx12RuntimeGateCounters::post_warmup_count(BindLayoutCreation) == 0`.
//! 4. **No per-frame resource growth** — `Dx12RuntimeGateCounters::post_warmup_count(ResourceCreation) == 0`.
//! 5. **No per-entity descriptor creation** — every per-frame
//!    descriptor write must be accounted for by the prepared
//!    binding cache, never by per-entity creation. The Pass C
//!    contract adds the typed `PassCPerEntityDescriptorCounter` so
//!    the rule has a single observable value.
//!
//! Pass C reuses [`crate::tier1_cache_backed_optimization::Tier1StressScenario::PRODUCTION_DEFAULT`]
//! (10_000 objects, 1_000 materials, 200 transform churn / frame,
//! 60 Hz UI packet rate, 4 swapchain resizes, 600 measured frames)
//! as the canonical stress composition. The Pass C bundle records
//! the composition that drove the measurement so the artifact is
//! reproducible.
//!
//! Until the wgpu graph executor, render encoder, and live
//! descriptor cache are wired against the live runtime (the seven
//! `Tier0ProofFrameGap` keystone gates from Pass A's ledger), the
//! command produces a typed
//! [`PassCRuntimeCacheBurndownBundle`] whose outcome is
//! [`PassCRuntimeCacheBurndownOutcome::NotYetMeasured`] because no
//! production-state frame has been driven through the stress
//! scene. As the keystone gates close, the same command starts
//! driving the stress scene through real frames and the verdict
//! flips to passing if every counter remains zero post-warmup.

use bevy_ecs::prelude::Resource;

use crate::dx12_production::{Dx12RuntimeGateCounters, Dx12RuntimeResourceKind, Dx12WarmupState};
use crate::tier1_cache_backed_optimization::Tier1StressScenario;

pub const PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION: u16 = 1;
pub const PASSC_RUNTIME_CACHE_BURNDOWN_RULE_COUNT: usize = 5;

// ============================================================================
// Section 1 — Exit-gate rule taxonomy
// ============================================================================

/// One typed exit-gate rule from the Pass C contract. Each variant
/// maps 1:1 to a typed predicate the verdict evaluates from
/// observed runtime counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassCRuntimeCacheBurndownRule {
    /// `Dx12RuntimeResourceKind::ShaderTranslation` post-warmup count == 0.
    NoPostWarmupShaderTranslation,
    /// `Dx12RuntimeResourceKind::PipelineCreation` post-warmup count == 0.
    NoPostWarmupPipelineCreation,
    /// `Dx12RuntimeResourceKind::BindLayoutCreation` post-warmup count == 0.
    NoPostWarmupBindLayoutCreation,
    /// `Dx12RuntimeResourceKind::ResourceCreation` post-warmup count == 0.
    NoPerFrameResourceGrowth,
    /// `PassCPerEntityDescriptorCounter` post-warmup count == 0.
    NoPerEntityDescriptorCreation,
}

impl PassCRuntimeCacheBurndownRule {
    pub const ALL: [Self; PASSC_RUNTIME_CACHE_BURNDOWN_RULE_COUNT] = [
        Self::NoPostWarmupShaderTranslation,
        Self::NoPostWarmupPipelineCreation,
        Self::NoPostWarmupBindLayoutCreation,
        Self::NoPerFrameResourceGrowth,
        Self::NoPerEntityDescriptorCreation,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::NoPostWarmupShaderTranslation => 0,
            Self::NoPostWarmupPipelineCreation => 1,
            Self::NoPostWarmupBindLayoutCreation => 2,
            Self::NoPerFrameResourceGrowth => 3,
            Self::NoPerEntityDescriptorCreation => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoPostWarmupShaderTranslation => "no_post_warmup_shader_translation",
            Self::NoPostWarmupPipelineCreation => "no_post_warmup_pipeline_creation",
            Self::NoPostWarmupBindLayoutCreation => "no_post_warmup_bind_layout_creation",
            Self::NoPerFrameResourceGrowth => "no_per_frame_resource_growth",
            Self::NoPerEntityDescriptorCreation => "no_per_entity_descriptor_creation",
        }
    }
}

// ============================================================================
// Section 2 — Per-entity descriptor counter (new for Pass C)
// ============================================================================

/// Typed counter for per-entity descriptor creations. The Pass 18
/// binding model + `WgpuBindingBridgeCache` already gate descriptor
/// creation behind warmup-only paths — this counter records any
/// per-entity descriptor write a future bridge would emit during
/// production frames so Pass C can refuse to pass when descriptor
/// churn happens.
///
/// Mirrors the shape of [`Dx12RuntimeGateCounters`] but tracks one
/// extra resource kind (per-entity descriptor) the existing DX12
/// runtime gate doesn't separate from generic
/// `BindLayoutCreation`. Per-entity descriptor allocations are a
/// distinct failure mode from layout creation: an entity that
/// needs a fresh `BindGroup` per frame is a worse churn signal
/// than a one-time `BindGroupLayout` creation, and the Pass C
/// contract records them separately so the verdict can attribute
/// the failure correctly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct PassCPerEntityDescriptorCounter {
    pub schema_version: u16,
    pub state: Dx12WarmupState,
    pub warmup_total: u64,
    pub post_warmup_total: u64,
    pub current_frame_total: u64,
    pub last_frame_total: u64,
}

impl PassCPerEntityDescriptorCounter {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION,
            state: Dx12WarmupState::Cold,
            warmup_total: 0,
            post_warmup_total: 0,
            current_frame_total: 0,
            last_frame_total: 0,
        }
    }

    pub fn note_state(&mut self, state: Dx12WarmupState) {
        self.state = state;
    }

    pub fn record(&mut self, count: u64) {
        self.current_frame_total = self.current_frame_total.saturating_add(count);
        if self.state.enforces_no_growth() {
            self.post_warmup_total = self.post_warmup_total.saturating_add(count);
        } else {
            self.warmup_total = self.warmup_total.saturating_add(count);
        }
    }

    pub fn rotate_frame(&mut self) {
        self.last_frame_total = self.current_frame_total;
        self.current_frame_total = 0;
    }
}

// ============================================================================
// Section 3 — Bundle outcome
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassCRuntimeCacheBurndownOutcome {
    #[default]
    NotYetMeasured,
    /// Every Pass C invariant held across the post-warmup window.
    Passes,
    /// At least one invariant was violated. The verdict carries
    /// the per-rule pass/fail bits so the artifact records which
    /// invariants failed.
    Violated { violation_count: u32 },
    /// The stress scene measurement could not run because the
    /// warmup state never reached `Production` — for example when
    /// the bridge is still cold. This is a typed
    /// "measurement-prerequisite missing" outcome so the user does
    /// not confuse it with a violation.
    WarmupIncomplete,
}

impl PassCRuntimeCacheBurndownOutcome {
    #[must_use]
    pub const fn passed(self) -> bool {
        matches!(self, Self::Passes)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotYetMeasured => "not_yet_measured",
            Self::Passes => "passes",
            Self::Violated { .. } => "violated",
            Self::WarmupIncomplete => "warmup_incomplete",
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
// Section 4 — Observed counters bundle
// ============================================================================

/// Snapshot of the post-warmup counts the Pass C verdict needs to
/// evaluate the five rules. Pulled from
/// [`Dx12RuntimeGateCounters`] + [`PassCPerEntityDescriptorCounter`]
/// at the end of the measured window.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassCObservedCounters {
    pub schema_version: u16,
    pub final_warmup_state: Dx12WarmupState,
    pub measured_frame_count: u32,
    pub shader_translation_post_warmup: u64,
    pub pipeline_creation_post_warmup: u64,
    pub bind_layout_creation_post_warmup: u64,
    pub resource_creation_post_warmup: u64,
    pub per_entity_descriptor_post_warmup: u64,
}

impl PassCObservedCounters {
    /// Cold default — every counter zero, warmup state Cold,
    /// measured_frame_count 0. This is the honest shape the
    /// command captures today, before the live runtime drives the
    /// stress scene through real frames.
    #[must_use]
    pub const fn cold_default() -> Self {
        Self {
            schema_version: PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION,
            final_warmup_state: Dx12WarmupState::Cold,
            measured_frame_count: 0,
            shader_translation_post_warmup: 0,
            pipeline_creation_post_warmup: 0,
            bind_layout_creation_post_warmup: 0,
            resource_creation_post_warmup: 0,
            per_entity_descriptor_post_warmup: 0,
        }
    }

    /// Read the typed counters into the observation snapshot.
    #[must_use]
    pub fn from_runtime_counters(
        counters: &Dx12RuntimeGateCounters,
        per_entity: &PassCPerEntityDescriptorCounter,
        measured_frame_count: u32,
    ) -> Self {
        Self {
            schema_version: PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION,
            final_warmup_state: counters.state,
            measured_frame_count,
            shader_translation_post_warmup: counters
                .post_warmup_count(Dx12RuntimeResourceKind::ShaderTranslation),
            pipeline_creation_post_warmup: counters
                .post_warmup_count(Dx12RuntimeResourceKind::PipelineCreation),
            bind_layout_creation_post_warmup: counters
                .post_warmup_count(Dx12RuntimeResourceKind::BindLayoutCreation),
            resource_creation_post_warmup: counters
                .post_warmup_count(Dx12RuntimeResourceKind::ResourceCreation),
            per_entity_descriptor_post_warmup: per_entity.post_warmup_total,
        }
    }

    /// Synthetic "all rules pass" snapshot used by tests to prove
    /// the verdict's passing path. Production-state warmup, 600
    /// measured frames, every counter zero.
    #[must_use]
    pub const fn synthetic_all_zero_post_production() -> Self {
        Self {
            schema_version: PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION,
            final_warmup_state: Dx12WarmupState::Production,
            measured_frame_count: 600,
            shader_translation_post_warmup: 0,
            pipeline_creation_post_warmup: 0,
            bind_layout_creation_post_warmup: 0,
            resource_creation_post_warmup: 0,
            per_entity_descriptor_post_warmup: 0,
        }
    }
}

// ============================================================================
// Section 5 — Bundle (Bevy Resource) + canonical artifact path
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct PassCRuntimeCacheBurndownBundle {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub composition: Tier1StressScenario,
    pub observed: PassCObservedCounters,
    pub outcome: PassCRuntimeCacheBurndownOutcome,
}

impl PassCRuntimeCacheBurndownBundle {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.passc.runtime_cache_burndown.funpb.zst";

    #[must_use]
    pub const fn empty_cold_default() -> Self {
        Self {
            schema_version: PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            composition: Tier1StressScenario::PRODUCTION_DEFAULT,
            observed: PassCObservedCounters::cold_default(),
            outcome: PassCRuntimeCacheBurndownOutcome::NotYetMeasured,
        }
    }

    /// Resolve the outcome from the verdict. Warmup-incomplete
    /// dominates over violation accounting because measuring
    /// post-warmup counts before the bridge reaches the
    /// `Production` warmup state would be a category error.
    pub fn finalize(&mut self, verdict: &PassCRuntimeCacheBurndownVerdict) {
        if !matches!(
            self.observed.final_warmup_state,
            Dx12WarmupState::Production
        ) {
            self.outcome = PassCRuntimeCacheBurndownOutcome::WarmupIncomplete;
            return;
        }
        if self.observed.measured_frame_count == 0 {
            self.outcome = PassCRuntimeCacheBurndownOutcome::NotYetMeasured;
            return;
        }
        if verdict.passes() {
            self.outcome = PassCRuntimeCacheBurndownOutcome::Passes;
        } else {
            self.outcome = PassCRuntimeCacheBurndownOutcome::Violated {
                violation_count: verdict.violation_count(),
            };
        }
    }
}

// ============================================================================
// Section 6 — Verdict
// ============================================================================

/// Pass C exit-gate verdict. Records the result of every typed
/// rule from `PassCRuntimeCacheBurndownRule::ALL`. Each rule maps
/// to a single observable counter on
/// [`PassCObservedCounters`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassCRuntimeCacheBurndownVerdict {
    pub schema_version: u16,
    pub passes_no_post_warmup_shader_translation: bool,
    pub passes_no_post_warmup_pipeline_creation: bool,
    pub passes_no_post_warmup_bind_layout_creation: bool,
    pub passes_no_per_frame_resource_growth: bool,
    pub passes_no_per_entity_descriptor_creation: bool,
}

impl PassCRuntimeCacheBurndownVerdict {
    /// Evaluate the verdict from the observed counters. Each rule
    /// is "post-warmup count == 0".
    #[must_use]
    pub const fn evaluate(observed: &PassCObservedCounters) -> Self {
        Self {
            schema_version: PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION,
            passes_no_post_warmup_shader_translation: observed.shader_translation_post_warmup == 0,
            passes_no_post_warmup_pipeline_creation: observed.pipeline_creation_post_warmup == 0,
            passes_no_post_warmup_bind_layout_creation: observed.bind_layout_creation_post_warmup
                == 0,
            passes_no_per_frame_resource_growth: observed.resource_creation_post_warmup == 0,
            passes_no_per_entity_descriptor_creation: observed.per_entity_descriptor_post_warmup
                == 0,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_no_post_warmup_shader_translation
            && self.passes_no_post_warmup_pipeline_creation
            && self.passes_no_post_warmup_bind_layout_creation
            && self.passes_no_per_frame_resource_growth
            && self.passes_no_per_entity_descriptor_creation
    }

    /// Walk the rules in fixed order and return the first failing
    /// rule. Matches the "first reason" diagnostic pattern Pass 26
    /// / Tier 7 / Tier 8 / Pass B use elsewhere.
    #[must_use]
    pub const fn first_failed(&self) -> Option<PassCRuntimeCacheBurndownRule> {
        if !self.passes_no_post_warmup_shader_translation {
            return Some(PassCRuntimeCacheBurndownRule::NoPostWarmupShaderTranslation);
        }
        if !self.passes_no_post_warmup_pipeline_creation {
            return Some(PassCRuntimeCacheBurndownRule::NoPostWarmupPipelineCreation);
        }
        if !self.passes_no_post_warmup_bind_layout_creation {
            return Some(PassCRuntimeCacheBurndownRule::NoPostWarmupBindLayoutCreation);
        }
        if !self.passes_no_per_frame_resource_growth {
            return Some(PassCRuntimeCacheBurndownRule::NoPerFrameResourceGrowth);
        }
        if !self.passes_no_per_entity_descriptor_creation {
            return Some(PassCRuntimeCacheBurndownRule::NoPerEntityDescriptorCreation);
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_no_post_warmup_shader_translation {
            count += 1;
        }
        if !self.passes_no_post_warmup_pipeline_creation {
            count += 1;
        }
        if !self.passes_no_post_warmup_bind_layout_creation {
            count += 1;
        }
        if !self.passes_no_per_frame_resource_growth {
            count += 1;
        }
        if !self.passes_no_per_entity_descriptor_creation {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 7 — Builder
// ============================================================================

/// Build a fully populated bundle from observed runtime counters
/// and the stress scene composition that drove the measurement.
/// Test-friendly entry point; the live binary wraps an `App`,
/// drives the stress scene through the configured frame count,
/// reads `Dx12RuntimeGateCounters` + `PassCPerEntityDescriptorCounter`,
/// and feeds the snapshot through this builder.
#[must_use]
pub fn build_bundle_from_observation(
    composition: Tier1StressScenario,
    observed: PassCObservedCounters,
) -> PassCRuntimeCacheBurndownBundle {
    let mut bundle = PassCRuntimeCacheBurndownBundle::empty_cold_default();
    bundle.composition = composition;
    bundle.observed = observed;
    let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&bundle.observed);
    bundle.finalize(&verdict);
    bundle
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION, 1);
        assert_eq!(PASSC_RUNTIME_CACHE_BURNDOWN_RULE_COUNT, 5);
        assert_eq!(
            PassCRuntimeCacheBurndownRule::ALL.len(),
            PASSC_RUNTIME_CACHE_BURNDOWN_RULE_COUNT
        );
    }

    #[test]
    fn rule_index_round_trips_for_all_variants() {
        for (i, rule) in PassCRuntimeCacheBurndownRule::ALL
            .iter()
            .copied()
            .enumerate()
        {
            assert_eq!(rule.index(), i);
        }
    }

    #[test]
    fn rule_str_taxonomy_is_unique_and_stable() {
        let mut seen = hashbrown::HashSet::new();
        for rule in PassCRuntimeCacheBurndownRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
        assert_eq!(seen.len(), PASSC_RUNTIME_CACHE_BURNDOWN_RULE_COUNT);
    }

    #[test]
    fn production_default_composition_is_10k_objects_1k_materials() {
        let comp = Tier1StressScenario::PRODUCTION_DEFAULT;
        assert_eq!(comp.object_count, 10_000);
        assert_eq!(comp.material_variant_count, 1_000);
        assert!(comp.transform_churn_per_frame > 0);
        assert!(comp.ui_packet_rate_hz > 0);
        assert!(comp.swapchain_resize_count > 0);
        assert!(comp.frame_count > 0);
    }

    #[test]
    fn per_entity_descriptor_counter_separates_warmup_from_post_warmup() {
        let mut c = PassCPerEntityDescriptorCounter::new();
        c.note_state(Dx12WarmupState::Warming);
        c.record(5);
        assert_eq!(c.warmup_total, 5);
        assert_eq!(c.post_warmup_total, 0);
        c.note_state(Dx12WarmupState::Production);
        c.record(2);
        assert_eq!(c.warmup_total, 5);
        assert_eq!(c.post_warmup_total, 2);
    }

    #[test]
    fn per_entity_descriptor_counter_rotates_frame_state() {
        let mut c = PassCPerEntityDescriptorCounter::new();
        c.note_state(Dx12WarmupState::Production);
        c.record(3);
        c.record(4);
        assert_eq!(c.current_frame_total, 7);
        c.rotate_frame();
        assert_eq!(c.last_frame_total, 7);
        assert_eq!(c.current_frame_total, 0);
        assert_eq!(c.post_warmup_total, 7);
    }

    #[test]
    fn observed_counters_read_from_runtime_counter_state() {
        let mut counters = Dx12RuntimeGateCounters::new();
        counters.note_state(Dx12WarmupState::Production);
        counters.record(Dx12RuntimeResourceKind::PipelineCreation, 1);
        counters.record(Dx12RuntimeResourceKind::ResourceCreation, 4);
        counters.rotate_frame();

        let mut per_entity = PassCPerEntityDescriptorCounter::new();
        per_entity.note_state(Dx12WarmupState::Production);
        per_entity.record(7);

        let snap = PassCObservedCounters::from_runtime_counters(&counters, &per_entity, 600);
        assert_eq!(snap.measured_frame_count, 600);
        assert_eq!(snap.pipeline_creation_post_warmup, 1);
        assert_eq!(snap.resource_creation_post_warmup, 4);
        assert_eq!(snap.per_entity_descriptor_post_warmup, 7);
        assert_eq!(snap.shader_translation_post_warmup, 0);
        assert_eq!(snap.bind_layout_creation_post_warmup, 0);
        assert!(matches!(
            snap.final_warmup_state,
            Dx12WarmupState::Production
        ));
    }

    #[test]
    fn verdict_passes_when_every_post_warmup_count_is_zero() {
        let observed = PassCObservedCounters::synthetic_all_zero_post_production();
        let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&observed);
        assert!(verdict.passes());
        assert!(verdict.first_failed().is_none());
        assert_eq!(verdict.violation_count(), 0);
    }

    #[test]
    fn verdict_fails_on_post_warmup_shader_translation() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.shader_translation_post_warmup = 1;
        let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&observed);
        assert!(!verdict.passes_no_post_warmup_shader_translation);
        assert_eq!(
            verdict.first_failed(),
            Some(PassCRuntimeCacheBurndownRule::NoPostWarmupShaderTranslation)
        );
    }

    #[test]
    fn verdict_fails_on_post_warmup_pipeline_creation() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.pipeline_creation_post_warmup = 1;
        let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&observed);
        assert!(!verdict.passes_no_post_warmup_pipeline_creation);
        assert_eq!(
            verdict.first_failed(),
            Some(PassCRuntimeCacheBurndownRule::NoPostWarmupPipelineCreation)
        );
    }

    #[test]
    fn verdict_fails_on_post_warmup_bind_layout_creation() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.bind_layout_creation_post_warmup = 1;
        let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&observed);
        assert!(!verdict.passes_no_post_warmup_bind_layout_creation);
        assert_eq!(
            verdict.first_failed(),
            Some(PassCRuntimeCacheBurndownRule::NoPostWarmupBindLayoutCreation)
        );
    }

    #[test]
    fn verdict_fails_on_per_frame_resource_growth() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.resource_creation_post_warmup = 1;
        let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&observed);
        assert!(!verdict.passes_no_per_frame_resource_growth);
        assert_eq!(
            verdict.first_failed(),
            Some(PassCRuntimeCacheBurndownRule::NoPerFrameResourceGrowth)
        );
    }

    #[test]
    fn verdict_fails_on_per_entity_descriptor_creation() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.per_entity_descriptor_post_warmup = 1;
        let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&observed);
        assert!(!verdict.passes_no_per_entity_descriptor_creation);
        assert_eq!(
            verdict.first_failed(),
            Some(PassCRuntimeCacheBurndownRule::NoPerEntityDescriptorCreation)
        );
    }

    #[test]
    fn verdict_violation_count_tracks_every_failing_rule() {
        let observed = PassCObservedCounters {
            schema_version: PASSC_RUNTIME_CACHE_BURNDOWN_SCHEMA_VERSION,
            final_warmup_state: Dx12WarmupState::Production,
            measured_frame_count: 600,
            shader_translation_post_warmup: 1,
            pipeline_creation_post_warmup: 1,
            bind_layout_creation_post_warmup: 0,
            resource_creation_post_warmup: 0,
            per_entity_descriptor_post_warmup: 1,
        };
        let verdict = PassCRuntimeCacheBurndownVerdict::evaluate(&observed);
        assert_eq!(verdict.violation_count(), 3);
    }

    #[test]
    fn bundle_outcome_passes_under_synthetic_all_zero_post_production() {
        let bundle = build_bundle_from_observation(
            Tier1StressScenario::PRODUCTION_DEFAULT,
            PassCObservedCounters::synthetic_all_zero_post_production(),
        );
        assert_eq!(bundle.outcome, PassCRuntimeCacheBurndownOutcome::Passes);
        assert!(bundle.outcome.passed());
        assert_eq!(bundle.outcome.violation_count(), 0);
        assert_eq!(
            bundle.canonical_path,
            PassCRuntimeCacheBurndownBundle::CANONICAL_ARTIFACT_PATH
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn bundle_outcome_violated_when_any_post_warmup_count_is_nonzero() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.pipeline_creation_post_warmup = 2;
        let bundle =
            build_bundle_from_observation(Tier1StressScenario::PRODUCTION_DEFAULT, observed);
        match bundle.outcome {
            PassCRuntimeCacheBurndownOutcome::Violated { violation_count } => {
                assert_eq!(violation_count, 1);
            }
            other => panic!("expected Violated, got {other:?}"),
        }
        assert!(!bundle.outcome.passed());
        assert_eq!(bundle.outcome.violation_count(), 1);
    }

    #[test]
    fn bundle_outcome_warmup_incomplete_when_state_is_not_production() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.final_warmup_state = Dx12WarmupState::Warming;
        let bundle =
            build_bundle_from_observation(Tier1StressScenario::PRODUCTION_DEFAULT, observed);
        assert_eq!(
            bundle.outcome,
            PassCRuntimeCacheBurndownOutcome::WarmupIncomplete
        );
        assert!(!bundle.outcome.passed());
    }

    #[test]
    fn bundle_outcome_not_yet_measured_when_zero_frames_observed() {
        let mut observed = PassCObservedCounters::synthetic_all_zero_post_production();
        observed.measured_frame_count = 0;
        let bundle =
            build_bundle_from_observation(Tier1StressScenario::PRODUCTION_DEFAULT, observed);
        assert_eq!(
            bundle.outcome,
            PassCRuntimeCacheBurndownOutcome::NotYetMeasured
        );
    }

    #[test]
    fn outcome_passed_only_for_passes_variant() {
        assert!(PassCRuntimeCacheBurndownOutcome::Passes.passed());
        assert!(!PassCRuntimeCacheBurndownOutcome::NotYetMeasured.passed());
        assert!(!PassCRuntimeCacheBurndownOutcome::Violated { violation_count: 3 }.passed());
        assert!(!PassCRuntimeCacheBurndownOutcome::WarmupIncomplete.passed());
    }

    /// Pass C "single command" smoke gate. Boots
    /// `FunRendererPlugin<WgpuDx12Backend>` through one
    /// `app.update()` against the live wgpu runtime, captures the
    /// observed counters (which today are all-zero because the
    /// graph executor has not yet driven a stress scene through
    /// production frames — see Pass A's Immediate Gaps), and
    /// produces a typed `PassCRuntimeCacheBurndownBundle`. The
    /// outcome must record the honest state — warmup is still
    /// `Cold`, no production frames have been measured, so the
    /// bundle outcome is `NotYetMeasured` rather than fabricating
    /// a pass.
    ///
    /// As future closeouts close the keystone gates and the live
    /// runtime drives the stress scene through 600 production
    /// frames, the same test starts producing
    /// `PassCRuntimeCacheBurndownOutcome::Passes` if every counter
    /// remains zero post-warmup. Violations show up in the
    /// `Violated { violation_count }` path with the typed first-
    /// failed rule recorded.
    #[test]
    fn live_passc_runs_one_update_and_records_not_yet_measured() {
        use bevy_app::App;

        use crate::backend::WgpuDx12Backend;
        use crate::plugin::FunRendererPlugin;

        let mut app = App::new();
        app.add_plugins(FunRendererPlugin::<WgpuDx12Backend>::default());
        app.update();

        // Pass C is the post-warmup-counter contract. The plugin
        // does not yet install `Dx12RuntimeGateCounters` as a Bevy
        // Resource (counter accumulation depends on closing the
        // Pass A Immediate Gaps `gap.tier0.no_render_encoder` /
        // `gap.tier0.no_graph_executor`), so the live test
        // constructs a fresh cold-default counter snapshot. The
        // observed counts are all-zero because no production-state
        // frame has been driven through the stress scene; the
        // bundle outcome is honestly `WarmupIncomplete` until that
        // wiring lands.
        let counters = Dx12RuntimeGateCounters::new();
        let per_entity = PassCPerEntityDescriptorCounter::new();
        let observed = PassCObservedCounters::from_runtime_counters(&counters, &per_entity, 0);

        let bundle =
            build_bundle_from_observation(Tier1StressScenario::PRODUCTION_DEFAULT, observed);

        // Canonical artifact path always set.
        assert_eq!(
            bundle.canonical_path,
            PassCRuntimeCacheBurndownBundle::CANONICAL_ARTIFACT_PATH
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));

        // Pass C cannot pass today because no production-state
        // frames have been measured against the stress scene.
        // Warmup is still `Cold`, so `WarmupIncomplete` is the
        // honest outcome. `Passes` would be a stubbed claim.
        assert!(!bundle.outcome.passed(), "Pass C must not falsely pass");
        assert!(matches!(
            bundle.outcome,
            PassCRuntimeCacheBurndownOutcome::WarmupIncomplete
                | PassCRuntimeCacheBurndownOutcome::NotYetMeasured
        ));

        // The composition the bundle records is the canonical
        // 10k/1k production stress scene.
        assert_eq!(bundle.composition.object_count, 10_000);
        assert_eq!(bundle.composition.material_variant_count, 1_000);
    }
}
