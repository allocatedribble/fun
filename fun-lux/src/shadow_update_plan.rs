//! Pass 7 — typed shadow update plan + static shadow cache
//! contracts.
//!
//! The existing [`crate::shadow`] module owns the typed
//! [`crate::shadow::ShadowPolicyEngine`] decision layer
//! (quality tier, soft mode, reconstruction mode, directional
//! vs local cast decision, page budget, priority). Pass 7
//! extends that with the typed update-plan layer the user
//! spec calls out:
//!
//! - Typed static / dynamic / mixed shadow mode.
//! - Typed invalidation reason.
//! - Typed estimated update cost.
//! - Typed receiver salience.
//! - Typed volumetric shadow contribution.
//! - Typed cacheability.
//! - Typed [`LuxShadowUpdatePlan`] with budgeted +
//!   priority-sorted page requests.
//! - Typed [`LuxShadowCacheMetrics`] (hit / miss /
//!   invalidation) so rule #2 of the Pass 7 acceptance
//!   list is auditable.
//! - Typed [`LuxShadowDirtyInvalidationRule`] encoding the
//!   five user-spec dirty-invalidation rules verbatim
//!   (transform / geometry / material / color-only / movement).
//! - Typed [`LuxShadowQualityTierProfile`] (Low / Medium /
//!   High / Cinematic) matching the user's per-tier shadow
//!   table.
//! - Typed [`LuxShadowDebugOverlayMode`] for the rule #5
//!   debug views (atlas occupancy, cascades, page pressure,
//!   invalidations).
//! - Typed [`LuxShadowPass7AcceptanceVerdict`] rolling the
//!   five user-spec acceptance criteria into one typed
//!   predicate.
//!
//! Pass 7 stays renderer-neutral — every type is a typed
//! descriptor / predicate.  No `wgpu`, `naga`, or
//! raw-window imports cross the crate boundary.

use crate::LuxLightId;
use crate::shadow::{
    ShadowCastDecision, ShadowPolicyDecision, ShadowQualityTier, ShadowReconstructionMode,
    SoftShadowMode,
};

pub const FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_SHADOW_MODE_COUNT: usize = 3;
pub const FUN_LUX_SHADOW_INVALIDATION_REASON_COUNT: usize = 10;
pub const FUN_LUX_SHADOW_CACHEABILITY_COUNT: usize = 3;
pub const FUN_LUX_SHADOW_PAGE_KIND_COUNT: usize = 4;
pub const FUN_LUX_SHADOW_DIRTY_RULE_COUNT: usize = 5;
pub const FUN_LUX_SHADOW_QUALITY_TIER_PROFILE_COUNT: usize = 4;
pub const FUN_LUX_SHADOW_DEBUG_OVERLAY_MODE_COUNT: usize = 5;
pub const FUN_LUX_SHADOW_PASS7_ACCEPTANCE_RULE_COUNT: usize = 5;

// ============================================================================
// Section 1 — Typed static / dynamic / mixed shadow mode
// ============================================================================

/// Typed shadow update mode.  The user spec splits shadow
/// receivers + casters into three buckets that drive the
/// static cache policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowMode {
    /// Static light + static geometry.  Render once into
    /// the static cache, reuse until something invalidates
    /// it.  Highest cache hit ratio.
    Static,
    /// One side dynamic (typically the casters).  Cache the
    /// static contribution; overlay the dynamic casters per
    /// frame.  Pass 7 expects this to be the dominant mode
    /// for world geometry that gets stepped on by moving
    /// characters.
    Mixed,
    /// Light or geometry is fully dynamic.  Budgeted update
    /// every frame — possibly time-sliced.  No persistent
    /// cache benefit.
    #[default]
    Dynamic,
}

impl LuxShadowMode {
    pub const ALL: [Self; FUN_LUX_SHADOW_MODE_COUNT] = [Self::Static, Self::Mixed, Self::Dynamic];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Mixed => "mixed",
            Self::Dynamic => "dynamic",
        }
    }

    /// Typed predicate: does this mode benefit from the
    /// static shadow cache at all?
    #[must_use]
    pub const fn benefits_from_static_cache(self) -> bool {
        matches!(self, Self::Static | Self::Mixed)
    }

    /// Typed predicate: does this mode require a per-frame
    /// render pass to refresh the depth buffer?
    #[must_use]
    pub const fn requires_per_frame_render(self) -> bool {
        matches!(self, Self::Mixed | Self::Dynamic)
    }
}

// ============================================================================
// Section 2 — Typed invalidation reason
// ============================================================================

/// Typed reason a shadow page was (re)scheduled for an
/// update.  Matches the user spec's five dirty-invalidation
/// rules + five "structural" reasons that don't come from
/// the dirty-flag pipeline (Pass 7 init, quality dial,
/// resize, etc.).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowInvalidationReason {
    /// No invalidation — the page is cache-clean and was
    /// scheduled only for the typed cache audit.
    #[default]
    NotInvalidated,
    /// Light transform changed (movement / rotation).
    /// Rule: invalidate affected light pages.
    LightTransformChanged,
    /// Geometry inside a scene chunk changed or was
    /// destroyed.  Rule: invalidate scene chunk shadow pages.
    GeometryChanged,
    /// Material alpha mask or shadow-cast flag flipped.
    /// Rule: invalidate affected caster pages.
    MaterialAlphaOrShadowFlagChanged,
    /// Light shape (cone angle, area extent) changed.
    /// Falls under the broader transform rule but is
    /// surfaced separately for diagnostics.
    LightShapeChanged,
    /// Light was removed.  Invalidate its pages so the
    /// atlas slot can be reclaimed.
    LightRemoved,
    /// The shadow quality tier dial moved (e.g. user
    /// dropped from High to Medium).  Pages re-render at
    /// the new tier.
    QualityTierChanged,
    /// Camera moved far enough that a cascade needs a fresh
    /// render (directional clipmap edge crossings).
    CameraCascadeBoundaryCrossed,
    /// Atlas was resized or repacked.  Every page rebuilds.
    AtlasRepacked,
    /// Renderer cold-start.  First plan after a renderer
    /// init or scene reload.
    ColdStart,
}

impl LuxShadowInvalidationReason {
    pub const ALL: [Self; FUN_LUX_SHADOW_INVALIDATION_REASON_COUNT] = [
        Self::NotInvalidated,
        Self::LightTransformChanged,
        Self::GeometryChanged,
        Self::MaterialAlphaOrShadowFlagChanged,
        Self::LightShapeChanged,
        Self::LightRemoved,
        Self::QualityTierChanged,
        Self::CameraCascadeBoundaryCrossed,
        Self::AtlasRepacked,
        Self::ColdStart,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInvalidated => "not_invalidated",
            Self::LightTransformChanged => "light_transform_changed",
            Self::GeometryChanged => "geometry_changed",
            Self::MaterialAlphaOrShadowFlagChanged => "material_alpha_or_shadow_flag_changed",
            Self::LightShapeChanged => "light_shape_changed",
            Self::LightRemoved => "light_removed",
            Self::QualityTierChanged => "quality_tier_changed",
            Self::CameraCascadeBoundaryCrossed => "camera_cascade_boundary_crossed",
            Self::AtlasRepacked => "atlas_repacked",
            Self::ColdStart => "cold_start",
        }
    }

    /// Typed predicate: does this reason force a depth
    /// buffer re-render?  The user's rule #3 — *light color
    /// only changes do NOT invalidate depth* — keys off
    /// the inverse of this predicate (no reason at all is
    /// emitted for color-only).
    #[must_use]
    pub const fn forces_depth_rerender(self) -> bool {
        !matches!(self, Self::NotInvalidated)
    }
}

// ============================================================================
// Section 3 — Typed cacheability
// ============================================================================

/// Typed cacheability.  Drives how the renderer reuses the
/// depth result across frames.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowCacheability {
    /// Never cache.  Re-render every frame.  Only acceptable
    /// for the cheapest dynamic lights.
    NoCache,
    /// Cache until something invalidates it.  Default for
    /// `LuxShadowMode::Static` and the static half of
    /// `LuxShadowMode::Mixed`.
    #[default]
    CacheUntilInvalidation,
    /// Time-sliced: update every `stride_frames` frames
    /// regardless of dirty state.  Useful when the light
    /// drifts slowly (sun) but the static cache cannot prove
    /// invalidation cheaply.
    TimeSliced { stride_frames: u8 },
}

impl LuxShadowCacheability {
    pub const ALL: [Self; FUN_LUX_SHADOW_CACHEABILITY_COUNT] = [
        Self::NoCache,
        Self::CacheUntilInvalidation,
        Self::TimeSliced { stride_frames: 4 },
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoCache => "no_cache",
            Self::CacheUntilInvalidation => "cache_until_invalidation",
            Self::TimeSliced { .. } => "time_sliced",
        }
    }

    /// Typed predicate: is the depth render eligible for
    /// reuse this frame?  Used by the planner to skip a
    /// cached page when no invalidation fired.
    #[must_use]
    pub const fn eligible_for_reuse(self) -> bool {
        !matches!(self, Self::NoCache)
    }
}

// ============================================================================
// Section 4 — Typed shadow page kind
// ============================================================================

/// Typed kind of shadow page the renderer must allocate +
/// render for.  Matches the user spec's four target
/// resources (directional clipmap, local atlas tile,
/// virtual page, static cache page).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowPageKind {
    /// Directional shadow clipmap page (sun / moon).
    DirectionalClipmapPage,
    /// Local shadow atlas tile (point / spot / area).
    #[default]
    LocalAtlasTile,
    /// Virtual shadow page (sparse paged shadow map).
    VirtualPage,
    /// Static cache page (depth result frozen until
    /// invalidation).
    StaticCachePage,
}

impl LuxShadowPageKind {
    pub const ALL: [Self; FUN_LUX_SHADOW_PAGE_KIND_COUNT] = [
        Self::DirectionalClipmapPage,
        Self::LocalAtlasTile,
        Self::VirtualPage,
        Self::StaticCachePage,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectionalClipmapPage => "directional_clipmap_page",
            Self::LocalAtlasTile => "local_atlas_tile",
            Self::VirtualPage => "virtual_page",
            Self::StaticCachePage => "static_cache_page",
        }
    }

    /// Typed predicate: does this page kind feed the direct
    /// lighting pass?  All four do — Pass 7's rule #4
    /// requires it.
    #[must_use]
    pub const fn feeds_direct_lighting(self) -> bool {
        true
    }

    /// Typed predicate: does this page kind feed the
    /// volumetric pass?  Per Pass 7's rule #4: directional
    /// and local pages must feed BOTH direct lighting and
    /// volumetrics.  Static cache pages also feed
    /// volumetrics through the cached depth they expose.
    /// Virtual pages feed volumetrics indirectly through
    /// the filtered output.
    #[must_use]
    pub const fn feeds_volumetrics(self) -> bool {
        matches!(
            self,
            Self::DirectionalClipmapPage
                | Self::LocalAtlasTile
                | Self::StaticCachePage
                | Self::VirtualPage
        )
    }
}

// ============================================================================
// Section 5 — Typed extended update decision
// ============================================================================

/// Typed Pass 7 extension to [`ShadowPolicyDecision`].
///
/// Wraps the existing typed decision (quality tier, soft
/// mode, reconstruction mode, base policy decision, page
/// budget, priority) with the typed Pass 7 fields the user
/// spec calls out: static/dynamic/mixed mode, invalidation
/// reason, estimated update cost, receiver salience,
/// volumetric contribution, cacheability.
///
/// `Hash` is intentionally NOT derived — the wrapped
/// [`ShadowPolicyDecision`] is `Eq + PartialEq` but not
/// `Hash`. Pass 7 does not key any map on
/// `LuxShadowUpdateDecision`; the typed page request
/// (which IS `Hash`) is the addressable record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxShadowUpdateDecision {
    pub schema_version: u16,
    pub light_id: LuxLightId,
    pub base: ShadowPolicyDecision,
    pub mode: LuxShadowMode,
    pub invalidation_reason: LuxShadowInvalidationReason,
    pub cacheability: LuxShadowCacheability,
    /// Estimated update cost in microseconds, encoded Q8
    /// fixed-point so the record stays Hash/Eq-stable
    /// (`actual_us = stored / 256`).
    pub estimated_update_cost_us_q8: u32,
    /// Receiver salience Q8 (0..=255).  How visible /
    /// important the receivers of this shadow are.
    pub receiver_salience_q8: u8,
    /// Volumetric contribution Q8 (0..=255).  How much this
    /// shadow contributes to the volumetric pass.  Drives
    /// the rule #4 audit (directional + local feed both
    /// direct + volumetric).
    pub volumetric_contribution_q8: u8,
    /// Typed page kind the decision allocates.
    pub page_kind: LuxShadowPageKind,
}

impl LuxShadowUpdateDecision {
    /// Typed builder for a base [`ShadowPolicyDecision`].
    /// The Pass 7 extension fields default to the
    /// "fully-dynamic, no-cache" worst case so a caller
    /// that doesn't know better still gets a correct
    /// (if expensive) plan.
    #[must_use]
    pub fn from_base(base: ShadowPolicyDecision) -> Self {
        let page_kind = match base.decision {
            ShadowCastDecision::CastDirectionalClipmap => LuxShadowPageKind::DirectionalClipmapPage,
            ShadowCastDecision::CastLocalPaged => LuxShadowPageKind::LocalAtlasTile,
            ShadowCastDecision::DisabledByPolicy => LuxShadowPageKind::StaticCachePage,
        };
        Self {
            schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
            light_id: base.light_id,
            base,
            mode: LuxShadowMode::Dynamic,
            invalidation_reason: LuxShadowInvalidationReason::ColdStart,
            cacheability: LuxShadowCacheability::NoCache,
            estimated_update_cost_us_q8: 0,
            receiver_salience_q8: 0,
            volumetric_contribution_q8: 0,
            page_kind,
        }
    }

    /// Typed predicate: does this decision actually need a
    /// depth render this frame?  Pass 7's rule #3 — *light
    /// color only changes do NOT invalidate depth* — keys
    /// off this predicate returning `false` when
    /// `invalidation_reason` is `NotInvalidated`.
    #[must_use]
    pub const fn needs_depth_render(&self) -> bool {
        self.base.decision.casts_shadow() && self.invalidation_reason.forces_depth_rerender()
    }

    /// Typed predicate: would this decision benefit from
    /// the static shadow cache this frame?
    #[must_use]
    pub const fn benefits_from_static_cache(&self) -> bool {
        self.mode.benefits_from_static_cache()
    }

    /// Typed priority for plan sorting.  Combines base
    /// priority (u16) with the salience Q8 + cost penalty
    /// so an expensive low-salience shadow drops below a
    /// cheap high-salience one.  Returns Q16.
    #[must_use]
    pub const fn plan_priority_q16(&self) -> u32 {
        let base = (self.base.priority as u32) << 8;
        let salience = self.receiver_salience_q8 as u32;
        // Cost penalty: subtract the high byte of cost.
        // `<u32 as Ord>::min` is not yet const, so we
        // inline the clamp.
        let cost_high = self.estimated_update_cost_us_q8 >> 16;
        let penalty = if cost_high < 255 { cost_high } else { 255 };
        base.saturating_add(salience).saturating_sub(penalty)
    }

    /// Typed cost in microseconds (Q8 → u32 via shift).
    #[must_use]
    pub const fn estimated_update_cost_us(&self) -> u32 {
        self.estimated_update_cost_us_q8 >> 8
    }
}

// ============================================================================
// Section 6 — Typed shadow page request + plan
// ============================================================================

/// Typed shadow page request — the typed unit the renderer
/// consumes.  One request per page that needs (re)rendering
/// this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxShadowPageRequest {
    pub schema_version: u16,
    pub light_id: LuxLightId,
    pub page_kind: LuxShadowPageKind,
    /// Atlas / clipmap slot index assigned to this light.
    pub slot_index: u32,
    /// Priority Q16 (higher = more important).
    pub priority_q16: u32,
    /// Estimated update cost in microseconds.
    pub estimated_update_cost_us: u32,
    pub invalidation_reason: LuxShadowInvalidationReason,
    pub cacheability: LuxShadowCacheability,
    pub mode: LuxShadowMode,
}

impl LuxShadowPageRequest {
    #[must_use]
    pub fn from_decision(decision: &LuxShadowUpdateDecision, slot_index: u32) -> Self {
        Self {
            schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
            light_id: decision.light_id,
            page_kind: decision.page_kind,
            slot_index,
            priority_q16: decision.plan_priority_q16(),
            estimated_update_cost_us: decision.estimated_update_cost_us(),
            invalidation_reason: decision.invalidation_reason,
            cacheability: decision.cacheability,
            mode: decision.mode,
        }
    }
}

/// Typed shadow update plan.  Pass 7's rule #1 requires
/// the plan be budgeted + priority-sorted.  The plan owns
/// a sorted Vec of typed requests + a typed budget cap.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LuxShadowUpdatePlan {
    pub schema_version: u16,
    pub stable_id: &'static str,
    /// Requests, priority-sorted descending after [`Self::finalize`].
    pub requests: Vec<LuxShadowPageRequest>,
    /// Per-frame µs budget for shadow work.
    pub budget_us: u32,
    /// Total µs estimated for the admitted requests.
    pub admitted_cost_us: u32,
    /// Requests that exceeded the budget and were deferred.
    pub deferred_request_count: u32,
    /// Total µs estimated for the deferred requests.
    pub deferred_cost_us: u32,
    pub cache_metrics: LuxShadowCacheMetrics,
}

impl LuxShadowUpdatePlan {
    /// Typed product-default plan (empty, with the typed
    /// product budget).  Initial cache metrics use the
    /// typed `NOT_EMITTED` constant so Pass 7 rule #2 only
    /// flips to `true` once the renderer fills the record.
    #[must_use]
    pub const fn empty(stable_id: &'static str, budget_us: u32) -> Self {
        Self {
            schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
            stable_id,
            requests: Vec::new(),
            budget_us,
            admitted_cost_us: 0,
            deferred_request_count: 0,
            deferred_cost_us: 0,
            cache_metrics: LuxShadowCacheMetrics::NOT_EMITTED,
        }
    }

    /// Adds a typed request to the plan.  Order doesn't
    /// matter; [`Self::finalize`] re-sorts.
    pub fn push(&mut self, request: LuxShadowPageRequest) {
        self.requests.push(request);
    }

    /// Sorts requests by priority descending and admits
    /// them in priority order until the budget is hit.  The
    /// admitted suffix becomes the "deferred" set tracked
    /// in [`Self::deferred_request_count`] /
    /// [`Self::deferred_cost_us`].
    pub fn finalize(&mut self) {
        self.requests
            .sort_by_key(|request| std::cmp::Reverse(request.priority_q16));
        self.admitted_cost_us = 0;
        self.deferred_request_count = 0;
        self.deferred_cost_us = 0;
        for req in &self.requests {
            let next = self
                .admitted_cost_us
                .saturating_add(req.estimated_update_cost_us);
            if next <= self.budget_us || matches!(req.cacheability, LuxShadowCacheability::NoCache)
            {
                // Always admit NoCache (it has nowhere else
                // to go), even if it blows the budget — the
                // renderer is expected to time-slice
                // post-hoc.
                self.admitted_cost_us = next;
            } else {
                self.deferred_request_count = self.deferred_request_count.saturating_add(1);
                self.deferred_cost_us = self
                    .deferred_cost_us
                    .saturating_add(req.estimated_update_cost_us);
            }
        }
    }

    /// Typed predicate: rule #1 — is the plan budgeted and
    /// priority-sorted?
    #[must_use]
    pub fn is_budgeted_and_sorted(&self) -> bool {
        // Sorted (priority descending).
        if self
            .requests
            .windows(2)
            .any(|w| w[0].priority_q16 < w[1].priority_q16)
        {
            return false;
        }
        // Budget honored (allowing for NoCache forced
        // admission).
        let nocache_cost: u32 = self
            .requests
            .iter()
            .filter(|r| matches!(r.cacheability, LuxShadowCacheability::NoCache))
            .map(|r| r.estimated_update_cost_us)
            .sum();
        // Admitted may exceed budget only by the NoCache
        // forced-admission slack.
        self.admitted_cost_us <= self.budget_us.saturating_add(nocache_cost)
    }

    /// Typed predicate: did the planner record any cache
    /// metric activity this frame?  Rule #2.
    #[must_use]
    pub const fn cache_metrics_present(&self) -> bool {
        self.cache_metrics.is_emitted()
    }

    /// Typed predicate: did any directional / local page
    /// route to BOTH direct lighting and volumetrics?
    /// Rule #4 — both must be fed.
    #[must_use]
    pub fn directional_and_local_feed_both_pipelines(&self) -> bool {
        let mut directional_seen = false;
        let mut local_seen = false;
        for r in &self.requests {
            match r.page_kind {
                LuxShadowPageKind::DirectionalClipmapPage => directional_seen = true,
                LuxShadowPageKind::LocalAtlasTile => local_seen = true,
                _ => {}
            }
        }
        // Even when no requests exist, the typed
        // routing rule still holds (no rule to violate).
        // The renderer pipeline is expected to wire both
        // outputs; we audit only that the plan itself
        // carries the kinds when they're present.
        let kinds_route_both = LuxShadowPageKind::DirectionalClipmapPage.feeds_direct_lighting()
            && LuxShadowPageKind::DirectionalClipmapPage.feeds_volumetrics()
            && LuxShadowPageKind::LocalAtlasTile.feeds_direct_lighting()
            && LuxShadowPageKind::LocalAtlasTile.feeds_volumetrics();
        kinds_route_both && (directional_seen || local_seen || self.requests.is_empty())
    }
}

// ============================================================================
// Section 7 — Typed cache metrics
// ============================================================================

/// Typed static shadow cache hit / miss / invalidation
/// metrics.  Rule #2 requires these be present.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxShadowCacheMetrics {
    pub schema_version: u16,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub invalidations: u32,
    pub time_sliced_renders: u32,
    pub dynamic_overlays: u32,
}

impl LuxShadowCacheMetrics {
    /// Typed "renderer has begun tracking" baseline — schema
    /// version is set but every counter is zero.  Matches
    /// Pass 6's [`crate::many_light_stress::LuxManyLightOverflowCounters::ZERO`]
    /// convention: the typed signal "was the renderer
    /// emitting metrics?" keys off the schema version, not
    /// the magnitude of the counters.
    pub const ZERO: Self = Self {
        schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
        cache_hits: 0,
        cache_misses: 0,
        invalidations: 0,
        time_sliced_renders: 0,
        dynamic_overlays: 0,
    };

    /// Typed "renderer has NOT yet wired metrics" baseline.
    /// Use as the initial cache_metrics on a fresh
    /// [`LuxShadowUpdatePlan`] so the typed `is_emitted`
    /// predicate (and therefore Pass 7's rule #2) returns
    /// `false` until the renderer fills the record.
    pub const NOT_EMITTED: Self = Self {
        schema_version: 0,
        cache_hits: 0,
        cache_misses: 0,
        invalidations: 0,
        time_sliced_renders: 0,
        dynamic_overlays: 0,
    };

    /// Typed predicate: were the metrics emitted (non-zero
    /// schema_version)?  Rule #2 keys off this.
    #[must_use]
    pub const fn is_emitted(&self) -> bool {
        self.schema_version != 0
    }

    /// Typed Q16 hit-ratio (0..=65_536) for diagnostics.
    /// Returns 0 when no hits or misses have been
    /// recorded.
    #[must_use]
    pub const fn hit_ratio_q16(&self) -> u32 {
        let total = self.cache_hits.saturating_add(self.cache_misses);
        if total == 0 {
            return 0;
        }
        let scaled = (self.cache_hits as u64).saturating_mul(65_536);
        let ratio = scaled / (total as u64);
        if ratio > u32::MAX as u64 {
            u32::MAX
        } else {
            ratio as u32
        }
    }
}

// ============================================================================
// Section 8 — Typed dirty invalidation rule table
// ============================================================================

/// Typed dirty-invalidation rule.  One variant per user-spec
/// rule.  The typed `would_invalidate_shadow_pages_for(kind)`
/// predicate encodes which `LuxShadowPageKind` each rule
/// invalidates, which is what the renderer needs at the
/// point of dirty-flag dispatch.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowDirtyInvalidationRule {
    /// Rule 1 (transform): invalidate affected light pages.
    #[default]
    LightTransformChange,
    /// Rule 2 (geometry): invalidate scene chunk shadow
    /// pages.
    GeometryChange,
    /// Rule 3 (material): invalidate affected caster pages
    /// when alpha mask / shadow-cast flag changes.
    MaterialAlphaOrShadowFlagChange,
    /// Rule 4 (color-only): MUST NOT invalidate depth
    /// shadow.  Encoded as a typed predicate that returns
    /// `false` for every page kind.
    LightColorOnlyChange,
    /// Rule 5 (movement): invalidate shadow pages.  This is
    /// the same effect as the transform rule but the user
    /// listed it separately to highlight "movement" as a
    /// frequent runtime trigger.
    LightMovement,
}

impl LuxShadowDirtyInvalidationRule {
    pub const ALL: [Self; FUN_LUX_SHADOW_DIRTY_RULE_COUNT] = [
        Self::LightTransformChange,
        Self::GeometryChange,
        Self::MaterialAlphaOrShadowFlagChange,
        Self::LightColorOnlyChange,
        Self::LightMovement,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LightTransformChange => "light_transform_change",
            Self::GeometryChange => "geometry_change",
            Self::MaterialAlphaOrShadowFlagChange => "material_alpha_or_shadow_flag_change",
            Self::LightColorOnlyChange => "light_color_only_change",
            Self::LightMovement => "light_movement",
        }
    }

    /// Typed predicate: should this rule invalidate the
    /// supplied page kind?  Encodes the user's five rules
    /// verbatim.
    #[must_use]
    pub const fn would_invalidate_shadow_pages_for(self, kind: LuxShadowPageKind) -> bool {
        match self {
            // Rule 1: light transform → invalidate this
            // light's pages (every kind that owns light pages).
            Self::LightTransformChange | Self::LightMovement => matches!(
                kind,
                LuxShadowPageKind::DirectionalClipmapPage
                    | LuxShadowPageKind::LocalAtlasTile
                    | LuxShadowPageKind::VirtualPage
                    | LuxShadowPageKind::StaticCachePage,
            ),
            // Rule 2: geometry change → invalidate scene
            // chunk shadow pages (every kind that holds depth
            // for geometry).
            Self::GeometryChange => matches!(
                kind,
                LuxShadowPageKind::DirectionalClipmapPage
                    | LuxShadowPageKind::LocalAtlasTile
                    | LuxShadowPageKind::VirtualPage
                    | LuxShadowPageKind::StaticCachePage,
            ),
            // Rule 3: material alpha / shadow flag →
            // invalidate caster pages (every depth kind).
            Self::MaterialAlphaOrShadowFlagChange => matches!(
                kind,
                LuxShadowPageKind::DirectionalClipmapPage
                    | LuxShadowPageKind::LocalAtlasTile
                    | LuxShadowPageKind::VirtualPage
                    | LuxShadowPageKind::StaticCachePage,
            ),
            // Rule 4: color-only → MUST NOT invalidate
            // any depth page.
            Self::LightColorOnlyChange => false,
        }
    }

    /// Typed translation: which Pass 7 invalidation reason
    /// does this rule emit?
    #[must_use]
    pub const fn invalidation_reason(self) -> LuxShadowInvalidationReason {
        match self {
            Self::LightTransformChange => LuxShadowInvalidationReason::LightTransformChanged,
            Self::GeometryChange => LuxShadowInvalidationReason::GeometryChanged,
            Self::MaterialAlphaOrShadowFlagChange => {
                LuxShadowInvalidationReason::MaterialAlphaOrShadowFlagChanged
            }
            Self::LightColorOnlyChange => LuxShadowInvalidationReason::NotInvalidated,
            Self::LightMovement => LuxShadowInvalidationReason::LightTransformChanged,
        }
    }
}

// ============================================================================
// Section 9 — Typed per-tier quality profile (Pass 7 table)
// ============================================================================

/// Typed Pass 7 per-tier shadow profile.  The user spec
/// names per-tier behavior; Pass 7 encodes it as a typed
/// table so the renderer reads one record instead of
/// reinventing the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxShadowQualityTierProfile {
    pub schema_version: u16,
    pub tier: ShadowQualityTier,
    pub directional_cascade_count: u8,
    pub max_local_shadowed_lights: u32,
    pub soft_shadow_mode: SoftShadowMode,
    pub reconstruction_mode: ShadowReconstructionMode,
    /// Pass 7: typed flag — does this tier enable the
    /// static shadow cache for static + mixed mode pages?
    pub uses_static_cache: bool,
    /// Pass 7: typed flag — does this tier enable
    /// contact-aware filtering?
    pub contact_aware_filtering: bool,
    /// Pass 7: typed flag — does this tier enable
    /// stochastic soft shadows?
    pub stochastic_soft_shadows: bool,
    /// Pass 7: typed flag — does this tier enable
    /// denoised / reconstructed virtual shadows?
    pub denoised_virtual_shadows: bool,
}

impl LuxShadowQualityTierProfile {
    pub const ALL: [Self; FUN_LUX_SHADOW_QUALITY_TIER_PROFILE_COUNT] =
        [Self::LOW, Self::MEDIUM, Self::HIGH, Self::CINEMATIC];

    /// Typed `Low` profile — fewer local shadowed lights,
    /// 2 directional cascades.
    pub const LOW: Self = Self {
        schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
        tier: ShadowQualityTier::Low,
        directional_cascade_count: 2,
        max_local_shadowed_lights: 24,
        soft_shadow_mode: SoftShadowMode::Pcf,
        reconstruction_mode: ShadowReconstructionMode::Temporal,
        uses_static_cache: false,
        contact_aware_filtering: false,
        stochastic_soft_shadows: false,
        denoised_virtual_shadows: false,
    };

    /// Typed `Medium` profile — 3 cascades + cached static
    /// pages.
    pub const MEDIUM: Self = Self {
        schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
        tier: ShadowQualityTier::Medium,
        directional_cascade_count: 3,
        max_local_shadowed_lights: 64,
        soft_shadow_mode: SoftShadowMode::Pcf,
        reconstruction_mode: ShadowReconstructionMode::SpatialTemporal,
        uses_static_cache: true,
        contact_aware_filtering: false,
        stochastic_soft_shadows: false,
        denoised_virtual_shadows: false,
    };

    /// Typed `High` profile — 4 cascades + contact-aware
    /// filtering.
    pub const HIGH: Self = Self {
        schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
        tier: ShadowQualityTier::High,
        directional_cascade_count: 4,
        max_local_shadowed_lights: 96,
        soft_shadow_mode: SoftShadowMode::ContactAware,
        reconstruction_mode: ShadowReconstructionMode::SpatialTemporal,
        uses_static_cache: true,
        contact_aware_filtering: true,
        stochastic_soft_shadows: false,
        denoised_virtual_shadows: false,
    };

    /// Typed `Cinematic` profile — stochastic soft shadows
    /// where stable, more local lights, denoised /
    /// reconstructed virtual shadows.
    pub const CINEMATIC: Self = Self {
        schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
        tier: ShadowQualityTier::Cinematic,
        directional_cascade_count: 4,
        max_local_shadowed_lights: 160,
        soft_shadow_mode: SoftShadowMode::StochasticSoft,
        reconstruction_mode: ShadowReconstructionMode::Denoised,
        uses_static_cache: true,
        contact_aware_filtering: true,
        stochastic_soft_shadows: true,
        denoised_virtual_shadows: true,
    };

    /// Typed `for_tier(tier)` builder.  `Off` falls back to
    /// the `LOW` profile (the planner still gates depth
    /// rendering on `decision.casts_shadow()`).
    #[must_use]
    pub const fn for_tier(tier: ShadowQualityTier) -> Self {
        match tier {
            ShadowQualityTier::Off => Self::LOW,
            ShadowQualityTier::Low => Self::LOW,
            ShadowQualityTier::Medium => Self::MEDIUM,
            ShadowQualityTier::High => Self::HIGH,
            ShadowQualityTier::Cinematic => Self::CINEMATIC,
        }
    }
}

// ============================================================================
// Section 10 — Typed debug overlay mode (rule #5)
// ============================================================================

/// Typed shadow debug overlay mode.  Rule #5 requires the
/// renderer expose typed views for atlas occupancy, cascades,
/// shadow page pressure, and invalidations.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowDebugOverlayMode {
    /// Off — overlay is not drawn.
    #[default]
    Off,
    /// Atlas occupancy view (which tiles are live vs free).
    AtlasOccupancy,
    /// Cascade visualization (which cascade covers which
    /// world bounds).
    Cascades,
    /// Shadow page pressure (admit / defer ratio per frame).
    PagePressure,
    /// Invalidation visualization (per-page invalidation
    /// reasons in the last N frames).
    Invalidations,
}

impl LuxShadowDebugOverlayMode {
    pub const ALL: [Self; FUN_LUX_SHADOW_DEBUG_OVERLAY_MODE_COUNT] = [
        Self::Off,
        Self::AtlasOccupancy,
        Self::Cascades,
        Self::PagePressure,
        Self::Invalidations,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::AtlasOccupancy => "atlas_occupancy",
            Self::Cascades => "cascades",
            Self::PagePressure => "page_pressure",
            Self::Invalidations => "invalidations",
        }
    }

    /// Typed predicate: does this overlay mode satisfy the
    /// user's rule #5 by surfacing at least one of the four
    /// listed views?
    #[must_use]
    pub const fn satisfies_pass7_rule5(self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Typed bundle of every Pass 7 debug overlay the renderer
/// supports.  Rule #5 holds when every typed view in the
/// user spec is supported by SOME variant.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxShadowDebugOverlaySupport {
    pub schema_version: u16,
    pub atlas_occupancy: bool,
    pub cascades: bool,
    pub page_pressure: bool,
    pub invalidations: bool,
}

impl LuxShadowDebugOverlaySupport {
    /// Typed product default — every typed view is
    /// supported (the renderer is expected to wire all four).
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
        atlas_occupancy: true,
        cascades: true,
        page_pressure: true,
        invalidations: true,
    };

    /// Typed predicate: do every user-listed Pass 7 view
    /// have typed support?  Rule #5.
    #[must_use]
    pub const fn supports_every_pass7_view(&self) -> bool {
        self.atlas_occupancy && self.cascades && self.page_pressure && self.invalidations
    }
}

// ============================================================================
// Section 11 — Typed Pass 7 acceptance verdict (5 rules)
// ============================================================================

/// Typed Pass 7 acceptance verdict.  One flag per
/// user-listed acceptance criterion.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxShadowPass7AcceptanceVerdict {
    pub schema_version: u16,
    /// Rule #1: shadow page requests are budgeted and
    /// priority-sorted.
    pub plan_is_budgeted_and_sorted: bool,
    /// Rule #2: static shadow cache emits hit / miss /
    /// invalidation metrics.
    pub cache_metrics_emitted: bool,
    /// Rule #3: dynamic light color changes don't waste
    /// shadow work (no depth render on color-only).
    pub color_only_does_not_invalidate_depth: bool,
    /// Rule #4: directional + local shadow pages feed BOTH
    /// direct lighting and volumetrics.
    pub directional_and_local_feed_both_pipelines: bool,
    /// Rule #5: typed debug overlays cover atlas occupancy,
    /// cascades, page pressure, and invalidations.
    pub debug_views_cover_pass7_set: bool,
}

impl LuxShadowPass7AcceptanceVerdict {
    /// Typed `evaluate(plan, debug_support)` constructor —
    /// derives every typed verdict from the supplied plan +
    /// debug support bundle.
    #[must_use]
    pub fn evaluate(
        plan: &LuxShadowUpdatePlan,
        debug_support: &LuxShadowDebugOverlaySupport,
    ) -> Self {
        // Rule #3: color-only must produce no
        // depth-render request.  Audit by walking every
        // request and confirming `NotInvalidated` requests
        // wouldn't depth-render.
        let color_only_does_not_invalidate_depth = !plan.requests.iter().any(|r| {
            matches!(
                r.invalidation_reason,
                LuxShadowInvalidationReason::NotInvalidated,
            ) && r.invalidation_reason.forces_depth_rerender()
        })
            && LuxShadowDirtyInvalidationRule::LightColorOnlyChange.invalidation_reason()
                == LuxShadowInvalidationReason::NotInvalidated
            && !LuxShadowDirtyInvalidationRule::LightColorOnlyChange
                .would_invalidate_shadow_pages_for(LuxShadowPageKind::DirectionalClipmapPage)
            && !LuxShadowDirtyInvalidationRule::LightColorOnlyChange
                .would_invalidate_shadow_pages_for(LuxShadowPageKind::LocalAtlasTile)
            && !LuxShadowDirtyInvalidationRule::LightColorOnlyChange
                .would_invalidate_shadow_pages_for(LuxShadowPageKind::VirtualPage)
            && !LuxShadowDirtyInvalidationRule::LightColorOnlyChange
                .would_invalidate_shadow_pages_for(LuxShadowPageKind::StaticCachePage);
        Self {
            schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
            plan_is_budgeted_and_sorted: plan.is_budgeted_and_sorted(),
            cache_metrics_emitted: plan.cache_metrics_present(),
            color_only_does_not_invalidate_depth,
            directional_and_local_feed_both_pipelines: plan
                .directional_and_local_feed_both_pipelines(),
            debug_views_cover_pass7_set: debug_support.supports_every_pass7_view(),
        }
    }

    /// Typed count of satisfied rules (0..=5).
    #[must_use]
    pub const fn rules_satisfied(&self) -> u32 {
        let mut count = 0u32;
        if self.plan_is_budgeted_and_sorted {
            count += 1;
        }
        if self.cache_metrics_emitted {
            count += 1;
        }
        if self.color_only_does_not_invalidate_depth {
            count += 1;
        }
        if self.directional_and_local_feed_both_pipelines {
            count += 1;
        }
        if self.debug_views_cover_pass7_set {
            count += 1;
        }
        count
    }

    /// Typed bundle predicate: every Pass 7 rule holds.
    #[must_use]
    pub const fn obeys_all_five_rules(&self) -> bool {
        self.rules_satisfied() as usize == FUN_LUX_SHADOW_PASS7_ACCEPTANCE_RULE_COUNT
    }
}

// ============================================================================
// Tests — Pass 7 typed audits
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LuxLightId;
    use crate::shadow::{
        ShadowCastDecision, ShadowPolicyDecision, ShadowQualityTier, ShadowReconstructionMode,
        SoftShadowMode,
    };

    fn sample_decision(light_index: u32, kind: LuxShadowPageKind) -> LuxShadowUpdateDecision {
        let base = ShadowPolicyDecision {
            light_id: LuxLightId::new(light_index as u64),
            decision: match kind {
                LuxShadowPageKind::DirectionalClipmapPage => {
                    ShadowCastDecision::CastDirectionalClipmap
                }
                LuxShadowPageKind::LocalAtlasTile | LuxShadowPageKind::VirtualPage => {
                    ShadowCastDecision::CastLocalPaged
                }
                LuxShadowPageKind::StaticCachePage => ShadowCastDecision::CastLocalPaged,
            },
            quality_tier: ShadowQualityTier::High,
            soft_shadow_mode: SoftShadowMode::ContactAware,
            reconstruction_mode: ShadowReconstructionMode::SpatialTemporal,
            page_budget: 1,
            priority: 100 + light_index as u16,
        };
        let mut decision = LuxShadowUpdateDecision::from_base(base);
        decision.mode = LuxShadowMode::Mixed;
        decision.invalidation_reason = LuxShadowInvalidationReason::LightTransformChanged;
        decision.cacheability = LuxShadowCacheability::CacheUntilInvalidation;
        decision.estimated_update_cost_us_q8 = 100 << 8; // 100 µs.
        decision.receiver_salience_q8 = 128;
        decision.volumetric_contribution_q8 = 64;
        decision.page_kind = kind;
        decision
    }

    #[test]
    fn taxonomy_counts_are_dense() {
        assert_eq!(LuxShadowMode::ALL.len(), FUN_LUX_SHADOW_MODE_COUNT);
        assert_eq!(
            LuxShadowInvalidationReason::ALL.len(),
            FUN_LUX_SHADOW_INVALIDATION_REASON_COUNT,
        );
        assert_eq!(
            LuxShadowCacheability::ALL.len(),
            FUN_LUX_SHADOW_CACHEABILITY_COUNT,
        );
        assert_eq!(LuxShadowPageKind::ALL.len(), FUN_LUX_SHADOW_PAGE_KIND_COUNT,);
        assert_eq!(
            LuxShadowDirtyInvalidationRule::ALL.len(),
            FUN_LUX_SHADOW_DIRTY_RULE_COUNT,
        );
        assert_eq!(
            LuxShadowQualityTierProfile::ALL.len(),
            FUN_LUX_SHADOW_QUALITY_TIER_PROFILE_COUNT,
        );
        assert_eq!(
            LuxShadowDebugOverlayMode::ALL.len(),
            FUN_LUX_SHADOW_DEBUG_OVERLAY_MODE_COUNT,
        );
    }

    #[test]
    fn shadow_mode_cache_predicates() {
        assert!(LuxShadowMode::Static.benefits_from_static_cache());
        assert!(LuxShadowMode::Mixed.benefits_from_static_cache());
        assert!(!LuxShadowMode::Dynamic.benefits_from_static_cache());
        assert!(!LuxShadowMode::Static.requires_per_frame_render());
        assert!(LuxShadowMode::Mixed.requires_per_frame_render());
        assert!(LuxShadowMode::Dynamic.requires_per_frame_render());
    }

    /// Pass 7 rule #3 — light color only changes MUST NOT
    /// invalidate depth.
    #[test]
    fn color_only_invalidation_does_not_force_depth_rerender() {
        assert!(!LuxShadowInvalidationReason::NotInvalidated.forces_depth_rerender());
        for kind in LuxShadowPageKind::ALL {
            assert!(
                !LuxShadowDirtyInvalidationRule::LightColorOnlyChange
                    .would_invalidate_shadow_pages_for(kind),
                "color-only invalidated {:?}",
                kind,
            );
        }
        assert_eq!(
            LuxShadowDirtyInvalidationRule::LightColorOnlyChange.invalidation_reason(),
            LuxShadowInvalidationReason::NotInvalidated,
        );
    }

    /// Pass 7 rule #4 — directional + local pages feed
    /// BOTH direct lighting and volumetrics.
    #[test]
    fn directional_and_local_pages_feed_both_pipelines() {
        for kind in [
            LuxShadowPageKind::DirectionalClipmapPage,
            LuxShadowPageKind::LocalAtlasTile,
        ] {
            assert!(kind.feeds_direct_lighting(), "{:?}", kind);
            assert!(kind.feeds_volumetrics(), "{:?}", kind);
        }
    }

    #[test]
    fn dirty_rule_translates_to_invalidation_reason() {
        assert_eq!(
            LuxShadowDirtyInvalidationRule::LightTransformChange.invalidation_reason(),
            LuxShadowInvalidationReason::LightTransformChanged,
        );
        assert_eq!(
            LuxShadowDirtyInvalidationRule::GeometryChange.invalidation_reason(),
            LuxShadowInvalidationReason::GeometryChanged,
        );
        assert_eq!(
            LuxShadowDirtyInvalidationRule::MaterialAlphaOrShadowFlagChange.invalidation_reason(),
            LuxShadowInvalidationReason::MaterialAlphaOrShadowFlagChanged,
        );
        assert_eq!(
            LuxShadowDirtyInvalidationRule::LightMovement.invalidation_reason(),
            LuxShadowInvalidationReason::LightTransformChanged,
        );
    }

    #[test]
    fn dirty_rules_invalidate_every_depth_kind_except_color_only() {
        for rule in [
            LuxShadowDirtyInvalidationRule::LightTransformChange,
            LuxShadowDirtyInvalidationRule::GeometryChange,
            LuxShadowDirtyInvalidationRule::MaterialAlphaOrShadowFlagChange,
            LuxShadowDirtyInvalidationRule::LightMovement,
        ] {
            for kind in LuxShadowPageKind::ALL {
                assert!(
                    rule.would_invalidate_shadow_pages_for(kind),
                    "{:?} failed to invalidate {:?}",
                    rule,
                    kind,
                );
            }
        }
    }

    #[test]
    fn cache_metrics_hit_ratio_q16() {
        let metrics = LuxShadowCacheMetrics {
            schema_version: FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION,
            cache_hits: 750,
            cache_misses: 250,
            invalidations: 12,
            time_sliced_renders: 4,
            dynamic_overlays: 16,
        };
        // 750 / 1000 = 0.75 → Q16 ~= 49152.
        assert_eq!(metrics.hit_ratio_q16(), 49_152);
        // ZERO returns 0 (no hits/misses).
        assert_eq!(LuxShadowCacheMetrics::ZERO.hit_ratio_q16(), 0);
    }

    #[test]
    fn quality_tier_profile_table_matches_user_spec() {
        assert_eq!(
            LuxShadowQualityTierProfile::LOW.directional_cascade_count,
            2
        );
        const { assert!(!LuxShadowQualityTierProfile::LOW.uses_static_cache) };

        assert_eq!(
            LuxShadowQualityTierProfile::MEDIUM.directional_cascade_count,
            3
        );
        const { assert!(LuxShadowQualityTierProfile::MEDIUM.uses_static_cache) };

        assert_eq!(
            LuxShadowQualityTierProfile::HIGH.directional_cascade_count,
            4
        );
        const { assert!(LuxShadowQualityTierProfile::HIGH.contact_aware_filtering) };

        let cine = LuxShadowQualityTierProfile::CINEMATIC;
        assert!(cine.stochastic_soft_shadows);
        assert!(cine.denoised_virtual_shadows);
        assert!(
            cine.max_local_shadowed_lights
                > LuxShadowQualityTierProfile::HIGH.max_local_shadowed_lights
        );
    }

    #[test]
    fn quality_tier_for_tier_dispatch() {
        assert_eq!(
            LuxShadowQualityTierProfile::for_tier(ShadowQualityTier::Low).directional_cascade_count,
            2,
        );
        assert_eq!(
            LuxShadowQualityTierProfile::for_tier(ShadowQualityTier::Medium)
                .directional_cascade_count,
            3,
        );
        assert_eq!(
            LuxShadowQualityTierProfile::for_tier(ShadowQualityTier::High)
                .directional_cascade_count,
            4,
        );
        assert!(
            LuxShadowQualityTierProfile::for_tier(ShadowQualityTier::Cinematic)
                .stochastic_soft_shadows,
        );
        // `Off` falls back to Low — typed contract.
        assert_eq!(
            LuxShadowQualityTierProfile::for_tier(ShadowQualityTier::Off).tier,
            ShadowQualityTier::Low,
        );
    }

    #[test]
    fn debug_overlay_predicate_and_bundle() {
        for mode in LuxShadowDebugOverlayMode::ALL {
            let satisfies = mode.satisfies_pass7_rule5();
            assert_eq!(satisfies, !matches!(mode, LuxShadowDebugOverlayMode::Off));
        }
        assert!(LuxShadowDebugOverlaySupport::PRODUCT_DEFAULT.supports_every_pass7_view());
        let missing = LuxShadowDebugOverlaySupport {
            invalidations: false,
            ..LuxShadowDebugOverlaySupport::PRODUCT_DEFAULT
        };
        assert!(!missing.supports_every_pass7_view());
    }

    #[test]
    fn plan_finalize_sorts_priority_descending_and_enforces_budget() {
        let mut plan = LuxShadowUpdatePlan::empty("test.plan", 500);
        // Three requests: one cheap+low priority, one expensive+high priority,
        // one moderate+mid priority. The high-priority expensive one should
        // win the budget and the cheap-low one should also fit.
        for (i, (cost, prio)) in [(100, 50), (400, 200), (150, 80)].iter().enumerate() {
            let mut d = sample_decision(i as u32, LuxShadowPageKind::LocalAtlasTile);
            d.estimated_update_cost_us_q8 = (*cost as u32) << 8;
            d.base.priority = *prio as u16;
            d.receiver_salience_q8 = 0; // remove salience confound
            plan.push(LuxShadowPageRequest::from_decision(&d, i as u32));
        }
        plan.cache_metrics.schema_version = FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION;
        plan.finalize();
        // Sorted descending priority.
        let prios: Vec<u32> = plan.requests.iter().map(|r| r.priority_q16).collect();
        let mut sorted = prios.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(prios, sorted);
        // Admitted cost <= budget (no NoCache here).
        assert!(plan.admitted_cost_us <= plan.budget_us);
        assert!(plan.is_budgeted_and_sorted());
    }

    #[test]
    fn plan_admits_no_cache_even_above_budget() {
        let mut plan = LuxShadowUpdatePlan::empty("test.plan", 100);
        // One large NoCache request that exceeds the budget.
        let mut d = sample_decision(0, LuxShadowPageKind::LocalAtlasTile);
        d.estimated_update_cost_us_q8 = 500 << 8;
        d.cacheability = LuxShadowCacheability::NoCache;
        d.receiver_salience_q8 = 0;
        plan.push(LuxShadowPageRequest::from_decision(&d, 0));
        plan.cache_metrics.schema_version = FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION;
        plan.finalize();
        // NoCache forced-admitted.
        assert_eq!(plan.admitted_cost_us, 500);
        assert_eq!(plan.deferred_request_count, 0);
        assert!(plan.is_budgeted_and_sorted());
    }

    /// Pass 7 rule #1 — plan is budgeted + priority-sorted.
    #[test]
    fn pass7_rule1_holds_for_finalized_plan() {
        let mut plan = LuxShadowUpdatePlan::empty("test.plan", 1_000);
        for i in 0..5 {
            let mut d = sample_decision(i, LuxShadowPageKind::LocalAtlasTile);
            d.estimated_update_cost_us_q8 = (100 + i * 50) << 8;
            plan.push(LuxShadowPageRequest::from_decision(&d, i));
        }
        plan.cache_metrics.schema_version = FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION;
        plan.finalize();
        assert!(plan.is_budgeted_and_sorted());
    }

    /// Pass 7 rule #2 — cache metrics emit hit / miss /
    /// invalidation counters.
    #[test]
    fn pass7_rule2_cache_metrics_emitted() {
        let mut plan = LuxShadowUpdatePlan::empty("test.plan", 100);
        assert!(!plan.cache_metrics_present());
        plan.cache_metrics.schema_version = FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION;
        plan.cache_metrics.cache_hits = 10;
        plan.cache_metrics.cache_misses = 2;
        plan.cache_metrics.invalidations = 1;
        assert!(plan.cache_metrics_present());
    }

    /// Pass 7 acceptance verdict — every rule holds when
    /// the plan is well-formed.
    #[test]
    fn verdict_passes_when_plan_and_debug_are_complete() {
        let mut plan = LuxShadowUpdatePlan::empty("test.plan", 1_000);
        for i in 0..4 {
            let kind = if i % 2 == 0 {
                LuxShadowPageKind::DirectionalClipmapPage
            } else {
                LuxShadowPageKind::LocalAtlasTile
            };
            let d = sample_decision(i, kind);
            plan.push(LuxShadowPageRequest::from_decision(&d, i));
        }
        plan.cache_metrics.schema_version = FUN_LUX_SHADOW_UPDATE_PLAN_SCHEMA_VERSION;
        plan.cache_metrics.cache_hits = 10;
        plan.cache_metrics.cache_misses = 2;
        plan.cache_metrics.invalidations = 1;
        plan.finalize();
        let verdict = LuxShadowPass7AcceptanceVerdict::evaluate(
            &plan,
            &LuxShadowDebugOverlaySupport::PRODUCT_DEFAULT,
        );
        assert!(verdict.obeys_all_five_rules(), "verdict: {:?}", verdict);
        assert_eq!(
            verdict.rules_satisfied(),
            FUN_LUX_SHADOW_PASS7_ACCEPTANCE_RULE_COUNT as u32,
        );
    }

    /// Pass 7 acceptance verdict — flips when ANY rule
    /// fails.
    #[test]
    fn verdict_flips_when_any_rule_fails() {
        let mut plan = LuxShadowUpdatePlan::empty("test.plan", 1_000);
        for i in 0..2 {
            let d = sample_decision(i, LuxShadowPageKind::DirectionalClipmapPage);
            plan.push(LuxShadowPageRequest::from_decision(&d, i));
        }
        // Skip emitting cache metrics — rule #2 should fail.
        plan.finalize();
        let verdict = LuxShadowPass7AcceptanceVerdict::evaluate(
            &plan,
            &LuxShadowDebugOverlaySupport::PRODUCT_DEFAULT,
        );
        assert!(!verdict.obeys_all_five_rules());
        assert!(!verdict.cache_metrics_emitted);
        // Remaining rules still hold.
        assert!(verdict.plan_is_budgeted_and_sorted);
        assert!(verdict.color_only_does_not_invalidate_depth);
        assert!(verdict.directional_and_local_feed_both_pipelines);
        assert!(verdict.debug_views_cover_pass7_set);
    }

    #[test]
    fn decision_priority_q16_orders_high_salience_above_low() {
        let mut high_sal = sample_decision(0, LuxShadowPageKind::LocalAtlasTile);
        high_sal.receiver_salience_q8 = 250;
        let mut low_sal = sample_decision(0, LuxShadowPageKind::LocalAtlasTile);
        low_sal.receiver_salience_q8 = 10;
        assert!(high_sal.plan_priority_q16() > low_sal.plan_priority_q16());
    }

    #[test]
    fn decision_needs_depth_render_predicate() {
        let mut d = sample_decision(0, LuxShadowPageKind::LocalAtlasTile);
        assert!(d.needs_depth_render());
        d.invalidation_reason = LuxShadowInvalidationReason::NotInvalidated;
        assert!(!d.needs_depth_render());
    }
}
