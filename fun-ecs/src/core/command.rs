use crate::{FunCommandBufferId, FunEcsResourceKind, FunEcsSubsystem};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunCommandBufferClass {
    #[default]
    WorldStructure = 0,
    ResourceTableMutation = 1,
    ArtifactPublication = 2,
    HandoffPublication = 3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunCommandDrainPolicy {
    #[default]
    DeterministicSortThenApply = 0,
    PreserveProducerOrder = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunCommandBufferDeclaration {
    pub id: FunCommandBufferId,
    pub owner: FunEcsSubsystem,
    pub target_resource: FunEcsResourceKind,
    pub class: FunCommandBufferClass,
    pub drain_policy: FunCommandDrainPolicy,
}

impl FunCommandBufferDeclaration {
    #[must_use]
    pub const fn deterministic(
        id: FunCommandBufferId,
        owner: FunEcsSubsystem,
        target_resource: FunEcsResourceKind,
        class: FunCommandBufferClass,
    ) -> Self {
        Self {
            id,
            owner,
            target_resource,
            class,
            drain_policy: FunCommandDrainPolicy::DeterministicSortThenApply,
        }
    }
}
