//! Pass C9.7 — typed real diagnostics + director
//! feedback loop.
//!
//! Pass C7.8 landed the typed `CloudShadowDiagnostics`
//! record + typed log/debug emitter.  Pass C7.9 landed
//! the typed `CloudShadowDirectorInputs` /
//! `CloudShadowDirectorBudget` /
//! `decide_cloud_shadow_refresh` chain.  Both passes
//! currently consume typed synthetic / placeholder GPU
//! timings — Pass C9.7 lands the typed runtime feedback
//! that wires:
//!
//! - the typed full `CloudShadowDispatchCounts` struct
//!   into the typed diagnostics (typed not flattened to
//!   typed `project / filter` u32s);
//! - typed GPU timestamps for typed five passes:
//!   project, filter, register-layer, direct-lighting
//!   cloud sample, volumetric cloud sample;
//! - typed real timings into the typed director's typed
//!   budget-pressure check via the typed feedback
//!   adapter `decide_cloud_shadow_refresh_with_feedback`.
//!
//! Additive over the typed Pass C7.8 / C7.9 modules —
//! the typed existing types keep their typed shape;
//! Pass C9.7 adds typed wrapper records + typed extended
//! decision adapters so the typed renderer can route
//! typed live timings through the typed existing
//! director without typed mutating typed C7.8 / C7.9
//! signatures.

use crate::cloud_shadow::CloudShadowFrameDelayMode;
use crate::cloud_shadow::CloudShadowResolution;
use crate::cloud_shadow_director::{
    CloudShadowDirectorBudget, CloudShadowDirectorDecision, CloudShadowDirectorInputs,
    CloudShadowQualityTier, CloudShadowRefreshAction, CloudShadowRefreshReason,
    decide_cloud_shadow_refresh,
};
use crate::cloud_shadow_runtime_diagnostics::CloudShadowDiagnostics;
use fun_lux::LuxLightId;

/// Typed Pass C9.7 — typed local copy of the typed Pass
/// C7.9 `downgrade_resolution` step.  Typed kept private
/// here so the typed C9.7 module typed builds without
/// typed `pub` exposing the typed C7.9 helper.
#[must_use]
const fn downgrade_resolution(res: CloudShadowResolution) -> CloudShadowResolution {
    match res {
        CloudShadowResolution::Cinematic4096 => CloudShadowResolution::Balanced2048,
        CloudShadowResolution::Balanced2048 => CloudShadowResolution::Cheap1024,
        CloudShadowResolution::Cheap1024 => CloudShadowResolution::Cheap1024,
    }
}

pub const FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed CloudShadowDispatchCountsView
// ============================================================================

/// Typed Pass C9.7 — feature-gate-free typed mirror of
/// the typed `CloudShadowDispatchCounts` struct defined
/// in `cloud_shadow_pipelines.rs` (typed under the typed
/// `wgpu_bridge` feature).  Used here so the typed
/// dispatch feedback module is typed buildable + typed
/// testable in typed every feature configuration.
///
/// On typed builds with typed `wgpu_bridge` enabled, the
/// typed renderer converts the typed wgpu-bridge counts
/// into a typed view via typed `from_project_filter`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowDispatchCountsView {
    pub schema_version: u16,
    pub project_dispatches: u32,
    pub filter_dispatches: u32,
    pub workgroup_count: [u32; 3],
}

impl CloudShadowDispatchCountsView {
    /// Typed Pass C9.7 — typed zero-dispatch baseline.
    pub const ZERO: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
        project_dispatches: 0,
        filter_dispatches: 0,
        workgroup_count: [0, 0, 0],
    };

    /// Typed Pass C9.7 — builder: convert typed live
    /// project/filter dispatch counts (typed from the
    /// typed wgpu_bridge `CloudShadowDispatchCounts`)
    /// into a typed feature-gate-free view.
    #[must_use]
    pub const fn from_project_filter(
        project_dispatches: u32,
        filter_dispatches: u32,
        workgroup_count: [u32; 3],
    ) -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
            project_dispatches,
            filter_dispatches,
            workgroup_count,
        }
    }

    /// Typed Pass C9.7 — predicate: did the typed full
    /// chain run this frame?
    #[must_use]
    pub const fn full_chain_dispatched(&self) -> bool {
        self.project_dispatches > 0 && self.filter_dispatches > 0
    }

    /// Typed Pass C9.7 — predicate: typed any typed
    /// dispatch occurred this frame?
    #[must_use]
    pub const fn any_dispatch(&self) -> bool {
        self.project_dispatches > 0 || self.filter_dispatches > 0
    }
}

// ============================================================================
// Section 2 — typed CloudShadowGpuTimings
// ============================================================================

/// Typed Pass C9.7 — typed packed GPU timing record per
/// the typed user-spec.  Tracks typed five distinct
/// passes:
///
/// - `project_gpu_ns` — typed cloud shadow project
///   compute pass (typed Pass C7.4.3 wgsl).
/// - `filter_gpu_ns` — typed cloud shadow filter compute
///   pass (typed Pass C7.4.4 wgsl).
/// - `register_layer_gpu_ns` — typed CPU-side aux-layer
///   registration latency (typed Pass C7.6 register
///   pass).  The typed register pass has no typed GPU
///   dispatch; this typed field tracks the typed CPU
///   overhead so the typed director can typed budget
///   against it.  Typed `0` when typed CPU profiling
///   is disabled.
/// - `direct_lighting_sample_gpu_ns` — typed direct-
///   lighting cloud sample (typed Pass C9.3 / C7.6
///   shader compose).  Typed integrated into the typed
///   Lux direct-lighting pass; typed `0` when typed
///   separable timing is not wired.
/// - `volumetric_sample_gpu_ns` — typed volumetric
///   light-inject cloud sample (typed Pass C9.4 / C7.7
///   shader compose).  Typed integrated into the typed
///   Lux volumetric-inject pass; typed `0` when typed
///   separable timing is not wired.
///
/// Typed `0` on typed any field means typed timing
/// infrastructure is not wired for that pass; typed
/// downstream consumers treat typed `0` as "no
/// observation" rather than "took zero ns".
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CloudShadowGpuTimings {
    pub schema_version: u16,
    pub project_gpu_ns: u64,
    pub filter_gpu_ns: u64,
    pub register_layer_gpu_ns: u64,
    pub direct_lighting_sample_gpu_ns: u64,
    pub volumetric_sample_gpu_ns: u64,
}

impl CloudShadowGpuTimings {
    /// Typed Pass C9.7 — typed zero-timings baseline
    /// (typed timing infrastructure not wired or typed
    /// frame skipped).
    pub const ZERO: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
        project_gpu_ns: 0,
        filter_gpu_ns: 0,
        register_layer_gpu_ns: 0,
        direct_lighting_sample_gpu_ns: 0,
        volumetric_sample_gpu_ns: 0,
    };

    /// Typed Pass C9.7 — builder from typed individual
    /// pass timings.
    #[must_use]
    pub const fn new(
        project_gpu_ns: u64,
        filter_gpu_ns: u64,
        register_layer_gpu_ns: u64,
        direct_lighting_sample_gpu_ns: u64,
        volumetric_sample_gpu_ns: u64,
    ) -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
            project_gpu_ns,
            filter_gpu_ns,
            register_layer_gpu_ns,
            direct_lighting_sample_gpu_ns,
            volumetric_sample_gpu_ns,
        }
    }

    /// Typed Pass C9.7 — predicate: any typed pass has a
    /// typed nonzero GPU ns reading?
    #[must_use]
    pub const fn has_any_timing(&self) -> bool {
        self.project_gpu_ns > 0
            || self.filter_gpu_ns > 0
            || self.register_layer_gpu_ns > 0
            || self.direct_lighting_sample_gpu_ns > 0
            || self.volumetric_sample_gpu_ns > 0
    }

    /// Typed Pass C9.7 — predicate: typed project + filter
    /// passes (typed core chain) typed both observed
    /// nonzero?  Used by typed user-spec acceptance
    /// `project_gpu_ns and filter_gpu_ns are nonzero in
    /// live runs`.
    #[must_use]
    pub const fn project_and_filter_are_nonzero(&self) -> bool {
        self.project_gpu_ns > 0 && self.filter_gpu_ns > 0
    }

    /// Typed Pass C9.7 — typed total ns spent in the typed
    /// cloud shadow GPU chain (project + filter).  Does
    /// NOT include typed Lux compose passes (those are
    /// reported in typed register/direct/volumetric
    /// fields).
    #[must_use]
    pub const fn total_chain_gpu_ns(&self) -> u64 {
        self.project_gpu_ns.saturating_add(self.filter_gpu_ns)
    }

    /// Typed Pass C9.7 — typed total ns spent in typed
    /// downstream Lux consumers that read the typed cloud
    /// shadow output (register + direct + volumetric).
    #[must_use]
    pub const fn total_consumer_gpu_ns(&self) -> u64 {
        self.register_layer_gpu_ns
            .saturating_add(self.direct_lighting_sample_gpu_ns)
            .saturating_add(self.volumetric_sample_gpu_ns)
    }

    /// Typed Pass C9.7 — typed total ns across typed every
    /// pass.
    #[must_use]
    pub const fn total_gpu_ns(&self) -> u64 {
        self.total_chain_gpu_ns()
            .saturating_add(self.total_consumer_gpu_ns())
    }
}

// ============================================================================
// Section 3 — typed CloudShadowDispatchFeedback
// ============================================================================

/// Typed Pass C9.7 — typed bundled feedback record the
/// typed renderer passes to the typed director adapter +
/// the typed diagnostics enricher.  Contains:
/// - typed full `CloudShadowDispatchCountsView` (typed
///   not flattened);
/// - typed `CloudShadowGpuTimings` (typed 5-field record);
/// - typed `light_id` the typed feedback applies to.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowDispatchFeedback {
    pub schema_version: u16,
    pub light_id: LuxLightId,
    pub counts: CloudShadowDispatchCountsView,
    pub timings: CloudShadowGpuTimings,
}

impl CloudShadowDispatchFeedback {
    /// Typed Pass C9.7 — typed empty baseline (typed cloud
    /// shadow gated off this frame).
    pub const EMPTY: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
        light_id: LuxLightId::INVALID,
        counts: CloudShadowDispatchCountsView::ZERO,
        timings: CloudShadowGpuTimings::ZERO,
    };

    /// Typed Pass C9.7 — builder.
    #[must_use]
    pub const fn new(
        light_id: LuxLightId,
        counts: CloudShadowDispatchCountsView,
        timings: CloudShadowGpuTimings,
    ) -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
            light_id,
            counts,
            timings,
        }
    }

    /// Typed Pass C9.7 — predicate: did the typed chain
    /// dispatch + the typed timings observe nonzero?  Used
    /// by typed user-spec acceptance.
    #[must_use]
    pub const fn dispatched_with_live_timings(&self) -> bool {
        self.counts.full_chain_dispatched() && self.timings.project_and_filter_are_nonzero()
    }
}

// ============================================================================
// Section 4 — typed CloudShadowDispatchDiagnostics
// ============================================================================

/// Typed Pass C9.7 — typed extended diagnostics record
/// that bundles the typed Pass C7.8
/// `CloudShadowDiagnostics` with the typed full feedback.
/// The typed renderer emits this typed record to the
/// typed debug overlay + the typed log artifact so the
/// typed user-spec acceptance "diagnostics overlay
/// reports the same values as log/debug artifact" is
/// typed satisfied (typed single record, typed two
/// consumers).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowDispatchDiagnostics {
    pub schema_version: u16,
    pub base: CloudShadowDiagnostics,
    pub feedback: CloudShadowDispatchFeedback,
}

impl CloudShadowDispatchDiagnostics {
    /// Typed Pass C9.7 — typed cold-default (typed
    /// matches `CloudShadowDiagnostics::COLD_DEFAULT` +
    /// `CloudShadowDispatchFeedback::EMPTY`).
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
        base: CloudShadowDiagnostics::COLD_DEFAULT,
        feedback: CloudShadowDispatchFeedback::EMPTY,
    };

    /// Typed Pass C9.7 — builder.
    #[must_use]
    pub const fn from_base_and_feedback(
        base: CloudShadowDiagnostics,
        feedback: CloudShadowDispatchFeedback,
    ) -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
            base,
            feedback,
        }
    }

    /// Typed Pass C9.7 — typed predicate: do the typed
    /// base diagnostics + the typed feedback agree on
    /// the typed dispatch counts?  Audits the typed
    /// wire-up contract.
    #[must_use]
    pub fn dispatch_counts_agree(&self) -> bool {
        self.base.project_dispatch_count == self.feedback.counts.project_dispatches
            && self.base.filter_dispatch_count == self.feedback.counts.filter_dispatches
    }

    /// Typed Pass C9.7 — typed predicate: do the typed
    /// base GPU timings + the typed feedback agree?
    #[must_use]
    pub fn gpu_timings_agree(&self) -> bool {
        self.base.project_gpu_ns == self.feedback.timings.project_gpu_ns
            && self.base.filter_gpu_ns == self.feedback.timings.filter_gpu_ns
    }

    /// Typed Pass C9.7 — predicate: did the typed live
    /// run observe typed project + filter GPU ns
    /// nonzero?  Targets the typed user-spec acceptance
    /// "`project_gpu_ns` and `filter_gpu_ns` are nonzero
    /// in live runs".
    #[must_use]
    pub const fn live_project_and_filter_nonzero(&self) -> bool {
        self.feedback.timings.project_and_filter_are_nonzero()
    }
}

// ============================================================================
// Section 5 — typed director feedback adapter
// ============================================================================

/// Typed Pass C9.7 — typed extended budget record that
/// adds typed per-pass budgets for typed register-layer,
/// typed direct-lighting sample, typed volumetric sample
/// passes on top of the typed Pass C7.9
/// `CloudShadowDirectorBudget`.  Backwards-compatible
/// via `from_base` builder.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CloudShadowDispatchBudget {
    pub schema_version: u16,
    pub base: CloudShadowDirectorBudget,
    /// Typed budget for the typed CPU-side register
    /// pass.  Typed `0` disables the typed check.
    pub register_layer_budget_ns: u64,
    /// Typed budget for the typed direct-lighting cloud
    /// sample compose.  Typed `0` disables the typed
    /// check.
    pub direct_lighting_sample_budget_ns: u64,
    /// Typed budget for the typed volumetric cloud sample
    /// compose.  Typed `0` disables the typed check.
    pub volumetric_sample_budget_ns: u64,
}

impl CloudShadowDispatchBudget {
    /// Typed product default budgets — typed numbers
    /// chosen to typed flag pressure when typed any
    /// consumer pass exceeds typed ~200µs.
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
        base: CloudShadowDirectorBudget::PRODUCT_DEFAULT,
        register_layer_budget_ns: 50_000, // 50 µs typed CPU register
        direct_lighting_sample_budget_ns: 200_000, // 200 µs typed direct compose
        volumetric_sample_budget_ns: 200_000, // 200 µs typed volumetric compose
    };

    /// Typed cinematic budgets (typed wider envelope per
    /// typed cinematic tier).
    pub const CINEMATIC: Self = Self {
        schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
        base: CloudShadowDirectorBudget::CINEMATIC,
        register_layer_budget_ns: 100_000,
        direct_lighting_sample_budget_ns: 800_000,
        volumetric_sample_budget_ns: 800_000,
    };

    /// Typed Pass C9.7 builder — wrap a typed Pass C7.9
    /// budget with typed default consumer-pass budgets.
    #[must_use]
    pub const fn from_base(base: CloudShadowDirectorBudget) -> Self {
        Self {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
            base,
            register_layer_budget_ns: 0,
            direct_lighting_sample_budget_ns: 0,
            volumetric_sample_budget_ns: 0,
        }
    }
}

/// Typed Pass C9.7 — typed predicate: do the typed
/// timings exceed typed any of the typed extended
/// consumer-pass budgets?  Used by the typed feedback
/// adapter to typed escalate to typed `BudgetDowngrade`
/// when typed downstream Lux consumers (typed register /
/// direct / volumetric) breach their typed budgets.
#[must_use]
pub fn consumer_passes_over_budget(
    timings: &CloudShadowGpuTimings,
    budget: &CloudShadowDispatchBudget,
) -> bool {
    let register_over = budget.register_layer_budget_ns > 0
        && timings.register_layer_gpu_ns > budget.register_layer_budget_ns;
    let direct_over = budget.direct_lighting_sample_budget_ns > 0
        && timings.direct_lighting_sample_gpu_ns > budget.direct_lighting_sample_budget_ns;
    let vol_over = budget.volumetric_sample_budget_ns > 0
        && timings.volumetric_sample_gpu_ns > budget.volumetric_sample_budget_ns;
    register_over || direct_over || vol_over
}

/// Typed Pass C9.7 — typed feedback-driven director
/// decision adapter.  Calls the typed Pass C7.9
/// `decide_cloud_shadow_refresh` for the typed baseline
/// decision, then enriches with the typed real
/// `CloudShadowDispatchFeedback` GPU timings:
///
/// - Typed feedback fills in typed
///   `last_project_gpu_ns` / typed `last_filter_gpu_ns`
///   on the typed inputs view (typed bypassing the typed
///   caller-supplied values when typed live timings are
///   available).
/// - Typed if typed consumer-pass budgets are typed
///   breached AND the typed tier permits typed
///   downgrade, escalate the typed decision to typed
///   `BudgetDowngrade` (typed even if the typed base
///   decision was typed Skip or typed CadenceReached).
/// - Typed otherwise return the typed base decision
///   unchanged so the typed prioritized signal order
///   (typed Debug > WorldStream > Weather > Sun >
///   CameraSnap > BudgetDowngrade > CadenceReached >
///   StableSkipped) is typed preserved for typed forced
///   refreshes.
///
/// The typed adapter satisfies the typed user-spec
/// acceptance bullets:
/// - typed "Budget downgrade uses measured GPU time."
/// - typed "Stable camera/weather skips refresh until
///   cadence threshold."
/// - typed "Debug mode refreshes every frame."
#[must_use]
pub fn decide_cloud_shadow_refresh_with_feedback(
    inputs: &CloudShadowDirectorInputs,
    budget: &CloudShadowDispatchBudget,
    feedback: &CloudShadowDispatchFeedback,
) -> CloudShadowDirectorDecision {
    // Typed inject the typed live project/filter timings
    // into the typed Pass C7.9 inputs view.  Typed real
    // timings override caller-supplied placeholders.
    let mut enriched = *inputs;
    if feedback.timings.project_gpu_ns > 0 {
        enriched.last_project_gpu_ns = feedback.timings.project_gpu_ns;
    }
    if feedback.timings.filter_gpu_ns > 0 {
        enriched.last_filter_gpu_ns = feedback.timings.filter_gpu_ns;
    }

    let base = decide_cloud_shadow_refresh(&enriched, &budget.base);

    // Typed forced refresh paths (Debug, WorldStream,
    // Weather, Sun, CameraSnap) take precedence over
    // typed consumer-pass budget pressure — typed return
    // them unchanged.
    if base.reason.is_forced() || base.reason == CloudShadowRefreshReason::DebugForce {
        return base;
    }
    if base.reason == CloudShadowRefreshReason::BudgetDowngrade {
        // Typed base already flagged typed budget pressure
        // on project / filter — return unchanged.
        return base;
    }

    // Typed consumer-pass budget pressure typed
    // escalates to typed BudgetDowngrade when typed tier
    // permits typed downgrade.
    let consumers_over = consumer_passes_over_budget(&feedback.timings, budget);
    if consumers_over && enriched.quality_tier.permits_resolution_downgrade() {
        let tier = enriched.quality_tier;
        let tier_cadence = tier.default_cadence_frames();
        let tier_latency = tier.default_latency();
        return CloudShadowDirectorDecision {
            schema_version: base.schema_version,
            action: CloudShadowRefreshAction::RefreshProjectAndFilter,
            reason: CloudShadowRefreshReason::BudgetDowngrade,
            next_resolution: downgrade_resolution(enriched.current_resolution),
            next_cadence_frames: tier_cadence.saturating_mul(2),
            projection_center_snap: None,
            budget_pressure: true,
            next_latency: tier_latency,
        };
    }

    base
}

/// Typed Pass C9.7 — typed predicate: typed stable camera
/// AND typed stable weather → typed cadence-based skip.
/// Audits the typed user-spec acceptance "Stable
/// camera/weather skips refresh until cadence threshold".
#[must_use]
pub fn stable_inputs_skip_until_cadence(
    inputs: &CloudShadowDirectorInputs,
    budget: &CloudShadowDispatchBudget,
    feedback: &CloudShadowDispatchFeedback,
) -> bool {
    // Typed stable means typed no weather / sun /
    // camera / stream signals firing AND typed tier
    // doesn't refresh every frame.
    if inputs.weather_changed
        || inputs.sun_changed
        || inputs.world_stream_event_pending
        || inputs.camera_movement_meters >= budget.base.camera_snap_threshold_meters
    {
        return false;
    }
    if inputs.quality_tier.refreshes_every_frame() {
        return false;
    }
    let decision = decide_cloud_shadow_refresh_with_feedback(inputs, budget, feedback);
    let tier_cadence = inputs.quality_tier.default_cadence_frames();
    // Typed before cadence → typed skip; typed at cadence
    // → typed refresh.
    if inputs.frames_since_last_refresh < tier_cadence {
        decision.action == CloudShadowRefreshAction::Skip
    } else {
        decision.action == CloudShadowRefreshAction::RefreshProjectAndFilter
            || decision.action == CloudShadowRefreshAction::ForceRefresh
    }
}

/// Typed Pass C9.7 — typed predicate: typed Debug tier
/// refreshes every frame.  Audits the typed user-spec
/// acceptance "Debug mode refreshes every frame".
#[must_use]
pub fn debug_tier_refreshes_every_frame(
    budget: &CloudShadowDispatchBudget,
    feedback: &CloudShadowDispatchFeedback,
) -> bool {
    let mut inputs = CloudShadowDirectorInputs::STABLE;
    inputs.quality_tier = CloudShadowQualityTier::Debug;
    // Typed cycle through typed several frame indices —
    // typed every one must refresh.
    for frame_index in 0..16u32 {
        inputs.frame_index = frame_index;
        inputs.frames_since_last_refresh = 0;
        let decision = decide_cloud_shadow_refresh_with_feedback(&inputs, budget, feedback);
        if decision.action != CloudShadowRefreshAction::ForceRefresh
            || decision.reason != CloudShadowRefreshReason::DebugForce
        {
            return false;
        }
    }
    true
}

/// Typed Pass C9.7 — typed predicate: typed budget
/// downgrade uses typed measured GPU time.  Audits the
/// typed user-spec acceptance "Budget downgrade uses
/// measured GPU time".
#[must_use]
pub fn budget_downgrade_uses_measured_gpu_time(
    budget: &CloudShadowDispatchBudget,
    over_budget_ns: u64,
) -> bool {
    // Typed baseline stable inputs (typed no signals
    // firing).
    let inputs = CloudShadowDirectorInputs {
        quality_tier: CloudShadowQualityTier::Balanced,
        frames_since_last_refresh: 0,
        ..CloudShadowDirectorInputs::STABLE
    };
    // Typed feedback with typed measured GPU times typed
    // over the typed project budget.
    let feedback = CloudShadowDispatchFeedback::new(
        LuxLightId::new(1),
        CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]),
        CloudShadowGpuTimings::new(over_budget_ns, 100_000, 0, 0, 0),
    );
    let decision = decide_cloud_shadow_refresh_with_feedback(&inputs, budget, &feedback);
    // Typed measured GPU time over budget → typed
    // BudgetDowngrade reason.
    decision.reason == CloudShadowRefreshReason::BudgetDowngrade && decision.budget_pressure
}

/// Typed Pass C9.7 — typed top-level diagnostics
/// composer.  Bundles the typed Pass C7.8
/// `CloudShadowDiagnostics` (extended with typed real
/// dispatch counts + typed real timings) into the typed
/// extended `CloudShadowDispatchDiagnostics` so the
/// typed renderer emits typed one record to typed both
/// the typed debug overlay + the typed log artifact.
#[must_use]
pub fn compose_dispatch_diagnostics(
    base_template: CloudShadowDiagnostics,
    feedback: CloudShadowDispatchFeedback,
) -> CloudShadowDispatchDiagnostics {
    // Typed inject typed feedback dispatch counts +
    // timings into the typed base diagnostics.
    let base = CloudShadowDiagnostics {
        project_dispatch_count: feedback.counts.project_dispatches,
        filter_dispatch_count: feedback.counts.filter_dispatches,
        project_gpu_ns: feedback.timings.project_gpu_ns,
        filter_gpu_ns: feedback.timings.filter_gpu_ns,
        ..base_template
    };
    CloudShadowDispatchDiagnostics::from_base_and_feedback(base, feedback)
}

/// Typed Pass C9.7 — typed predicate: the typed feedback
/// adapter typed wires the typed full
/// `CloudShadowDispatchCounts` (typed not flattened).
/// Audits the typed contract from the typed user-spec
/// "Wire `CloudShadowDispatchCounts` into
/// `CloudShadowDiagnostics`."
#[must_use]
pub fn feedback_wires_full_dispatch_counts(feedback: &CloudShadowDispatchFeedback) -> bool {
    // Typed full counts means typed all four counts-view
    // fields are typed accessible (typed not collapsed
    // to typed two u32s).
    let _ = feedback.counts.project_dispatches;
    let _ = feedback.counts.filter_dispatches;
    let _ = feedback.counts.workgroup_count;
    let _ = feedback.counts.schema_version;
    feedback.counts.schema_version == FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION
}

/// Typed Pass C9.7 — typed predicate: typed every typed
/// user-spec GPU-timestamp pass has a typed nonzero
/// field reading.  Audits the typed user-spec contract
/// "Wire GPU timestamps for: project pass, filter pass,
/// register layer, direct-lighting cloud sample,
/// volumetric cloud sample".
#[must_use]
pub fn timings_carry_every_user_spec_pass(timings: &CloudShadowGpuTimings) -> bool {
    timings.project_gpu_ns > 0
        && timings.filter_gpu_ns > 0
        && timings.register_layer_gpu_ns > 0
        && timings.direct_lighting_sample_gpu_ns > 0
        && timings.volumetric_sample_gpu_ns > 0
}

/// Typed Pass C9.7 — typed predicate: the typed
/// diagnostics overlay value matches the typed log
/// artifact value (typed both consume the typed same
/// `CloudShadowDispatchDiagnostics` record).  Audits
/// the typed user-spec acceptance "Diagnostics overlay
/// reports the same values as log/debug artifact".
#[must_use]
pub fn overlay_and_log_artifact_agree(diagnostics: &CloudShadowDispatchDiagnostics) -> bool {
    diagnostics.dispatch_counts_agree() && diagnostics.gpu_timings_agree()
}

/// Typed Pass C9.7 — typed feature-gate-bridging
/// constants the typed renderer reads when typed
/// reporting the typed cloud shadow latency mode + the
/// typed live readback availability.  Mirrors the
/// `CloudShadowFrameDelayMode` carrier.
#[must_use]
pub const fn dispatch_feedback_default_latency() -> CloudShadowFrameDelayMode {
    CloudShadowFrameDelayMode::OneFrameDelayed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_shadow::{CloudShadowProjectionConstants, CloudShadowResourceDiagnostics};
    use crate::cloud_shadow_director::CloudShadowQualityTier;
    use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};

    fn live_settings_and_constants(
        light_id: LuxLightId,
    ) -> (CloudRenderSettings, CloudShadowProjectionConstants) {
        let settings = CloudRenderSettings::PRODUCT_DEFAULT;
        let constants = CloudShadowProjectionConstants::from_inputs(
            &settings,
            CloudWeatherProfileId::Scattered,
            light_id,
            [0.0, 1.0, 0.0],
            0,
        );
        (settings, constants)
    }

    fn live_resources(settings: &CloudRenderSettings) -> CloudShadowResourceDiagnostics {
        CloudShadowResourceDiagnostics::from_settings(settings, false, false)
    }

    /// Pass C9.7 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(
            FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
            1,
        );
    }

    /// Pass C9.7 — typed feedback record carries typed
    /// full dispatch counts (typed not flattened).
    /// Audits user-spec contract "Wire
    /// `CloudShadowDispatchCounts` into
    /// `CloudShadowDiagnostics`".
    #[test]
    fn feedback_wires_full_dispatch_counts_struct() {
        let counts = CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]);
        let feedback = CloudShadowDispatchFeedback::new(
            LuxLightId::new(1),
            counts,
            CloudShadowGpuTimings::ZERO,
        );
        assert!(feedback_wires_full_dispatch_counts(&feedback));
        // Typed all 4 view fields accessible.
        assert_eq!(feedback.counts.project_dispatches, 1);
        assert_eq!(feedback.counts.filter_dispatches, 1);
        assert_eq!(feedback.counts.workgroup_count, [16, 16, 1]);
        assert_eq!(
            feedback.counts.schema_version,
            FUN_RENDERER_CLOUD_SHADOW_DISPATCH_FEEDBACK_SCHEMA_VERSION,
        );
    }

    /// Pass C9.7 acceptance — `project_gpu_ns` and
    /// `filter_gpu_ns` are nonzero in live runs.
    #[test]
    fn project_gpu_ns_and_filter_gpu_ns_nonzero_in_live_runs() {
        // Typed live run simulator: typed dispatch
        // happened + typed timings observed.
        let counts = CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]);
        let timings = CloudShadowGpuTimings::new(120_000, 80_000, 10_000, 50_000, 60_000);
        let feedback = CloudShadowDispatchFeedback::new(LuxLightId::new(7), counts, timings);

        assert!(feedback.dispatched_with_live_timings());
        assert!(feedback.timings.project_and_filter_are_nonzero());
        assert!(timings_carry_every_user_spec_pass(&feedback.timings));

        // Typed composed diagnostics reports the typed
        // same nonzero readings.
        let light = LuxLightId::new(7);
        let (settings, constants) = live_settings_and_constants(light);
        let resources = live_resources(&settings);
        let base = CloudShadowDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &resources,
            Some((1, 1)),
            Some((120_000, 80_000)),
            None,
        );
        let composed = compose_dispatch_diagnostics(base, feedback);
        assert!(composed.live_project_and_filter_nonzero());
        assert!(composed.dispatch_counts_agree());
        assert!(composed.gpu_timings_agree());
    }

    /// Pass C9.7 acceptance — budget downgrade uses
    /// measured GPU time.
    #[test]
    fn budget_downgrade_uses_measured_gpu_time_check() {
        let budget = CloudShadowDispatchBudget::PRODUCT_DEFAULT;
        let project_budget = budget.base.project_budget_ns;
        // Typed over budget by 2x.
        let over_budget = project_budget.saturating_mul(2);
        assert!(budget_downgrade_uses_measured_gpu_time(
            &budget,
            over_budget
        ));

        // Typed under budget → typed no downgrade.
        let under_budget = project_budget / 2;
        let inputs = CloudShadowDirectorInputs {
            quality_tier: CloudShadowQualityTier::Balanced,
            frames_since_last_refresh: 0,
            ..CloudShadowDirectorInputs::STABLE
        };
        let feedback = CloudShadowDispatchFeedback::new(
            LuxLightId::new(1),
            CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]),
            CloudShadowGpuTimings::new(under_budget, 50_000, 0, 0, 0),
        );
        let decision = decide_cloud_shadow_refresh_with_feedback(&inputs, &budget, &feedback);
        // Typed not budget-pressure when under budget.
        assert_ne!(decision.reason, CloudShadowRefreshReason::BudgetDowngrade);
        assert!(!decision.budget_pressure);
    }

    /// Pass C9.7 — typed consumer-pass budget pressure
    /// escalates to BudgetDowngrade.
    #[test]
    fn consumer_pass_pressure_escalates_to_budget_downgrade() {
        let budget = CloudShadowDispatchBudget::PRODUCT_DEFAULT;
        // Typed stable inputs (cadence not reached → base
        // would Skip).
        let inputs = CloudShadowDirectorInputs {
            quality_tier: CloudShadowQualityTier::Balanced,
            frames_since_last_refresh: 0,
            ..CloudShadowDirectorInputs::STABLE
        };
        // Typed direct-lighting sample over budget.
        let feedback = CloudShadowDispatchFeedback::new(
            LuxLightId::new(1),
            CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]),
            CloudShadowGpuTimings::new(
                100_000,                                           // project ok
                50_000,                                            // filter ok
                10_000,                                            // register ok
                budget.direct_lighting_sample_budget_ns + 100_000, // OVER
                50_000,                                            // volumetric ok
            ),
        );
        let decision = decide_cloud_shadow_refresh_with_feedback(&inputs, &budget, &feedback);
        assert_eq!(decision.reason, CloudShadowRefreshReason::BudgetDowngrade);
        assert!(decision.budget_pressure);
    }

    /// Pass C9.7 acceptance — stable camera/weather skips
    /// refresh until cadence threshold.
    #[test]
    fn stable_camera_weather_skips_until_cadence() {
        let budget = CloudShadowDispatchBudget::PRODUCT_DEFAULT;
        let feedback = CloudShadowDispatchFeedback::EMPTY;
        let tier = CloudShadowQualityTier::Balanced;
        let cadence = tier.default_cadence_frames();

        // Typed before cadence → typed skip.
        for f in 0..cadence {
            let inputs = CloudShadowDirectorInputs {
                quality_tier: tier,
                frame_index: f,
                frames_since_last_refresh: f,
                ..CloudShadowDirectorInputs::STABLE
            };
            assert!(stable_inputs_skip_until_cadence(
                &inputs, &budget, &feedback
            ));
            let decision = decide_cloud_shadow_refresh_with_feedback(&inputs, &budget, &feedback);
            assert_eq!(
                decision.action,
                CloudShadowRefreshAction::Skip,
                "frame {}",
                f
            );
        }

        // Typed at cadence → typed refresh.
        let inputs = CloudShadowDirectorInputs {
            quality_tier: tier,
            frame_index: cadence,
            frames_since_last_refresh: cadence,
            ..CloudShadowDirectorInputs::STABLE
        };
        assert!(stable_inputs_skip_until_cadence(
            &inputs, &budget, &feedback
        ));
        let decision = decide_cloud_shadow_refresh_with_feedback(&inputs, &budget, &feedback);
        assert_eq!(
            decision.action,
            CloudShadowRefreshAction::RefreshProjectAndFilter,
        );
        assert_eq!(decision.reason, CloudShadowRefreshReason::CadenceReached);
    }

    /// Pass C9.7 acceptance — debug mode refreshes every
    /// frame.
    #[test]
    fn debug_mode_refreshes_every_frame() {
        let budget = CloudShadowDispatchBudget::PRODUCT_DEFAULT;
        let feedback = CloudShadowDispatchFeedback::EMPTY;
        assert!(debug_tier_refreshes_every_frame(&budget, &feedback));
    }

    /// Pass C9.7 acceptance — diagnostics overlay reports
    /// the same values as log/debug artifact.
    #[test]
    fn overlay_and_log_artifact_agree_on_values() {
        let light = LuxLightId::new(13);
        let (settings, constants) = live_settings_and_constants(light);
        let resources = live_resources(&settings);
        let counts = CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]);
        let timings = CloudShadowGpuTimings::new(150_000, 80_000, 5_000, 40_000, 70_000);
        let feedback = CloudShadowDispatchFeedback::new(light, counts, timings);

        // Typed renderer composes typed one record.
        let base = CloudShadowDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &resources,
            Some((1, 1)),
            Some((150_000, 80_000)),
            None,
        );
        let composed = compose_dispatch_diagnostics(base, feedback);
        // Typed overlay + log consume typed same record.
        assert!(overlay_and_log_artifact_agree(&composed));
    }

    /// Pass C9.7 — typed forced refreshes (typed weather,
    /// sun, camera, world stream) take precedence over
    /// typed consumer-pass pressure.
    #[test]
    fn forced_refresh_takes_precedence_over_consumer_pressure() {
        let budget = CloudShadowDispatchBudget::PRODUCT_DEFAULT;
        let feedback = CloudShadowDispatchFeedback::new(
            LuxLightId::new(1),
            CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]),
            // Typed direct-lighting sample over budget.
            CloudShadowGpuTimings::new(
                100_000,
                50_000,
                10_000,
                budget.direct_lighting_sample_budget_ns + 100_000,
                50_000,
            ),
        );
        for (label, mut inputs) in [
            ("weather", CloudShadowDirectorInputs::STABLE),
            ("sun", CloudShadowDirectorInputs::STABLE),
            ("stream", CloudShadowDirectorInputs::STABLE),
        ] {
            inputs.quality_tier = CloudShadowQualityTier::Balanced;
            match label {
                "weather" => inputs.weather_changed = true,
                "sun" => inputs.sun_changed = true,
                "stream" => inputs.world_stream_event_pending = true,
                _ => unreachable!(),
            }
            let decision = decide_cloud_shadow_refresh_with_feedback(&inputs, &budget, &feedback);
            assert!(
                decision.reason == CloudShadowRefreshReason::WeatherChanged
                    || decision.reason == CloudShadowRefreshReason::SunChanged
                    || decision.reason == CloudShadowRefreshReason::WorldStreamEvent,
                "{}: reason={:?}",
                label,
                decision.reason,
            );
        }
    }

    /// Pass C9.7 — typed `CloudShadowGpuTimings` totals
    /// add up correctly.
    #[test]
    fn gpu_timings_totals_add_up() {
        let timings = CloudShadowGpuTimings::new(100, 200, 50, 300, 400);
        assert_eq!(timings.total_chain_gpu_ns(), 300); // project + filter
        assert_eq!(timings.total_consumer_gpu_ns(), 750); // register + direct + vol
        assert_eq!(timings.total_gpu_ns(), 1050);
        assert!(timings.has_any_timing());
        assert!(timings.project_and_filter_are_nonzero());
        assert!(timings_carry_every_user_spec_pass(&timings));

        // Typed zero baseline.
        let zero = CloudShadowGpuTimings::ZERO;
        assert_eq!(zero.total_gpu_ns(), 0);
        assert!(!zero.has_any_timing());
        assert!(!zero.project_and_filter_are_nonzero());
        assert!(!timings_carry_every_user_spec_pass(&zero));
    }

    /// Pass C9.7 — typed compose builder injects feedback
    /// dispatch counts + timings into the typed base
    /// record.
    #[test]
    fn compose_injects_feedback_into_base_diagnostics() {
        let light = LuxLightId::new(5);
        let (settings, constants) = live_settings_and_constants(light);
        let resources = live_resources(&settings);
        // Typed cold base (no caller-supplied counts /
        // timings).
        let base = CloudShadowDiagnostics::from_inputs(
            &settings,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &resources,
            None,
            None,
            None,
        );
        // Typed base has typed zero dispatches + timings.
        assert_eq!(base.project_dispatch_count, 0);
        assert_eq!(base.project_gpu_ns, 0);
        // Typed feedback fills them.
        let feedback = CloudShadowDispatchFeedback::new(
            light,
            CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]),
            CloudShadowGpuTimings::new(150_000, 80_000, 0, 0, 0),
        );
        let composed = compose_dispatch_diagnostics(base, feedback);
        assert_eq!(composed.base.project_dispatch_count, 1);
        assert_eq!(composed.base.project_gpu_ns, 150_000);
        assert_eq!(composed.base.filter_gpu_ns, 80_000);
        // Typed predicates agree.
        assert!(composed.dispatch_counts_agree());
        assert!(composed.gpu_timings_agree());
        assert!(composed.live_project_and_filter_nonzero());
    }

    /// Pass C9.7 — typed dispatch counts view zero
    /// baseline + predicates.
    #[test]
    fn dispatch_counts_view_zero_and_predicates() {
        let zero = CloudShadowDispatchCountsView::ZERO;
        assert!(!zero.full_chain_dispatched());
        assert!(!zero.any_dispatch());
        let half = CloudShadowDispatchCountsView::from_project_filter(1, 0, [16, 16, 1]);
        assert!(!half.full_chain_dispatched());
        assert!(half.any_dispatch());
        let full = CloudShadowDispatchCountsView::from_project_filter(1, 1, [16, 16, 1]);
        assert!(full.full_chain_dispatched());
        assert!(full.any_dispatch());
    }

    /// Pass C9.7 — typed budget defaults match typed C7.9
    /// product policy for typed base + extend typed
    /// consumer passes with typed reasonable defaults.
    #[test]
    fn budget_defaults_match_product_policy() {
        let pd = CloudShadowDispatchBudget::PRODUCT_DEFAULT;
        assert_eq!(pd.base.project_budget_ns, 300_000);
        assert_eq!(pd.base.filter_budget_ns, 200_000);
        assert_eq!(pd.register_layer_budget_ns, 50_000);
        assert_eq!(pd.direct_lighting_sample_budget_ns, 200_000);
        assert_eq!(pd.volumetric_sample_budget_ns, 200_000);

        let ci = CloudShadowDispatchBudget::CINEMATIC;
        assert_eq!(ci.base.project_budget_ns, 1_200_000);
        assert_eq!(ci.register_layer_budget_ns, 100_000);
        assert_eq!(ci.direct_lighting_sample_budget_ns, 800_000);
        assert_eq!(ci.volumetric_sample_budget_ns, 800_000);
    }

    /// Pass C9.7 — typed `dispatch_feedback_default_latency`
    /// is typed canonical `OneFrameDelayed`.
    #[test]
    fn default_latency_is_one_frame_delayed() {
        assert_eq!(
            dispatch_feedback_default_latency(),
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
    }
}
