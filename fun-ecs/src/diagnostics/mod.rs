use crate::runtime::{FunEcsScheduleWorkGraph, scheduler_bridge_report};

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
