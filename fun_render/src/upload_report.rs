use std::collections::BTreeMap;

use crate::{
    FunUploadBudgetDecision, FunUploadSubsystem, FunUploadWriteIntent, UploadWriteLabel,
    upload_label_descriptor_or_default,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunUploadBudgetDecisionCounts {
    pub admit: u32,
    pub defer: u32,
    pub fallback_raw_write: u32,
    pub reject_oversized: u32,
    pub reject_unaligned: u32,
}

impl FunUploadBudgetDecisionCounts {
    pub fn record(&mut self, decision: FunUploadBudgetDecision, count: u32) {
        match decision {
            FunUploadBudgetDecision::Admit => {
                self.admit = self.admit.saturating_add(count);
            }
            FunUploadBudgetDecision::Defer => {
                self.defer = self.defer.saturating_add(count);
            }
            FunUploadBudgetDecision::FallbackRawWrite => {
                self.fallback_raw_write = self.fallback_raw_write.saturating_add(count);
            }
            FunUploadBudgetDecision::RejectOversized => {
                self.reject_oversized = self.reject_oversized.saturating_add(count);
            }
            FunUploadBudgetDecision::RejectUnaligned => {
                self.reject_unaligned = self.reject_unaligned.saturating_add(count);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunUploadFrameReport {
    pub frame_index: u64,
    pub total_calls: u32,
    pub total_bytes: u64,
    pub budget_exceeded: bool,
    pub deferred_calls: u32,
    pub deferred_bytes: u64,
    pub raw_write_fallbacks: u32,
    pub top_labels: Vec<FunUploadLabelReport>,
}

impl FunUploadFrameReport {
    pub fn from_arena_stats(stats: &crate::FunUploadArenaStats, top_label_limit: usize) -> Self {
        let mut builder = FunUploadFrameReportBuilder::new(stats.frame_index);
        for (label, label_stats) in &stats.label_stats {
            let descriptor = upload_label_descriptor_or_default(*label);
            builder.record_aggregate(
                *label,
                descriptor.subsystem,
                label_stats.calls,
                label_stats.bytes,
                label_stats.raw_write_fallbacks,
            );
        }
        builder.into_report(top_label_limit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunUploadLabelReport {
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub calls: u32,
    pub bytes: u64,
    pub raw_write_fallbacks: u32,
    pub budget_decision_counts: FunUploadBudgetDecisionCounts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunUploadFrameReportBuilder {
    frame_index: u64,
    labels: BTreeMap<UploadWriteLabel, UploadLabelAccumulator>,
    total_calls: u32,
    total_bytes: u64,
    budget_exceeded: bool,
    deferred_calls: u32,
    deferred_bytes: u64,
    raw_write_fallbacks: u32,
}

impl FunUploadFrameReportBuilder {
    pub fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            labels: BTreeMap::new(),
            total_calls: 0,
            total_bytes: 0,
            budget_exceeded: false,
            deferred_calls: 0,
            deferred_bytes: 0,
            raw_write_fallbacks: 0,
        }
    }

    pub fn record_intent(
        &mut self,
        intent: FunUploadWriteIntent,
        decision: FunUploadBudgetDecision,
    ) {
        self.record(intent.label, intent.subsystem, intent.bytes, decision);
    }

    pub fn record(
        &mut self,
        label: UploadWriteLabel,
        subsystem: FunUploadSubsystem,
        bytes: u64,
        decision: FunUploadBudgetDecision,
    ) {
        self.total_calls = self.total_calls.saturating_add(1);
        let moved_bytes = moved_bytes_for_decision(bytes, decision);
        self.total_bytes = self.total_bytes.saturating_add(moved_bytes);

        if decision == FunUploadBudgetDecision::Defer {
            self.budget_exceeded = true;
            self.deferred_calls = self.deferred_calls.saturating_add(1);
            self.deferred_bytes = self.deferred_bytes.saturating_add(bytes);
        }

        if matches!(
            decision,
            FunUploadBudgetDecision::RejectOversized | FunUploadBudgetDecision::RejectUnaligned
        ) {
            self.budget_exceeded = true;
        }

        if decision == FunUploadBudgetDecision::FallbackRawWrite {
            self.raw_write_fallbacks = self.raw_write_fallbacks.saturating_add(1);
        }

        let accumulator = self.labels.entry(label).or_insert(UploadLabelAccumulator {
            subsystem,
            calls: 0,
            bytes: 0,
            raw_write_fallbacks: 0,
            decision_counts: FunUploadBudgetDecisionCounts::default(),
        });
        accumulator.subsystem = subsystem;
        accumulator.calls = accumulator.calls.saturating_add(1);
        accumulator.bytes = accumulator.bytes.saturating_add(moved_bytes);
        if decision == FunUploadBudgetDecision::FallbackRawWrite {
            accumulator.raw_write_fallbacks = accumulator.raw_write_fallbacks.saturating_add(1);
        }
        accumulator.decision_counts.record(decision, 1);
    }

    pub fn record_aggregate(
        &mut self,
        label: UploadWriteLabel,
        subsystem: FunUploadSubsystem,
        calls: u64,
        bytes: u64,
        raw_write_fallbacks: u64,
    ) {
        let calls = saturating_u32(calls);
        let raw_write_fallbacks = saturating_u32(raw_write_fallbacks);
        self.total_calls = self.total_calls.saturating_add(calls);
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.raw_write_fallbacks = self.raw_write_fallbacks.saturating_add(raw_write_fallbacks);

        let accumulator = self.labels.entry(label).or_insert(UploadLabelAccumulator {
            subsystem,
            calls: 0,
            bytes: 0,
            raw_write_fallbacks: 0,
            decision_counts: FunUploadBudgetDecisionCounts::default(),
        });
        accumulator.subsystem = subsystem;
        accumulator.calls = accumulator.calls.saturating_add(calls);
        accumulator.bytes = accumulator.bytes.saturating_add(bytes);
        accumulator.raw_write_fallbacks = accumulator
            .raw_write_fallbacks
            .saturating_add(raw_write_fallbacks);
        accumulator
            .decision_counts
            .record(FunUploadBudgetDecision::Admit, calls);
        accumulator.decision_counts.record(
            FunUploadBudgetDecision::FallbackRawWrite,
            raw_write_fallbacks,
        );
    }

    pub fn into_report(self, top_label_limit: usize) -> FunUploadFrameReport {
        let mut top_labels = self
            .labels
            .into_iter()
            .map(|(label, accumulator)| FunUploadLabelReport {
                label,
                subsystem: accumulator.subsystem,
                calls: accumulator.calls,
                bytes: accumulator.bytes,
                raw_write_fallbacks: accumulator.raw_write_fallbacks,
                budget_decision_counts: accumulator.decision_counts,
            })
            .collect::<Vec<_>>();

        top_labels.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| right.calls.cmp(&left.calls))
                .then_with(|| left.label.cmp(&right.label))
        });
        top_labels.truncate(top_label_limit);

        FunUploadFrameReport {
            frame_index: self.frame_index,
            total_calls: self.total_calls,
            total_bytes: self.total_bytes,
            budget_exceeded: self.budget_exceeded,
            deferred_calls: self.deferred_calls,
            deferred_bytes: self.deferred_bytes,
            raw_write_fallbacks: self.raw_write_fallbacks,
            top_labels,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UploadLabelAccumulator {
    subsystem: FunUploadSubsystem,
    calls: u32,
    bytes: u64,
    raw_write_fallbacks: u32,
    decision_counts: FunUploadBudgetDecisionCounts,
}

const fn moved_bytes_for_decision(bytes: u64, decision: FunUploadBudgetDecision) -> u64 {
    match decision {
        FunUploadBudgetDecision::Admit | FunUploadBudgetDecision::FallbackRawWrite => bytes,
        FunUploadBudgetDecision::Defer
        | FunUploadBudgetDecision::RejectOversized
        | FunUploadBudgetDecision::RejectUnaligned => 0,
    }
}

const fn saturating_u32(value: u64) -> u32 {
    if value > u32::MAX as u64 {
        u32::MAX
    } else {
        value as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        UPLOAD_CEF_CPU_FULL_FRAME, UPLOAD_MESHLET_INSTANCE_RANGE, UPLOAD_MESHLET_MATERIAL_RANGE,
        UPLOAD_SOLARI_PARAMS,
    };

    #[test]
    fn top_label_report_is_sorted_by_bytes_then_calls() {
        let mut builder = FunUploadFrameReportBuilder::new(7);
        builder.record(
            UPLOAD_SOLARI_PARAMS,
            FunUploadSubsystem::Solari,
            32,
            FunUploadBudgetDecision::Admit,
        );
        builder.record(
            UPLOAD_MESHLET_INSTANCE_RANGE,
            FunUploadSubsystem::MeshletInstance,
            64,
            FunUploadBudgetDecision::Admit,
        );
        builder.record(
            UPLOAD_MESHLET_MATERIAL_RANGE,
            FunUploadSubsystem::MeshletMaterial,
            64,
            FunUploadBudgetDecision::Admit,
        );
        builder.record(
            UPLOAD_MESHLET_MATERIAL_RANGE,
            FunUploadSubsystem::MeshletMaterial,
            0,
            FunUploadBudgetDecision::Admit,
        );
        let report = builder.into_report(20);

        assert_eq!(report.top_labels[0].label, UPLOAD_MESHLET_MATERIAL_RANGE);
        assert_eq!(report.top_labels[1].label, UPLOAD_MESHLET_INSTANCE_RANGE);
        assert_eq!(report.top_labels[2].label, UPLOAD_SOLARI_PARAMS);
    }

    #[test]
    fn report_counts_deferred_and_raw_fallback_decisions() {
        let mut builder = FunUploadFrameReportBuilder::new(11);
        builder.record(
            UPLOAD_CEF_CPU_FULL_FRAME,
            FunUploadSubsystem::CefCpuPaint,
            1024,
            FunUploadBudgetDecision::FallbackRawWrite,
        );
        builder.record(
            UPLOAD_CEF_CPU_FULL_FRAME,
            FunUploadSubsystem::CefCpuPaint,
            2048,
            FunUploadBudgetDecision::Defer,
        );
        let report = builder.into_report(20);

        assert_eq!(report.total_bytes, 1024);
        assert!(report.budget_exceeded);
        assert_eq!(report.deferred_calls, 1);
        assert_eq!(report.deferred_bytes, 2048);
        assert_eq!(report.raw_write_fallbacks, 1);
        assert_eq!(
            report.top_labels[0]
                .budget_decision_counts
                .fallback_raw_write,
            1
        );
        assert_eq!(report.top_labels[0].budget_decision_counts.defer, 1);
    }
}
