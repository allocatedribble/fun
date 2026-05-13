use fun_scheduler_types::{
    EcsChunkKey, EcsEntityId, EcsSpatialDomainKind, EcsVirtualResourceKey, WorkWaitToken,
};

use crate::{
    EcsDerivedArtifactId, EcsPageChannel, EcsSpatialPageKey, FunEcsComponentKind,
    FunEcsResourceKind,
};

macro_rules! fun_invalid_zero_id {
    ($name:ident, $raw:ty) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub $raw);

        impl $name {
            pub const INVALID: Self = Self(0);

            #[must_use]
            pub const fn new(value: $raw) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> $raw {
                self.0
            }

            #[must_use]
            pub const fn is_valid(self) -> bool {
                self.0 != 0
            }
        }
    };
}

fun_invalid_zero_id!(FunEntityGeneration, u32);
fun_invalid_zero_id!(FunComponentId, u64);
fun_invalid_zero_id!(FunResourceId, u64);
fun_invalid_zero_id!(FunResourceTableId, u64);
fun_invalid_zero_id!(FunSystemId, u32);
fun_invalid_zero_id!(FunSystemSetId, u32);
fun_invalid_zero_id!(FunCommandBufferId, u32);
fun_invalid_zero_id!(FunArchetypeId, u64);
fun_invalid_zero_id!(FunStorageChunkId, u64);
fun_invalid_zero_id!(FunExternalSlabId, u64);
fun_invalid_zero_id!(FunArtifactId, u64);
fun_invalid_zero_id!(FunFrameId, u64);
fun_invalid_zero_id!(FunSchedulerWaitTokenId, u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunEntity {
    pub slot: u64,
    pub generation: FunEntityGeneration,
}

impl FunEntity {
    pub const INVALID: Self = Self {
        slot: 0,
        generation: FunEntityGeneration::INVALID,
    };

    #[must_use]
    pub const fn new(slot: u64, generation: FunEntityGeneration) -> Self {
        Self { slot, generation }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.slot != 0 && self.generation.is_valid()
    }

    #[must_use]
    pub const fn scheduler_bits(self) -> u64 {
        ((self.generation.get() as u64) << 32) | (self.slot & 0xffff_ffff)
    }

    #[must_use]
    pub const fn to_bits(self) -> u64 {
        self.scheduler_bits()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunSchedulerVirtualResourceKey {
    pub domain: EcsSpatialDomainKind,
    pub layer: u16,
    pub level: u8,
    pub channel: u16,
    pub chunk: EcsChunkKey,
    pub generation: u32,
}

impl FunSchedulerVirtualResourceKey {
    #[must_use]
    pub const fn new(
        domain: EcsSpatialDomainKind,
        layer: u16,
        level: u8,
        channel: u16,
        chunk: EcsChunkKey,
        generation: u32,
    ) -> Self {
        Self {
            domain,
            layer,
            level,
            channel,
            chunk,
            generation,
        }
    }

    #[must_use]
    pub const fn from_resource_chunk(
        resource: FunResourceId,
        chunk: FunStorageChunkId,
        generation: u32,
    ) -> Self {
        Self {
            domain: EcsSpatialDomainKind::Debug,
            layer: (resource.get() & 0xffff) as u16,
            level: 0,
            channel: 0,
            chunk: EcsChunkKey::new(chunk.get()),
            generation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSpatialPageVirtualResourceKey {
    pub page: EcsSpatialPageKey,
    pub generation: u32,
}

impl FunSpatialPageVirtualResourceKey {
    #[must_use]
    pub const fn new(page: EcsSpatialPageKey, generation: u32) -> Self {
        Self { page, generation }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunSchedulerWaitToken {
    pub id: FunSchedulerWaitTokenId,
}

impl FunSchedulerWaitToken {
    #[must_use]
    pub const fn new(id: FunSchedulerWaitTokenId) -> Self {
        Self { id }
    }
}

pub trait IntoSchedulerEntityId {
    #[must_use]
    fn into_scheduler_entity_id(self) -> EcsEntityId;
}

pub trait IntoSchedulerChunkKey {
    #[must_use]
    fn into_scheduler_chunk_key(self) -> EcsChunkKey;
}

pub trait IntoSchedulerVirtualResourceKey {
    #[must_use]
    fn into_scheduler_virtual_resource_key(self) -> EcsVirtualResourceKey;
}

pub trait IntoSchedulerWaitToken {
    #[must_use]
    fn into_scheduler_wait_token(self) -> WorkWaitToken;
}

impl IntoSchedulerEntityId for FunEntity {
    fn into_scheduler_entity_id(self) -> EcsEntityId {
        if self.is_valid() {
            EcsEntityId::new(self.scheduler_bits())
        } else {
            EcsEntityId::new(0)
        }
    }
}

impl IntoSchedulerChunkKey for FunStorageChunkId {
    fn into_scheduler_chunk_key(self) -> EcsChunkKey {
        EcsChunkKey::new(self.get())
    }
}

impl IntoSchedulerChunkKey for FunResourceTableId {
    fn into_scheduler_chunk_key(self) -> EcsChunkKey {
        EcsChunkKey::new(self.get())
    }
}

impl IntoSchedulerChunkKey for EcsSpatialPageKey {
    fn into_scheduler_chunk_key(self) -> EcsChunkKey {
        self.chunk_key()
    }
}

impl IntoSchedulerVirtualResourceKey for FunSchedulerVirtualResourceKey {
    fn into_scheduler_virtual_resource_key(self) -> EcsVirtualResourceKey {
        EcsVirtualResourceKey::new(
            self.domain,
            self.layer,
            self.level,
            self.channel,
            self.chunk,
            self.generation,
        )
    }
}

impl IntoSchedulerVirtualResourceKey for FunSpatialPageVirtualResourceKey {
    fn into_scheduler_virtual_resource_key(self) -> EcsVirtualResourceKey {
        self.page.virtual_resource_key(self.generation)
    }
}

impl IntoSchedulerWaitToken for FunSchedulerWaitToken {
    fn into_scheduler_wait_token(self) -> WorkWaitToken {
        WorkWaitToken::new(self.id.get())
    }
}

impl FunComponentId {
    #[must_use]
    pub const fn from_component_kind(kind: FunEcsComponentKind) -> Self {
        Self(kind as u64 + 1)
    }
}

impl FunResourceId {
    #[must_use]
    pub const fn from_resource_kind(kind: FunEcsResourceKind) -> Self {
        Self(kind as u64 + 1)
    }
}

impl FunResourceTableId {
    #[must_use]
    pub const fn from_resource_kind(kind: FunEcsResourceKind) -> Self {
        Self(kind as u64 + 1)
    }
}

impl FunArtifactId {
    #[must_use]
    pub const fn from_derived_artifact_id(id: EcsDerivedArtifactId) -> Self {
        Self(id.get())
    }
}

impl FunSchedulerVirtualResourceKey {
    #[must_use]
    pub const fn spatial_debug_resource(
        chunk: FunStorageChunkId,
        channel: EcsPageChannel,
        generation: u32,
    ) -> Self {
        Self::new(
            EcsSpatialDomainKind::Debug,
            0,
            0,
            channel as u16,
            EcsChunkKey::new(chunk.get()),
            generation,
        )
    }
}

#[cfg(test)]
mod tests {
    use fun_scheduler_types::EcsSpatialDomainKind;

    use crate::{EcsPageChannel, EcsSpatialGridId};

    use super::*;

    #[test]
    fn entity_ids_are_generational_and_bridge_to_scheduler_entity_ids() {
        let entity = FunEntity::new(42, FunEntityGeneration::new(7));
        let scheduler = entity.into_scheduler_entity_id();

        assert!(entity.is_valid());
        assert_eq!(scheduler.get(), (7_u64 << 32) | 42);
        assert_eq!(FunEntity::INVALID.into_scheduler_entity_id().get(), 0);
    }

    #[test]
    fn page_and_resource_ids_bridge_to_scheduler_keys() {
        let page = EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(3),
            2,
            1,
            2,
            3,
            EcsPageChannel::Surface,
        );
        let key =
            FunSpatialPageVirtualResourceKey::new(page, 9).into_scheduler_virtual_resource_key();
        assert_eq!(key.domain, EcsSpatialDomainKind::Terrain);
        assert_eq!(key.generation, 9);
        assert_eq!(page.into_scheduler_chunk_key(), key.chunk);

        let resource =
            FunResourceId::from_resource_kind(FunEcsResourceKind::DerivedArtifactRegistry);
        assert!(resource.is_valid());
        let chunk = FunStorageChunkId::new(77).into_scheduler_chunk_key();
        assert_eq!(chunk.get(), 77);
    }
}
