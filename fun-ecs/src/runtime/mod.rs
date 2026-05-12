use fun_scheduler_types::{
    CommitPolicy, DeterministicDescriptor, EcsWorkKind, GraphExecutionMode, GraphInvariantError,
    ScheduleDeadline, ScheduleDomain, TaskPriority, WorkEdge, WorkGraph, WorkGraphId, WorkNode,
    WorkNodeId,
};

pub mod node_runner;
pub use node_runner::{
    EcsChunkExecutionContext, EcsCommandApplyContext, EcsNodeContext, EcsNodeError,
    EcsNodeExecutionMode, EcsNodeMetrics, EcsNodeOutcome, EcsNodeRunner, EcsSpatialFrameRunReport,
    EcsSpatialWorldDigest,
};

use crate::{
    EcsSpatialCompiledScheduleNode, EcsSpatialCompiledScheduleNodeKind, ScheduleRevision,
    compile_spatial_schedule_graph,
};

pub mod frame;
pub type FunEcsScheduleWorkGraph = WorkGraph<FunEcsRuntimeWork>;
pub use frame::{
    FUN_FRAME_STAGE_COUNT, FUN_FRAME_STAGES, FunFrameBudgetPressure, FunFrameCompileError,
    FunFrameCompiler, FunFrameContext, FunFrameDigest, FunFrameExecutionMode,
    FunFrameFallbackAvailability, FunFrameFallbackKind, FunFrameFallbackPlan, FunFrameGraph,
    FunFrameImportedGraph, FunFrameIntent, FunFrameReport, FunFrameResourceReadPlan,
    FunFrameResourceSource, FunFrameSchedule, FunFrameStage, FunFrameStageDeclaration,
    FunFrameStageNode, FunFrameSubsystemReadiness, FunFrameWaitPlan, FunFrameWaitTokenKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsRuntimeWork {
    pub schedule_node: EcsSpatialCompiledScheduleNode,
    pub work_kind: EcsWorkKind,
    pub observed_revision: ScheduleRevision,
}

impl FunEcsRuntimeWork {
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.schedule_node.label()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsSchedulerBridgeReport {
    pub nodes: u32,
    pub barriers: u32,
    pub dependency_edges: u32,
}

pub fn compile_spatial_schedule_work_graph(
    graph_id: WorkGraphId,
) -> Result<FunEcsScheduleWorkGraph, GraphInvariantError> {
    compile_spatial_schedule_work_graph_at_revision(graph_id, ScheduleRevision::INITIAL)
}

pub fn compile_spatial_schedule_work_graph_at_revision(
    graph_id: WorkGraphId,
    observed_revision: ScheduleRevision,
) -> Result<FunEcsScheduleWorkGraph, GraphInvariantError> {
    let mut graph = WorkGraph::new(
        graph_id,
        ScheduleDomain::FunEcs,
        GraphExecutionMode::SingleThreadDeterministic,
        CommitPolicy::DescriptorOrder,
    )
    .with_deadline(ScheduleDeadline::Frame);

    let mut previous: Option<WorkNodeId> = None;
    for schedule_node in compile_spatial_schedule_graph() {
        let id = graph.next_node_id();
        let contract = schedule_node.execution_contract();
        let work = FunEcsRuntimeWork {
            schedule_node: *schedule_node,
            work_kind: schedule_node.work_kind(),
            observed_revision,
        };
        let node = WorkNode::new(
            id,
            contract.domain,
            contract.lane,
            schedule_node.work_kind().phase(),
            priority_for_deadline(contract.deadline),
            contract.budget,
            contract.deadline,
            work,
        )
        .with_deterministic_descriptor(DeterministicDescriptor::new(
            schedule_node.label(),
            u64::from(schedule_node.ordinal),
        ));
        graph.add_node(node);
        if let Some(from) = previous {
            graph.add_edge(WorkEdge::Dependency { from, to: id });
        }
        if matches!(
            schedule_node.kind,
            EcsSpatialCompiledScheduleNodeKind::Barrier(_)
        ) {
            graph.add_edge(WorkEdge::Barrier { at: id });
        }
        previous = Some(id);
    }

    graph.validate(false)?;
    Ok(graph)
}

#[must_use]
pub fn scheduler_bridge_report(graph: &FunEcsScheduleWorkGraph) -> FunEcsSchedulerBridgeReport {
    FunEcsSchedulerBridgeReport {
        nodes: graph.nodes.len() as u32,
        barriers: graph.barriers.len() as u32,
        dependency_edges: graph
            .edges
            .iter()
            .filter(|edge| matches!(edge, WorkEdge::Dependency { .. }))
            .count() as u32,
    }
}

const fn priority_for_deadline(deadline: ScheduleDeadline) -> TaskPriority {
    match deadline {
        ScheduleDeadline::Frame | ScheduleDeadline::FixedStep | ScheduleDeadline::Present => {
            TaskPriority::Critical
        }
        ScheduleDeadline::Stream | ScheduleDeadline::Request => TaskPriority::High,
        ScheduleDeadline::IdleWindow | ScheduleDeadline::None => TaskPriority::Idle,
        _ => TaskPriority::Normal,
    }
}

#[cfg(test)]
mod tests {
    use fun_scheduler_types::{ScheduleLane, WorkGraphId};

    use crate::{
        ECS_SPATIAL_COMPILED_SCHEDULE_NODE_COUNT, EcsSpatialCommandBarrierKind,
        EcsSpatialCompiledScheduleNodeKind,
    };

    use super::*;

    #[test]
    fn spatial_schedule_declarations_compile_to_fun_scheduler_work_graph() {
        let graph =
            compile_spatial_schedule_work_graph(WorkGraphId::new(900)).expect("compile graph");
        let report = scheduler_bridge_report(&graph);

        assert_eq!(
            report.nodes as usize,
            ECS_SPATIAL_COMPILED_SCHEDULE_NODE_COUNT
        );
        assert_eq!(report.barriers, 4);
        assert_eq!(report.dependency_edges, report.nodes - 1);
        assert_eq!(graph.domain, ScheduleDomain::FunEcs);
        assert!(graph.nodes.iter().any(|node| {
            node.lane == ScheduleLane::EcsCommandBarrier
                && matches!(
                    node.work_descriptor.schedule_node.kind,
                    EcsSpatialCompiledScheduleNodeKind::Barrier(
                        EcsSpatialCommandBarrierKind::ApplyArtifactCommands
                    )
                )
        }));
    }

    #[test]
    fn scheduler_work_nodes_carry_observed_schedule_revision() {
        let revision = ScheduleRevision::new(77);
        let graph =
            compile_spatial_schedule_work_graph_at_revision(WorkGraphId::new(901), revision)
                .expect("compile graph at revision");

        assert!(
            graph
                .nodes
                .iter()
                .all(|node| node.work_descriptor.observed_revision == revision)
        );
    }
}
