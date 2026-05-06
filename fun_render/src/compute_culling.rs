use std::{cmp::Ordering, collections::BTreeMap};

use bevy::{
    asset::{AssetServer, Handle, embedded_asset, load_embedded_asset},
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        render_resource::{
            BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedComputePipelineId,
            ComputePipelineDescriptor, PipelineCache, ShaderStages, ShaderType,
            binding_types::{storage_buffer, storage_buffer_read_only, uniform_buffer},
        },
    },
    shader::Shader,
};

use crate::{FunGeometryClass, FunMaterialClass, FunRenderPath, FunRenderPhaseKind};

pub const FUN_COMPUTE_CULLING_SCHEMA_VERSION: u16 = 1;
pub const FUN_COMPUTE_CULLING_WORKGROUP_SIZE: u32 = 64;
pub const FUN_COMPUTE_CULLING_SHADER: &str = "compute_culling.wgsl";
pub const FUN_GPU_CULLING_ENV: &str = "FUN_RENDER_GPU_CULLING";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource)]
pub struct FunComputeCullingConfig {
    pub mode: FunComputeCullingRuntimeMode,
    pub policy: FunComputeCullingPolicy,
}

impl FunComputeCullingConfig {
    pub fn from_env() -> Self {
        let mode = match std::env::var(FUN_GPU_CULLING_ENV)
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Ok("1") | Ok("true") | Ok("on") | Ok("gpu") => FunComputeCullingRuntimeMode::Gpu,
            Ok("diagnostic") | Ok("diagnostics") | Ok("diag") => {
                FunComputeCullingRuntimeMode::Diagnostics
            }
            Ok("0") | Ok("false") | Ok("off") | Ok("") => FunComputeCullingRuntimeMode::Off,
            Ok(_) | Err(_) => {
                if std::env::var_os("FUN_RENDER_DIAGNOSTICS").is_some() {
                    FunComputeCullingRuntimeMode::Diagnostics
                } else {
                    FunComputeCullingRuntimeMode::Off
                }
            }
        };
        Self {
            mode,
            policy: FunComputeCullingPolicy::default(),
        }
    }

    pub const fn queue_pipelines(self) -> bool {
        matches!(
            self.mode,
            FunComputeCullingRuntimeMode::Diagnostics | FunComputeCullingRuntimeMode::Gpu
        )
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FunComputeCullingRuntimeMode {
    #[default]
    Off,
    Diagnostics,
    Gpu,
}

impl FunComputeCullingRuntimeMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Diagnostics => "diagnostics",
            Self::Gpu => "gpu",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunComputeCullingStage {
    InstanceFrustumCull,
    LodSelect,
    MeshletClusterCull,
    HiZOcclusion,
    Compaction,
    IndirectArgumentGeneration,
}

impl FunComputeCullingStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstanceFrustumCull => "instance_frustum_cull",
            Self::LodSelect => "lod_select",
            Self::MeshletClusterCull => "meshlet_cluster_cull",
            Self::HiZOcclusion => "hiz_occlusion",
            Self::Compaction => "compaction",
            Self::IndirectArgumentGeneration => "indirect_argument_generation",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FunCullingExecutionPath {
    #[default]
    Cpu,
    Gpu,
    GpuWithHiZ,
    DiagnosticsOnly,
}

impl FunCullingExecutionPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Gpu => "gpu",
            Self::GpuWithHiZ => "gpu_hiz",
            Self::DiagnosticsOnly => "diagnostics_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunComputeCullingPolicy {
    pub gpu_candidate_threshold: u32,
    pub gpu_visibility_ratio_threshold_per_mille: u16,
    pub hiz_candidate_threshold: u32,
    pub hiz_occluder_density_threshold_per_mille: u16,
    pub min_draw_calls_to_save: u32,
    pub max_compute_cost_budget_ns: u64,
}

impl Default for FunComputeCullingPolicy {
    fn default() -> Self {
        Self {
            gpu_candidate_threshold: 2_048,
            gpu_visibility_ratio_threshold_per_mille: 650,
            hiz_candidate_threshold: 8_192,
            hiz_occluder_density_threshold_per_mille: 350,
            min_draw_calls_to_save: 32,
            max_compute_cost_budget_ns: 750_000,
        }
    }
}

impl FunComputeCullingPolicy {
    pub fn decide(self, input: FunComputeCullingDecisionInput) -> FunCullingExecutionPath {
        if input.debug_path || input.unsupported_hardware || input.candidate_count == 0 {
            return FunCullingExecutionPath::Cpu;
        }
        if input
            .p95_net_effect_ns
            .is_some_and(|net_effect_ns| net_effect_ns > 0)
        {
            return FunCullingExecutionPath::DiagnosticsOnly;
        }
        if input.candidate_count < self.gpu_candidate_threshold
            || !input.static_dynamic_buffers_resident
            || input.expected_visibility_ratio_per_mille
                > self.gpu_visibility_ratio_threshold_per_mille
            || input
                .draw_calls_before
                .saturating_sub(input.draw_calls_after)
                < self.min_draw_calls_to_save
        {
            return FunCullingExecutionPath::Cpu;
        }
        if input.depth_pyramid_available
            && input.candidate_count >= self.hiz_candidate_threshold
            && input.occluder_density_per_mille >= self.hiz_occluder_density_threshold_per_mille
        {
            FunCullingExecutionPath::GpuWithHiZ
        } else {
            FunCullingExecutionPath::Gpu
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunComputeCullingDecisionInput {
    pub candidate_count: u32,
    pub expected_visibility_ratio_per_mille: u16,
    pub static_dynamic_buffers_resident: bool,
    pub draw_calls_before: u32,
    pub draw_calls_after: u32,
    pub debug_path: bool,
    pub unsupported_hardware: bool,
    pub depth_pyramid_available: bool,
    pub occluder_density_per_mille: u16,
    pub p95_net_effect_ns: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunComputeCullingBufferLifetime {
    Persistent,
    PerFrameRing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunComputeCullingBufferKind {
    InstanceMetadata,
    MeshletMetadata,
    MaterialTable,
    TransformTable,
    VisibilityFlags,
    CompactVisibleIds,
    IndirectArgs,
    CameraViewConstants,
    Counters,
    SmallDynamicUpdates,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunComputeCullingBufferDescriptor {
    pub kind: FunComputeCullingBufferKind,
    pub lifetime: FunComputeCullingBufferLifetime,
    pub stride_bytes: u64,
    pub alignment_bytes: u64,
    pub full_buffer_write_allowed: bool,
    pub normal_readback_allowed: bool,
}

pub const FUN_COMPUTE_CULLING_BUFFER_PLAN: &[FunComputeCullingBufferDescriptor] = &[
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::InstanceMetadata,
        lifetime: FunComputeCullingBufferLifetime::Persistent,
        stride_bytes: FunGpuCullingInstanceMetadata::SIZE_BYTES,
        alignment_bytes: FunGpuCullingInstanceMetadata::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::MeshletMetadata,
        lifetime: FunComputeCullingBufferLifetime::Persistent,
        stride_bytes: FunGpuMeshletClusterMetadata::SIZE_BYTES,
        alignment_bytes: FunGpuMeshletClusterMetadata::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::MaterialTable,
        lifetime: FunComputeCullingBufferLifetime::Persistent,
        stride_bytes: 16,
        alignment_bytes: 16,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::TransformTable,
        lifetime: FunComputeCullingBufferLifetime::Persistent,
        stride_bytes: 64,
        alignment_bytes: 16,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::VisibilityFlags,
        lifetime: FunComputeCullingBufferLifetime::Persistent,
        stride_bytes: 4,
        alignment_bytes: 4,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::CompactVisibleIds,
        lifetime: FunComputeCullingBufferLifetime::Persistent,
        stride_bytes: 4,
        alignment_bytes: 4,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::IndirectArgs,
        lifetime: FunComputeCullingBufferLifetime::Persistent,
        stride_bytes: FunGpuDrawIndirectArgs::SIZE_BYTES,
        alignment_bytes: FunGpuDrawIndirectArgs::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::CameraViewConstants,
        lifetime: FunComputeCullingBufferLifetime::PerFrameRing,
        stride_bytes: FunGpuCullingViewConstants::SIZE_BYTES,
        alignment_bytes: FunGpuCullingViewConstants::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::Counters,
        lifetime: FunComputeCullingBufferLifetime::PerFrameRing,
        stride_bytes: FunGpuVisibilityCounters::SIZE_BYTES,
        alignment_bytes: FunGpuVisibilityCounters::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: true,
    },
    FunComputeCullingBufferDescriptor {
        kind: FunComputeCullingBufferKind::SmallDynamicUpdates,
        lifetime: FunComputeCullingBufferLifetime::PerFrameRing,
        stride_bytes: 16,
        alignment_bytes: 16,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
];

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct FunGpuCullingInstanceMetadata {
    pub bounds_center_radius: [f32; 4],
    pub bounds_extent_lod: [f32; 4],
    pub meshlet_range: [u32; 4],
    pub bucket_key: [u32; 4],
    pub table_indices_flags: [u32; 4],
}

impl FunGpuCullingInstanceMetadata {
    pub const SIZE_BYTES: u64 = 80;
    pub const ALIGN_BYTES: u64 = 16;
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct FunGpuMeshletClusterMetadata {
    pub bounds_center_radius: [f32; 4],
    pub normal_cone: [f32; 4],
    pub instance_meshlet_range: [u32; 4],
    pub flags: [u32; 4],
}

impl FunGpuMeshletClusterMetadata {
    pub const SIZE_BYTES: u64 = 64;
    pub const ALIGN_BYTES: u64 = 16;
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct FunGpuCullingViewConstants {
    pub frustum_planes: [[f32; 4]; 6],
    pub camera_position_lod: [f32; 4],
    pub viewport_hysteresis: [f32; 4],
    pub counts: [u32; 4],
}

impl FunGpuCullingViewConstants {
    pub const SIZE_BYTES: u64 = 144;
    pub const ALIGN_BYTES: u64 = 16;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ShaderType)]
pub struct FunGpuVisibilityCounters {
    pub visible_instance_count: u32,
    pub selected_lod_count: u32,
    pub visible_meshlet_count: u32,
    pub indirect_arg_count: u32,
}

impl FunGpuVisibilityCounters {
    pub const SIZE_BYTES: u64 = 16;
    pub const ALIGN_BYTES: u64 = 4;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ShaderType)]
pub struct FunGpuDrawIndirectArgs {
    pub vertex_count_per_instance: u32,
    pub instance_count: u32,
    pub first_vertex: u32,
    pub first_instance: u32,
}

impl FunGpuDrawIndirectArgs {
    pub const SIZE_BYTES: u64 = 16;
    pub const ALIGN_BYTES: u64 = 4;
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ShaderType)]
pub struct FunGpuDispatchIndirectArgs {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub _pad: u32,
}

impl FunGpuDispatchIndirectArgs {
    pub const SIZE_BYTES: u64 = 16;
    pub const ALIGN_BYTES: u64 = 4;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunCullingBucketKey {
    pub phase: FunRenderPhaseKind,
    pub material_class: FunMaterialClass,
    pub geometry_class: FunGeometryClass,
    pub render_path: FunRenderPath,
    pub lod: u8,
}

impl FunCullingBucketKey {
    pub const fn packed(self) -> u64 {
        ((phase_code(self.phase) as u64) << 40)
            | ((material_code(self.material_class) as u64) << 32)
            | ((geometry_code(self.geometry_class) as u64) << 24)
            | ((render_path_code(self.render_path) as u64) << 16)
            | self.lod as u64
    }
}

impl Ord for FunCullingBucketKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.packed().cmp(&other.packed())
    }
}

impl PartialOrd for FunCullingBucketKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunCullingBucketRecord {
    pub key: FunCullingBucketKey,
    pub source_order: u32,
    pub draw_count: u32,
    pub instance_count: u32,
}

pub fn stable_sort_culling_buckets(records: &mut [FunCullingBucketRecord]) {
    records.sort_by_key(|record| record.key);
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunComputeCullingBenchmarkSample {
    pub culling_pass_cost_ns: u64,
    pub draw_calls_before: u32,
    pub draw_calls_after: u32,
    pub cpu_submission_saved_ns: u64,
    pub p95_net_effect_ns: i64,
}

impl FunComputeCullingBenchmarkSample {
    pub const fn draw_call_reduction(self) -> u32 {
        self.draw_calls_before.saturating_sub(self.draw_calls_after)
    }

    pub const fn p95_improved(self) -> bool {
        self.p95_net_effect_ns < 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunComputeCullingFrameReport {
    pub frame_index: u64,
    pub execution_path: FunCullingExecutionPath,
    pub culling_pass_cost_ns: u64,
    pub draw_calls_before: u32,
    pub draw_calls_after: u32,
    pub draw_call_reduction: u32,
    pub cpu_submission_saved_ns: u64,
    pub p95_net_effect_ns: i64,
    pub keep_behind_diagnostics: bool,
}

impl FunComputeCullingFrameReport {
    pub fn from_benchmark_sample(
        frame_index: u64,
        execution_path: FunCullingExecutionPath,
        sample: FunComputeCullingBenchmarkSample,
    ) -> Self {
        Self {
            frame_index,
            execution_path,
            culling_pass_cost_ns: sample.culling_pass_cost_ns,
            draw_calls_before: sample.draw_calls_before,
            draw_calls_after: sample.draw_calls_after,
            draw_call_reduction: sample.draw_call_reduction(),
            cpu_submission_saved_ns: sample.cpu_submission_saved_ns,
            p95_net_effect_ns: sample.p95_net_effect_ns,
            keep_behind_diagnostics: sample.p95_net_effect_ns > 0,
        }
    }
}

#[derive(Debug, Resource)]
pub struct FunComputeCullingPipelineLayout {
    pub culling: BindGroupLayoutDescriptor,
}

#[derive(Debug, Resource)]
pub struct FunComputeCullingPipelines {
    pub reset: CachedComputePipelineId,
    pub instance_frustum_cull: CachedComputePipelineId,
    pub lod_select: CachedComputePipelineId,
    pub meshlet_cluster_cull: CachedComputePipelineId,
    pub hiz_occlusion_cull: CachedComputePipelineId,
    pub compact_visible_instances: CachedComputePipelineId,
    pub generate_indirect_args: CachedComputePipelineId,
}

pub const CULLING_VIEW_CONSTANTS_READ_ONLY: bool = false;
pub const CULLING_INSTANCE_METADATA_READ_ONLY: bool = true;
pub const CULLING_MESHLET_CLUSTER_METADATA_READ_ONLY: bool = true;
pub const CULLING_VISIBILITY_FLAGS_READ_ONLY: bool = false;
pub const CULLING_COMPACT_IDS_READ_ONLY: bool = false;
pub const CULLING_INDIRECT_ARGS_READ_ONLY: bool = false;
pub const CULLING_COUNTERS_READ_ONLY: bool = false;
pub const CULLING_HIZ_READ_ONLY: bool = true;
pub const CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET: bool = false;

pub fn install_fun_compute_culling(app: &mut App) {
    let config = FunComputeCullingConfig::from_env();
    app.insert_resource(config);
    if !config.queue_pipelines() {
        return;
    }

    load_compute_culling_shader_assets(app);
    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };
    render_app.insert_resource(config);
    render_app.add_systems(RenderStartup, init_compute_culling_pipelines);
    game_shared::fun_diag_info!(
        target: "fun::render",
        mode = config.mode.as_str(),
        schema_version = FUN_COMPUTE_CULLING_SCHEMA_VERSION,
        "Fun compute culling pipelines queued"
    );
}

pub fn load_compute_culling_shader_assets(app: &mut App) {
    embedded_asset!(app, "compute_culling.wgsl");
}

pub fn init_compute_culling_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
) {
    let culling = BindGroupLayoutDescriptor::new(
        "fun_compute_culling_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<FunGpuCullingViewConstants>(CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET),
                storage_buffer_read_only::<FunGpuCullingInstanceMetadata>(
                    CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET,
                ),
                storage_buffer_read_only::<FunGpuMeshletClusterMetadata>(
                    CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET,
                ),
                storage_buffer::<u32>(CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET),
                storage_buffer::<u32>(CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET),
                storage_buffer::<FunGpuDrawIndirectArgs>(CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET),
                storage_buffer::<u32>(CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET),
                storage_buffer_read_only::<f32>(CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET),
            ),
        ),
    );

    let shader = load_embedded_asset!(asset_server.as_ref(), "compute_culling.wgsl");
    let pipelines = FunComputeCullingPipelines {
        reset: queue_compute_culling_pipeline(
            &pipeline_cache,
            &shader,
            culling.clone(),
            "fun_compute_culling_reset_pipeline",
            "reset_culling_state",
        ),
        instance_frustum_cull: queue_compute_culling_pipeline(
            &pipeline_cache,
            &shader,
            culling.clone(),
            "fun_compute_culling_instance_frustum_pipeline",
            "instance_frustum_cull",
        ),
        lod_select: queue_compute_culling_pipeline(
            &pipeline_cache,
            &shader,
            culling.clone(),
            "fun_compute_culling_lod_select_pipeline",
            "lod_select",
        ),
        meshlet_cluster_cull: queue_compute_culling_pipeline(
            &pipeline_cache,
            &shader,
            culling.clone(),
            "fun_compute_culling_meshlet_cluster_pipeline",
            "meshlet_cluster_cull",
        ),
        hiz_occlusion_cull: queue_compute_culling_pipeline(
            &pipeline_cache,
            &shader,
            culling.clone(),
            "fun_compute_culling_hiz_occlusion_pipeline",
            "hiz_occlusion_cull",
        ),
        compact_visible_instances: queue_compute_culling_pipeline(
            &pipeline_cache,
            &shader,
            culling.clone(),
            "fun_compute_culling_compaction_pipeline",
            "compact_visible_instances",
        ),
        generate_indirect_args: queue_compute_culling_pipeline(
            &pipeline_cache,
            &shader,
            culling.clone(),
            "fun_compute_culling_indirect_args_pipeline",
            "generate_indirect_args",
        ),
    };

    commands.insert_resource(FunComputeCullingPipelineLayout { culling });
    commands.insert_resource(pipelines);
}

fn queue_compute_culling_pipeline(
    pipeline_cache: &PipelineCache,
    shader: &Handle<Shader>,
    layout: BindGroupLayoutDescriptor,
    label: &'static str,
    entry_point: &'static str,
) -> CachedComputePipelineId {
    pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
        label: Some(label.into()),
        layout: vec![layout],
        shader: shader.clone(),
        entry_point: Some(entry_point.into()),
        zero_initialize_workgroup_memory: true,
        ..default()
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunCullingCpuCandidate {
    pub instance_id: u32,
    pub center: [f32; 3],
    pub radius: f32,
    pub screen_space_size: f32,
    pub previous_lod: u32,
    pub lod_count: u32,
    pub meshlet_root: u32,
    pub meshlet_count: u32,
    pub bucket_key: FunCullingBucketKey,
    pub occluded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FunCullingCpuPlane {
    pub normal: [f32; 3],
    pub d: f32,
}

impl FunCullingCpuPlane {
    pub const fn new(normal: [f32; 3], d: f32) -> Self {
        Self { normal, d }
    }

    fn signed_distance(self, point: [f32; 3]) -> f32 {
        self.normal[0].mul_add(
            point[0],
            self.normal[1].mul_add(point[1], self.normal[2].mul_add(point[2], self.d)),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunCullingCpuReferenceOutput {
    pub visible_instance_ids: Vec<u32>,
    pub selected_lods: BTreeMap<u32, u32>,
    pub visible_meshlet_ids: Vec<u32>,
    pub draw_args: Vec<FunGpuDrawIndirectArgs>,
    pub dispatch_args: Vec<FunGpuDispatchIndirectArgs>,
}

pub fn simulate_compute_culling_cpu_reference(
    candidates: &[FunCullingCpuCandidate],
    planes: &[FunCullingCpuPlane],
    use_hiz: bool,
    lod_hysteresis: f32,
) -> FunCullingCpuReferenceOutput {
    let mut visible = Vec::new();
    let mut selected_lods = BTreeMap::new();
    let mut visible_meshlets = Vec::new();
    let mut bucket_instances: BTreeMap<FunCullingBucketKey, u32> = BTreeMap::new();

    for candidate in candidates {
        if !sphere_intersects_frustum(candidate.center, candidate.radius, planes) {
            continue;
        }
        if use_hiz && candidate.occluded {
            continue;
        }

        visible.push(candidate.instance_id);
        let selected_lod = select_lod_with_hysteresis(
            candidate.screen_space_size,
            candidate.previous_lod,
            candidate.lod_count,
            lod_hysteresis,
        );
        selected_lods.insert(candidate.instance_id, selected_lod);
        for meshlet_offset in 0..candidate.meshlet_count {
            visible_meshlets.push(candidate.meshlet_root.saturating_add(meshlet_offset));
        }
        let count = bucket_instances.entry(candidate.bucket_key).or_default();
        *count = count.saturating_add(1);
    }

    let draw_args = bucket_instances
        .values()
        .copied()
        .map(|instance_count| FunGpuDrawIndirectArgs {
            vertex_count_per_instance: 3,
            instance_count,
            first_vertex: 0,
            first_instance: 0,
        })
        .collect::<Vec<_>>();
    let dispatch_args = if visible_meshlets.is_empty() {
        Vec::new()
    } else {
        vec![FunGpuDispatchIndirectArgs {
            x: (visible_meshlets.len() as u32).div_ceil(FUN_COMPUTE_CULLING_WORKGROUP_SIZE),
            y: 1,
            z: 1,
            _pad: 0,
        }]
    };

    FunCullingCpuReferenceOutput {
        visible_instance_ids: visible,
        selected_lods,
        visible_meshlet_ids: visible_meshlets,
        draw_args,
        dispatch_args,
    }
}

fn sphere_intersects_frustum(center: [f32; 3], radius: f32, planes: &[FunCullingCpuPlane]) -> bool {
    planes
        .iter()
        .all(|plane| plane.signed_distance(center) + radius >= 0.0)
}

fn select_lod_with_hysteresis(
    screen_space_size: f32,
    previous_lod: u32,
    lod_count: u32,
    hysteresis: f32,
) -> u32 {
    if lod_count <= 1 {
        return 0;
    }

    let desired_lod = if screen_space_size >= 0.45 {
        0
    } else if screen_space_size >= 0.20 {
        1
    } else {
        2
    }
    .min(lod_count.saturating_sub(1));

    if desired_lod != previous_lod {
        let threshold = if desired_lod > previous_lod {
            0.45
        } else {
            0.20
        };
        if (screen_space_size - threshold).abs() <= hysteresis {
            return previous_lod.min(lod_count.saturating_sub(1));
        }
    }
    desired_lod
}

const fn phase_code(phase: FunRenderPhaseKind) -> u8 {
    match phase {
        FunRenderPhaseKind::DepthPrepass => 1,
        FunRenderPhaseKind::Shadow => 2,
        FunRenderPhaseKind::MainOpaque => 3,
        FunRenderPhaseKind::DeferredGBuffer => 4,
        FunRenderPhaseKind::Transparent => 5,
        FunRenderPhaseKind::MeshletVisibility => 6,
        FunRenderPhaseKind::MeshletResolve => 7,
        FunRenderPhaseKind::Clouds => 8,
        FunRenderPhaseKind::Solari => 9,
        FunRenderPhaseKind::PostProcess => 10,
        FunRenderPhaseKind::CefUi => 11,
        FunRenderPhaseKind::DebugOverlay => 12,
    }
}

const fn material_code(material_class: FunMaterialClass) -> u8 {
    match material_class {
        FunMaterialClass::OpaqueSimple => 1,
        FunMaterialClass::OpaqueComplex => 2,
        FunMaterialClass::Transparent => 3,
        FunMaterialClass::Emissive => 4,
        FunMaterialClass::Viewmodel => 5,
        FunMaterialClass::Ui => 6,
        FunMaterialClass::Debug => 7,
    }
}

const fn geometry_code(geometry_class: FunGeometryClass) -> u8 {
    match geometry_class {
        FunGeometryClass::StaticOpaqueDense => 1,
        FunGeometryClass::StaticOpaqueSimple => 2,
        FunGeometryClass::DynamicOpaque => 3,
        FunGeometryClass::SkinnedCharacter => 4,
        FunGeometryClass::Vehicle => 5,
        FunGeometryClass::Foliage => 6,
        FunGeometryClass::Particle => 7,
        FunGeometryClass::Decal => 8,
        FunGeometryClass::Viewmodel => 9,
        FunGeometryClass::Ui => 10,
    }
}

const fn render_path_code(render_path: FunRenderPath) -> u8 {
    match render_path {
        FunRenderPath::StandardRaster => 1,
        FunRenderPath::InstancedRaster => 2,
        FunRenderPath::GpuCulledIndirect => 3,
        FunRenderPath::MeshletStaticDense => 4,
        FunRenderPath::MeshletDynamicDense => 5,
        FunRenderPath::RayProxyOnly => 6,
        FunRenderPath::Viewmodel => 7,
        FunRenderPath::CefUi => 8,
        FunRenderPath::DebugOnly => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    fn default_bucket(lod: u8) -> FunCullingBucketKey {
        FunCullingBucketKey {
            phase: FunRenderPhaseKind::MainOpaque,
            material_class: FunMaterialClass::OpaqueSimple,
            geometry_class: FunGeometryClass::StaticOpaqueSimple,
            render_path: FunRenderPath::GpuCulledIndirect,
            lod,
        }
    }

    fn inside_candidate(instance_id: u32) -> FunCullingCpuCandidate {
        FunCullingCpuCandidate {
            instance_id,
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
            screen_space_size: 0.5,
            previous_lod: 0,
            lod_count: 3,
            meshlet_root: 10,
            meshlet_count: 2,
            bucket_key: default_bucket(0),
            occluded: false,
        }
    }

    fn positive_x_plane() -> [FunCullingCpuPlane; 1] {
        [FunCullingCpuPlane::new([1.0, 0.0, 0.0], 2.0)]
    }

    #[test]
    fn buffer_layout_size_and_alignment_are_stable() {
        assert_eq!(
            mem::size_of::<FunGpuCullingInstanceMetadata>() as u64,
            FunGpuCullingInstanceMetadata::SIZE_BYTES
        );
        assert_eq!(
            mem::align_of::<FunGpuCullingInstanceMetadata>() as u64,
            FunGpuCullingInstanceMetadata::ALIGN_BYTES
        );
        assert_eq!(
            mem::size_of::<FunGpuMeshletClusterMetadata>() as u64,
            FunGpuMeshletClusterMetadata::SIZE_BYTES
        );
        assert_eq!(
            mem::size_of::<FunGpuCullingViewConstants>() as u64,
            FunGpuCullingViewConstants::SIZE_BYTES
        );
        assert_eq!(
            mem::size_of::<FunGpuDrawIndirectArgs>() as u64,
            FunGpuDrawIndirectArgs::SIZE_BYTES
        );
        assert_eq!(
            mem::size_of::<FunGpuDispatchIndirectArgs>() as u64,
            FunGpuDispatchIndirectArgs::SIZE_BYTES
        );
    }

    #[test]
    fn buffer_plan_keeps_visibility_buffers_persistent_and_unread() {
        let visibility = FUN_COMPUTE_CULLING_BUFFER_PLAN
            .iter()
            .find(|descriptor| descriptor.kind == FunComputeCullingBufferKind::VisibilityFlags)
            .expect("visibility buffer descriptor should exist");
        assert_eq!(
            visibility.lifetime,
            FunComputeCullingBufferLifetime::Persistent
        );
        assert!(!visibility.full_buffer_write_allowed);
        assert!(!visibility.normal_readback_allowed);

        let compact = FUN_COMPUTE_CULLING_BUFFER_PLAN
            .iter()
            .find(|descriptor| descriptor.kind == FunComputeCullingBufferKind::CompactVisibleIds)
            .expect("compact buffer descriptor should exist");
        assert_eq!(
            compact.lifetime,
            FunComputeCullingBufferLifetime::Persistent
        );
        assert!(!compact.full_buffer_write_allowed);
        assert!(!compact.normal_readback_allowed);
    }

    #[test]
    fn bucket_key_determinism() {
        let a = default_bucket(2);
        let b = default_bucket(2);
        let c = default_bucket(1);

        assert_eq!(a.packed(), b.packed());
        assert_ne!(a.packed(), c.packed());
    }

    #[test]
    fn stable_sorting_preserves_equal_bucket_order() {
        let shared = default_bucket(0);
        let later = default_bucket(1);
        let mut records = [
            FunCullingBucketRecord {
                key: later,
                source_order: 0,
                draw_count: 1,
                instance_count: 1,
            },
            FunCullingBucketRecord {
                key: shared,
                source_order: 1,
                draw_count: 1,
                instance_count: 1,
            },
            FunCullingBucketRecord {
                key: shared,
                source_order: 2,
                draw_count: 1,
                instance_count: 1,
            },
        ];

        stable_sort_culling_buckets(&mut records);

        assert_eq!(records[0].source_order, 1);
        assert_eq!(records[1].source_order, 2);
        assert_eq!(records[2].source_order, 0);
    }

    #[test]
    fn gpu_smoke_no_candidates_produces_zero_draw_args() {
        let output = simulate_compute_culling_cpu_reference(&[], &positive_x_plane(), false, 0.05);

        assert!(output.visible_instance_ids.is_empty());
        assert!(output.draw_args.is_empty());
        assert!(output.dispatch_args.is_empty());
    }

    #[test]
    fn gpu_smoke_one_visible_candidate_produces_one_draw_arg() {
        let output = simulate_compute_culling_cpu_reference(
            &[inside_candidate(7)],
            &positive_x_plane(),
            false,
            0.05,
        );

        assert_eq!(output.visible_instance_ids, [7]);
        assert_eq!(output.visible_meshlet_ids, [10, 11]);
        assert_eq!(output.draw_args.len(), 1);
        assert_eq!(output.draw_args[0].instance_count, 1);
        assert_eq!(output.dispatch_args[0].x, 1);
    }

    #[test]
    fn gpu_smoke_frustum_out_candidate_is_culled() {
        let mut candidate = inside_candidate(8);
        candidate.center = [-4.0, 0.0, 0.0];

        let output =
            simulate_compute_culling_cpu_reference(&[candidate], &positive_x_plane(), false, 0.05);

        assert!(output.visible_instance_ids.is_empty());
        assert!(output.draw_args.is_empty());
    }

    #[test]
    fn gpu_smoke_lod_hysteresis_prevents_flicker() {
        let mut candidate = inside_candidate(9);
        candidate.screen_space_size = 0.43;
        candidate.previous_lod = 0;

        let output =
            simulate_compute_culling_cpu_reference(&[candidate], &positive_x_plane(), false, 0.05);

        assert_eq!(output.selected_lods[&9], 0);
    }

    #[test]
    fn gpu_policy_uses_cpu_for_small_worlds() {
        let path = FunComputeCullingPolicy::default().decide(FunComputeCullingDecisionInput {
            candidate_count: 128,
            static_dynamic_buffers_resident: true,
            draw_calls_before: 80,
            draw_calls_after: 20,
            expected_visibility_ratio_per_mille: 250,
            ..Default::default()
        });

        assert_eq!(path, FunCullingExecutionPath::Cpu);
    }

    #[test]
    fn gpu_policy_selects_hiz_when_amortized() {
        let path = FunComputeCullingPolicy::default().decide(FunComputeCullingDecisionInput {
            candidate_count: 16_384,
            static_dynamic_buffers_resident: true,
            draw_calls_before: 500,
            draw_calls_after: 50,
            expected_visibility_ratio_per_mille: 250,
            depth_pyramid_available: true,
            occluder_density_per_mille: 500,
            p95_net_effect_ns: Some(-1_000_000),
            ..Default::default()
        });

        assert_eq!(path, FunCullingExecutionPath::GpuWithHiZ);
    }

    #[test]
    fn worse_p95_keeps_gpu_culling_behind_diagnostics() {
        let path = FunComputeCullingPolicy::default().decide(FunComputeCullingDecisionInput {
            candidate_count: 16_384,
            static_dynamic_buffers_resident: true,
            draw_calls_before: 500,
            draw_calls_after: 50,
            expected_visibility_ratio_per_mille: 250,
            p95_net_effect_ns: Some(500_000),
            ..Default::default()
        });

        assert_eq!(path, FunCullingExecutionPath::DiagnosticsOnly);
    }

    #[test]
    fn benchmark_records_cost_reduction_and_p95_effect() {
        let sample = FunComputeCullingBenchmarkSample {
            culling_pass_cost_ns: 400_000,
            draw_calls_before: 240,
            draw_calls_after: 40,
            cpu_submission_saved_ns: 1_100_000,
            p95_net_effect_ns: -700_000,
        };
        let report = FunComputeCullingFrameReport::from_benchmark_sample(
            17,
            FunCullingExecutionPath::Gpu,
            sample,
        );

        assert_eq!(report.draw_call_reduction, 200);
        assert_eq!(report.culling_pass_cost_ns, 400_000);
        assert_eq!(report.p95_net_effect_ns, -700_000);
        assert!(!report.keep_behind_diagnostics);
    }

    #[test]
    fn shader_declares_all_compute_stage_entry_points() {
        let shader = include_str!("compute_culling.wgsl");
        for entry_point in [
            "fn reset_culling_state",
            "fn instance_frustum_cull",
            "fn lod_select",
            "fn meshlet_cluster_cull",
            "fn hiz_occlusion_cull",
            "fn compact_visible_instances",
            "fn generate_indirect_args",
        ] {
            assert!(
                shader.contains(entry_point),
                "missing compute culling shader entry point: {entry_point}"
            );
        }
    }

    #[test]
    fn shader_read_only_storage_inputs_match_pipeline_layout_contract() {
        let shader = include_str!("compute_culling.wgsl");

        assert!(shader.contains("@group(0) @binding(1) var<storage, read> instances"));
        assert!(shader.contains("@group(0) @binding(2) var<storage, read> clusters"));
        assert!(CULLING_INSTANCE_METADATA_READ_ONLY);
        assert!(CULLING_MESHLET_CLUSTER_METADATA_READ_ONLY);
        assert!(CULLING_HIZ_READ_ONLY);
        assert!(!CULLING_VIEW_CONSTANTS_READ_ONLY);
        assert!(!CULLING_VISIBILITY_FLAGS_READ_ONLY);
        assert!(!CULLING_COMPACT_IDS_READ_ONLY);
        assert!(!CULLING_INDIRECT_ARGS_READ_ONLY);
        assert!(!CULLING_COUNTERS_READ_ONLY);
        assert!(!CULLING_BINDINGS_HAVE_DYNAMIC_OFFSET);
    }
}
