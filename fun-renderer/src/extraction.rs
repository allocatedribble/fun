use std::{marker::PhantomData, mem::size_of, time::Instant};

use bevy_app::App;
use bevy_ecs::{
    entity::Entity,
    lifecycle::RemovedComponents,
    prelude::{Added, Changed, Or, Query, Res, ResMut, Resource, With, Without},
};
use bevy_transform::components::Transform;

use crate::{component_api::*, plugin::RendererFrameIndex};

pub const RENDER_WORLD_EXTRACTION_SCHEMA_VERSION: u16 = 1;

const SYNTHETIC_STABLE_ID_BIT: u64 = 1 << 63;
const STABLE_KIND_SHIFT: u64 = 56;
const STABLE_ENTITY_MASK: u64 = 0x00ff_ffff_ffff_ffff;

pub trait RenderGenerationalId: Copy + Eq {
    const KIND: RenderStableIdKind;

    #[must_use]
    fn new(slot: u32, generation: u32) -> Self;

    #[must_use]
    fn slot(self) -> u32;

    #[must_use]
    fn generation(self) -> u32;

    #[must_use]
    fn invalid() -> Self;

    #[must_use]
    fn is_valid(self) -> bool;
}

macro_rules! impl_render_generational_id {
    ($name:ident, $kind:ident) => {
        impl RenderGenerationalId for $name {
            const KIND: RenderStableIdKind = RenderStableIdKind::$kind;

            fn new(slot: u32, generation: u32) -> Self {
                Self { slot, generation }
            }

            fn slot(self) -> u32 {
                self.slot
            }

            fn generation(self) -> u32 {
                self.generation
            }

            fn invalid() -> Self {
                Self::INVALID
            }

            fn is_valid(self) -> bool {
                self.is_valid()
            }
        }
    };
}

impl_render_generational_id!(RenderObjectId, Object);
impl_render_generational_id!(RenderMeshId, Mesh);
impl_render_generational_id!(RenderMaterialId, Material);
impl_render_generational_id!(RenderTextureId, Texture);
impl_render_generational_id!(RenderViewId, View);
impl_render_generational_id!(RenderLightId, Light);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderStableIdKind {
    Object,
    Mesh,
    Material,
    Texture,
    View,
    Light,
    UiSurface,
    CefSurface,
    PostProcessVolume,
}

impl RenderStableIdKind {
    #[must_use]
    pub const fn namespace(self) -> u64 {
        match self {
            Self::Object => 1,
            Self::Mesh => 2,
            Self::Material => 3,
            Self::Texture => 4,
            Self::View => 5,
            Self::Light => 6,
            Self::UiSurface => 7,
            Self::CefSurface => 8,
            Self::PostProcessVolume => 9,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Mesh => "mesh",
            Self::Material => "material",
            Self::Texture => "texture",
            Self::View => "view",
            Self::Light => "light",
            Self::UiSurface => "ui_surface",
            Self::CefSurface => "cef_surface",
            Self::PostProcessVolume => "post_process_volume",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RenderIdSlot {
    generation: u32,
    live: bool,
    stable_id: RenderStableId,
}

impl Default for RenderIdSlot {
    fn default() -> Self {
        Self {
            generation: 1,
            live: false,
            stable_id: RenderStableId::INVALID,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedRenderIdAllocator<I: RenderGenerationalId> {
    slots: Vec<RenderIdSlot>,
    stable_to_id: Vec<(RenderStableId, I)>,
    free_slots: Vec<u32>,
    allocated_count: u32,
    removed_count: u32,
    stale_reject_count: u32,
    _id: PhantomData<fn() -> I>,
}

impl<I: RenderGenerationalId> Default for TypedRenderIdAllocator<I> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            stable_to_id: Vec::new(),
            free_slots: Vec::new(),
            allocated_count: 0,
            removed_count: 0,
            stale_reject_count: 0,
            _id: PhantomData,
        }
    }
}

impl<I: RenderGenerationalId> TypedRenderIdAllocator<I> {
    #[must_use]
    pub fn get(&self, stable_id: RenderStableId) -> Option<I> {
        self.stable_to_id
            .iter()
            .find_map(|(candidate, id)| (*candidate == stable_id).then_some(*id))
            .filter(|id| self.is_live(*id))
    }

    pub fn allocate_or_get(&mut self, stable_id: RenderStableId) -> I {
        if !stable_id.is_valid() {
            return I::invalid();
        }
        if let Some(id) = self.get(stable_id) {
            return id;
        }

        let slot = self.free_slots.pop().unwrap_or(self.slots.len() as u32);
        if slot as usize == self.slots.len() {
            self.slots.push(RenderIdSlot::default());
        }
        let record = &mut self.slots[slot as usize];
        record.live = true;
        record.stable_id = stable_id;
        let id = I::new(slot, record.generation.max(1));
        self.stable_to_id.push((stable_id, id));
        self.stable_to_id.sort_by_key(|(stable, _)| stable.0);
        self.allocated_count = self.allocated_count.saturating_add(1);
        id
    }

    pub fn remove(&mut self, stable_id: RenderStableId) -> Option<I> {
        let index = self
            .stable_to_id
            .iter()
            .position(|(candidate, _)| *candidate == stable_id)?;
        let (_, id) = self.stable_to_id.swap_remove(index);
        if let Some(slot) = self.slots.get_mut(id.slot() as usize)
            && slot.live
            && slot.generation == id.generation()
        {
            slot.live = false;
            slot.stable_id = RenderStableId::INVALID;
            slot.generation = slot.generation.saturating_add(1).max(1);
            self.free_slots.push(id.slot());
            self.removed_count = self.removed_count.saturating_add(1);
        }
        Some(id)
    }

    #[must_use]
    pub fn is_live(&self, id: I) -> bool {
        if !id.is_valid() {
            return false;
        }
        self.slots
            .get(id.slot() as usize)
            .is_some_and(|slot| slot.live && slot.generation == id.generation())
    }

    pub fn validate(&mut self, id: I) -> bool {
        let live = self.is_live(id);
        if id.is_valid() && !live {
            self.stale_reject_count = self.stale_reject_count.saturating_add(1);
        }
        live
    }

    #[must_use]
    pub fn live_count(&self) -> u32 {
        self.slots.iter().filter(|slot| slot.live).count() as u32
    }

    #[must_use]
    pub const fn allocated_count(&self) -> u32 {
        self.allocated_count
    }

    #[must_use]
    pub const fn removed_count(&self) -> u32 {
        self.removed_count
    }

    #[must_use]
    pub const fn stale_reject_count(&self) -> u32 {
        self.stale_reject_count
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct EntityStableMap {
    records: Vec<(Entity, RenderStableId)>,
}

impl EntityStableMap {
    fn bind(
        &mut self,
        kind: RenderStableIdKind,
        entity: Entity,
        explicit: RenderStableId,
    ) -> (RenderStableId, Option<RenderStableId>) {
        let stable_id = if explicit.is_valid() {
            explicit
        } else {
            synthetic_stable_id(kind, entity)
        };

        if let Some((_, existing)) = self
            .records
            .iter_mut()
            .find(|(candidate, _)| *candidate == entity)
        {
            if *existing == stable_id {
                return (stable_id, None);
            }
            let previous = *existing;
            *existing = stable_id;
            return (stable_id, Some(previous));
        }

        self.records.push((entity, stable_id));
        self.records.sort_by_key(|(entity, _)| entity.to_bits());
        (stable_id, None)
    }

    fn remove(&mut self, entity: Entity) -> Option<RenderStableId> {
        let index = self
            .records
            .iter()
            .position(|(candidate, _)| *candidate == entity)?;
        Some(self.records.swap_remove(index).1)
    }

    #[must_use]
    fn len(&self) -> usize {
        self.records.len()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct RenderStableIdAllocator {
    objects: TypedRenderIdAllocator<RenderObjectId>,
    meshes: TypedRenderIdAllocator<RenderMeshId>,
    materials: TypedRenderIdAllocator<RenderMaterialId>,
    textures: TypedRenderIdAllocator<RenderTextureId>,
    views: TypedRenderIdAllocator<RenderViewId>,
    lights: TypedRenderIdAllocator<RenderLightId>,
    object_entities: EntityStableMap,
    view_entities: EntityStableMap,
    light_entities: EntityStableMap,
    ui_entities: EntityStableMap,
    cef_entities: EntityStableMap,
    post_entities: EntityStableMap,
}

impl RenderStableIdAllocator {
    pub fn object_id_for_stable(&mut self, stable_id: RenderStableId) -> RenderObjectId {
        self.objects.allocate_or_get(stable_id)
    }

    pub fn mesh_id_for_asset(&mut self, asset: RenderMeshAssetId) -> RenderMeshId {
        self.meshes
            .allocate_or_get(stable_id_from_asset(RenderStableIdKind::Mesh, asset))
    }

    pub fn material_id_for_asset(&mut self, asset: RenderMaterialAssetId) -> RenderMaterialId {
        self.materials
            .allocate_or_get(stable_id_from_asset(RenderStableIdKind::Material, asset))
    }

    pub fn texture_id_for_asset(&mut self, asset: RenderTextureAssetId) -> RenderTextureId {
        self.textures
            .allocate_or_get(stable_id_from_asset(RenderStableIdKind::Texture, asset))
    }

    pub fn view_id_for_stable(&mut self, stable_id: RenderStableId) -> RenderViewId {
        self.views.allocate_or_get(stable_id)
    }

    pub fn light_id_for_stable(&mut self, stable_id: RenderStableId) -> RenderLightId {
        self.lights.allocate_or_get(stable_id)
    }

    pub fn remove_object(&mut self, stable_id: RenderStableId) -> Option<RenderObjectId> {
        self.objects.remove(stable_id)
    }

    pub fn remove_view(&mut self, stable_id: RenderStableId) -> Option<RenderViewId> {
        self.views.remove(stable_id)
    }

    pub fn remove_light(&mut self, stable_id: RenderStableId) -> Option<RenderLightId> {
        self.lights.remove(stable_id)
    }

    #[must_use]
    pub fn is_live_object(&self, id: RenderObjectId) -> bool {
        self.objects.is_live(id)
    }

    pub fn validate_object(&mut self, id: RenderObjectId) -> bool {
        self.objects.validate(id)
    }

    #[must_use]
    pub fn allocator_diagnostics(&self) -> RenderIdAllocatorDiagnostics {
        RenderIdAllocatorDiagnostics {
            object_count: self.objects.live_count(),
            mesh_count: self.meshes.live_count(),
            material_count: self.materials.live_count(),
            texture_count: self.textures.live_count(),
            view_count: self.views.live_count(),
            light_count: self.lights.live_count(),
            removed_object_count: self.objects.removed_count(),
            stale_reference_reject_count: self.objects.stale_reject_count()
                + self.meshes.stale_reject_count()
                + self.materials.stale_reject_count()
                + self.textures.stale_reject_count()
                + self.views.stale_reject_count()
                + self.lights.stale_reject_count(),
            tracked_entity_count: (self.object_entities.len()
                + self.view_entities.len()
                + self.light_entities.len()
                + self.ui_entities.len()
                + self.cef_entities.len()
                + self.post_entities.len()) as u32,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderIdAllocatorDiagnostics {
    pub object_count: u32,
    pub mesh_count: u32,
    pub material_count: u32,
    pub texture_count: u32,
    pub view_count: u32,
    pub light_count: u32,
    pub removed_object_count: u32,
    pub stale_reference_reject_count: u32,
    pub tracked_entity_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractedTransform {
    pub translation: RenderVec3,
    pub rotation_xyzw: [f32; 4],
    pub scale: RenderVec3,
}

impl ExtractedTransform {
    pub const IDENTITY: Self = Self {
        translation: RenderVec3::ZERO,
        rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
        scale: RenderVec3::new(1.0, 1.0, 1.0),
    };
}

impl Default for ExtractedTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<&Transform> for ExtractedTransform {
    fn from(transform: &Transform) -> Self {
        Self {
            translation: RenderVec3::new(
                transform.translation.x,
                transform.translation.y,
                transform.translation.z,
            ),
            rotation_xyzw: [
                transform.rotation.x,
                transform.rotation.y,
                transform.rotation.z,
                transform.rotation.w,
            ],
            scale: RenderVec3::new(transform.scale.x, transform.scale.y, transform.scale.z),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtractedLightKind {
    Directional,
    Point,
    Spot,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractedView {
    pub view_id: RenderViewId,
    pub stable_id: RenderStableId,
    pub transform: ExtractedTransform,
    pub camera: RenderCamera,
    pub projection: CameraProjection,
    pub exposure: CameraExposure,
    pub jitter: CameraJitter,
    pub history: CameraHistory,
    pub target: CameraRenderTarget,
    pub debug_view: CameraDebugView,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractedRenderable {
    pub object_id: RenderObjectId,
    pub stable_id: RenderStableId,
    pub transform: ExtractedTransform,
    pub mesh_id: RenderMeshId,
    pub material_id: RenderMaterialId,
    pub bounds: RenderBounds,
    pub layer: RenderLayer,
    pub visibility: RenderVisibility,
    pub is_static: bool,
    pub is_dynamic: bool,
    pub debug_name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractedMeshInstance {
    pub mesh_id: RenderMeshId,
    pub object_id: RenderObjectId,
    pub source_mesh: RenderMeshAssetId,
    pub transform: ExtractedTransform,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractedMaterialInstance {
    pub material_id: RenderMaterialId,
    pub source_material: RenderMaterialAssetId,
    pub feature_mask: MaterialFeatureMask,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractedLight {
    pub light_id: RenderLightId,
    pub stable_id: RenderStableId,
    pub kind: ExtractedLightKind,
    pub transform: ExtractedTransform,
    pub color: RenderColor,
    pub intensity: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
    pub shadow: ShadowCaster,
    pub layer: LightLayer,
    pub bounds: LightBounds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractedUiSurface {
    pub stable_id: RenderStableId,
    pub surface: UiSurface,
    pub layer: UiLayer,
    pub composite_order: UiCompositeOrder,
    pub opacity: UiOpacity,
    pub color_space: UiColorSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractedCefSurface {
    pub stable_id: RenderStableId,
    pub surface: CefSurface,
    pub producer: CefFrameProducer,
    pub health: CefSurfaceHealth,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExtractedPostProcessVolume {
    pub view_id: RenderViewId,
    pub stable_id: RenderStableId,
    pub tone_mapping: ToneMappingSettings,
    pub bloom: BloomSettings,
    pub taa: TaaSettings,
    pub hdr_output: HdrOutputSettings,
    pub upscaler: UpscalerSettings,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct RenderWorldTables {
    pub object_table: Vec<ExtractedRenderable>,
    pub transform_table: Vec<(RenderObjectId, ExtractedTransform)>,
    pub bounds_table: Vec<(RenderObjectId, RenderBounds)>,
    pub material_table: Vec<ExtractedMaterialInstance>,
    pub mesh_instance_table: Vec<ExtractedMeshInstance>,
    pub light_table: Vec<ExtractedLight>,
    pub view_table: Vec<ExtractedView>,
    pub ui_surface_table: Vec<ExtractedUiSurface>,
    pub cef_surface_table: Vec<ExtractedCefSurface>,
    pub post_process_volume_table: Vec<ExtractedPostProcessVolume>,
}

impl RenderWorldTables {
    #[must_use]
    pub fn table_counts(&self) -> RenderWorldTableCounts {
        RenderWorldTableCounts {
            objects: self.object_table.len() as u32,
            transforms: self.transform_table.len() as u32,
            bounds: self.bounds_table.len() as u32,
            materials: self.material_table.len() as u32,
            mesh_instances: self.mesh_instance_table.len() as u32,
            lights: self.light_table.len() as u32,
            views: self.view_table.len() as u32,
            ui_surfaces: self.ui_surface_table.len() as u32,
            cef_surfaces: self.cef_surface_table.len() as u32,
            post_process_volumes: self.post_process_volume_table.len() as u32,
        }
    }

    #[must_use]
    pub fn memory_bytes(&self) -> u64 {
        capacity_bytes::<ExtractedRenderable>(&self.object_table)
            + capacity_bytes::<(RenderObjectId, ExtractedTransform)>(&self.transform_table)
            + capacity_bytes::<(RenderObjectId, RenderBounds)>(&self.bounds_table)
            + capacity_bytes::<ExtractedMaterialInstance>(&self.material_table)
            + capacity_bytes::<ExtractedMeshInstance>(&self.mesh_instance_table)
            + capacity_bytes::<ExtractedLight>(&self.light_table)
            + capacity_bytes::<ExtractedView>(&self.view_table)
            + capacity_bytes::<ExtractedUiSurface>(&self.ui_surface_table)
            + capacity_bytes::<ExtractedCefSurface>(&self.cef_surface_table)
            + capacity_bytes::<ExtractedPostProcessVolume>(&self.post_process_volume_table)
    }

    pub fn remove_object(&mut self, object_id: RenderObjectId) {
        self.object_table
            .retain(|record| record.object_id != object_id);
        self.transform_table
            .retain(|(candidate, _)| *candidate != object_id);
        self.bounds_table
            .retain(|(candidate, _)| *candidate != object_id);
        self.mesh_instance_table
            .retain(|record| record.object_id != object_id);
    }

    pub fn remove_view(&mut self, view_id: RenderViewId) {
        self.view_table.retain(|record| record.view_id != view_id);
        self.post_process_volume_table
            .retain(|record| record.view_id != view_id);
    }

    pub fn remove_light(&mut self, light_id: RenderLightId) {
        self.light_table
            .retain(|record| record.light_id != light_id);
    }

    fn upsert_object(&mut self, record: ExtractedRenderable) {
        upsert_by(&mut self.object_table, record, |record| record.object_id);
        upsert_by(
            &mut self.transform_table,
            (record.object_id, record.transform),
            |record| record.0,
        );
        upsert_by(
            &mut self.bounds_table,
            (record.object_id, record.bounds),
            |record| record.0,
        );
    }

    fn upsert_mesh_instance(&mut self, record: ExtractedMeshInstance) {
        if record.mesh_id.is_valid() {
            upsert_by(&mut self.mesh_instance_table, record, |record| {
                record.object_id
            });
        }
    }

    fn upsert_material(&mut self, record: ExtractedMaterialInstance) {
        if record.material_id.is_valid() {
            upsert_by(&mut self.material_table, record, |record| {
                record.material_id
            });
        }
    }

    fn upsert_light(&mut self, record: ExtractedLight) {
        upsert_by(&mut self.light_table, record, |record| record.light_id);
    }

    fn upsert_view(&mut self, record: ExtractedView) {
        upsert_by(&mut self.view_table, record, |record| record.view_id);
    }

    fn upsert_ui_surface(&mut self, record: ExtractedUiSurface) {
        upsert_by(&mut self.ui_surface_table, record, |record| {
            record.stable_id
        });
    }

    fn upsert_cef_surface(&mut self, record: ExtractedCefSurface) {
        upsert_by(&mut self.cef_surface_table, record, |record| {
            record.stable_id
        });
    }

    fn upsert_post_process_volume(&mut self, record: ExtractedPostProcessVolume) {
        upsert_by(&mut self.post_process_volume_table, record, |record| {
            record.view_id
        });
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RenderWorldTableCounts {
    pub objects: u32,
    pub transforms: u32,
    pub bounds: u32,
    pub materials: u32,
    pub mesh_instances: u32,
    pub lights: u32,
    pub views: u32,
    pub ui_surfaces: u32,
    pub cef_surfaces: u32,
    pub post_process_volumes: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct RendererAssetEvents {
    pub changed_meshes: Vec<RenderMeshAssetId>,
    pub changed_materials: Vec<RenderMaterialAssetId>,
    pub changed_textures: Vec<RenderTextureAssetId>,
}

impl RendererAssetEvents {
    pub fn record_mesh_changed(&mut self, mesh: RenderMeshAssetId) {
        if mesh.is_valid() {
            self.changed_meshes.push(mesh);
        }
    }

    pub fn record_material_changed(&mut self, material: RenderMaterialAssetId) {
        if material.is_valid() {
            self.changed_materials.push(material);
        }
    }

    pub fn record_texture_changed(&mut self, texture: RenderTextureAssetId) {
        if texture.is_valid() {
            self.changed_textures.push(texture);
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderWorldFullRebuildReason {
    #[default]
    None,
    ExplicitOperatorRequest,
    AssetRegistryInvalidated,
    StableIdAllocatorReset,
}

impl RenderWorldFullRebuildReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ExplicitOperatorRequest => "explicit_operator_request",
            Self::AssetRegistryInvalidated => "asset_registry_invalidated",
            Self::StableIdAllocatorReset => "stable_id_allocator_reset",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RenderWorldExtractionControl {
    pub force_full_rebuild: bool,
    pub full_rebuild_reason: RenderWorldFullRebuildReason,
}

impl Default for RenderWorldExtractionControl {
    fn default() -> Self {
        Self {
            force_full_rebuild: false,
            full_rebuild_reason: RenderWorldFullRebuildReason::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct RenderWorldExtractionDiagnostics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub queried_entities: u32,
    pub changed_entities: u32,
    pub extracted_objects: u32,
    pub extracted_materials: u32,
    pub extracted_mesh_instances: u32,
    pub extracted_lights: u32,
    pub extracted_views: u32,
    pub extracted_ui_surfaces: u32,
    pub extracted_cef_surfaces: u32,
    pub extracted_post_process_volumes: u32,
    pub removed_objects: u32,
    pub removed_views: u32,
    pub removed_lights: u32,
    pub asset_mesh_events: u32,
    pub asset_material_events: u32,
    pub asset_texture_events: u32,
    pub full_rebuild_reason: RenderWorldFullRebuildReason,
    pub extraction_cpu_time_ns: u64,
    pub render_world_memory_before_bytes: u64,
    pub render_world_memory_bytes: u64,
    pub render_world_memory_growth_bytes: i64,
    pub stale_reference_rejects: u32,
}

impl RenderWorldExtractionDiagnostics {
    pub const DEFAULT: Self = Self {
        schema_version: RENDER_WORLD_EXTRACTION_SCHEMA_VERSION,
        frame_index: 0,
        queried_entities: 0,
        changed_entities: 0,
        extracted_objects: 0,
        extracted_materials: 0,
        extracted_mesh_instances: 0,
        extracted_lights: 0,
        extracted_views: 0,
        extracted_ui_surfaces: 0,
        extracted_cef_surfaces: 0,
        extracted_post_process_volumes: 0,
        removed_objects: 0,
        removed_views: 0,
        removed_lights: 0,
        asset_mesh_events: 0,
        asset_material_events: 0,
        asset_texture_events: 0,
        full_rebuild_reason: RenderWorldFullRebuildReason::None,
        extraction_cpu_time_ns: 0,
        render_world_memory_before_bytes: 0,
        render_world_memory_bytes: 0,
        render_world_memory_growth_bytes: 0,
        stale_reference_rejects: 0,
    };

    pub fn begin_frame(
        &mut self,
        frame_index: u64,
        memory_before: u64,
        full_rebuild_reason: RenderWorldFullRebuildReason,
    ) {
        *self = Self {
            frame_index,
            render_world_memory_before_bytes: memory_before,
            render_world_memory_bytes: memory_before,
            full_rebuild_reason,
            ..Self::DEFAULT
        };
    }

    fn record_cpu_time(&mut self, started_at: Instant) {
        self.extraction_cpu_time_ns = self
            .extraction_cpu_time_ns
            .saturating_add(started_at.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64);
    }

    fn record_memory_after(&mut self, memory_after: u64) {
        self.render_world_memory_bytes = memory_after;
        self.render_world_memory_growth_bytes =
            memory_after as i64 - self.render_world_memory_before_bytes as i64;
    }
}

impl Default for RenderWorldExtractionDiagnostics {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub fn install_render_world_extraction_resources(app: &mut App) {
    app.init_resource::<RenderStableIdAllocator>()
        .init_resource::<RenderWorldTables>()
        .init_resource::<RendererAssetEvents>()
        .init_resource::<RenderWorldExtractionControl>()
        .init_resource::<RenderWorldExtractionDiagnostics>();
}

pub fn begin_render_world_extraction_frame(
    frame_index: Res<RendererFrameIndex>,
    tables: Res<RenderWorldTables>,
    mut control: ResMut<RenderWorldExtractionControl>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let reason = if control.force_full_rebuild {
        control.force_full_rebuild = false;
        control.full_rebuild_reason
    } else {
        RenderWorldFullRebuildReason::None
    };
    diagnostics.begin_frame(frame_index.0, tables.memory_bytes(), reason);
}

pub fn extract_renderer_asset_events(
    mut events: ResMut<RendererAssetEvents>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let started_at = Instant::now();
    diagnostics.asset_mesh_events = events.changed_meshes.len() as u32;
    diagnostics.asset_material_events = events.changed_materials.len() as u32;
    diagnostics.asset_texture_events = events.changed_textures.len() as u32;
    events.changed_meshes.clear();
    events.changed_materials.clear();
    events.changed_textures.clear();
    diagnostics.record_cpu_time(started_at);
}

#[allow(clippy::type_complexity)]
pub fn extract_renderer_renderables(
    query: Query<
        (
            Entity,
            &Renderable,
            Option<&RenderMesh>,
            Option<&RenderMaterial>,
            Option<&RenderBounds>,
            Option<&RenderLayer>,
            Option<&RenderVisibility>,
            Option<&Transform>,
            Option<&RenderStatic>,
            Option<&RenderDynamic>,
            Option<&RenderDebugName>,
        ),
        Or<(
            Added<Renderable>,
            Changed<Renderable>,
            Changed<RenderMesh>,
            Changed<RenderMaterial>,
            Changed<RenderBounds>,
            Changed<RenderLayer>,
            Changed<RenderVisibility>,
            Changed<Transform>,
            Changed<RenderStatic>,
            Changed<RenderDynamic>,
            Changed<RenderDebugName>,
        )>,
    >,
    mut removed: RemovedComponents<Renderable>,
    mut allocator: ResMut<RenderStableIdAllocator>,
    mut tables: ResMut<RenderWorldTables>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let started_at = Instant::now();
    for (
        entity,
        renderable,
        mesh,
        material,
        bounds,
        layer,
        visibility,
        transform,
        static_marker,
        dynamic_marker,
        debug_name,
    ) in query.iter()
    {
        diagnostics.queried_entities = diagnostics.queried_entities.saturating_add(1);
        diagnostics.changed_entities = diagnostics.changed_entities.saturating_add(1);

        let (stable_id, previous_stable) = allocator.object_entities.bind(
            RenderStableIdKind::Object,
            entity,
            renderable.object_id,
        );
        if let Some(previous_stable) = previous_stable
            && let Some(previous_id) = allocator.remove_object(previous_stable)
        {
            tables.remove_object(previous_id);
            diagnostics.removed_objects = diagnostics.removed_objects.saturating_add(1);
        }

        let object_id = allocator.object_id_for_stable(stable_id);
        let source_mesh = mesh.map_or(RenderMeshAssetId::INVALID, |mesh| mesh.mesh);
        let source_material =
            material.map_or(RenderMaterialAssetId::INVALID, |material| material.material);
        let mesh_id = allocator.mesh_id_for_asset(source_mesh);
        let material_id = allocator.material_id_for_asset(source_material);
        let transform = transform.map_or(ExtractedTransform::IDENTITY, ExtractedTransform::from);
        let bounds = bounds.copied().unwrap_or_default();
        let layer = layer.copied().unwrap_or_default();
        let visibility = visibility.copied().unwrap_or_default();
        let is_dynamic = dynamic_marker.is_some() || renderable.dirty != RenderDirtyFlags::NONE;
        let record = ExtractedRenderable {
            object_id,
            stable_id,
            transform,
            mesh_id,
            material_id,
            bounds,
            layer,
            visibility,
            is_static: static_marker.is_some() || !is_dynamic,
            is_dynamic,
            debug_name: debug_name.map_or("", |debug_name| debug_name.name),
        };
        tables.upsert_object(record);
        tables.upsert_mesh_instance(ExtractedMeshInstance {
            mesh_id,
            object_id,
            source_mesh,
            transform,
        });
        tables.upsert_material(ExtractedMaterialInstance {
            material_id,
            source_material,
            feature_mask: MaterialFeatureMask::NONE,
        });
        diagnostics.extracted_objects = diagnostics.extracted_objects.saturating_add(1);
        if mesh_id.is_valid() {
            diagnostics.extracted_mesh_instances =
                diagnostics.extracted_mesh_instances.saturating_add(1);
        }
        if material_id.is_valid() {
            diagnostics.extracted_materials = diagnostics.extracted_materials.saturating_add(1);
        }
    }

    for entity in removed.read() {
        if let Some(stable_id) = allocator.object_entities.remove(entity)
            && let Some(object_id) = allocator.remove_object(stable_id)
        {
            tables.remove_object(object_id);
            diagnostics.removed_objects = diagnostics.removed_objects.saturating_add(1);
        }
    }

    diagnostics.stale_reference_rejects = allocator
        .allocator_diagnostics()
        .stale_reference_reject_count;
    diagnostics.record_memory_after(tables.memory_bytes());
    diagnostics.record_cpu_time(started_at);
}

#[allow(clippy::type_complexity)]
pub fn extract_renderer_views(
    query: Query<
        (
            Entity,
            &RenderCamera,
            Option<&Transform>,
            Option<&CameraProjection>,
            Option<&CameraExposure>,
            Option<&CameraJitter>,
            Option<&CameraHistory>,
            Option<&CameraRenderTarget>,
            Option<&CameraDebugView>,
        ),
        Or<(
            Added<RenderCamera>,
            Changed<RenderCamera>,
            Changed<Transform>,
            Changed<CameraProjection>,
            Changed<CameraExposure>,
            Changed<CameraJitter>,
            Changed<CameraHistory>,
            Changed<CameraRenderTarget>,
            Changed<CameraDebugView>,
        )>,
    >,
    mut removed: RemovedComponents<RenderCamera>,
    mut allocator: ResMut<RenderStableIdAllocator>,
    mut tables: ResMut<RenderWorldTables>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let started_at = Instant::now();
    for (entity, camera, transform, projection, exposure, jitter, history, target, debug_view) in
        query.iter()
    {
        diagnostics.queried_entities = diagnostics.queried_entities.saturating_add(1);
        diagnostics.changed_entities = diagnostics.changed_entities.saturating_add(1);
        let explicit = history.map_or(RenderStableId::INVALID, |history| history.history_id);
        let (stable_id, previous_stable) =
            allocator
                .view_entities
                .bind(RenderStableIdKind::View, entity, explicit);
        if let Some(previous_stable) = previous_stable
            && let Some(previous_id) = allocator.remove_view(previous_stable)
        {
            tables.remove_view(previous_id);
            diagnostics.removed_views = diagnostics.removed_views.saturating_add(1);
        }
        let view_id = allocator.view_id_for_stable(stable_id);
        tables.upsert_view(ExtractedView {
            view_id,
            stable_id,
            transform: transform.map_or(ExtractedTransform::IDENTITY, ExtractedTransform::from),
            camera: *camera,
            projection: projection.copied().unwrap_or_default(),
            exposure: exposure.copied().unwrap_or_default(),
            jitter: jitter.copied().unwrap_or_default(),
            history: history.copied().unwrap_or_default(),
            target: target.copied().unwrap_or_default(),
            debug_view: debug_view.copied().unwrap_or_default(),
        });
        diagnostics.extracted_views = diagnostics.extracted_views.saturating_add(1);
    }

    for entity in removed.read() {
        if let Some(stable_id) = allocator.view_entities.remove(entity)
            && let Some(view_id) = allocator.remove_view(stable_id)
        {
            tables.remove_view(view_id);
            diagnostics.removed_views = diagnostics.removed_views.saturating_add(1);
        }
    }
    diagnostics.record_memory_after(tables.memory_bytes());
    diagnostics.record_cpu_time(started_at);
}

#[allow(clippy::type_complexity)]
pub fn extract_renderer_lights(
    query: Query<
        (
            Entity,
            Option<&DirectionalLight>,
            Option<&PointLight>,
            Option<&SpotLight>,
            Option<&Transform>,
            Option<&ShadowCaster>,
            Option<&LightLayer>,
            Option<&LightBounds>,
        ),
        (
            Or<(
                Added<DirectionalLight>,
                Changed<DirectionalLight>,
                Added<PointLight>,
                Changed<PointLight>,
                Added<SpotLight>,
                Changed<SpotLight>,
                Changed<Transform>,
                Changed<ShadowCaster>,
                Changed<LightLayer>,
                Changed<LightBounds>,
            )>,
            Without<Renderable>,
        ),
    >,
    mut removed_directional: RemovedComponents<DirectionalLight>,
    mut removed_point: RemovedComponents<PointLight>,
    mut removed_spot: RemovedComponents<SpotLight>,
    mut allocator: ResMut<RenderStableIdAllocator>,
    mut tables: ResMut<RenderWorldTables>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let started_at = Instant::now();
    for (entity, directional, point, spot, transform, shadow, layer, bounds) in query.iter() {
        let Some((kind, color, intensity, range, inner, outer)) =
            extracted_light_shape(directional, point, spot)
        else {
            continue;
        };
        diagnostics.queried_entities = diagnostics.queried_entities.saturating_add(1);
        diagnostics.changed_entities = diagnostics.changed_entities.saturating_add(1);
        let (stable_id, previous_stable) = allocator.light_entities.bind(
            RenderStableIdKind::Light,
            entity,
            RenderStableId::INVALID,
        );
        if let Some(previous_stable) = previous_stable
            && let Some(previous_id) = allocator.remove_light(previous_stable)
        {
            tables.remove_light(previous_id);
            diagnostics.removed_lights = diagnostics.removed_lights.saturating_add(1);
        }
        let light_id = allocator.light_id_for_stable(stable_id);
        tables.upsert_light(ExtractedLight {
            light_id,
            stable_id,
            kind,
            transform: transform.map_or(ExtractedTransform::IDENTITY, ExtractedTransform::from),
            color,
            intensity,
            range,
            inner_angle_radians: inner,
            outer_angle_radians: outer,
            shadow: shadow.copied().unwrap_or_default(),
            layer: layer.copied().unwrap_or_default(),
            bounds: bounds.copied().unwrap_or_default(),
        });
        diagnostics.extracted_lights = diagnostics.extracted_lights.saturating_add(1);
    }

    for entity in removed_directional
        .read()
        .chain(removed_point.read())
        .chain(removed_spot.read())
    {
        if let Some(stable_id) = allocator.light_entities.remove(entity)
            && let Some(light_id) = allocator.remove_light(stable_id)
        {
            tables.remove_light(light_id);
            diagnostics.removed_lights = diagnostics.removed_lights.saturating_add(1);
        }
    }
    diagnostics.record_memory_after(tables.memory_bytes());
    diagnostics.record_cpu_time(started_at);
}

#[allow(clippy::type_complexity)]
pub fn extract_renderer_ui_surfaces(
    query: Query<
        (
            Entity,
            &UiSurface,
            Option<&UiLayer>,
            Option<&UiCompositeOrder>,
            Option<&UiOpacity>,
            Option<&UiColorSpace>,
        ),
        Or<(
            Added<UiSurface>,
            Changed<UiSurface>,
            Changed<UiLayer>,
            Changed<UiCompositeOrder>,
            Changed<UiOpacity>,
            Changed<UiColorSpace>,
        )>,
    >,
    mut removed: RemovedComponents<UiSurface>,
    mut allocator: ResMut<RenderStableIdAllocator>,
    mut tables: ResMut<RenderWorldTables>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let started_at = Instant::now();
    for (entity, surface, layer, composite_order, opacity, color_space) in query.iter() {
        diagnostics.queried_entities = diagnostics.queried_entities.saturating_add(1);
        diagnostics.changed_entities = diagnostics.changed_entities.saturating_add(1);
        let (stable_id, _) =
            allocator
                .ui_entities
                .bind(RenderStableIdKind::UiSurface, entity, surface.surface_id);
        tables.upsert_ui_surface(ExtractedUiSurface {
            stable_id,
            surface: *surface,
            layer: layer.copied().unwrap_or_default(),
            composite_order: composite_order.copied().unwrap_or_default(),
            opacity: opacity.copied().unwrap_or_default(),
            color_space: color_space.copied().unwrap_or_default(),
        });
        diagnostics.extracted_ui_surfaces = diagnostics.extracted_ui_surfaces.saturating_add(1);
    }
    for entity in removed.read() {
        if let Some(stable_id) = allocator.ui_entities.remove(entity) {
            tables
                .ui_surface_table
                .retain(|surface| surface.stable_id != stable_id);
        }
    }
    diagnostics.record_memory_after(tables.memory_bytes());
    diagnostics.record_cpu_time(started_at);
}

#[allow(clippy::type_complexity)]
pub fn extract_renderer_cef_surfaces(
    query: Query<
        (
            Entity,
            &CefSurface,
            Option<&CefFrameProducer>,
            Option<&CefSurfaceHealth>,
        ),
        Or<(
            Added<CefSurface>,
            Changed<CefSurface>,
            Changed<CefFrameProducer>,
            Changed<CefSurfaceHealth>,
        )>,
    >,
    mut removed: RemovedComponents<CefSurface>,
    mut allocator: ResMut<RenderStableIdAllocator>,
    mut tables: ResMut<RenderWorldTables>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let started_at = Instant::now();
    for (entity, surface, producer, health) in query.iter() {
        diagnostics.queried_entities = diagnostics.queried_entities.saturating_add(1);
        diagnostics.changed_entities = diagnostics.changed_entities.saturating_add(1);
        let (stable_id, _) =
            allocator
                .cef_entities
                .bind(RenderStableIdKind::CefSurface, entity, surface.surface_id);
        tables.upsert_cef_surface(ExtractedCefSurface {
            stable_id,
            surface: *surface,
            producer: producer.copied().unwrap_or_default(),
            health: health.copied().unwrap_or_default(),
        });
        diagnostics.extracted_cef_surfaces = diagnostics.extracted_cef_surfaces.saturating_add(1);
    }
    for entity in removed.read() {
        if let Some(stable_id) = allocator.cef_entities.remove(entity) {
            tables
                .cef_surface_table
                .retain(|surface| surface.stable_id != stable_id);
        }
    }
    diagnostics.record_memory_after(tables.memory_bytes());
    diagnostics.record_cpu_time(started_at);
}

#[allow(clippy::type_complexity)]
pub fn extract_renderer_post_process_volumes(
    query: Query<
        (
            Entity,
            Option<&CameraHistory>,
            Option<&ToneMappingSettings>,
            Option<&BloomSettings>,
            Option<&TaaSettings>,
            Option<&HdrOutputSettings>,
            Option<&UpscalerSettings>,
        ),
        (
            Or<(
                Added<ToneMappingSettings>,
                Changed<ToneMappingSettings>,
                Added<BloomSettings>,
                Changed<BloomSettings>,
                Added<TaaSettings>,
                Changed<TaaSettings>,
                Added<HdrOutputSettings>,
                Changed<HdrOutputSettings>,
                Added<UpscalerSettings>,
                Changed<UpscalerSettings>,
            )>,
            With<RenderCamera>,
        ),
    >,
    mut allocator: ResMut<RenderStableIdAllocator>,
    mut tables: ResMut<RenderWorldTables>,
    mut diagnostics: ResMut<RenderWorldExtractionDiagnostics>,
) {
    let started_at = Instant::now();
    for (entity, history, tone_mapping, bloom, taa, hdr_output, upscaler) in query.iter() {
        diagnostics.queried_entities = diagnostics.queried_entities.saturating_add(1);
        diagnostics.changed_entities = diagnostics.changed_entities.saturating_add(1);
        let explicit = history.map_or(RenderStableId::INVALID, |history| history.history_id);
        let (stable_id, _) =
            allocator
                .post_entities
                .bind(RenderStableIdKind::PostProcessVolume, entity, explicit);
        let view_id = allocator.view_id_for_stable(stable_id);
        tables.upsert_post_process_volume(ExtractedPostProcessVolume {
            view_id,
            stable_id,
            tone_mapping: tone_mapping.copied().unwrap_or_default(),
            bloom: bloom.copied().unwrap_or_default(),
            taa: taa.copied().unwrap_or_default(),
            hdr_output: hdr_output.copied().unwrap_or_default(),
            upscaler: upscaler.copied().unwrap_or_default(),
        });
        diagnostics.extracted_post_process_volumes =
            diagnostics.extracted_post_process_volumes.saturating_add(1);
    }
    diagnostics.record_memory_after(tables.memory_bytes());
    diagnostics.record_cpu_time(started_at);
}

fn extracted_light_shape(
    directional: Option<&DirectionalLight>,
    point: Option<&PointLight>,
    spot: Option<&SpotLight>,
) -> Option<(ExtractedLightKind, RenderColor, f32, f32, f32, f32)> {
    if let Some(light) = directional {
        return Some((
            ExtractedLightKind::Directional,
            light.color,
            light.illuminance_lux,
            0.0,
            0.0,
            light.angular_radius_radians,
        ));
    }
    if let Some(light) = point {
        return Some((
            ExtractedLightKind::Point,
            light.color,
            light.intensity_lumens,
            light.range,
            0.0,
            0.0,
        ));
    }
    spot.map(|light| {
        (
            ExtractedLightKind::Spot,
            light.color,
            light.intensity_lumens,
            light.range,
            light.inner_angle_radians,
            light.outer_angle_radians,
        )
    })
}

fn synthetic_stable_id(kind: RenderStableIdKind, entity: Entity) -> RenderStableId {
    RenderStableId(
        SYNTHETIC_STABLE_ID_BIT
            | (kind.namespace() << STABLE_KIND_SHIFT)
            | (entity.to_bits() & STABLE_ENTITY_MASK),
    )
}

fn stable_id_from_asset(kind: RenderStableIdKind, asset: RenderAssetId) -> RenderStableId {
    if !asset.is_valid() {
        return RenderStableId::INVALID;
    }
    RenderStableId(
        (kind.namespace() << STABLE_KIND_SHIFT)
            | ((u64::from(asset.generation)) << 32)
            | u64::from(asset.slot),
    )
}

fn capacity_bytes<T>(table: &Vec<T>) -> u64 {
    table.capacity().saturating_mul(size_of::<T>()) as u64
}

fn upsert_by<T, K>(table: &mut Vec<T>, record: T, key: impl Fn(&T) -> K)
where
    K: Ord + Copy,
{
    let record_key = key(&record);
    if let Some(existing) = table
        .iter_mut()
        .find(|candidate| key(candidate) == record_key)
    {
        *existing = record;
        return;
    }
    table.push(record);
    table.sort_by_key(key);
}

#[cfg(test)]
mod tests {
    use bevy_ecs::{
        prelude::World,
        schedule::{IntoScheduleConfigs, Schedule},
    };

    use super::*;

    fn install_resources(world: &mut World) {
        world.insert_resource(RendererFrameIndex::default());
        world.insert_resource(RenderStableIdAllocator::default());
        world.insert_resource(RenderWorldTables::default());
        world.insert_resource(RendererAssetEvents::default());
        world.insert_resource(RenderWorldExtractionControl::default());
        world.insert_resource(RenderWorldExtractionDiagnostics::default());
    }

    #[test]
    fn stable_render_ids_use_generations_and_reject_stale_handles() {
        let mut allocator = RenderStableIdAllocator::default();
        let stable = RenderStableId::new(42);
        let first = allocator.object_id_for_stable(stable);
        assert!(first.is_valid());
        assert!(allocator.is_live_object(first));

        let removed = allocator.remove_object(stable).expect("removed id");
        assert_eq!(removed, first);
        assert!(!allocator.is_live_object(first));
        assert!(!allocator.validate_object(first));

        let second = allocator.object_id_for_stable(stable);
        assert_eq!(first.slot, second.slot);
        assert_ne!(first.generation, second.generation);
        assert_eq!(
            allocator
                .allocator_diagnostics()
                .stale_reference_reject_count,
            1
        );
    }

    #[test]
    fn renderer_component_extraction_populates_backend_neutral_dense_tables() {
        let mut world = World::new();
        install_resources(&mut world);
        world.spawn((
            Renderable {
                object_id: RenderStableId::new(7),
                dirty: RenderDirtyFlags::GEOMETRY,
            },
            RenderMesh {
                mesh: RenderMeshAssetId::first(3),
            },
            RenderMaterial {
                material: RenderMaterialAssetId::first(9),
            },
            RenderBounds {
                local: RenderAabb::new(RenderVec3::ZERO, RenderVec3::new(2.0, 2.0, 2.0)),
            },
            RenderStatic,
            Transform::from_xyz(1.0, 2.0, 3.0),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                begin_render_world_extraction_frame,
                extract_renderer_renderables,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let tables = world.resource::<RenderWorldTables>();
        let counts = tables.table_counts();
        assert_eq!(counts.objects, 1);
        assert_eq!(counts.transforms, 1);
        assert_eq!(counts.bounds, 1);
        assert_eq!(counts.materials, 1);
        assert_eq!(counts.mesh_instances, 1);
        assert_eq!(tables.object_table[0].stable_id, RenderStableId::new(7));
        assert_eq!(tables.transform_table[0].1.translation.x, 1.0);
        assert!(tables.object_table[0].mesh_id.is_valid());
        assert!(tables.object_table[0].material_id.is_valid());

        let diagnostics = world.resource::<RenderWorldExtractionDiagnostics>();
        assert_eq!(diagnostics.queried_entities, 1);
        assert_eq!(diagnostics.changed_entities, 1);
        assert_eq!(diagnostics.extracted_objects, 1);
        assert_eq!(diagnostics.extracted_materials, 1);
        assert!(diagnostics.render_world_memory_bytes > 0);
    }

    #[test]
    fn extraction_is_incremental_until_a_component_changes() {
        let mut world = World::new();
        install_resources(&mut world);
        let entity = world
            .spawn((
                Renderable {
                    object_id: RenderStableId::new(8),
                    dirty: RenderDirtyFlags::NONE,
                },
                RenderMesh {
                    mesh: RenderMeshAssetId::first(4),
                },
                RenderMaterial {
                    material: RenderMaterialAssetId::first(5),
                },
                Transform::IDENTITY,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                begin_render_world_extraction_frame,
                extract_renderer_renderables,
            )
                .chain(),
        );
        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<RenderWorldExtractionDiagnostics>()
                .extracted_objects,
            1
        );

        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<RenderWorldExtractionDiagnostics>()
                .extracted_objects,
            0
        );

        world
            .entity_mut(entity)
            .get_mut::<Transform>()
            .expect("transform")
            .translation
            .x = 12.0;
        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<RenderWorldExtractionDiagnostics>()
                .extracted_objects,
            1
        );
        assert_eq!(
            world.resource::<RenderWorldTables>().transform_table[0]
                .1
                .translation
                .x,
            12.0
        );
    }

    #[test]
    fn removal_tracks_despawn_and_prevents_stale_object_references() {
        let mut world = World::new();
        install_resources(&mut world);
        let entity = world
            .spawn((
                Renderable {
                    object_id: RenderStableId::new(9),
                    dirty: RenderDirtyFlags::NONE,
                },
                RenderMesh {
                    mesh: RenderMeshAssetId::first(6),
                },
                RenderMaterial {
                    material: RenderMaterialAssetId::first(7),
                },
                Transform::IDENTITY,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                begin_render_world_extraction_frame,
                extract_renderer_renderables,
            )
                .chain(),
        );
        schedule.run(&mut world);
        let stale_id = world.resource::<RenderWorldTables>().object_table[0].object_id;
        let _ = world.despawn(entity);
        schedule.run(&mut world);

        assert_eq!(
            world.resource::<RenderWorldTables>().table_counts().objects,
            0
        );
        assert_eq!(
            world
                .resource::<RenderWorldExtractionDiagnostics>()
                .removed_objects,
            1
        );
        assert!(
            !world
                .resource::<RenderStableIdAllocator>()
                .is_live_object(stale_id)
        );
    }

    #[test]
    fn view_light_ui_cef_and_post_extract_into_separate_dense_tables() {
        let mut world = World::new();
        install_resources(&mut world);
        world.spawn((
            RenderCamera::default(),
            CameraHistory {
                history_id: RenderStableId::new(100),
                reset: false,
            },
            ToneMappingSettings::default(),
            BloomSettings::default(),
            TaaSettings::default(),
            HdrOutputSettings::default(),
            UpscalerSettings::default(),
            Transform::from_xyz(0.0, 1.0, 2.0),
        ));
        world.spawn((
            DirectionalLight::default(),
            ShadowCaster::default(),
            LightLayer::default(),
            Transform::from_xyz(0.0, 4.0, 0.0),
        ));
        world.spawn((
            UiSurface {
                surface_id: RenderStableId::new(200),
                extent: RenderExtent2d::new(1920, 1080),
            },
            UiLayer { layer: 4 },
            UiCompositeOrder { order: 2 },
            UiOpacity { alpha: 0.75 },
            UiColorSpace::Srgb,
        ));
        world.spawn((
            CefSurface {
                surface_id: RenderStableId::new(300),
                transport: CefTransportMode::GpuSharedTexture,
                gpu_only: true,
            },
            CefFrameProducer {
                producer_id: RenderStableId::new(301),
                max_frame_rate_hz: 120,
            },
            CefSurfaceHealth {
                state: CefHealthState::Healthy,
            },
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                begin_render_world_extraction_frame,
                extract_renderer_views,
                extract_renderer_lights,
                extract_renderer_ui_surfaces,
                extract_renderer_cef_surfaces,
                extract_renderer_post_process_volumes,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let counts = world.resource::<RenderWorldTables>().table_counts();
        assert_eq!(counts.views, 1);
        assert_eq!(counts.lights, 1);
        assert_eq!(counts.ui_surfaces, 1);
        assert_eq!(counts.cef_surfaces, 1);
        assert_eq!(counts.post_process_volumes, 1);
        assert_eq!(
            world.resource::<RenderWorldTables>().light_table[0].kind,
            ExtractedLightKind::Directional
        );
        assert_eq!(
            world.resource::<RenderWorldTables>().cef_surface_table[0]
                .producer
                .max_frame_rate_hz,
            120
        );
    }

    #[test]
    fn asset_events_are_counted_and_drained_without_backend_handles() {
        let mut world = World::new();
        install_resources(&mut world);
        {
            let mut events = world.resource_mut::<RendererAssetEvents>();
            events.record_mesh_changed(RenderMeshAssetId::first(1));
            events.record_material_changed(RenderMaterialAssetId::first(2));
            events.record_texture_changed(RenderTextureAssetId::first(3));
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                begin_render_world_extraction_frame,
                extract_renderer_asset_events,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let diagnostics = world.resource::<RenderWorldExtractionDiagnostics>();
        assert_eq!(diagnostics.asset_mesh_events, 1);
        assert_eq!(diagnostics.asset_material_events, 1);
        assert_eq!(diagnostics.asset_texture_events, 1);
        let events = world.resource::<RendererAssetEvents>();
        assert!(events.changed_meshes.is_empty());
        assert!(events.changed_materials.is_empty());
        assert!(events.changed_textures.is_empty());
    }

    #[test]
    fn extraction_public_names_do_not_leak_backend_terms() {
        for name in [
            "ExtractedView",
            "ExtractedRenderable",
            "ExtractedMeshInstance",
            "ExtractedMaterialInstance",
            "ExtractedLight",
            "ExtractedUiSurface",
            "ExtractedCefSurface",
            "ExtractedPostProcessVolume",
            "RenderWorldTables",
            "RenderStableIdAllocator",
        ] {
            for forbidden in PUBLIC_RENDERER_API_FORBIDDEN_TERMS {
                assert!(
                    !name.contains(forbidden),
                    "{name} leaked forbidden backend term {forbidden}"
                );
            }
        }
    }
}
