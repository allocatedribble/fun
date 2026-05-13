#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsBenchmarkSuiteKind {
    SpatialBaseline = 0,
    EcsEntityKernel = 1,
    EcsResourceTableKernel = 2,
    EcsSchedulerKernel = 3,
    ProceduralTerrainPrototype = 4,
}

impl EcsBenchmarkSuiteKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SpatialBaseline => "spatial_baseline",
            Self::EcsEntityKernel => "ecs_entity_kernel",
            Self::EcsResourceTableKernel => "ecs_resource_table_kernel",
            Self::EcsSchedulerKernel => "ecs_scheduler_kernel",
            Self::ProceduralTerrainPrototype => "procedural_terrain_prototype",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsBenchmarkWorkloadKind {
    BuildInterest = 0,
    DiffRequests = 1,
    DecodePages = 2,
    BuildArtifacts = 3,
    PublishHandoffs = 4,
    DirtyPropagation = 5,
    EntitySpawnDespawn = 16,
    EntityAddRemoveComponent = 17,
    EntityQueryDenseArchetype = 18,
    EntityQuerySparseComponent = 19,
    EntityChangedFilter = 20,
    EntityCommandApply = 21,
    ResourceTableInsertRows = 32,
    ResourceTableScanRows = 33,
    ResourceTableChunkRows = 34,
    ResourceTableUpdatePriorities = 35,
    ResourceTableUpdateEpochs = 36,
    ResourceTableCompactEvict = 37,
    ResourceTableDigest = 38,
    SchedulerCompileSpatialSets = 48,
    SchedulerCompileSystems = 49,
    SchedulerCompileChunkedDecodeArtifactGraph = 50,
    SchedulerRunDeterministicGraph = 51,
    SchedulerRunDeterministicParallelGraph = 52,
    SchedulerApplyBarriers = 53,
    ProceduralTerrainColdSpawn = 64,
    ProceduralTerrainStreamingTreadmill = 65,
    ProceduralTerrainTeleport = 66,
    ProceduralTerrainMultiplayerDigest = 67,
    ProceduralTerrainNegativeCoordinateWorld = 68,
}

impl EcsBenchmarkWorkloadKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BuildInterest => "build_interest",
            Self::DiffRequests => "diff_requests",
            Self::DecodePages => "decode_pages",
            Self::BuildArtifacts => "build_artifacts",
            Self::PublishHandoffs => "publish_handoffs",
            Self::DirtyPropagation => "dirty_propagation",
            Self::EntitySpawnDespawn => "entity_spawn_despawn",
            Self::EntityAddRemoveComponent => "entity_add_remove_component",
            Self::EntityQueryDenseArchetype => "entity_query_dense_archetype",
            Self::EntityQuerySparseComponent => "entity_query_sparse_component",
            Self::EntityChangedFilter => "entity_changed_filter",
            Self::EntityCommandApply => "entity_command_apply",
            Self::ResourceTableInsertRows => "resource_table_insert_rows",
            Self::ResourceTableScanRows => "resource_table_scan_rows",
            Self::ResourceTableChunkRows => "resource_table_chunk_rows",
            Self::ResourceTableUpdatePriorities => "resource_table_update_priorities",
            Self::ResourceTableUpdateEpochs => "resource_table_update_epochs",
            Self::ResourceTableCompactEvict => "resource_table_compact_evict",
            Self::ResourceTableDigest => "resource_table_digest",
            Self::SchedulerCompileSpatialSets => "scheduler_compile_spatial_sets",
            Self::SchedulerCompileSystems => "scheduler_compile_systems",
            Self::SchedulerCompileChunkedDecodeArtifactGraph => {
                "scheduler_compile_chunked_decode_artifact_graph"
            }
            Self::SchedulerRunDeterministicGraph => "scheduler_run_deterministic_graph",
            Self::SchedulerRunDeterministicParallelGraph => {
                "scheduler_run_deterministic_parallel_graph"
            }
            Self::SchedulerApplyBarriers => "scheduler_apply_barriers",
            Self::ProceduralTerrainColdSpawn => "procedural_terrain_cold_spawn",
            Self::ProceduralTerrainStreamingTreadmill => "procedural_terrain_streaming_treadmill",
            Self::ProceduralTerrainTeleport => "procedural_terrain_teleport",
            Self::ProceduralTerrainMultiplayerDigest => "procedural_terrain_multiplayer_digest",
            Self::ProceduralTerrainNegativeCoordinateWorld => {
                "procedural_terrain_negative_coordinate_world"
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsBenchmarkConsumer {
    #[default]
    None = 0,
    Renderer = 1,
    Lux = 2,
    AvisPhysics = 3,
    Thunder = 4,
}

impl EcsBenchmarkConsumer {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Renderer => "renderer",
            Self::Lux => "lux",
            Self::AvisPhysics => "avis_physics",
            Self::Thunder => "thunder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsBenchmarkWorkload {
    pub suite: EcsBenchmarkSuiteKind,
    pub kind: EcsBenchmarkWorkloadKind,
    pub label: &'static str,
    pub cameras: u16,
    pub rows: u32,
    pub systems: u32,
    pub consumer: EcsBenchmarkConsumer,
}

impl EcsBenchmarkWorkload {
    #[must_use]
    pub const fn spatial(
        kind: EcsBenchmarkWorkloadKind,
        label: &'static str,
        cameras: u16,
        rows: u32,
        consumer: EcsBenchmarkConsumer,
    ) -> Self {
        Self {
            suite: EcsBenchmarkSuiteKind::SpatialBaseline,
            kind,
            label,
            cameras,
            rows,
            systems: 0,
            consumer,
        }
    }

    #[must_use]
    pub const fn entity(kind: EcsBenchmarkWorkloadKind, label: &'static str) -> Self {
        Self {
            suite: EcsBenchmarkSuiteKind::EcsEntityKernel,
            kind,
            label,
            cameras: 0,
            rows: 0,
            systems: 0,
            consumer: EcsBenchmarkConsumer::None,
        }
    }

    #[must_use]
    pub const fn table(kind: EcsBenchmarkWorkloadKind, label: &'static str) -> Self {
        Self {
            suite: EcsBenchmarkSuiteKind::EcsResourceTableKernel,
            kind,
            label,
            cameras: 0,
            rows: 0,
            systems: 0,
            consumer: EcsBenchmarkConsumer::None,
        }
    }

    #[must_use]
    pub const fn scheduler(
        kind: EcsBenchmarkWorkloadKind,
        label: &'static str,
        systems: u32,
    ) -> Self {
        Self {
            suite: EcsBenchmarkSuiteKind::EcsSchedulerKernel,
            kind,
            label,
            cameras: 0,
            rows: 0,
            systems,
            consumer: EcsBenchmarkConsumer::None,
        }
    }

    #[must_use]
    pub const fn procedural_terrain(
        kind: EcsBenchmarkWorkloadKind,
        label: &'static str,
        cameras: u16,
        rows: u32,
    ) -> Self {
        Self {
            suite: EcsBenchmarkSuiteKind::ProceduralTerrainPrototype,
            kind,
            label,
            cameras,
            rows,
            systems: 0,
            consumer: EcsBenchmarkConsumer::Renderer,
        }
    }
}

pub const ECS_SPATIAL_BASELINE_BENCHMARKS: [EcsBenchmarkWorkload; 18] = [
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::BuildInterest,
        "build_interest_1_camera",
        1,
        0,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::BuildInterest,
        "build_interest_8_cameras",
        8,
        0,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::BuildInterest,
        "build_interest_64_cameras",
        64,
        0,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DiffRequests,
        "diff_requests_empty_residency",
        0,
        0,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DiffRequests,
        "diff_requests_100k_pages",
        0,
        100_000,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DiffRequests,
        "diff_requests_1m_pages",
        0,
        1_000_000,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DecodePages,
        "decode_1k_source_rows",
        0,
        1_000,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DecodePages,
        "decode_32k_source_rows",
        0,
        32_000,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::BuildArtifacts,
        "build_artifacts_one_decoded_page",
        0,
        1,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::BuildArtifacts,
        "build_artifacts_1k_decoded_pages",
        0,
        1_000,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::BuildArtifacts,
        "build_artifacts_32k_decoded_pages",
        0,
        32_000,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::PublishHandoffs,
        "publish_handoffs_renderer",
        0,
        1_024,
        EcsBenchmarkConsumer::Renderer,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::PublishHandoffs,
        "publish_handoffs_lux",
        0,
        1_024,
        EcsBenchmarkConsumer::Lux,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::PublishHandoffs,
        "publish_handoffs_physics",
        0,
        1_024,
        EcsBenchmarkConsumer::AvisPhysics,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::PublishHandoffs,
        "publish_handoffs_thunder",
        0,
        1_024,
        EcsBenchmarkConsumer::Thunder,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DirtyPropagation,
        "dirty_propagation_1_voxel_edit",
        0,
        1,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DirtyPropagation,
        "dirty_propagation_1k_dirty_regions",
        0,
        1_000,
        EcsBenchmarkConsumer::None,
    ),
    EcsBenchmarkWorkload::spatial(
        EcsBenchmarkWorkloadKind::DirtyPropagation,
        "dirty_propagation_16k_dirty_regions",
        0,
        16_000,
        EcsBenchmarkConsumer::None,
    ),
];

pub const ECS_ENTITY_KERNEL_BENCHMARKS: [EcsBenchmarkWorkload; 6] = [
    EcsBenchmarkWorkload::entity(
        EcsBenchmarkWorkloadKind::EntitySpawnDespawn,
        "entity_spawn_despawn",
    ),
    EcsBenchmarkWorkload::entity(
        EcsBenchmarkWorkloadKind::EntityAddRemoveComponent,
        "entity_add_remove_component",
    ),
    EcsBenchmarkWorkload::entity(
        EcsBenchmarkWorkloadKind::EntityQueryDenseArchetype,
        "entity_query_dense_archetype",
    ),
    EcsBenchmarkWorkload::entity(
        EcsBenchmarkWorkloadKind::EntityQuerySparseComponent,
        "entity_query_sparse_component",
    ),
    EcsBenchmarkWorkload::entity(
        EcsBenchmarkWorkloadKind::EntityChangedFilter,
        "changed_filter",
    ),
    EcsBenchmarkWorkload::entity(
        EcsBenchmarkWorkloadKind::EntityCommandApply,
        "command_apply",
    ),
];

pub const ECS_RESOURCE_TABLE_KERNEL_BENCHMARKS: [EcsBenchmarkWorkload; 7] = [
    EcsBenchmarkWorkload::table(
        EcsBenchmarkWorkloadKind::ResourceTableInsertRows,
        "insert_rows",
    ),
    EcsBenchmarkWorkload::table(EcsBenchmarkWorkloadKind::ResourceTableScanRows, "scan_rows"),
    EcsBenchmarkWorkload::table(
        EcsBenchmarkWorkloadKind::ResourceTableChunkRows,
        "chunk_rows",
    ),
    EcsBenchmarkWorkload::table(
        EcsBenchmarkWorkloadKind::ResourceTableUpdatePriorities,
        "update_priorities",
    ),
    EcsBenchmarkWorkload::table(
        EcsBenchmarkWorkloadKind::ResourceTableUpdateEpochs,
        "update_epochs",
    ),
    EcsBenchmarkWorkload::table(
        EcsBenchmarkWorkloadKind::ResourceTableCompactEvict,
        "compact_evict",
    ),
    EcsBenchmarkWorkload::table(EcsBenchmarkWorkloadKind::ResourceTableDigest, "digest"),
];

pub const ECS_SCHEDULER_KERNEL_BENCHMARKS: [EcsBenchmarkWorkload; 7] = [
    EcsBenchmarkWorkload::scheduler(
        EcsBenchmarkWorkloadKind::SchedulerCompileSpatialSets,
        "compile_15_spatial_sets",
        15,
    ),
    EcsBenchmarkWorkload::scheduler(
        EcsBenchmarkWorkloadKind::SchedulerCompileSystems,
        "compile_100_systems",
        100,
    ),
    EcsBenchmarkWorkload::scheduler(
        EcsBenchmarkWorkloadKind::SchedulerCompileSystems,
        "compile_1k_systems",
        1_000,
    ),
    EcsBenchmarkWorkload::scheduler(
        EcsBenchmarkWorkloadKind::SchedulerCompileChunkedDecodeArtifactGraph,
        "compile_chunked_decode_artifact_graph",
        0,
    ),
    EcsBenchmarkWorkload::scheduler(
        EcsBenchmarkWorkloadKind::SchedulerRunDeterministicGraph,
        "run_deterministic_graph",
        0,
    ),
    EcsBenchmarkWorkload::scheduler(
        EcsBenchmarkWorkloadKind::SchedulerRunDeterministicParallelGraph,
        "run_deterministic_parallel_graph",
        0,
    ),
    EcsBenchmarkWorkload::scheduler(
        EcsBenchmarkWorkloadKind::SchedulerApplyBarriers,
        "apply_barriers",
        0,
    ),
];

pub const ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS: [EcsBenchmarkWorkload; 5] = [
    EcsBenchmarkWorkload::procedural_terrain(
        EcsBenchmarkWorkloadKind::ProceduralTerrainColdSpawn,
        "cold_spawn_required_2_desired_5",
        1,
        1_331,
    ),
    EcsBenchmarkWorkload::procedural_terrain(
        EcsBenchmarkWorkloadKind::ProceduralTerrainStreamingTreadmill,
        "streaming_treadmill_12_frames",
        1,
        343,
    ),
    EcsBenchmarkWorkload::procedural_terrain(
        EcsBenchmarkWorkloadKind::ProceduralTerrainTeleport,
        "teleport_far_region",
        1,
        1_331,
    ),
    EcsBenchmarkWorkload::procedural_terrain(
        EcsBenchmarkWorkloadKind::ProceduralTerrainMultiplayerDigest,
        "multiplayer_digest_server_two_clients",
        3,
        125,
    ),
    EcsBenchmarkWorkload::procedural_terrain(
        EcsBenchmarkWorkloadKind::ProceduralTerrainNegativeCoordinateWorld,
        "negative_coordinate_world",
        1,
        343,
    ),
];

pub const ECS_BENCHMARK_TOTAL_WORKLOADS: usize = ECS_SPATIAL_BASELINE_BENCHMARKS.len()
    + ECS_ENTITY_KERNEL_BENCHMARKS.len()
    + ECS_RESOURCE_TABLE_KERNEL_BENCHMARKS.len()
    + ECS_SCHEDULER_KERNEL_BENCHMARKS.len()
    + ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS.len();

pub fn ecs_benchmark_workloads() -> impl Iterator<Item = &'static EcsBenchmarkWorkload> {
    ECS_SPATIAL_BASELINE_BENCHMARKS
        .iter()
        .chain(ECS_ENTITY_KERNEL_BENCHMARKS.iter())
        .chain(ECS_RESOURCE_TABLE_KERNEL_BENCHMARKS.iter())
        .chain(ECS_SCHEDULER_KERNEL_BENCHMARKS.iter())
        .chain(ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS.iter())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contains_spatial(
        kind: EcsBenchmarkWorkloadKind,
        cameras: u16,
        rows: u32,
        consumer: EcsBenchmarkConsumer,
    ) -> bool {
        ECS_SPATIAL_BASELINE_BENCHMARKS.iter().any(|workload| {
            workload.kind == kind
                && workload.cameras == cameras
                && workload.rows == rows
                && workload.consumer == consumer
        })
    }

    #[test]
    fn spatial_baseline_covers_requested_workloads() {
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::BuildInterest,
            1,
            0,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::BuildInterest,
            8,
            0,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::BuildInterest,
            64,
            0,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::DiffRequests,
            0,
            100_000,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::DiffRequests,
            0,
            1_000_000,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::DecodePages,
            0,
            32_000,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::BuildArtifacts,
            0,
            32_000,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::PublishHandoffs,
            0,
            1_024,
            EcsBenchmarkConsumer::Thunder
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::DirtyPropagation,
            0,
            1_000,
            EcsBenchmarkConsumer::None
        ));
        assert!(contains_spatial(
            EcsBenchmarkWorkloadKind::DirtyPropagation,
            0,
            16_000,
            EcsBenchmarkConsumer::None
        ));
    }

    #[test]
    fn ecs_kernel_catalog_covers_entity_table_and_scheduler_gates() {
        assert_eq!(ECS_ENTITY_KERNEL_BENCHMARKS.len(), 6);
        assert_eq!(ECS_RESOURCE_TABLE_KERNEL_BENCHMARKS.len(), 7);
        assert_eq!(ECS_SCHEDULER_KERNEL_BENCHMARKS.len(), 7);
        assert!(
            ECS_SCHEDULER_KERNEL_BENCHMARKS
                .iter()
                .any(|workload| workload.systems == 100)
        );
        assert!(
            ECS_SCHEDULER_KERNEL_BENCHMARKS
                .iter()
                .any(|workload| workload.systems == 1_000)
        );
        assert_eq!(
            ecs_benchmark_workloads().count(),
            ECS_BENCHMARK_TOTAL_WORKLOADS
        );
    }

    #[test]
    fn procedural_terrain_catalog_covers_requested_prototype_benchmarks() {
        assert_eq!(ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS.len(), 5);
        for kind in [
            EcsBenchmarkWorkloadKind::ProceduralTerrainColdSpawn,
            EcsBenchmarkWorkloadKind::ProceduralTerrainStreamingTreadmill,
            EcsBenchmarkWorkloadKind::ProceduralTerrainTeleport,
            EcsBenchmarkWorkloadKind::ProceduralTerrainMultiplayerDigest,
            EcsBenchmarkWorkloadKind::ProceduralTerrainNegativeCoordinateWorld,
        ] {
            assert!(
                ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS
                    .iter()
                    .any(|workload| workload.kind == kind
                        && workload.suite == EcsBenchmarkSuiteKind::ProceduralTerrainPrototype)
            );
        }
    }
}
