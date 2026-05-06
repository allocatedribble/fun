use bevy_ecs::prelude::Component;

use crate::FunFromTemplate;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FunFromTemplate, Component)]
pub struct GameplaySalient {
    pub score: u8,
}

impl GameplaySalient {
    pub const LOW: Self = Self { score: 64 };
    pub const NORMAL: Self = Self { score: 128 };
    pub const HIGH: Self = Self { score: 192 };
    pub const CRITICAL: Self = Self { score: u8::MAX };

    #[must_use]
    pub const fn new(score: u8) -> Self {
        Self { score }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FunFromTemplate, Component)]
pub struct EditorSelection {
    pub rank: u16,
    pub salience: u8,
}

impl EditorSelection {
    pub const PRIMARY: Self = Self {
        rank: 0,
        salience: u8::MAX,
    };

    #[must_use]
    pub const fn new(rank: u16, salience: u8) -> Self {
        Self { rank, salience }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FunFromTemplate, Component)]
pub struct StreamingPriority {
    pub score: u8,
}

impl StreamingPriority {
    pub const BACKGROUND: Self = Self { score: 32 };
    pub const NORMAL: Self = Self { score: 128 };
    pub const WORLD_CRITICAL: Self = Self { score: u8::MAX };

    #[must_use]
    pub const fn new(score: u8) -> Self {
        Self { score }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, FunFromTemplate, Component)]
pub struct TemporalInstability {
    pub motion: u8,
    pub topology: u8,
}

impl TemporalInstability {
    pub const STABLE: Self = Self {
        motion: 0,
        topology: 0,
    };
    pub const TRANSFORM_UNSTABLE: Self = Self {
        motion: 192,
        topology: 0,
    };
    pub const DESTRUCTIBLE: Self = Self {
        motion: 128,
        topology: u8::MAX,
    };

    #[must_use]
    pub const fn new(motion: u8, topology: u8) -> Self {
        Self { motion, topology }
    }

    #[must_use]
    pub const fn score(self) -> u8 {
        if self.motion > self.topology {
            self.motion
        } else {
            self.topology
        }
    }
}
