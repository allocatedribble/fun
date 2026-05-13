use fun_ecs::Component;
use thunder::prelude::{NetEntity, WorldLevelId, WorldRevision, WorldStreamChunk};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct FunSceneEntityId(pub u64);

impl FunSceneEntityId {
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct SceneStableIdentity(pub NetEntity);

impl SceneStableIdentity {
    #[must_use]
    pub const fn new(value: NetEntity) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0.is_valid()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct SceneRevision(pub WorldRevision);

impl SceneRevision {
    #[must_use]
    pub const fn new(value: WorldRevision) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Component)]
pub struct SceneChunkId {
    pub level: WorldLevelId,
    pub chunk_index: u16,
    pub revision: WorldRevision,
}

impl SceneChunkId {
    #[must_use]
    pub fn new(level: WorldLevelId, chunk_index: u16, revision: WorldRevision) -> Self {
        Self {
            level,
            chunk_index,
            revision,
        }
    }

    #[must_use]
    pub fn from_world_stream_chunk(chunk: &WorldStreamChunk) -> Self {
        Self {
            level: chunk.level_id.clone(),
            chunk_index: chunk.chunk_index,
            revision: chunk.revision,
        }
    }
}
