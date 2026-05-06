use bevy_ecs::{
    entity::Entity,
    prelude::{Commands, Component, Query, ResMut, Resource, Without},
};
use thunder::prelude::{
    AuthorityMode, NetClientId, NetEntity, NetworkAuthority, NetworkIdentity, Networked,
    PredictionOwner, ReplicationClass, ReplicationPriority, ReplicationScope,
};

use crate::{SceneNetworkManifest, SceneStableIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct NetworkedSceneEntity {
    pub class: ReplicationClass,
    pub authority: AuthorityMode,
    pub scope: ReplicationScope,
    pub priority: ReplicationPriority,
}

impl NetworkedSceneEntity {
    #[must_use]
    pub fn world() -> Self {
        Self::from(Networked::world())
    }

    #[must_use]
    pub fn server(class: ReplicationClass) -> Self {
        Self::from(Networked::server(class))
    }

    #[must_use]
    pub fn client_predicted(class: ReplicationClass, owner: NetClientId) -> Self {
        Self::from(Networked::client_predicted(class, owner))
    }

    #[must_use]
    pub const fn with_scope(mut self, scope: ReplicationScope) -> Self {
        self.scope = scope;
        self
    }

    #[must_use]
    pub const fn with_priority(mut self, priority: ReplicationPriority) -> Self {
        self.priority = priority;
        self
    }
}

impl Default for NetworkedSceneEntity {
    fn default() -> Self {
        Self::world()
    }
}

impl From<Networked> for NetworkedSceneEntity {
    fn from(value: Networked) -> Self {
        Self {
            class: value.class,
            authority: value.authority,
            scope: value.scope,
            priority: value.priority,
        }
    }
}

impl From<NetworkedSceneEntity> for Networked {
    fn from(value: NetworkedSceneEntity) -> Self {
        Self {
            class: value.class,
            authority: value.authority,
            scope: value.scope,
            priority: value.priority,
        }
    }
}

impl From<NetworkedSceneEntity> for SceneNetworkManifest {
    fn from(value: NetworkedSceneEntity) -> Self {
        Self::from_parts(value.class, value.authority, value.scope, value.priority)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct SceneStableHistoryKey(pub NetEntity);

impl SceneStableHistoryKey {
    #[must_use]
    pub const fn new(value: NetEntity) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0.is_valid()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct SceneStableEntityIndex {
    pub records: Vec<SceneStableEntityRecord>,
}

impl SceneStableEntityIndex {
    pub fn clear(&mut self) {
        self.records.clear();
    }

    pub fn insert(&mut self, stable_identity: NetEntity, runtime_entity: Entity) {
        if let Some(record) = self
            .records
            .iter_mut()
            .find(|record| record.stable_identity == stable_identity)
        {
            record.runtime_entity = runtime_entity;
        } else {
            self.records.push(SceneStableEntityRecord {
                stable_identity,
                runtime_entity,
            });
        }
        self.records.sort_by_key(|record| record.stable_identity.0);
    }

    #[must_use]
    pub fn resolve(&self, stable_identity: NetEntity) -> Option<Entity> {
        self.records
            .iter()
            .find(|record| record.stable_identity == stable_identity)
            .map(|record| record.runtime_entity)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneStableEntityRecord {
    pub stable_identity: NetEntity,
    pub runtime_entity: Entity,
}

pub fn rebuild_scene_stable_entity_index(
    mut index: ResMut<SceneStableEntityIndex>,
    query: Query<(Entity, &SceneStableIdentity)>,
) {
    index.clear();
    for (entity, stable_identity) in &query {
        if stable_identity.is_valid() {
            index.insert(stable_identity.0, entity);
        }
    }
}

pub fn apply_networked_scene_identities(
    mut commands: Commands,
    query: Query<(Entity, &SceneStableIdentity, &NetworkedSceneEntity), Without<NetworkIdentity>>,
) {
    for (entity, stable_identity, networked) in &query {
        if !stable_identity.is_valid() {
            continue;
        }
        let mut entity_commands = commands.entity(entity);
        entity_commands.insert((
            Networked::from(*networked),
            NetworkIdentity {
                entity: stable_identity.0,
                class: networked.class,
            },
            NetworkAuthority {
                mode: networked.authority,
            },
            networked.scope,
            networked.priority,
            SceneStableHistoryKey::new(stable_identity.0),
        ));
        if let AuthorityMode::ClientPredicted { owner } = networked.authority {
            entity_commands.insert(PredictionOwner { client_id: owner });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneNetworkRole {
    Server,
    Client,
    Editor,
}

impl SceneNetworkRole {
    pub const ALL: [Self; 3] = [Self::Server, Self::Client, Self::Editor];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Server => "server",
            Self::Client => "client",
            Self::Editor => "editor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneNetworkRoleDescriptor {
    pub role: SceneNetworkRole,
    pub authoritative_scene_spawn: bool,
    pub assigns_stable_identity: bool,
    pub generates_world_stream_manifest: bool,
    pub consumes_streamed_chunks: bool,
    pub spawns_renderable_scene_subset: bool,
    pub applies_prediction: bool,
    pub edits_full_scene_graph: bool,
    pub previews_same_renderer_path: bool,
    pub generates_deterministic_test_manifests: bool,
}

pub const SCENE_NETWORK_ROLE_DESCRIPTORS: [SceneNetworkRoleDescriptor; 3] = [
    SceneNetworkRoleDescriptor {
        role: SceneNetworkRole::Server,
        authoritative_scene_spawn: true,
        assigns_stable_identity: true,
        generates_world_stream_manifest: true,
        consumes_streamed_chunks: false,
        spawns_renderable_scene_subset: false,
        applies_prediction: false,
        edits_full_scene_graph: false,
        previews_same_renderer_path: false,
        generates_deterministic_test_manifests: false,
    },
    SceneNetworkRoleDescriptor {
        role: SceneNetworkRole::Client,
        authoritative_scene_spawn: false,
        assigns_stable_identity: false,
        generates_world_stream_manifest: false,
        consumes_streamed_chunks: true,
        spawns_renderable_scene_subset: true,
        applies_prediction: true,
        edits_full_scene_graph: false,
        previews_same_renderer_path: true,
        generates_deterministic_test_manifests: false,
    },
    SceneNetworkRoleDescriptor {
        role: SceneNetworkRole::Editor,
        authoritative_scene_spawn: false,
        assigns_stable_identity: false,
        generates_world_stream_manifest: false,
        consumes_streamed_chunks: false,
        spawns_renderable_scene_subset: false,
        applies_prediction: false,
        edits_full_scene_graph: true,
        previews_same_renderer_path: true,
        generates_deterministic_test_manifests: true,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneNetworkingPolicy {
    pub stable_identity_separate_from_runtime_entity: bool,
    pub server_authoritative_scene_spawn: bool,
    pub deterministic_manifest_signature_required: bool,
    pub deterministic_chunking_required: bool,
    pub stable_ids_survive_scene_patches: bool,
    pub renderer_uses_stable_identity_for_gpu_history: bool,
    pub renderer_uses_stable_identity_for_motion_vectors: bool,
    pub renderer_uses_stable_identity_for_cache_invalidation: bool,
}

impl SceneNetworkingPolicy {
    pub const DEFAULT: Self = Self {
        stable_identity_separate_from_runtime_entity: true,
        server_authoritative_scene_spawn: true,
        deterministic_manifest_signature_required: true,
        deterministic_chunking_required: true,
        stable_ids_survive_scene_patches: true,
        renderer_uses_stable_identity_for_gpu_history: true,
        renderer_uses_stable_identity_for_motion_vectors: true,
        renderer_uses_stable_identity_for_cache_invalidation: true,
    };
}

impl Default for SceneNetworkingPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::{schedule::Schedule, world::World};
    use thunder::prelude::{NetClientId, RelevanceLayers};

    use super::*;

    #[test]
    fn networked_scene_entity_preserves_thunder_network_fields() {
        let scene_networked =
            NetworkedSceneEntity::client_predicted(ReplicationClass::Pawn, NetClientId(9))
                .with_scope(ReplicationScope {
                    radius: 128.0,
                    layers: RelevanceLayers::INFANTRY,
                    faction: 3,
                    always_relevant: false,
                })
                .with_priority(ReplicationPriority(3.5));
        let networked = Networked::from(scene_networked);

        assert_eq!(networked.class, ReplicationClass::Pawn);
        assert_eq!(
            networked.authority,
            AuthorityMode::ClientPredicted {
                owner: NetClientId(9)
            }
        );
        assert_eq!(networked.scope.layers, RelevanceLayers::INFANTRY);
        assert_eq!(networked.priority, ReplicationPriority(3.5));
    }

    #[test]
    fn stable_identity_index_stays_separate_from_runtime_entity() {
        let mut world = World::new();
        world.init_resource::<SceneStableEntityIndex>();
        let stable_id = NetEntity(44);
        let entity = world.spawn((SceneStableIdentity(stable_id),)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(rebuild_scene_stable_entity_index);
        schedule.run(&mut world);

        let index = world.resource::<SceneStableEntityIndex>();
        assert_eq!(index.resolve(stable_id), Some(entity));
        assert_eq!(index.records[0].stable_identity, stable_id);
        assert_eq!(index.records[0].runtime_entity, entity);
    }

    #[test]
    fn applying_networked_scene_identity_inserts_stable_history_key() {
        let mut world = World::new();
        let stable_id = NetEntity(77);
        let entity = world
            .spawn((
                SceneStableIdentity(stable_id),
                NetworkedSceneEntity::world(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(apply_networked_scene_identities);
        schedule.run(&mut world);

        assert_eq!(
            world.get::<NetworkIdentity>(entity),
            Some(&NetworkIdentity {
                entity: stable_id,
                class: ReplicationClass::World,
            })
        );
        assert_eq!(
            world.get::<SceneStableHistoryKey>(entity),
            Some(&SceneStableHistoryKey(stable_id))
        );
    }

    #[test]
    fn scene_network_roles_match_server_client_editor_authority() {
        let server = SCENE_NETWORK_ROLE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.role == SceneNetworkRole::Server)
            .expect("server role is declared");
        let client = SCENE_NETWORK_ROLE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.role == SceneNetworkRole::Client)
            .expect("client role is declared");
        let editor = SCENE_NETWORK_ROLE_DESCRIPTORS
            .iter()
            .find(|descriptor| descriptor.role == SceneNetworkRole::Editor)
            .expect("editor role is declared");

        assert!(server.authoritative_scene_spawn);
        assert!(server.assigns_stable_identity);
        assert!(server.generates_world_stream_manifest);
        assert!(client.consumes_streamed_chunks);
        assert!(client.spawns_renderable_scene_subset);
        assert!(client.applies_prediction);
        assert!(editor.edits_full_scene_graph);
        assert!(editor.previews_same_renderer_path);
        assert!(editor.generates_deterministic_test_manifests);
    }
}
