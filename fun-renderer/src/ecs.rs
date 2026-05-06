use bevy_ecs::{
    prelude::{Component, Message, Resource},
    schedule::SystemSet,
};

use crate::{
    FunRendererBackend, FunRendererFrameGenerationContract, FunRendererFrameGraphStage,
    FunRendererRuntimeBackend, FunRendererSubsystem,
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
pub struct FunGpuSceneDatabase {
    pub revision: u64,
    pub object_count: u32,
    pub dirty_object_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunFrameGraph {
    pub revision: u64,
    pub node_count: u16,
    pub compiled: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRendererPageAllocator {
    pub resident_pages: u32,
    pub pending_faults: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRendererUploadArena {
    pub frame_index: u64,
    pub bytes_reserved: u64,
    pub bytes_written: u64,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunPipelineRegistry {
    pub pipeline_count: u32,
    pub dirty_pipeline_count: u32,
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

#[must_use]
pub const fn default_backend_for_target() -> FunRendererBackend {
    if cfg!(target_os = "windows") {
        FunRendererBackend::Dx12
    } else {
        FunRendererBackend::Vulkan
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::message::Messages;
    use bevy_ecs::world::World;
    use fun_lux::{LuxLight, LuxLightDatabase, LuxLightId, LuxLightKind};

    use super::*;

    #[test]
    fn renderer_core_has_working_bevy_ecs_surface() {
        let mut world = World::new();
        world.insert_resource(FunRendererEcsSchedulePolicy::DEFAULT);
        world.insert_resource(FunGpuSceneDatabase::default());
        world.insert_resource(FunFrameGraph::default());
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
}
