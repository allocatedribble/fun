use thunder::prelude::{
    AuthorityMode, NetEntity, PackedColorRgba8, QuantizedTransform3, RelevanceLayers,
    ReplicationClass, ReplicationPriority, ReplicationScope, WorldCatalogRef, WorldPrimitive,
    WorldRevision, WorldStreamChunk,
};

use crate::streaming::{
    FNV64_OFFSET_BASIS, fnv1a, fnv1a_i32, fnv1a_str, fnv1a_u8, fnv1a_u64, hash_authority_mode,
    hash_catalog, hash_color, hash_render, hash_replication_class, hash_transform,
};

pub type SceneStreamChunk = WorldStreamChunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneId(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneManifestSignature(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneNetworkManifest {
    pub class: ReplicationClass,
    pub authority: AuthorityMode,
    pub scope_radius_millimeters: i32,
    pub scope_layers: RelevanceLayers,
    pub scope_faction: u32,
    pub scope_always_relevant: bool,
    pub priority_microunits: i32,
}

impl SceneNetworkManifest {
    #[must_use]
    pub fn from_parts(
        class: ReplicationClass,
        authority: AuthorityMode,
        scope: ReplicationScope,
        priority: ReplicationPriority,
    ) -> Self {
        Self {
            class,
            authority,
            scope_radius_millimeters: quantize_f32(scope.radius, 1_000.0),
            scope_layers: scope.layers,
            scope_faction: scope.faction,
            scope_always_relevant: scope.always_relevant,
            priority_microunits: quantize_f32(priority.0, 1_000_000.0),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SceneDescriptor {
    pub id: SceneId,
    pub display_name: &'static str,
    pub scene_function_name: &'static str,
    pub signature: SceneManifestSignature,
}

#[derive(Debug, Clone)]
pub struct SceneManifest {
    pub id: SceneId,
    pub display_name: &'static str,
    pub scene_function_name: &'static str,
    pub signature: SceneManifestSignature,
    pub entities: Vec<SceneEntityManifest>,
    pub chunks: Vec<SceneStreamChunk>,
    pub renderer: SceneRendererManifest,
    pub lux: SceneLuxManifest,
}

impl SceneManifest {
    #[must_use]
    pub fn descriptor(&self) -> SceneDescriptor {
        SceneDescriptor {
            id: self.id,
            display_name: self.display_name,
            scene_function_name: self.scene_function_name,
            signature: self.signature,
        }
    }

    #[must_use]
    pub fn shared_consumer_contract(&self) -> SceneConsumerContract {
        SceneConsumerContract {
            server_uses_chunks: true,
            client_uses_chunks: true,
            editor_uses_manifest: true,
            renderer_extracts_chunk_deltas: true,
            virtual_page_service_receives_chunk_priorities: true,
            chunk_count: self.chunks.len() as u16,
            signature: self.signature,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SceneEntityManifest {
    pub stable_identity: NetEntity,
    pub name: String,
    pub network: SceneNetworkManifest,
    pub transform: QuantizedTransform3,
    pub catalog: Option<WorldCatalogRef>,
    pub render: Option<WorldPrimitive>,
    pub material_color: Option<PackedColorRgba8>,
}

#[must_use]
pub fn scene_manifest_signature(entities: &[SceneEntityManifest]) -> SceneManifestSignature {
    let mut hash = FNV64_OFFSET_BASIS;
    hash = fnv1a_u64(hash, entities.len() as u64);
    for entity in entities {
        hash = fnv1a_u64(hash, entity.stable_identity.0);
        hash = fnv1a_str(hash, &entity.name);
        hash = hash_scene_network(hash, entity.network);
        hash = hash_transform(hash, entity.transform);
        hash = hash_catalog(hash, entity.catalog);
        hash = hash_render(hash, entity.render);
        hash = hash_color(hash, entity.material_color);
    }
    SceneManifestSignature(hash)
}

fn hash_scene_network(mut hash: u64, network: SceneNetworkManifest) -> u64 {
    hash = hash_replication_class(hash, network.class);
    hash = hash_authority_mode(hash, network.authority);
    hash = fnv1a_i32(hash, network.scope_radius_millimeters);
    hash = fnv1a_u64(hash, network.scope_layers.0);
    hash = fnv1a(hash, network.scope_faction);
    hash = fnv1a_u8(hash, u8::from(network.scope_always_relevant));
    fnv1a_i32(hash, network.priority_microunits)
}

fn quantize_f32(value: f32, scale: f32) -> i32 {
    let scaled = (value * scale).round();
    scaled.clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScenePreviewCamera {
    pub transform: QuantizedTransform3,
    pub vertical_fov_radians: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneLightingDescriptor {
    pub sun_direction: [f32; 3],
    pub sun_illuminance_lux: f32,
    pub ambient_rgb: [f32; 3],
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SceneRendererManifest {
    pub render_entity_count: u32,
    pub virtual_geometry_entity_count: u32,
    pub stream_chunk_count: u16,
    pub virtual_page_priority: u8,
    pub preview_camera: Option<ScenePreviewCamera>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct SceneLuxManifest {
    pub light_count: u32,
    pub emissive_candidate_count: u32,
    pub gi_participant_count: u32,
    pub virtual_shadow_participant_count: u32,
    pub lighting: Option<SceneLightingDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneConsumerContract {
    pub server_uses_chunks: bool,
    pub client_uses_chunks: bool,
    pub editor_uses_manifest: bool,
    pub renderer_extracts_chunk_deltas: bool,
    pub virtual_page_service_receives_chunk_priorities: bool,
    pub chunk_count: u16,
    pub signature: SceneManifestSignature,
}

pub trait SceneManifestProvider {
    fn scene_manifest(&self, revision: WorldRevision) -> SceneManifest;
}

#[deprecated(note = "use SceneManifestProvider and SceneManifest")]
pub trait SceneRenderManifestProvider {
    fn scene_render_manifest(&self, revision: WorldRevision) -> SceneManifest;
}

#[deprecated(note = "use SceneManifest")]
pub type SceneRenderManifest = SceneManifest;

#[deprecated(note = "use SceneEntityManifest")]
pub type SceneRenderEntity = SceneEntityManifest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneManifestPolicy {
    pub deterministic_network_ids: bool,
    pub shared_server_editor_client_manifests: bool,
    pub chunk_world_streaming: bool,
    pub renderer_bake_hooks: bool,
}

impl FunSceneManifestPolicy {
    pub const DEFAULT: Self = Self {
        deterministic_network_ids: true,
        shared_server_editor_client_manifests: true,
        chunk_world_streaming: true,
        renderer_bake_hooks: true,
    };
}

impl Default for FunSceneManifestPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use thunder::prelude::{NetClientId, QuantizedTransform3, RelevanceLayers, WorldCatalogRef};

    use super::*;

    fn entity_manifest() -> SceneEntityManifest {
        SceneEntityManifest {
            stable_identity: NetEntity(1),
            name: String::from("Door"),
            network: SceneNetworkManifest::from_parts(
                ReplicationClass::World,
                AuthorityMode::StaticServer,
                ReplicationScope {
                    radius: 512.0,
                    layers: RelevanceLayers::DEFAULT,
                    faction: 0,
                    always_relevant: true,
                },
                ReplicationPriority(0.5),
            ),
            transform: QuantizedTransform3::default(),
            catalog: Some(WorldCatalogRef {
                asset_id: 10,
                material_id: 20,
                collider_id: 30,
            }),
            render: None,
            material_color: None,
        }
    }

    #[test]
    fn manifest_signature_changes_when_entity_fields_change() {
        let original = vec![entity_manifest()];
        let mut changed = original.clone();
        changed[0].network = SceneNetworkManifest::from_parts(
            ReplicationClass::Pawn,
            AuthorityMode::ClientPredicted {
                owner: NetClientId(5),
            },
            ReplicationScope {
                radius: 64.0,
                layers: RelevanceLayers::INFANTRY,
                faction: 2,
                always_relevant: false,
            },
            ReplicationPriority(2.25),
        );

        assert_ne!(
            scene_manifest_signature(&original),
            scene_manifest_signature(&changed)
        );
    }

    #[test]
    fn network_manifest_quantizes_float_fields_for_stable_signatures() {
        let network = SceneNetworkManifest::from_parts(
            ReplicationClass::World,
            AuthorityMode::StaticServer,
            ReplicationScope {
                radius: 1.2345,
                layers: RelevanceLayers::DEFAULT,
                faction: 0,
                always_relevant: true,
            },
            ReplicationPriority(0.25),
        );

        assert_eq!(network.scope_radius_millimeters, 1_235);
        assert_eq!(network.priority_microunits, 250_000);
    }
}
