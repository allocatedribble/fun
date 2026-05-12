pub mod archetype;
pub mod materialized_group;
pub mod snapshot;
pub mod sparse_pool;
pub mod table_query;

pub use archetype::{
    Archetype, ArchetypeChunk, ArchetypeTable, ChunkDirtyMask, ChunkRevision, ComponentColumn,
    EntityLocation, FUN_ECS_ARCHETYPE_DEFAULT_CHUNK_CAPACITY,
};
pub use materialized_group::{
    MaterializedGroup, MaterializedGroupComponentMask, MaterializedGroupDigest,
    MaterializedGroupKind, MaterializedGroupMaintainer, MaterializedGroupRow,
    MaterializedGroupSourceRow,
};
pub use snapshot::{
    ResourceTableSnapshot, SnapshotConsumer, SnapshotGeneration, SnapshotLease,
    SnapshotRetireQueue, WorldSnapshot,
};
pub use sparse_pool::{
    FUN_ECS_SPARSE_DEFAULT_MAX_PAGES, FUN_ECS_SPARSE_DEFAULT_PAGE_SIZE, SparseChunkKey,
    SparseDenseIndex, SparsePage, SparsePagedPool, SparsePagedPoolConfig,
};
pub use table_query::{
    TableAdded, TableChanged, TableChunkQuery, TableFilter, TableQuery, TableQueryRow,
};

pub const FUN_ECS_STORAGE_EXPERIMENTS_REQUIRE_SAFE_RUST: bool = true;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunEcsExperimentKind {
    #[default]
    StoragePromotion = 0,
    MaterializedGroups = 1,
    SpeculativeSystems = 2,
    TemporalSnapshots = 3,
    ChunkedArchetypeTables = 4,
    SparsePagedComponentPools = 5,
    EntitylessTableQueries = 6,
}

impl FunEcsExperimentKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::StoragePromotion => "storage_promotion",
            Self::MaterializedGroups => "materialized_groups",
            Self::SpeculativeSystems => "speculative_systems",
            Self::TemporalSnapshots => "temporal_snapshots",
            Self::ChunkedArchetypeTables => "chunked_archetype_tables",
            Self::SparsePagedComponentPools => "sparse_paged_component_pools",
            Self::EntitylessTableQueries => "entityless_table_queries",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StorageExperimentError {
    InvalidEntity,
    DuplicateEntity,
    MissingEntity,
    InvalidComponent,
    SparsePageSizeZero,
    SparsePoolFull,
    InvalidMaterializedGroup,
    InvalidSnapshotGeneration,
}

impl StorageExperimentError {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::InvalidEntity => "invalid_entity",
            Self::DuplicateEntity => "duplicate_entity",
            Self::MissingEntity => "missing_entity",
            Self::InvalidComponent => "invalid_component",
            Self::SparsePageSizeZero => "sparse_page_size_zero",
            Self::SparsePoolFull => "sparse_pool_full",
            Self::InvalidMaterializedGroup => "invalid_materialized_group",
            Self::InvalidSnapshotGeneration => "invalid_snapshot_generation",
        }
    }
}
