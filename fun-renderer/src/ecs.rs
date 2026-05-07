use bevy_ecs::{
    lifecycle::RemovedComponents,
    prelude::{Added, Changed, Component, Message, Resource},
    schedule::SystemSet,
    system::{Query, ResMut},
};
use bevy_transform::components::Transform;
use fun_scene::{
    CefSurface, PagePriorityHint, Renderable, SceneStableHistoryKey, SuperResolutionMode,
    UpscalePolicy, ViewportRenderPolicy, VirtualGeometryAuthoring, VirtualGeometryMode,
};

use crate::{
    FunRendererBackend, FunRendererFrameGenerationContract, FunRendererFrameGraphStage,
    FunRendererRuntimeBackend, FunRendererSubsystem,
    pipeline::{PIPELINE_REGISTRY_SCHEMA_VERSION, PipelineRegistry, PipelineRuntimeCounters},
    resource::{
        RENDERER_RESOURCE_SCHEMA_VERSION, RendererResourceKind, ResourceFrameAllocationDiagnostics,
        ResourceOwnershipPhase, UploadResourceKind,
    },
};

pub const FUN_RENDERER_ECS_SCHEMA_VERSION: u16 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunRendererGpuObjectId(pub u64);

impl FunRendererGpuObjectId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct RenderableIdentity(pub u64);

impl RenderableIdentity {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct GeometryHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct MaterialHandle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewportId(pub u32);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Component)]
pub struct RendererInstanceId(pub u32);

impl RendererInstanceId {
    pub const INVALID: Self = Self(u32::MAX);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunRendererGpuSceneObject {
    pub object_id: FunRendererGpuObjectId,
    pub subsystem: FunRendererSubsystem,
}

impl FunRendererGpuSceneObject {
    #[must_use]
    pub const fn new(object_id: FunRendererGpuObjectId, subsystem: FunRendererSubsystem) -> Self {
        Self {
            object_id,
            subsystem,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunRendererFrameGraphNode {
    pub stage: FunRendererFrameGraphStage,
    pub order_key: u16,
}

impl FunRendererFrameGraphNode {
    #[must_use]
    pub const fn new(stage: FunRendererFrameGraphStage) -> Self {
        Self {
            stage,
            order_key: stage.order_key(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunStaticRenderable;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunDynamicRenderable;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunVirtualGeometry;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunSkinnedRenderable;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunProceduralRenderable;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunShadowCaster;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunShadowReceiver;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunEditorSelectable;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunGameplaySalient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunVirtualGeometryFlags {
    pub bits: u8,
}

impl FunVirtualGeometryFlags {
    pub const ENABLED: u8 = 1 << 0;
    pub const DEMAND_PAGED: u8 = 1 << 1;
    pub const PROCEDURAL_SOURCE: u8 = 1 << 2;

    #[must_use]
    pub const fn new(bits: u8) -> Self {
        Self { bits }
    }

    #[must_use]
    pub const fn demand_paged() -> Self {
        Self::new(Self::ENABLED | Self::DEMAND_PAGED)
    }

    #[must_use]
    pub const fn contains(self, flag: u8) -> bool {
        self.bits & flag == flag
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunCefUiLayer {
    pub layer_id: u16,
    pub z_index: i16,
    pub gpu_shared_texture_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunUpscalerViewport {
    pub viewport_id: ViewportId,
    pub super_resolution_capable: bool,
    pub frame_generation_capable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunFrameGenerationPolicyComponent {
    pub hud_less_scene_color_required: bool,
    pub ui_color_required: bool,
    pub depth_required: bool,
    pub motion_vectors_required: bool,
    pub present_time_resource_lifetimes_required: bool,
}

impl FunFrameGenerationPolicyComponent {
    #[must_use]
    pub const fn from_product_contract(contract: FunRendererFrameGenerationContract) -> Self {
        Self {
            hud_less_scene_color_required: contract.hud_less_scene_color_required,
            ui_color_required: contract.ui_color_required,
            depth_required: contract.depth_required,
            motion_vectors_required: contract.motion_vectors_required,
            present_time_resource_lifetimes_required: contract
                .present_time_resource_lifetimes_required,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunEditorSelectionSalience {
    pub selection_rank: u16,
    pub salience: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererArchetypeClass {
    StaticRenderable,
    DynamicRenderable,
    VirtualGeometry,
    SkinnedRenderable,
    ProceduralRenderable,
    ShadowCaster,
    ShadowReceiver,
    LuxLight,
    LuxEmissive,
    LuxGiParticipant,
    EditorSelectable,
    GameplaySalient,
}

impl FunRendererArchetypeClass {
    pub const HOT_PATH_ORDER: [Self; 12] = [
        Self::StaticRenderable,
        Self::DynamicRenderable,
        Self::VirtualGeometry,
        Self::SkinnedRenderable,
        Self::ProceduralRenderable,
        Self::ShadowCaster,
        Self::ShadowReceiver,
        Self::LuxLight,
        Self::LuxEmissive,
        Self::LuxGiParticipant,
        Self::EditorSelectable,
        Self::GameplaySalient,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticRenderable => "fun_static_renderable",
            Self::DynamicRenderable => "fun_dynamic_renderable",
            Self::VirtualGeometry => "fun_virtual_geometry",
            Self::SkinnedRenderable => "fun_skinned_renderable",
            Self::ProceduralRenderable => "fun_procedural_renderable",
            Self::ShadowCaster => "fun_shadow_caster",
            Self::ShadowReceiver => "fun_shadow_receiver",
            Self::LuxLight => "fun_lux_light",
            Self::LuxEmissive => "fun_lux_emissive",
            Self::LuxGiParticipant => "fun_lux_gi_participant",
            Self::EditorSelectable => "fun_editor_selectable",
            Self::GameplaySalient => "fun_gameplay_salient",
        }
    }

    #[must_use]
    pub const fn sparse_component_ok(self) -> bool {
        matches!(
            self,
            Self::EditorSelectable | Self::LuxGiParticipant | Self::GameplaySalient
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererDataPlacementPolicy {
    pub compact_handles_required_on_entities: bool,
    pub large_material_data_on_entities_allowed: bool,
    pub gpu_buffer_handles_on_many_entities_allowed: bool,
    pub heavy_data_owner: &'static str,
}

pub const FUN_RENDERER_DATA_PLACEMENT_POLICY: FunRendererDataPlacementPolicy =
    FunRendererDataPlacementPolicy {
        compact_handles_required_on_entities: true,
        large_material_data_on_entities_allowed: false,
        gpu_buffer_handles_on_many_entities_allowed: false,
        heavy_data_owner: "resources_assets_tables",
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsGpuSourceComponent {
    Transform,
    Renderable,
    VirtualGeometryAuthoring,
    LuxLight,
    SceneStableIdentity,
}

impl EcsGpuSourceComponent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transform => "Transform",
            Self::Renderable => "Renderable",
            Self::VirtualGeometryAuthoring => "VirtualGeometryAuthoring",
            Self::LuxLight => "LuxLight",
            Self::SceneStableIdentity => "SceneStableIdentity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuTableKind {
    Instance,
    Transform,
    Material,
    GeometryPage,
    Light,
    ShadowPage,
}

impl GpuTableKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instance => "instance_table",
            Self::Transform => "transform_table",
            Self::Material => "material_table",
            Self::GeometryPage => "geometry_page_table",
            Self::Light => "light_table",
            Self::ShadowPage => "shadow_page_table",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuTableLayout {
    StructOfArrays,
}

impl GpuTableLayout {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StructOfArrays => "struct_of_arrays",
        }
    }
}

pub const TRANSFORM_GPU_TABLES: [GpuTableKind; 1] = [GpuTableKind::Transform];
pub const RENDERABLE_GPU_TABLES: [GpuTableKind; 3] = [
    GpuTableKind::Instance,
    GpuTableKind::Material,
    GpuTableKind::GeometryPage,
];
pub const VIRTUAL_GEOMETRY_GPU_TABLES: [GpuTableKind; 2] =
    [GpuTableKind::GeometryPage, GpuTableKind::ShadowPage];
pub const LUX_LIGHT_GPU_TABLES: [GpuTableKind; 2] = [GpuTableKind::Light, GpuTableKind::ShadowPage];
pub const SCENE_STABLE_IDENTITY_GPU_TABLES: [GpuTableKind; 6] = [
    GpuTableKind::Instance,
    GpuTableKind::Transform,
    GpuTableKind::Material,
    GpuTableKind::GeometryPage,
    GpuTableKind::Light,
    GpuTableKind::ShadowPage,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentGpuMapping {
    pub component: EcsGpuSourceComponent,
    pub gpu_tables: &'static [GpuTableKind],
    pub ecs_component_is_typed_and_ergonomic: bool,
    pub gpu_tables_are_compact_soa: bool,
    pub heavy_gpu_object_attached_to_entity: bool,
}

pub const COMPONENT_GPU_MAPPINGS: [ComponentGpuMapping; 5] = [
    ComponentGpuMapping {
        component: EcsGpuSourceComponent::Transform,
        gpu_tables: &TRANSFORM_GPU_TABLES,
        ecs_component_is_typed_and_ergonomic: true,
        gpu_tables_are_compact_soa: true,
        heavy_gpu_object_attached_to_entity: false,
    },
    ComponentGpuMapping {
        component: EcsGpuSourceComponent::Renderable,
        gpu_tables: &RENDERABLE_GPU_TABLES,
        ecs_component_is_typed_and_ergonomic: true,
        gpu_tables_are_compact_soa: true,
        heavy_gpu_object_attached_to_entity: false,
    },
    ComponentGpuMapping {
        component: EcsGpuSourceComponent::VirtualGeometryAuthoring,
        gpu_tables: &VIRTUAL_GEOMETRY_GPU_TABLES,
        ecs_component_is_typed_and_ergonomic: true,
        gpu_tables_are_compact_soa: true,
        heavy_gpu_object_attached_to_entity: false,
    },
    ComponentGpuMapping {
        component: EcsGpuSourceComponent::LuxLight,
        gpu_tables: &LUX_LIGHT_GPU_TABLES,
        ecs_component_is_typed_and_ergonomic: true,
        gpu_tables_are_compact_soa: true,
        heavy_gpu_object_attached_to_entity: false,
    },
    ComponentGpuMapping {
        component: EcsGpuSourceComponent::SceneStableIdentity,
        gpu_tables: &SCENE_STABLE_IDENTITY_GPU_TABLES,
        ecs_component_is_typed_and_ergonomic: true,
        gpu_tables_are_compact_soa: true,
        heavy_gpu_object_attached_to_entity: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntityAttachedReferenceKind {
    GeometryRef,
    MaterialRef,
    LuxLightId,
    SceneStableIdentity,
    RendererInstanceId,
}

impl EntityAttachedReferenceKind {
    pub const ALL: [Self; 5] = ENTITY_ATTACHED_REFERENCE_KINDS;

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GeometryRef => "GeometryRef",
            Self::MaterialRef => "MaterialRef",
            Self::LuxLightId => "LuxLightId",
            Self::SceneStableIdentity => "SceneStableIdentity",
            Self::RendererInstanceId => "RendererInstanceId",
        }
    }
}

pub const ENTITY_ATTACHED_REFERENCE_KINDS: [EntityAttachedReferenceKind; 5] = [
    EntityAttachedReferenceKind::GeometryRef,
    EntityAttachedReferenceKind::MaterialRef,
    EntityAttachedReferenceKind::LuxLightId,
    EntityAttachedReferenceKind::SceneStableIdentity,
    EntityAttachedReferenceKind::RendererInstanceId,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuDataLayoutPolicy {
    pub ecs_components_are_typed: bool,
    pub gpu_tables_are_soa: bool,
    pub attach_heavy_gpu_objects_to_entities: bool,
    pub heavyweight_owner: &'static str,
    pub allowed_entity_references: &'static [EntityAttachedReferenceKind],
}

pub const GPU_DATA_LAYOUT_POLICY: GpuDataLayoutPolicy = GpuDataLayoutPolicy {
    ecs_components_are_typed: true,
    gpu_tables_are_soa: true,
    attach_heavy_gpu_objects_to_entities: false,
    heavyweight_owner: "resources_assets_tables",
    allowed_entity_references: &ENTITY_ATTACHED_REFERENCE_KINDS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryTableKind {
    MotionVectors,
    VirtualGeometryPages,
    ShadowPages,
    GiCache,
    LightReservoirs,
}

impl HistoryTableKind {
    pub const ALL: [Self; 5] = [
        Self::MotionVectors,
        Self::VirtualGeometryPages,
        Self::ShadowPages,
        Self::GiCache,
        Self::LightReservoirs,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MotionVectors => "motion_vector_history",
            Self::VirtualGeometryPages => "virtual_geometry_page_history",
            Self::ShadowPages => "shadow_page_history",
            Self::GiCache => "gi_cache_history",
            Self::LightReservoirs => "light_reservoir_history",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryRespawnPolicy {
    Recover,
    Reset,
}

impl HistoryRespawnPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Recover => "recover",
            Self::Reset => "reset",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableHistoryId(pub u64);

impl StableHistoryId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn from_scene_key(key: SceneStableHistoryKey) -> Self {
        Self(key.0.0)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StableHistoryRecord {
    pub stable_id: StableHistoryId,
    pub table: HistoryTableKind,
    pub live_instance: Option<RendererInstanceId>,
    pub generation: u64,
    pub recover_count: u32,
    pub reset_count: u32,
}

impl StableHistoryRecord {
    #[must_use]
    pub const fn new(
        stable_id: StableHistoryId,
        table: HistoryTableKind,
        live_instance: RendererInstanceId,
    ) -> Self {
        Self {
            stable_id,
            table,
            live_instance: Some(live_instance),
            generation: 1,
            recover_count: 0,
            reset_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct StableHistoryTable {
    pub records: Vec<StableHistoryRecord>,
    pub compacted_removed_instances: u32,
    pub unrelated_history_invalidations: u32,
}

impl StableHistoryTable {
    pub fn note_spawn(
        &mut self,
        stable_id: StableHistoryId,
        table: HistoryTableKind,
        instance: RendererInstanceId,
        policy: HistoryRespawnPolicy,
    ) {
        if !stable_id.is_valid() || !instance.is_valid() {
            return;
        }
        if let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.stable_id == stable_id && record.table == table)
        {
            match policy {
                HistoryRespawnPolicy::Recover => {
                    record.recover_count = record.recover_count.saturating_add(1);
                }
                HistoryRespawnPolicy::Reset => {
                    record.generation = record.generation.saturating_add(1);
                    record.reset_count = record.reset_count.saturating_add(1);
                }
            }
            record.live_instance = Some(instance);
            return;
        }
        self.records
            .push(StableHistoryRecord::new(stable_id, table, instance));
        self.records
            .sort_by_key(|record| (record.stable_id, record.table.as_str()));
    }

    pub fn note_despawn(&mut self, stable_id: StableHistoryId) {
        for record in self
            .records
            .iter_mut()
            .filter(|record| record.stable_id == stable_id)
        {
            record.live_instance = None;
        }
    }

    pub fn note_compacted_removed_instance(&mut self, removed_instance: RendererInstanceId) {
        if !removed_instance.is_valid() {
            return;
        }
        self.compacted_removed_instances = self.compacted_removed_instances.saturating_add(1);
    }

    #[must_use]
    pub fn record(
        &self,
        stable_id: StableHistoryId,
        table: HistoryTableKind,
    ) -> Option<&StableHistoryRecord> {
        self.records
            .iter()
            .find(|record| record.stable_id == stable_id && record.table == table)
    }

    #[must_use]
    pub fn generation(&self, stable_id: StableHistoryId, table: HistoryTableKind) -> Option<u64> {
        self.record(stable_id, table)
            .map(|record| record.generation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StableHistoryPolicy {
    pub key_motion_vectors_by_stable_id: bool,
    pub key_virtual_geometry_pages_by_stable_id: bool,
    pub key_shadow_pages_by_stable_id: bool,
    pub key_gi_cache_by_stable_id: bool,
    pub key_light_reservoirs_by_stable_id: bool,
    pub key_long_lived_history_by_bevy_entity_allowed: bool,
}

pub const STABLE_HISTORY_POLICY: StableHistoryPolicy = StableHistoryPolicy {
    key_motion_vectors_by_stable_id: true,
    key_virtual_geometry_pages_by_stable_id: true,
    key_shadow_pages_by_stable_id: true,
    key_gi_cache_by_stable_id: true,
    key_light_reservoirs_by_stable_id: true,
    key_long_lived_history_by_bevy_entity_allowed: false,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererEcsPhase {
    MainWorld,
    Extract,
    RenderWorld,
}

impl FunRendererEcsPhase {
    pub const ORDER: [Self; 3] = [Self::MainWorld, Self::Extract, Self::RenderWorld];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MainWorld => "main_world",
            Self::Extract => "extract",
            Self::RenderWorld => "render_world",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererEcsPhaseDescriptor {
    pub phase: FunRendererEcsPhase,
    pub order_key: u16,
    pub owner: &'static str,
    pub consumes_opaque_scene_blobs: bool,
}

pub const FUN_RENDERER_ECS_PHASE_DESCRIPTORS: [FunRendererEcsPhaseDescriptor; 3] = [
    FunRendererEcsPhaseDescriptor {
        phase: FunRendererEcsPhase::MainWorld,
        order_key: 10,
        owner: "fun_scene_bevy_ecs",
        consumes_opaque_scene_blobs: false,
    },
    FunRendererEcsPhaseDescriptor {
        phase: FunRendererEcsPhase::Extract,
        order_key: 20,
        owner: "fun_render",
        consumes_opaque_scene_blobs: false,
    },
    FunRendererEcsPhaseDescriptor {
        phase: FunRendererEcsPhase::RenderWorld,
        order_key: 30,
        owner: "fun_renderer",
        consumes_opaque_scene_blobs: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum FunRendererSet {
    Extract,
    PrepareScene,
    PrepareResources,
    Visibility,
    VirtualGeometry,
    VirtualShadows,
    Lux,
    Upscale,
    UiComposite,
    Present,
}

impl FunRendererSet {
    pub const ORDER: [Self; 10] = [
        Self::Extract,
        Self::PrepareScene,
        Self::PrepareResources,
        Self::Visibility,
        Self::VirtualGeometry,
        Self::VirtualShadows,
        Self::Lux,
        Self::Upscale,
        Self::UiComposite,
        Self::Present,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::Extract => 10,
            Self::PrepareScene => 20,
            Self::PrepareResources => 30,
            Self::Visibility => 40,
            Self::VirtualGeometry => 50,
            Self::VirtualShadows => 60,
            Self::Lux => 70,
            Self::Upscale => 80,
            Self::UiComposite => 90,
            Self::Present => 100,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::PrepareScene => "prepare_scene",
            Self::PrepareResources => "prepare_resources",
            Self::Visibility => "visibility",
            Self::VirtualGeometry => "virtual_geometry",
            Self::VirtualShadows => "virtual_shadows",
            Self::Lux => "lux",
            Self::Upscale => "upscale",
            Self::UiComposite => "ui_composite",
            Self::Present => "present",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum RendererExtractSet {
    ExtractSceneEntities,
    ExtractRendererComponents,
    ExtractLuxComponents,
    ExtractViewports,
    ExtractCefSurfaces,
}

impl RendererExtractSet {
    pub const ORDER: [Self; 5] = [
        Self::ExtractSceneEntities,
        Self::ExtractRendererComponents,
        Self::ExtractLuxComponents,
        Self::ExtractViewports,
        Self::ExtractCefSurfaces,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::ExtractSceneEntities => 10,
            Self::ExtractRendererComponents => 20,
            Self::ExtractLuxComponents => 30,
            Self::ExtractViewports => 40,
            Self::ExtractCefSurfaces => 50,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExtractSceneEntities => "extract_scene_entities",
            Self::ExtractRendererComponents => "extract_renderer_components",
            Self::ExtractLuxComponents => "extract_lux_components",
            Self::ExtractViewports => "extract_viewports",
            Self::ExtractCefSurfaces => "extract_cef_surfaces",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum RendererPrepareSet {
    ApplySceneDeltasToGpuScene,
    UpdateInstanceTables,
    UpdateMaterialTables,
    UpdateLightTables,
    UpdatePageRequests,
    UpdateMotionVectors,
}

impl RendererPrepareSet {
    pub const ORDER: [Self; 6] = [
        Self::ApplySceneDeltasToGpuScene,
        Self::UpdateInstanceTables,
        Self::UpdateMaterialTables,
        Self::UpdateLightTables,
        Self::UpdatePageRequests,
        Self::UpdateMotionVectors,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::ApplySceneDeltasToGpuScene => 10,
            Self::UpdateInstanceTables => 20,
            Self::UpdateMaterialTables => 30,
            Self::UpdateLightTables => 40,
            Self::UpdatePageRequests => 50,
            Self::UpdateMotionVectors => 60,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ApplySceneDeltasToGpuScene => "apply_scene_deltas_to_gpu_scene",
            Self::UpdateInstanceTables => "update_instance_tables",
            Self::UpdateMaterialTables => "update_material_tables",
            Self::UpdateLightTables => "update_light_tables",
            Self::UpdatePageRequests => "update_page_requests",
            Self::UpdateMotionVectors => "update_motion_vectors",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum RendererVisibilitySet {
    BuildHzb,
    CullStaticVirtualGeometry,
    CullDynamicRenderables,
    CompactVisibleClusters,
    EmitVisibilityFeedback,
}

impl RendererVisibilitySet {
    pub const ORDER: [Self; 5] = [
        Self::BuildHzb,
        Self::CullStaticVirtualGeometry,
        Self::CullDynamicRenderables,
        Self::CompactVisibleClusters,
        Self::EmitVisibilityFeedback,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::BuildHzb => 10,
            Self::CullStaticVirtualGeometry => 20,
            Self::CullDynamicRenderables => 30,
            Self::CompactVisibleClusters => 40,
            Self::EmitVisibilityFeedback => 50,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BuildHzb => "build_hzb",
            Self::CullStaticVirtualGeometry => "cull_static_virtual_geometry",
            Self::CullDynamicRenderables => "cull_dynamic_renderables",
            Self::CompactVisibleClusters => "compact_visible_clusters",
            Self::EmitVisibilityFeedback => "emit_visibility_feedback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum RendererFrameSet {
    BuildFrameGraph,
    ExecuteFrameGraph,
    CollectDiagnostics,
}

impl RendererFrameSet {
    pub const ORDER: [Self; 3] = [
        Self::BuildFrameGraph,
        Self::ExecuteFrameGraph,
        Self::CollectDiagnostics,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::BuildFrameGraph => 10,
            Self::ExecuteFrameGraph => 20,
            Self::CollectDiagnostics => 30,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BuildFrameGraph => "build_frame_graph",
            Self::ExecuteFrameGraph => "execute_frame_graph",
            Self::CollectDiagnostics => "collect_diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRendererConfig {
    pub runtime_backend: FunRendererRuntimeBackend,
    pub backend: FunRendererBackend,
    pub consume_ecs_components_only: bool,
}

impl Default for FunRendererConfig {
    fn default() -> Self {
        Self {
            runtime_backend: FunRendererRuntimeBackend::Fun,
            backend: default_backend_for_target(),
            consume_ecs_components_only: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct GpuScene {
    pub instances: GpuInstanceTable,
    pub transforms: GpuTransformTable,
    pub materials: GpuMaterialTable,
    pub geometry: GpuGeometryTable,
    pub geometry_pages: GpuGeometryPageTable,
    pub lights: GpuLightTable,
    pub shadow_pages: GpuShadowPageTable,
    pub pages: GpuPageTable,
    pub revisions: GpuSceneRevisionTable,
}

impl GpuScene {
    pub fn record_static_renderable(&mut self, renderable: &Renderable, transform: &Transform) {
        self.instances.instance_count = self.instances.instance_count.saturating_add(1);
        self.instances.dirty_instance_count = self.instances.dirty_instance_count.saturating_add(1);
        self.transforms.transform_count = self.transforms.transform_count.saturating_add(1);
        self.transforms.dirty_transform_count =
            self.transforms.dirty_transform_count.saturating_add(1);
        if renderable.material.is_valid() {
            self.materials.material_count = self.materials.material_count.saturating_add(1);
        }
        if renderable.geometry.is_valid() {
            self.geometry.geometry_count = self.geometry.geometry_count.saturating_add(1);
            self.geometry_pages.page_record_count =
                self.geometry_pages.page_record_count.saturating_add(1);
        }
        self.revisions.scene_revision = self.revisions.scene_revision.saturating_add(1);
        self.revisions.instance_revision = self.revisions.instance_revision.saturating_add(1);
        self.instances.current_transform_signature = transform_signature(transform);
        self.transforms.current_transform_signature = self.instances.current_transform_signature;
    }

    pub fn record_removed_renderable(&mut self) {
        self.instances.instance_count = self.instances.instance_count.saturating_sub(1);
        self.instances.removed_instance_count =
            self.instances.removed_instance_count.saturating_add(1);
        self.instances.compacted_instance_count =
            self.instances.compacted_instance_count.saturating_add(1);
        self.transforms.transform_count = self.transforms.transform_count.saturating_sub(1);
        self.transforms.compacted_transform_count =
            self.transforms.compacted_transform_count.saturating_add(1);
        self.geometry_pages.compacted_page_record_count = self
            .geometry_pages
            .compacted_page_record_count
            .saturating_add(1);
        self.pages.released_page_reference_count =
            self.pages.released_page_reference_count.saturating_add(1);
        self.pages.shadow_invalidation_count =
            self.pages.shadow_invalidation_count.saturating_add(1);
        self.pages.gi_invalidation_count = self.pages.gi_invalidation_count.saturating_add(1);
        self.shadow_pages.invalidated_shadow_page_count = self
            .shadow_pages
            .invalidated_shadow_page_count
            .saturating_add(1);
        self.revisions.scene_revision = self.revisions.scene_revision.saturating_add(1);
        self.revisions.instance_revision = self.revisions.instance_revision.saturating_add(1);
        self.revisions.page_revision = self.revisions.page_revision.saturating_add(1);
    }

    pub fn record_material_patch(&mut self) {
        self.materials.dirty_material_count = self.materials.dirty_material_count.saturating_add(1);
        self.revisions.material_revision = self.revisions.material_revision.saturating_add(1);
    }

    pub fn record_geometry_patch(&mut self) {
        self.geometry.dirty_geometry_count = self.geometry.dirty_geometry_count.saturating_add(1);
        self.geometry_pages.dirty_page_record_count = self
            .geometry_pages
            .dirty_page_record_count
            .saturating_add(1);
        self.pages.shadow_invalidation_count =
            self.pages.shadow_invalidation_count.saturating_add(1);
        self.pages.gi_invalidation_count = self.pages.gi_invalidation_count.saturating_add(1);
        self.shadow_pages.invalidated_shadow_page_count = self
            .shadow_pages
            .invalidated_shadow_page_count
            .saturating_add(1);
        self.revisions.geometry_revision = self.revisions.geometry_revision.saturating_add(1);
        self.revisions.page_revision = self.revisions.page_revision.saturating_add(1);
    }

    pub fn record_light_patch(&mut self) {
        self.lights.dirty_light_count = self.lights.dirty_light_count.saturating_add(1);
        self.pages.shadow_invalidation_count =
            self.pages.shadow_invalidation_count.saturating_add(1);
        self.shadow_pages.dirty_shadow_page_count =
            self.shadow_pages.dirty_shadow_page_count.saturating_add(1);
        self.revisions.light_revision = self.revisions.light_revision.saturating_add(1);
        self.revisions.page_revision = self.revisions.page_revision.saturating_add(1);
    }

    pub fn record_virtual_geometry_authoring(&mut self, authoring: &VirtualGeometryAuthoring) {
        if matches!(authoring.mode, VirtualGeometryMode::Disabled) {
            return;
        }
        self.pages.requested_page_count = self.pages.requested_page_count.saturating_add(1);
        self.pages.metadata_request_count = self.pages.metadata_request_count.saturating_add(1);
        self.pages.meshlet_bake_check_count = self.pages.meshlet_bake_check_count.saturating_add(1);
        self.pages.bounds_registration_count =
            self.pages.bounds_registration_count.saturating_add(1);
        self.geometry_pages.page_record_count =
            self.geometry_pages.page_record_count.saturating_add(1);
        self.geometry_pages.dirty_page_record_count = self
            .geometry_pages
            .dirty_page_record_count
            .saturating_add(1);
        self.pages.highest_priority = self
            .pages
            .highest_priority
            .max(page_priority(authoring.page_priority));
        self.revisions.page_revision = self.revisions.page_revision.saturating_add(1);
    }

    pub fn record_transform_change(&mut self, transform: &Transform) {
        self.instances.previous_transform_signature = self.instances.current_transform_signature;
        self.instances.current_transform_signature = transform_signature(transform);
        self.transforms.previous_transform_signature = self.transforms.current_transform_signature;
        self.transforms.current_transform_signature = self.instances.current_transform_signature;
        self.transforms.dirty_transform_count =
            self.transforms.dirty_transform_count.saturating_add(1);
        self.instances.motion_vector_update_count =
            self.instances.motion_vector_update_count.saturating_add(1);
        self.instances.dirty_instance_count = self.instances.dirty_instance_count.saturating_add(1);
        self.pages.shadow_invalidation_count =
            self.pages.shadow_invalidation_count.saturating_add(1);
        self.shadow_pages.invalidated_shadow_page_count = self
            .shadow_pages
            .invalidated_shadow_page_count
            .saturating_add(1);
        self.revisions.instance_revision = self.revisions.instance_revision.saturating_add(1);
    }

    pub fn record_history_keyed_spawn(&mut self) {
        self.transforms.history_keyed_transform_count = self
            .transforms
            .history_keyed_transform_count
            .saturating_add(1);
        self.geometry_pages.history_keyed_page_count = self
            .geometry_pages
            .history_keyed_page_count
            .saturating_add(1);
        self.shadow_pages.history_keyed_shadow_page_count = self
            .shadow_pages
            .history_keyed_shadow_page_count
            .saturating_add(1);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuInstanceTable {
    pub instance_count: u32,
    pub dirty_instance_count: u32,
    pub removed_instance_count: u32,
    pub compacted_instance_count: u32,
    pub motion_vector_update_count: u32,
    pub previous_transform_signature: u64,
    pub current_transform_signature: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuTransformTable {
    pub transform_count: u32,
    pub dirty_transform_count: u32,
    pub compacted_transform_count: u32,
    pub history_keyed_transform_count: u32,
    pub previous_transform_signature: u64,
    pub current_transform_signature: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuMaterialTable {
    pub material_count: u32,
    pub dirty_material_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuGeometryTable {
    pub geometry_count: u32,
    pub dirty_geometry_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuGeometryPageTable {
    pub page_record_count: u32,
    pub dirty_page_record_count: u32,
    pub compacted_page_record_count: u32,
    pub history_keyed_page_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuLightTable {
    pub light_count: u32,
    pub dirty_light_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuShadowPageTable {
    pub shadow_page_count: u32,
    pub dirty_shadow_page_count: u32,
    pub invalidated_shadow_page_count: u32,
    pub history_keyed_shadow_page_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuPageTable {
    pub resident_page_count: u32,
    pub requested_page_count: u32,
    pub metadata_request_count: u32,
    pub meshlet_bake_check_count: u32,
    pub bounds_registration_count: u32,
    pub released_page_reference_count: u32,
    pub shadow_invalidation_count: u32,
    pub gi_invalidation_count: u32,
    pub highest_priority: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuSceneRevisionTable {
    pub scene_revision: u64,
    pub instance_revision: u64,
    pub material_revision: u64,
    pub geometry_revision: u64,
    pub light_revision: u64,
    pub page_revision: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FrameGraph {
    pub revision: u64,
    pub node_count: u16,
    pub cef_gpu_import_nodes: u16,
    pub ui_composite_nodes: u16,
    pub super_resolution_nodes: u16,
    pub gi_nodes: u16,
    pub virtual_geometry_nodes: u16,
    pub compiled: bool,
}

impl FrameGraph {
    pub fn compile_from_rule(&mut self, node: FrameGraphNodeKind) {
        self.revision = self.revision.saturating_add(1);
        self.node_count = self.node_count.saturating_add(1);
        self.compiled = true;
        match node {
            FrameGraphNodeKind::CefGpuImport => {
                self.cef_gpu_import_nodes = self.cef_gpu_import_nodes.saturating_add(1);
            }
            FrameGraphNodeKind::UiComposite => {
                self.ui_composite_nodes = self.ui_composite_nodes.saturating_add(1);
            }
            FrameGraphNodeKind::DlssSuperResolution | FrameGraphNodeKind::FsrSuperResolution => {
                self.super_resolution_nodes = self.super_resolution_nodes.saturating_add(1);
            }
            FrameGraphNodeKind::HybridGi => {
                self.gi_nodes = self.gi_nodes.saturating_add(1);
            }
            FrameGraphNodeKind::VirtualGeometry => {
                self.virtual_geometry_nodes = self.virtual_geometry_nodes.saturating_add(1);
            }
            FrameGraphNodeKind::Present => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererDeltaKind {
    TransformChanged,
    VisibilityFlagsChanged,
    GeometryRefChanged,
    MaterialRefChanged,
    LightRefChanged,
    SceneChunkLoaded,
    SceneChunkUnloaded,
    EditorSalienceChanged,
    VirtualGeometryAdded,
    RenderableRemoved,
    CefSurfaceChanged,
    ViewportChanged,
    UpscalePolicyChanged,
}

impl RendererDeltaKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TransformChanged => "transform_changed",
            Self::VisibilityFlagsChanged => "visibility_flags_changed",
            Self::GeometryRefChanged => "geometry_ref_changed",
            Self::MaterialRefChanged => "material_ref_changed",
            Self::LightRefChanged => "light_ref_changed",
            Self::SceneChunkLoaded => "scene_chunk_loaded",
            Self::SceneChunkUnloaded => "scene_chunk_unloaded",
            Self::EditorSalienceChanged => "editor_salience_changed",
            Self::VirtualGeometryAdded => "virtual_geometry_added",
            Self::RenderableRemoved => "renderable_removed",
            Self::CefSurfaceChanged => "cef_surface_changed",
            Self::ViewportChanged => "viewport_changed",
            Self::UpscalePolicyChanged => "upscale_policy_changed",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct ExtractedSceneDeltas {
    pub changed_transforms: u32,
    pub changed_visibility_flags: u32,
    pub changed_geometry_refs: u32,
    pub changed_material_refs: u32,
    pub changed_light_refs: u32,
    pub loaded_chunks: u32,
    pub unloaded_chunks: u32,
    pub changed_editor_salience: u32,
    pub added_virtual_geometry: u32,
    pub removed_renderables: u32,
    pub changed_cef_surfaces: u32,
    pub changed_viewports: u32,
    pub changed_upscale_policies: u32,
    pub full_scene_graph_clone_count: u32,
}

impl ExtractedSceneDeltas {
    pub fn record(&mut self, kind: RendererDeltaKind) {
        match kind {
            RendererDeltaKind::TransformChanged => {
                self.changed_transforms = self.changed_transforms.saturating_add(1);
            }
            RendererDeltaKind::VisibilityFlagsChanged => {
                self.changed_visibility_flags = self.changed_visibility_flags.saturating_add(1);
            }
            RendererDeltaKind::GeometryRefChanged => {
                self.changed_geometry_refs = self.changed_geometry_refs.saturating_add(1);
            }
            RendererDeltaKind::MaterialRefChanged => {
                self.changed_material_refs = self.changed_material_refs.saturating_add(1);
            }
            RendererDeltaKind::LightRefChanged => {
                self.changed_light_refs = self.changed_light_refs.saturating_add(1);
            }
            RendererDeltaKind::SceneChunkLoaded => {
                self.loaded_chunks = self.loaded_chunks.saturating_add(1);
            }
            RendererDeltaKind::SceneChunkUnloaded => {
                self.unloaded_chunks = self.unloaded_chunks.saturating_add(1);
            }
            RendererDeltaKind::EditorSalienceChanged => {
                self.changed_editor_salience = self.changed_editor_salience.saturating_add(1);
            }
            RendererDeltaKind::VirtualGeometryAdded => {
                self.added_virtual_geometry = self.added_virtual_geometry.saturating_add(1);
            }
            RendererDeltaKind::RenderableRemoved => {
                self.removed_renderables = self.removed_renderables.saturating_add(1);
            }
            RendererDeltaKind::CefSurfaceChanged => {
                self.changed_cef_surfaces = self.changed_cef_surfaces.saturating_add(1);
            }
            RendererDeltaKind::ViewportChanged => {
                self.changed_viewports = self.changed_viewports.saturating_add(1);
            }
            RendererDeltaKind::UpscalePolicyChanged => {
                self.changed_upscale_policies = self.changed_upscale_policies.saturating_add(1);
            }
        }
    }

    #[must_use]
    pub const fn uses_full_scene_graph_clones(self) -> bool {
        self.full_scene_graph_clone_count != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphComponentTrigger {
    CefSurface,
    UpscalePolicyDlss,
    UpscalePolicyFsr,
    LuxGiModeHybrid,
    VirtualGeometryAuthoring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphNodeKind {
    CefGpuImport,
    UiComposite,
    DlssSuperResolution,
    FsrSuperResolution,
    HybridGi,
    VirtualGeometry,
    Present,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGraphComponentRule {
    pub trigger: FrameGraphComponentTrigger,
    pub node: FrameGraphNodeKind,
}

pub const FRAME_GRAPH_COMPONENT_RULES: [FrameGraphComponentRule; 6] = [
    FrameGraphComponentRule {
        trigger: FrameGraphComponentTrigger::CefSurface,
        node: FrameGraphNodeKind::CefGpuImport,
    },
    FrameGraphComponentRule {
        trigger: FrameGraphComponentTrigger::CefSurface,
        node: FrameGraphNodeKind::UiComposite,
    },
    FrameGraphComponentRule {
        trigger: FrameGraphComponentTrigger::UpscalePolicyDlss,
        node: FrameGraphNodeKind::DlssSuperResolution,
    },
    FrameGraphComponentRule {
        trigger: FrameGraphComponentTrigger::UpscalePolicyFsr,
        node: FrameGraphNodeKind::FsrSuperResolution,
    },
    FrameGraphComponentRule {
        trigger: FrameGraphComponentTrigger::LuxGiModeHybrid,
        node: FrameGraphNodeKind::HybridGi,
    },
    FrameGraphComponentRule {
        trigger: FrameGraphComponentTrigger::VirtualGeometryAuthoring,
        node: FrameGraphNodeKind::VirtualGeometry,
    },
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum LuxGiMode {
    Off,
    ProbeOnly,
    #[default]
    Hybrid,
}

fn transform_signature(transform: &Transform) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for bits in [
        transform.translation.x.to_bits(),
        transform.translation.y.to_bits(),
        transform.translation.z.to_bits(),
        transform.rotation.x.to_bits(),
        transform.rotation.y.to_bits(),
        transform.rotation.z.to_bits(),
        transform.rotation.w.to_bits(),
        transform.scale.x.to_bits(),
        transform.scale.y.to_bits(),
        transform.scale.z.to_bits(),
    ] {
        hash ^= u64::from(bits);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRendererPageAllocator {
    pub resident_pages: u32,
    pub pending_faults: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRendererUploadArena {
    pub schema_version: u16,
    pub resource_kind: RendererResourceKind,
    pub ownership_phase: ResourceOwnershipPhase,
    pub frame_index: u64,
    pub bytes_reserved: u64,
    pub bytes_written: u64,
    pub diagnostics: ResourceFrameAllocationDiagnostics,
}

impl Default for FunRendererUploadArena {
    fn default() -> Self {
        Self {
            schema_version: RENDERER_RESOURCE_SCHEMA_VERSION,
            resource_kind: RendererResourceKind::Upload(UploadResourceKind::StagingBufferPages),
            ownership_phase: ResourceOwnershipPhase::RendererOwnedPolicy,
            frame_index: 0,
            bytes_reserved: 0,
            bytes_written: 0,
            diagnostics: ResourceFrameAllocationDiagnostics::default(),
        }
    }
}

impl FunRendererUploadArena {
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.bytes_reserved = 0;
        self.bytes_written = 0;
        self.diagnostics.begin_frame(frame_index);
    }

    pub fn record_upload_allocation(
        &mut self,
        stable_id: &'static str,
        kind: RendererResourceKind,
        bytes_reserved: u64,
        bytes_written: u64,
        allocations: u32,
    ) {
        self.bytes_reserved = self.bytes_reserved.saturating_add(bytes_reserved);
        self.bytes_written = self.bytes_written.saturating_add(bytes_written);
        self.diagnostics
            .record_allocation(stable_id, kind, bytes_reserved, allocations);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRenderCapabilityMatrix {
    pub dx12: bool,
    pub vulkan: bool,
    pub super_resolution: bool,
    pub frame_generation: bool,
    pub cef_gpu_shared_textures: bool,
}

impl Default for FunRenderCapabilityMatrix {
    fn default() -> Self {
        Self {
            dx12: cfg!(target_os = "windows"),
            vulkan: true,
            super_resolution: false,
            frame_generation: false,
            cef_gpu_shared_textures: cfg!(target_os = "windows"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunPipelineRegistry {
    pub schema_version: u16,
    pub pipeline_count: u32,
    pub shader_module_count: u32,
    pub shader_variant_count: u32,
    pub dirty_pipeline_count: u32,
    pub runtime_counters: PipelineRuntimeCounters,
}

impl Default for FunPipelineRegistry {
    fn default() -> Self {
        let registry = PipelineRegistry::default();
        Self {
            schema_version: PIPELINE_REGISTRY_SCHEMA_VERSION,
            pipeline_count: registry.pipeline_count() as u32,
            shader_module_count: registry.shader_module_count() as u32,
            shader_variant_count: registry.shader_variant_count() as u32,
            dirty_pipeline_count: 0,
            runtime_counters: PipelineRuntimeCounters::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunSceneManifestRegistry {
    pub manifest_count: u32,
    pub latest_signature: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunViewportRegistry {
    pub viewport_count: u16,
    pub super_resolution_capable_count: u16,
    pub frame_generation_capable_count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRendererEcsSchedulePolicy {
    pub main_world_is_fun_scene_authored: bool,
    pub extraction_uses_bevy_ecs: bool,
    pub extraction_copies_compact_deltas: bool,
    pub render_world_uses_ecs_archetypes: bool,
    pub gpu_scene_database_uses_components: bool,
    pub frame_graph_uses_schedule_sets: bool,
    pub backend: FunRendererBackend,
}

impl FunRendererEcsSchedulePolicy {
    pub const DEFAULT: Self = Self {
        main_world_is_fun_scene_authored: true,
        extraction_uses_bevy_ecs: true,
        extraction_copies_compact_deltas: true,
        render_world_uses_ecs_archetypes: true,
        gpu_scene_database_uses_components: true,
        frame_graph_uses_schedule_sets: true,
        backend: default_backend_for_target(),
    };
}

impl Default for FunRendererEcsSchedulePolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererEcsEventKind {
    SceneSpawned,
    ScenePatched,
    ChunkLoaded,
    ChunkUnloaded,
    GeometryChanged,
    MaterialChanged,
    LightChanged,
    TransformChanged,
    PageFault,
    ShadowInvalidated,
    GiCacheInvalidated,
    UpscalerReset,
    DeviceLost,
    DeviceRestored,
    CefGpuFrameAvailable,
}

impl FunRendererEcsEventKind {
    pub const ALL: [Self; 15] = [
        Self::SceneSpawned,
        Self::ScenePatched,
        Self::ChunkLoaded,
        Self::ChunkUnloaded,
        Self::GeometryChanged,
        Self::MaterialChanged,
        Self::LightChanged,
        Self::TransformChanged,
        Self::PageFault,
        Self::ShadowInvalidated,
        Self::GiCacheInvalidated,
        Self::UpscalerReset,
        Self::DeviceLost,
        Self::DeviceRestored,
        Self::CefGpuFrameAvailable,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SceneSpawned => "scene_spawned",
            Self::ScenePatched => "scene_patched",
            Self::ChunkLoaded => "chunk_loaded",
            Self::ChunkUnloaded => "chunk_unloaded",
            Self::GeometryChanged => "geometry_changed",
            Self::MaterialChanged => "material_changed",
            Self::LightChanged => "light_changed",
            Self::TransformChanged => "transform_changed",
            Self::PageFault => "page_fault",
            Self::ShadowInvalidated => "shadow_invalidated",
            Self::GiCacheInvalidated => "gi_cache_invalidated",
            Self::UpscalerReset => "upscaler_reset",
            Self::DeviceLost => "device_lost",
            Self::DeviceRestored => "device_restored",
            Self::CefGpuFrameAvailable => "cef_gpu_frame_available",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub struct FunRendererEcsEvent {
    pub kind: FunRendererEcsEventKind,
    pub entity: Option<RenderableIdentity>,
    pub revision: u64,
}

impl FunRendererEcsEvent {
    #[must_use]
    pub const fn new(
        kind: FunRendererEcsEventKind,
        entity: Option<RenderableIdentity>,
        revision: u64,
    ) -> Self {
        Self {
            kind,
            entity,
            revision,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererChangeSource {
    Transform,
    Material,
    Geometry,
    Light,
    ScenePatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunRendererChangeTarget {
    MotionData,
    MaterialTable,
    PageResidency,
    LuxCandidateTables,
    ShadowInvalidation,
    ManifestSignatures,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRendererChangeDetectionRule {
    pub source: FunRendererChangeSource,
    pub target: FunRendererChangeTarget,
}

pub const FUN_RENDERER_CHANGE_DETECTION_RULES: [FunRendererChangeDetectionRule; 6] = [
    FunRendererChangeDetectionRule {
        source: FunRendererChangeSource::Transform,
        target: FunRendererChangeTarget::MotionData,
    },
    FunRendererChangeDetectionRule {
        source: FunRendererChangeSource::Material,
        target: FunRendererChangeTarget::MaterialTable,
    },
    FunRendererChangeDetectionRule {
        source: FunRendererChangeSource::Geometry,
        target: FunRendererChangeTarget::PageResidency,
    },
    FunRendererChangeDetectionRule {
        source: FunRendererChangeSource::Light,
        target: FunRendererChangeTarget::LuxCandidateTables,
    },
    FunRendererChangeDetectionRule {
        source: FunRendererChangeSource::Light,
        target: FunRendererChangeTarget::ShadowInvalidation,
    },
    FunRendererChangeDetectionRule {
        source: FunRendererChangeSource::ScenePatch,
        target: FunRendererChangeTarget::ManifestSignatures,
    },
];

pub fn extract_scene_entities(
    query: Query<(), Added<Renderable>>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for _ in query.iter() {
        deltas.record(RendererDeltaKind::SceneChunkLoaded);
    }
}

pub fn extract_renderer_components(
    query: Query<&Renderable, Changed<Renderable>>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for renderable in query.iter() {
        if renderable.geometry.is_valid() {
            deltas.record(RendererDeltaKind::GeometryRefChanged);
        }
        if renderable.material.is_valid() {
            deltas.record(RendererDeltaKind::MaterialRefChanged);
        }
    }
}

pub fn extract_lux_components(
    query: Query<&fun_scene::LuxLight, Changed<fun_scene::LuxLight>>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for _ in query.iter() {
        deltas.record(RendererDeltaKind::LightRefChanged);
    }
}

pub fn extract_viewports(
    query: Query<&ViewportRenderPolicy, Changed<ViewportRenderPolicy>>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for _ in query.iter() {
        deltas.record(RendererDeltaKind::ViewportChanged);
    }
}

pub fn extract_cef_surfaces(
    query: Query<&CefSurface, Changed<CefSurface>>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for _ in query.iter() {
        deltas.record(RendererDeltaKind::CefSurfaceChanged);
    }
}

pub fn apply_scene_deltas_to_gpu_scene(
    added: Query<(&Renderable, &Transform, Option<&SceneStableHistoryKey>), Added<Renderable>>,
    mut removed: RemovedComponents<Renderable>,
    mut gpu_scene: ResMut<GpuScene>,
    mut stable_history: Option<ResMut<StableHistoryTable>>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for (renderable, transform, stable_key) in added.iter() {
        let instance = RendererInstanceId::new(gpu_scene.instances.instance_count);
        gpu_scene.record_static_renderable(renderable, transform);
        if let Some(stable_key) = stable_key {
            gpu_scene.record_history_keyed_spawn();
            if let Some(stable_history) = stable_history.as_deref_mut() {
                let stable_id = StableHistoryId::from_scene_key(*stable_key);
                for table in HistoryTableKind::ALL {
                    stable_history.note_spawn(
                        stable_id,
                        table,
                        instance,
                        HistoryRespawnPolicy::Recover,
                    );
                }
            }
        }
    }
    for _ in removed.read() {
        gpu_scene.record_removed_renderable();
        deltas.record(RendererDeltaKind::RenderableRemoved);
    }
}

pub fn update_instance_tables(
    query: Query<(), Changed<RenderableIdentity>>,
    mut gpu_scene: ResMut<GpuScene>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for _ in query.iter() {
        gpu_scene.instances.dirty_instance_count =
            gpu_scene.instances.dirty_instance_count.saturating_add(1);
        gpu_scene.revisions.instance_revision =
            gpu_scene.revisions.instance_revision.saturating_add(1);
        deltas.record(RendererDeltaKind::VisibilityFlagsChanged);
    }
}

pub fn update_material_tables(
    query: Query<&Renderable, Changed<Renderable>>,
    mut gpu_scene: ResMut<GpuScene>,
) {
    for renderable in query.iter() {
        if renderable.material.is_valid() {
            gpu_scene.record_material_patch();
        }
    }
}

pub fn update_light_tables(
    query: Query<&fun_scene::LuxLight, Changed<fun_scene::LuxLight>>,
    mut gpu_scene: ResMut<GpuScene>,
) {
    for _ in query.iter() {
        gpu_scene.record_light_patch();
    }
}

pub fn update_page_requests(
    query: Query<&VirtualGeometryAuthoring, Added<VirtualGeometryAuthoring>>,
    mut gpu_scene: ResMut<GpuScene>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for authoring in query.iter() {
        gpu_scene.record_virtual_geometry_authoring(authoring);
        deltas.record(RendererDeltaKind::VirtualGeometryAdded);
    }
}

pub fn update_motion_vectors(
    query: Query<&Transform, Changed<Transform>>,
    mut gpu_scene: ResMut<GpuScene>,
    mut deltas: ResMut<ExtractedSceneDeltas>,
) {
    for transform in query.iter() {
        gpu_scene.record_transform_change(transform);
        deltas.record(RendererDeltaKind::TransformChanged);
    }
}

pub fn build_frame_graph(
    cef_surfaces: Query<&CefSurface>,
    upscale_policies: Query<&UpscalePolicy>,
    gi_modes: Query<&LuxGiMode>,
    virtual_geometry: Query<&VirtualGeometryAuthoring>,
    mut frame_graph: ResMut<FrameGraph>,
) {
    *frame_graph = FrameGraph::default();

    if !cef_surfaces.is_empty() {
        frame_graph.compile_from_rule(FrameGraphNodeKind::CefGpuImport);
        frame_graph.compile_from_rule(FrameGraphNodeKind::UiComposite);
    }

    if upscale_policies
        .iter()
        .any(|policy| policy.sr == SuperResolutionMode::Dlss)
    {
        frame_graph.compile_from_rule(FrameGraphNodeKind::DlssSuperResolution);
    }

    if upscale_policies
        .iter()
        .any(|policy| policy.sr == SuperResolutionMode::Fsr)
    {
        frame_graph.compile_from_rule(FrameGraphNodeKind::FsrSuperResolution);
    }

    if gi_modes.iter().any(|mode| *mode == LuxGiMode::Hybrid) {
        frame_graph.compile_from_rule(FrameGraphNodeKind::HybridGi);
    }

    if !virtual_geometry.is_empty() {
        frame_graph.compile_from_rule(FrameGraphNodeKind::VirtualGeometry);
    }

    frame_graph.compile_from_rule(FrameGraphNodeKind::Present);
}

pub fn execute_frame_graph(mut frame_graph: ResMut<FrameGraph>) {
    frame_graph.compiled = true;
}

pub fn collect_diagnostics() {}

pub fn build_hzb() {}

pub fn cull_static_virtual_geometry() {}

pub fn cull_dynamic_renderables() {}

pub fn compact_visible_clusters() {}

pub fn emit_visibility_feedback() {}

#[must_use]
pub const fn page_priority(priority: PagePriorityHint) -> u8 {
    match priority {
        PagePriorityHint::Low => 32,
        PagePriorityHint::Normal => 96,
        PagePriorityHint::High => 180,
        PagePriorityHint::Critical | PagePriorityHint::WorldCritical => u8::MAX,
    }
}

#[must_use]
pub const fn default_backend_for_target() -> FunRendererBackend {
    if cfg!(target_os = "windows") {
        FunRendererBackend::Dx12
    } else if cfg!(target_os = "macos") {
        FunRendererBackend::Metal
    } else {
        FunRendererBackend::Vulkan
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::message::Messages;
    use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
    use bevy_ecs::world::World;
    use fun_lux::{LuxLight, LuxLightDatabase, LuxLightId, LuxLightKind};
    use fun_scene::{GeometryRef, MaterialRef, RenderableFlags};

    use super::*;

    #[test]
    fn renderer_core_has_working_bevy_ecs_surface() {
        let mut world = World::new();
        world.insert_resource(FunRendererEcsSchedulePolicy::DEFAULT);
        world.insert_resource(GpuScene::default());
        world.insert_resource(FrameGraph::default());
        world.insert_resource(LuxLightDatabase::default());

        let entity = world
            .spawn((
                RenderableIdentity::new(42),
                GeometryHandle(3),
                MaterialHandle(9),
                FunStaticRenderable,
                FunShadowCaster,
                FunRendererGpuSceneObject::new(
                    FunRendererGpuObjectId::new(7),
                    FunRendererSubsystem::GpuSceneDatabase,
                ),
                FunRendererFrameGraphNode::new(FunRendererFrameGraphStage::GpuSceneDatabase),
            ))
            .id();

        let scene_object = world
            .get::<FunRendererGpuSceneObject>(entity)
            .expect("spawned entity should carry renderer scene object component");
        assert!(scene_object.object_id.is_valid());
        assert_eq!(
            scene_object.subsystem,
            FunRendererSubsystem::GpuSceneDatabase
        );

        let node = world
            .get::<FunRendererFrameGraphNode>(entity)
            .expect("spawned entity should carry frame graph node component");
        assert_eq!(
            node.order_key,
            FunRendererFrameGraphStage::GpuSceneDatabase.order_key()
        );

        let policy = world.resource::<FunRendererEcsSchedulePolicy>();
        assert!(policy.main_world_is_fun_scene_authored);
        assert!(policy.extraction_uses_bevy_ecs);
        assert!(policy.extraction_copies_compact_deltas);
        assert!(policy.render_world_uses_ecs_archetypes);
        assert!(policy.gpu_scene_database_uses_components);
        assert!(policy.frame_graph_uses_schedule_sets);
    }

    #[test]
    fn renderer_events_are_bevy_ecs_messages() {
        let mut world = World::new();
        world.init_resource::<Messages<FunRendererEcsEvent>>();
        world
            .resource_mut::<Messages<FunRendererEcsEvent>>()
            .write(FunRendererEcsEvent::new(
                FunRendererEcsEventKind::TransformChanged,
                Some(RenderableIdentity::new(11)),
                4,
            ));

        assert_eq!(world.resource::<Messages<FunRendererEcsEvent>>().len(), 1);
        assert_eq!(FunRendererEcsEventKind::ALL.len(), 15);
    }

    #[test]
    fn renderer_sets_follow_extract_to_present_order() {
        let mut previous = 0;
        for set in FunRendererSet::ORDER {
            assert!(set.order_key() > previous, "{}", set.as_str());
            previous = set.order_key();
        }

        assert!(
            FunRendererSet::VirtualGeometry.order_key()
                < FunRendererSet::VirtualShadows.order_key()
        );
        assert!(FunRendererSet::Upscale.order_key() < FunRendererSet::UiComposite.order_key());
        assert!(FunRendererSet::Present.order_key() > FunRendererSet::Lux.order_key());
    }

    #[test]
    fn render_world_system_sets_cover_section_six_lanes() {
        assert_ordered(
            RendererExtractSet::ORDER
                .iter()
                .map(|set| (set.order_key(), set.as_str())),
        );
        assert_ordered(
            RendererPrepareSet::ORDER
                .iter()
                .map(|set| (set.order_key(), set.as_str())),
        );
        assert_ordered(
            RendererVisibilitySet::ORDER
                .iter()
                .map(|set| (set.order_key(), set.as_str())),
        );
        assert_ordered(
            RendererFrameSet::ORDER
                .iter()
                .map(|set| (set.order_key(), set.as_str())),
        );

        assert_eq!(
            RendererExtractSet::ExtractRendererComponents.as_str(),
            "extract_renderer_components"
        );
        assert_eq!(
            RendererPrepareSet::UpdateMotionVectors.as_str(),
            "update_motion_vectors"
        );
        assert_eq!(
            RendererFrameSet::BuildFrameGraph.as_str(),
            "build_frame_graph"
        );
    }

    #[test]
    fn archetype_policy_keeps_heavy_data_out_of_entity_tables() {
        let policy = core::hint::black_box(FUN_RENDERER_DATA_PLACEMENT_POLICY);

        assert!(policy.compact_handles_required_on_entities);
        assert!(!policy.large_material_data_on_entities_allowed);
        assert!(!policy.gpu_buffer_handles_on_many_entities_allowed);
        assert_eq!(policy.heavy_data_owner, "resources_assets_tables");
        assert_eq!(FunRendererArchetypeClass::HOT_PATH_ORDER.len(), 12);
        assert!(FunRendererArchetypeClass::EditorSelectable.sparse_component_ok());
        assert!(!FunRendererArchetypeClass::StaticRenderable.sparse_component_ok());
    }

    #[test]
    fn component_gpu_mapping_keeps_ecs_typed_and_gpu_tables_soa() {
        let layout = core::hint::black_box(GPU_DATA_LAYOUT_POLICY);

        assert!(layout.ecs_components_are_typed);
        assert!(layout.gpu_tables_are_soa);
        assert!(!layout.attach_heavy_gpu_objects_to_entities);
        assert_eq!(layout.heavyweight_owner, "resources_assets_tables");
        assert_eq!(
            layout.allowed_entity_references,
            &[
                EntityAttachedReferenceKind::GeometryRef,
                EntityAttachedReferenceKind::MaterialRef,
                EntityAttachedReferenceKind::LuxLightId,
                EntityAttachedReferenceKind::SceneStableIdentity,
                EntityAttachedReferenceKind::RendererInstanceId,
            ]
        );
        assert_eq!(GpuTableLayout::StructOfArrays.as_str(), "struct_of_arrays");
        assert_eq!(COMPONENT_GPU_MAPPINGS.len(), 5);
        assert!(COMPONENT_GPU_MAPPINGS.iter().all(|mapping| {
            mapping.ecs_component_is_typed_and_ergonomic
                && mapping.gpu_tables_are_compact_soa
                && !mapping.heavy_gpu_object_attached_to_entity
        }));
        assert!(
            COMPONENT_GPU_MAPPINGS
                .iter()
                .find(|mapping| mapping.component == EcsGpuSourceComponent::Renderable)
                .expect("renderable mapping should exist")
                .gpu_tables
                .contains(&GpuTableKind::Instance)
        );
        assert!(
            COMPONENT_GPU_MAPPINGS
                .iter()
                .find(|mapping| mapping.component == EcsGpuSourceComponent::SceneStableIdentity)
                .expect("stable identity mapping should exist")
                .gpu_tables
                .contains(&GpuTableKind::ShadowPage)
        );
    }

    #[test]
    fn render_world_can_host_lux_components_without_renderer_owning_lighting() {
        let mut world = World::new();
        world.insert_resource(LuxLightDatabase::with_revision(5));
        let entity = world
            .spawn((
                LuxLight::new(
                    LuxLightId::new(9),
                    LuxLightKind::EmissiveCandidate,
                    400.0,
                    true,
                ),
                FunGameplaySalient,
            ))
            .id();

        let light = world
            .get::<LuxLight>(entity)
            .expect("lux light should be an ECS component");
        assert_eq!(light.kind, LuxLightKind::EmissiveCandidate);
        assert!(world.resource::<LuxLightDatabase>().revision == 5);
    }

    #[test]
    fn change_detection_rules_cover_required_renderer_mutations() {
        assert_eq!(FUN_RENDERER_CHANGE_DETECTION_RULES.len(), 6);
        assert!(
            FUN_RENDERER_CHANGE_DETECTION_RULES
                .iter()
                .any(|rule| rule.source == FunRendererChangeSource::Transform
                    && rule.target == FunRendererChangeTarget::MotionData)
        );
        assert!(
            FUN_RENDERER_CHANGE_DETECTION_RULES
                .iter()
                .any(|rule| rule.source == FunRendererChangeSource::Light
                    && rule.target == FunRendererChangeTarget::ShadowInvalidation)
        );
        assert!(
            FUN_RENDERER_CHANGE_DETECTION_RULES
                .iter()
                .any(|rule| rule.source == FunRendererChangeSource::ScenePatch
                    && rule.target == FunRendererChangeTarget::ManifestSignatures)
        );
    }

    #[test]
    fn static_scene_spawned_with_fun_macro_components_updates_gpu_scene() {
        let mut world = World::new();
        world.insert_resource(GpuScene::default());
        world.insert_resource(ExtractedSceneDeltas::default());
        world.spawn((
            Renderable::new(
                GeometryRef::new(3),
                MaterialRef::new(9),
                RenderableFlags::STATIC,
            ),
            Transform::from_xyz(1.0, 2.0, 3.0),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                extract_scene_entities,
                extract_renderer_components,
                apply_scene_deltas_to_gpu_scene,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let gpu_scene = world.resource::<GpuScene>();
        assert_eq!(gpu_scene.instances.instance_count, 1);
        assert_eq!(gpu_scene.transforms.transform_count, 1);
        assert_eq!(gpu_scene.materials.material_count, 1);
        assert_eq!(gpu_scene.geometry.geometry_count, 1);
        assert_eq!(gpu_scene.geometry_pages.page_record_count, 1);
        assert_ne!(gpu_scene.instances.current_transform_signature, 0);
        assert!(
            !world
                .resource::<ExtractedSceneDeltas>()
                .uses_full_scene_graph_clones()
        );
    }

    #[test]
    fn removed_scene_entity_removes_gpu_instance_entry() {
        let mut world = World::new();
        world.insert_resource(GpuScene::default());
        world.insert_resource(ExtractedSceneDeltas::default());
        let entity = world
            .spawn((
                Renderable::new(
                    GeometryRef::new(1),
                    MaterialRef::new(1),
                    RenderableFlags::STATIC,
                ),
                Transform::IDENTITY,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_scene_deltas_to_gpu_scene);
        schedule.run(&mut world);
        assert_eq!(world.resource::<GpuScene>().instances.instance_count, 1);

        let _ = world.despawn(entity);
        schedule.run(&mut world);

        let gpu_scene = world.resource::<GpuScene>();
        assert_eq!(gpu_scene.instances.instance_count, 0);
        assert_eq!(gpu_scene.instances.removed_instance_count, 1);
        assert_eq!(gpu_scene.instances.compacted_instance_count, 1);
        assert_eq!(gpu_scene.transforms.compacted_transform_count, 1);
        assert_eq!(gpu_scene.geometry_pages.compacted_page_record_count, 1);
        assert_eq!(gpu_scene.pages.released_page_reference_count, 1);
        assert_eq!(gpu_scene.pages.shadow_invalidation_count, 1);
        assert_eq!(gpu_scene.pages.gi_invalidation_count, 1);
        assert_eq!(gpu_scene.shadow_pages.invalidated_shadow_page_count, 1);
        assert_eq!(
            world.resource::<ExtractedSceneDeltas>().removed_renderables,
            1
        );
    }

    #[test]
    fn material_patch_updates_material_table_only() {
        let mut world = World::new();
        world.insert_resource(GpuScene {
            instances: GpuInstanceTable {
                instance_count: 1,
                dirty_instance_count: 0,
                removed_instance_count: 0,
                compacted_instance_count: 0,
                motion_vector_update_count: 0,
                previous_transform_signature: 0,
                current_transform_signature: 0,
            },
            revisions: GpuSceneRevisionTable {
                instance_revision: 7,
                ..Default::default()
            },
            ..Default::default()
        });
        let entity = world
            .spawn(Renderable::new(
                GeometryRef::new(1),
                MaterialRef::new(1),
                RenderableFlags::STATIC,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_material_tables);
        schedule.run(&mut world);
        {
            let mut gpu_scene = world.resource_mut::<GpuScene>();
            gpu_scene.materials.dirty_material_count = 0;
            gpu_scene.revisions.material_revision = 0;
            gpu_scene.revisions.instance_revision = 7;
        }

        world
            .entity_mut(entity)
            .get_mut::<Renderable>()
            .expect("renderable should exist")
            .material = MaterialRef::new(4);
        schedule.run(&mut world);

        let gpu_scene = world.resource::<GpuScene>();
        assert_eq!(gpu_scene.instances.instance_count, 1);
        assert_eq!(gpu_scene.revisions.instance_revision, 7);
        assert_eq!(gpu_scene.revisions.geometry_revision, 0);
        assert_eq!(gpu_scene.materials.dirty_material_count, 1);
        assert_eq!(gpu_scene.revisions.material_revision, 1);
    }

    #[test]
    fn moving_entity_updates_previous_current_transform_for_motion_vectors() {
        let mut world = World::new();
        world.insert_resource(GpuScene::default());
        world.insert_resource(ExtractedSceneDeltas::default());
        let entity = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_motion_vectors);
        schedule.run(&mut world);
        let first_signature = world
            .resource::<GpuScene>()
            .instances
            .current_transform_signature;

        world
            .entity_mut(entity)
            .get_mut::<Transform>()
            .expect("transform should exist")
            .translation
            .x = 12.0;
        schedule.run(&mut world);

        let gpu_scene = world.resource::<GpuScene>();
        assert_eq!(
            gpu_scene.instances.previous_transform_signature,
            first_signature
        );
        assert_eq!(
            gpu_scene.transforms.previous_transform_signature,
            first_signature
        );
        assert_ne!(
            gpu_scene.instances.current_transform_signature,
            first_signature
        );
        assert_eq!(
            gpu_scene.transforms.current_transform_signature,
            gpu_scene.instances.current_transform_signature
        );
        assert_eq!(gpu_scene.instances.motion_vector_update_count, 2);
        assert_eq!(gpu_scene.transforms.dirty_transform_count, 2);
        assert_eq!(gpu_scene.pages.shadow_invalidation_count, 2);
        assert_eq!(gpu_scene.shadow_pages.invalidated_shadow_page_count, 2);
        assert_eq!(
            world.resource::<ExtractedSceneDeltas>().changed_transforms,
            2
        );
    }

    #[test]
    fn virtual_geometry_lights_ui_and_upscale_compile_frame_graph_from_ecs() {
        let mut world = World::new();
        world.insert_resource(GpuScene::default());
        world.insert_resource(FrameGraph::default());
        world.insert_resource(ExtractedSceneDeltas::default());
        world.spawn((
            VirtualGeometryAuthoring {
                mode: VirtualGeometryMode::StaticClusterPages,
                page_priority: PagePriorityHint::WorldCritical,
                dynamic_policy: fun_scene::DynamicGeometryPolicy::StaticOnly,
            },
            CefSurface::default(),
            UpscalePolicy {
                sr: SuperResolutionMode::Dlss,
                ..Default::default()
            },
            LuxGiMode::Hybrid,
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                update_page_requests,
                extract_cef_surfaces,
                build_frame_graph,
                execute_frame_graph,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let gpu_scene = world.resource::<GpuScene>();
        assert_eq!(gpu_scene.pages.requested_page_count, 1);
        assert_eq!(gpu_scene.pages.metadata_request_count, 1);
        assert_eq!(gpu_scene.pages.meshlet_bake_check_count, 1);
        assert_eq!(gpu_scene.pages.bounds_registration_count, 1);
        assert_eq!(gpu_scene.geometry_pages.page_record_count, 1);
        assert_eq!(gpu_scene.geometry_pages.dirty_page_record_count, 1);
        assert_eq!(gpu_scene.pages.highest_priority, u8::MAX);
        let frame_graph = world.resource::<FrameGraph>();
        assert!(frame_graph.compiled);
        assert_eq!(frame_graph.cef_gpu_import_nodes, 1);
        assert_eq!(frame_graph.ui_composite_nodes, 1);
        assert_eq!(frame_graph.super_resolution_nodes, 1);
        assert_eq!(frame_graph.gi_nodes, 1);
        assert_eq!(frame_graph.virtual_geometry_nodes, 1);
    }

    #[test]
    fn stable_history_recovers_or_resets_when_scene_identity_respawns() {
        let mut history = StableHistoryTable::default();
        let stable = StableHistoryId::new(42);
        let first_instance = RendererInstanceId::new(0);
        let recovered_instance = RendererInstanceId::new(7);
        let reset_instance = RendererInstanceId::new(8);

        history.note_spawn(
            stable,
            HistoryTableKind::MotionVectors,
            first_instance,
            HistoryRespawnPolicy::Recover,
        );
        assert_eq!(
            history.generation(stable, HistoryTableKind::MotionVectors),
            Some(1)
        );
        history.note_despawn(stable);
        assert_eq!(
            history
                .record(stable, HistoryTableKind::MotionVectors)
                .expect("history should exist")
                .live_instance,
            None
        );

        history.note_spawn(
            stable,
            HistoryTableKind::MotionVectors,
            recovered_instance,
            HistoryRespawnPolicy::Recover,
        );
        let recovered = history
            .record(stable, HistoryTableKind::MotionVectors)
            .expect("history should recover");
        assert_eq!(recovered.generation, 1);
        assert_eq!(recovered.recover_count, 1);
        assert_eq!(recovered.live_instance, Some(recovered_instance));

        history.note_spawn(
            stable,
            HistoryTableKind::MotionVectors,
            reset_instance,
            HistoryRespawnPolicy::Reset,
        );
        let reset = history
            .record(stable, HistoryTableKind::MotionVectors)
            .expect("history should reset");
        assert_eq!(reset.generation, 2);
        assert_eq!(reset.reset_count, 1);
        assert_eq!(reset.live_instance, Some(reset_instance));
    }

    #[test]
    fn compacting_removed_gpu_instances_keeps_unrelated_stable_histories() {
        let mut history = StableHistoryTable::default();
        let removed = StableHistoryId::new(100);
        let unrelated = StableHistoryId::new(200);

        history.note_spawn(
            removed,
            HistoryTableKind::VirtualGeometryPages,
            RendererInstanceId::new(1),
            HistoryRespawnPolicy::Recover,
        );
        history.note_spawn(
            unrelated,
            HistoryTableKind::VirtualGeometryPages,
            RendererInstanceId::new(2),
            HistoryRespawnPolicy::Recover,
        );
        let unrelated_generation = history
            .generation(unrelated, HistoryTableKind::VirtualGeometryPages)
            .expect("unrelated history should exist");

        history.note_despawn(removed);
        history.note_compacted_removed_instance(RendererInstanceId::new(1));

        assert_eq!(history.compacted_removed_instances, 1);
        assert_eq!(history.unrelated_history_invalidations, 0);
        assert_eq!(
            history.generation(unrelated, HistoryTableKind::VirtualGeometryPages),
            Some(unrelated_generation)
        );
        assert_eq!(
            history
                .record(unrelated, HistoryTableKind::VirtualGeometryPages)
                .expect("unrelated history should remain live")
                .live_instance,
            Some(RendererInstanceId::new(2))
        );
    }

    fn assert_ordered<'a>(items: impl Iterator<Item = (u16, &'a str)>) {
        let mut previous = 0;
        for (order_key, label) in items {
            assert!(order_key > previous, "{label}");
            previous = order_key;
        }
    }
}
