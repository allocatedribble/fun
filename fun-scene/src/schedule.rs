use bevy_ecs::schedule::SystemSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum FunSceneSet {
    Resolve,
    Spawn,
    Patch,
    Validate,
}

impl FunSceneSet {
    pub const ORDER: [Self; 4] = [Self::Resolve, Self::Validate, Self::Spawn, Self::Patch];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::Resolve => 10,
            Self::Validate => 20,
            Self::Spawn => 30,
            Self::Patch => 40,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resolve => "resolve",
            Self::Spawn => "spawn",
            Self::Patch => "patch",
            Self::Validate => "validate",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_set_order_keeps_validation_before_spawn() {
        let mut previous = 0;
        for set in FunSceneSet::ORDER {
            assert!(set.order_key() > previous, "{}", set.as_str());
            previous = set.order_key();
        }

        assert!(FunSceneSet::Validate.order_key() < FunSceneSet::Spawn.order_key());
        assert!(FunSceneSet::Patch.order_key() > FunSceneSet::Spawn.order_key());
    }
}
