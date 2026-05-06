use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use fun_scene::prelude::*;
#[allow(deprecated)]
use fun_scene::{SceneRenderManifest, SceneRenderManifestProvider};
use game_shared::{
    ASSET_COVER_CUBE, ASSET_FLOOR, ASSET_FLOOR_COLLIDER, ASSET_RAMP, ASSET_WALL,
    COLLIDER_COVER_CUBE, COLLIDER_FLOOR, COLLIDER_RAMP, COLLIDER_WALL, DEMO_LEVEL_ID,
    MATERIAL_COVER, MATERIAL_FLOOR, MATERIAL_RAMP, MATERIAL_WALL,
};
use thunder::prelude::*;

pub const DEFAULT_SCENE_ID: SceneId = SceneId("arena-blockout");
const DEFAULT_SCENE_FUNCTION_NAME: &str = "spawn_default_scene";
const DEFAULT_RENDERABLE_ENTITY_COUNT: u32 = 6;
const DEFAULT_VIRTUAL_PAGE_PRIORITY: u8 = 180;
const FLOOR_ENTITY: NetEntity = NetEntity(1);
const FLOOR_COLLIDER_ENTITY: NetEntity = NetEntity(2);
const WALL_ENTITY: NetEntity = NetEntity(3);
const RAMP_ENTITY: NetEntity = NetEntity(4);
const COVER_A_ENTITY: NetEntity = NetEntity(5);
const COVER_B_ENTITY: NetEntity = NetEntity(6);
const COVER_C_ENTITY: NetEntity = NetEntity(7);

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultSceneManifestProvider;

impl SceneManifestProvider for DefaultSceneManifestProvider {
    fn scene_manifest(&self, revision: WorldRevision) -> SceneManifest {
        default_scene_manifest(revision)
    }
}

#[allow(deprecated)]
impl SceneRenderManifestProvider for DefaultSceneManifestProvider {
    fn scene_render_manifest(&self, revision: WorldRevision) -> SceneManifest {
        default_scene_manifest(revision)
    }
}

#[allow(deprecated)]
#[deprecated(note = "use DefaultSceneManifestProvider")]
pub type DefaultSceneRenderManifestProvider = DefaultSceneManifestProvider;

#[derive(Debug, Default, Clone, Component)]
pub struct StreamedWorldEntity {
    pub catalog: Option<WorldCatalogRef>,
    pub render: Option<WorldPrimitive>,
    pub collider: Option<WorldCollider>,
    pub color: Option<PackedColorRgba8>,
}

impl StreamedWorldEntity {
    #[must_use]
    pub const fn catalog(catalog: WorldCatalogRef) -> Self {
        Self {
            catalog: Some(catalog),
            render: None,
            collider: None,
            color: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SceneEntity {
    entity: NetEntity,
    name: &'static str,
    network: NetworkedSceneEntity,
    catalog: WorldCatalogRef,
    translation: Vec3,
    rotation_z_radians: f32,
}

impl SceneEntity {
    fn transform(self) -> Transform {
        Transform::from_translation(self.translation)
            .with_rotation(Quat::from_rotation_z(self.rotation_z_radians))
    }

    fn world_spec(self) -> WorldEntitySpec {
        WorldEntitySpec {
            entity: self.entity,
            name: self.name.to_owned(),
            class: self.network.class,
            authority: self.network.authority,
            transform: qtransform(&self.transform()),
            catalog: Some(self.catalog),
            render: None,
            collider: None,
            color: None,
        }
    }

    fn manifest(self) -> SceneEntityManifest {
        let spec = self.world_spec();
        SceneEntityManifest {
            stable_identity: spec.entity,
            name: spec.name,
            network: SceneNetworkManifest::from(self.network),
            transform: spec.transform,
            catalog: spec.catalog,
            render: spec.render,
            material_color: spec.color,
        }
    }
}

pub fn spawn_default_scene(mut commands: Commands) {
    commands.spawn_fun_scene_list(fun_list![
        (
            #Floor
            fun_value(NetworkedSceneEntity::world())
            fun_value(SceneStableIdentity(FLOOR_ENTITY))
            Renderable {
                geometry: GeometryRef({ASSET_FLOOR.0}),
                material: MaterialRef({MATERIAL_FLOOR.0}),
                flags: RenderableFlags::STATIC_WORLD
            }
            VirtualGeometryAuthoring {
                mode: VirtualGeometryMode::StaticClusterPages,
                page_priority: PagePriorityHint::WorldCritical,
                dynamic_policy: DynamicGeometryPolicy::StaticOnly
            }
            VirtualShadowCaster {
                policy: ShadowCasterPolicy::VirtualPages,
                invalidation: ShadowInvalidationPolicy::OnTransformOrGeometryChange
            }
            VirtualShadowReceiver {
                priority: ShadowReceiverPriority::High,
                filter_policy: ShadowFilterPolicy::ContactAware
            }
            LuxGiParticipant {
                bounce_policy: GiBouncePolicy::StaticSingleBounce,
                cache_policy: GiCachePolicy::Surface
            }
            RendererBounds { streaming_radius: 64.0 }
            fun_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_FLOOR.0,
                MATERIAL_FLOOR.0,
                0,
            )))
            Transform::default()
        ),
        (
            #FloorCollider
            Name::new("FloorCollider")
            fun_value(NetworkedSceneEntity::world())
            fun_value(SceneStableIdentity(FLOOR_COLLIDER_ENTITY))
            fun_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_FLOOR_COLLIDER.0,
                MATERIAL_FLOOR.0,
                COLLIDER_FLOOR.0,
            )))
            fun_value(RigidBody::Static)
            Collider::cuboid(60.0, 0.5, 60.0)
            Transform::from_xyz(0.0, -0.25, 0.0)
        ),
        (
            #Wall
            fun_value(NetworkedSceneEntity::world())
            fun_value(SceneStableIdentity(WALL_ENTITY))
            Renderable {
                geometry: GeometryRef({ASSET_WALL.0}),
                material: MaterialRef({MATERIAL_WALL.0}),
                flags: RenderableFlags::STATIC_WORLD
            }
            VirtualGeometryAuthoring {
                mode: VirtualGeometryMode::StaticClusterPages,
                page_priority: PagePriorityHint::WorldCritical,
                dynamic_policy: DynamicGeometryPolicy::StaticOnly
            }
            VirtualShadowCaster {
                policy: ShadowCasterPolicy::VirtualPages,
                invalidation: ShadowInvalidationPolicy::OnTransformOrGeometryChange
            }
            VirtualShadowReceiver {
                priority: ShadowReceiverPriority::High,
                filter_policy: ShadowFilterPolicy::ContactAware
            }
            LuxGiParticipant {
                bounce_policy: GiBouncePolicy::StaticSingleBounce,
                cache_policy: GiCachePolicy::Surface
            }
            RendererBounds { streaming_radius: 24.0 }
            fun_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_WALL.0,
                MATERIAL_WALL.0,
                COLLIDER_WALL.0,
            )))
            fun_value(RigidBody::Static)
            Collider::cuboid(5.0, 3.0, 1.0)
            Transform::from_xyz(0.0, 1.5, -8.0)
        ),
        (
            #Ramp
            fun_value(NetworkedSceneEntity::world())
            fun_value(SceneStableIdentity(RAMP_ENTITY))
            Renderable {
                geometry: GeometryRef({ASSET_RAMP.0}),
                material: MaterialRef({MATERIAL_RAMP.0}),
                flags: RenderableFlags::STATIC_WORLD
            }
            VirtualGeometryAuthoring {
                mode: VirtualGeometryMode::StaticClusterPages,
                page_priority: PagePriorityHint::WorldCritical,
                dynamic_policy: DynamicGeometryPolicy::StaticOnly
            }
            VirtualShadowCaster {
                policy: ShadowCasterPolicy::VirtualPages,
                invalidation: ShadowInvalidationPolicy::OnTransformOrGeometryChange
            }
            VirtualShadowReceiver {
                priority: ShadowReceiverPriority::High,
                filter_policy: ShadowFilterPolicy::ContactAware
            }
            LuxGiParticipant {
                bounce_policy: GiBouncePolicy::StaticSingleBounce,
                cache_policy: GiCachePolicy::Surface
            }
            RendererBounds { streaming_radius: 24.0 }
            fun_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_RAMP.0,
                MATERIAL_RAMP.0,
                COLLIDER_RAMP.0,
            )))
            fun_value(RigidBody::Static)
            Collider::cuboid(3.0, 0.5, 6.0)
            fun_value(Transform::from_xyz(-6.0, 0.25, -2.0)
                .with_rotation(Quat::from_rotation_z(-12.0_f32.to_radians())))
        ),
        demo_cube(COVER_A_ENTITY, Vec3::new(3.0, 1.0, 2.0)),
        demo_cube(COVER_B_ENTITY, Vec3::new(5.0, 1.0, -1.5)),
        demo_cube(COVER_C_ENTITY, Vec3::new(7.0, 2.0, 4.0)),
    ]);
}

fn demo_cube(entity: NetEntity, translation: Vec3) -> impl FunScene {
    fun! {
        fun_value(NetworkedSceneEntity::world())
        fun_value(SceneStableIdentity(entity))
        Renderable {
            geometry: GeometryRef({ASSET_COVER_CUBE.0}),
            material: MaterialRef({MATERIAL_COVER.0}),
            flags: RenderableFlags::STATIC_WORLD
        }
        VirtualGeometryAuthoring {
            mode: VirtualGeometryMode::StaticClusterPages,
            page_priority: PagePriorityHint::WorldCritical,
            dynamic_policy: DynamicGeometryPolicy::StaticOnly
        }
        VirtualShadowCaster {
            policy: ShadowCasterPolicy::VirtualPages,
            invalidation: ShadowInvalidationPolicy::OnTransformOrGeometryChange
        }
        VirtualShadowReceiver {
            priority: ShadowReceiverPriority::High,
            filter_policy: ShadowFilterPolicy::ContactAware
        }
        LuxGiParticipant {
            bounce_policy: GiBouncePolicy::StaticSingleBounce,
            cache_policy: GiCachePolicy::Surface
        }
        RendererBounds { streaming_radius: 8.0 }
        fun_value(StreamedWorldEntity::catalog(catalog_ref(
            ASSET_COVER_CUBE.0,
            MATERIAL_COVER.0,
            COLLIDER_COVER_CUBE.0,
        )))
        fun_value(RigidBody::Static)
        Collider::cuboid(1.0, 1.0, 1.0)
        fun_value(Transform::from_translation(translation))
    }
}

pub fn apply_scene_stable_identities(
    commands: Commands,
    query: Query<(Entity, &SceneStableIdentity, &NetworkedSceneEntity), Without<NetworkIdentity>>,
) {
    apply_networked_scene_identities(commands, query);
}

#[must_use]
pub fn scene_descriptors() -> [SceneDescriptor; 1] {
    [default_scene_descriptor()]
}

#[must_use]
pub fn scene_manifest(scene_id: SceneId, revision: WorldRevision) -> Option<SceneManifest> {
    if scene_id == DEFAULT_SCENE_ID {
        return Some(default_scene_manifest(revision));
    }
    None
}

#[must_use]
pub fn scene_manifest_by_id(scene_id: &str, revision: WorldRevision) -> Option<SceneManifest> {
    if scene_id == DEFAULT_SCENE_ID.0 {
        return Some(default_scene_manifest(revision));
    }
    None
}

#[allow(deprecated)]
#[must_use]
#[deprecated(note = "use scene_descriptors")]
pub fn scene_render_descriptors() -> [SceneDescriptor; 1] {
    scene_descriptors()
}

#[allow(deprecated)]
#[must_use]
#[deprecated(note = "use scene_manifest")]
pub fn scene_render_manifest(
    scene_id: SceneId,
    revision: WorldRevision,
) -> Option<SceneRenderManifest> {
    scene_manifest(scene_id, revision)
}

#[allow(deprecated)]
#[must_use]
#[deprecated(note = "use scene_manifest_by_id")]
pub fn scene_render_manifest_by_id(
    scene_id: &str,
    revision: WorldRevision,
) -> Option<SceneRenderManifest> {
    scene_manifest_by_id(scene_id, revision)
}

#[must_use]
pub fn default_scene_descriptor() -> SceneDescriptor {
    default_scene_manifest(WorldRevision(0)).descriptor()
}

#[must_use]
pub fn default_scene_manifest(revision: WorldRevision) -> SceneManifest {
    let specs = default_scene_world_specs();
    let entities = default_scene_entity_manifests();
    let signature = scene_manifest_signature(&entities);
    let chunks =
        fun_scene::chunk_world_specs_with_signature(DEMO_LEVEL_ID, revision, specs, signature.0);
    let stream_chunk_count = chunks.len() as u16;
    SceneManifest {
        id: DEFAULT_SCENE_ID,
        display_name: "Arena Blockout",
        scene_function_name: DEFAULT_SCENE_FUNCTION_NAME,
        signature,
        entities,
        chunks,
        renderer: SceneRendererManifest {
            render_entity_count: DEFAULT_RENDERABLE_ENTITY_COUNT,
            virtual_geometry_entity_count: DEFAULT_RENDERABLE_ENTITY_COUNT,
            stream_chunk_count,
            virtual_page_priority: DEFAULT_VIRTUAL_PAGE_PRIORITY,
            preview_camera: Some(ScenePreviewCamera {
                transform: qtransform(
                    &Transform::from_xyz(-8.0, 5.0, 10.0)
                        .looking_at(Vec3::new(1.0, 1.0, -1.0), Vec3::Y),
                ),
                vertical_fov_radians: 65.0_f32.to_radians(),
            }),
        },
        lux: SceneLuxManifest {
            light_count: 1,
            emissive_candidate_count: 0,
            gi_participant_count: DEFAULT_RENDERABLE_ENTITY_COUNT,
            virtual_shadow_participant_count: DEFAULT_RENDERABLE_ENTITY_COUNT,
            lighting: Some(SceneLightingDescriptor {
                sun_direction: [-0.4, -1.0, -0.35],
                sun_illuminance_lux: 25_000.0,
                ambient_rgb: [0.03, 0.035, 0.04],
            }),
        },
    }
}

#[allow(deprecated)]
#[must_use]
#[deprecated(note = "use default_scene_manifest")]
pub fn default_scene_render_manifest(revision: WorldRevision) -> SceneRenderManifest {
    default_scene_manifest(revision)
}

#[must_use]
pub fn default_scene_world_stream_chunks(revision: WorldRevision) -> Vec<SceneStreamChunk> {
    default_scene_manifest(revision).chunks
}

#[must_use]
pub fn default_scene_world_specs() -> Vec<WorldEntitySpec> {
    default_scene_entities()
        .into_iter()
        .map(SceneEntity::world_spec)
        .collect()
}

fn default_scene_entity_manifests() -> Vec<SceneEntityManifest> {
    default_scene_entities()
        .into_iter()
        .map(SceneEntity::manifest)
        .collect()
}

fn default_scene_entities() -> [SceneEntity; 7] {
    [
        SceneEntity {
            entity: FLOOR_ENTITY,
            name: "Floor",
            network: NetworkedSceneEntity::world(),
            catalog: catalog_ref(ASSET_FLOOR.0, MATERIAL_FLOOR.0, 0),
            translation: Vec3::ZERO,
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: FLOOR_COLLIDER_ENTITY,
            name: "FloorCollider",
            network: NetworkedSceneEntity::world(),
            catalog: catalog_ref(ASSET_FLOOR_COLLIDER.0, MATERIAL_FLOOR.0, COLLIDER_FLOOR.0),
            translation: Vec3::new(0.0, -0.25, 0.0),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: WALL_ENTITY,
            name: "Wall",
            network: NetworkedSceneEntity::world(),
            catalog: catalog_ref(ASSET_WALL.0, MATERIAL_WALL.0, COLLIDER_WALL.0),
            translation: Vec3::new(0.0, 1.5, -8.0),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: RAMP_ENTITY,
            name: "Ramp",
            network: NetworkedSceneEntity::world(),
            catalog: catalog_ref(ASSET_RAMP.0, MATERIAL_RAMP.0, COLLIDER_RAMP.0),
            translation: Vec3::new(-6.0, 0.25, -2.0),
            rotation_z_radians: -12.0_f32.to_radians(),
        },
        SceneEntity {
            entity: COVER_A_ENTITY,
            name: "CoverA",
            network: NetworkedSceneEntity::world(),
            catalog: catalog_ref(ASSET_COVER_CUBE.0, MATERIAL_COVER.0, COLLIDER_COVER_CUBE.0),
            translation: Vec3::new(3.0, 1.0, 2.0),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: COVER_B_ENTITY,
            name: "CoverB",
            network: NetworkedSceneEntity::world(),
            catalog: catalog_ref(ASSET_COVER_CUBE.0, MATERIAL_COVER.0, COLLIDER_COVER_CUBE.0),
            translation: Vec3::new(5.0, 1.0, -1.5),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: COVER_C_ENTITY,
            name: "CoverC",
            network: NetworkedSceneEntity::world(),
            catalog: catalog_ref(ASSET_COVER_CUBE.0, MATERIAL_COVER.0, COLLIDER_COVER_CUBE.0),
            translation: Vec3::new(7.0, 2.0, 4.0),
            rotation_z_radians: 0.0,
        },
    ]
}

#[cfg(test)]
fn default_scene_manifest_signature() -> SceneManifestSignature {
    scene_manifest_signature(&default_scene_entity_manifests())
}

const fn catalog_ref(asset_id: u32, material_id: u32, collider_id: u32) -> WorldCatalogRef {
    WorldCatalogRef {
        asset_id,
        material_id,
        collider_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scene_stream_uses_stable_entity_ids() {
        let specs = default_scene_world_specs();
        let ids = specs.iter().map(|spec| spec.entity.0).collect::<Vec<_>>();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn default_scene_stream_chunks_are_deterministic() {
        let first = default_scene_world_stream_chunks(WorldRevision(1));
        let second = default_scene_world_stream_chunks(WorldRevision(1));
        assert_eq!(first, second);
    }

    #[test]
    fn default_scene_manifest_exposes_stream_contract() {
        let manifest = default_scene_manifest(WorldRevision(7));
        let contract = manifest.shared_consumer_contract();

        assert_eq!(manifest.id, DEFAULT_SCENE_ID);
        assert_eq!(manifest.entities.len(), 7);
        assert_eq!(manifest.chunks.len(), 1);
        assert_eq!(manifest.chunks[0].revision, WorldRevision(7));
        assert_eq!(manifest.entities[0].stable_identity, FLOOR_ENTITY);
        assert!(
            manifest
                .entities
                .iter()
                .all(|entity| entity.catalog.is_some())
        );
        assert_eq!(manifest.renderer.render_entity_count, 6);
        assert_eq!(manifest.renderer.virtual_geometry_entity_count, 6);
        assert!(manifest.renderer.preview_camera.is_some());
        assert_eq!(manifest.lux.gi_participant_count, 6);
        assert_eq!(manifest.lux.virtual_shadow_participant_count, 6);
        assert!(manifest.lux.lighting.is_some());
        assert!(contract.server_uses_chunks);
        assert!(contract.client_uses_chunks);
        assert!(contract.editor_uses_manifest);
        assert!(contract.renderer_extracts_chunk_deltas);
        assert!(contract.virtual_page_service_receives_chunk_priorities);
    }

    #[test]
    fn default_scene_manifest_signature_uses_fun_scene_manifest_signature() {
        let entities = default_scene_entity_manifests();
        let manifest = default_scene_manifest(WorldRevision(3));

        assert_eq!(
            default_scene_manifest_signature(),
            scene_manifest_signature(&entities)
        );
        assert_eq!(manifest.signature, default_scene_manifest_signature());
        assert!(
            manifest
                .chunks
                .iter()
                .all(|chunk| chunk.manifest_signature == manifest.signature.0)
        );
    }

    #[test]
    fn default_scene_spawn_applies_stable_network_identities() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            FunScenePlugin,
        ));
        app.add_systems(
            Startup,
            (spawn_default_scene, apply_scene_stable_identities).chain(),
        );

        app.update();

        let mut query = app.world_mut().query::<&NetworkIdentity>();
        let mut ids = query
            .iter(app.world())
            .map(|identity| identity.entity.0)
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7]);

        let mut history_keys = app.world_mut().query::<&SceneStableHistoryKey>();
        let mut keys = history_keys
            .iter(app.world())
            .map(|key| key.0.0)
            .collect::<Vec<_>>();
        keys.sort_unstable();
        assert_eq!(keys, ids);
    }

    #[test]
    fn default_scene_manifest_signature_changes_without_reassigning_stable_ids() {
        let mut entities = default_scene_entity_manifests();
        let original_ids = entities
            .iter()
            .map(|entity| entity.stable_identity)
            .collect::<Vec<_>>();
        let original_signature = scene_manifest_signature(&entities);

        entities[0].network.scope_radius_millimeters += 1;

        assert_eq!(
            original_ids,
            entities
                .iter()
                .map(|entity| entity.stable_identity)
                .collect::<Vec<_>>()
        );
        assert_ne!(original_signature, scene_manifest_signature(&entities));
    }
}
