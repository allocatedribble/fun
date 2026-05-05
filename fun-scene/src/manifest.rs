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
