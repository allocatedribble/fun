#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunEcsExperimentKind {
    #[default]
    StoragePromotion = 0,
    MaterializedGroups = 1,
    SpeculativeSystems = 2,
    TemporalSnapshots = 3,
}

impl FunEcsExperimentKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::StoragePromotion => "storage_promotion",
            Self::MaterializedGroups => "materialized_groups",
            Self::SpeculativeSystems => "speculative_systems",
            Self::TemporalSnapshots => "temporal_snapshots",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsExperimentGate {
    pub kind: FunEcsExperimentKind,
    pub enabled_by_default: bool,
}

impl FunEcsExperimentGate {
    #[must_use]
    pub const fn opt_in(kind: FunEcsExperimentKind) -> Self {
        Self {
            kind,
            enabled_by_default: false,
        }
    }
}
