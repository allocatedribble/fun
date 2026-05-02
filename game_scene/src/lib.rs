use avian3d::prelude::{Collider, RigidBody};
use bevy::{
    prelude::*,
    scene::{
        prelude::{CommandsSceneExt, Scene as BsnScene, bsn, bsn_list},
        template_value,
    },
};
use game_shared::{
    ASSET_COVER_CUBE, ASSET_FLOOR, ASSET_FLOOR_COLLIDER, ASSET_RAMP, ASSET_WALL,
    COLLIDER_COVER_CUBE, COLLIDER_FLOOR, COLLIDER_RAMP, COLLIDER_WALL, DEMO_LEVEL_ID,
    MATERIAL_COVER, MATERIAL_FLOOR, MATERIAL_RAMP, MATERIAL_WALL,
};
use thunder::prelude::*;

pub const WORLD_STREAM_ENTITIES_PER_CHUNK: usize = 16;
pub const DEFAULT_SCENE_ID: SceneId = SceneId("arena-blockout");
const DEFAULT_SCENE_FUNCTION_NAME: &str = "spawn_default_scene";
const FLOOR_ENTITY: NetEntity = NetEntity(1);
const FLOOR_COLLIDER_ENTITY: NetEntity = NetEntity(2);
const WALL_ENTITY: NetEntity = NetEntity(3);
const RAMP_ENTITY: NetEntity = NetEntity(4);
const COVER_A_ENTITY: NetEntity = NetEntity(5);
const COVER_B_ENTITY: NetEntity = NetEntity(6);
const COVER_C_ENTITY: NetEntity = NetEntity(7);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneId(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneManifestSignature(pub u64);

#[derive(Debug, Clone, Copy)]
pub struct SceneDescriptor {
    pub id: SceneId,
    pub display_name: &'static str,
    pub scene_function_name: &'static str,
    pub signature: SceneManifestSignature,
}

#[derive(Debug, Clone)]
pub struct SceneRenderManifest {
    pub descriptor: SceneDescriptor,
    pub entities: Vec<SceneRenderEntity>,
    pub preview_camera: ScenePreviewCamera,
    pub lighting: SceneLightingDescriptor,
    pub world_stream_chunks: Vec<WorldStreamChunk>,
}

#[derive(Debug, Clone)]
pub struct SceneRenderEntity {
    pub stable_identity: NetEntity,
    pub name: String,
    pub transform: QuantizedTransform3,
    pub catalog: Option<WorldCatalogRef>,
    pub render: Option<WorldPrimitive>,
    pub material_color: Option<PackedColorRgba8>,
}

#[derive(Debug, Clone, Copy)]
pub struct ScenePreviewCamera {
    pub transform: QuantizedTransform3,
    pub vertical_fov_radians: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SceneLightingDescriptor {
    pub sun_direction: [f32; 3],
    pub sun_illuminance_lux: f32,
    pub ambient_rgb: [f32; 3],
}

pub trait SceneRenderManifestProvider {
    fn scene_render_manifest(&self, revision: WorldRevision) -> SceneRenderManifest;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultSceneRenderManifestProvider;

impl SceneRenderManifestProvider for DefaultSceneRenderManifestProvider {
    fn scene_render_manifest(&self, revision: WorldRevision) -> SceneRenderManifest {
        default_scene_render_manifest(revision)
    }
}

#[derive(Debug, Default, Clone, Component)]
pub struct StreamedWorldEntity {
    pub catalog: Option<WorldCatalogRef>,
    pub render: Option<WorldPrimitive>,
    pub collider: Option<WorldCollider>,
    pub color: Option<PackedColorRgba8>,
}

#[derive(Debug, Default, Clone, Copy, Component)]
pub struct SceneStableIdentity(pub NetEntity);

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
    catalog: WorldCatalogRef,
    translation: Vec3,
    rotation_z_radians: f32,
}

impl SceneEntity {
    fn transform(self) -> Transform {
        Transform::from_translation(self.translation)
            .with_rotation(Quat::from_rotation_z(self.rotation_z_radians))
    }
}

pub fn spawn_default_scene(mut commands: Commands) {
    commands.spawn_scene_list(bsn_list![
        (
            #Floor
            template_value(Networked::world())
            template_value(SceneStableIdentity(FLOOR_ENTITY))
            template_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_FLOOR.0,
                MATERIAL_FLOOR.0,
                0,
            )))
            Transform::default()
        ),
        (
            #FloorCollider
            Name::new("FloorCollider")
            template_value(Networked::world())
            template_value(SceneStableIdentity(FLOOR_COLLIDER_ENTITY))
            template_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_FLOOR_COLLIDER.0,
                MATERIAL_FLOOR.0,
                COLLIDER_FLOOR.0,
            )))
            template_value(RigidBody::Static)
            Collider::cuboid(60.0, 0.5, 60.0)
            Transform::from_xyz(0.0, -0.25, 0.0)
        ),
        (
            #Wall
            template_value(Networked::world())
            template_value(SceneStableIdentity(WALL_ENTITY))
            template_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_WALL.0,
                MATERIAL_WALL.0,
                COLLIDER_WALL.0,
            )))
            template_value(RigidBody::Static)
            Collider::cuboid(5.0, 3.0, 1.0)
            Transform::from_xyz(0.0, 1.5, -8.0)
        ),
        (
            #Ramp
            template_value(Networked::world())
            template_value(SceneStableIdentity(RAMP_ENTITY))
            template_value(StreamedWorldEntity::catalog(catalog_ref(
                ASSET_RAMP.0,
                MATERIAL_RAMP.0,
                COLLIDER_RAMP.0,
            )))
            template_value(RigidBody::Static)
            Collider::cuboid(3.0, 0.5, 6.0)
            template_value(Transform::from_xyz(-6.0, 0.25, -2.0)
                .with_rotation(Quat::from_rotation_z(-12.0_f32.to_radians())))
        ),
        demo_cube(COVER_A_ENTITY, Vec3::new(3.0, 1.0, 2.0)),
        demo_cube(COVER_B_ENTITY, Vec3::new(5.0, 1.0, -1.5)),
        demo_cube(COVER_C_ENTITY, Vec3::new(7.0, 2.0, 4.0)),
    ]);
}

fn demo_cube(entity: NetEntity, translation: Vec3) -> impl BsnScene {
    bsn! {
        template_value(Networked::world())
        template_value(SceneStableIdentity(entity))
        template_value(StreamedWorldEntity::catalog(catalog_ref(
            ASSET_COVER_CUBE.0,
            MATERIAL_COVER.0,
            COLLIDER_COVER_CUBE.0,
        )))
        template_value(RigidBody::Static)
        Collider::cuboid(1.0, 1.0, 1.0)
        template_value(Transform::from_translation(translation))
    }
}

pub fn apply_scene_stable_identities(
    mut commands: Commands,
    query: Query<(Entity, &SceneStableIdentity, &Networked), Without<NetworkIdentity>>,
) {
    for (entity, stable_identity, networked) in &query {
        commands.entity(entity).insert((
            NetworkIdentity {
                entity: stable_identity.0,
                class: networked.class,
            },
            NetworkAuthority {
                mode: networked.authority,
            },
            networked.scope,
            networked.priority,
        ));
    }
}

#[must_use]
pub fn scene_render_descriptors() -> [SceneDescriptor; 1] {
    [default_scene_descriptor()]
}

#[must_use]
pub fn scene_render_manifest(
    scene_id: SceneId,
    revision: WorldRevision,
) -> Option<SceneRenderManifest> {
    if scene_id == DEFAULT_SCENE_ID {
        return Some(default_scene_render_manifest(revision));
    }
    None
}

#[must_use]
pub fn default_scene_descriptor() -> SceneDescriptor {
    SceneDescriptor {
        id: DEFAULT_SCENE_ID,
        display_name: "Arena Blockout",
        scene_function_name: DEFAULT_SCENE_FUNCTION_NAME,
        signature: default_scene_manifest_signature(),
    }
}

#[must_use]
pub fn default_scene_render_manifest(revision: WorldRevision) -> SceneRenderManifest {
    let specs = default_scene_world_specs();
    let entities = specs
        .iter()
        .map(|spec| SceneRenderEntity {
            stable_identity: spec.entity,
            name: spec.name.clone(),
            transform: spec.transform,
            catalog: spec.catalog,
            render: spec.render,
            material_color: spec.color,
        })
        .collect::<Vec<_>>();
    let world_stream_chunks = chunk_world_specs(DEMO_LEVEL_ID, revision, specs);
    SceneRenderManifest {
        descriptor: default_scene_descriptor(),
        entities,
        preview_camera: ScenePreviewCamera {
            transform: qtransform(
                &Transform::from_xyz(-8.0, 5.0, 10.0)
                    .looking_at(Vec3::new(1.0, 1.0, -1.0), Vec3::Y),
            ),
            vertical_fov_radians: 65.0_f32.to_radians(),
        },
        lighting: SceneLightingDescriptor {
            sun_direction: [-0.4, -1.0, -0.35],
            sun_illuminance_lux: 25_000.0,
            ambient_rgb: [0.03, 0.035, 0.04],
        },
        world_stream_chunks,
    }
}

#[must_use]
pub fn default_scene_world_stream_chunks(revision: WorldRevision) -> Vec<WorldStreamChunk> {
    default_scene_render_manifest(revision).world_stream_chunks
}

#[must_use]
pub fn default_scene_world_specs() -> Vec<WorldEntitySpec> {
    default_scene_entities()
        .into_iter()
        .map(|entity| WorldEntitySpec {
            entity: entity.entity,
            name: entity.name.to_owned(),
            class: ReplicationClass::World,
            authority: AuthorityMode::StaticServer,
            transform: qtransform(&entity.transform()),
            catalog: Some(entity.catalog),
            render: None,
            collider: None,
            color: None,
        })
        .collect()
}

#[must_use]
pub fn chunk_world_specs(
    level_id: &str,
    revision: WorldRevision,
    specs: Vec<WorldEntitySpec>,
) -> Vec<WorldStreamChunk> {
    let chunk_count = specs
        .len()
        .div_ceil(WORLD_STREAM_ENTITIES_PER_CHUNK)
        .max(1)
        .min(u16::MAX as usize) as u16;

    specs
        .chunks(WORLD_STREAM_ENTITIES_PER_CHUNK)
        .enumerate()
        .map(|(chunk_index, entities)| WorldStreamChunk {
            level_id: WorldLevelId(level_id.to_owned()),
            revision,
            chunk_index: chunk_index as u16,
            chunk_count,
            entities: entities.to_vec(),
        })
        .collect()
}

#[must_use]
pub fn qtransform(transform: &Transform) -> QuantizedTransform3 {
    QuantizedTransform3 {
        translation: qvec(transform.translation),
        rotation: QuantizedQuat::from_f32([
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
        ]),
    }
}

fn default_scene_entities() -> [SceneEntity; 7] {
    [
        SceneEntity {
            entity: FLOOR_ENTITY,
            name: "Floor",
            catalog: catalog_ref(ASSET_FLOOR.0, MATERIAL_FLOOR.0, 0),
            translation: Vec3::ZERO,
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: FLOOR_COLLIDER_ENTITY,
            name: "FloorCollider",
            catalog: catalog_ref(ASSET_FLOOR_COLLIDER.0, MATERIAL_FLOOR.0, COLLIDER_FLOOR.0),
            translation: Vec3::new(0.0, -0.25, 0.0),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: WALL_ENTITY,
            name: "Wall",
            catalog: catalog_ref(ASSET_WALL.0, MATERIAL_WALL.0, COLLIDER_WALL.0),
            translation: Vec3::new(0.0, 1.5, -8.0),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: RAMP_ENTITY,
            name: "Ramp",
            catalog: catalog_ref(ASSET_RAMP.0, MATERIAL_RAMP.0, COLLIDER_RAMP.0),
            translation: Vec3::new(-6.0, 0.25, -2.0),
            rotation_z_radians: -12.0_f32.to_radians(),
        },
        SceneEntity {
            entity: COVER_A_ENTITY,
            name: "CoverA",
            catalog: catalog_ref(ASSET_COVER_CUBE.0, MATERIAL_COVER.0, COLLIDER_COVER_CUBE.0),
            translation: Vec3::new(3.0, 1.0, 2.0),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: COVER_B_ENTITY,
            name: "CoverB",
            catalog: catalog_ref(ASSET_COVER_CUBE.0, MATERIAL_COVER.0, COLLIDER_COVER_CUBE.0),
            translation: Vec3::new(5.0, 1.0, -1.5),
            rotation_z_radians: 0.0,
        },
        SceneEntity {
            entity: COVER_C_ENTITY,
            name: "CoverC",
            catalog: catalog_ref(ASSET_COVER_CUBE.0, MATERIAL_COVER.0, COLLIDER_COVER_CUBE.0),
            translation: Vec3::new(7.0, 2.0, 4.0),
            rotation_z_radians: 0.0,
        },
    ]
}

fn default_scene_manifest_signature() -> SceneManifestSignature {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for entity in default_scene_entities() {
        hash = fnv1a_u64(hash, entity.entity.0);
        hash = fnv1a(hash, entity.catalog.asset_id);
        hash = fnv1a(hash, entity.catalog.material_id);
        hash = fnv1a(hash, entity.catalog.collider_id);
        for value in entity.translation.to_array() {
            hash = fnv1a(hash, value.to_bits());
        }
        hash = fnv1a(hash, entity.rotation_z_radians.to_bits());
    }
    SceneManifestSignature(hash)
}

const fn catalog_ref(asset_id: u32, material_id: u32, collider_id: u32) -> WorldCatalogRef {
    WorldCatalogRef {
        asset_id,
        material_id,
        collider_id,
    }
}

fn qvec(value: Vec3) -> QuantizedVec3 {
    QuantizedVec3::from_f32(value.to_array(), Quantization::MILLIMETERS)
}

fn fnv1a(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv1a_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
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
    fn default_scene_manifest_exposes_render_structure() {
        let manifest = default_scene_render_manifest(WorldRevision(7));
        assert_eq!(manifest.descriptor.id, DEFAULT_SCENE_ID);
        assert_eq!(manifest.entities.len(), 7);
        assert_eq!(manifest.world_stream_chunks.len(), 1);
        assert_eq!(manifest.world_stream_chunks[0].revision, WorldRevision(7));
        assert_eq!(manifest.entities[0].stable_identity, FLOOR_ENTITY);
        assert!(
            manifest
                .entities
                .iter()
                .all(|entity| entity.catalog.is_some())
        );
    }

    #[test]
    fn default_scene_spawn_applies_stable_network_identities() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::asset::AssetPlugin::default(),
            bevy::scene::ScenePlugin,
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
    }
}
