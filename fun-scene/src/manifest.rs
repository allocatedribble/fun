use thunder::prelude::{
    NetEntity, PackedColorRgba8, QuantizedTransform3, WorldCatalogRef, WorldPrimitive,
    WorldRevision, WorldStreamChunk,
};

pub type SceneStreamChunk = WorldStreamChunk;

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
    pub transform: QuantizedTransform3,
    pub catalog: Option<WorldCatalogRef>,
    pub render: Option<WorldPrimitive>,
    pub material_color: Option<PackedColorRgba8>,
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
