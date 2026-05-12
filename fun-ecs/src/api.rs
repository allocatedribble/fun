use fun_scheduler_core::Runtime;
use fun_scheduler_types::{
    EcsVirtualResourceKey, GraphInvariantError, ProductRegistry, WorkGraphId,
};

use crate::{
    DenseResourceTable, DenseResourceTableChunk, DenseResourceTableKey, EcsDerivedArtifactRecord,
    EcsNodeError, EcsSpatialScheduleCompileError, EcsSpatialScheduleCompileOutput,
    FunCommandApplyReport, FunCommandJournal, FunExternalSlabId, FunRevision, FunSystem,
    FunSystemDescriptor, FunWorld, FunWorldRevision, IntoSchedulerVirtualResourceKey,
    ScheduleRevision, external_slab_virtual_resource_key,
};

pub use crate::{
    ArtifactDag, ArtifactReadinessToken, Commands, EcsNodeRunner, FunFrameContext, FunRunCondition,
    FunSystemExecutionContract, FunSystemSet, FunWorldBuilder, SpatialCommands,
};
pub use crate::{
    DeterministicMathMode, EcsBiomeRecipe, EcsProceduralTerrainSource, EcsProceduralWorldManifest,
    EcsTerrainGeneratorVersion, NetworkPlayerId, ProceduralFeature, ProceduralFeatureMask,
    ProceduralPageDigest, ProceduralPageDigestProbe, ProceduralTerrainProfileId,
    ProceduralWorldAuthorityPolicy, ProceduralWorldSyncManifest, WorldOriginPolicy,
    generate_procedural_page_digest, generate_procedural_terrain_page,
};

pub type World = FunWorld;
pub type ResourceTable<T> = DenseResourceTable<ResourceTableKey, T>;
pub type ResourceTableChunk<T> = DenseResourceTableChunk<ResourceTableKey, T>;
pub type CommandJournal<C> = FunCommandJournal<C>;
pub type CommandApplyReport = FunCommandApplyReport;
pub type ArtifactManifest = EcsDerivedArtifactRecord;
pub type CrossDomainHandoffQueue = crate::EcsCrossDomainHandoffQueues;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceTableKey(pub u64);

impl ResourceTableKey {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

impl DenseResourceTableKey for ResourceTableKey {
    fn is_valid(self) -> bool {
        self.is_valid()
    }

    fn stable_key(self) -> u64 {
        self.get()
    }
}

impl crate::DenseSlotKey for ResourceTableKey {
    fn is_valid(self) -> bool {
        self.is_valid()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VirtualResourceHandle {
    pub key: EcsVirtualResourceKey,
}

impl VirtualResourceHandle {
    #[must_use]
    pub const fn new(key: EcsVirtualResourceKey) -> Self {
        Self { key }
    }
}

impl From<EcsVirtualResourceKey> for VirtualResourceHandle {
    fn from(key: EcsVirtualResourceKey) -> Self {
        Self::new(key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalSlabHandle {
    pub id: FunExternalSlabId,
    pub virtual_resource: VirtualResourceHandle,
}

impl ExternalSlabHandle {
    #[must_use]
    pub const fn new(id: FunExternalSlabId) -> Self {
        Self {
            id,
            virtual_resource: VirtualResourceHandle::new(external_slab_virtual_resource_key(id)),
        }
    }
}

impl IntoSchedulerVirtualResourceKey for ExternalSlabHandle {
    fn into_scheduler_virtual_resource_key(self) -> EcsVirtualResourceKey {
        self.virtual_resource.key
    }
}

pub trait IntoFunSystem {
    #[must_use]
    fn into_fun_system(self) -> FunSystem;
}

impl IntoFunSystem for FunSystem {
    fn into_fun_system(self) -> FunSystem {
        self
    }
}

impl IntoFunSystem for FunSystemDescriptor {
    fn into_fun_system(self) -> FunSystem {
        FunSystem::new(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSchedule {
    pub graph_id: WorkGraphId,
    pub observed_revision: ScheduleRevision,
}

impl Default for FunSchedule {
    fn default() -> Self {
        Self {
            graph_id: WorkGraphId::new(1_600),
            observed_revision: ScheduleRevision::INITIAL,
        }
    }
}

impl FunSchedule {
    #[must_use]
    pub const fn new(graph_id: WorkGraphId) -> Self {
        Self {
            graph_id,
            observed_revision: ScheduleRevision::INITIAL,
        }
    }

    #[must_use]
    pub const fn with_observed_revision(mut self, revision: ScheduleRevision) -> Self {
        self.observed_revision = revision;
        self
    }

    #[must_use]
    pub const fn from_world_revision(mut self, revision: FunWorldRevision) -> Self {
        self.observed_revision = ScheduleRevision::from_revision(FunRevision::new(revision.get()));
        self
    }
}

pub fn compile_schedule_graph(
    schedule: FunSchedule,
) -> Result<crate::FunEcsScheduleWorkGraph, GraphInvariantError> {
    crate::compile_spatial_schedule_work_graph_at_revision(
        schedule.graph_id,
        schedule.observed_revision,
    )
}

pub fn compile_spatial_frame_graph(
    world: &FunWorld,
) -> Result<EcsSpatialScheduleCompileOutput, EcsSpatialScheduleCompileError> {
    world.compile_spatial_frame_graph()
}

pub fn submit_to_fun_scheduler(
    world: &mut FunWorld,
    runtime: &Runtime<ProductRegistry>,
) -> Result<crate::EcsSpatialFrameRunReport, EcsNodeError> {
    world.submit_spatial_frame(runtime)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunStableApiArea {
    World = 0,
    Storage = 1,
    Systems = 2,
    Commands = 3,
    Scheduler = 4,
    Artifacts = 5,
    Terrain = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunStableApiSymbol {
    pub area: FunStableApiArea,
    pub name: &'static str,
}

pub const FUN_ECS_STABLE_API_SYMBOLS: [FunStableApiSymbol; 43] = [
    FunStableApiSymbol {
        area: FunStableApiArea::World,
        name: "World",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::World,
        name: "FunWorld",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::World,
        name: "FunWorldBuilder",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::World,
        name: "FunWorldRevision",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::World,
        name: "FunFrameContext",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Storage,
        name: "ResourceTable",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Storage,
        name: "ResourceTableKey",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Storage,
        name: "ResourceTableChunk",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Storage,
        name: "ExternalSlabHandle",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Storage,
        name: "VirtualResourceHandle",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Systems,
        name: "IntoFunSystem",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Systems,
        name: "FunSystemSet",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Systems,
        name: "FunSchedule",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Systems,
        name: "FunRunCondition",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Systems,
        name: "FunSystemExecutionContract",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Commands,
        name: "Commands",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Commands,
        name: "SpatialCommands",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Commands,
        name: "CommandJournal",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Commands,
        name: "CommandApplyReport",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Scheduler,
        name: "compile_schedule_graph",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Scheduler,
        name: "compile_spatial_frame_graph",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Scheduler,
        name: "submit_to_fun_scheduler",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Scheduler,
        name: "EcsNodeRunner",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Artifacts,
        name: "ArtifactDag",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Artifacts,
        name: "ArtifactManifest",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Artifacts,
        name: "ArtifactReadinessToken",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Artifacts,
        name: "CrossDomainHandoffQueue",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "EcsProceduralWorldManifest",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "EcsBiomeRecipe",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "EcsProceduralTerrainSource",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "EcsTerrainGeneratorVersion",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "ProceduralTerrainProfileId",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "NetworkPlayerId",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "ProceduralWorldSyncManifest",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "WorldOriginPolicy",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "DeterministicMathMode",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "ProceduralFeature",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "ProceduralFeatureMask",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "ProceduralWorldAuthorityPolicy",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "ProceduralPageDigest",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "ProceduralPageDigestProbe",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "generate_procedural_terrain_page",
    },
    FunStableApiSymbol {
        area: FunStableApiArea::Terrain,
        name: "generate_procedural_page_digest",
    },
];

#[must_use]
pub const fn stable_api_symbols() -> &'static [FunStableApiSymbol; 43] {
    &FUN_ECS_STABLE_API_SYMBOLS
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunExperimentalApiFeature {
    NativeArchetypes = 0,
    StoragePromotion = 1,
    MaterializedGroups = 2,
    TemporalSnapshots = 3,
    SpeculativeSystems = 4,
    UnsafeStorage = 5,
}

impl FunExperimentalApiFeature {
    #[must_use]
    pub const fn cargo_feature(self) -> &'static str {
        match self {
            Self::NativeArchetypes => "experimental-native-archetypes",
            Self::StoragePromotion => "experimental-storage-promotion",
            Self::MaterializedGroups => "experimental-materialized-groups",
            Self::TemporalSnapshots => "experimental-temporal-snapshots",
            Self::SpeculativeSystems => "experimental-speculative-systems",
            Self::UnsafeStorage => "experimental-unsafe-storage",
        }
    }

    #[must_use]
    pub const fn enabled(self) -> bool {
        match self {
            Self::NativeArchetypes => cfg!(feature = "experimental-native-archetypes"),
            Self::StoragePromotion => cfg!(feature = "experimental-storage-promotion"),
            Self::MaterializedGroups => cfg!(feature = "experimental-materialized-groups"),
            Self::TemporalSnapshots => cfg!(feature = "experimental-temporal-snapshots"),
            Self::SpeculativeSystems => cfg!(feature = "experimental-speculative-systems"),
            Self::UnsafeStorage => cfg!(feature = "experimental-unsafe-storage"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunExperimentalApiGate {
    pub feature: FunExperimentalApiFeature,
    pub cargo_feature: &'static str,
    pub enabled: bool,
    pub enabled_by_default: bool,
    pub product_critical_allowed: bool,
}

impl FunExperimentalApiGate {
    #[must_use]
    pub const fn from_feature(feature: FunExperimentalApiFeature) -> Self {
        Self {
            feature,
            cargo_feature: feature.cargo_feature(),
            enabled: feature.enabled(),
            enabled_by_default: false,
            product_critical_allowed: false,
        }
    }
}

#[must_use]
pub const fn experimental_api_gates() -> [FunExperimentalApiGate; 6] {
    [
        FunExperimentalApiGate::from_feature(FunExperimentalApiFeature::NativeArchetypes),
        FunExperimentalApiGate::from_feature(FunExperimentalApiFeature::StoragePromotion),
        FunExperimentalApiGate::from_feature(FunExperimentalApiFeature::MaterializedGroups),
        FunExperimentalApiGate::from_feature(FunExperimentalApiFeature::TemporalSnapshots),
        FunExperimentalApiGate::from_feature(FunExperimentalApiFeature::SpeculativeSystems),
        FunExperimentalApiGate::from_feature(FunExperimentalApiFeature::UnsafeStorage),
    ]
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunPublicApiContractReport {
    pub stable_symbol_count: u16,
    pub experimental_gate_count: u16,
    pub enabled_by_default_experimental_count: u16,
    pub product_critical_experimental_count: u16,
}

#[must_use]
pub fn public_api_contract_report() -> FunPublicApiContractReport {
    let gates = experimental_api_gates();
    FunPublicApiContractReport {
        stable_symbol_count: FUN_ECS_STABLE_API_SYMBOLS.len() as u16,
        experimental_gate_count: gates.len() as u16,
        enabled_by_default_experimental_count: gates
            .iter()
            .filter(|gate| gate.enabled_by_default)
            .count() as u16,
        product_critical_experimental_count: gates
            .iter()
            .filter(|gate| gate.product_critical_allowed)
            .count() as u16,
    }
}

pub mod experimental {
    #[cfg(feature = "experimental-native-archetypes")]
    pub use crate::experiments::{
        Archetype, ArchetypeChunk, ArchetypeTable, ChunkDirtyMask, ChunkRevision, ComponentColumn,
        EntityLocation,
    };

    #[cfg(feature = "experimental-materialized-groups")]
    pub use crate::experiments::{
        MaterializedGroup, MaterializedGroupComponentMask, MaterializedGroupDigest,
        MaterializedGroupKind, MaterializedGroupMaintainer, MaterializedGroupRow,
        MaterializedGroupSourceRow,
    };

    #[cfg(feature = "experimental-temporal-snapshots")]
    pub use crate::experiments::{
        ResourceTableSnapshot, SnapshotConsumer, SnapshotGeneration, SnapshotLease,
        SnapshotRetireQueue, WorldSnapshot,
    };

    #[cfg(feature = "experimental-storage-promotion")]
    pub use crate::experiments::{
        SparseChunkKey, SparseDenseIndex, SparsePage, SparsePagedPool, SparsePagedPoolConfig,
        TableAdded, TableChanged, TableChunkQuery, TableFilter, TableQuery, TableQueryRow,
    };
}

#[cfg(test)]
mod tests {
    use crate::{
        FunEcsResourceKind, FunResourceTableId, FunSystemDescriptor, FunSystemId, FunWorldBuilder,
        ResourceTableLayout,
    };

    use super::*;

    fn has_symbol(name: &str) -> bool {
        stable_api_symbols()
            .iter()
            .any(|symbol| symbol.name == name)
    }

    #[test]
    fn minimal_stable_api_lists_requested_symbols() {
        for name in [
            "World",
            "FunWorld",
            "FunWorldBuilder",
            "FunWorldRevision",
            "FunFrameContext",
            "ResourceTable",
            "ResourceTableKey",
            "ResourceTableChunk",
            "ExternalSlabHandle",
            "VirtualResourceHandle",
            "IntoFunSystem",
            "FunSystemSet",
            "FunSchedule",
            "FunRunCondition",
            "FunSystemExecutionContract",
            "Commands",
            "SpatialCommands",
            "CommandJournal",
            "CommandApplyReport",
            "compile_schedule_graph",
            "compile_spatial_frame_graph",
            "submit_to_fun_scheduler",
            "EcsNodeRunner",
            "ArtifactDag",
            "ArtifactManifest",
            "ArtifactReadinessToken",
            "CrossDomainHandoffQueue",
            "EcsProceduralWorldManifest",
            "EcsBiomeRecipe",
            "EcsProceduralTerrainSource",
            "EcsTerrainGeneratorVersion",
            "ProceduralTerrainProfileId",
            "NetworkPlayerId",
            "ProceduralWorldSyncManifest",
            "WorldOriginPolicy",
            "DeterministicMathMode",
            "ProceduralFeature",
            "ProceduralFeatureMask",
            "ProceduralWorldAuthorityPolicy",
            "ProceduralPageDigest",
            "ProceduralPageDigestProbe",
            "generate_procedural_terrain_page",
            "generate_procedural_page_digest",
        ] {
            assert!(has_symbol(name), "missing stable API symbol {name}");
        }
        assert_eq!(stable_api_symbols().len(), 43);
    }

    #[test]
    fn stable_facade_compiles_world_schedule_table_and_system_shapes() {
        let world = FunWorldBuilder::default().build();
        assert_eq!(world.storage_backend, crate::FunWorldStorageBackend::Hybrid);

        let schedule = FunSchedule::default().from_world_revision(world.revision);
        let graph = compile_schedule_graph(schedule).expect("compile stable schedule graph");
        assert!(!graph.nodes.is_empty());

        let table: ResourceTable<u64> = ResourceTable::new(
            crate::DenseResourceTableConfig::new(
                FunEcsResourceKind::PageResidencyTable,
                FunResourceTableId::from_resource_kind(FunEcsResourceKind::PageResidencyTable),
                8,
            )
            .with_layout(ResourceTableLayout::AoS),
        );
        assert_eq!(table.len(), 0);

        let system =
            FunSystemDescriptor::new(FunSystemId::new(77), "stable_api_system").into_fun_system();
        assert_eq!(system.descriptor.label, "stable_api_system");

        let slab = ExternalSlabHandle::new(crate::FunExternalSlabId::new(12));
        assert_eq!(slab.id.get(), 12);
        assert_eq!(
            slab.into_scheduler_virtual_resource_key(),
            external_slab_virtual_resource_key(crate::FunExternalSlabId::new(12))
        );
    }

    #[test]
    fn experimental_api_gates_are_opt_in_and_not_product_critical() {
        let report = public_api_contract_report();
        assert_eq!(report.experimental_gate_count, 6);
        assert_eq!(report.enabled_by_default_experimental_count, 0);
        assert_eq!(report.product_critical_experimental_count, 0);

        let gates = experimental_api_gates();
        for gate in gates {
            assert!(!gate.enabled_by_default);
            assert!(!gate.product_critical_allowed);
            assert!(gate.cargo_feature.starts_with("experimental-"));
        }
    }
}
