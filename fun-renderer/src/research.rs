use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use serde::Serialize;

pub const RENDERER_RESEARCH_SPIKES_SCHEMA: &str = "fun.renderer.research_spikes.v1";
pub const RENDERER_RESEARCH_SPIKES_SCHEMA_VERSION: u16 = 1;
pub const RENDERER_RESEARCH_SPIKE_COUNT: usize = 5;
pub const RENDERER_RESEARCH_SPIKE_ARTIFACT_ENV: &str = "FUN_RENDERER_RESEARCH_SPIKE_ARTIFACT";

pub const EXPERIMENTAL_WORK_GRAPHS_FEATURE: &str = "experimental_work_graphs";
pub const MESH_SHADER_PATH_FEATURE: &str = "mesh_shader_path";
pub const RADIANCE_NEURAL_CACHE_FEATURE: &str = "fun-lux/radiance_neural_cache";
pub const LEARNED_PAGE_PRIORITY_PREDICTOR_FEATURE: &str = "learned_page_priority_predictor";
pub const NEURAL_TEXTURE_COMPRESSION_FEATURE: &str = "neural_texture_compression";

const WORK_GRAPH_EXPLORE_ITEMS: &[&str] = &[
    "cluster_refinement",
    "shadow_page_generation",
    "light_candidate_work",
    "gi_cache_updates",
];
const MESH_SHADER_EXPLORE_ITEMS: &[&str] = &[
    "virtual_geometry_fast_path",
    "optional_cluster_amplification",
];
const RADIANCE_NEURAL_CACHE_COMPARE_ITEMS: &[&str] = &[
    "stability",
    "invalidation_behavior",
    "latency",
    "memory_pressure",
    "visual_quality_proxy",
];
const LEARNED_PAGE_PRIORITY_PROMOTION_ITEMS: &[&str] = &[
    "page_faults_fall",
    "p95_p99_stable_or_better",
    "false_negatives_controlled",
    "deterministic_fallback",
];
const NEURAL_TEXTURE_COMPRESSION_COMPARE_ITEMS: &[&str] = &[
    "asset_size",
    "decode_cost",
    "quality",
    "streaming_behavior",
    "cache_pressure",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchSpikeKind {
    WorkGraphs,
    MeshShaderPath,
    RadianceNeuralCache,
    LearnedPagePriorityPredictor,
    NeuralTextureCompression,
}

impl ResearchSpikeKind {
    pub const ALL: [Self; RENDERER_RESEARCH_SPIKE_COUNT] = [
        Self::WorkGraphs,
        Self::MeshShaderPath,
        Self::RadianceNeuralCache,
        Self::LearnedPagePriorityPredictor,
        Self::NeuralTextureCompression,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkGraphs => "work_graphs",
            Self::MeshShaderPath => "mesh_shader_path",
            Self::RadianceNeuralCache => "radiance_neural_cache",
            Self::LearnedPagePriorityPredictor => "learned_page_priority_predictor",
            Self::NeuralTextureCompression => "neural_texture_compression",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchSpikeOwner {
    Renderer,
    Lux,
    FunAi,
    OfflineAssetPipeline,
}

impl ResearchSpikeOwner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Renderer => crate::FUN_RENDERER_CRATE_NAME,
            Self::Lux => fun_lux::FUN_LUX_CRATE_NAME,
            Self::FunAi => crate::FUN_RENDERER_AI_OWNER_PACKAGE_NAME,
            Self::OfflineAssetPipeline => "offline_asset_pipeline",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResearchSpikeDescriptor {
    pub stable_id: &'static str,
    pub kind: ResearchSpikeKind,
    pub owner: ResearchSpikeOwner,
    pub feature_flag: &'static str,
    pub compile_time_flag_required: bool,
    pub runtime_opt_in_required: bool,
    pub default_boot_requirement: bool,
    pub default_renderer_dependency_allowed: bool,
    pub capability_check_required: bool,
    pub benchmark_comparison_required: bool,
    pub deterministic_fallback_required: bool,
    pub compute_fallback_required: bool,
    pub promotion_requires_evidence: bool,
    pub evaluation_terms: &'static [&'static str],
}

pub const RENDERER_RESEARCH_SPIKES: [ResearchSpikeDescriptor; RENDERER_RESEARCH_SPIKE_COUNT] = [
    ResearchSpikeDescriptor {
        stable_id: "renderer.research.work_graphs",
        kind: ResearchSpikeKind::WorkGraphs,
        owner: ResearchSpikeOwner::Renderer,
        feature_flag: EXPERIMENTAL_WORK_GRAPHS_FEATURE,
        compile_time_flag_required: true,
        runtime_opt_in_required: true,
        default_boot_requirement: false,
        default_renderer_dependency_allowed: false,
        capability_check_required: true,
        benchmark_comparison_required: true,
        deterministic_fallback_required: true,
        compute_fallback_required: true,
        promotion_requires_evidence: true,
        evaluation_terms: WORK_GRAPH_EXPLORE_ITEMS,
    },
    ResearchSpikeDescriptor {
        stable_id: "renderer.research.mesh_shader_path",
        kind: ResearchSpikeKind::MeshShaderPath,
        owner: ResearchSpikeOwner::Renderer,
        feature_flag: MESH_SHADER_PATH_FEATURE,
        compile_time_flag_required: true,
        runtime_opt_in_required: true,
        default_boot_requirement: false,
        default_renderer_dependency_allowed: false,
        capability_check_required: true,
        benchmark_comparison_required: true,
        deterministic_fallback_required: true,
        compute_fallback_required: true,
        promotion_requires_evidence: true,
        evaluation_terms: MESH_SHADER_EXPLORE_ITEMS,
    },
    ResearchSpikeDescriptor {
        stable_id: "renderer.research.radiance_neural_cache",
        kind: ResearchSpikeKind::RadianceNeuralCache,
        owner: ResearchSpikeOwner::Lux,
        feature_flag: RADIANCE_NEURAL_CACHE_FEATURE,
        compile_time_flag_required: true,
        runtime_opt_in_required: true,
        default_boot_requirement: false,
        default_renderer_dependency_allowed: false,
        capability_check_required: true,
        benchmark_comparison_required: true,
        deterministic_fallback_required: true,
        compute_fallback_required: false,
        promotion_requires_evidence: true,
        evaluation_terms: RADIANCE_NEURAL_CACHE_COMPARE_ITEMS,
    },
    ResearchSpikeDescriptor {
        stable_id: "renderer.research.learned_page_priority_predictor",
        kind: ResearchSpikeKind::LearnedPagePriorityPredictor,
        owner: ResearchSpikeOwner::FunAi,
        feature_flag: LEARNED_PAGE_PRIORITY_PREDICTOR_FEATURE,
        compile_time_flag_required: true,
        runtime_opt_in_required: true,
        default_boot_requirement: false,
        default_renderer_dependency_allowed: false,
        capability_check_required: true,
        benchmark_comparison_required: true,
        deterministic_fallback_required: true,
        compute_fallback_required: false,
        promotion_requires_evidence: true,
        evaluation_terms: LEARNED_PAGE_PRIORITY_PROMOTION_ITEMS,
    },
    ResearchSpikeDescriptor {
        stable_id: "renderer.research.neural_texture_compression",
        kind: ResearchSpikeKind::NeuralTextureCompression,
        owner: ResearchSpikeOwner::OfflineAssetPipeline,
        feature_flag: NEURAL_TEXTURE_COMPRESSION_FEATURE,
        compile_time_flag_required: true,
        runtime_opt_in_required: true,
        default_boot_requirement: false,
        default_renderer_dependency_allowed: false,
        capability_check_required: true,
        benchmark_comparison_required: true,
        deterministic_fallback_required: true,
        compute_fallback_required: false,
        promotion_requires_evidence: true,
        evaluation_terms: NEURAL_TEXTURE_COMPRESSION_COMPARE_ITEMS,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResearchCompiledFeatureSupport {
    pub experimental_work_graphs: bool,
    pub mesh_shader_path: bool,
    pub radiance_neural_cache: bool,
    pub learned_page_priority_predictor: bool,
    pub neural_texture_compression: bool,
}

impl ResearchCompiledFeatureSupport {
    pub const COMPILED: Self = Self {
        experimental_work_graphs: cfg!(feature = "experimental_work_graphs"),
        mesh_shader_path: cfg!(feature = "mesh_shader_path"),
        radiance_neural_cache: false,
        learned_page_priority_predictor: cfg!(feature = "learned_page_priority_predictor"),
        neural_texture_compression: cfg!(feature = "neural_texture_compression"),
    };
}

impl Default for ResearchCompiledFeatureSupport {
    fn default() -> Self {
        Self::COMPILED
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResearchBenchmarkComparison {
    pub baseline_scene_id: &'static str,
    pub spike_scene_id: &'static str,
    pub p95_delta_us: i32,
    pub p99_delta_us: i32,
    pub page_fault_delta: i32,
    pub eviction_delta: i32,
    pub false_negative_delta: i32,
    pub latency_delta_us: i32,
    pub memory_pressure_delta_kb: i32,
    pub asset_size_delta_kb: i32,
    pub decode_cost_delta_us: i32,
    pub quality_proxy_delta_per_mille: i16,
    pub stability_delta_per_mille: i16,
    pub deterministic_fallback: bool,
    pub capability_checked: bool,
    pub compute_fallback_preserved: bool,
    pub compile_time_disabled_supported: bool,
    pub runtime_disable_supported: bool,
    pub benchmark_artifact_present: bool,
}

impl ResearchBenchmarkComparison {
    #[must_use]
    pub const fn conservative_baseline(spike_scene_id: &'static str) -> Self {
        Self {
            baseline_scene_id: "renderer.benchmark.scene.static_scene",
            spike_scene_id,
            p95_delta_us: 0,
            p99_delta_us: 0,
            page_fault_delta: 0,
            eviction_delta: 0,
            false_negative_delta: 0,
            latency_delta_us: 0,
            memory_pressure_delta_kb: 0,
            asset_size_delta_kb: 0,
            decode_cost_delta_us: 0,
            quality_proxy_delta_per_mille: 0,
            stability_delta_per_mille: 0,
            deterministic_fallback: true,
            capability_checked: true,
            compute_fallback_preserved: true,
            compile_time_disabled_supported: true,
            runtime_disable_supported: true,
            benchmark_artifact_present: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchRecommendation {
    Abandon,
    KeepExperimental,
    PromoteToProductionPass,
}

impl ResearchRecommendation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Abandon => "abandon",
            Self::KeepExperimental => "keep_experimental",
            Self::PromoteToProductionPass => "promote_to_production_pass",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResearchSpikeEvaluation {
    pub descriptor: ResearchSpikeDescriptor,
    pub comparison: ResearchBenchmarkComparison,
    pub recommendation: ResearchRecommendation,
    pub promotion_allowed: bool,
    pub reasons: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResearchSpikeArtifact {
    pub schema: &'static str,
    pub schema_version: u16,
    pub compiled_features: ResearchCompiledFeatureSupport,
    pub evaluations: Vec<ResearchSpikeEvaluation>,
}

#[must_use]
pub fn research_spike_descriptor(kind: ResearchSpikeKind) -> &'static ResearchSpikeDescriptor {
    RENDERER_RESEARCH_SPIKES
        .iter()
        .find(|descriptor| descriptor.kind == kind)
        .expect("research spike table must cover every kind")
}

#[must_use]
pub fn evaluate_research_spike(
    descriptor: ResearchSpikeDescriptor,
    comparison: ResearchBenchmarkComparison,
) -> ResearchSpikeEvaluation {
    let mut reasons = Vec::new();
    if descriptor.default_boot_requirement || descriptor.default_renderer_dependency_allowed {
        reasons.push("spike_must_not_bind_default_renderer");
    }
    if descriptor.compile_time_flag_required && !comparison.compile_time_disabled_supported {
        reasons.push("compile_time_disable_missing");
    }
    if descriptor.runtime_opt_in_required && !comparison.runtime_disable_supported {
        reasons.push("runtime_disable_missing");
    }
    if descriptor.capability_check_required && !comparison.capability_checked {
        reasons.push("capability_check_missing");
    }
    if descriptor.benchmark_comparison_required && !comparison.benchmark_artifact_present {
        reasons.push("benchmark_artifact_missing");
    }
    if descriptor.deterministic_fallback_required && !comparison.deterministic_fallback {
        reasons.push("deterministic_fallback_missing");
    }
    if descriptor.compute_fallback_required && !comparison.compute_fallback_preserved {
        reasons.push("compute_fallback_missing");
    }

    let recommendation = if !reasons.is_empty() {
        ResearchRecommendation::Abandon
    } else if promotion_evidence_satisfied(descriptor.kind, comparison) {
        ResearchRecommendation::PromoteToProductionPass
    } else {
        ResearchRecommendation::KeepExperimental
    };

    ResearchSpikeEvaluation {
        descriptor,
        comparison,
        recommendation,
        promotion_allowed: matches!(
            recommendation,
            ResearchRecommendation::PromoteToProductionPass
        ),
        reasons,
    }
}

fn promotion_evidence_satisfied(
    kind: ResearchSpikeKind,
    comparison: ResearchBenchmarkComparison,
) -> bool {
    match kind {
        ResearchSpikeKind::WorkGraphs => {
            comparison.p95_delta_us <= -100
                && comparison.p99_delta_us <= 0
                && comparison.latency_delta_us <= 0
                && comparison.stability_delta_per_mille >= 0
        }
        ResearchSpikeKind::MeshShaderPath => {
            comparison.p95_delta_us < 0
                && comparison.p99_delta_us <= 0
                && comparison.compute_fallback_preserved
                && comparison.quality_proxy_delta_per_mille >= 0
        }
        ResearchSpikeKind::RadianceNeuralCache => {
            comparison.stability_delta_per_mille > 0
                && comparison.p95_delta_us <= 0
                && comparison.p99_delta_us <= 0
                && comparison.latency_delta_us <= 0
                && comparison.memory_pressure_delta_kb <= 0
                && comparison.quality_proxy_delta_per_mille > 0
        }
        ResearchSpikeKind::LearnedPagePriorityPredictor => {
            comparison.page_fault_delta < 0
                && comparison.p95_delta_us <= 0
                && comparison.p99_delta_us <= 0
                && comparison.false_negative_delta <= 0
                && comparison.deterministic_fallback
        }
        ResearchSpikeKind::NeuralTextureCompression => {
            comparison.asset_size_delta_kb < 0
                && comparison.decode_cost_delta_us <= 0
                && comparison.memory_pressure_delta_kb <= 0
                && comparison.quality_proxy_delta_per_mille >= 0
                && comparison.page_fault_delta <= 0
        }
    }
}

#[must_use]
pub fn synthetic_pass22_research_comparisons()
-> [ResearchBenchmarkComparison; RENDERER_RESEARCH_SPIKE_COUNT] {
    [
        ResearchBenchmarkComparison {
            p95_delta_us: -80,
            p99_delta_us: 20,
            latency_delta_us: -30,
            stability_delta_per_mille: 0,
            ..ResearchBenchmarkComparison::conservative_baseline(
                "renderer.research.scene.work_graphs_cluster_refinement",
            )
        },
        ResearchBenchmarkComparison {
            p95_delta_us: -160,
            p99_delta_us: -40,
            quality_proxy_delta_per_mille: 5,
            ..ResearchBenchmarkComparison::conservative_baseline(
                "renderer.research.scene.mesh_shader_virtual_geometry",
            )
        },
        ResearchBenchmarkComparison {
            p95_delta_us: 120,
            p99_delta_us: 240,
            latency_delta_us: 90,
            memory_pressure_delta_kb: 32_768,
            quality_proxy_delta_per_mille: 80,
            stability_delta_per_mille: 35,
            ..ResearchBenchmarkComparison::conservative_baseline(
                "renderer.research.scene.radiance_neural_cache",
            )
        },
        ResearchBenchmarkComparison {
            p95_delta_us: -20,
            p99_delta_us: 0,
            page_fault_delta: -128,
            false_negative_delta: 0,
            ..ResearchBenchmarkComparison::conservative_baseline(
                "renderer.research.scene.learned_page_priority_predictor",
            )
        },
        ResearchBenchmarkComparison {
            page_fault_delta: -64,
            asset_size_delta_kb: -16_384,
            decode_cost_delta_us: 40,
            memory_pressure_delta_kb: -8_192,
            quality_proxy_delta_per_mille: 15,
            ..ResearchBenchmarkComparison::conservative_baseline(
                "renderer.research.scene.neural_texture_compression",
            )
        },
    ]
}

#[must_use]
pub fn evaluate_registered_research_spikes(
    comparisons: [ResearchBenchmarkComparison; RENDERER_RESEARCH_SPIKE_COUNT],
) -> Vec<ResearchSpikeEvaluation> {
    RENDERER_RESEARCH_SPIKES
        .into_iter()
        .zip(comparisons)
        .map(|(descriptor, comparison)| evaluate_research_spike(descriptor, comparison))
        .collect()
}

#[must_use]
pub fn synthetic_pass22_research_artifact() -> ResearchSpikeArtifact {
    ResearchSpikeArtifact {
        schema: RENDERER_RESEARCH_SPIKES_SCHEMA,
        schema_version: RENDERER_RESEARCH_SPIKES_SCHEMA_VERSION,
        compiled_features: ResearchCompiledFeatureSupport::COMPILED,
        evaluations: evaluate_registered_research_spikes(synthetic_pass22_research_comparisons()),
    }
}

pub fn write_research_spike_artifacts(
    json_path: impl AsRef<Path>,
    artifact: &ResearchSpikeArtifact,
) -> io::Result<PathBuf> {
    let json_path = json_path.as_ref();
    if let Some(parent) = json_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(artifact).map_err(io::Error::other)?;
    fs::write(json_path, json)?;

    let markdown_path = json_path.with_extension("md");
    fs::write(
        &markdown_path,
        ResearchSpikeMarkdownSummary::from_artifact(artifact)
            .content
            .as_bytes(),
    )?;
    Ok(markdown_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchSpikeMarkdownSummary {
    pub schema_version: u16,
    pub content: String,
}

impl ResearchSpikeMarkdownSummary {
    #[must_use]
    pub fn from_artifact(artifact: &ResearchSpikeArtifact) -> Self {
        let mut content = String::new();
        let _ = writeln!(content, "# Renderer Research Spikes");
        let _ = writeln!(content);
        let _ = writeln!(content, "schema: `{}`", artifact.schema);
        let _ = writeln!(content, "schema_version: `{}`", artifact.schema_version);
        let _ = writeln!(content);
        let _ = writeln!(
            content,
            "| spike | feature | owner | recommendation | reasons |"
        );
        let _ = writeln!(content, "| --- | --- | --- | --- | --- |");
        for evaluation in &artifact.evaluations {
            let reasons = if evaluation.reasons.is_empty() {
                "none".to_owned()
            } else {
                evaluation.reasons.join(",")
            };
            let _ = writeln!(
                content,
                "| `{}` | `{}` | `{}` | `{}` | `{}` |",
                evaluation.descriptor.kind.as_str(),
                evaluation.descriptor.feature_flag,
                evaluation.descriptor.owner.as_str(),
                evaluation.recommendation.as_str(),
                reasons
            );
        }
        Self {
            schema_version: artifact.schema_version,
            content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn research_catalog_registers_five_isolated_spikes() {
        assert_eq!(ResearchSpikeKind::ALL.len(), RENDERER_RESEARCH_SPIKE_COUNT);
        assert_eq!(
            RENDERER_RESEARCH_SPIKES.len(),
            RENDERER_RESEARCH_SPIKE_COUNT
        );

        for descriptor in RENDERER_RESEARCH_SPIKES {
            assert!(descriptor.compile_time_flag_required);
            assert!(descriptor.runtime_opt_in_required);
            assert!(!descriptor.default_boot_requirement);
            assert!(!descriptor.default_renderer_dependency_allowed);
            assert!(descriptor.benchmark_comparison_required);
            assert!(descriptor.promotion_requires_evidence);
            assert!(!descriptor.feature_flag.is_empty());
        }

        assert_eq!(
            research_spike_descriptor(ResearchSpikeKind::RadianceNeuralCache).owner,
            ResearchSpikeOwner::Lux
        );
        assert_eq!(
            research_spike_descriptor(ResearchSpikeKind::LearnedPagePriorityPredictor).owner,
            ResearchSpikeOwner::FunAi
        );
    }

    #[test]
    fn learned_page_predictor_requires_controlled_false_negatives_and_fallback() {
        let descriptor =
            *research_spike_descriptor(ResearchSpikeKind::LearnedPagePriorityPredictor);
        let mut comparison = ResearchBenchmarkComparison::conservative_baseline(
            "renderer.research.scene.learned_page_priority_predictor",
        );
        comparison.page_fault_delta = -200;
        comparison.p95_delta_us = -10;
        comparison.p99_delta_us = -10;
        comparison.false_negative_delta = 2;
        comparison.deterministic_fallback = false;

        let evaluation = evaluate_research_spike(descriptor, comparison);

        assert_eq!(evaluation.recommendation, ResearchRecommendation::Abandon);
        assert!(
            evaluation
                .reasons
                .contains(&"deterministic_fallback_missing")
        );
        assert!(!evaluation.promotion_allowed);
    }

    #[test]
    fn mesh_shader_spike_cannot_drop_compute_fallback() {
        let descriptor = *research_spike_descriptor(ResearchSpikeKind::MeshShaderPath);
        let mut comparison = ResearchBenchmarkComparison::conservative_baseline(
            "renderer.research.scene.mesh_shader_virtual_geometry",
        );
        comparison.p95_delta_us = -400;
        comparison.p99_delta_us = -300;
        comparison.compute_fallback_preserved = false;

        let evaluation = evaluate_research_spike(descriptor, comparison);

        assert_eq!(evaluation.recommendation, ResearchRecommendation::Abandon);
        assert!(evaluation.reasons.contains(&"compute_fallback_missing"));
    }

    #[test]
    fn positive_evidence_can_promote_only_when_isolation_stays_intact() {
        let mut comparisons = synthetic_pass22_research_comparisons();
        comparisons[3].page_fault_delta = -512;
        comparisons[3].p95_delta_us = -40;
        comparisons[3].p99_delta_us = -20;
        comparisons[3].false_negative_delta = 0;

        let evaluations = evaluate_registered_research_spikes(comparisons);
        let learned = evaluations
            .iter()
            .find(|evaluation| {
                evaluation.descriptor.kind == ResearchSpikeKind::LearnedPagePriorityPredictor
            })
            .expect("learned predictor spike should be present");

        assert_eq!(
            learned.recommendation,
            ResearchRecommendation::PromoteToProductionPass
        );
        assert!(learned.promotion_allowed);
        for evaluation in evaluations {
            assert!(!evaluation.descriptor.default_boot_requirement);
            assert!(!evaluation.descriptor.default_renderer_dependency_allowed);
        }
    }

    #[test]
    fn research_spike_artifact_records_recommendations() {
        let artifact = synthetic_pass22_research_artifact();

        assert_eq!(artifact.schema, RENDERER_RESEARCH_SPIKES_SCHEMA);
        assert_eq!(artifact.evaluations.len(), RENDERER_RESEARCH_SPIKE_COUNT);
        assert!(artifact.evaluations.iter().any(
            |evaluation| evaluation.recommendation == ResearchRecommendation::KeepExperimental
        ));

        let json = serde_json::to_value(&artifact).expect("artifact should serialize to JSON");
        assert_eq!(
            json["schema"],
            Value::String(RENDERER_RESEARCH_SPIKES_SCHEMA.into())
        );
        assert_eq!(
            json["evaluations"][0]["descriptor"]["feature_flag"],
            Value::String(EXPERIMENTAL_WORK_GRAPHS_FEATURE.into())
        );

        let summary = ResearchSpikeMarkdownSummary::from_artifact(&artifact);
        assert!(summary.content.contains("Renderer Research Spikes"));
        assert!(summary.content.contains("learned_page_priority_predictor"));

        if let Some(path) = std::env::var_os(RENDERER_RESEARCH_SPIKE_ARTIFACT_ENV) {
            let markdown = write_research_spike_artifacts(path, &artifact)
                .expect("research spike artifact should be writable");
            assert!(markdown.exists());
        }
    }
}
