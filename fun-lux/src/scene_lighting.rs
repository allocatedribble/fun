//! Pass 4 — typed multi-scene scene-lighting model.
//!
//! Encodes the user's "4.1 Scene lighting data" + "4.4
//! Multi-scene support" rules:
//!
//! - `FunLuxScene` — a typed lighting island with stable id,
//!   bounds, light list, four typed caches (static / dynamic
//!   / volumetric / shadow), and a typed dirty region list.
//! - `FunLuxSceneLightingData` — typed per-scene index lists
//!   (directional / point / spot / area / emissive /
//!   reflection probe / irradiance probe / fog volume /
//!   local volumetric / shadow caster) so the renderer can
//!   iterate one kind at a time.
//! - `FunLuxWorldLighting` — the typed top-level world record
//!   carrying every scene + the typed scheduling table.
//! - `FunLuxSceneKind` — typed canonical scene categories
//!   (main world / interior / UI preview / portal additive /
//!   cutscene / editor preview).
//! - `FunLuxSceneSchedule` + `FunLuxSceneUpdateCadence` +
//!   `FunLuxSceneVisibility` — typed scheduling state per
//!   scene, with typed predicates encoding "update visible
//!   scenes every frame" / "skip fully hidden scenes" /
//!   "partial-update dirty scenes" / "preserve static cached
//!   resources until invalidated."

use crate::aabb::LuxAabb;
use crate::dirty::{LuxDirtyFlags, LuxDirtyRegion};
use crate::frame_plan::LuxSceneId;

pub const FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_SCENE_KIND_COUNT: usize = 6;
pub const FUN_LUX_SCENE_VISIBILITY_COUNT: usize = 3;
pub const FUN_LUX_SCENE_UPDATE_CADENCE_COUNT: usize = 4;

// ============================================================================
// Section 1 — Typed scene kind (4.4 canonical scene categories)
// ============================================================================

/// Typed canonical scene kinds. Each kind drives a typed
/// default visibility + scheduling cadence.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxSceneKind {
    #[default]
    MainWorld,
    Interior,
    UiWorldPreview,
    PortalAdditive,
    Cutscene,
    EditorPreview,
}

impl FunLuxSceneKind {
    pub const ALL: [Self; FUN_LUX_SCENE_KIND_COUNT] = [
        Self::MainWorld,
        Self::Interior,
        Self::UiWorldPreview,
        Self::PortalAdditive,
        Self::Cutscene,
        Self::EditorPreview,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MainWorld => "main_world",
            Self::Interior => "interior",
            Self::UiWorldPreview => "ui_world_preview",
            Self::PortalAdditive => "portal_additive",
            Self::Cutscene => "cutscene",
            Self::EditorPreview => "editor_preview",
        }
    }

    /// Typed predicate: is this scene the production-default
    /// "always visible" main world?
    #[must_use]
    pub const fn is_main_world(self) -> bool {
        matches!(self, Self::MainWorld)
    }

    /// Typed predicate: is this scene an editor / tooling
    /// scene that should be skipped under product builds?
    #[must_use]
    pub const fn is_editor_only(self) -> bool {
        matches!(self, Self::EditorPreview)
    }
}

// ============================================================================
// Section 2 — Typed scene cache (4.4 cache states)
// ============================================================================

/// Typed cache state for one of a scene's four lighting
/// caches (static / dynamic / volumetric / shadow). Each
/// cache tracks its typed revision + byte size + validity
/// flag so the renderer can skip rebuilds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxSceneCache {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub revision: u64,
    pub byte_size: u64,
    pub valid: bool,
}

impl FunLuxSceneCache {
    #[must_use]
    pub const fn empty(stable_id: &'static str) -> Self {
        Self {
            schema_version: FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION,
            stable_id,
            revision: 0,
            byte_size: 0,
            valid: false,
        }
    }

    /// Typed mutation: bump the revision + mark valid.
    pub fn mark_rebuilt(&mut self, byte_size: u64) {
        self.revision = self.revision.saturating_add(1);
        self.byte_size = byte_size;
        self.valid = true;
    }

    /// Typed mutation: invalidate the cache (drives a
    /// rebuild next frame).
    pub fn invalidate(&mut self) {
        self.valid = false;
    }

    /// Pass 4 acceptance: typed predicate — "preserve static
    /// cached resources until invalidated." Returns `true`
    /// when the cache is valid and the renderer can read
    /// the cached resource without rebuilding.
    #[must_use]
    pub const fn can_preserve_without_rebuild(self) -> bool {
        self.valid
    }
}

// ============================================================================
// Section 3 — Typed scene lighting data (4.1 stored kinds)
// ============================================================================

/// Typed per-kind index lists. Each list carries indices
/// into the typed dense `LuxLightDatabase::records` table so
/// the renderer can iterate one kind at a time without
/// re-walking the dense table.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunLuxSceneLightingData {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub directional_light_indices: Vec<u32>,
    pub point_light_indices: Vec<u32>,
    pub spot_light_indices: Vec<u32>,
    pub area_light_indices: Vec<u32>,
    pub emissive_renderer_indices: Vec<u32>,
    pub reflection_probe_indices: Vec<u32>,
    pub irradiance_probe_indices: Vec<u32>,
    pub fog_volume_indices: Vec<u32>,
    pub local_volumetric_volume_indices: Vec<u32>,
    pub shadow_caster_indices: Vec<u32>,
    pub static_lighting_data_revision: u64,
    pub per_scene_dirty_flags: LuxDirtyFlags,
}

impl FunLuxSceneLightingData {
    #[must_use]
    pub fn empty(scene_id: LuxSceneId) -> Self {
        Self {
            schema_version: FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION,
            scene_id,
            directional_light_indices: Vec::new(),
            point_light_indices: Vec::new(),
            spot_light_indices: Vec::new(),
            area_light_indices: Vec::new(),
            emissive_renderer_indices: Vec::new(),
            reflection_probe_indices: Vec::new(),
            irradiance_probe_indices: Vec::new(),
            fog_volume_indices: Vec::new(),
            local_volumetric_volume_indices: Vec::new(),
            shadow_caster_indices: Vec::new(),
            static_lighting_data_revision: 0,
            per_scene_dirty_flags: LuxDirtyFlags::NONE,
        }
    }

    /// Typed total count across every per-kind list.
    #[must_use]
    pub fn total_record_count(&self) -> u32 {
        let counts = [
            self.directional_light_indices.len(),
            self.point_light_indices.len(),
            self.spot_light_indices.len(),
            self.area_light_indices.len(),
            self.emissive_renderer_indices.len(),
            self.reflection_probe_indices.len(),
            self.irradiance_probe_indices.len(),
            self.fog_volume_indices.len(),
            self.local_volumetric_volume_indices.len(),
            self.shadow_caster_indices.len(),
        ];
        counts.iter().map(|c| *c as u32).sum()
    }

    /// Typed predicate: is the scene's lighting data
    /// quiescent (no lights of any kind + no dirty flags)?
    #[must_use]
    pub fn is_quiescent(&self) -> bool {
        self.total_record_count() == 0 && self.per_scene_dirty_flags.is_empty()
    }
}

// ============================================================================
// Section 4 — Typed scene scheduling (4.4 scheduling)
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxSceneVisibility {
    #[default]
    FullyVisible,
    PartiallyVisible,
    FullyHidden,
}

impl FunLuxSceneVisibility {
    pub const ALL: [Self; FUN_LUX_SCENE_VISIBILITY_COUNT] = [
        Self::FullyVisible,
        Self::PartiallyVisible,
        Self::FullyHidden,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullyVisible => "fully_visible",
            Self::PartiallyVisible => "partially_visible",
            Self::FullyHidden => "fully_hidden",
        }
    }

    /// Pass 4 acceptance: typed predicate — "skip fully
    /// hidden scenes."
    #[must_use]
    pub const fn should_skip_this_frame(self) -> bool {
        matches!(self, Self::FullyHidden)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxSceneUpdateCadence {
    #[default]
    EveryFrame,
    EveryNFrames(u32),
    OnChangeOnly,
    Skipped,
}

impl FunLuxSceneUpdateCadence {
    pub const ALL_NAMES: [&'static str; FUN_LUX_SCENE_UPDATE_CADENCE_COUNT] =
        ["every_frame", "every_n_frames", "on_change_only", "skipped"];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EveryFrame => "every_frame",
            Self::EveryNFrames(_) => "every_n_frames",
            Self::OnChangeOnly => "on_change_only",
            Self::Skipped => "skipped",
        }
    }

    /// Typed predicate: should this cadence trigger an
    /// update on the given frame?
    #[must_use]
    pub const fn triggers(self, frame_index: u64, dirty: bool) -> bool {
        match self {
            Self::EveryFrame => true,
            Self::EveryNFrames(n) => n > 0 && frame_index.is_multiple_of(n as u64),
            Self::OnChangeOnly => dirty,
            Self::Skipped => false,
        }
    }
}

/// Typed per-scene schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxSceneSchedule {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub visibility: FunLuxSceneVisibility,
    pub priority: u16,
    pub update_cadence: FunLuxSceneUpdateCadence,
}

impl FunLuxSceneSchedule {
    /// Typed product-default for a fully visible scene:
    /// update every frame at default priority.
    #[must_use]
    pub const fn for_visible_scene(scene_id: LuxSceneId) -> Self {
        Self {
            schema_version: FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION,
            scene_id,
            visibility: FunLuxSceneVisibility::FullyVisible,
            priority: 128,
            update_cadence: FunLuxSceneUpdateCadence::EveryFrame,
        }
    }

    /// Typed product-default for a hidden scene: never
    /// update, lowest priority.
    #[must_use]
    pub const fn for_hidden_scene(scene_id: LuxSceneId) -> Self {
        Self {
            schema_version: FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION,
            scene_id,
            visibility: FunLuxSceneVisibility::FullyHidden,
            priority: 0,
            update_cadence: FunLuxSceneUpdateCadence::Skipped,
        }
    }

    /// Typed product-default for a low-priority scene:
    /// update over multiple frames (cadence
    /// `EveryNFrames(4)`).
    #[must_use]
    pub const fn for_low_priority_scene(scene_id: LuxSceneId) -> Self {
        Self {
            schema_version: FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION,
            scene_id,
            visibility: FunLuxSceneVisibility::PartiallyVisible,
            priority: 64,
            update_cadence: FunLuxSceneUpdateCadence::EveryNFrames(4),
        }
    }

    /// Pass 4 acceptance: typed predicate — "skip fully
    /// hidden scenes."
    #[must_use]
    pub const fn should_skip(self) -> bool {
        self.visibility.should_skip_this_frame()
    }

    /// Pass 4 acceptance: typed predicate — does the schedule
    /// trigger an update on `frame_index` given the scene's
    /// dirty state?
    #[must_use]
    pub const fn triggers_update(self, frame_index: u64, dirty: bool) -> bool {
        if self.should_skip() {
            return false;
        }
        self.update_cadence.triggers(frame_index, dirty)
    }
}

// ============================================================================
// Section 5 — Typed scene (4.1 + 4.4)
// ============================================================================

/// Typed lighting island — one logical scene with its own
/// stable id, world-space bounds, light list, four typed
/// caches, dirty region list, and lighting data table.
#[derive(Debug, Clone, PartialEq)]
pub struct FunLuxScene {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub scene_kind: FunLuxSceneKind,
    pub bounds_world: LuxAabb,
    pub light_list: Vec<u64>,
    pub static_lighting_cache: FunLuxSceneCache,
    pub dynamic_lighting_cache: FunLuxSceneCache,
    pub volumetric_cache: FunLuxSceneCache,
    pub shadow_cache: FunLuxSceneCache,
    pub dirty_region_list: Vec<LuxDirtyRegion>,
    pub lighting_data: FunLuxSceneLightingData,
}

impl FunLuxScene {
    /// Typed cold-default scene: empty caches, no lights, no
    /// dirty regions.
    #[must_use]
    pub fn cold_default(scene_id: LuxSceneId, scene_kind: FunLuxSceneKind) -> Self {
        Self {
            schema_version: FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION,
            scene_id,
            scene_kind,
            bounds_world: LuxAabb::ZERO,
            light_list: Vec::new(),
            static_lighting_cache: FunLuxSceneCache::empty("fun_lux.scene.static_lighting_cache"),
            dynamic_lighting_cache: FunLuxSceneCache::empty("fun_lux.scene.dynamic_lighting_cache"),
            volumetric_cache: FunLuxSceneCache::empty("fun_lux.scene.volumetric_cache"),
            shadow_cache: FunLuxSceneCache::empty("fun_lux.scene.shadow_cache"),
            dirty_region_list: Vec::new(),
            lighting_data: FunLuxSceneLightingData::empty(scene_id),
        }
    }

    /// Pass 4 acceptance: typed predicate — "preserve static
    /// cached resources until invalidated." Returns `true`
    /// when the typed static lighting cache is still valid.
    #[must_use]
    pub fn static_cache_preservable(&self) -> bool {
        self.static_lighting_cache.can_preserve_without_rebuild()
    }

    /// Pass 4 acceptance: typed predicate — "partially
    /// update scenes with dirty lights or dirty volumes."
    /// Returns `true` when any dirty region in the typed
    /// list invalidates light or volumetric state.
    #[must_use]
    pub fn needs_partial_update(&self) -> bool {
        self.dirty_region_list.iter().any(|region| {
            region.invalidates_direct_light_clusters()
                || region.touches_volumetric_only()
                || region.invalidates_shadow_maps()
        })
    }

    /// Typed predicate: is the scene quiescent (no lights,
    /// no dirty regions, every cache invalid because it was
    /// never built)?
    #[must_use]
    pub fn is_quiescent(&self) -> bool {
        self.light_list.is_empty()
            && self.dirty_region_list.is_empty()
            && self.lighting_data.is_quiescent()
    }
}

// ============================================================================
// Section 6 — Typed world lighting (4.4 multi-scene root)
// ============================================================================

/// Typed top-level world lighting record. Aggregates every
/// active `FunLuxScene` + the typed per-scene scheduling
/// table.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FunLuxWorldLighting {
    pub schema_version: u16,
    pub scenes: Vec<FunLuxScene>,
    pub schedules: Vec<FunLuxSceneSchedule>,
}

impl FunLuxWorldLighting {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            schema_version: FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION,
            scenes: Vec::new(),
            schedules: Vec::new(),
        }
    }

    /// Typed registration: add a scene + its default
    /// schedule.
    pub fn register_scene(
        &mut self,
        scene_kind: FunLuxSceneKind,
        scene_id: LuxSceneId,
        schedule: FunLuxSceneSchedule,
    ) {
        // De-dup by scene id.
        if self.scenes.iter().any(|s| s.scene_id == scene_id) {
            return;
        }
        self.scenes
            .push(FunLuxScene::cold_default(scene_id, scene_kind));
        self.schedules.push(schedule);
    }

    /// Typed lookup by scene id.
    #[must_use]
    pub fn scene_for(&self, scene_id: LuxSceneId) -> Option<&FunLuxScene> {
        self.scenes.iter().find(|s| s.scene_id == scene_id)
    }

    /// Typed mutable lookup by scene id.
    #[must_use]
    pub fn scene_for_mut(&mut self, scene_id: LuxSceneId) -> Option<&mut FunLuxScene> {
        self.scenes.iter_mut().find(|s| s.scene_id == scene_id)
    }

    /// Typed lookup of the schedule for a scene.
    #[must_use]
    pub fn schedule_for(&self, scene_id: LuxSceneId) -> Option<&FunLuxSceneSchedule> {
        self.schedules.iter().find(|s| s.scene_id == scene_id)
    }

    /// Pass 4 acceptance: typed predicate — "update visible
    /// scenes every frame." Returns the typed list of scene
    /// ids that should update on the given frame.
    #[must_use]
    pub fn scenes_to_update_this_frame(&self, frame_index: u64) -> Vec<LuxSceneId> {
        self.schedules
            .iter()
            .filter(|s| {
                let dirty = self
                    .scene_for(s.scene_id)
                    .map(|scene| scene.needs_partial_update())
                    .unwrap_or(false);
                s.triggers_update(frame_index, dirty)
            })
            .map(|s| s.scene_id)
            .collect()
    }

    /// Pass 4 acceptance: typed predicate — "skip fully
    /// hidden scenes."
    #[must_use]
    pub fn scenes_to_skip_this_frame(&self) -> Vec<LuxSceneId> {
        self.schedules
            .iter()
            .filter(|s| s.should_skip())
            .map(|s| s.scene_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_SCENE_LIGHTING_SCHEMA_VERSION, 1);
        assert_eq!(FUN_LUX_SCENE_KIND_COUNT, 6);
        assert_eq!(FunLuxSceneKind::ALL.len(), FUN_LUX_SCENE_KIND_COUNT);
        assert_eq!(FUN_LUX_SCENE_VISIBILITY_COUNT, 3);
        assert_eq!(FUN_LUX_SCENE_UPDATE_CADENCE_COUNT, 4);
    }

    #[test]
    fn scene_kind_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for kind in FunLuxSceneKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn scene_kind_predicates() {
        assert!(FunLuxSceneKind::MainWorld.is_main_world());
        assert!(!FunLuxSceneKind::Interior.is_main_world());
        assert!(FunLuxSceneKind::EditorPreview.is_editor_only());
        assert!(!FunLuxSceneKind::MainWorld.is_editor_only());
    }

    #[test]
    fn scene_cache_lifecycle() {
        let mut cache = FunLuxSceneCache::empty("fun_lux.scene.test_cache");
        assert!(!cache.can_preserve_without_rebuild());
        cache.mark_rebuilt(4096);
        assert!(cache.can_preserve_without_rebuild());
        assert_eq!(cache.revision, 1);
        assert_eq!(cache.byte_size, 4096);
        cache.invalidate();
        assert!(!cache.can_preserve_without_rebuild());
        // Revision stays bumped after invalidate.
        assert_eq!(cache.revision, 1);
    }

    #[test]
    fn scene_lighting_data_total_count_walks_all_kinds() {
        let mut data = FunLuxSceneLightingData::empty(LuxSceneId::PROOF_SCENE);
        data.directional_light_indices.push(0);
        data.point_light_indices.extend([1, 2]);
        data.spot_light_indices.push(3);
        data.area_light_indices.push(4);
        data.emissive_renderer_indices.extend([5, 6, 7]);
        data.reflection_probe_indices.push(8);
        data.irradiance_probe_indices.push(9);
        data.fog_volume_indices.push(10);
        data.local_volumetric_volume_indices.push(11);
        data.shadow_caster_indices.push(12);
        assert_eq!(data.total_record_count(), 13);
        assert!(!data.is_quiescent());
    }

    #[test]
    fn empty_scene_lighting_data_is_quiescent() {
        let data = FunLuxSceneLightingData::empty(LuxSceneId::PROOF_SCENE);
        assert!(data.is_quiescent());
        assert_eq!(data.total_record_count(), 0);
    }

    #[test]
    fn scene_visibility_skip_predicate() {
        assert!(FunLuxSceneVisibility::FullyHidden.should_skip_this_frame());
        assert!(!FunLuxSceneVisibility::FullyVisible.should_skip_this_frame());
        assert!(!FunLuxSceneVisibility::PartiallyVisible.should_skip_this_frame());
    }

    #[test]
    fn update_cadence_triggers_match_taxonomy() {
        let cadence = FunLuxSceneUpdateCadence::EveryFrame;
        assert!(cadence.triggers(0, false));
        assert!(cadence.triggers(42, false));

        let cadence = FunLuxSceneUpdateCadence::EveryNFrames(4);
        assert!(cadence.triggers(0, false));
        assert!(cadence.triggers(4, false));
        assert!(!cadence.triggers(5, false));

        let cadence = FunLuxSceneUpdateCadence::OnChangeOnly;
        assert!(cadence.triggers(0, true));
        assert!(!cadence.triggers(0, false));

        let cadence = FunLuxSceneUpdateCadence::Skipped;
        assert!(!cadence.triggers(0, true));
    }

    /// Pass 4 acceptance: "update visible scenes every
    /// frame."
    #[test]
    fn for_visible_scene_uses_every_frame_cadence() {
        let schedule = FunLuxSceneSchedule::for_visible_scene(LuxSceneId::PROOF_SCENE);
        assert!(matches!(
            schedule.update_cadence,
            FunLuxSceneUpdateCadence::EveryFrame
        ));
        assert!(schedule.triggers_update(0, false));
        assert!(schedule.triggers_update(1, true));
        assert!(!schedule.should_skip());
    }

    /// Pass 4 acceptance: "skip fully hidden scenes."
    #[test]
    fn for_hidden_scene_is_skipped() {
        let schedule = FunLuxSceneSchedule::for_hidden_scene(LuxSceneId::PROOF_SCENE);
        assert!(schedule.should_skip());
        assert!(!schedule.triggers_update(0, true));
    }

    /// Pass 4 acceptance: "update low-priority scenes over
    /// multiple frames."
    #[test]
    fn for_low_priority_scene_uses_every_n_frames_cadence() {
        let schedule = FunLuxSceneSchedule::for_low_priority_scene(LuxSceneId::PROOF_SCENE);
        assert!(matches!(
            schedule.update_cadence,
            FunLuxSceneUpdateCadence::EveryNFrames(_)
        ));
        // Triggers on frames 0, 4, 8, ...
        assert!(schedule.triggers_update(0, false));
        assert!(!schedule.triggers_update(1, false));
        assert!(schedule.triggers_update(4, false));
    }

    #[test]
    fn cold_default_scene_is_quiescent() {
        let scene = FunLuxScene::cold_default(LuxSceneId::PROOF_SCENE, FunLuxSceneKind::MainWorld);
        assert!(scene.is_quiescent());
        assert!(!scene.static_cache_preservable());
        assert!(!scene.needs_partial_update());
    }

    /// Pass 4 acceptance: "preserve static cached resources
    /// until invalidated." After `mark_rebuilt`, the typed
    /// predicate returns `true`; after `invalidate`, false.
    #[test]
    fn static_cache_preservation_lifecycle() {
        let mut scene =
            FunLuxScene::cold_default(LuxSceneId::PROOF_SCENE, FunLuxSceneKind::MainWorld);
        scene.static_lighting_cache.mark_rebuilt(4096);
        assert!(scene.static_cache_preservable());
        scene.static_lighting_cache.invalidate();
        assert!(!scene.static_cache_preservable());
    }

    /// Pass 4 acceptance: "partially update scenes with
    /// dirty lights or dirty volumes."
    #[test]
    fn scene_needs_partial_update_when_dirty_region_present() {
        let mut scene =
            FunLuxScene::cold_default(LuxSceneId::PROOF_SCENE, FunLuxSceneKind::MainWorld);
        // No dirty regions → no partial update needed.
        assert!(!scene.needs_partial_update());
        // Add a TRANSFORM-flagged dirty region → partial update.
        scene.dirty_region_list.push(LuxDirtyRegion::whole_scene(
            LuxSceneId::PROOF_SCENE,
            LuxDirtyFlags::TRANSFORM,
        ));
        assert!(scene.needs_partial_update());
        // Replace with VOLUMETRIC-only → still a partial update
        // (volumetric needs work).
        scene.dirty_region_list.clear();
        scene.dirty_region_list.push(LuxDirtyRegion::whole_scene(
            LuxSceneId::PROOF_SCENE,
            LuxDirtyFlags::VOLUMETRIC,
        ));
        assert!(scene.needs_partial_update());
    }

    /// Pass 4 acceptance: typed world lighting walks every
    /// scene + its schedule.
    #[test]
    fn world_lighting_scenes_to_update_this_frame() {
        let mut world = FunLuxWorldLighting::empty();
        let main = LuxSceneId(1);
        let interior = LuxSceneId(2);
        let editor = LuxSceneId(3);
        world.register_scene(
            FunLuxSceneKind::MainWorld,
            main,
            FunLuxSceneSchedule::for_visible_scene(main),
        );
        world.register_scene(
            FunLuxSceneKind::Interior,
            interior,
            FunLuxSceneSchedule::for_low_priority_scene(interior),
        );
        world.register_scene(
            FunLuxSceneKind::EditorPreview,
            editor,
            FunLuxSceneSchedule::for_hidden_scene(editor),
        );
        // Frame 0: main fires (every frame), interior fires
        // (every 4 frames, 0 % 4 == 0), editor skipped.
        let updates_frame_0 = world.scenes_to_update_this_frame(0);
        assert!(updates_frame_0.contains(&main));
        assert!(updates_frame_0.contains(&interior));
        assert!(!updates_frame_0.contains(&editor));
        // Frame 1: main fires, interior doesn't (1 % 4 != 0),
        // editor skipped.
        let updates_frame_1 = world.scenes_to_update_this_frame(1);
        assert!(updates_frame_1.contains(&main));
        assert!(!updates_frame_1.contains(&interior));
        assert!(!updates_frame_1.contains(&editor));
        // Skip list contains only the editor.
        let skipped = world.scenes_to_skip_this_frame();
        assert_eq!(skipped.len(), 1);
        assert_eq!(skipped[0], editor);
    }

    #[test]
    fn world_lighting_register_scene_is_idempotent() {
        let mut world = FunLuxWorldLighting::empty();
        let id = LuxSceneId(1);
        world.register_scene(
            FunLuxSceneKind::MainWorld,
            id,
            FunLuxSceneSchedule::for_visible_scene(id),
        );
        world.register_scene(
            FunLuxSceneKind::MainWorld,
            id,
            FunLuxSceneSchedule::for_visible_scene(id),
        );
        assert_eq!(world.scenes.len(), 1);
        assert_eq!(world.schedules.len(), 1);
    }
}
