//! Typed Pass 2 partial-update lighting state.
//!
//! This module ships the typed surfaces the user's "Pass 2:
//! Convert `LuxWorld` from counters to real partial-update
//! state" rule asked for. The renderer reads these typed
//! records each frame and only updates the dirty subset of
//! the lighting state — light transforms only re-bin affected
//! clusters, color changes leave shadow maps alone, fog-only
//! changes skip direct-light passes, and static scenes emit
//! no work.
//!
//! The existing `lib.rs` `LuxWorld` counter scaffolding stays
//! for backward compatibility; the new typed records below
//! are owned alongside (added as fields on the extended
//! `LuxWorld` and as standalone typed surfaces the `runtime`
//! planner consumes).

use crate::aabb::{LuxAabb, LuxTableRange};
use crate::dirty::{LuxDirtyFlags, LuxDirtyQueue, LuxDirtyRegion};
use crate::frame_plan::{LuxSceneId, LuxScenePriority};

pub const FUN_LUX_WORLD_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — Scene registry
// ============================================================================

/// Typed per-scene record. The renderer reads the record to
/// know whether a scene is visible, what range of dense
/// `LuxLightDatabase::records` belongs to it, and what
/// per-subsystem revisions have advanced. The planner reads
/// the typed dirty queue + per-subsystem revisions to decide
/// what work to emit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuxSceneRecord {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub revision: u64,
    pub bounds_world: LuxAabb,
    pub visible: bool,
    pub priority: LuxScenePriority,
    pub light_range: LuxTableRange,
    pub static_cache_revision: u64,
    pub dynamic_revision: u64,
    pub volumetric_revision: u64,
}

impl LuxSceneRecord {
    #[must_use]
    pub fn new(scene_id: LuxSceneId) -> Self {
        Self {
            schema_version: FUN_LUX_WORLD_SCHEMA_VERSION,
            scene_id,
            revision: 0,
            bounds_world: LuxAabb::ZERO,
            visible: true,
            priority: LuxScenePriority::default(),
            light_range: LuxTableRange::EMPTY,
            static_cache_revision: 0,
            dynamic_revision: 0,
            volumetric_revision: 0,
        }
    }

    /// Typed predicate: is this scene the typed cold-default
    /// (no lights, never advanced)?
    #[must_use]
    pub const fn is_quiescent(&self) -> bool {
        self.light_range.is_empty()
            && self.dynamic_revision == 0
            && self.volumetric_revision == 0
            && self.static_cache_revision == 0
    }
}

/// Typed scene registry. Owns the active scene list, the
/// per-scene records, and the typed dirty queue. The planner
/// walks the registry each frame.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LuxSceneRegistry {
    pub schema_version: u16,
    pub active_scenes: Vec<LuxSceneId>,
    pub records: Vec<LuxSceneRecord>,
    pub dirty_queue: LuxDirtyQueue,
}

impl LuxSceneRegistry {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: FUN_LUX_WORLD_SCHEMA_VERSION,
            active_scenes: Vec::new(),
            records: Vec::new(),
            dirty_queue: LuxDirtyQueue::empty(),
        }
    }

    /// Typed registration: add a scene to the active list +
    /// register a default record. Returns the typed index
    /// the record occupies.
    pub fn register_scene(&mut self, scene_id: LuxSceneId) -> u32 {
        if let Some(i) = self.active_scenes.iter().position(|s| *s == scene_id) {
            return i as u32;
        }
        let index = self.active_scenes.len() as u32;
        self.active_scenes.push(scene_id);
        self.records.push(LuxSceneRecord::new(scene_id));
        index
    }

    /// Typed lookup by scene id.
    #[must_use]
    pub fn record_for(&self, scene_id: LuxSceneId) -> Option<&LuxSceneRecord> {
        self.records.iter().find(|r| r.scene_id == scene_id)
    }

    /// Typed mutable lookup by scene id.
    #[must_use]
    pub fn record_for_mut(&mut self, scene_id: LuxSceneId) -> Option<&mut LuxSceneRecord> {
        self.records.iter_mut().find(|r| r.scene_id == scene_id)
    }

    /// Typed predicate: do any scenes have non-empty dirty
    /// queues? Used by the planner to decide whether the
    /// whole frame is a no-op.
    #[must_use]
    pub fn any_scene_dirty(&self) -> bool {
        !self.dirty_queue.is_empty()
    }

    /// Pass 2 acceptance: typed predicate recording whether
    /// every visible scene is quiescent. Used by the planner
    /// to short-circuit the entire frame plan when no
    /// changes occurred.
    #[must_use]
    pub fn every_visible_scene_is_quiescent(&self) -> bool {
        self.records
            .iter()
            .filter(|r| r.visible)
            .all(|r| r.is_quiescent())
            && self.dirty_queue.is_empty()
    }
}

// ============================================================================
// Section 2 — Dense light database
// ============================================================================

/// Typed dense per-light record. Stored in
/// `LuxLightDatabase::records` indexed by light index. The
/// renderer reads the typed dirty_flags to know which fields
/// need re-upload to the GPU light buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuxLightRecord {
    pub schema_version: u16,
    pub stable_light_key: u64,
    pub generation: u32,
    pub position: [f32; 3],
    pub color_rgb: [f32; 3],
    pub intensity_lux: f32,
    pub range: f32,
    pub casts_shadow: bool,
    pub emissive_candidate: bool,
    pub dirty_flags: LuxDirtyFlags,
}

impl LuxLightRecord {
    #[must_use]
    pub const fn new_created(
        stable_light_key: u64,
        generation: u32,
        position: [f32; 3],
        color_rgb: [f32; 3],
        intensity_lux: f32,
        range: f32,
        casts_shadow: bool,
        emissive_candidate: bool,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_WORLD_SCHEMA_VERSION,
            stable_light_key,
            generation,
            position,
            color_rgb,
            intensity_lux,
            range,
            casts_shadow,
            emissive_candidate,
            dirty_flags: LuxDirtyFlags::CREATED,
        }
    }

    /// Typed transition: mark this light's transform dirty
    /// after a position move.
    pub fn mark_transform_dirty(&mut self, new_position: [f32; 3]) {
        self.position = new_position;
        self.dirty_flags.insert(LuxDirtyFlags::TRANSFORM);
    }

    /// Typed transition: mark this light's color dirty
    /// after a color tweak. Pass 2 acceptance: this does NOT
    /// set TRANSFORM, so shadow maps stay valid.
    pub fn mark_color_dirty(&mut self, new_color_rgb: [f32; 3]) {
        self.color_rgb = new_color_rgb;
        self.dirty_flags.insert(LuxDirtyFlags::COLOR);
    }

    /// Typed transition: mark intensity dirty.
    pub fn mark_intensity_dirty(&mut self, new_intensity_lux: f32) {
        self.intensity_lux = new_intensity_lux;
        self.dirty_flags.insert(LuxDirtyFlags::INTENSITY);
    }

    /// Typed transition: mark removed.
    pub fn mark_removed(&mut self) {
        self.dirty_flags.insert(LuxDirtyFlags::REMOVED);
    }

    /// Typed acknowledge: clear all dirty flags after the
    /// renderer has consumed them.
    pub fn clear_dirty_flags(&mut self) {
        self.dirty_flags = LuxDirtyFlags::NONE;
        self.generation = self.generation.saturating_add(1);
    }

    /// Typed predicate: is this light's dirty state quiescent?
    #[must_use]
    pub const fn is_quiescent(&self) -> bool {
        self.dirty_flags.is_empty()
    }
}

// ============================================================================
// Section 3 — Typed shadow / GI / volumetric records
// ============================================================================

/// Typed shadow invalidation request. Emitted when a shadow
/// caster moves or a light's shadow policy changes. The
/// renderer reads the typed flags + affected_bounds to
/// invalidate only the affected shadow pages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuxShadowRequest {
    pub schema_version: u16,
    pub light_index: u32,
    pub affected_bounds: LuxAabb,
    pub flags: LuxDirtyFlags,
    pub frame_deadline: Option<u64>,
}

impl LuxShadowRequest {
    #[must_use]
    pub const fn new(light_index: u32, affected_bounds: LuxAabb, flags: LuxDirtyFlags) -> Self {
        Self {
            schema_version: FUN_LUX_WORLD_SCHEMA_VERSION,
            light_index,
            affected_bounds,
            flags,
            frame_deadline: None,
        }
    }

    #[must_use]
    pub const fn with_deadline_frame(mut self, frame: u64) -> Self {
        self.frame_deadline = Some(frame);
        self
    }
}

/// Typed surface-cache update request. Emitted when the GI
/// surface cache needs to refresh a bounded volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceCacheUpdateRequest {
    pub schema_version: u16,
    pub bounds: LuxAabb,
    pub flags: LuxDirtyFlags,
}

impl SurfaceCacheUpdateRequest {
    #[must_use]
    pub const fn new(bounds: LuxAabb, flags: LuxDirtyFlags) -> Self {
        Self {
            schema_version: FUN_LUX_WORLD_SCHEMA_VERSION,
            bounds,
            flags,
        }
    }
}

/// Typed indirect-scene-change event. Emitted when the GI
/// cache must respond to a scene-level mutation (e.g.
/// dynamic geometry teleported into a probe).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IndirectSceneChange {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub bounds: LuxAabb,
    pub flags: LuxDirtyFlags,
}

impl IndirectSceneChange {
    #[must_use]
    pub const fn new(scene_id: LuxSceneId, bounds: LuxAabb, flags: LuxDirtyFlags) -> Self {
        Self {
            schema_version: FUN_LUX_WORLD_SCHEMA_VERSION,
            scene_id,
            bounds,
            flags,
        }
    }
}

// ============================================================================
// Section 4 — Volumetric world
// ============================================================================

/// Typed volumetric partial-update state. Tracks per-froxel
/// dirty indices + bounded dirty volumes the renderer reads
/// to update only the affected froxels.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LuxVolumetricWorld {
    pub schema_version: u16,
    pub fog_revision: u64,
    pub light_inject_revision: u64,
    pub dirty_froxel_indices: Vec<u32>,
    pub dirty_volumes: Vec<LuxAabb>,
}

impl LuxVolumetricWorld {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            schema_version: FUN_LUX_WORLD_SCHEMA_VERSION,
            fog_revision: 0,
            light_inject_revision: 0,
            dirty_froxel_indices: Vec::new(),
            dirty_volumes: Vec::new(),
        }
    }

    /// Typed mutation: bump the fog revision + record a
    /// dirty volume.
    pub fn mark_fog_dirty(&mut self, bounds: LuxAabb) {
        self.fog_revision = self.fog_revision.saturating_add(1);
        self.dirty_volumes.push(bounds);
    }

    /// Typed mutation: bump the light-inject revision (used
    /// when a light moves and its volumetric contribution
    /// needs to refresh).
    pub fn mark_light_inject_dirty(&mut self) {
        self.light_inject_revision = self.light_inject_revision.saturating_add(1);
    }

    pub fn clear(&mut self) {
        self.dirty_froxel_indices.clear();
        self.dirty_volumes.clear();
    }

    #[must_use]
    pub const fn is_quiescent(&self) -> bool {
        self.dirty_froxel_indices.is_empty()
            && self.fog_revision == 0
            && self.light_inject_revision == 0
    }

    #[must_use]
    pub fn has_dirty_volumes(&self) -> bool {
        !self.dirty_volumes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_plan::LuxSceneId;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_WORLD_SCHEMA_VERSION, 1);
    }

    #[test]
    fn scene_record_default_is_quiescent() {
        let r = LuxSceneRecord::new(LuxSceneId::PROOF_SCENE);
        assert!(r.is_quiescent());
        assert!(r.visible);
        assert_eq!(r.revision, 0);
    }

    #[test]
    fn scene_registry_register_returns_index() {
        let mut reg = LuxSceneRegistry::empty();
        let i0 = reg.register_scene(LuxSceneId::PROOF_SCENE);
        let i1 = reg.register_scene(LuxSceneId::PROOF_SCENE);
        assert_eq!(i0, 0);
        assert_eq!(i1, 0); // re-register returns the same index
        let i2 = reg.register_scene(LuxSceneId(7));
        assert_eq!(i2, 1);
        assert_eq!(reg.active_scenes.len(), 2);
        assert_eq!(reg.records.len(), 2);
    }

    #[test]
    fn scene_registry_quiescent_predicate() {
        let mut reg = LuxSceneRegistry::empty();
        reg.register_scene(LuxSceneId::PROOF_SCENE);
        // Empty queue + quiescent record → every visible
        // scene is quiescent.
        assert!(reg.every_visible_scene_is_quiescent());
        // Push a dirty region → no longer quiescent.
        reg.dirty_queue.push(LuxDirtyRegion::whole_scene(
            LuxSceneId::PROOF_SCENE,
            LuxDirtyFlags::TRANSFORM,
        ));
        assert!(!reg.every_visible_scene_is_quiescent());
    }

    #[test]
    fn light_record_transition_set_dirty_flags() {
        let mut light = LuxLightRecord::new_created(
            42,
            1,
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            1000.0,
            10.0,
            true,
            false,
        );
        assert!(light.dirty_flags.contains(LuxDirtyFlags::CREATED));
        light.clear_dirty_flags();
        assert!(light.is_quiescent());

        // Transform mutation sets TRANSFORM only.
        light.mark_transform_dirty([1.0, 2.0, 3.0]);
        assert!(light.dirty_flags.contains(LuxDirtyFlags::TRANSFORM));
        assert!(!light.dirty_flags.contains(LuxDirtyFlags::COLOR));
        // Color mutation adds COLOR (TRANSFORM still set
        // until cleared).
        light.mark_color_dirty([0.5, 0.5, 0.5]);
        assert!(light.dirty_flags.contains(LuxDirtyFlags::COLOR));
        assert!(light.dirty_flags.contains(LuxDirtyFlags::TRANSFORM));
        // Clear acknowledges + bumps generation.
        let g = light.generation;
        light.clear_dirty_flags();
        assert!(light.is_quiescent());
        assert_eq!(light.generation, g + 1);
    }

    /// Pass 2 acceptance: a light record whose only mutation
    /// was a color change carries COLOR (and nothing else
    /// that would invalidate shadows).
    #[test]
    fn light_record_color_only_does_not_invalidate_shadows() {
        let mut light =
            LuxLightRecord::new_created(1, 1, [0.0; 3], [1.0; 3], 500.0, 5.0, true, false);
        light.clear_dirty_flags();
        light.mark_color_dirty([0.0, 1.0, 0.0]);
        assert!(!light.dirty_flags.invalidates_shadow_maps());
        // But it still invalidates GI cache + reflection
        // cache (chromatic effects).
        assert!(light.dirty_flags.invalidates_gi_cache());
        assert!(light.dirty_flags.invalidates_reflection_cache());
    }

    #[test]
    fn shadow_request_builds_with_deadline() {
        let req = LuxShadowRequest::new(
            7,
            LuxAabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
            LuxDirtyFlags::TRANSFORM.with(LuxDirtyFlags::SHADOW_POLICY),
        )
        .with_deadline_frame(123);
        assert_eq!(req.light_index, 7);
        assert_eq!(req.frame_deadline, Some(123));
        assert!(req.flags.invalidates_shadow_maps());
    }

    #[test]
    fn surface_cache_update_request_carries_flags_and_bounds() {
        let req = SurfaceCacheUpdateRequest::new(LuxAabb::WHOLE_WORLD, LuxDirtyFlags::TRANSFORM);
        assert!(req.bounds.is_whole_world());
        assert!(req.flags.contains(LuxDirtyFlags::TRANSFORM));
    }

    #[test]
    fn indirect_scene_change_carries_scene_id() {
        let change = IndirectSceneChange::new(
            LuxSceneId(42),
            LuxAabb::WHOLE_WORLD,
            LuxDirtyFlags::TRANSFORM,
        );
        assert_eq!(change.scene_id, LuxSceneId(42));
    }

    #[test]
    fn volumetric_world_marks_fog_dirty_and_clears() {
        let mut v = LuxVolumetricWorld::empty();
        assert!(v.is_quiescent());
        v.mark_fog_dirty(LuxAabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]));
        assert!(!v.is_quiescent());
        assert!(v.has_dirty_volumes());
        v.clear();
        // Revision stays bumped (so the renderer sees the
        // typed history); dirty_volumes is cleared.
        assert!(!v.has_dirty_volumes());
        assert!(!v.is_quiescent()); // revision != 0
    }
}
