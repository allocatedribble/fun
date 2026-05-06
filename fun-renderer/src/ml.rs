use std::{fmt::Write as _, io, path::Path};

use fun_ai_core::{
    RendererGpuResourceKind, RendererModelHookDescriptor, RendererModelHookKind,
    shadow_page_priority_prior_hook,
};

use crate::virtual_shadow::{
    ShadowPagePriorityInputs, ShadowPagePriorityScore, ShadowVirtualPageId,
};

pub const RENDERER_ML_SCHEMA_VERSION: u16 = 1;
pub const RENDERER_ML_BENCHMARK_ARTIFACT_ENV: &str = "FUN_RENDERER_ML_BENCHMARK_ARTIFACT";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererMlFeature {
    #[default]
    ShadowPagePriorityPrior,
}

impl RendererMlFeature {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShadowPagePriorityPrior => "shadow_page_priority_prior",
        }
    }

    #[must_use]
    pub const fn hook_kind(self) -> RendererModelHookKind {
        match self {
            Self::ShadowPagePriorityPrior => RendererModelHookKind::ShadowPagePriorityPrior,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererMlFallbackReason {
    #[default]
    None,
    DisabledByPolicy,
    ModelRuntimeUnavailable,
    ModelLoadFailed,
    QueueUnavailable,
    InferenceLate,
    InvalidModelOutput,
    CapabilityMismatch,
    MissingGpuResourceHandle,
}

impl RendererMlFallbackReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DisabledByPolicy => "disabled_by_policy",
            Self::ModelRuntimeUnavailable => "model_runtime_unavailable",
            Self::ModelLoadFailed => "model_load_failed",
            Self::QueueUnavailable => "queue_unavailable",
            Self::InferenceLate => "inference_late",
            Self::InvalidModelOutput => "invalid_model_output",
            Self::CapabilityMismatch => "capability_mismatch",
            Self::MissingGpuResourceHandle => "missing_gpu_resource_handle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererMlPolicy {
    pub feature: RendererMlFeature,
    pub enabled: bool,
    pub max_inference_latency_us: u32,
    pub require_gpu_resource_handles: bool,
}

impl RendererMlPolicy {
    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            feature: RendererMlFeature::ShadowPagePriorityPrior,
            enabled: false,
            max_inference_latency_us: 0,
            require_gpu_resource_handles: true,
        }
    }

    #[must_use]
    pub const fn shadow_page_priority_prior() -> Self {
        Self {
            feature: RendererMlFeature::ShadowPagePriorityPrior,
            enabled: true,
            max_inference_latency_us: 2_000,
            require_gpu_resource_handles: true,
        }
    }
}

impl Default for RendererMlPolicy {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererMlGpuResourceHandle {
    pub kind: RendererGpuResourceKind,
    pub stable_handle: u64,
    pub generation: u32,
    pub resident: bool,
}

impl RendererMlGpuResourceHandle {
    #[must_use]
    pub const fn new(
        kind: RendererGpuResourceKind,
        stable_handle: u64,
        generation: u32,
        resident: bool,
    ) -> Self {
        Self {
            kind,
            stable_handle,
            generation,
            resident,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.stable_handle != 0 && self.generation != 0 && self.resident
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererMlRequestMetadata {
    pub frame_index: u64,
    pub view_id: u32,
    pub quality_tier: u8,
    pub budget_mode: u8,
    pub backend_truth_state: &'static str,
}

impl RendererMlRequestMetadata {
    #[must_use]
    pub const fn test_frame(frame_index: u64) -> Self {
        Self {
            frame_index,
            view_id: 1,
            quality_tier: 2,
            budget_mode: 1,
            backend_truth_state: "test_backend_truth",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowPagePriorityCandidate {
    pub virtual_page: ShadowVirtualPageId,
    pub inputs: ShadowPagePriorityInputs,
}

impl ShadowPagePriorityCandidate {
    #[must_use]
    pub const fn new(virtual_page: ShadowVirtualPageId, inputs: ShadowPagePriorityInputs) -> Self {
        Self {
            virtual_page,
            inputs,
        }
    }

    #[must_use]
    pub fn heuristic_score(self) -> ShadowPagePriorityScore {
        self.inputs.score()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShadowPagePriorityPrior {
    pub virtual_page: ShadowVirtualPageId,
    pub priority: u8,
    pub confidence_milli: u16,
}

impl ShadowPagePriorityPrior {
    #[must_use]
    pub const fn new(
        virtual_page: ShadowVirtualPageId,
        priority: u8,
        confidence_milli: u16,
    ) -> Self {
        Self {
            virtual_page,
            priority,
            confidence_milli,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.confidence_milli <= 1_000
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowPagePriorityPriorBatch {
    pub hook_kind: RendererModelHookKind,
    pub priors: Vec<ShadowPagePriorityPrior>,
}

impl ShadowPagePriorityPriorBatch {
    #[must_use]
    pub fn new(priors: Vec<ShadowPagePriorityPrior>) -> Self {
        Self {
            hook_kind: RendererModelHookKind::ShadowPagePriorityPrior,
            priors,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererPredictionStatus {
    Complete,
    RuntimeUnavailable,
    ModelLoadFailed,
    QueueUnavailable,
    Late,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererPredictionResult<T> {
    pub status: RendererPredictionStatus,
    pub latency_us: u32,
    pub output: Option<T>,
}

impl<T> RendererPredictionResult<T> {
    #[must_use]
    pub const fn complete(latency_us: u32, output: T) -> Self {
        Self {
            status: RendererPredictionStatus::Complete,
            latency_us,
            output: Some(output),
        }
    }

    #[must_use]
    pub const fn fallback(status: RendererPredictionStatus, latency_us: u32) -> Self {
        Self {
            status,
            latency_us,
            output: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowPagePriorityPriorRequest<'a> {
    pub contract: &'static RendererModelHookDescriptor,
    pub metadata: RendererMlRequestMetadata,
    pub candidates: &'a [ShadowPagePriorityCandidate],
    pub gpu_resources: &'a [RendererMlGpuResourceHandle],
}

pub trait RendererPredictionClient {
    fn predict_shadow_page_priority_prior(
        &self,
        request: &ShadowPagePriorityPriorRequest<'_>,
    ) -> RendererPredictionResult<ShadowPagePriorityPriorBatch>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowPagePriorityDecision {
    pub virtual_page: ShadowVirtualPageId,
    pub priority: ShadowPagePriorityScore,
    pub model_priority: Option<u8>,
    pub model_confidence_milli: Option<u16>,
    pub model_used: bool,
    pub fallback_reason: RendererMlFallbackReason,
}

impl ShadowPagePriorityDecision {
    #[must_use]
    pub fn fallback(
        candidate: ShadowPagePriorityCandidate,
        reason: RendererMlFallbackReason,
    ) -> Self {
        Self {
            virtual_page: candidate.virtual_page,
            priority: candidate.heuristic_score(),
            model_priority: None,
            model_confidence_milli: None,
            model_used: false,
            fallback_reason: reason,
        }
    }

    #[must_use]
    pub fn model_assisted(
        candidate: ShadowPagePriorityCandidate,
        prior: ShadowPagePriorityPrior,
    ) -> Self {
        let heuristic = candidate.heuristic_score();
        let model_score = u16::from(prior.priority) * 257;
        let confidence = u32::from(prior.confidence_milli.min(1_000));
        let blended = ((u32::from(heuristic.value) * (1_000 - confidence))
            + (u32::from(model_score) * confidence))
            / 1_000;
        Self {
            virtual_page: candidate.virtual_page,
            priority: ShadowPagePriorityScore {
                value: blended.min(u32::from(u16::MAX)) as u16,
                inputs: candidate.inputs,
            },
            model_priority: Some(prior.priority),
            model_confidence_milli: Some(prior.confidence_milli),
            model_used: true,
            fallback_reason: RendererMlFallbackReason::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererMlDiagnostics {
    pub feature: RendererMlFeature,
    pub requested: bool,
    pub model_used_count: u32,
    pub fallback_count: u32,
    pub fallback_reason: RendererMlFallbackReason,
    pub inference_latency_us: u32,
    pub fun_ai_contract_id: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowPagePrioritySelection {
    pub decisions: Vec<ShadowPagePriorityDecision>,
    pub diagnostics: RendererMlDiagnostics,
}

pub fn select_shadow_page_priority_priors(
    policy: RendererMlPolicy,
    client: Option<&dyn RendererPredictionClient>,
    candidates: &[ShadowPagePriorityCandidate],
    gpu_resources: &[RendererMlGpuResourceHandle],
    metadata: RendererMlRequestMetadata,
) -> ShadowPagePrioritySelection {
    let contract = shadow_page_priority_prior_hook();
    if !policy.enabled {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::DisabledByPolicy,
            0,
            contract.stable_id,
        );
    }
    if policy.feature.hook_kind() != contract.kind {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::CapabilityMismatch,
            0,
            contract.stable_id,
        );
    }
    if policy.require_gpu_resource_handles && !gpu_resources_satisfy_contract(gpu_resources) {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::MissingGpuResourceHandle,
            0,
            contract.stable_id,
        );
    }
    let Some(client) = client else {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::ModelRuntimeUnavailable,
            0,
            contract.stable_id,
        );
    };
    let request = ShadowPagePriorityPriorRequest {
        contract,
        metadata,
        candidates,
        gpu_resources,
    };
    let result = client.predict_shadow_page_priority_prior(&request);
    let fallback_reason = fallback_reason_for_status(result.status);
    if fallback_reason != RendererMlFallbackReason::None {
        return fallback_selection(
            policy,
            candidates,
            fallback_reason,
            result.latency_us,
            contract.stable_id,
        );
    }
    if result.latency_us > policy.max_inference_latency_us {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::InferenceLate,
            result.latency_us,
            contract.stable_id,
        );
    }
    let Some(output) = result.output else {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::InvalidModelOutput,
            result.latency_us,
            contract.stable_id,
        );
    };
    if output.hook_kind != RendererModelHookKind::ShadowPagePriorityPrior {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::CapabilityMismatch,
            result.latency_us,
            contract.stable_id,
        );
    }
    let Some(decisions) = model_assisted_decisions(candidates, &output.priors) else {
        return fallback_selection(
            policy,
            candidates,
            RendererMlFallbackReason::InvalidModelOutput,
            result.latency_us,
            contract.stable_id,
        );
    };
    ShadowPagePrioritySelection {
        diagnostics: RendererMlDiagnostics {
            feature: policy.feature,
            requested: true,
            model_used_count: decisions.len().try_into().unwrap_or(u32::MAX),
            fallback_count: 0,
            fallback_reason: RendererMlFallbackReason::None,
            inference_latency_us: result.latency_us,
            fun_ai_contract_id: contract.stable_id,
        },
        decisions,
    }
}

fn fallback_selection(
    policy: RendererMlPolicy,
    candidates: &[ShadowPagePriorityCandidate],
    reason: RendererMlFallbackReason,
    inference_latency_us: u32,
    fun_ai_contract_id: &'static str,
) -> ShadowPagePrioritySelection {
    let decisions = candidates
        .iter()
        .copied()
        .map(|candidate| ShadowPagePriorityDecision::fallback(candidate, reason))
        .collect::<Vec<_>>();
    ShadowPagePrioritySelection {
        diagnostics: RendererMlDiagnostics {
            feature: policy.feature,
            requested: policy.enabled,
            model_used_count: 0,
            fallback_count: decisions.len().try_into().unwrap_or(u32::MAX),
            fallback_reason: reason,
            inference_latency_us,
            fun_ai_contract_id,
        },
        decisions,
    }
}

fn fallback_reason_for_status(status: RendererPredictionStatus) -> RendererMlFallbackReason {
    match status {
        RendererPredictionStatus::Complete => RendererMlFallbackReason::None,
        RendererPredictionStatus::RuntimeUnavailable => {
            RendererMlFallbackReason::ModelRuntimeUnavailable
        }
        RendererPredictionStatus::ModelLoadFailed => RendererMlFallbackReason::ModelLoadFailed,
        RendererPredictionStatus::QueueUnavailable => RendererMlFallbackReason::QueueUnavailable,
        RendererPredictionStatus::Late => RendererMlFallbackReason::InferenceLate,
    }
}

fn gpu_resources_satisfy_contract(resources: &[RendererMlGpuResourceHandle]) -> bool {
    has_valid_resource(resources, RendererGpuResourceKind::Depth)
        && has_valid_resource(resources, RendererGpuResourceKind::ShadowPageTable)
}

fn has_valid_resource(
    resources: &[RendererMlGpuResourceHandle],
    kind: RendererGpuResourceKind,
) -> bool {
    resources
        .iter()
        .any(|resource| resource.kind == kind && resource.is_valid())
}

fn model_assisted_decisions(
    candidates: &[ShadowPagePriorityCandidate],
    priors: &[ShadowPagePriorityPrior],
) -> Option<Vec<ShadowPagePriorityDecision>> {
    let mut decisions = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let prior = priors
            .iter()
            .copied()
            .find(|prior| prior.virtual_page == candidate.virtual_page)?;
        if !prior.is_valid() {
            return None;
        }
        decisions.push(ShadowPagePriorityDecision::model_assisted(
            *candidate, prior,
        ));
    }
    Some(decisions)
}

pub fn write_renderer_ml_benchmark_artifact(
    path: impl AsRef<Path>,
    heuristic: &ShadowPagePrioritySelection,
    model_assisted: &ShadowPagePrioritySelection,
    model_disabled: &ShadowPagePrioritySelection,
) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let artifact = renderer_ml_benchmark_artifact(heuristic, model_assisted, model_disabled);
    std::fs::write(path, artifact)
}

#[must_use]
pub fn renderer_ml_benchmark_artifact(
    heuristic: &ShadowPagePrioritySelection,
    model_assisted: &ShadowPagePrioritySelection,
    model_disabled: &ShadowPagePrioritySelection,
) -> String {
    let mut artifact = String::with_capacity(768);
    let _ = writeln!(
        artifact,
        "schema_version={} feature={} contract={}",
        RENDERER_ML_SCHEMA_VERSION,
        RendererMlFeature::ShadowPagePriorityPrior.as_str(),
        shadow_page_priority_prior_hook().stable_id
    );
    write_selection_line(&mut artifact, "heuristic_only", heuristic);
    write_selection_line(&mut artifact, "model_assisted", model_assisted);
    write_selection_line(&mut artifact, "model_disabled_fallback", model_disabled);
    artifact
}

fn write_selection_line(
    artifact: &mut String,
    mode: &'static str,
    selection: &ShadowPagePrioritySelection,
) {
    let _ = writeln!(
        artifact,
        "mode={} requested={} model_used_count={} fallback_count={} fallback_reason={} latency_us={} priority_sum={}",
        mode,
        selection.diagnostics.requested,
        selection.diagnostics.model_used_count,
        selection.diagnostics.fallback_count,
        selection.diagnostics.fallback_reason.as_str(),
        selection.diagnostics.inference_latency_us,
        selection.decisions.iter().fold(0_u64, |sum, decision| sum
            + u64::from(decision.priority.value))
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::virtual_shadow::{ShadowPageTableId, ShadowVirtualPageId};

    struct MockShadowPriorClient {
        status: RendererPredictionStatus,
        latency_us: u32,
        priors: Vec<ShadowPagePriorityPrior>,
    }

    impl RendererPredictionClient for MockShadowPriorClient {
        fn predict_shadow_page_priority_prior(
            &self,
            request: &ShadowPagePriorityPriorRequest<'_>,
        ) -> RendererPredictionResult<ShadowPagePriorityPriorBatch> {
            assert_eq!(
                request.contract.kind,
                RendererModelHookKind::ShadowPagePriorityPrior
            );
            if self.status != RendererPredictionStatus::Complete {
                return RendererPredictionResult::fallback(self.status, self.latency_us);
            }
            RendererPredictionResult::complete(
                self.latency_us,
                ShadowPagePriorityPriorBatch::new(self.priors.clone()),
            )
        }
    }

    #[test]
    fn shadow_page_priority_prior_uses_model_when_contract_is_valid() {
        let candidates = test_candidates();
        let gpu_resources = test_gpu_resources();
        let priors = vec![
            ShadowPagePriorityPrior::new(candidates[0].virtual_page, 10, 900),
            ShadowPagePriorityPrior::new(candidates[1].virtual_page, 240, 900),
        ];
        let client = MockShadowPriorClient {
            status: RendererPredictionStatus::Complete,
            latency_us: 1_000,
            priors,
        };

        let selection = select_shadow_page_priority_priors(
            RendererMlPolicy::shadow_page_priority_prior(),
            Some(&client),
            &candidates,
            &gpu_resources,
            RendererMlRequestMetadata::test_frame(7),
        );

        assert_eq!(
            selection.diagnostics.fallback_reason,
            RendererMlFallbackReason::None
        );
        assert_eq!(selection.diagnostics.model_used_count, 2);
        assert!(
            selection
                .decisions
                .iter()
                .all(|decision| decision.model_used)
        );
        assert!(selection.decisions[1].priority.value > candidates[1].heuristic_score().value);
    }

    #[test]
    fn shadow_page_priority_prior_falls_back_without_runtime() {
        let candidates = test_candidates();
        let selection = select_shadow_page_priority_priors(
            RendererMlPolicy::shadow_page_priority_prior(),
            None,
            &candidates,
            &test_gpu_resources(),
            RendererMlRequestMetadata::test_frame(7),
        );

        assert_eq!(
            selection.diagnostics.fallback_reason,
            RendererMlFallbackReason::ModelRuntimeUnavailable
        );
        assert_eq!(selection.diagnostics.fallback_count, 2);
        assert!(
            !selection
                .decisions
                .iter()
                .any(|decision| decision.model_used)
        );
    }

    #[test]
    fn shadow_page_priority_prior_falls_back_when_inference_is_late() {
        let candidates = test_candidates();
        let client = MockShadowPriorClient {
            status: RendererPredictionStatus::Complete,
            latency_us: 3_000,
            priors: Vec::new(),
        };

        let selection = select_shadow_page_priority_priors(
            RendererMlPolicy::shadow_page_priority_prior(),
            Some(&client),
            &candidates,
            &test_gpu_resources(),
            RendererMlRequestMetadata::test_frame(7),
        );

        assert_eq!(
            selection.diagnostics.fallback_reason,
            RendererMlFallbackReason::InferenceLate
        );
        assert_eq!(
            selection.decisions[0].priority.value,
            candidates[0].heuristic_score().value
        );
    }

    #[test]
    fn shadow_page_priority_prior_falls_back_when_model_load_fails() {
        let candidates = test_candidates();
        let client = MockShadowPriorClient {
            status: RendererPredictionStatus::ModelLoadFailed,
            latency_us: 200,
            priors: Vec::new(),
        };

        let selection = select_shadow_page_priority_priors(
            RendererMlPolicy::shadow_page_priority_prior(),
            Some(&client),
            &candidates,
            &test_gpu_resources(),
            RendererMlRequestMetadata::test_frame(7),
        );

        assert_eq!(
            selection.diagnostics.fallback_reason,
            RendererMlFallbackReason::ModelLoadFailed
        );
        assert_eq!(selection.diagnostics.fallback_count, 2);
    }

    #[test]
    fn shadow_page_priority_prior_falls_back_for_missing_gpu_handles() {
        let candidates = test_candidates();
        let client = MockShadowPriorClient {
            status: RendererPredictionStatus::Complete,
            latency_us: 100,
            priors: Vec::new(),
        };

        let selection = select_shadow_page_priority_priors(
            RendererMlPolicy::shadow_page_priority_prior(),
            Some(&client),
            &candidates,
            &[],
            RendererMlRequestMetadata::test_frame(7),
        );

        assert_eq!(
            selection.diagnostics.fallback_reason,
            RendererMlFallbackReason::MissingGpuResourceHandle
        );
        assert_eq!(selection.diagnostics.fallback_count, 2);
    }

    #[test]
    fn benchmark_artifact_compares_required_lanes() {
        let candidates = test_candidates();
        let gpu_resources = test_gpu_resources();
        let priors = vec![
            ShadowPagePriorityPrior::new(candidates[0].virtual_page, 20, 900),
            ShadowPagePriorityPrior::new(candidates[1].virtual_page, 220, 900),
        ];
        let client = MockShadowPriorClient {
            status: RendererPredictionStatus::Complete,
            latency_us: 1_000,
            priors,
        };
        let heuristic = select_shadow_page_priority_priors(
            RendererMlPolicy::disabled(),
            Some(&client),
            &candidates,
            &gpu_resources,
            RendererMlRequestMetadata::test_frame(7),
        );
        let model_assisted = select_shadow_page_priority_priors(
            RendererMlPolicy::shadow_page_priority_prior(),
            Some(&client),
            &candidates,
            &gpu_resources,
            RendererMlRequestMetadata::test_frame(7),
        );
        let model_disabled = select_shadow_page_priority_priors(
            RendererMlPolicy::disabled(),
            None,
            &candidates,
            &gpu_resources,
            RendererMlRequestMetadata::test_frame(7),
        );
        let artifact = renderer_ml_benchmark_artifact(&heuristic, &model_assisted, &model_disabled);

        assert!(artifact.contains("mode=heuristic_only"));
        assert!(artifact.contains("mode=model_assisted"));
        assert!(artifact.contains("mode=model_disabled_fallback"));
        assert!(artifact.contains("model_used_count=2"));

        if let Ok(path) = std::env::var(RENDERER_ML_BENCHMARK_ARTIFACT_ENV) {
            write_renderer_ml_benchmark_artifact(
                path,
                &heuristic,
                &model_assisted,
                &model_disabled,
            )
            .expect("benchmark artifact writes");
        }
    }

    fn test_candidates() -> [ShadowPagePriorityCandidate; 2] {
        [
            ShadowPagePriorityCandidate::new(
                ShadowVirtualPageId::new(ShadowPageTableId::new(1), 1),
                ShadowPagePriorityInputs {
                    visible_receiver_demand: 20,
                    screen_coverage: 10,
                    contrast: 10,
                    temporal_instability: 5,
                    gameplay_salience: 0,
                    editor_focus: 0,
                    light_importance: 20,
                },
            ),
            ShadowPagePriorityCandidate::new(
                ShadowVirtualPageId::new(ShadowPageTableId::new(1), 2),
                ShadowPagePriorityInputs {
                    visible_receiver_demand: 60,
                    screen_coverage: 45,
                    contrast: 40,
                    temporal_instability: 10,
                    gameplay_salience: 30,
                    editor_focus: 0,
                    light_importance: 60,
                },
            ),
        ]
    }

    fn test_gpu_resources() -> [RendererMlGpuResourceHandle; 2] {
        [
            RendererMlGpuResourceHandle::new(RendererGpuResourceKind::Depth, 100, 1, true),
            RendererMlGpuResourceHandle::new(
                RendererGpuResourceKind::ShadowPageTable,
                200,
                1,
                true,
            ),
        ]
    }
}
