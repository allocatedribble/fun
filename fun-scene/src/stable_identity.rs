use bevy_ecs::prelude::Component;

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
pub struct FunSceneStableIdentity<T = u64>(pub T);

impl<T> FunSceneStableIdentity<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}
