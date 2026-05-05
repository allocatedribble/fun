use std::collections::BTreeSet;

use super::handles::Dx12DeviceQueueHandles;
use crate::{
    FunMaterialSignature,
    virtual_geometry::{
        VirtualGeometryAsset, VirtualGeometryAssetId, VirtualGeometryAssetKind,
        evaluate_virtual_geometry_asset,
    },
};

pub const DX12_MESH_SHADER_EXPERIMENT_SCHEMA_VERSION: u16 = 1;
pub const DX12_MESH_SHADER_EXPERIMENT_FEATURE: &str = "dx12_mesh_shader_experiment";
pub const DX12_MESH_SHADER_EXPERIMENT_NATIVE_BOUNDARY: &str = "fun_render::dx12_native";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Dx12MeshShaderClusterSource {
    #[default]
    FunVgLiteBakedClusters,
}

impl Dx12MeshShaderClusterSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunVgLiteBakedClusters => "funvg_lite_baked_clusters",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dx12MeshShaderBenchmarkScene {
    #[default]
    DenseStaticOpaqueWorld,
    SyntheticClusterDemo,
}

impl Dx12MeshShaderBenchmarkScene {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DenseStaticOpaqueWorld => "dense_static_opaque_world",
            Self::SyntheticClusterDemo => "synthetic_cluster_demo",
        }
    }

    pub const fn proves_real_dense_static_scene(self) -> bool {
        matches!(self, Self::DenseStaticOpaqueWorld)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Dx12MeshShaderComparisonPath {
    #[default]
    StandardRaster,
    ComputeCullIndirectRaster,
    NativeDx12MeshShader,
}

impl Dx12MeshShaderComparisonPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StandardRaster => "standard_raster",
            Self::ComputeCullIndirectRaster => "compute_cull_indirect_raster",
            Self::NativeDx12MeshShader => "native_dx12_mesh_shader",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dx12MeshShaderClusterInputSummary {
    pub source: Dx12MeshShaderClusterSource,
    pub asset_id: Option<VirtualGeometryAssetId>,
    pub static_opaque_funvg_lite: bool,
    pub baked_offline: bool,
    pub gpu_culling_ready: bool,
    pub indirect_raster_ready: bool,
    pub all_cluster_pages_known: bool,
    pub cluster_count: u32,
    pub triangle_count: u32,
    pub page_count: u32,
    pub material_path_count: u16,
    pub material_signature: Option<FunMaterialSignature>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dx12MeshShaderAdmissionInput {
    pub backend_is_dx12: bool,
    pub native_handles_valid: bool,
    pub compute_indirect_ready: bool,
    pub standard_raster_baseline_ready: bool,
    pub compute_indirect_baseline_ready: bool,
    pub benchmark_scene: Dx12MeshShaderBenchmarkScene,
    pub clusters: Dx12MeshShaderClusterInputSummary,
    pub cef_active: bool,
    pub skinned_mesh_count: u32,
    pub transparent_cluster_count: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Dx12MeshShaderAdmissionDecision {
    #[default]
    RejectNonDx12Backend,
    RejectNativeBoundaryUnavailable,
    RejectComputeIndirectNotReady,
    RejectMissingBenchmarkBaselines,
    RejectNonFunVgLiteStaticOpaqueClusters,
    RejectNoStaticOpaqueClusters,
    RejectUnknownClusterPages,
    RejectMultipleMaterialPaths,
    RejectCefActive,
    RejectSkinnedMeshes,
    RejectTransparency,
    RejectSyntheticBenchmarkScene,
    AdmitExperiment,
}

impl Dx12MeshShaderAdmissionDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RejectNonDx12Backend => "reject_non_dx12_backend",
            Self::RejectNativeBoundaryUnavailable => "reject_native_boundary_unavailable",
            Self::RejectComputeIndirectNotReady => "reject_compute_indirect_not_ready",
            Self::RejectMissingBenchmarkBaselines => "reject_missing_benchmark_baselines",
            Self::RejectNonFunVgLiteStaticOpaqueClusters => {
                "reject_non_funvg_lite_static_opaque_clusters"
            }
            Self::RejectNoStaticOpaqueClusters => "reject_no_static_opaque_clusters",
            Self::RejectUnknownClusterPages => "reject_unknown_cluster_pages",
            Self::RejectMultipleMaterialPaths => "reject_multiple_material_paths",
            Self::RejectCefActive => "reject_cef_active",
            Self::RejectSkinnedMeshes => "reject_skinned_meshes",
            Self::RejectTransparency => "reject_transparency",
            Self::RejectSyntheticBenchmarkScene => "reject_synthetic_benchmark_scene",
            Self::AdmitExperiment => "admit_experiment",
        }
    }

    pub const fn admitted(self) -> bool {
        matches!(self, Self::AdmitExperiment)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12MeshShaderDispatchPlan {
    pub schema_version: u16,
    pub feature_gate: &'static str,
    pub native_boundary: &'static str,
    pub decision: Dx12MeshShaderAdmissionDecision,
    pub cluster_source: Dx12MeshShaderClusterSource,
    pub benchmark_scene: Dx12MeshShaderBenchmarkScene,
    pub cluster_count: u32,
    pub triangle_count: u32,
    pub material_path_count: u16,
    pub material_signature: Option<FunMaterialSignature>,
    pub compare_standard_raster: bool,
    pub compare_compute_indirect_raster: bool,
    pub compare_native_dx12_mesh_shader: bool,
    pub cef_allowed: bool,
    pub skinned_meshes_allowed: bool,
    pub transparency_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12MeshShaderBenchmarkPolicy {
    pub min_real_scene_visible_clusters: u32,
    pub min_p95_win_over_compute_indirect_per_mille: u16,
}

impl Default for Dx12MeshShaderBenchmarkPolicy {
    fn default() -> Self {
        Self {
            min_real_scene_visible_clusters: 4_096,
            min_p95_win_over_compute_indirect_per_mille: 50,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12MeshShaderBenchmarkSample {
    pub path: Dx12MeshShaderComparisonPath,
    pub scene: Dx12MeshShaderBenchmarkScene,
    pub visible_cluster_count: u32,
    pub draw_or_dispatch_count: u32,
    pub cpu_submit_ns: u64,
    pub gpu_work_ns: u64,
    pub p95_frame_ns: u64,
}

impl Dx12MeshShaderBenchmarkSample {
    pub const fn total_work_ns(self) -> u64 {
        self.cpu_submit_ns.saturating_add(self.gpu_work_ns)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Dx12MeshShaderBenchmarkOutcome {
    KeepExperiment,
    LeaveAsExperimentSyntheticOnly,
    LeaveAsExperimentInsufficientDenseScene,
    #[default]
    KillComputeIndirectWinsOrTies,
}

impl Dx12MeshShaderBenchmarkOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KeepExperiment => "keep_experiment",
            Self::LeaveAsExperimentSyntheticOnly => "leave_as_experiment_synthetic_only",
            Self::LeaveAsExperimentInsufficientDenseScene => {
                "leave_as_experiment_insufficient_dense_scene"
            }
            Self::KillComputeIndirectWinsOrTies => "kill_compute_indirect_wins_or_ties",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12MeshShaderBenchmarkReport {
    pub schema_version: u16,
    pub scene: Dx12MeshShaderBenchmarkScene,
    pub standard_raster: Dx12MeshShaderBenchmarkSample,
    pub compute_indirect_raster: Dx12MeshShaderBenchmarkSample,
    pub native_dx12_mesh_shader: Dx12MeshShaderBenchmarkSample,
    pub best_path: Dx12MeshShaderComparisonPath,
    pub mesh_shader_vs_compute_indirect_p95_delta_ns: i64,
    pub mesh_shader_win_over_compute_indirect_per_mille: u16,
    pub outcome: Dx12MeshShaderBenchmarkOutcome,
}

pub fn summarize_funvg_lite_clusters(
    asset: &VirtualGeometryAsset,
) -> Dx12MeshShaderClusterInputSummary {
    let mut material_signatures = BTreeSet::new();
    let mut all_cluster_pages_known = true;
    for cluster in &asset.clusters {
        material_signatures.insert(cluster.material_signature);
        all_cluster_pages_known &= asset.page(cluster.page_id).is_some();
    }
    let static_opaque_funvg_lite = asset.kind == VirtualGeometryAssetKind::StaticOpaqueDense
        && evaluate_virtual_geometry_asset(asset).accepted()
        && asset.lite_policy.gpu_culling_required
        && asset.lite_policy.indirect_raster_first
        && !asset.lite_policy.software_visibility_buffer_enabled
        && !asset.lite_policy.bindless_mega_system_enabled;

    Dx12MeshShaderClusterInputSummary {
        source: Dx12MeshShaderClusterSource::FunVgLiteBakedClusters,
        asset_id: Some(asset.asset_id),
        static_opaque_funvg_lite,
        baked_offline: asset.lite_policy.baked_offline,
        gpu_culling_ready: asset.lite_policy.gpu_culling_required,
        indirect_raster_ready: asset.lite_policy.indirect_raster_first,
        all_cluster_pages_known,
        cluster_count: saturating_u32(asset.clusters.len()),
        triangle_count: asset.triangle_count,
        page_count: saturating_u32(asset.pages.len()),
        material_path_count: u16::try_from(material_signatures.len()).unwrap_or(u16::MAX),
        material_signature: material_signatures.iter().next().copied(),
    }
}

pub fn dx12_mesh_shader_native_boundary_valid(handles: Dx12DeviceQueueHandles) -> bool {
    !handles.device.is_null() && !handles.queue.is_null()
}

pub const fn decide_dx12_mesh_shader_experiment(
    input: Dx12MeshShaderAdmissionInput,
) -> Dx12MeshShaderAdmissionDecision {
    if !input.backend_is_dx12 {
        return Dx12MeshShaderAdmissionDecision::RejectNonDx12Backend;
    }
    if !input.native_handles_valid {
        return Dx12MeshShaderAdmissionDecision::RejectNativeBoundaryUnavailable;
    }
    if !input.compute_indirect_ready {
        return Dx12MeshShaderAdmissionDecision::RejectComputeIndirectNotReady;
    }
    if !input.standard_raster_baseline_ready || !input.compute_indirect_baseline_ready {
        return Dx12MeshShaderAdmissionDecision::RejectMissingBenchmarkBaselines;
    }
    if !input.clusters.static_opaque_funvg_lite
        || !input.clusters.baked_offline
        || !input.clusters.gpu_culling_ready
        || !input.clusters.indirect_raster_ready
    {
        return Dx12MeshShaderAdmissionDecision::RejectNonFunVgLiteStaticOpaqueClusters;
    }
    if input.clusters.cluster_count == 0 {
        return Dx12MeshShaderAdmissionDecision::RejectNoStaticOpaqueClusters;
    }
    if !input.clusters.all_cluster_pages_known {
        return Dx12MeshShaderAdmissionDecision::RejectUnknownClusterPages;
    }
    if input.clusters.material_path_count != 1 {
        return Dx12MeshShaderAdmissionDecision::RejectMultipleMaterialPaths;
    }
    if input.cef_active {
        return Dx12MeshShaderAdmissionDecision::RejectCefActive;
    }
    if input.skinned_mesh_count > 0 {
        return Dx12MeshShaderAdmissionDecision::RejectSkinnedMeshes;
    }
    if input.transparent_cluster_count > 0 {
        return Dx12MeshShaderAdmissionDecision::RejectTransparency;
    }
    if !input.benchmark_scene.proves_real_dense_static_scene() {
        return Dx12MeshShaderAdmissionDecision::RejectSyntheticBenchmarkScene;
    }

    Dx12MeshShaderAdmissionDecision::AdmitExperiment
}

pub const fn plan_dx12_mesh_shader_experiment(
    input: Dx12MeshShaderAdmissionInput,
) -> Dx12MeshShaderDispatchPlan {
    let decision = decide_dx12_mesh_shader_experiment(input);
    Dx12MeshShaderDispatchPlan {
        schema_version: DX12_MESH_SHADER_EXPERIMENT_SCHEMA_VERSION,
        feature_gate: DX12_MESH_SHADER_EXPERIMENT_FEATURE,
        native_boundary: DX12_MESH_SHADER_EXPERIMENT_NATIVE_BOUNDARY,
        decision,
        cluster_source: input.clusters.source,
        benchmark_scene: input.benchmark_scene,
        cluster_count: input.clusters.cluster_count,
        triangle_count: input.clusters.triangle_count,
        material_path_count: input.clusters.material_path_count,
        material_signature: input.clusters.material_signature,
        compare_standard_raster: input.standard_raster_baseline_ready,
        compare_compute_indirect_raster: input.compute_indirect_baseline_ready,
        compare_native_dx12_mesh_shader: decision.admitted(),
        cef_allowed: false,
        skinned_meshes_allowed: false,
        transparency_allowed: false,
    }
}

pub fn evaluate_dx12_mesh_shader_benchmark(
    standard_raster: Dx12MeshShaderBenchmarkSample,
    compute_indirect_raster: Dx12MeshShaderBenchmarkSample,
    native_dx12_mesh_shader: Dx12MeshShaderBenchmarkSample,
    policy: Dx12MeshShaderBenchmarkPolicy,
) -> Dx12MeshShaderBenchmarkReport {
    let delta = i128::from(native_dx12_mesh_shader.p95_frame_ns)
        - i128::from(compute_indirect_raster.p95_frame_ns);
    let win_per_mille = mesh_shader_win_per_mille(
        compute_indirect_raster.p95_frame_ns,
        native_dx12_mesh_shader.p95_frame_ns,
    );
    let scene = native_dx12_mesh_shader.scene;
    let best_path = best_path_by_p95(
        standard_raster,
        compute_indirect_raster,
        native_dx12_mesh_shader,
    );
    let outcome = if !scene.proves_real_dense_static_scene()
        && native_dx12_mesh_shader.p95_frame_ns < compute_indirect_raster.p95_frame_ns
    {
        Dx12MeshShaderBenchmarkOutcome::LeaveAsExperimentSyntheticOnly
    } else if native_dx12_mesh_shader.visible_cluster_count < policy.min_real_scene_visible_clusters
    {
        Dx12MeshShaderBenchmarkOutcome::LeaveAsExperimentInsufficientDenseScene
    } else if scene.proves_real_dense_static_scene()
        && win_per_mille >= policy.min_p95_win_over_compute_indirect_per_mille
    {
        Dx12MeshShaderBenchmarkOutcome::KeepExperiment
    } else {
        Dx12MeshShaderBenchmarkOutcome::KillComputeIndirectWinsOrTies
    };

    Dx12MeshShaderBenchmarkReport {
        schema_version: DX12_MESH_SHADER_EXPERIMENT_SCHEMA_VERSION,
        scene,
        standard_raster,
        compute_indirect_raster,
        native_dx12_mesh_shader,
        best_path,
        mesh_shader_vs_compute_indirect_p95_delta_ns: clamp_i128_to_i64(delta),
        mesh_shader_win_over_compute_indirect_per_mille: win_per_mille,
        outcome,
    }
}

fn best_path_by_p95(
    standard_raster: Dx12MeshShaderBenchmarkSample,
    compute_indirect_raster: Dx12MeshShaderBenchmarkSample,
    native_dx12_mesh_shader: Dx12MeshShaderBenchmarkSample,
) -> Dx12MeshShaderComparisonPath {
    let mut best_path = standard_raster.path;
    let mut best_p95 = standard_raster.p95_frame_ns;
    if compute_indirect_raster.p95_frame_ns < best_p95 {
        best_path = compute_indirect_raster.path;
        best_p95 = compute_indirect_raster.p95_frame_ns;
    }
    if native_dx12_mesh_shader.p95_frame_ns < best_p95 {
        best_path = native_dx12_mesh_shader.path;
    }
    best_path
}

fn mesh_shader_win_per_mille(compute_indirect_p95_ns: u64, mesh_shader_p95_ns: u64) -> u16 {
    if mesh_shader_p95_ns >= compute_indirect_p95_ns || compute_indirect_p95_ns == 0 {
        return 0;
    }
    let saved = compute_indirect_p95_ns - mesh_shader_p95_ns;
    let per_mille = (u128::from(saved) * 1_000) / u128::from(compute_indirect_p95_ns);
    u16::try_from(per_mille).unwrap_or(u16::MAX)
}

fn clamp_i128_to_i64(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use core::ptr::null_mut;

    use crate::{
        virtual_geometry::{
            VirtualGeometryBounds, VirtualGeometryChildRange, VirtualGeometryCluster,
            VirtualGeometryClusterCullHint, VirtualGeometryClusterId, VirtualGeometryHierarchyNode,
            VirtualGeometryHierarchyNodeId, VirtualGeometryLitePolicy, VirtualGeometryNormalCone,
            VirtualGeometryPage, VirtualGeometryPageId, VirtualGeometryRange,
        },
        {FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION, FunMaterialSignature},
    };

    use super::*;

    const ASSET_ID: VirtualGeometryAssetId = VirtualGeometryAssetId(10);
    const PAGE_ID: VirtualGeometryPageId = VirtualGeometryPageId(20);
    const MATERIAL: FunMaterialSignature = FunMaterialSignature(30);

    fn dense_static_asset(materials: &[FunMaterialSignature]) -> VirtualGeometryAsset {
        let clusters = materials
            .iter()
            .enumerate()
            .map(|(index, material_signature)| VirtualGeometryCluster {
                id: VirtualGeometryClusterId(index as u64),
                page_id: PAGE_ID,
                material_id: index as u32,
                material_signature: *material_signature,
                local_bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], 1.0),
                world_bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], 1.0),
                triangle_range: VirtualGeometryRange::new((index as u32) * 64, 64),
                index_range: VirtualGeometryRange::new((index as u32) * 192, 192),
                normal_cone: VirtualGeometryNormalCone::new([0.0, 1.0, 0.0], 0.5),
                parent_cluster_id: None,
                child_range: VirtualGeometryChildRange::EMPTY,
                triangle_count: 64,
                bounds_radius: 1.0,
                cluster_error: 0.01,
                screen_error: 0.01,
                screen_area: 0.2,
                distance_meters: 10.0,
                occluder: true,
                material_id_count: 1,
                cull_hint: VirtualGeometryClusterCullHint::Visible,
            })
            .collect::<Vec<_>>();

        VirtualGeometryAsset {
            schema_version: FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
            asset_id: ASSET_ID,
            root_node: VirtualGeometryHierarchyNodeId(0),
            kind: VirtualGeometryAssetKind::StaticOpaqueDense,
            lite_policy: VirtualGeometryLitePolicy::default(),
            pages: vec![VirtualGeometryPage {
                id: PAGE_ID,
                compressed_bytes: 4096,
                first_cluster: 0,
                cluster_count: materials.len() as u32,
            }],
            clusters,
            hierarchy_nodes: vec![VirtualGeometryHierarchyNode {
                id: VirtualGeometryHierarchyNodeId(0),
                cluster_id: VirtualGeometryClusterId(0),
                parent: None,
                child_range: VirtualGeometryChildRange::EMPTY,
                children: Vec::new(),
            }],
            alternate_cluster_sets: 0,
            material_id_count: 1,
            triangle_count: (materials.len() as u32) * 64,
        }
    }

    fn admitted_input() -> Dx12MeshShaderAdmissionInput {
        Dx12MeshShaderAdmissionInput {
            backend_is_dx12: true,
            native_handles_valid: true,
            compute_indirect_ready: true,
            standard_raster_baseline_ready: true,
            compute_indirect_baseline_ready: true,
            benchmark_scene: Dx12MeshShaderBenchmarkScene::DenseStaticOpaqueWorld,
            clusters: summarize_funvg_lite_clusters(&dense_static_asset(&[MATERIAL])),
            cef_active: false,
            skinned_mesh_count: 0,
            transparent_cluster_count: 0,
        }
    }

    fn sample(
        path: Dx12MeshShaderComparisonPath,
        scene: Dx12MeshShaderBenchmarkScene,
        p95_frame_ns: u64,
    ) -> Dx12MeshShaderBenchmarkSample {
        Dx12MeshShaderBenchmarkSample {
            path,
            scene,
            visible_cluster_count: 8_192,
            draw_or_dispatch_count: 64,
            cpu_submit_ns: p95_frame_ns / 10,
            gpu_work_ns: p95_frame_ns / 2,
            p95_frame_ns,
        }
    }

    #[test]
    fn feature_contract_is_native_dx12_only() {
        let plan = plan_dx12_mesh_shader_experiment(admitted_input());

        assert_eq!(plan.feature_gate, DX12_MESH_SHADER_EXPERIMENT_FEATURE);
        assert_eq!(
            plan.native_boundary,
            DX12_MESH_SHADER_EXPERIMENT_NATIVE_BOUNDARY
        );
        assert!(!plan.cef_allowed);
        assert!(!plan.skinned_meshes_allowed);
        assert!(!plan.transparency_allowed);
    }

    #[test]
    fn native_boundary_rejects_null_handles() {
        let handles = Dx12DeviceQueueHandles {
            device: null_mut(),
            queue: null_mut(),
        };

        assert!(!dx12_mesh_shader_native_boundary_valid(handles));
    }

    #[test]
    fn admits_only_after_compute_indirect_and_baselines_are_ready() {
        let plan = plan_dx12_mesh_shader_experiment(admitted_input());

        assert_eq!(
            plan.decision,
            Dx12MeshShaderAdmissionDecision::AdmitExperiment
        );
        assert!(plan.compare_standard_raster);
        assert!(plan.compare_compute_indirect_raster);
        assert!(plan.compare_native_dx12_mesh_shader);
        assert_eq!(plan.material_path_count, 1);
    }

    #[test]
    fn rejects_before_compute_indirect_works() {
        let input = Dx12MeshShaderAdmissionInput {
            compute_indirect_ready: false,
            ..admitted_input()
        };

        assert_eq!(
            decide_dx12_mesh_shader_experiment(input),
            Dx12MeshShaderAdmissionDecision::RejectComputeIndirectNotReady
        );
    }

    #[test]
    fn rejects_non_funvg_lite_static_opaque_clusters() {
        let mut asset = dense_static_asset(&[MATERIAL]);
        asset.kind = VirtualGeometryAssetKind::Transparent;
        let input = Dx12MeshShaderAdmissionInput {
            clusters: summarize_funvg_lite_clusters(&asset),
            ..admitted_input()
        };

        assert_eq!(
            decide_dx12_mesh_shader_experiment(input),
            Dx12MeshShaderAdmissionDecision::RejectNonFunVgLiteStaticOpaqueClusters
        );
    }

    #[test]
    fn rejects_multiple_material_paths() {
        let input = Dx12MeshShaderAdmissionInput {
            clusters: summarize_funvg_lite_clusters(&dense_static_asset(&[
                FunMaterialSignature(1),
                FunMaterialSignature(2),
            ])),
            ..admitted_input()
        };

        assert_eq!(
            decide_dx12_mesh_shader_experiment(input),
            Dx12MeshShaderAdmissionDecision::RejectMultipleMaterialPaths
        );
    }

    #[test]
    fn rejects_cef_skinned_or_transparent_content() {
        let cef = Dx12MeshShaderAdmissionInput {
            cef_active: true,
            ..admitted_input()
        };
        let skinned = Dx12MeshShaderAdmissionInput {
            skinned_mesh_count: 1,
            ..admitted_input()
        };
        let transparent = Dx12MeshShaderAdmissionInput {
            transparent_cluster_count: 1,
            ..admitted_input()
        };

        assert_eq!(
            decide_dx12_mesh_shader_experiment(cef),
            Dx12MeshShaderAdmissionDecision::RejectCefActive
        );
        assert_eq!(
            decide_dx12_mesh_shader_experiment(skinned),
            Dx12MeshShaderAdmissionDecision::RejectSkinnedMeshes
        );
        assert_eq!(
            decide_dx12_mesh_shader_experiment(transparent),
            Dx12MeshShaderAdmissionDecision::RejectTransparency
        );
    }

    #[test]
    fn synthetic_scene_win_stays_experiment_only() {
        let scene = Dx12MeshShaderBenchmarkScene::SyntheticClusterDemo;
        let report = evaluate_dx12_mesh_shader_benchmark(
            sample(
                Dx12MeshShaderComparisonPath::StandardRaster,
                scene,
                12_000_000,
            ),
            sample(
                Dx12MeshShaderComparisonPath::ComputeCullIndirectRaster,
                scene,
                8_000_000,
            ),
            sample(
                Dx12MeshShaderComparisonPath::NativeDx12MeshShader,
                scene,
                6_000_000,
            ),
            Dx12MeshShaderBenchmarkPolicy::default(),
        );

        assert_eq!(
            report.outcome,
            Dx12MeshShaderBenchmarkOutcome::LeaveAsExperimentSyntheticOnly
        );
        assert_eq!(
            report.best_path,
            Dx12MeshShaderComparisonPath::NativeDx12MeshShader
        );
    }

    #[test]
    fn real_dense_scene_must_beat_compute_indirect_to_keep() {
        let scene = Dx12MeshShaderBenchmarkScene::DenseStaticOpaqueWorld;
        let report = evaluate_dx12_mesh_shader_benchmark(
            sample(
                Dx12MeshShaderComparisonPath::StandardRaster,
                scene,
                15_000_000,
            ),
            sample(
                Dx12MeshShaderComparisonPath::ComputeCullIndirectRaster,
                scene,
                10_000_000,
            ),
            sample(
                Dx12MeshShaderComparisonPath::NativeDx12MeshShader,
                scene,
                9_000_000,
            ),
            Dx12MeshShaderBenchmarkPolicy::default(),
        );

        assert_eq!(
            report.outcome,
            Dx12MeshShaderBenchmarkOutcome::KeepExperiment
        );
        assert_eq!(report.mesh_shader_win_over_compute_indirect_per_mille, 100);
        assert_eq!(
            report.mesh_shader_vs_compute_indirect_p95_delta_ns,
            -1_000_000
        );
    }

    #[test]
    fn compute_indirect_win_kills_mesh_shader_experiment() {
        let scene = Dx12MeshShaderBenchmarkScene::DenseStaticOpaqueWorld;
        let report = evaluate_dx12_mesh_shader_benchmark(
            sample(
                Dx12MeshShaderComparisonPath::StandardRaster,
                scene,
                15_000_000,
            ),
            sample(
                Dx12MeshShaderComparisonPath::ComputeCullIndirectRaster,
                scene,
                8_000_000,
            ),
            sample(
                Dx12MeshShaderComparisonPath::NativeDx12MeshShader,
                scene,
                9_000_000,
            ),
            Dx12MeshShaderBenchmarkPolicy::default(),
        );

        assert_eq!(
            report.outcome,
            Dx12MeshShaderBenchmarkOutcome::KillComputeIndirectWinsOrTies
        );
        assert_eq!(
            report.best_path,
            Dx12MeshShaderComparisonPath::ComputeCullIndirectRaster
        );
    }
}
