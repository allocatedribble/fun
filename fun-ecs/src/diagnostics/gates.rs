#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsReplacementGateInput {
    pub deterministic_digest_a: u64,
    pub deterministic_digest_b: u64,
    pub command_order_digest_a: u64,
    pub command_order_digest_b: u64,
    pub direct_path_digest: u64,
    pub scheduler_path_digest: u64,
    pub graph_liveness_proven: bool,
    pub small_scene_regression_percent: i16,
    pub large_table_improvement_or_equal: bool,
    pub page_entity_count: u32,
    pub scheduler_compile_node_count: u32,
    pub scheduler_compile_node_limit: u32,
    pub command_buffer_rows: u32,
    pub command_buffer_row_limit: u32,
    pub unsafe_used: bool,
    pub undeclared_access_count: u32,
    pub hidden_blocking_count: u32,
    pub optional_critical_path_edges: u32,
    pub stale_generation_mutations: u32,
}

impl EcsReplacementGateInput {
    #[must_use]
    pub const fn passing_fixture() -> Self {
        Self {
            deterministic_digest_a: 1,
            deterministic_digest_b: 1,
            command_order_digest_a: 2,
            command_order_digest_b: 2,
            direct_path_digest: 3,
            scheduler_path_digest: 3,
            graph_liveness_proven: true,
            small_scene_regression_percent: 0,
            large_table_improvement_or_equal: true,
            page_entity_count: 0,
            scheduler_compile_node_count: 64,
            scheduler_compile_node_limit: 256,
            command_buffer_rows: 1_024,
            command_buffer_row_limit: 65_536,
            unsafe_used: false,
            undeclared_access_count: 0,
            hidden_blocking_count: 0,
            optional_critical_path_edges: 0,
            stale_generation_mutations: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsReplacementGateCategory {
    Correctness = 0,
    Performance = 1,
    Safety = 2,
}

impl EcsReplacementGateCategory {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Correctness => "correctness",
            Self::Performance => "performance",
            Self::Safety => "safety",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsReplacementGateRejectReason {
    DeterministicDigestUnstable = 0,
    GraphLivenessProofMissing = 1,
    CommandOrderingUnstable = 2,
    DirectSchedulerDigestMismatch = 3,
    SmallSceneRegression = 16,
    LargeTableNotEqualOrImproved = 17,
    PagePerEntityPath = 18,
    SchedulerGraphCompileExplosion = 19,
    UnboundedCommandBufferGrowth = 20,
    UnsafeProductionPath = 32,
    UndeclaredAccess = 33,
    HiddenBlocking = 34,
    OptionalWorkGatesCriticalPath = 35,
    StaleGenerationMutation = 36,
}

impl EcsReplacementGateRejectReason {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::DeterministicDigestUnstable => "deterministic_digest_unstable",
            Self::GraphLivenessProofMissing => "graph_liveness_proof_missing",
            Self::CommandOrderingUnstable => "command_ordering_unstable",
            Self::DirectSchedulerDigestMismatch => "direct_scheduler_digest_mismatch",
            Self::SmallSceneRegression => "small_scene_regression",
            Self::LargeTableNotEqualOrImproved => "large_table_not_equal_or_improved",
            Self::PagePerEntityPath => "page_per_entity_path",
            Self::SchedulerGraphCompileExplosion => "scheduler_graph_compile_explosion",
            Self::UnboundedCommandBufferGrowth => "unbounded_command_buffer_growth",
            Self::UnsafeProductionPath => "unsafe_production_path",
            Self::UndeclaredAccess => "undeclared_access",
            Self::HiddenBlocking => "hidden_blocking",
            Self::OptionalWorkGatesCriticalPath => "optional_work_gates_critical_path",
            Self::StaleGenerationMutation => "stale_generation_mutation",
        }
    }

    #[must_use]
    pub const fn category(self) -> EcsReplacementGateCategory {
        match self {
            Self::DeterministicDigestUnstable
            | Self::GraphLivenessProofMissing
            | Self::CommandOrderingUnstable
            | Self::DirectSchedulerDigestMismatch => EcsReplacementGateCategory::Correctness,
            Self::SmallSceneRegression
            | Self::LargeTableNotEqualOrImproved
            | Self::PagePerEntityPath
            | Self::SchedulerGraphCompileExplosion
            | Self::UnboundedCommandBufferGrowth => EcsReplacementGateCategory::Performance,
            Self::UnsafeProductionPath
            | Self::UndeclaredAccess
            | Self::HiddenBlocking
            | Self::OptionalWorkGatesCriticalPath
            | Self::StaleGenerationMutation => EcsReplacementGateCategory::Safety,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EcsReplacementGateReport {
    pub accepted: bool,
    pub correctness_rejects: u32,
    pub performance_rejects: u32,
    pub safety_rejects: u32,
    pub reasons: Vec<EcsReplacementGateRejectReason>,
}

impl EcsReplacementGateReport {
    #[must_use]
    pub fn evaluate(input: EcsReplacementGateInput) -> Self {
        let mut report = Self {
            accepted: true,
            ..Self::default()
        };

        if input.deterministic_digest_a != input.deterministic_digest_b {
            report.reject(EcsReplacementGateRejectReason::DeterministicDigestUnstable);
        }
        if !input.graph_liveness_proven {
            report.reject(EcsReplacementGateRejectReason::GraphLivenessProofMissing);
        }
        if input.command_order_digest_a != input.command_order_digest_b {
            report.reject(EcsReplacementGateRejectReason::CommandOrderingUnstable);
        }
        if input.direct_path_digest != input.scheduler_path_digest {
            report.reject(EcsReplacementGateRejectReason::DirectSchedulerDigestMismatch);
        }
        if input.small_scene_regression_percent > 0 {
            report.reject(EcsReplacementGateRejectReason::SmallSceneRegression);
        }
        if !input.large_table_improvement_or_equal {
            report.reject(EcsReplacementGateRejectReason::LargeTableNotEqualOrImproved);
        }
        if input.page_entity_count > 0 {
            report.reject(EcsReplacementGateRejectReason::PagePerEntityPath);
        }
        if input.scheduler_compile_node_count > input.scheduler_compile_node_limit {
            report.reject(EcsReplacementGateRejectReason::SchedulerGraphCompileExplosion);
        }
        if input.command_buffer_rows > input.command_buffer_row_limit {
            report.reject(EcsReplacementGateRejectReason::UnboundedCommandBufferGrowth);
        }
        if input.unsafe_used {
            report.reject(EcsReplacementGateRejectReason::UnsafeProductionPath);
        }
        if input.undeclared_access_count > 0 {
            report.reject(EcsReplacementGateRejectReason::UndeclaredAccess);
        }
        if input.hidden_blocking_count > 0 {
            report.reject(EcsReplacementGateRejectReason::HiddenBlocking);
        }
        if input.optional_critical_path_edges > 0 {
            report.reject(EcsReplacementGateRejectReason::OptionalWorkGatesCriticalPath);
        }
        if input.stale_generation_mutations > 0 {
            report.reject(EcsReplacementGateRejectReason::StaleGenerationMutation);
        }

        report.accepted = report.reasons.is_empty();
        report
    }

    #[must_use]
    pub fn can_replace(&self) -> bool {
        self.accepted
    }

    fn reject(&mut self, reason: EcsReplacementGateRejectReason) {
        match reason.category() {
            EcsReplacementGateCategory::Correctness => self.correctness_rejects += 1,
            EcsReplacementGateCategory::Performance => self.performance_rejects += 1,
            EcsReplacementGateCategory::Safety => self.safety_rejects += 1,
        }
        self.reasons.push(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passing_replacement_gate_accepts() {
        let report = EcsReplacementGateReport::evaluate(EcsReplacementGateInput::passing_fixture());

        assert!(report.can_replace());
        assert!(report.reasons.is_empty());
    }

    #[test]
    fn replacement_gate_rejects_each_required_failure_class() {
        let mut input = EcsReplacementGateInput::passing_fixture();
        input.deterministic_digest_b = 10;
        input.graph_liveness_proven = false;
        input.command_order_digest_b = 20;
        input.scheduler_path_digest = 30;
        input.small_scene_regression_percent = 1;
        input.large_table_improvement_or_equal = false;
        input.page_entity_count = 1;
        input.scheduler_compile_node_count = 257;
        input.command_buffer_rows = 65_537;
        input.unsafe_used = true;
        input.undeclared_access_count = 1;
        input.hidden_blocking_count = 1;
        input.optional_critical_path_edges = 1;
        input.stale_generation_mutations = 1;

        let report = EcsReplacementGateReport::evaluate(input);

        assert!(!report.can_replace());
        assert_eq!(report.correctness_rejects, 4);
        assert_eq!(report.performance_rejects, 5);
        assert_eq!(report.safety_rejects, 5);
        assert_eq!(report.reasons.len(), 14);
        assert!(
            report
                .reasons
                .contains(&EcsReplacementGateRejectReason::PagePerEntityPath)
        );
        assert!(
            report
                .reasons
                .contains(&EcsReplacementGateRejectReason::OptionalWorkGatesCriticalPath)
        );
    }
}
