use std::collections::{BTreeSet, VecDeque};

use bevy::prelude::Resource;

use crate::{
    FunDrawBudgetLane,
    gpu_visibility::{GpuVisibilityBatchId, GpuVisibilityCellId, GpuVisibilityObjectId},
    virtual_geometry::VirtualGeometryClusterId,
};

pub const FUN_HIZ_OCCLUSION_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunHiZOcclusionPipelineStage {
    ConsumeFrustumBatchClusterCullResults,
    DepthPrepassSelectedOccluders,
    BuildDepthPyramid,
    OcclusionTestCells,
    OcclusionTestBatches,
    OcclusionTestClusters,
    CompactVisibleList,
    DrawVisibleList,
}

impl FunHiZOcclusionPipelineStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ConsumeFrustumBatchClusterCullResults => {
                "consume_frustum_batch_cluster_cull_results"
            }
            Self::DepthPrepassSelectedOccluders => "depth_prepass_selected_occluders",
            Self::BuildDepthPyramid => "build_depth_pyramid",
            Self::OcclusionTestCells => "occlusion_test_cells",
            Self::OcclusionTestBatches => "occlusion_test_batches",
            Self::OcclusionTestClusters => "occlusion_test_clusters",
            Self::CompactVisibleList => "compact_visible_list",
            Self::DrawVisibleList => "draw_visible_list",
        }
    }
}

pub const FUN_HIZ_OCCLUSION_PIPELINE: &[FunHiZOcclusionPipelineStage] = &[
    FunHiZOcclusionPipelineStage::ConsumeFrustumBatchClusterCullResults,
    FunHiZOcclusionPipelineStage::DepthPrepassSelectedOccluders,
    FunHiZOcclusionPipelineStage::BuildDepthPyramid,
    FunHiZOcclusionPipelineStage::OcclusionTestCells,
    FunHiZOcclusionPipelineStage::OcclusionTestBatches,
    FunHiZOcclusionPipelineStage::OcclusionTestClusters,
    FunHiZOcclusionPipelineStage::CompactVisibleList,
    FunHiZOcclusionPipelineStage::DrawVisibleList,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunHiZOcclusionPrerequisite {
    RuntimeMeshCreationReduced,
    MaterialDuplicationReduced,
    StaticEntityBatchingReady,
    CpuDrawSubmissionReduced,
    FullBufferUploadsReduced,
    FrustumCullingReady,
    BatchCullingReady,
    ClusterCullingReady,
}

impl FunHiZOcclusionPrerequisite {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RuntimeMeshCreationReduced => "runtime_mesh_creation_reduced",
            Self::MaterialDuplicationReduced => "material_duplication_reduced",
            Self::StaticEntityBatchingReady => "static_entity_batching_ready",
            Self::CpuDrawSubmissionReduced => "cpu_draw_submission_reduced",
            Self::FullBufferUploadsReduced => "full_buffer_uploads_reduced",
            Self::FrustumCullingReady => "frustum_culling_ready",
            Self::BatchCullingReady => "batch_culling_ready",
            Self::ClusterCullingReady => "cluster_culling_ready",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunHiZOcclusionPrerequisites {
    pub runtime_mesh_creation_reduced: bool,
    pub material_duplication_reduced: bool,
    pub static_entity_batching_ready: bool,
    pub cpu_draw_submission_reduced: bool,
    pub full_buffer_uploads_reduced: bool,
    pub frustum_culling_ready: bool,
    pub batch_culling_ready: bool,
    pub cluster_culling_ready: bool,
}

impl FunHiZOcclusionPrerequisites {
    pub const fn ready() -> Self {
        Self {
            runtime_mesh_creation_reduced: true,
            material_duplication_reduced: true,
            static_entity_batching_ready: true,
            cpu_draw_submission_reduced: true,
            full_buffer_uploads_reduced: true,
            frustum_culling_ready: true,
            batch_culling_ready: true,
            cluster_culling_ready: true,
        }
    }

    pub const fn ready_for_hiz(self) -> bool {
        self.first_missing().is_none()
    }

    pub const fn first_missing(self) -> Option<FunHiZOcclusionPrerequisite> {
        if !self.runtime_mesh_creation_reduced {
            return Some(FunHiZOcclusionPrerequisite::RuntimeMeshCreationReduced);
        }
        if !self.material_duplication_reduced {
            return Some(FunHiZOcclusionPrerequisite::MaterialDuplicationReduced);
        }
        if !self.static_entity_batching_ready {
            return Some(FunHiZOcclusionPrerequisite::StaticEntityBatchingReady);
        }
        if !self.cpu_draw_submission_reduced {
            return Some(FunHiZOcclusionPrerequisite::CpuDrawSubmissionReduced);
        }
        if !self.full_buffer_uploads_reduced {
            return Some(FunHiZOcclusionPrerequisite::FullBufferUploadsReduced);
        }
        if !self.frustum_culling_ready {
            return Some(FunHiZOcclusionPrerequisite::FrustumCullingReady);
        }
        if !self.batch_culling_ready {
            return Some(FunHiZOcclusionPrerequisite::BatchCullingReady);
        }
        if !self.cluster_culling_ready {
            return Some(FunHiZOcclusionPrerequisite::ClusterCullingReady);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunOccluderClass {
    LargeWall,
    TerrainChunk,
    Building,
    CloseVehicle,
    StaticOpaqueStructure,
    Foliage,
    AlphaTestedAggregateGeometry,
    Particle,
    #[default]
    TinyProp,
    TransparentSurface,
    CefUi,
}

impl FunOccluderClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LargeWall => "large_wall",
            Self::TerrainChunk => "terrain_chunk",
            Self::Building => "building",
            Self::CloseVehicle => "close_vehicle",
            Self::StaticOpaqueStructure => "static_opaque_structure",
            Self::Foliage => "foliage",
            Self::AlphaTestedAggregateGeometry => "alpha_tested_aggregate_geometry",
            Self::Particle => "particle",
            Self::TinyProp => "tiny_prop",
            Self::TransparentSurface => "transparent_surface",
            Self::CefUi => "cef_ui",
        }
    }

    pub const fn is_good_occluder(self) -> bool {
        matches!(
            self,
            Self::LargeWall
                | Self::TerrainChunk
                | Self::Building
                | Self::CloseVehicle
                | Self::StaticOpaqueStructure
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunOccluderSelectionPolicy {
    pub min_screen_area: f32,
    pub close_vehicle_max_distance_meters: f32,
    pub max_occluders_per_frame: u32,
}

impl Default for FunOccluderSelectionPolicy {
    fn default() -> Self {
        Self {
            min_screen_area: 0.015,
            close_vehicle_max_distance_meters: 55.0,
            max_occluders_per_frame: 256,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunOccluderCandidate {
    pub id: u32,
    pub occluder_class: FunOccluderClass,
    pub screen_area: f32,
    pub distance_meters: f32,
    pub opaque: bool,
    pub draw_cost_ns: u64,
    pub estimated_coverage_per_mille: u16,
}

impl FunOccluderCandidate {
    pub fn score(self) -> i32 {
        let area = score_f32(self.screen_area, 0.0, 1.0, 4_000);
        let coverage = i32::from(self.estimated_coverage_per_mille.min(1_000)) * 4;
        let near = score_inverse_distance(self.distance_meters);
        let class = occluder_class_score(self.occluder_class);
        let cost_penalty = (self.draw_cost_ns / 25_000).min(1_000) as i32;
        area + coverage + near + class - cost_penalty
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunOccluderSelectionDecision {
    Selected,
    RejectedBadClass,
    RejectedTransparent,
    RejectedTooSmall,
    RejectedTooFar,
    RejectedBudget,
}

impl FunOccluderSelectionDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::RejectedBadClass => "rejected_bad_class",
            Self::RejectedTransparent => "rejected_transparent",
            Self::RejectedTooSmall => "rejected_too_small",
            Self::RejectedTooFar => "rejected_too_far",
            Self::RejectedBudget => "rejected_budget",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunOccluderSelectionReport {
    pub selected_occluder_ids: Vec<u32>,
    pub occluder_count: u32,
    pub occluder_draw_cost_ns: u64,
    pub rejected_bad_occluders: u32,
    pub rejected_transparent: u32,
    pub rejected_too_small: u32,
    pub rejected_too_far: u32,
    pub rejected_budget: u32,
    pub estimated_coverage_per_mille: u16,
}

impl FunOccluderSelectionReport {
    pub fn empty() -> Self {
        Self {
            selected_occluder_ids: Vec::new(),
            occluder_count: 0,
            occluder_draw_cost_ns: 0,
            rejected_bad_occluders: 0,
            rejected_transparent: 0,
            rejected_too_small: 0,
            rejected_too_far: 0,
            rejected_budget: 0,
            estimated_coverage_per_mille: 0,
        }
    }

    fn record_rejection(&mut self, decision: FunOccluderSelectionDecision) {
        match decision {
            FunOccluderSelectionDecision::Selected => {}
            FunOccluderSelectionDecision::RejectedBadClass => {
                self.rejected_bad_occluders = self.rejected_bad_occluders.saturating_add(1);
            }
            FunOccluderSelectionDecision::RejectedTransparent => {
                self.rejected_transparent = self.rejected_transparent.saturating_add(1);
            }
            FunOccluderSelectionDecision::RejectedTooSmall => {
                self.rejected_too_small = self.rejected_too_small.saturating_add(1);
            }
            FunOccluderSelectionDecision::RejectedTooFar => {
                self.rejected_too_far = self.rejected_too_far.saturating_add(1);
            }
            FunOccluderSelectionDecision::RejectedBudget => {
                self.rejected_budget = self.rejected_budget.saturating_add(1);
            }
        }
    }
}

pub fn evaluate_occluder(
    candidate: FunOccluderCandidate,
    policy: FunOccluderSelectionPolicy,
) -> FunOccluderSelectionDecision {
    if !candidate.opaque
        || matches!(
            candidate.occluder_class,
            FunOccluderClass::TransparentSurface | FunOccluderClass::CefUi
        )
    {
        return FunOccluderSelectionDecision::RejectedTransparent;
    }
    if !candidate.occluder_class.is_good_occluder() {
        return FunOccluderSelectionDecision::RejectedBadClass;
    }
    if candidate.screen_area < policy.min_screen_area {
        return FunOccluderSelectionDecision::RejectedTooSmall;
    }
    if candidate.occluder_class == FunOccluderClass::CloseVehicle
        && candidate.distance_meters > policy.close_vehicle_max_distance_meters
    {
        return FunOccluderSelectionDecision::RejectedTooFar;
    }
    FunOccluderSelectionDecision::Selected
}

pub fn select_occluders(
    candidates: &[FunOccluderCandidate],
    policy: FunOccluderSelectionPolicy,
) -> FunOccluderSelectionReport {
    let mut report = FunOccluderSelectionReport::empty();
    let mut selected = Vec::new();
    for candidate in candidates {
        match evaluate_occluder(*candidate, policy) {
            FunOccluderSelectionDecision::Selected => selected.push(*candidate),
            decision => report.record_rejection(decision),
        }
    }

    selected.sort_by(|left, right| {
        right
            .score()
            .cmp(&left.score())
            .then_with(|| left.id.cmp(&right.id))
    });

    for candidate in selected {
        if report.occluder_count >= policy.max_occluders_per_frame {
            report.record_rejection(FunOccluderSelectionDecision::RejectedBudget);
            continue;
        }
        report.selected_occluder_ids.push(candidate.id);
        report.occluder_count = report.occluder_count.saturating_add(1);
        report.occluder_draw_cost_ns = report
            .occluder_draw_cost_ns
            .saturating_add(candidate.draw_cost_ns);
        report.estimated_coverage_per_mille = report
            .estimated_coverage_per_mille
            .saturating_add(candidate.estimated_coverage_per_mille)
            .min(1_000);
    }

    report
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunHiZOcclusionPolicy {
    pub candidate_threshold: u32,
    pub tiny_scene_candidate_threshold: u32,
    pub min_occluder_count: u32,
    pub min_occluder_coverage_per_mille: u16,
    pub min_saved_draws: u32,
    pub min_saved_meshlets: u32,
    pub max_total_occlusion_cost_ns: u64,
    pub poor_payoff_window_frames: u8,
    pub poor_payoff_threshold_per_mille: u16,
    pub default_frequency_frames: u8,
    pub reduced_frequency_frames: u8,
    pub temporal_hidden_confirm_frames: u8,
    pub conservative_bounds_scale_per_mille: u16,
}

impl Default for FunHiZOcclusionPolicy {
    fn default() -> Self {
        Self {
            candidate_threshold: 4_096,
            tiny_scene_candidate_threshold: 768,
            min_occluder_count: 8,
            min_occluder_coverage_per_mille: 180,
            min_saved_draws: 24,
            min_saved_meshlets: 512,
            max_total_occlusion_cost_ns: 900_000,
            poor_payoff_window_frames: 8,
            poor_payoff_threshold_per_mille: 625,
            default_frequency_frames: 1,
            reduced_frequency_frames: 4,
            temporal_hidden_confirm_frames: 2,
            conservative_bounds_scale_per_mille: 1_100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunHiZOcclusionFrameInput {
    pub frame_index: u64,
    pub lane: FunDrawBudgetLane,
    pub prerequisites: FunHiZOcclusionPrerequisites,
    pub candidate_count: u32,
    pub occluder_count: u32,
    pub occluder_coverage_per_mille: u16,
    pub occluder_depth_prepass_supported: bool,
    pub depth_pyramid_cost_amortized: bool,
    pub camera_in_open_terrain: bool,
    pub debug_path: bool,
    pub unsupported_hardware: bool,
    pub last_p95_net_effect_ns: Option<i64>,
}

impl Default for FunHiZOcclusionFrameInput {
    fn default() -> Self {
        Self {
            frame_index: 0,
            lane: FunDrawBudgetLane::FullRuntime,
            prerequisites: FunHiZOcclusionPrerequisites::default(),
            candidate_count: 0,
            occluder_count: 0,
            occluder_coverage_per_mille: 0,
            occluder_depth_prepass_supported: true,
            depth_pyramid_cost_amortized: false,
            camera_in_open_terrain: false,
            debug_path: false,
            unsupported_hardware: false,
            last_p95_net_effect_ns: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FunHiZOcclusionDecision {
    #[default]
    DisabledTinyScene,
    DisabledUnsupportedHardware,
    DisabledDebugPath,
    DisabledPrerequisiteNotMet,
    DisabledNoDepthPrepass,
    DisabledDepthPyramidNotAmortized,
    DisabledFewCandidates,
    DisabledFewOccluders,
    DisabledLowOccluderCoverage,
    DisabledOpenTerrain,
    SkipOpenTerrainReducedFrequency,
    SkipReducedFrequency,
    OptInOnlyP95Regression,
    Enabled,
}

impl FunHiZOcclusionDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DisabledTinyScene => "disabled_tiny_scene",
            Self::DisabledUnsupportedHardware => "disabled_unsupported_hardware",
            Self::DisabledDebugPath => "disabled_debug_path",
            Self::DisabledPrerequisiteNotMet => "disabled_prerequisite_not_met",
            Self::DisabledNoDepthPrepass => "disabled_no_depth_prepass",
            Self::DisabledDepthPyramidNotAmortized => "disabled_depth_pyramid_not_amortized",
            Self::DisabledFewCandidates => "disabled_few_candidates",
            Self::DisabledFewOccluders => "disabled_few_occluders",
            Self::DisabledLowOccluderCoverage => "disabled_low_occluder_coverage",
            Self::DisabledOpenTerrain => "disabled_open_terrain",
            Self::SkipOpenTerrainReducedFrequency => "skip_open_terrain_reduced_frequency",
            Self::SkipReducedFrequency => "skip_reduced_frequency",
            Self::OptInOnlyP95Regression => "opt_in_only_p95_regression",
            Self::Enabled => "enabled",
        }
    }
}

impl FunHiZOcclusionPolicy {
    pub fn decide(
        self,
        input: FunHiZOcclusionFrameInput,
        adaptive: &FunHiZOcclusionAdaptiveState,
    ) -> FunHiZOcclusionDecision {
        if input.unsupported_hardware {
            return FunHiZOcclusionDecision::DisabledUnsupportedHardware;
        }
        if input.debug_path {
            return FunHiZOcclusionDecision::DisabledDebugPath;
        }
        if !input.prerequisites.ready_for_hiz() {
            return FunHiZOcclusionDecision::DisabledPrerequisiteNotMet;
        }
        if input.candidate_count <= self.tiny_scene_candidate_threshold {
            return FunHiZOcclusionDecision::DisabledTinyScene;
        }
        if !input.occluder_depth_prepass_supported {
            return FunHiZOcclusionDecision::DisabledNoDepthPrepass;
        }
        if !input.depth_pyramid_cost_amortized {
            return FunHiZOcclusionDecision::DisabledDepthPyramidNotAmortized;
        }
        if input.candidate_count < self.candidate_threshold {
            return FunHiZOcclusionDecision::DisabledFewCandidates;
        }
        if input.occluder_count < self.min_occluder_count {
            return FunHiZOcclusionDecision::DisabledFewOccluders;
        }
        if input.occluder_coverage_per_mille < self.min_occluder_coverage_per_mille {
            return if input.camera_in_open_terrain {
                FunHiZOcclusionDecision::DisabledOpenTerrain
            } else {
                FunHiZOcclusionDecision::DisabledLowOccluderCoverage
            };
        }
        if input.camera_in_open_terrain
            && !input
                .frame_index
                .is_multiple_of(u64::from(self.reduced_frequency_frames.max(1)))
        {
            return FunHiZOcclusionDecision::SkipOpenTerrainReducedFrequency;
        }
        if input
            .last_p95_net_effect_ns
            .is_some_and(|p95_net_effect_ns| p95_net_effect_ns > 0)
        {
            return FunHiZOcclusionDecision::OptInOnlyP95Regression;
        }
        if !adaptive.should_run_on_frame(input.frame_index, self) {
            return FunHiZOcclusionDecision::SkipReducedFrequency;
        }

        FunHiZOcclusionDecision::Enabled
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunHiZOcclusionBenchmarkSample {
    pub occluder_count: u32,
    pub occluder_draw_cost_ns: u64,
    pub hi_z_build_ns: u64,
    pub occlusion_cull_ns: u64,
    pub occlusion_saved_draws: u32,
    pub occlusion_saved_meshlets: u32,
    pub occlusion_saved_fragments_estimate: u64,
    pub p95_net_effect_ns: i64,
}

impl FunHiZOcclusionBenchmarkSample {
    pub const fn total_occlusion_cost_ns(self) -> u64 {
        self.occluder_draw_cost_ns
            .saturating_add(self.hi_z_build_ns)
            .saturating_add(self.occlusion_cull_ns)
    }

    pub const fn net_p95_win(self) -> bool {
        self.p95_net_effect_ns < 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunHiZOcclusionFrameReport {
    pub schema_version: u16,
    pub frame_index: u64,
    pub lane: FunDrawBudgetLane,
    pub decision: FunHiZOcclusionDecision,
    pub occluder_count: u32,
    pub occluder_draw_cost_ns: u64,
    pub hi_z_build_ns: u64,
    pub occlusion_cull_ns: u64,
    pub occlusion_saved_draws: u32,
    pub occlusion_saved_meshlets: u32,
    pub occlusion_saved_fragments_estimate: u64,
    pub total_occlusion_cost_ns: u64,
    pub p95_net_effect_ns: i64,
    pub net_p95_win: bool,
    pub keep_opt_in: bool,
}

impl FunHiZOcclusionFrameReport {
    pub fn from_benchmark_sample(
        frame_index: u64,
        lane: FunDrawBudgetLane,
        decision: FunHiZOcclusionDecision,
        sample: FunHiZOcclusionBenchmarkSample,
    ) -> Self {
        Self {
            schema_version: FUN_HIZ_OCCLUSION_SCHEMA_VERSION,
            frame_index,
            lane,
            decision,
            occluder_count: sample.occluder_count,
            occluder_draw_cost_ns: sample.occluder_draw_cost_ns,
            hi_z_build_ns: sample.hi_z_build_ns,
            occlusion_cull_ns: sample.occlusion_cull_ns,
            occlusion_saved_draws: sample.occlusion_saved_draws,
            occlusion_saved_meshlets: sample.occlusion_saved_meshlets,
            occlusion_saved_fragments_estimate: sample.occlusion_saved_fragments_estimate,
            total_occlusion_cost_ns: sample.total_occlusion_cost_ns(),
            p95_net_effect_ns: sample.p95_net_effect_ns,
            net_p95_win: sample.net_p95_win(),
            keep_opt_in: sample.occlusion_saved_draws > 0 && sample.p95_net_effect_ns > 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunHiZOcclusionTestLevel {
    #[default]
    Cell,
    Batch,
    Cluster,
}

impl FunHiZOcclusionTestLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cell => "cell",
            Self::Batch => "batch",
            Self::Cluster => "cluster",
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Cell => 0,
            Self::Batch => 1,
            Self::Cluster => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunHiZOcclusionTestCandidate {
    pub level: FunHiZOcclusionTestLevel,
    pub cell_id: GpuVisibilityCellId,
    pub batch_id: Option<GpuVisibilityBatchId>,
    pub object_id: Option<GpuVisibilityObjectId>,
    pub cluster_id: Option<VirtualGeometryClusterId>,
    pub survived_frustum_batch_cluster_culling: bool,
    pub depth_test_hidden: bool,
    pub visible_last_frame: bool,
    pub consecutive_hidden_depth_reads: u8,
    pub bounds_scale_per_mille: u16,
    pub estimated_saved_draws: u32,
    pub estimated_saved_meshlets: u32,
    pub estimated_saved_fragments: u64,
}

impl FunHiZOcclusionTestCandidate {
    fn stable_sort_key(self) -> (u8, u32, u32, u32, u64) {
        (
            self.level.rank(),
            self.cell_id.0,
            self.batch_id.map_or(0, |batch_id| batch_id.0),
            self.object_id.map_or(0, |object_id| object_id.0),
            self.cluster_id.map_or(0, |cluster_id| cluster_id.0),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunHiZOcclusionCullReport {
    pub visible_cell_ids: Vec<GpuVisibilityCellId>,
    pub visible_batch_ids: Vec<GpuVisibilityBatchId>,
    pub visible_cluster_ids: Vec<VirtualGeometryClusterId>,
    pub hidden_cell_count: u32,
    pub hidden_batch_count: u32,
    pub hidden_cluster_count: u32,
    pub prior_cull_rejected_count: u32,
    pub parent_hidden_count: u32,
    pub held_visible_count: u32,
    pub conservative_visible_count: u32,
    pub occlusion_saved_draws: u32,
    pub occlusion_saved_meshlets: u32,
    pub occlusion_saved_fragments_estimate: u64,
}

impl FunHiZOcclusionCullReport {
    pub fn visible_submission_count(&self) -> u32 {
        saturating_u32(
            self.visible_cell_ids
                .len()
                .saturating_add(self.visible_batch_ids.len())
                .saturating_add(self.visible_cluster_ids.len()),
        )
    }

    fn record_visible(
        &mut self,
        candidate: FunHiZOcclusionTestCandidate,
        decision: FunTemporalOcclusionDecision,
    ) {
        match candidate.level {
            FunHiZOcclusionTestLevel::Cell => self.visible_cell_ids.push(candidate.cell_id),
            FunHiZOcclusionTestLevel::Batch => {
                if let Some(batch_id) = candidate.batch_id {
                    self.visible_batch_ids.push(batch_id);
                }
            }
            FunHiZOcclusionTestLevel::Cluster => {
                if let Some(cluster_id) = candidate.cluster_id {
                    self.visible_cluster_ids.push(cluster_id);
                }
            }
        }
        match decision {
            FunTemporalOcclusionDecision::HoldVisible => {
                self.held_visible_count = self.held_visible_count.saturating_add(1);
            }
            FunTemporalOcclusionDecision::ConservativeVisible => {
                self.conservative_visible_count = self.conservative_visible_count.saturating_add(1);
            }
            FunTemporalOcclusionDecision::Visible | FunTemporalOcclusionDecision::Hidden => {}
        }
    }

    fn record_hidden(&mut self, candidate: FunHiZOcclusionTestCandidate) {
        match candidate.level {
            FunHiZOcclusionTestLevel::Cell => {
                self.hidden_cell_count = self.hidden_cell_count.saturating_add(1);
            }
            FunHiZOcclusionTestLevel::Batch => {
                self.hidden_batch_count = self.hidden_batch_count.saturating_add(1);
            }
            FunHiZOcclusionTestLevel::Cluster => {
                self.hidden_cluster_count = self.hidden_cluster_count.saturating_add(1);
            }
        }
        self.occlusion_saved_draws = self
            .occlusion_saved_draws
            .saturating_add(candidate.estimated_saved_draws);
        self.occlusion_saved_meshlets = self
            .occlusion_saved_meshlets
            .saturating_add(candidate.estimated_saved_meshlets);
        self.occlusion_saved_fragments_estimate = self
            .occlusion_saved_fragments_estimate
            .saturating_add(candidate.estimated_saved_fragments);
    }
}

pub fn occlusion_cull_after_visibility_culling(
    candidates: &[FunHiZOcclusionTestCandidate],
    policy: FunHiZOcclusionPolicy,
) -> FunHiZOcclusionCullReport {
    let mut sorted = candidates.to_vec();
    sorted.sort_by_key(|candidate| candidate.stable_sort_key());

    let mut hidden_cells = BTreeSet::new();
    let mut hidden_batches = BTreeSet::new();
    let mut report = FunHiZOcclusionCullReport::default();

    for candidate in sorted {
        if !candidate.survived_frustum_batch_cluster_culling {
            report.prior_cull_rejected_count = report.prior_cull_rejected_count.saturating_add(1);
            continue;
        }
        if hidden_cells.contains(&candidate.cell_id) {
            report.parent_hidden_count = report.parent_hidden_count.saturating_add(1);
            continue;
        }
        if let Some(batch_id) = candidate.batch_id
            && hidden_batches.contains(&batch_id)
        {
            report.parent_hidden_count = report.parent_hidden_count.saturating_add(1);
            continue;
        }

        let decision = temporal_occlusion_decision(
            FunTemporalOcclusionInput {
                depth_test_hidden: candidate.depth_test_hidden,
                visible_last_frame: candidate.visible_last_frame,
                consecutive_hidden_depth_reads: candidate.consecutive_hidden_depth_reads,
                bounds_scale_per_mille: candidate.bounds_scale_per_mille,
            },
            policy,
        );

        if decision == FunTemporalOcclusionDecision::Hidden {
            if candidate.level == FunHiZOcclusionTestLevel::Cell {
                hidden_cells.insert(candidate.cell_id);
            }
            if candidate.level == FunHiZOcclusionTestLevel::Batch
                && let Some(batch_id) = candidate.batch_id
            {
                hidden_batches.insert(batch_id);
            }
            report.record_hidden(candidate);
        } else {
            report.record_visible(candidate, decision);
        }
    }

    report
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunHiZOcclusionPayoffSample {
    pub frame_index: u64,
    pub total_occlusion_cost_ns: u64,
    pub occlusion_saved_draws: u32,
    pub occlusion_saved_meshlets: u32,
    pub p95_net_effect_ns: i64,
}

impl FunHiZOcclusionPayoffSample {
    pub const fn from_report(report: &FunHiZOcclusionFrameReport) -> Self {
        Self {
            frame_index: report.frame_index,
            total_occlusion_cost_ns: report.total_occlusion_cost_ns,
            occlusion_saved_draws: report.occlusion_saved_draws,
            occlusion_saved_meshlets: report.occlusion_saved_meshlets,
            p95_net_effect_ns: report.p95_net_effect_ns,
        }
    }

    pub const fn poor_payoff(self, policy: FunHiZOcclusionPolicy) -> bool {
        self.p95_net_effect_ns > 0
            || self.total_occlusion_cost_ns > policy.max_total_occlusion_cost_ns
            || self.occlusion_saved_draws < policy.min_saved_draws
            || self.occlusion_saved_meshlets < policy.min_saved_meshlets
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct FunHiZOcclusionAdaptiveState {
    recent_samples: VecDeque<FunHiZOcclusionPayoffSample>,
}

impl FunHiZOcclusionAdaptiveState {
    pub fn record_sample(
        &mut self,
        sample: FunHiZOcclusionPayoffSample,
        policy: FunHiZOcclusionPolicy,
    ) {
        self.recent_samples.push_back(sample);
        while self.recent_samples.len() > usize::from(policy.poor_payoff_window_frames) {
            self.recent_samples.pop_front();
        }
    }

    pub fn sample_count(&self) -> usize {
        self.recent_samples.len()
    }

    pub fn poor_payoff_ratio_per_mille(&self, policy: FunHiZOcclusionPolicy) -> u16 {
        if self.recent_samples.is_empty() {
            return 0;
        }
        let poor = self
            .recent_samples
            .iter()
            .filter(|sample| sample.poor_payoff(policy))
            .count();
        ((poor * 1_000) / self.recent_samples.len()) as u16
    }

    pub fn reduced_frequency_active(&self, policy: FunHiZOcclusionPolicy) -> bool {
        self.recent_samples.len() >= usize::from(policy.poor_payoff_window_frames)
            && self.poor_payoff_ratio_per_mille(policy) >= policy.poor_payoff_threshold_per_mille
    }

    pub fn should_run_on_frame(&self, frame_index: u64, policy: FunHiZOcclusionPolicy) -> bool {
        let frequency = if self.reduced_frequency_active(policy) {
            policy.reduced_frequency_frames
        } else {
            policy.default_frequency_frames
        }
        .max(1);

        frame_index.is_multiple_of(u64::from(frequency))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FunTemporalOcclusionDecision {
    #[default]
    Visible,
    HoldVisible,
    ConservativeVisible,
    Hidden,
}

impl FunTemporalOcclusionDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::HoldVisible => "hold_visible",
            Self::ConservativeVisible => "conservative_visible",
            Self::Hidden => "hidden",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunTemporalOcclusionInput {
    pub depth_test_hidden: bool,
    pub visible_last_frame: bool,
    pub consecutive_hidden_depth_reads: u8,
    pub bounds_scale_per_mille: u16,
}

pub const fn temporal_occlusion_decision(
    input: FunTemporalOcclusionInput,
    policy: FunHiZOcclusionPolicy,
) -> FunTemporalOcclusionDecision {
    if !input.depth_test_hidden {
        return FunTemporalOcclusionDecision::Visible;
    }
    if input.bounds_scale_per_mille < policy.conservative_bounds_scale_per_mille {
        return FunTemporalOcclusionDecision::ConservativeVisible;
    }
    if input.visible_last_frame
        && input.consecutive_hidden_depth_reads < policy.temporal_hidden_confirm_frames
    {
        return FunTemporalOcclusionDecision::HoldVisible;
    }
    FunTemporalOcclusionDecision::Hidden
}

fn score_f32(value: f32, min: f32, max: f32, scale: i32) -> i32 {
    if !value.is_finite() || max <= min {
        return 0;
    }
    let normalized = ((value.clamp(min, max) - min) / (max - min)).clamp(0.0, 1.0);
    (normalized * scale as f32) as i32
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

fn score_inverse_distance(distance_meters: f32) -> i32 {
    if !distance_meters.is_finite() {
        return 0;
    }
    let distance = distance_meters.max(0.0);
    ((1.0 / (1.0 + distance / 50.0)) * 1_000.0) as i32
}

const fn occluder_class_score(occluder_class: FunOccluderClass) -> i32 {
    match occluder_class {
        FunOccluderClass::LargeWall => 2_000,
        FunOccluderClass::TerrainChunk => 1_600,
        FunOccluderClass::Building => 1_900,
        FunOccluderClass::CloseVehicle => 1_100,
        FunOccluderClass::StaticOpaqueStructure => 1_700,
        FunOccluderClass::Foliage
        | FunOccluderClass::AlphaTestedAggregateGeometry
        | FunOccluderClass::Particle
        | FunOccluderClass::TinyProp
        | FunOccluderClass::TransparentSurface
        | FunOccluderClass::CefUi => -2_000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_candidate(id: u32, occluder_class: FunOccluderClass) -> FunOccluderCandidate {
        FunOccluderCandidate {
            id,
            occluder_class,
            screen_area: 0.12,
            distance_meters: 20.0,
            opaque: true,
            draw_cost_ns: 50_000,
            estimated_coverage_per_mille: 120,
        }
    }

    fn frame_input(candidate_count: u32, occluder_count: u32) -> FunHiZOcclusionFrameInput {
        FunHiZOcclusionFrameInput {
            frame_index: 8,
            lane: FunDrawBudgetLane::StreamingSpike,
            prerequisites: FunHiZOcclusionPrerequisites::ready(),
            candidate_count,
            occluder_count,
            occluder_coverage_per_mille: 300,
            occluder_depth_prepass_supported: true,
            depth_pyramid_cost_amortized: true,
            camera_in_open_terrain: false,
            debug_path: false,
            unsupported_hardware: false,
            last_p95_net_effect_ns: Some(-500_000),
        }
    }

    fn occlusion_candidate(
        level: FunHiZOcclusionTestLevel,
        cell_id: u32,
        batch_id: Option<u32>,
        cluster_id: Option<u64>,
        depth_test_hidden: bool,
    ) -> FunHiZOcclusionTestCandidate {
        FunHiZOcclusionTestCandidate {
            level,
            cell_id: GpuVisibilityCellId(cell_id),
            batch_id: batch_id.map(GpuVisibilityBatchId),
            object_id: Some(GpuVisibilityObjectId(cell_id.saturating_mul(10))),
            cluster_id: cluster_id.map(VirtualGeometryClusterId),
            survived_frustum_batch_cluster_culling: true,
            depth_test_hidden,
            visible_last_frame: false,
            consecutive_hidden_depth_reads: 3,
            bounds_scale_per_mille: 1_100,
            estimated_saved_draws: 1,
            estimated_saved_meshlets: 32,
            estimated_saved_fragments: 4_096,
        }
    }

    #[test]
    fn occluder_pipeline_order_is_stable() {
        let stages = FUN_HIZ_OCCLUSION_PIPELINE
            .iter()
            .map(|stage| stage.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            stages,
            [
                "consume_frustum_batch_cluster_cull_results",
                "depth_prepass_selected_occluders",
                "build_depth_pyramid",
                "occlusion_test_cells",
                "occlusion_test_batches",
                "occlusion_test_clusters",
                "compact_visible_list",
                "draw_visible_list"
            ]
        );
    }

    #[test]
    fn hiz_prerequisites_document_first_fixes_before_occlusion() {
        let prerequisites = FunHiZOcclusionPrerequisites {
            runtime_mesh_creation_reduced: true,
            material_duplication_reduced: true,
            static_entity_batching_ready: true,
            cpu_draw_submission_reduced: false,
            full_buffer_uploads_reduced: true,
            frustum_culling_ready: true,
            batch_culling_ready: true,
            cluster_culling_ready: true,
        };

        assert_eq!(
            prerequisites.first_missing(),
            Some(FunHiZOcclusionPrerequisite::CpuDrawSubmissionReduced)
        );
        assert_eq!(
            prerequisites
                .first_missing()
                .map(|missing| missing.as_str()),
            Some("cpu_draw_submission_reduced")
        );
    }

    #[test]
    fn good_occluders_are_selected_and_bad_occluders_are_rejected() {
        let candidates = [
            good_candidate(1, FunOccluderClass::LargeWall),
            good_candidate(2, FunOccluderClass::Building),
            good_candidate(3, FunOccluderClass::Foliage),
            FunOccluderCandidate {
                id: 4,
                occluder_class: FunOccluderClass::TransparentSurface,
                opaque: false,
                ..good_candidate(4, FunOccluderClass::TransparentSurface)
            },
        ];

        let report = select_occluders(&candidates, FunOccluderSelectionPolicy::default());

        assert_eq!(report.occluder_count, 2);
        assert_eq!(report.rejected_bad_occluders, 1);
        assert_eq!(report.rejected_transparent, 1);
        assert_eq!(report.selected_occluder_ids, [1, 2]);
    }

    #[test]
    fn close_vehicle_is_good_only_near_camera() {
        let policy = FunOccluderSelectionPolicy::default();
        let near = good_candidate(10, FunOccluderClass::CloseVehicle);
        let far = FunOccluderCandidate {
            distance_meters: 120.0,
            ..near
        };

        assert_eq!(
            evaluate_occluder(near, policy),
            FunOccluderSelectionDecision::Selected
        );
        assert_eq!(
            evaluate_occluder(far, policy),
            FunOccluderSelectionDecision::RejectedTooFar
        );
    }

    #[test]
    fn tiny_scene_disables_hiz() {
        let decision = FunHiZOcclusionPolicy::default().decide(
            frame_input(128, 16),
            &FunHiZOcclusionAdaptiveState::default(),
        );

        assert_eq!(decision, FunHiZOcclusionDecision::DisabledTinyScene);
    }

    #[test]
    fn hiz_stays_disabled_until_prior_culling_and_hot_path_work_is_ready() {
        let input = FunHiZOcclusionFrameInput {
            prerequisites: FunHiZOcclusionPrerequisites {
                runtime_mesh_creation_reduced: true,
                material_duplication_reduced: true,
                static_entity_batching_ready: true,
                cpu_draw_submission_reduced: true,
                full_buffer_uploads_reduced: true,
                frustum_culling_ready: true,
                batch_culling_ready: false,
                cluster_culling_ready: true,
            },
            ..frame_input(8_192, 16)
        };

        let decision = FunHiZOcclusionPolicy::default()
            .decide(input, &FunHiZOcclusionAdaptiveState::default());

        assert_eq!(
            decision,
            FunHiZOcclusionDecision::DisabledPrerequisiteNotMet
        );
    }

    #[test]
    fn depth_pyramid_cost_must_be_amortized() {
        let input = FunHiZOcclusionFrameInput {
            depth_pyramid_cost_amortized: false,
            ..frame_input(8_192, 16)
        };

        let decision = FunHiZOcclusionPolicy::default()
            .decide(input, &FunHiZOcclusionAdaptiveState::default());

        assert_eq!(
            decision,
            FunHiZOcclusionDecision::DisabledDepthPyramidNotAmortized
        );
    }

    #[test]
    fn open_terrain_with_few_occluders_disables_hiz() {
        let input = FunHiZOcclusionFrameInput {
            camera_in_open_terrain: true,
            occluder_coverage_per_mille: 50,
            ..frame_input(8_192, 16)
        };

        let decision = FunHiZOcclusionPolicy::default()
            .decide(input, &FunHiZOcclusionAdaptiveState::default());

        assert_eq!(decision, FunHiZOcclusionDecision::DisabledOpenTerrain);
    }

    #[test]
    fn low_occluder_coverage_disables_hiz_even_outside_open_terrain() {
        let input = FunHiZOcclusionFrameInput {
            occluder_coverage_per_mille: 50,
            ..frame_input(8_192, 16)
        };

        let decision = FunHiZOcclusionPolicy::default()
            .decide(input, &FunHiZOcclusionAdaptiveState::default());

        assert_eq!(
            decision,
            FunHiZOcclusionDecision::DisabledLowOccluderCoverage
        );
    }

    #[test]
    fn open_terrain_reduces_hiz_frequency_when_coverage_is_high() {
        let input = FunHiZOcclusionFrameInput {
            frame_index: 10,
            camera_in_open_terrain: true,
            occluder_coverage_per_mille: 450,
            ..frame_input(8_192, 16)
        };

        let decision = FunHiZOcclusionPolicy::default()
            .decide(input, &FunHiZOcclusionAdaptiveState::default());

        assert_eq!(
            decision,
            FunHiZOcclusionDecision::SkipOpenTerrainReducedFrequency
        );
    }

    #[test]
    fn hiz_enables_when_candidate_and_occluder_thresholds_are_met() {
        let decision = FunHiZOcclusionPolicy::default().decide(
            frame_input(8_192, 16),
            &FunHiZOcclusionAdaptiveState::default(),
        );

        assert_eq!(decision, FunHiZOcclusionDecision::Enabled);
    }

    #[test]
    fn occlusion_tests_cells_batches_clusters_in_order() {
        let candidates = [
            occlusion_candidate(
                FunHiZOcclusionTestLevel::Cluster,
                1,
                Some(10),
                Some(100),
                false,
            ),
            occlusion_candidate(FunHiZOcclusionTestLevel::Cell, 1, None, None, true),
            occlusion_candidate(FunHiZOcclusionTestLevel::Batch, 1, Some(10), None, false),
        ];

        let report =
            occlusion_cull_after_visibility_culling(&candidates, FunHiZOcclusionPolicy::default());

        assert_eq!(report.hidden_cell_count, 1);
        assert_eq!(report.parent_hidden_count, 2);
        assert!(report.visible_batch_ids.is_empty());
        assert!(report.visible_cluster_ids.is_empty());
        assert_eq!(report.occlusion_saved_draws, 1);
    }

    #[test]
    fn hidden_dense_cluster_does_not_reach_draw_submission() {
        let candidates = [
            occlusion_candidate(FunHiZOcclusionTestLevel::Cell, 2, None, None, false),
            occlusion_candidate(FunHiZOcclusionTestLevel::Batch, 2, Some(20), None, false),
            occlusion_candidate(
                FunHiZOcclusionTestLevel::Cluster,
                2,
                Some(20),
                Some(200),
                true,
            ),
        ];

        let report =
            occlusion_cull_after_visibility_culling(&candidates, FunHiZOcclusionPolicy::default());

        assert_eq!(report.visible_cell_ids, [GpuVisibilityCellId(2)]);
        assert_eq!(report.visible_batch_ids, [GpuVisibilityBatchId(20)]);
        assert!(report.visible_cluster_ids.is_empty());
        assert_eq!(report.hidden_cluster_count, 1);
        assert_eq!(report.visible_submission_count(), 2);
        assert_eq!(report.occlusion_saved_meshlets, 32);
    }

    #[test]
    fn prior_frustum_batch_cluster_rejections_do_not_run_hiz_again() {
        let rejected = FunHiZOcclusionTestCandidate {
            survived_frustum_batch_cluster_culling: false,
            ..occlusion_candidate(
                FunHiZOcclusionTestLevel::Cluster,
                3,
                Some(30),
                Some(300),
                true,
            )
        };

        let report =
            occlusion_cull_after_visibility_culling(&[rejected], FunHiZOcclusionPolicy::default());

        assert_eq!(report.prior_cull_rejected_count, 1);
        assert_eq!(report.hidden_cluster_count, 0);
        assert_eq!(report.visible_submission_count(), 0);
    }

    #[test]
    fn adaptive_poor_payoff_reduces_occlusion_frequency() {
        let policy = FunHiZOcclusionPolicy {
            poor_payoff_window_frames: 4,
            poor_payoff_threshold_per_mille: 500,
            reduced_frequency_frames: 4,
            ..Default::default()
        };
        let mut adaptive = FunHiZOcclusionAdaptiveState::default();
        for frame_index in 0..4 {
            adaptive.record_sample(
                FunHiZOcclusionPayoffSample {
                    frame_index,
                    total_occlusion_cost_ns: 1_200_000,
                    occlusion_saved_draws: 4,
                    occlusion_saved_meshlets: 16,
                    p95_net_effect_ns: 300_000,
                },
                policy,
            );
        }

        assert!(adaptive.reduced_frequency_active(policy));
        assert!(!adaptive.should_run_on_frame(10, policy));
        assert!(adaptive.should_run_on_frame(12, policy));
    }

    #[test]
    fn report_tracks_occlusion_cost_savings_and_p95_gate() {
        let report = FunHiZOcclusionFrameReport::from_benchmark_sample(
            20,
            FunDrawBudgetLane::StreamingSpike,
            FunHiZOcclusionDecision::Enabled,
            FunHiZOcclusionBenchmarkSample {
                occluder_count: 32,
                occluder_draw_cost_ns: 200_000,
                hi_z_build_ns: 180_000,
                occlusion_cull_ns: 140_000,
                occlusion_saved_draws: 90,
                occlusion_saved_meshlets: 4_500,
                occlusion_saved_fragments_estimate: 8_000_000,
                p95_net_effect_ns: -650_000,
            },
        );

        assert_eq!(report.schema_version, FUN_HIZ_OCCLUSION_SCHEMA_VERSION);
        assert_eq!(report.occluder_count, 32);
        assert_eq!(report.occluder_draw_cost_ns, 200_000);
        assert_eq!(report.hi_z_build_ns, 180_000);
        assert_eq!(report.occlusion_cull_ns, 140_000);
        assert_eq!(report.total_occlusion_cost_ns, 520_000);
        assert_eq!(report.occlusion_saved_draws, 90);
        assert_eq!(report.occlusion_saved_meshlets, 4_500);
        assert_eq!(report.occlusion_saved_fragments_estimate, 8_000_000);
        assert!(report.net_p95_win);
        assert!(!report.keep_opt_in);
    }

    #[test]
    fn p95_regression_keeps_hiz_opt_in_even_when_draws_are_saved() {
        let report = FunHiZOcclusionFrameReport::from_benchmark_sample(
            21,
            FunDrawBudgetLane::StreamingSpike,
            FunHiZOcclusionDecision::OptInOnlyP95Regression,
            FunHiZOcclusionBenchmarkSample {
                occluder_count: 24,
                occlusion_saved_draws: 60,
                occlusion_saved_meshlets: 2_000,
                p95_net_effect_ns: 400_000,
                ..Default::default()
            },
        );

        assert!(!report.net_p95_win);
        assert!(report.keep_opt_in);
    }

    #[test]
    fn temporal_visibility_requires_consecutive_hidden_reads() {
        let policy = FunHiZOcclusionPolicy::default();

        assert_eq!(
            temporal_occlusion_decision(
                FunTemporalOcclusionInput {
                    depth_test_hidden: true,
                    visible_last_frame: true,
                    consecutive_hidden_depth_reads: 1,
                    bounds_scale_per_mille: 1_100,
                },
                policy
            ),
            FunTemporalOcclusionDecision::HoldVisible
        );
        assert_eq!(
            temporal_occlusion_decision(
                FunTemporalOcclusionInput {
                    depth_test_hidden: true,
                    visible_last_frame: true,
                    consecutive_hidden_depth_reads: 2,
                    bounds_scale_per_mille: 1_100,
                },
                policy
            ),
            FunTemporalOcclusionDecision::Hidden
        );
    }

    #[test]
    fn non_conservative_bounds_keep_candidate_visible() {
        let decision = temporal_occlusion_decision(
            FunTemporalOcclusionInput {
                depth_test_hidden: true,
                visible_last_frame: false,
                consecutive_hidden_depth_reads: 4,
                bounds_scale_per_mille: 1_000,
            },
            FunHiZOcclusionPolicy::default(),
        );

        assert_eq!(decision, FunTemporalOcclusionDecision::ConservativeVisible);
    }
}
