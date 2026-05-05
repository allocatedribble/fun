#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneStreamingPolicy {
    pub chunked_world_streams: bool,
    pub stable_chunk_indices: bool,
    pub procedural_expansion_after_validation: bool,
}

impl FunSceneStreamingPolicy {
    pub const DEFAULT: Self = Self {
        chunked_world_streams: true,
        stable_chunk_indices: true,
        procedural_expansion_after_validation: true,
    };
}

impl Default for FunSceneStreamingPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
