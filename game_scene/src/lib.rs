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
const FLOOR_ENTITY: NetEntity = NetEntity(1);
const FLOOR_COLLIDER_ENTITY: NetEntity = NetEntity(2);
const WALL_ENTITY: NetEntity = NetEntity(3);
const RAMP_ENTITY: NetEntity = NetEntity(4);
const COVER_A_ENTITY: NetEntity = NetEntity(5);
const COVER_B_ENTITY: NetEntity = NetEntity(6);
const COVER_C_ENTITY: NetEntity = NetEntity(7);

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
pub fn default_scene_world_stream_chunks(revision: WorldRevision) -> Vec<WorldStreamChunk> {
    chunk_world_specs(DEMO_LEVEL_ID, revision, default_scene_world_specs())
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
