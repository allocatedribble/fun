//! Typed dirty flags + world-space dirty regions for Pass 2's
//! partial-update lighting state.
//!
//! `LuxDirtyFlags` is the typed 12-bit set that names which
//! lighting subsystems a light / cluster / scene mutation
//! invalidates. The typed predicates (`invalidates_shadow_maps`,
//! `invalidates_direct_light_clusters`, `touches_volumetric_only`,
//! `drives_static_cache_rebuild`) encode Pass 2's acceptance
//! criteria at the type-system layer so the planner cannot
//! accidentally schedule wasted work.
//!
//! `LuxDirtyRegion` is the typed world-space dirty volume
//! (`LuxAabb` + `LuxDirtyFlags` + priority + cost estimate +
//! optional deadline frame). The renderer reads
//! `LuxDirtyRegion::flags` to know which passes a particular
//! dirty volume must drive; the typed planner aggregates the
//! regions before emitting passes so that overlapping dirty
//! volumes coalesce.

use crate::aabb::LuxAabb;
use crate::frame_plan::LuxSceneId;

pub const FUN_LUX_DIRTY_SCHEMA_VERSION: u16 = 1;
pub const LUX_DIRTY_FLAG_BIT_COUNT: usize = 12;

/// Typed dirty flag set. Each bit names one lighting
/// subsystem that a light / cluster / scene mutation
/// invalidates. The typed predicates record Pass 2's
/// acceptance criteria.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxDirtyFlags(pub u32);

impl LuxDirtyFlags {
    pub const NONE: Self = Self(0);
    pub const TRANSFORM: Self = Self(1 << 0);
    pub const COLOR: Self = Self(1 << 1);
    pub const INTENSITY: Self = Self(1 << 2);
    pub const SHAPE: Self = Self(1 << 3);
    pub const RANGE: Self = Self(1 << 4);
    pub const SHADOW_POLICY: Self = Self(1 << 5);
    pub const VOLUMETRIC: Self = Self(1 << 6);
    pub const STATIC_CACHE: Self = Self(1 << 7);
    pub const GI_CACHE: Self = Self(1 << 8);
    pub const REFLECTION_CACHE: Self = Self(1 << 9);
    pub const REMOVED: Self = Self(1 << 10);
    pub const CREATED: Self = Self(1 << 11);

    /// Typed mask: every defined flag bit.
    pub const ALL: Self = Self(
        Self::TRANSFORM.0
            | Self::COLOR.0
            | Self::INTENSITY.0
            | Self::SHAPE.0
            | Self::RANGE.0
            | Self::SHADOW_POLICY.0
            | Self::VOLUMETRIC.0
            | Self::STATIC_CACHE.0
            | Self::GI_CACHE.0
            | Self::REFLECTION_CACHE.0
            | Self::REMOVED.0
            | Self::CREATED.0,
    );

    /// Typed predicate: are these flags empty (no
    /// invalidation)?
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Typed predicate: does `self` contain every bit in
    /// `other`?
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Typed predicate: does `self` intersect `other`?
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Typed bit insertion (returns the new flag set).
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Typed bit insertion (in place).
    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    /// Typed bit removal (in place).
    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    /// Typed bit removal (returns the new flag set).
    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    // ========================================================
    // Acceptance-criteria predicates (Pass 2)
    // ========================================================

    /// Pass 2 acceptance: light color / intensity changes
    /// MUST NOT invalidate shadow maps. Returns `true` for
    /// the typed mutation classes that DO invalidate the
    /// shadow map: TRANSFORM, SHAPE, SHADOW_POLICY, REMOVED.
    /// Returns `false` when ONLY COLOR / INTENSITY are set
    /// (or any subset thereof) — those mutations leave the
    /// shadow map valid.
    #[must_use]
    pub const fn invalidates_shadow_maps(self) -> bool {
        let mask = Self::TRANSFORM.0 | Self::SHAPE.0 | Self::SHADOW_POLICY.0 | Self::REMOVED.0;
        (self.0 & mask) != 0
    }

    /// Pass 2 acceptance: light transform changes MUST
    /// trigger direct-light cluster re-binning in the same
    /// frame. Returns `true` for the typed mutation classes
    /// that affect cluster bins: TRANSFORM, INTENSITY, RANGE,
    /// SHAPE, REMOVED, CREATED.
    ///
    /// Notes:
    /// - COLOR alone does NOT trigger cluster re-binning
    ///   (clusters are spatial, not chromatic).
    /// - VOLUMETRIC alone does NOT trigger cluster re-binning
    ///   (volumetric fog changes don't move lights).
    #[must_use]
    pub const fn invalidates_direct_light_clusters(self) -> bool {
        let mask = Self::TRANSFORM.0
            | Self::INTENSITY.0
            | Self::RANGE.0
            | Self::SHAPE.0
            | Self::REMOVED.0
            | Self::CREATED.0;
        (self.0 & mask) != 0
    }

    /// Pass 2: returns `true` when ONLY volumetric flags
    /// are set (no other bits). Fog-only changes MUST NOT
    /// rebuild direct-light clusters.
    #[must_use]
    pub const fn touches_volumetric_only(self) -> bool {
        self.0 != 0 && (self.0 & !Self::VOLUMETRIC.0) == 0
    }

    /// Pass 2: returns `true` when ONLY color / intensity
    /// flags are set. Color-only / intensity-only mutations
    /// must not invalidate shadow maps OR re-bin clusters
    /// for transform. Cluster re-binning may still fire for
    /// INTENSITY (intensity range affects bin radius) — see
    /// [`Self::invalidates_direct_light_clusters`].
    #[must_use]
    pub const fn touches_color_or_intensity_only(self) -> bool {
        let mask = Self::COLOR.0 | Self::INTENSITY.0;
        self.0 != 0 && (self.0 & !mask) == 0
    }

    /// Pass 2: returns `true` for the typed mutation classes
    /// that invalidate the GI cache. The GI cache is
    /// chromatic + spatial, so almost every light mutation
    /// invalidates it EXCEPT VOLUMETRIC-only changes.
    #[must_use]
    pub const fn invalidates_gi_cache(self) -> bool {
        let mask = Self::TRANSFORM.0
            | Self::COLOR.0
            | Self::INTENSITY.0
            | Self::SHAPE.0
            | Self::RANGE.0
            | Self::STATIC_CACHE.0
            | Self::GI_CACHE.0
            | Self::REMOVED.0
            | Self::CREATED.0;
        (self.0 & mask) != 0
    }

    /// Pass 2: returns `true` for the typed mutation classes
    /// that invalidate the reflection / surface cache.
    #[must_use]
    pub const fn invalidates_reflection_cache(self) -> bool {
        let mask = Self::TRANSFORM.0
            | Self::COLOR.0
            | Self::INTENSITY.0
            | Self::SHAPE.0
            | Self::REFLECTION_CACHE.0
            | Self::STATIC_CACHE.0
            | Self::REMOVED.0
            | Self::CREATED.0;
        (self.0 & mask) != 0
    }

    /// Pass 2: returns `true` when the dirty flags drive a
    /// static-cache rebuild (e.g. baked light maps need a
    /// fresh bake).
    #[must_use]
    pub const fn drives_static_cache_rebuild(self) -> bool {
        let mask = Self::STATIC_CACHE.0 | Self::TRANSFORM.0 | Self::SHAPE.0 | Self::REMOVED.0;
        (self.0 & mask) != 0
    }

    /// Typed predicate: are these flags strictly empty —
    /// i.e. "no changes" — so the planner can skip every
    /// pass for the owning scene?
    #[must_use]
    pub const fn implies_no_update_work(self) -> bool {
        self.0 == 0
    }
}

// ============================================================================
// Typed dirty region (Pass 2 world-space)
// ============================================================================

/// Typed world-space dirty region. Replaces Pass 1's
/// pixel-rect `LuxDirtyRegion` with a richer typed record
/// that carries the affected world volume, the typed
/// invalidation flags, a priority, a cost estimate, and an
/// optional deadline frame the renderer must hit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuxDirtyRegion {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub bounds: LuxAabb,
    pub flags: LuxDirtyFlags,
    pub priority: u16,
    pub estimated_cost_micros: u32,
    pub deadline_frame: Option<u64>,
}

impl LuxDirtyRegion {
    /// Typed sentinel: a dirty region covering the entire
    /// scene's volume with `flags = ALL`. The renderer
    /// treats this as "no spatial pruning is possible."
    /// Useful for tests and for the initial-boot frame
    /// before the planner has narrowed any volumes.
    #[must_use]
    pub const fn whole_scene(scene_id: LuxSceneId, flags: LuxDirtyFlags) -> Self {
        Self {
            schema_version: FUN_LUX_DIRTY_SCHEMA_VERSION,
            scene_id,
            bounds: LuxAabb::WHOLE_WORLD,
            flags,
            priority: 128,
            estimated_cost_micros: 0,
            deadline_frame: None,
        }
    }

    /// Typed constructor for a specific dirty volume.
    #[must_use]
    pub const fn new(
        scene_id: LuxSceneId,
        bounds: LuxAabb,
        flags: LuxDirtyFlags,
        priority: u16,
        estimated_cost_micros: u32,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_DIRTY_SCHEMA_VERSION,
            scene_id,
            bounds,
            flags,
            priority,
            estimated_cost_micros,
            deadline_frame: None,
        }
    }

    /// Typed builder: attach a deadline frame the renderer
    /// must hit when processing this region.
    #[must_use]
    pub const fn with_deadline_frame(mut self, frame: u64) -> Self {
        self.deadline_frame = Some(frame);
        self
    }

    /// Typed predicate: does the region invalidate shadow
    /// maps? (See [`LuxDirtyFlags::invalidates_shadow_maps`].)
    #[must_use]
    pub const fn invalidates_shadow_maps(&self) -> bool {
        self.flags.invalidates_shadow_maps()
    }

    /// Typed predicate: does the region invalidate
    /// direct-light clusters?
    #[must_use]
    pub const fn invalidates_direct_light_clusters(&self) -> bool {
        self.flags.invalidates_direct_light_clusters()
    }

    /// Typed predicate: is this a fog-only region?
    #[must_use]
    pub const fn touches_volumetric_only(&self) -> bool {
        self.flags.touches_volumetric_only()
    }
}

// ============================================================================
// Typed dirty queue
// ============================================================================

/// Typed dirty region queue. Owned by `LuxSceneRegistry`;
/// the planner consumes the queue each frame and clears it
/// after emitting passes. Records the total estimated cost
/// so the planner can time-slice work when budgets shrink.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LuxDirtyQueue {
    pub schema_version: u16,
    pub regions: Vec<LuxDirtyRegion>,
    pub total_estimated_cost_micros: u64,
}

impl LuxDirtyQueue {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            schema_version: FUN_LUX_DIRTY_SCHEMA_VERSION,
            regions: Vec::new(),
            total_estimated_cost_micros: 0,
        }
    }

    pub fn push(&mut self, region: LuxDirtyRegion) {
        self.total_estimated_cost_micros = self
            .total_estimated_cost_micros
            .saturating_add(region.estimated_cost_micros as u64);
        self.regions.push(region);
    }

    pub fn clear(&mut self) {
        self.regions.clear();
        self.total_estimated_cost_micros = 0;
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.regions.len()
    }

    /// Typed predicate: does the queue carry any region
    /// whose flags satisfy the typed predicate?
    #[must_use]
    pub fn any_flag_predicate(&self, predicate: impl Fn(LuxDirtyFlags) -> bool) -> bool {
        self.regions.iter().any(|r| predicate(r.flags))
    }

    /// Typed predicate: does the queue invalidate any shadow
    /// map? (Used by the planner to decide whether to skip
    /// shadow passes entirely.)
    #[must_use]
    pub fn invalidates_any_shadow_map(&self) -> bool {
        self.any_flag_predicate(|f| f.invalidates_shadow_maps())
    }

    /// Typed predicate: does the queue invalidate any
    /// direct-light cluster bin?
    #[must_use]
    pub fn invalidates_any_cluster(&self) -> bool {
        self.any_flag_predicate(|f| f.invalidates_direct_light_clusters())
    }

    /// Typed predicate: do every region's flags touch only
    /// volumetric state? (Used by the planner to decide
    /// whether to skip cluster + shadow + GI work for a
    /// fog-only frame.)
    #[must_use]
    pub fn touches_volumetric_only(&self) -> bool {
        !self.regions.is_empty() && self.regions.iter().all(|r| r.touches_volumetric_only())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_plan::LuxSceneId;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_DIRTY_SCHEMA_VERSION, 1);
        assert_eq!(LUX_DIRTY_FLAG_BIT_COUNT, 12);
    }

    #[test]
    fn dirty_flags_bit_constants_are_distinct() {
        let constants = [
            LuxDirtyFlags::TRANSFORM,
            LuxDirtyFlags::COLOR,
            LuxDirtyFlags::INTENSITY,
            LuxDirtyFlags::SHAPE,
            LuxDirtyFlags::RANGE,
            LuxDirtyFlags::SHADOW_POLICY,
            LuxDirtyFlags::VOLUMETRIC,
            LuxDirtyFlags::STATIC_CACHE,
            LuxDirtyFlags::GI_CACHE,
            LuxDirtyFlags::REFLECTION_CACHE,
            LuxDirtyFlags::REMOVED,
            LuxDirtyFlags::CREATED,
        ];
        assert_eq!(constants.len(), LUX_DIRTY_FLAG_BIT_COUNT);
        let mut seen = hashbrown::HashSet::new();
        for c in constants {
            assert!(seen.insert(c.0));
        }
        // ALL contains every constant.
        for c in constants {
            assert!(LuxDirtyFlags::ALL.contains(c));
        }
    }

    #[test]
    fn dirty_flags_insert_remove_with_without() {
        let mut f = LuxDirtyFlags::NONE;
        f.insert(LuxDirtyFlags::TRANSFORM);
        assert!(f.contains(LuxDirtyFlags::TRANSFORM));
        f.insert(LuxDirtyFlags::COLOR);
        assert!(f.contains(LuxDirtyFlags::COLOR));
        f.remove(LuxDirtyFlags::TRANSFORM);
        assert!(!f.contains(LuxDirtyFlags::TRANSFORM));
        let combined = LuxDirtyFlags::TRANSFORM.with(LuxDirtyFlags::COLOR);
        assert!(combined.contains(LuxDirtyFlags::TRANSFORM));
        let dropped = combined.without(LuxDirtyFlags::COLOR);
        assert!(!dropped.contains(LuxDirtyFlags::COLOR));
    }

    /// Pass 2 acceptance: light color/intensity changes do
    /// NOT invalidate shadow maps.
    #[test]
    fn color_or_intensity_only_does_not_invalidate_shadow_maps() {
        assert!(!LuxDirtyFlags::COLOR.invalidates_shadow_maps());
        assert!(!LuxDirtyFlags::INTENSITY.invalidates_shadow_maps());
        let combined = LuxDirtyFlags::COLOR.with(LuxDirtyFlags::INTENSITY);
        assert!(!combined.invalidates_shadow_maps());
        // But adding TRANSFORM, SHAPE, SHADOW_POLICY, or REMOVED does.
        for f in [
            LuxDirtyFlags::TRANSFORM,
            LuxDirtyFlags::SHAPE,
            LuxDirtyFlags::SHADOW_POLICY,
            LuxDirtyFlags::REMOVED,
        ] {
            assert!(
                f.invalidates_shadow_maps(),
                "{:?} must invalidate shadow maps",
                f,
            );
        }
    }

    /// Pass 2 acceptance: light transform changes drive
    /// cluster re-binning. Color-only / intensity-only is
    /// nuanced — see the docstring on
    /// `invalidates_direct_light_clusters`.
    #[test]
    fn transform_invalidates_direct_light_clusters() {
        assert!(LuxDirtyFlags::TRANSFORM.invalidates_direct_light_clusters());
        // COLOR alone does NOT trigger cluster re-binning.
        assert!(!LuxDirtyFlags::COLOR.invalidates_direct_light_clusters());
        // INTENSITY does (because intensity range affects
        // bin radius).
        assert!(LuxDirtyFlags::INTENSITY.invalidates_direct_light_clusters());
        // VOLUMETRIC alone does NOT.
        assert!(!LuxDirtyFlags::VOLUMETRIC.invalidates_direct_light_clusters());
    }

    /// Pass 2 acceptance: fog-only changes do not rebuild
    /// direct-light clusters.
    #[test]
    fn volumetric_only_does_not_rebuild_clusters() {
        let f = LuxDirtyFlags::VOLUMETRIC;
        assert!(f.touches_volumetric_only());
        assert!(!f.invalidates_direct_light_clusters());
        assert!(!f.invalidates_shadow_maps());
        // Adding any non-volumetric flag flips
        // touches_volumetric_only to false.
        let mixed = LuxDirtyFlags::VOLUMETRIC.with(LuxDirtyFlags::TRANSFORM);
        assert!(!mixed.touches_volumetric_only());
    }

    #[test]
    fn color_or_intensity_only_predicate() {
        assert!(LuxDirtyFlags::COLOR.touches_color_or_intensity_only());
        assert!(LuxDirtyFlags::INTENSITY.touches_color_or_intensity_only());
        let mixed = LuxDirtyFlags::COLOR.with(LuxDirtyFlags::INTENSITY);
        assert!(mixed.touches_color_or_intensity_only());
        // Adding TRANSFORM flips it to false.
        assert!(
            !LuxDirtyFlags::COLOR
                .with(LuxDirtyFlags::TRANSFORM)
                .touches_color_or_intensity_only()
        );
        // NONE is not "color or intensity only" — it's just empty.
        assert!(!LuxDirtyFlags::NONE.touches_color_or_intensity_only());
    }

    #[test]
    fn gi_cache_invalidation_excludes_volumetric_only() {
        assert!(!LuxDirtyFlags::VOLUMETRIC.invalidates_gi_cache());
        assert!(LuxDirtyFlags::TRANSFORM.invalidates_gi_cache());
        assert!(LuxDirtyFlags::COLOR.invalidates_gi_cache());
    }

    #[test]
    fn drives_static_cache_rebuild_predicate() {
        assert!(LuxDirtyFlags::STATIC_CACHE.drives_static_cache_rebuild());
        assert!(LuxDirtyFlags::TRANSFORM.drives_static_cache_rebuild());
        assert!(LuxDirtyFlags::SHAPE.drives_static_cache_rebuild());
        assert!(LuxDirtyFlags::REMOVED.drives_static_cache_rebuild());
        // COLOR alone does not (static caches are spatial).
        assert!(!LuxDirtyFlags::COLOR.drives_static_cache_rebuild());
    }

    #[test]
    fn implies_no_update_work_predicate() {
        assert!(LuxDirtyFlags::NONE.implies_no_update_work());
        assert!(!LuxDirtyFlags::TRANSFORM.implies_no_update_work());
    }

    #[test]
    fn dirty_region_whole_scene_sentinel() {
        let r = LuxDirtyRegion::whole_scene(
            LuxSceneId::PROOF_SCENE,
            LuxDirtyFlags::TRANSFORM.with(LuxDirtyFlags::SHADOW_POLICY),
        );
        assert!(r.bounds.is_whole_world());
        assert!(r.flags.contains(LuxDirtyFlags::TRANSFORM));
        assert!(r.flags.contains(LuxDirtyFlags::SHADOW_POLICY));
        assert!(r.invalidates_shadow_maps());
        assert!(r.invalidates_direct_light_clusters());
        assert!(!r.touches_volumetric_only());
    }

    #[test]
    fn dirty_region_with_deadline_frame() {
        let r = LuxDirtyRegion::new(
            LuxSceneId::PROOF_SCENE,
            LuxAabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
            LuxDirtyFlags::TRANSFORM,
            64,
            42,
        )
        .with_deadline_frame(123);
        assert_eq!(r.deadline_frame, Some(123));
        assert_eq!(r.priority, 64);
        assert_eq!(r.estimated_cost_micros, 42);
    }

    #[test]
    fn dirty_queue_push_clear_len_predicates() {
        let mut q = LuxDirtyQueue::empty();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
        q.push(LuxDirtyRegion::new(
            LuxSceneId::PROOF_SCENE,
            LuxAabb::WHOLE_WORLD,
            LuxDirtyFlags::TRANSFORM,
            128,
            42,
        ));
        assert!(!q.is_empty());
        assert_eq!(q.len(), 1);
        assert_eq!(q.total_estimated_cost_micros, 42);
        q.clear();
        assert!(q.is_empty());
        assert_eq!(q.total_estimated_cost_micros, 0);
    }

    #[test]
    fn dirty_queue_invalidates_predicates_walk_regions() {
        let mut q = LuxDirtyQueue::empty();
        q.push(LuxDirtyRegion::new(
            LuxSceneId::PROOF_SCENE,
            LuxAabb::WHOLE_WORLD,
            LuxDirtyFlags::COLOR,
            128,
            10,
        ));
        // Color-only queue does not invalidate shadows.
        assert!(!q.invalidates_any_shadow_map());
        // Adding a TRANSFORM region flips it.
        q.push(LuxDirtyRegion::new(
            LuxSceneId::PROOF_SCENE,
            LuxAabb::WHOLE_WORLD,
            LuxDirtyFlags::TRANSFORM,
            128,
            20,
        ));
        assert!(q.invalidates_any_shadow_map());
        assert!(q.invalidates_any_cluster());
        // Not volumetric-only (mixed flags).
        assert!(!q.touches_volumetric_only());
    }

    #[test]
    fn dirty_queue_touches_volumetric_only_when_every_region_is_volumetric() {
        let mut q = LuxDirtyQueue::empty();
        q.push(LuxDirtyRegion::new(
            LuxSceneId::PROOF_SCENE,
            LuxAabb::WHOLE_WORLD,
            LuxDirtyFlags::VOLUMETRIC,
            128,
            5,
        ));
        q.push(LuxDirtyRegion::new(
            LuxSceneId::PROOF_SCENE,
            LuxAabb::WHOLE_WORLD,
            LuxDirtyFlags::VOLUMETRIC,
            128,
            5,
        ));
        assert!(q.touches_volumetric_only());
        assert!(!q.invalidates_any_shadow_map());
        assert!(!q.invalidates_any_cluster());
        // Empty queue is NOT volumetric-only — it's "no work."
        let empty = LuxDirtyQueue::empty();
        assert!(!empty.touches_volumetric_only());
    }
}
