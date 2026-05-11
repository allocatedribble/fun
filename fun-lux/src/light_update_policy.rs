//! Pass 6 — typed light update policy for the clustered
//! Forward+ + reservoir many-light path.
//!
//! Encodes the user's "Dynamic update rules" verbatim:
//!
//! - Transform change: update light buffer / mark affected
//!   clusters dirty / shadow invalidation only if shadowed
//!   and moved enough.
//! - Color/intensity change: update light buffer / update
//!   reservoir stamps / do not invalidate shadow pages.
//! - Range/shape change: update light buffer / recull
//!   affected clusters.
//! - Removal: tombstone or swap-remove / invalidate
//!   reservoirs referencing removed light / invalidate
//!   shadow pages if needed.
//!
//! The rules land as typed predicates on
//! [`LuxLightUpdateImpact`] so the renderer (or any other
//! consumer) can ask, in one typed call, "what does this
//! light update touch?" without reimplementing the policy.
//!
//! Pass 6 keeps every record-level type renderer-neutral —
//! fun-lux still owns no `wgpu` / `naga` / raw-window
//! imports.

use crate::dirty::LuxDirtyFlags;

pub const FUN_LUX_LIGHT_UPDATE_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_LIGHT_UPDATE_KIND_COUNT: usize = 7;
pub const FUN_LUX_LIGHT_UPDATE_IMPACT_FLAG_COUNT: usize = 6;
pub const FUN_LUX_LIGHT_REMOVAL_STRATEGY_COUNT: usize = 2;

// ============================================================================
// Section 1 — Typed light update kind (4 user classes + creation)
// ============================================================================

/// Typed light-update class. Pass 6 splits the user's
/// "Color/intensity change" rule into two typed kinds
/// (`ColorChange` + `IntensityChange`) because intensity
/// changes touch reservoir weights more aggressively, and
/// adds a typed `Creation` kind so the policy table covers
/// every mutation a system can emit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLightUpdateKind {
    /// Light moved or rotated. Triggers cluster re-binning;
    /// shadow page invalidation gated on
    /// `moved_enough_for_shadow_invalidation`.
    TransformChange,
    /// Light color (hue / chroma) changed. Does NOT
    /// invalidate shadow pages. Updates reservoir stamps so
    /// stale reservoirs reject themselves on the next reuse
    /// pass.
    ColorChange,
    /// Light intensity (lumens / radiant power) changed.
    /// Same shadow rule as `ColorChange`; reservoir stamps
    /// are dirtied; cluster re-binning fires because
    /// intensity affects effective range.
    IntensityChange,
    /// Light range / falloff radius changed. Triggers
    /// cluster re-culling but leaves shadow pages valid (the
    /// shadow caster set hasn't moved).
    RangeChange,
    /// Light shape (point → spot → area, or cone angle)
    /// changed. Triggers cluster re-binning AND shadow page
    /// invalidation (shape affects shadow projection).
    ShapeChange,
    /// Light removed. Either tombstoned or swap-removed.
    /// Always invalidates reservoirs that reference the
    /// removed light ID; shadow pages invalidated only if
    /// the light was shadowed.
    Removal,
    /// Light created. Fresh buffer slot + cluster binning;
    /// no reservoir to invalidate (reservoirs key off the
    /// stable ID which is new).
    #[default]
    Creation,
}

impl LuxLightUpdateKind {
    pub const ALL: [Self; FUN_LUX_LIGHT_UPDATE_KIND_COUNT] = [
        Self::TransformChange,
        Self::ColorChange,
        Self::IntensityChange,
        Self::RangeChange,
        Self::ShapeChange,
        Self::Removal,
        Self::Creation,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TransformChange => "transform_change",
            Self::ColorChange => "color_change",
            Self::IntensityChange => "intensity_change",
            Self::RangeChange => "range_change",
            Self::ShapeChange => "shape_change",
            Self::Removal => "removal",
            Self::Creation => "creation",
        }
    }

    /// Typed translation into the Pass 2
    /// [`LuxDirtyFlags`] mask emitted by this update class.
    /// Lets the canonical dirty-tracking pipeline consume
    /// Pass 6 updates without re-classifying.
    #[must_use]
    pub const fn dirty_flags(self) -> LuxDirtyFlags {
        match self {
            Self::TransformChange => LuxDirtyFlags::TRANSFORM,
            Self::ColorChange => LuxDirtyFlags::COLOR,
            Self::IntensityChange => LuxDirtyFlags::INTENSITY,
            Self::RangeChange => LuxDirtyFlags::RANGE,
            Self::ShapeChange => LuxDirtyFlags::SHAPE,
            Self::Removal => LuxDirtyFlags::REMOVED,
            Self::Creation => LuxDirtyFlags::CREATED,
        }
    }
}

// ============================================================================
// Section 2 — Typed update context (shadow + motion gating)
// ============================================================================

/// Typed context the policy needs to decide shadow-page
/// invalidation for transform updates. The renderer is
/// expected to supply both flags per update event.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxLightUpdateContext {
    /// Was the light casting shadows at the time of the
    /// update? If `false`, the shadow page invalidation
    /// branch is short-circuited regardless of motion.
    pub is_shadowed: bool,
    /// Did the light move far enough (renderer-defined
    /// threshold) to require shadow-page invalidation? Pass
    /// 6 leaves the threshold to the renderer (it depends on
    /// shadow page texel size, distance to camera, etc.) and
    /// only encodes the policy that consumes the boolean.
    pub moved_enough_for_shadow_invalidation: bool,
}

impl LuxLightUpdateContext {
    /// Typed default: unshadowed + no motion. Equivalent to
    /// `Default::default()` but `const`.
    pub const UNSHADOWED: Self = Self {
        is_shadowed: false,
        moved_enough_for_shadow_invalidation: false,
    };

    /// Typed builder: shadowed + moved beyond the renderer's
    /// invalidation threshold.
    #[must_use]
    pub const fn shadowed_and_moved_enough() -> Self {
        Self {
            is_shadowed: true,
            moved_enough_for_shadow_invalidation: true,
        }
    }

    /// Typed builder: shadowed but did NOT move beyond the
    /// threshold (small jitter, settling, etc.). The policy
    /// MUST NOT invalidate shadow pages for this case.
    #[must_use]
    pub const fn shadowed_but_below_threshold() -> Self {
        Self {
            is_shadowed: true,
            moved_enough_for_shadow_invalidation: false,
        }
    }
}

// ============================================================================
// Section 3 — Typed impact flag set (what does this touch?)
// ============================================================================

/// Typed bit set of "what this light update invalidates."
/// Mirrors the Pass 2 `LuxDirtyFlags` shape (a typed bag of
/// const bit masks + predicates) but stays scoped to the
/// many-light buffers Pass 6 introduces.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxLightUpdateImpactFlags(pub u32);

impl LuxLightUpdateImpactFlags {
    pub const NONE: Self = Self(0);
    /// The typed `LightRecord` slot in the GPU light buffer
    /// must be re-uploaded.
    pub const LIGHT_BUFFER_DIRTY: Self = Self(1 << 0);
    /// One or more clusters in the cluster grid must be
    /// re-binned.
    pub const CLUSTERS_DIRTY: Self = Self(1 << 1);
    /// The cluster → light-index list must be rebuilt.
    pub const LIGHT_INDEX_DIRTY: Self = Self(1 << 2);
    /// Reservoir update stamps for samples referencing this
    /// light must bump so stale reservoirs reject themselves.
    pub const RESERVOIR_STAMPS_DIRTY: Self = Self(1 << 3);
    /// Reservoirs referencing this light must be
    /// invalidated outright (e.g., light was removed).
    pub const RESERVOIRS_INVALIDATED: Self = Self(1 << 4);
    /// Shadow page(s) for this light must be invalidated.
    pub const SHADOW_PAGES_INVALIDATED: Self = Self(1 << 5);

    pub const ALL: Self = Self(
        Self::LIGHT_BUFFER_DIRTY.0
            | Self::CLUSTERS_DIRTY.0
            | Self::LIGHT_INDEX_DIRTY.0
            | Self::RESERVOIR_STAMPS_DIRTY.0
            | Self::RESERVOIRS_INVALIDATED.0
            | Self::SHADOW_PAGES_INVALIDATED.0,
    );

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

// ============================================================================
// Section 4 — Typed impact record (kind + flags + context)
// ============================================================================

/// Typed impact record. Bundles the originating kind, the
/// context the policy consumed, and the resulting flag set
/// so a downstream consumer (graph compiler, diagnostics,
/// tests) can prove the policy fired the way the user spec
/// requires.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxLightUpdateImpact {
    pub schema_version: u16,
    pub kind: LuxLightUpdateKind,
    pub context: LuxLightUpdateContext,
    pub flags: LuxLightUpdateImpactFlags,
}

impl LuxLightUpdateImpact {
    /// Typed constructor: encodes the user's four typed
    /// rules + the two extra kinds (`IntensityChange` split,
    /// `Creation`).
    ///
    /// The rules are encoded INLINE here so the policy is
    /// readable in one place; the predicates in Section 5
    /// merely re-check them as acceptance-style tests.
    #[must_use]
    pub const fn for_kind(kind: LuxLightUpdateKind, context: LuxLightUpdateContext) -> Self {
        let flags = match kind {
            // ----------------------------------------------------------------
            // Transform change: update light buffer / mark
            // affected clusters dirty / shadow invalidation
            // only if shadowed AND moved enough.
            // ----------------------------------------------------------------
            LuxLightUpdateKind::TransformChange => {
                let base = LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY
                    .with(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
                    .with(LuxLightUpdateImpactFlags::LIGHT_INDEX_DIRTY)
                    // Reservoir stamps must bump because a
                    // moved light invalidates the cached
                    // visibility / contribution estimate.
                    .with(LuxLightUpdateImpactFlags::RESERVOIR_STAMPS_DIRTY);
                if context.is_shadowed && context.moved_enough_for_shadow_invalidation {
                    base.with(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
                } else {
                    base
                }
            }
            // ----------------------------------------------------------------
            // Color change: update light buffer / update
            // reservoir stamps / do NOT invalidate shadow
            // pages. Clusters stay valid (color is not
            // spatial).
            // ----------------------------------------------------------------
            LuxLightUpdateKind::ColorChange => LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY
                .with(LuxLightUpdateImpactFlags::RESERVOIR_STAMPS_DIRTY),
            // ----------------------------------------------------------------
            // Intensity change: update light buffer / update
            // reservoir stamps / do NOT invalidate shadow
            // pages. Cluster re-binning fires because
            // intensity affects effective range (matches
            // Pass 2's `invalidates_direct_light_clusters`
            // mask which includes INTENSITY).
            // ----------------------------------------------------------------
            LuxLightUpdateKind::IntensityChange => LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY
                .with(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
                .with(LuxLightUpdateImpactFlags::LIGHT_INDEX_DIRTY)
                .with(LuxLightUpdateImpactFlags::RESERVOIR_STAMPS_DIRTY),
            // ----------------------------------------------------------------
            // Range change: update light buffer / recull
            // affected clusters. Shadow pages stay valid.
            // ----------------------------------------------------------------
            LuxLightUpdateKind::RangeChange => LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY
                .with(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
                .with(LuxLightUpdateImpactFlags::LIGHT_INDEX_DIRTY)
                .with(LuxLightUpdateImpactFlags::RESERVOIR_STAMPS_DIRTY),
            // ----------------------------------------------------------------
            // Shape change: update light buffer / recull
            // affected clusters / shadow pages invalidated
            // when the light is shadowed (shape affects
            // shadow projection).
            // ----------------------------------------------------------------
            LuxLightUpdateKind::ShapeChange => {
                let base = LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY
                    .with(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
                    .with(LuxLightUpdateImpactFlags::LIGHT_INDEX_DIRTY)
                    .with(LuxLightUpdateImpactFlags::RESERVOIR_STAMPS_DIRTY);
                if context.is_shadowed {
                    base.with(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
                } else {
                    base
                }
            }
            // ----------------------------------------------------------------
            // Removal: tombstone or swap-remove / invalidate
            // reservoirs referencing removed light /
            // invalidate shadow pages if shadowed.
            // ----------------------------------------------------------------
            LuxLightUpdateKind::Removal => {
                let base = LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY
                    .with(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
                    .with(LuxLightUpdateImpactFlags::LIGHT_INDEX_DIRTY)
                    .with(LuxLightUpdateImpactFlags::RESERVOIRS_INVALIDATED);
                if context.is_shadowed {
                    base.with(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
                } else {
                    base
                }
            }
            // ----------------------------------------------------------------
            // Creation: light buffer + cluster bind. No
            // reservoir to invalidate (reservoirs key off
            // stable IDs, the new ID is unique).
            // ----------------------------------------------------------------
            LuxLightUpdateKind::Creation => LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY
                .with(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
                .with(LuxLightUpdateImpactFlags::LIGHT_INDEX_DIRTY),
        };
        Self {
            schema_version: FUN_LUX_LIGHT_UPDATE_SCHEMA_VERSION,
            kind,
            context,
            flags,
        }
    }

    /// Convenience: typed impact for the unshadowed default
    /// context. Equivalent to
    /// `Self::for_kind(kind, LuxLightUpdateContext::UNSHADOWED)`.
    #[must_use]
    pub const fn for_kind_unshadowed(kind: LuxLightUpdateKind) -> Self {
        Self::for_kind(kind, LuxLightUpdateContext::UNSHADOWED)
    }
}

// ============================================================================
// Section 5 — Typed acceptance predicates (Pass 6 rule audits)
// ============================================================================

impl LuxLightUpdateImpact {
    /// Typed predicate matching the user spec rule:
    /// *Transform change: shadow invalidation only if
    /// shadowed and moved enough.*
    #[must_use]
    pub const fn obeys_transform_shadow_gate(self) -> bool {
        match self.kind {
            LuxLightUpdateKind::TransformChange => {
                let must_invalidate =
                    self.context.is_shadowed && self.context.moved_enough_for_shadow_invalidation;
                self.flags
                    .contains(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
                    == must_invalidate
            }
            _ => true,
        }
    }

    /// Typed predicate matching the user spec rule:
    /// *Color / intensity change: do NOT invalidate shadow
    /// pages.*
    #[must_use]
    pub const fn obeys_color_intensity_shadow_invariance(self) -> bool {
        match self.kind {
            LuxLightUpdateKind::ColorChange | LuxLightUpdateKind::IntensityChange => !self
                .flags
                .intersects(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED),
            _ => true,
        }
    }

    /// Typed predicate matching the user spec rule:
    /// *Range / shape change: recull affected clusters.*
    #[must_use]
    pub const fn obeys_range_shape_recull(self) -> bool {
        match self.kind {
            LuxLightUpdateKind::RangeChange | LuxLightUpdateKind::ShapeChange => self
                .flags
                .contains(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY),
            _ => true,
        }
    }

    /// Typed predicate matching the user spec rule:
    /// *Removal: invalidate reservoirs referencing the
    /// removed light.*
    #[must_use]
    pub const fn obeys_removal_reservoir_invalidation(self) -> bool {
        match self.kind {
            LuxLightUpdateKind::Removal => self
                .flags
                .contains(LuxLightUpdateImpactFlags::RESERVOIRS_INVALIDATED),
            _ => true,
        }
    }

    /// Typed bundle predicate: every Pass 6 update rule
    /// holds for this impact record. Returns `true` only
    /// when ALL four typed rules are satisfied
    /// simultaneously.
    #[must_use]
    pub const fn obeys_all_pass6_rules(self) -> bool {
        self.obeys_transform_shadow_gate()
            && self.obeys_color_intensity_shadow_invariance()
            && self.obeys_range_shape_recull()
            && self.obeys_removal_reservoir_invalidation()
    }
}

// ============================================================================
// Section 6 — Typed removal strategy ("tombstone or swap-remove")
// ============================================================================

/// Typed light-removal strategy. The user spec calls out
/// "tombstone or swap-remove" — both are valid; Pass 6
/// encodes the choice as a typed enum + a typed heuristic
/// so the renderer doesn't reinvent the policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLightRemovalStrategy {
    /// Mark the slot as a tombstone (live record byte
    /// pattern reserved for "dead"). Preserves stable IDs
    /// but wastes a slot until the next compaction.
    ///
    /// Right answer when stable IDs are referenced by
    /// long-lived data structures (reservoir history, AI
    /// hints, network replication tables) and a one-frame
    /// swap would invalidate too much.
    Tombstone,
    /// Swap the last live record into the freed slot, then
    /// truncate. Compact but rewrites the moved record's
    /// stable-ID → slot map.
    ///
    /// Typed default — Pass 6 expects most scenes to favor
    /// the compact path because reservoir history reuses
    /// the *light's* stable ID (not its slot) so swap-remove
    /// stays safe.
    #[default]
    SwapRemove,
}

impl LuxLightRemovalStrategy {
    pub const ALL: [Self; FUN_LUX_LIGHT_REMOVAL_STRATEGY_COUNT] =
        [Self::Tombstone, Self::SwapRemove];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tombstone => "tombstone",
            Self::SwapRemove => "swap_remove",
        }
    }

    /// Typed heuristic: pick a strategy based on the live
    /// light record count. Tombstone scales worse with
    /// growth (slot waste accumulates) so above a threshold
    /// we prefer swap-remove. Threshold is tuned to keep the
    /// dense path under 1024 records — matching the user's
    /// many-light stress targets (100/500/1000).
    #[must_use]
    pub const fn for_live_record_count(count: u32) -> Self {
        if count >= 1024 {
            Self::SwapRemove
        } else {
            Self::Tombstone
        }
    }

    /// Typed predicate: does this strategy preserve stable
    /// IDs in place (no slot remap)?
    #[must_use]
    pub const fn preserves_slot_identity(self) -> bool {
        matches!(self, Self::Tombstone)
    }
}

// ============================================================================
// Section 7 — Typed bundle (policy + removal strategy)
// ============================================================================

/// Typed Pass 6 policy bundle. Carries the per-kind policy
/// + the removal strategy so callers consume a single
/// typed handle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxLightUpdatePolicy {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub removal_strategy: LuxLightRemovalStrategy,
}

impl LuxLightUpdatePolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_LIGHT_UPDATE_SCHEMA_VERSION,
        stable_id: "fun_lux.light_update.policy.product_default",
        removal_strategy: LuxLightRemovalStrategy::SwapRemove,
    };

    /// Typed dispatch: evaluate the policy for a given
    /// update kind + context. Equivalent to
    /// `LuxLightUpdateImpact::for_kind(kind, context)` but
    /// keeps the policy struct as the entry point.
    #[must_use]
    pub const fn impact_for(
        &self,
        kind: LuxLightUpdateKind,
        context: LuxLightUpdateContext,
    ) -> LuxLightUpdateImpact {
        LuxLightUpdateImpact::for_kind(kind, context)
    }
}

// ============================================================================
// Tests — Pass 6 acceptance audits (one test per typed rule)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_kind_taxonomy_is_dense() {
        assert_eq!(
            LuxLightUpdateKind::ALL.len(),
            FUN_LUX_LIGHT_UPDATE_KIND_COUNT,
        );
        for k in LuxLightUpdateKind::ALL {
            assert!(!k.as_str().is_empty());
        }
    }

    #[test]
    fn update_kind_translates_to_dirty_flags() {
        assert_eq!(
            LuxLightUpdateKind::TransformChange.dirty_flags(),
            LuxDirtyFlags::TRANSFORM,
        );
        assert_eq!(
            LuxLightUpdateKind::ColorChange.dirty_flags(),
            LuxDirtyFlags::COLOR,
        );
        assert_eq!(
            LuxLightUpdateKind::IntensityChange.dirty_flags(),
            LuxDirtyFlags::INTENSITY,
        );
        assert_eq!(
            LuxLightUpdateKind::RangeChange.dirty_flags(),
            LuxDirtyFlags::RANGE,
        );
        assert_eq!(
            LuxLightUpdateKind::ShapeChange.dirty_flags(),
            LuxDirtyFlags::SHAPE,
        );
        assert_eq!(
            LuxLightUpdateKind::Removal.dirty_flags(),
            LuxDirtyFlags::REMOVED,
        );
        assert_eq!(
            LuxLightUpdateKind::Creation.dirty_flags(),
            LuxDirtyFlags::CREATED,
        );
    }

    /// Pass 6 user rule: *Transform change: shadow
    /// invalidation only if shadowed and moved enough.*
    #[test]
    fn transform_shadow_gate_holds() {
        // Unshadowed + moved → no shadow invalidation.
        let impact = LuxLightUpdateImpact::for_kind(
            LuxLightUpdateKind::TransformChange,
            LuxLightUpdateContext {
                is_shadowed: false,
                moved_enough_for_shadow_invalidation: true,
            },
        );
        assert!(
            !impact
                .flags
                .intersects(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
        );
        assert!(impact.obeys_transform_shadow_gate());

        // Shadowed but below threshold → no shadow
        // invalidation.
        let impact = LuxLightUpdateImpact::for_kind(
            LuxLightUpdateKind::TransformChange,
            LuxLightUpdateContext::shadowed_but_below_threshold(),
        );
        assert!(
            !impact
                .flags
                .intersects(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
        );
        assert!(impact.obeys_transform_shadow_gate());

        // Shadowed + moved enough → invalidate.
        let impact = LuxLightUpdateImpact::for_kind(
            LuxLightUpdateKind::TransformChange,
            LuxLightUpdateContext::shadowed_and_moved_enough(),
        );
        assert!(
            impact
                .flags
                .contains(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
        );
        assert!(impact.obeys_transform_shadow_gate());
    }

    /// Pass 6 user rule: *Color / intensity change: do NOT
    /// invalidate shadow pages.*
    #[test]
    fn color_intensity_never_invalidate_shadow_pages() {
        for kind in [
            LuxLightUpdateKind::ColorChange,
            LuxLightUpdateKind::IntensityChange,
        ] {
            for ctx in [
                LuxLightUpdateContext::UNSHADOWED,
                LuxLightUpdateContext::shadowed_and_moved_enough(),
                LuxLightUpdateContext::shadowed_but_below_threshold(),
            ] {
                let impact = LuxLightUpdateImpact::for_kind(kind, ctx);
                assert!(
                    !impact
                        .flags
                        .intersects(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED),
                    "kind {:?} ctx {:?} unexpectedly invalidated shadow pages",
                    kind,
                    ctx,
                );
                assert!(impact.obeys_color_intensity_shadow_invariance());
            }
        }
    }

    /// Pass 6 user rule: *Color / intensity change: update
    /// reservoir stamps.*
    #[test]
    fn color_intensity_bump_reservoir_stamps() {
        for kind in [
            LuxLightUpdateKind::ColorChange,
            LuxLightUpdateKind::IntensityChange,
        ] {
            let impact = LuxLightUpdateImpact::for_kind_unshadowed(kind);
            assert!(
                impact
                    .flags
                    .contains(LuxLightUpdateImpactFlags::RESERVOIR_STAMPS_DIRTY)
            );
            assert!(
                impact
                    .flags
                    .contains(LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY)
            );
        }
    }

    /// Pass 6 user rule: *Range / shape change: recull
    /// affected clusters.*
    #[test]
    fn range_and_shape_change_recull_clusters() {
        for kind in [
            LuxLightUpdateKind::RangeChange,
            LuxLightUpdateKind::ShapeChange,
        ] {
            let impact = LuxLightUpdateImpact::for_kind_unshadowed(kind);
            assert!(
                impact
                    .flags
                    .contains(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
            );
            assert!(
                impact
                    .flags
                    .contains(LuxLightUpdateImpactFlags::LIGHT_INDEX_DIRTY)
            );
            assert!(impact.obeys_range_shape_recull());
        }
    }

    /// Pass 6 user rule: *Removal: invalidate reservoirs
    /// referencing the removed light.*
    #[test]
    fn removal_invalidates_reservoirs() {
        let impact = LuxLightUpdateImpact::for_kind_unshadowed(LuxLightUpdateKind::Removal);
        assert!(
            impact
                .flags
                .contains(LuxLightUpdateImpactFlags::RESERVOIRS_INVALIDATED)
        );
        assert!(impact.obeys_removal_reservoir_invalidation());
    }

    /// Pass 6 user rule: *Removal: invalidate shadow pages
    /// if needed.* — only when the removed light was
    /// shadowed.
    #[test]
    fn removal_shadow_invalidation_gated_on_shadowed() {
        let unshadowed = LuxLightUpdateImpact::for_kind_unshadowed(LuxLightUpdateKind::Removal);
        assert!(
            !unshadowed
                .flags
                .intersects(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
        );

        let shadowed = LuxLightUpdateImpact::for_kind(
            LuxLightUpdateKind::Removal,
            LuxLightUpdateContext::shadowed_and_moved_enough(),
        );
        assert!(
            shadowed
                .flags
                .contains(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
        );
    }

    /// Typed bundle audit: every typed kind produced by
    /// `for_kind` satisfies every Pass 6 rule.
    #[test]
    fn every_kind_satisfies_every_pass6_rule() {
        for kind in LuxLightUpdateKind::ALL {
            for ctx in [
                LuxLightUpdateContext::UNSHADOWED,
                LuxLightUpdateContext::shadowed_and_moved_enough(),
                LuxLightUpdateContext::shadowed_but_below_threshold(),
            ] {
                let impact = LuxLightUpdateImpact::for_kind(kind, ctx);
                assert!(
                    impact.obeys_all_pass6_rules(),
                    "kind {:?} ctx {:?} failed",
                    kind,
                    ctx,
                );
            }
        }
    }

    #[test]
    fn creation_skips_reservoir_invalidation() {
        let impact = LuxLightUpdateImpact::for_kind_unshadowed(LuxLightUpdateKind::Creation);
        assert!(
            !impact
                .flags
                .intersects(LuxLightUpdateImpactFlags::RESERVOIRS_INVALIDATED)
        );
        assert!(
            !impact
                .flags
                .intersects(LuxLightUpdateImpactFlags::RESERVOIR_STAMPS_DIRTY)
        );
        assert!(
            impact
                .flags
                .contains(LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY)
        );
        assert!(
            impact
                .flags
                .contains(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY)
        );
    }

    #[test]
    fn impact_flags_bitset_invariants() {
        let mut f = LuxLightUpdateImpactFlags::NONE;
        assert!(f.is_empty());
        f.insert(LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY);
        assert!(f.contains(LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY));
        assert!(!f.contains(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY));
        let g = f.with(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY);
        assert!(g.contains(LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY));
        assert!(g.contains(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY));
        let h = g.without(LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY);
        assert!(!h.contains(LuxLightUpdateImpactFlags::LIGHT_BUFFER_DIRTY));
        assert!(h.contains(LuxLightUpdateImpactFlags::CLUSTERS_DIRTY));
        assert!(LuxLightUpdateImpactFlags::ALL.contains(h));
    }

    #[test]
    fn removal_strategy_heuristic_picks_swap_remove_above_1024() {
        assert_eq!(
            LuxLightRemovalStrategy::for_live_record_count(0),
            LuxLightRemovalStrategy::Tombstone,
        );
        assert_eq!(
            LuxLightRemovalStrategy::for_live_record_count(100),
            LuxLightRemovalStrategy::Tombstone,
        );
        assert_eq!(
            LuxLightRemovalStrategy::for_live_record_count(500),
            LuxLightRemovalStrategy::Tombstone,
        );
        assert_eq!(
            LuxLightRemovalStrategy::for_live_record_count(1023),
            LuxLightRemovalStrategy::Tombstone,
        );
        assert_eq!(
            LuxLightRemovalStrategy::for_live_record_count(1024),
            LuxLightRemovalStrategy::SwapRemove,
        );
        assert_eq!(
            LuxLightRemovalStrategy::for_live_record_count(10_000),
            LuxLightRemovalStrategy::SwapRemove,
        );
    }

    #[test]
    fn removal_strategy_slot_identity_predicate() {
        assert!(LuxLightRemovalStrategy::Tombstone.preserves_slot_identity());
        assert!(!LuxLightRemovalStrategy::SwapRemove.preserves_slot_identity());
    }

    #[test]
    fn policy_product_default_dispatches_correctly() {
        let policy = LuxLightUpdatePolicy::PRODUCT_DEFAULT;
        let impact = policy.impact_for(
            LuxLightUpdateKind::TransformChange,
            LuxLightUpdateContext::shadowed_and_moved_enough(),
        );
        assert!(
            impact
                .flags
                .contains(LuxLightUpdateImpactFlags::SHADOW_PAGES_INVALIDATED)
        );
        assert!(impact.obeys_all_pass6_rules());
        assert_eq!(policy.removal_strategy, LuxLightRemovalStrategy::SwapRemove);
    }
}
