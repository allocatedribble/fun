use fun_scheduler_types::{EcsWorkKind, WorkEdge};

pub mod benchmarks;
pub mod gates;
pub mod procedural_terrain;

pub use benchmarks::{
    ECS_BENCHMARK_TOTAL_WORKLOADS, ECS_ENTITY_KERNEL_BENCHMARKS,
    ECS_PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS, ECS_RESOURCE_TABLE_KERNEL_BENCHMARKS,
    ECS_SCHEDULER_KERNEL_BENCHMARKS, ECS_SPATIAL_BASELINE_BENCHMARKS, EcsBenchmarkConsumer,
    EcsBenchmarkSuiteKind, EcsBenchmarkWorkload, EcsBenchmarkWorkloadKind, ecs_benchmark_workloads,
};
pub use gates::{
    EcsReplacementGateCategory, EcsReplacementGateInput, EcsReplacementGateRejectReason,
    EcsReplacementGateReport,
};
pub use procedural_terrain::{
    PROCEDURAL_TERRAIN_BENCHMARK_SEED, PROCEDURAL_TERRAIN_COLD_SPAWN_CAMERA_FT,
    PROCEDURAL_TERRAIN_COLD_SPAWN_DESIRED_SHELLS, PROCEDURAL_TERRAIN_COLD_SPAWN_REQUIRED_SHELLS,
    PROCEDURAL_TERRAIN_NEGATIVE_CAMERA_FT, PROCEDURAL_TERRAIN_PROTOTYPE_BENCHMARKS,
    PROCEDURAL_TERRAIN_TELEPORT_DESTINATION_FT, PROCEDURAL_TERRAIN_TREADMILL_DESIRED_SHELLS,
    PROCEDURAL_TERRAIN_TREADMILL_FRAMES, PROCEDURAL_TERRAIN_TREADMILL_REQUIRED_SHELLS,
    PROCEDURAL_TERRAIN_TREADMILL_STEP_FT, ProceduralTerrainPrototypeBenchmarkKind,
    ProceduralTerrainPrototypeBenchmarkReport, run_procedural_terrain_prototype_benchmark,
};

use crate::{
    DenseResourceTableStats, ECS_SPATIAL_MAX_DECODED_PAGE_ROWS, ECS_SPATIAL_MAX_DERIVED_ARTIFACTS,
    ECS_SPATIAL_MAX_DIRTY_REGIONS, ECS_SPATIAL_MAX_HANDOFF_ROWS, ECS_SPATIAL_MAX_PAGE_RECORDS,
    ECS_SPATIAL_MAX_SOURCE_QUEUE_ROWS, ECS_SPATIAL_MAX_STREAM_INTERESTS,
    ECS_SPATIAL_MAX_STREAM_REQUESTS, ECS_SPATIAL_MAX_STREAM_WAVES, EcsArtifactState,
    EcsDerivedArtifactRegistry, EcsLuxHandoffQueue, EcsPhysicsCookQueue, EcsRendererHandoffQueue,
    EcsSpatialCommandApplyReport, EcsSpatialFrameRunReport, EcsSpatialProductWorkGraph,
    FunCommandApplyReport, FunEcsScheduleWorkGraph, FunFrameDigest, FunFrameSchedule, FunRevision,
    FunWorld, SnapshotGeneration, scheduler_bridge_report,
};

pub const ECS_DIAGNOSTICS_UI_VIEW_COUNT: usize = 7;
pub const ECS_DIAGNOSTICS_UI_VIEWS: [EcsDiagnosticsUiView; ECS_DIAGNOSTICS_UI_VIEW_COUNT] = [
    EcsDiagnosticsUiView::ScheduleGraph,
    EcsDiagnosticsUiView::FrameFlow,
    EcsDiagnosticsUiView::CommandBarrier,
    EcsDiagnosticsUiView::ArtifactDag,
    EcsDiagnosticsUiView::TableHeatmap,
    EcsDiagnosticsUiView::SubsystemHandoffQueue,
    EcsDiagnosticsUiView::WaitTokenLedger,
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsGraphDebugSnapshot {
    pub node_count: u32,
    pub barrier_count: u32,
    pub dependency_edge_count: u32,
}

impl FunEcsGraphDebugSnapshot {
    #[must_use]
    pub fn from_schedule_graph(graph: &FunEcsScheduleWorkGraph) -> Self {
        let report = scheduler_bridge_report(graph);
        Self {
            node_count: report.nodes,
            barrier_count: report.barriers,
            dependency_edge_count: report.dependency_edges,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsStorageReport {
    pub entity_count: u64,
    pub resource_count: u32,
    pub table_row_count: u64,
    pub table_capacity: u64,
    pub chunk_count: u32,
    pub sparse_pool_count: u32,
    pub external_slab_handle_count: u32,
}

impl EcsStorageReport {
    #[must_use]
    pub const fn from_counts(
        entity_count: u64,
        resource_count: u32,
        table_row_count: u64,
        table_capacity: u64,
        chunk_count: u32,
        sparse_pool_count: u32,
        external_slab_handle_count: u32,
    ) -> Self {
        Self {
            entity_count,
            resource_count,
            table_row_count,
            table_capacity,
            chunk_count,
            sparse_pool_count,
            external_slab_handle_count,
        }
    }

    #[must_use]
    pub fn from_world(world: &FunWorld) -> Self {
        let table_row_count = world.spatial_page_table.len()
            + world.residency_table.keys.len()
            + world.dirty_ledger.regions.len()
            + world.stream_interest_table.interests.len()
            + world.stream_request_queue.requests.len()
            + world.source_acquire_queue.rows.len()
            + world.decoded_page_queue.rows.len()
            + world.derived_artifact_registry.len()
            + world.cross_domain_handoff_queues.rows.len()
            + world.renderer_handoff_queue.items.len()
            + world.lux_handoff_queue.items.len()
            + world.physics_cook_queue.items.len();
        Self {
            entity_count: 0,
            resource_count: u32::from(world.diagnostics.initialized_spatial_resources),
            table_row_count: table_row_count as u64,
            table_capacity: spatial_capacity_total(),
            chunk_count: 0,
            sparse_pool_count: 0,
            external_slab_handle_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsTableReport {
    pub table_count: u32,
    pub row_count: u64,
    pub table_capacity: u64,
    pub chunk_count: u32,
    pub sparse_pool_count: u32,
    pub external_slab_handle_count: u32,
}

impl EcsTableReport {
    #[must_use]
    pub const fn from_counts(
        table_count: u32,
        row_count: u64,
        table_capacity: u64,
        chunk_count: u32,
        sparse_pool_count: u32,
        external_slab_handle_count: u32,
    ) -> Self {
        Self {
            table_count,
            row_count,
            table_capacity,
            chunk_count,
            sparse_pool_count,
            external_slab_handle_count,
        }
    }

    #[must_use]
    pub const fn from_dense_stats(stats: DenseResourceTableStats) -> Self {
        Self {
            table_count: 1,
            row_count: stats.len as u64,
            table_capacity: stats.capacity as u64,
            chunk_count: 1,
            sparse_pool_count: 0,
            external_slab_handle_count: 0,
        }
    }

    #[must_use]
    pub fn from_world(world: &FunWorld) -> Self {
        let storage = EcsStorageReport::from_world(world);
        Self {
            table_count: u32::from(world.diagnostics.initialized_spatial_resources),
            row_count: storage.table_row_count,
            table_capacity: storage.table_capacity,
            chunk_count: storage.chunk_count,
            sparse_pool_count: storage.sparse_pool_count,
            external_slab_handle_count: storage.external_slab_handle_count,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsQueryReport {
    pub query_count: u32,
    pub component_read_count: u32,
    pub component_write_count: u32,
    pub resource_read_count: u32,
    pub resource_write_count: u32,
    pub table_read_count: u32,
    pub table_write_count: u32,
    pub chunk_read_count: u32,
    pub chunk_write_count: u32,
    pub external_slab_read_count: u32,
    pub external_slab_write_count: u32,
}

impl EcsQueryReport {
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the report is intentionally a flat machine-readable counter row"
    )]
    pub const fn from_counts(
        query_count: u32,
        component_read_count: u32,
        component_write_count: u32,
        resource_read_count: u32,
        resource_write_count: u32,
        table_read_count: u32,
        table_write_count: u32,
        chunk_read_count: u32,
        chunk_write_count: u32,
        external_slab_read_count: u32,
        external_slab_write_count: u32,
    ) -> Self {
        Self {
            query_count,
            component_read_count,
            component_write_count,
            resource_read_count,
            resource_write_count,
            table_read_count,
            table_write_count,
            chunk_read_count,
            chunk_write_count,
            external_slab_read_count,
            external_slab_write_count,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsScheduleReport {
    pub graph_build_time_ns: u64,
    pub node_count: u32,
    pub edge_count: u32,
    pub wait_token_count: u32,
    pub conflict_set_count: u32,
    pub barrier_count: u32,
    pub chunk_node_count: u32,
    pub liveness_reject_count: u32,
}

impl EcsScheduleReport {
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the report is intentionally a flat machine-readable counter row"
    )]
    pub const fn from_counts(
        graph_build_time_ns: u64,
        node_count: u32,
        edge_count: u32,
        wait_token_count: u32,
        conflict_set_count: u32,
        barrier_count: u32,
        chunk_node_count: u32,
        liveness_reject_count: u32,
    ) -> Self {
        Self {
            graph_build_time_ns,
            node_count,
            edge_count,
            wait_token_count,
            conflict_set_count,
            barrier_count,
            chunk_node_count,
            liveness_reject_count,
        }
    }

    #[must_use]
    pub fn from_schedule_graph(
        graph: &FunEcsScheduleWorkGraph,
        graph_build_time_ns: u64,
        liveness_reject_count: u32,
    ) -> Self {
        Self {
            graph_build_time_ns,
            node_count: graph.nodes.len() as u32,
            edge_count: graph.edges.len() as u32,
            wait_token_count: graph.wait_for_edges.len() as u32,
            conflict_set_count: graph.conflict_sets.len() as u32,
            barrier_count: graph.barriers.len() as u32,
            chunk_node_count: graph
                .nodes
                .iter()
                .filter(|node| node.work_descriptor.work_kind == EcsWorkKind::RunSystemChunk)
                .count() as u32,
            liveness_reject_count,
        }
    }

    #[must_use]
    pub fn from_spatial_product_graph(
        graph: &EcsSpatialProductWorkGraph,
        graph_build_time_ns: u64,
        liveness_reject_count: u32,
    ) -> Self {
        Self {
            graph_build_time_ns,
            node_count: graph.nodes.len() as u32,
            edge_count: graph.edges.len() as u32,
            wait_token_count: graph.wait_for_edges.len() as u32,
            conflict_set_count: graph.conflict_sets.len() as u32,
            barrier_count: graph.barriers.len() as u32,
            chunk_node_count: graph
                .nodes
                .iter()
                .filter(|node| node.work_descriptor.kind == EcsWorkKind::RunSystemChunk)
                .count() as u32,
            liveness_reject_count,
        }
    }

    #[must_use]
    pub fn from_frame_schedule(
        schedule: &FunFrameSchedule,
        graph_build_time_ns: u64,
        liveness_reject_count: u32,
    ) -> Self {
        Self {
            graph_build_time_ns,
            node_count: schedule.graph.nodes.len() as u32,
            edge_count: schedule.graph.edges.len() as u32,
            wait_token_count: schedule
                .graph
                .nodes
                .iter()
                .map(|node| {
                    node.work_descriptor.awaited_tokens.len()
                        + node.work_descriptor.produced_tokens.len()
                })
                .sum::<usize>() as u32,
            conflict_set_count: schedule.graph.conflict_sets.len() as u32,
            barrier_count: schedule.graph.barriers.len() as u32,
            chunk_node_count: schedule
                .graph
                .nodes
                .iter()
                .filter(|node| node.work_descriptor.kind == EcsWorkKind::RunSystemChunk)
                .count() as u32,
            liveness_reject_count,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsCommandReport {
    pub buffers_emitted: u32,
    pub commands_recorded: u64,
    pub commands_applied: u64,
    pub commands_rejected: u64,
    pub stale_revisions: u64,
    pub merge_time_ns: u64,
    pub apply_time_ns: u64,
}

impl EcsCommandReport {
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the report is intentionally a flat machine-readable counter row"
    )]
    pub const fn from_counts(
        buffers_emitted: u32,
        commands_recorded: u64,
        commands_applied: u64,
        commands_rejected: u64,
        stale_revisions: u64,
        merge_time_ns: u64,
        apply_time_ns: u64,
    ) -> Self {
        Self {
            buffers_emitted,
            commands_recorded,
            commands_applied,
            commands_rejected,
            stale_revisions,
            merge_time_ns,
            apply_time_ns,
        }
    }

    #[must_use]
    pub const fn from_fun_command_apply(
        buffers_emitted: u32,
        report: &FunCommandApplyReport,
        stale_revisions: u64,
        merge_time_ns: u64,
        apply_time_ns: u64,
    ) -> Self {
        Self {
            buffers_emitted,
            commands_recorded: report.inspected as u64,
            commands_applied: report.applied as u64,
            commands_rejected: report.rejected as u64,
            stale_revisions,
            merge_time_ns,
            apply_time_ns,
        }
    }

    #[must_use]
    pub const fn from_spatial_apply(
        buffers_emitted: u32,
        report: &EcsSpatialCommandApplyReport,
        stale_revisions: u64,
        merge_time_ns: u64,
        apply_time_ns: u64,
    ) -> Self {
        Self {
            buffers_emitted,
            commands_recorded: report.inspected as u64,
            commands_applied: report.applied as u64,
            commands_rejected: report.ignored as u64,
            stale_revisions,
            merge_time_ns,
            apply_time_ns,
        }
    }

    #[must_use]
    pub const fn from_frame_run(report: &EcsSpatialFrameRunReport) -> Self {
        Self {
            buffers_emitted: 0,
            commands_recorded: report.metrics.commands_emitted,
            commands_applied: report.metrics.commands_applied,
            commands_rejected: 0,
            stale_revisions: 0,
            merge_time_ns: 0,
            apply_time_ns: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsArtifactReport {
    pub produced_artifacts: u64,
    pub ready_artifacts: u64,
    pub optional_artifacts: u64,
    pub retired_artifacts: u64,
    pub fallback_uses: u64,
    pub missing_producer_rejects: u64,
}

impl EcsArtifactReport {
    #[must_use]
    pub const fn from_counts(
        produced_artifacts: u64,
        ready_artifacts: u64,
        optional_artifacts: u64,
        retired_artifacts: u64,
        fallback_uses: u64,
        missing_producer_rejects: u64,
    ) -> Self {
        Self {
            produced_artifacts,
            ready_artifacts,
            optional_artifacts,
            retired_artifacts,
            fallback_uses,
            missing_producer_rejects,
        }
    }

    #[must_use]
    pub fn from_registry(
        registry: &EcsDerivedArtifactRegistry,
        fallback_uses: u64,
        missing_producer_rejects: u64,
    ) -> Self {
        let mut report = Self {
            produced_artifacts: registry.len() as u64,
            fallback_uses,
            missing_producer_rejects,
            ..Self::default()
        };
        for (_id, artifact) in registry.artifacts.iter() {
            if matches!(
                artifact.state,
                EcsArtifactState::Ready
                    | EcsArtifactState::HandoffQueued
                    | EcsArtifactState::ExternalPublishing
                    | EcsArtifactState::Published
            ) {
                report.ready_artifacts = report.ready_artifacts.saturating_add(1);
            }
            if artifact.requiredness.is_optional() {
                report.optional_artifacts = report.optional_artifacts.saturating_add(1);
            }
            if artifact.state == EcsArtifactState::Retiring {
                report.retired_artifacts = report.retired_artifacts.saturating_add(1);
            }
        }
        report
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsHandoffReport {
    pub renderer_queue_rows: u32,
    pub lux_queue_rows: u32,
    pub physics_cook_rows: u32,
    pub thunder_network_rows: u32,
    pub rvelte_ui_packet_rows: u32,
    pub consumer_lag: u32,
}

impl EcsHandoffReport {
    #[must_use]
    pub const fn from_counts(
        renderer_queue_rows: u32,
        lux_queue_rows: u32,
        physics_cook_rows: u32,
        thunder_network_rows: u32,
        rvelte_ui_packet_rows: u32,
        consumer_lag: u32,
    ) -> Self {
        Self {
            renderer_queue_rows,
            lux_queue_rows,
            physics_cook_rows,
            thunder_network_rows,
            rvelte_ui_packet_rows,
            consumer_lag,
        }
    }

    #[must_use]
    pub fn from_spatial_queues(
        renderer: &EcsRendererHandoffQueue,
        lux: &EcsLuxHandoffQueue,
        physics: &EcsPhysicsCookQueue,
        thunder_network_rows: u32,
        rvelte_ui_packet_rows: u32,
        consumer_lag: u32,
    ) -> Self {
        Self {
            renderer_queue_rows: renderer.items.len() as u32,
            lux_queue_rows: lux.items.len() as u32,
            physics_cook_rows: physics.items.len() as u32,
            thunder_network_rows,
            rvelte_ui_packet_rows,
            consumer_lag,
        }
    }

    #[must_use]
    pub fn from_world(world: &FunWorld, rvelte_ui_packet_rows: u32, consumer_lag: u32) -> Self {
        Self::from_spatial_queues(
            &world.renderer_handoff_queue,
            &world.lux_handoff_queue,
            &world.physics_cook_queue,
            world.cross_domain_handoff_queues.rows.len() as u32,
            rvelte_ui_packet_rows,
            consumer_lag,
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsCrossDomainReport {
    pub wait_token_count: u32,
    pub produced_token_count: u32,
    pub consumed_token_count: u32,
    pub cancellation_path_count: u32,
    pub timeout_or_fallback_count: u32,
    pub liveness_reject_count: u32,
}

impl EcsCrossDomainReport {
    #[must_use]
    pub const fn from_counts(
        wait_token_count: u32,
        produced_token_count: u32,
        consumed_token_count: u32,
        cancellation_path_count: u32,
        timeout_or_fallback_count: u32,
        liveness_reject_count: u32,
    ) -> Self {
        Self {
            wait_token_count,
            produced_token_count,
            consumed_token_count,
            cancellation_path_count,
            timeout_or_fallback_count,
            liveness_reject_count,
        }
    }

    #[must_use]
    pub fn from_frame_schedule(schedule: &FunFrameSchedule, liveness_reject_count: u32) -> Self {
        Self {
            wait_token_count: schedule.wait_plans.len() as u32,
            produced_token_count: schedule
                .graph
                .nodes
                .iter()
                .map(|node| node.work_descriptor.produced_tokens.len())
                .sum::<usize>() as u32,
            consumed_token_count: schedule
                .graph
                .nodes
                .iter()
                .map(|node| node.work_descriptor.awaited_tokens.len())
                .sum::<usize>() as u32,
            cancellation_path_count: schedule
                .graph
                .edges
                .iter()
                .filter(|edge| matches!(edge, WorkEdge::CancellationPropagation { .. }))
                .count() as u32,
            timeout_or_fallback_count: schedule
                .wait_plans
                .iter()
                .filter(|plan| plan.fallback.has_timeout_or_fallback())
                .count() as u32,
            liveness_reject_count,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFrameReport {
    pub frame: u64,
    pub storage: EcsStorageReport,
    pub table: EcsTableReport,
    pub query: EcsQueryReport,
    pub schedule: EcsScheduleReport,
    pub commands: EcsCommandReport,
    pub artifacts: EcsArtifactReport,
    pub handoffs: EcsHandoffReport,
    pub cross_domain: EcsCrossDomainReport,
    pub diagnostics_present_critical: bool,
    pub digest: FunFrameDigest,
}

impl EcsFrameReport {
    #[must_use]
    #[allow(
        clippy::too_many_arguments,
        reason = "the report aggregates the Pass 14 report rows without hiding fields behind builders"
    )]
    pub const fn from_parts(
        frame: u64,
        storage: EcsStorageReport,
        table: EcsTableReport,
        query: EcsQueryReport,
        schedule: EcsScheduleReport,
        commands: EcsCommandReport,
        artifacts: EcsArtifactReport,
        handoffs: EcsHandoffReport,
        cross_domain: EcsCrossDomainReport,
        diagnostics_present_critical: bool,
        digest: FunFrameDigest,
    ) -> Self {
        Self {
            frame,
            storage,
            table,
            query,
            schedule,
            commands,
            artifacts,
            handoffs,
            cross_domain,
            diagnostics_present_critical,
            digest,
        }
    }

    #[must_use]
    pub fn from_frame_schedule(
        schedule: &FunFrameSchedule,
        world: &FunWorld,
        commands: EcsCommandReport,
        artifacts: EcsArtifactReport,
        rvelte_ui_packet_rows: u32,
    ) -> Self {
        Self {
            frame: schedule.intent.frame.get(),
            storage: EcsStorageReport::from_world(world),
            table: EcsTableReport::from_world(world),
            query: EcsQueryReport::default(),
            schedule: EcsScheduleReport::from_frame_schedule(schedule, 0, 0),
            commands,
            artifacts,
            handoffs: EcsHandoffReport::from_world(world, rvelte_ui_packet_rows, 0),
            cross_domain: EcsCrossDomainReport::from_frame_schedule(schedule, 0),
            diagnostics_present_critical: false,
            digest: schedule.digest,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsDiagnosticsUiView {
    #[default]
    ScheduleGraph = 0,
    FrameFlow = 1,
    CommandBarrier = 2,
    ArtifactDag = 3,
    TableHeatmap = 4,
    SubsystemHandoffQueue = 5,
    WaitTokenLedger = 6,
}

impl EcsDiagnosticsUiView {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ScheduleGraph => "schedule_graph",
            Self::FrameFlow => "frame_flow",
            Self::CommandBarrier => "command_barrier",
            Self::ArtifactDag => "artifact_dag",
            Self::TableHeatmap => "table_heatmap",
            Self::SubsystemHandoffQueue => "subsystem_handoff_queue",
            Self::WaitTokenLedger => "wait_token_ledger",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsDiagnosticsUiPacket {
    pub view: EcsDiagnosticsUiView,
    pub generation: SnapshotGeneration,
    pub revision: FunRevision,
    pub row_count: u32,
    pub digest: u64,
    pub renderer_independent: bool,
    pub present_critical: bool,
}

impl EcsDiagnosticsUiPacket {
    #[must_use]
    pub const fn new(
        view: EcsDiagnosticsUiView,
        generation: SnapshotGeneration,
        revision: FunRevision,
        row_count: u32,
        digest: u64,
    ) -> Self {
        Self {
            view,
            generation,
            revision,
            row_count,
            digest,
            renderer_independent: true,
            present_critical: false,
        }
    }

    #[must_use]
    pub const fn with_developer_mode_present_critical(mut self, developer_mode: bool) -> Self {
        self.present_critical = developer_mode;
        self
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EcsDiagnosticsUiPacketSet {
    pub packets: Vec<EcsDiagnosticsUiPacket>,
    pub developer_mode_present_critical: bool,
}

impl EcsDiagnosticsUiPacketSet {
    #[must_use]
    pub fn from_frame_schedule(
        schedule: &FunFrameSchedule,
        generation: SnapshotGeneration,
        developer_mode_present_critical: bool,
    ) -> Self {
        let revision = schedule.context.observed_revision;
        let packets = ECS_DIAGNOSTICS_UI_VIEWS
            .iter()
            .copied()
            .map(|view| {
                let row_count = diagnostics_ui_row_count(view, schedule);
                let digest =
                    diagnostics_ui_packet_digest(schedule.digest, view, generation, row_count);
                EcsDiagnosticsUiPacket::new(view, generation, revision, row_count, digest)
                    .with_developer_mode_present_critical(developer_mode_present_critical)
            })
            .collect();
        Self {
            packets,
            developer_mode_present_critical,
        }
    }

    #[must_use]
    pub fn packet(&self, view: EcsDiagnosticsUiView) -> Option<&EcsDiagnosticsUiPacket> {
        self.packets.iter().find(|packet| packet.view == view)
    }

    #[must_use]
    pub fn present_critical_count(&self) -> usize {
        self.packets
            .iter()
            .filter(|packet| packet.present_critical)
            .count()
    }
}

const fn spatial_capacity_total() -> u64 {
    ECS_SPATIAL_MAX_PAGE_RECORDS as u64
        + ECS_SPATIAL_MAX_PAGE_RECORDS as u64
        + ECS_SPATIAL_MAX_DIRTY_REGIONS as u64
        + ECS_SPATIAL_MAX_STREAM_INTERESTS as u64
        + ECS_SPATIAL_MAX_STREAM_WAVES as u64
        + ECS_SPATIAL_MAX_STREAM_REQUESTS as u64
        + ECS_SPATIAL_MAX_SOURCE_QUEUE_ROWS as u64
        + ECS_SPATIAL_MAX_DECODED_PAGE_ROWS as u64
        + ECS_SPATIAL_MAX_DERIVED_ARTIFACTS as u64
        + ECS_SPATIAL_MAX_HANDOFF_ROWS as u64
        + ECS_SPATIAL_MAX_HANDOFF_ROWS as u64
        + ECS_SPATIAL_MAX_HANDOFF_ROWS as u64
        + ECS_SPATIAL_MAX_HANDOFF_ROWS as u64
}

fn diagnostics_ui_row_count(view: EcsDiagnosticsUiView, schedule: &FunFrameSchedule) -> u32 {
    match view {
        EcsDiagnosticsUiView::ScheduleGraph => schedule.graph.nodes.len() as u32,
        EcsDiagnosticsUiView::FrameFlow => schedule.stages.len() as u32,
        EcsDiagnosticsUiView::CommandBarrier => schedule.graph.barriers.len() as u32,
        EcsDiagnosticsUiView::ArtifactDag => schedule.resource_reads.len() as u32,
        EcsDiagnosticsUiView::TableHeatmap => schedule.context.active_systems,
        EcsDiagnosticsUiView::SubsystemHandoffQueue => schedule.imported_graphs.len() as u32,
        EcsDiagnosticsUiView::WaitTokenLedger => schedule.wait_plans.len() as u32,
    }
}

const fn diagnostics_ui_packet_digest(
    frame_digest: FunFrameDigest,
    view: EcsDiagnosticsUiView,
    generation: SnapshotGeneration,
    row_count: u32,
) -> u64 {
    let mut hash = frame_digest.value ^ 0xecd1_a960_51c5_0001;
    hash = (hash ^ view as u64).wrapping_mul(0x0000_0100_0000_01b3);
    hash = (hash ^ generation.get()).wrapping_mul(0x0000_0100_0000_01b3);
    (hash ^ row_count as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EcsSpatialScheduleBuildInput, EcsSpatialScheduleCompiler, FunFrameCompiler,
        FunFrameContext, FunFrameId, FunFrameIntent, FunFrameStage, FunFrameWaitTokenKind,
        WorkGraphId, WorkRequiredness,
    };

    #[test]
    fn ecs_reports_cover_requested_storage_table_and_handoff_fields() {
        let world = FunWorld::hybrid(crate::FunWorldId::ROOT);
        let storage = EcsStorageReport::from_world(&world);
        let table = EcsTableReport::from_world(&world);
        let handoffs = EcsHandoffReport::from_world(&world, 7, 3);

        assert_eq!(storage.entity_count, 0);
        assert_eq!(
            storage.resource_count,
            u32::from(crate::FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT)
        );
        assert_eq!(storage.table_row_count, 0);
        assert!(storage.table_capacity >= ECS_SPATIAL_MAX_DERIVED_ARTIFACTS as u64);
        assert_eq!(table.table_count, storage.resource_count);
        assert_eq!(table.row_count, storage.table_row_count);
        assert_eq!(handoffs.renderer_queue_rows, 0);
        assert_eq!(handoffs.lux_queue_rows, 0);
        assert_eq!(handoffs.physics_cook_rows, 0);
        assert_eq!(handoffs.thunder_network_rows, 0);
        assert_eq!(handoffs.rvelte_ui_packet_rows, 7);
        assert_eq!(handoffs.consumer_lag, 3);
    }

    #[test]
    fn schedule_report_tracks_graph_counts_and_chunk_nodes() {
        let output = EcsSpatialScheduleCompiler::compile(
            EcsSpatialScheduleBuildInput::default()
                .with_graph_id(WorkGraphId::new(14))
                .with_decode_chunk_count(2),
        )
        .expect("compile spatial schedule");
        let report = EcsScheduleReport::from_spatial_product_graph(&output.graph, 42, 0);

        assert_eq!(report.graph_build_time_ns, 42);
        assert_eq!(report.node_count, output.build_report.work_nodes);
        assert_eq!(report.edge_count, output.graph.edges.len() as u32);
        assert_eq!(
            report.wait_token_count,
            output.graph.wait_for_edges.len() as u32
        );
        assert_eq!(report.conflict_set_count, output.build_report.conflict_sets);
        assert_eq!(report.barrier_count, output.build_report.barrier_nodes);
        assert_eq!(report.chunk_node_count, output.build_report.chunk_nodes);
        assert_eq!(report.liveness_reject_count, 0);
    }

    #[test]
    fn frame_reports_track_scheduler_commands_artifacts_and_cross_domain_waits() {
        let mut context = FunFrameContext::default().with_pending_commands(5);
        context.active_systems = 9;
        let schedule = FunFrameCompiler::compile_with_intent(
            FunFrameIntent::default().with_frame(FunFrameId::new(14)),
            context,
        )
        .expect("compile frame schedule");
        let schedule_report = EcsScheduleReport::from_frame_schedule(&schedule, 100, 2);
        let command_report = EcsCommandReport::from_counts(4, 5, 3, 2, 1, 10, 20);
        let artifact_report = EcsArtifactReport::from_counts(8, 6, 1, 1, 2, 0);
        let cross_domain = EcsCrossDomainReport::from_frame_schedule(&schedule, 0);

        assert_eq!(schedule_report.graph_build_time_ns, 100);
        assert_eq!(
            schedule_report.node_count,
            schedule.graph.nodes.len() as u32
        );
        assert_eq!(
            schedule_report.barrier_count,
            schedule.graph.barriers.len() as u32
        );
        assert_eq!(schedule_report.liveness_reject_count, 2);
        assert_eq!(command_report.buffers_emitted, 4);
        assert_eq!(command_report.commands_recorded, 5);
        assert_eq!(command_report.commands_applied, 3);
        assert_eq!(command_report.commands_rejected, 2);
        assert_eq!(artifact_report.produced_artifacts, 8);
        assert_eq!(artifact_report.ready_artifacts, 6);
        assert_eq!(artifact_report.optional_artifacts, 1);
        assert_eq!(artifact_report.retired_artifacts, 1);
        assert_eq!(
            cross_domain.wait_token_count,
            schedule.wait_plans.len() as u32
        );
        assert_eq!(
            cross_domain.timeout_or_fallback_count,
            schedule.wait_plans.len() as u32
        );
        assert!(
            schedule
                .wait_plans
                .iter()
                .any(|plan| plan.token == FunFrameWaitTokenKind::RveltePaintReady
                    && plan.consumer == FunFrameStage::RendererConsume)
        );
    }

    #[test]
    fn artifact_report_counts_ready_optional_and_retiring_rows() {
        let mut registry = EcsDerivedArtifactRegistry::default();
        let page = crate::EcsSpatialPageKey::new(
            crate::EcsSpatialDomainKind::Terrain,
            crate::EcsSpatialGridId::new(1),
            0,
            4,
            0,
            0,
            crate::EcsPageChannel::Surface,
        );
        for (id, state, requiredness) in [
            (
                crate::EcsDerivedArtifactId::new(1),
                EcsArtifactState::Ready,
                WorkRequiredness::Required,
            ),
            (
                crate::EcsDerivedArtifactId::new(2),
                EcsArtifactState::Published,
                WorkRequiredness::Optional,
            ),
            (
                crate::EcsDerivedArtifactId::new(3),
                EcsArtifactState::Retiring,
                WorkRequiredness::Required,
            ),
        ] {
            registry
                .push(crate::EcsDerivedArtifactRecord {
                    artifact_id: id,
                    source_page: page,
                    kind: crate::EcsDerivedArtifactKind::TerrainSurfacePackets,
                    source_epoch: 1,
                    source_digest: crate::derived_artifact_source_digest(page, 1, 1),
                    artifact_epoch: 1,
                    state,
                    requiredness,
                    consumer: crate::EcsArtifactConsumer::Renderer,
                })
                .expect("artifact row");
        }

        let report = EcsArtifactReport::from_registry(&registry, 2, 1);
        assert_eq!(report.produced_artifacts, 3);
        assert_eq!(report.ready_artifacts, 2);
        assert_eq!(report.optional_artifacts, 1);
        assert_eq!(report.retired_artifacts, 1);
        assert_eq!(report.fallback_uses, 2);
        assert_eq!(report.missing_producer_rejects, 1);
    }

    #[test]
    fn rvelte_diagnostic_packets_are_renderer_independent_and_not_present_critical_by_default() {
        let schedule =
            FunFrameCompiler::compile(FunFrameContext::default()).expect("compile frame schedule");
        let packets = EcsDiagnosticsUiPacketSet::from_frame_schedule(
            &schedule,
            SnapshotGeneration::new(1),
            false,
        );

        assert_eq!(packets.packets.len(), ECS_DIAGNOSTICS_UI_VIEW_COUNT);
        assert_eq!(packets.present_critical_count(), 0);
        for view in ECS_DIAGNOSTICS_UI_VIEWS {
            let packet = packets.packet(view).expect("diagnostic packet");
            assert_eq!(packet.view.label(), view.label());
            assert!(packet.renderer_independent);
            assert!(!packet.present_critical);
            assert_ne!(packet.digest, 0);
        }
    }

    #[test]
    fn developer_mode_can_explicitly_make_diagnostics_present_critical() {
        let schedule =
            FunFrameCompiler::compile(FunFrameContext::default()).expect("compile frame schedule");
        let packets = EcsDiagnosticsUiPacketSet::from_frame_schedule(
            &schedule,
            SnapshotGeneration::new(2),
            true,
        );

        assert_eq!(packets.packets.len(), ECS_DIAGNOSTICS_UI_VIEW_COUNT);
        assert_eq!(
            packets.present_critical_count(),
            ECS_DIAGNOSTICS_UI_VIEW_COUNT
        );
        assert!(packets.developer_mode_present_critical);
    }
}
