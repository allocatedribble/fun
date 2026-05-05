use std::collections::{BTreeMap, BTreeSet};

use crate::{FunRenderPath, FunRenderPhaseKind, compute_culling::FunCullingCpuReferenceOutput};

pub const FUN_INDIRECT_DRAW_SCHEMA_VERSION: u16 = 1;

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct FunMaterialSignature(pub u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunShaderPipelineSignature(pub u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunMeshletFormat {
    #[default]
    RasterMesh,
    MeshletMesh,
    ClusterMesh,
    ParticleBillboard,
    UiQuad,
    DebugLine,
}

impl FunMeshletFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RasterMesh => "raster_mesh",
            Self::MeshletMesh => "meshlet_mesh",
            Self::ClusterMesh => "cluster_mesh",
            Self::ParticleBillboard => "particle_billboard",
            Self::UiQuad => "ui_quad",
            Self::DebugLine => "debug_line",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunTextureTableCompatibility {
    #[default]
    SharedTable,
    BindlessCompatible,
    SameAtlas,
    UniqueTable,
    Incompatible,
}

impl FunTextureTableCompatibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SharedTable => "shared_table",
            Self::BindlessCompatible => "bindless_compatible",
            Self::SameAtlas => "same_atlas",
            Self::UniqueTable => "unique_table",
            Self::Incompatible => "incompatible",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunDepthPrepassMode {
    None,
    #[default]
    DepthOnly,
    DepthAndMotion,
    ShadowDepth,
}

impl FunDepthPrepassMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DepthOnly => "depth_only",
            Self::DepthAndMotion => "depth_and_motion",
            Self::ShadowDepth => "shadow_depth",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunTransparencyMode {
    #[default]
    Opaque,
    AlphaClip,
    AlphaBlend,
    Additive,
    Particle,
}

impl FunTransparencyMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::AlphaClip => "alpha_clip",
            Self::AlphaBlend => "alpha_blend",
            Self::Additive => "additive",
            Self::Particle => "particle",
        }
    }

    pub const fn requires_direct_fallback(self) -> bool {
        matches!(self, Self::AlphaBlend | Self::Additive | Self::Particle)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunSkeletalMode {
    #[default]
    Static,
    Skinned,
    Viewmodel,
}

impl FunSkeletalMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Skinned => "skinned",
            Self::Viewmodel => "viewmodel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunDrawBucketKey {
    pub phase: FunRenderPhaseKind,
    pub material_signature: FunMaterialSignature,
    pub shader_pipeline_signature: FunShaderPipelineSignature,
    pub meshlet_format: FunMeshletFormat,
    pub texture_table: FunTextureTableCompatibility,
    pub depth_prepass: FunDepthPrepassMode,
    pub transparency: FunTransparencyMode,
    pub skeletal: FunSkeletalMode,
    pub render_path: FunRenderPath,
}

impl FunDrawBucketKey {
    pub const fn opaque_static(
        material_signature: FunMaterialSignature,
        shader_pipeline_signature: FunShaderPipelineSignature,
        meshlet_format: FunMeshletFormat,
        render_path: FunRenderPath,
    ) -> Self {
        Self {
            phase: FunRenderPhaseKind::MainOpaque,
            material_signature,
            shader_pipeline_signature,
            meshlet_format,
            texture_table: FunTextureTableCompatibility::SharedTable,
            depth_prepass: FunDepthPrepassMode::DepthOnly,
            transparency: FunTransparencyMode::Opaque,
            skeletal: FunSkeletalMode::Static,
            render_path,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunMultiDrawGroupKey {
    pub phase: FunRenderPhaseKind,
    pub shader_pipeline_signature: FunShaderPipelineSignature,
    pub meshlet_format: FunMeshletFormat,
    pub texture_table: FunTextureTableCompatibility,
    pub depth_prepass: FunDepthPrepassMode,
    pub transparency: FunTransparencyMode,
    pub skeletal: FunSkeletalMode,
    pub render_path: FunRenderPath,
}

impl From<FunDrawBucketKey> for FunMultiDrawGroupKey {
    fn from(key: FunDrawBucketKey) -> Self {
        Self {
            phase: key.phase,
            shader_pipeline_signature: key.shader_pipeline_signature,
            meshlet_format: key.meshlet_format,
            texture_table: key.texture_table,
            depth_prepass: key.depth_prepass,
            transparency: key.transparency,
            skeletal: key.skeletal,
            render_path: key.render_path,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FunDrawSubmissionPath {
    #[default]
    Direct,
    Indirect,
    MultiDrawIndirect,
}

impl FunDrawSubmissionPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Indirect => "indirect",
            Self::MultiDrawIndirect => "multi_draw_indirect",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunDrawFallbackReason {
    UnsupportedBackend,
    DebugRendering,
    TinyScene,
    Transparent,
    Viewmodel,
    EditorOverlay,
}

impl FunDrawFallbackReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedBackend => "unsupported_backend",
            Self::DebugRendering => "debug_rendering",
            Self::TinyScene => "tiny_scene",
            Self::Transparent => "transparent",
            Self::Viewmodel => "viewmodel",
            Self::EditorOverlay => "editor_overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunDrawSubmissionCapabilities {
    pub backend_supported: bool,
    pub indirect_supported: bool,
    pub multi_draw_indirect_supported: bool,
}

impl Default for FunDrawSubmissionCapabilities {
    fn default() -> Self {
        Self {
            backend_supported: true,
            indirect_supported: true,
            multi_draw_indirect_supported: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunDrawBudgetLane {
    PresentationFloor,
    FullRuntime,
    StreamingSpike,
    SolariCloudHeavy,
    Competitive5v5,
    LargeBattle,
}

impl FunDrawBudgetLane {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PresentationFloor => "presentation_floor",
            Self::FullRuntime => "full_runtime",
            Self::StreamingSpike => "streaming_spike",
            Self::SolariCloudHeavy => "solari_cloud_heavy",
            Self::Competitive5v5 => "competitive_5v5",
            Self::LargeBattle => "large_battle",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunDrawCallBudget {
    pub lane: FunDrawBudgetLane,
    pub static_opaque_bucket_draw_target: u32,
    pub instanced_prop_bucket_draw_target: u32,
    pub max_direct_draws: u32,
    pub max_indirect_submit_calls: u32,
    pub max_multi_draw_submit_calls: u32,
    pub min_instances_per_indirect_draw: u32,
    pub material_diversity_warning_threshold: u32,
    pub transparent_report_only: bool,
}

pub const FUN_DRAW_CALL_BUDGETS: &[FunDrawCallBudget] = &[
    FunDrawCallBudget {
        lane: FunDrawBudgetLane::PresentationFloor,
        static_opaque_bucket_draw_target: 16,
        instanced_prop_bucket_draw_target: 12,
        max_direct_draws: 64,
        max_indirect_submit_calls: 32,
        max_multi_draw_submit_calls: 8,
        min_instances_per_indirect_draw: 8,
        material_diversity_warning_threshold: 12,
        transparent_report_only: true,
    },
    FunDrawCallBudget {
        lane: FunDrawBudgetLane::FullRuntime,
        static_opaque_bucket_draw_target: 64,
        instanced_prop_bucket_draw_target: 48,
        max_direct_draws: 192,
        max_indirect_submit_calls: 96,
        max_multi_draw_submit_calls: 24,
        min_instances_per_indirect_draw: 16,
        material_diversity_warning_threshold: 32,
        transparent_report_only: true,
    },
    FunDrawCallBudget {
        lane: FunDrawBudgetLane::StreamingSpike,
        static_opaque_bucket_draw_target: 96,
        instanced_prop_bucket_draw_target: 64,
        max_direct_draws: 384,
        max_indirect_submit_calls: 128,
        max_multi_draw_submit_calls: 32,
        min_instances_per_indirect_draw: 12,
        material_diversity_warning_threshold: 48,
        transparent_report_only: true,
    },
    FunDrawCallBudget {
        lane: FunDrawBudgetLane::SolariCloudHeavy,
        static_opaque_bucket_draw_target: 48,
        instanced_prop_bucket_draw_target: 32,
        max_direct_draws: 128,
        max_indirect_submit_calls: 64,
        max_multi_draw_submit_calls: 16,
        min_instances_per_indirect_draw: 24,
        material_diversity_warning_threshold: 24,
        transparent_report_only: true,
    },
    FunDrawCallBudget {
        lane: FunDrawBudgetLane::Competitive5v5,
        static_opaque_bucket_draw_target: 40,
        instanced_prop_bucket_draw_target: 32,
        max_direct_draws: 160,
        max_indirect_submit_calls: 72,
        max_multi_draw_submit_calls: 18,
        min_instances_per_indirect_draw: 8,
        material_diversity_warning_threshold: 28,
        transparent_report_only: true,
    },
    FunDrawCallBudget {
        lane: FunDrawBudgetLane::LargeBattle,
        static_opaque_bucket_draw_target: 128,
        instanced_prop_bucket_draw_target: 96,
        max_direct_draws: 512,
        max_indirect_submit_calls: 192,
        max_multi_draw_submit_calls: 48,
        min_instances_per_indirect_draw: 32,
        material_diversity_warning_threshold: 64,
        transparent_report_only: true,
    },
];

pub const fn draw_call_budget_for_lane(lane: FunDrawBudgetLane) -> FunDrawCallBudget {
    match lane {
        FunDrawBudgetLane::PresentationFloor => FUN_DRAW_CALL_BUDGETS[0],
        FunDrawBudgetLane::FullRuntime => FUN_DRAW_CALL_BUDGETS[1],
        FunDrawBudgetLane::StreamingSpike => FUN_DRAW_CALL_BUDGETS[2],
        FunDrawBudgetLane::SolariCloudHeavy => FUN_DRAW_CALL_BUDGETS[3],
        FunDrawBudgetLane::Competitive5v5 => FUN_DRAW_CALL_BUDGETS[4],
        FunDrawBudgetLane::LargeBattle => FUN_DRAW_CALL_BUDGETS[5],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunDrawPacketBuildOptions {
    pub frame_index: u64,
    pub lane: FunDrawBudgetLane,
    pub capabilities: FunDrawSubmissionCapabilities,
    pub tiny_scene_instance_threshold: u32,
    pub multi_draw_min_buckets: u32,
    pub debug_rendering: bool,
    pub editor_overlay: bool,
}

impl Default for FunDrawPacketBuildOptions {
    fn default() -> Self {
        Self {
            frame_index: 0,
            lane: FunDrawBudgetLane::FullRuntime,
            capabilities: FunDrawSubmissionCapabilities::default(),
            tiny_scene_instance_threshold: 8,
            multi_draw_min_buckets: 2,
            debug_rendering: false,
            editor_overlay: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunDrawPacketSource {
    pub instance_id: u32,
    pub bucket_key: FunDrawBucketKey,
    pub meshlet_count: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunIndirectArgsRange {
    pub first_arg_index: u32,
    pub arg_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunIndirectArgsBuffer {
    pub frame_index: u64,
    pub capacity_args: u32,
    pub used_args: u32,
    pub ranges: Vec<FunIndirectArgsRange>,
}

impl FunIndirectArgsBuffer {
    pub const fn new(frame_index: u64, capacity_args: u32) -> Self {
        Self {
            frame_index,
            capacity_args,
            used_args: 0,
            ranges: Vec::new(),
        }
    }

    pub fn allocate_range(&mut self, arg_count: u32) -> Option<FunIndirectArgsRange> {
        if self.used_args.saturating_add(arg_count) > self.capacity_args {
            return None;
        }
        let range = FunIndirectArgsRange {
            first_arg_index: self.used_args,
            arg_count,
        };
        self.used_args = self.used_args.saturating_add(arg_count);
        self.ranges.push(range);
        Some(range)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunDrawBucket {
    pub key: FunDrawBucketKey,
    pub submission_path: FunDrawSubmissionPath,
    pub fallback_reason: Option<FunDrawFallbackReason>,
    pub compact_visible_offset: u32,
    pub compact_visible_count: u32,
    pub visible_instances: u32,
    pub visible_meshlets: u32,
    pub indirect_args_range: Option<FunIndirectArgsRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunIndirectDrawPacket {
    pub bucket_key: FunDrawBucketKey,
    pub submission_path: FunDrawSubmissionPath,
    pub compact_visible_offset: u32,
    pub compact_visible_count: u32,
    pub visible_instances: u32,
    pub visible_meshlets: u32,
    pub indirect_args_range: FunIndirectArgsRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunDrawBucketOffender {
    pub key: FunDrawBucketKey,
    pub visible_instances: u32,
    pub visible_meshlets: u32,
    pub direct_draws: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunDrawPacketReport {
    pub frame_index: u64,
    pub lane: FunDrawBudgetLane,
    pub direct_draw_count: u32,
    pub indirect_draw_count: u32,
    pub multi_draw_count: u32,
    pub indirect_args_count: u32,
    pub bucket_count: u32,
    pub visible_instance_count: u32,
    pub submitted_instance_count: u32,
    pub average_instances_per_draw: f32,
    pub draw_call_savings_vs_direct: u32,
    pub budget_exceeded: bool,
    pub material_diversity_bottleneck: bool,
    pub worst_bucket_offender: Option<FunDrawBucketOffender>,
    pub fallback_draws_by_reason: BTreeMap<FunDrawFallbackReason, u32>,
    pub indirect_args_buffer: FunIndirectArgsBuffer,
    pub buckets: Vec<FunDrawBucket>,
    pub packets: Vec<FunIndirectDrawPacket>,
}

pub fn build_draw_packets_from_compacted_ids(
    sources: &[FunDrawPacketSource],
    compacted_visible_ids: &[u32],
    options: FunDrawPacketBuildOptions,
) -> FunDrawPacketReport {
    let budget = draw_call_budget_for_lane(options.lane);
    let mut source_by_instance = BTreeMap::new();
    for source in sources {
        source_by_instance.insert(source.instance_id, *source);
    }

    let mut grouped = BTreeMap::<FunDrawBucketKey, BucketBuildState>::new();
    for (compact_index, instance_id) in compacted_visible_ids.iter().copied().enumerate() {
        let Some(source) = source_by_instance.get(&instance_id).copied() else {
            continue;
        };
        let group = grouped
            .entry(source.bucket_key)
            .or_insert(BucketBuildState {
                first_compact_index: compact_index as u32,
                visible_instances: 0,
                visible_meshlets: 0,
            });
        group.visible_instances = group.visible_instances.saturating_add(1);
        group.visible_meshlets = group.visible_meshlets.saturating_add(source.meshlet_count);
    }

    let visible_instance_count = grouped
        .values()
        .map(|bucket| bucket.visible_instances)
        .sum::<u32>();
    let mut indirect_args_buffer =
        FunIndirectArgsBuffer::new(options.frame_index, grouped.len() as u32);
    let use_multi_draw = options.capabilities.multi_draw_indirect_supported
        && grouped.len() as u32 >= options.multi_draw_min_buckets;
    let multi_draw_groups = if use_multi_draw {
        grouped
            .keys()
            .copied()
            .filter(|key| direct_fallback_reason(*key, visible_instance_count, options).is_none())
            .map(FunMultiDrawGroupKey::from)
            .collect::<BTreeSet<_>>()
            .len() as u32
    } else {
        0
    };

    let mut direct_draw_count = 0_u32;
    let mut indirect_draw_count = 0_u32;
    let mut indirect_args_count = 0_u32;
    let mut fallback_draws_by_reason = BTreeMap::<FunDrawFallbackReason, u32>::new();
    let mut buckets = Vec::with_capacity(grouped.len());
    let mut packets = Vec::new();

    for (key, state) in grouped {
        let fallback_reason = direct_fallback_reason(key, visible_instance_count, options);
        let (submission_path, indirect_args_range) = if let Some(reason) = fallback_reason {
            direct_draw_count = direct_draw_count.saturating_add(state.visible_instances);
            let count = fallback_draws_by_reason.entry(reason).or_default();
            *count = count.saturating_add(state.visible_instances);
            (FunDrawSubmissionPath::Direct, None)
        } else {
            let range = indirect_args_buffer
                .allocate_range(1)
                .expect("args buffer is sized from bucket count");
            indirect_args_count = indirect_args_count.saturating_add(1);
            let submission_path = if use_multi_draw {
                FunDrawSubmissionPath::MultiDrawIndirect
            } else {
                indirect_draw_count = indirect_draw_count.saturating_add(1);
                FunDrawSubmissionPath::Indirect
            };
            packets.push(FunIndirectDrawPacket {
                bucket_key: key,
                submission_path,
                compact_visible_offset: state.first_compact_index,
                compact_visible_count: state.visible_instances,
                visible_instances: state.visible_instances,
                visible_meshlets: state.visible_meshlets,
                indirect_args_range: range,
            });
            (submission_path, Some(range))
        };

        buckets.push(FunDrawBucket {
            key,
            submission_path,
            fallback_reason,
            compact_visible_offset: state.first_compact_index,
            compact_visible_count: state.visible_instances,
            visible_instances: state.visible_instances,
            visible_meshlets: state.visible_meshlets,
            indirect_args_range,
        });
    }

    let multi_draw_count = multi_draw_groups;
    let submission_draw_count = direct_draw_count
        .saturating_add(indirect_draw_count)
        .saturating_add(multi_draw_count);
    let average_instances_per_draw = if submission_draw_count == 0 {
        0.0
    } else {
        visible_instance_count as f32 / submission_draw_count as f32
    };
    let worst_bucket_offender = buckets
        .iter()
        .max_by_key(|bucket| {
            (
                bucket.visible_instances,
                bucket.visible_meshlets,
                bucket.key.material_signature,
            )
        })
        .map(|bucket| FunDrawBucketOffender {
            key: bucket.key,
            visible_instances: bucket.visible_instances,
            visible_meshlets: bucket.visible_meshlets,
            direct_draws: if bucket.submission_path == FunDrawSubmissionPath::Direct {
                bucket.visible_instances
            } else {
                0
            },
        });
    let unique_material_count = buckets
        .iter()
        .map(|bucket| bucket.key.material_signature)
        .collect::<BTreeSet<_>>()
        .len() as u32;
    let material_diversity_bottleneck = unique_material_count
        >= budget.material_diversity_warning_threshold
        || (unique_material_count == buckets.len() as u32
            && buckets.len() > 1
            && average_instances_per_draw <= 1.5);
    let budget_exceeded = direct_draw_count > budget.max_direct_draws
        || indirect_draw_count > budget.max_indirect_submit_calls
        || multi_draw_count > budget.max_multi_draw_submit_calls;

    FunDrawPacketReport {
        frame_index: options.frame_index,
        lane: options.lane,
        direct_draw_count,
        indirect_draw_count,
        multi_draw_count,
        indirect_args_count,
        bucket_count: buckets.len() as u32,
        visible_instance_count,
        submitted_instance_count: compacted_visible_ids.len() as u32,
        average_instances_per_draw,
        draw_call_savings_vs_direct: visible_instance_count.saturating_sub(submission_draw_count),
        budget_exceeded,
        material_diversity_bottleneck,
        worst_bucket_offender,
        fallback_draws_by_reason,
        indirect_args_buffer,
        buckets,
        packets,
    }
}

pub fn build_draw_packets_from_culling_output(
    sources: &[FunDrawPacketSource],
    culling_output: &FunCullingCpuReferenceOutput,
    options: FunDrawPacketBuildOptions,
) -> FunDrawPacketReport {
    build_draw_packets_from_compacted_ids(sources, &culling_output.visible_instance_ids, options)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunDrawSubmissionSummary {
    pub direct_submit_calls: u32,
    pub indirect_submit_calls: u32,
    pub multi_draw_submit_calls: u32,
    pub indirect_args_submitted: u32,
    pub instances_submitted: u32,
}

pub trait FunDrawSubmissionSink {
    fn submit_direct_bucket(&mut self, bucket: &FunDrawBucket);
    fn submit_indirect_packet(&mut self, packet: &FunIndirectDrawPacket);
    fn submit_multi_draw_group(
        &mut self,
        group_key: FunMultiDrawGroupKey,
        packets: &[FunIndirectDrawPacket],
    );
}

pub fn submit_draw_packet_report<S>(
    report: &FunDrawPacketReport,
    sink: &mut S,
) -> FunDrawSubmissionSummary
where
    S: FunDrawSubmissionSink,
{
    let mut summary = FunDrawSubmissionSummary::default();
    for bucket in &report.buckets {
        if bucket.submission_path == FunDrawSubmissionPath::Direct {
            sink.submit_direct_bucket(bucket);
            summary.direct_submit_calls = summary.direct_submit_calls.saturating_add(1);
            summary.instances_submitted = summary
                .instances_submitted
                .saturating_add(bucket.visible_instances);
        }
    }

    let mut multi_draw_groups = BTreeMap::<FunMultiDrawGroupKey, Vec<FunIndirectDrawPacket>>::new();
    for packet in &report.packets {
        match packet.submission_path {
            FunDrawSubmissionPath::Direct => {}
            FunDrawSubmissionPath::Indirect => {
                sink.submit_indirect_packet(packet);
                summary.indirect_submit_calls = summary.indirect_submit_calls.saturating_add(1);
                summary.indirect_args_submitted = summary
                    .indirect_args_submitted
                    .saturating_add(packet.indirect_args_range.arg_count);
                summary.instances_submitted = summary
                    .instances_submitted
                    .saturating_add(packet.visible_instances);
            }
            FunDrawSubmissionPath::MultiDrawIndirect => {
                multi_draw_groups
                    .entry(FunMultiDrawGroupKey::from(packet.bucket_key))
                    .or_default()
                    .push(packet.clone());
            }
        }
    }

    for (group_key, packets) in multi_draw_groups {
        sink.submit_multi_draw_group(group_key, &packets);
        summary.multi_draw_submit_calls = summary.multi_draw_submit_calls.saturating_add(1);
        summary.indirect_args_submitted = summary.indirect_args_submitted.saturating_add(
            packets
                .iter()
                .map(|packet| packet.indirect_args_range.arg_count)
                .sum(),
        );
        summary.instances_submitted = summary
            .instances_submitted
            .saturating_add(packets.iter().map(|packet| packet.visible_instances).sum());
    }

    summary
}

#[derive(Debug, Clone, Copy)]
struct BucketBuildState {
    first_compact_index: u32,
    visible_instances: u32,
    visible_meshlets: u32,
}

fn direct_fallback_reason(
    key: FunDrawBucketKey,
    total_visible_instances: u32,
    options: FunDrawPacketBuildOptions,
) -> Option<FunDrawFallbackReason> {
    if !options.capabilities.backend_supported || !options.capabilities.indirect_supported {
        return Some(FunDrawFallbackReason::UnsupportedBackend);
    }
    if options.debug_rendering || key.phase == FunRenderPhaseKind::DebugOverlay {
        return Some(FunDrawFallbackReason::DebugRendering);
    }
    if options.editor_overlay {
        return Some(FunDrawFallbackReason::EditorOverlay);
    }
    if total_visible_instances <= options.tiny_scene_instance_threshold {
        return Some(FunDrawFallbackReason::TinyScene);
    }
    if key.transparency.requires_direct_fallback() || key.phase == FunRenderPhaseKind::Transparent {
        return Some(FunDrawFallbackReason::Transparent);
    }
    if key.skeletal == FunSkeletalMode::Viewmodel || key.render_path == FunRenderPath::Viewmodel {
        return Some(FunDrawFallbackReason::Viewmodel);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque_key(material: u64) -> FunDrawBucketKey {
        FunDrawBucketKey::opaque_static(
            FunMaterialSignature(material),
            FunShaderPipelineSignature(7),
            FunMeshletFormat::RasterMesh,
            FunRenderPath::GpuCulledIndirect,
        )
    }

    fn source(instance_id: u32, key: FunDrawBucketKey) -> FunDrawPacketSource {
        FunDrawPacketSource {
            instance_id,
            bucket_key: key,
            meshlet_count: 1,
        }
    }

    fn indirect_options() -> FunDrawPacketBuildOptions {
        FunDrawPacketBuildOptions {
            tiny_scene_instance_threshold: 0,
            ..Default::default()
        }
    }

    #[test]
    fn identical_props_collapse_to_one_indirect_bucket() {
        let key = opaque_key(1);
        let sources = (0..128).map(|id| source(id, key)).collect::<Vec<_>>();
        let visible = (0..128).collect::<Vec<_>>();

        let report = build_draw_packets_from_compacted_ids(&sources, &visible, indirect_options());

        assert_eq!(report.bucket_count, 1);
        assert_eq!(report.indirect_draw_count, 1);
        assert_eq!(report.direct_draw_count, 0);
        assert_eq!(report.average_instances_per_draw, 128.0);
        assert_eq!(report.draw_call_savings_vs_direct, 127);
    }

    #[test]
    fn identical_prop_draw_count_stays_flat_as_count_rises() {
        let key = opaque_key(2);
        let one_source = [source(0, key)];
        let one_report =
            build_draw_packets_from_compacted_ids(&one_source, &[0], indirect_options());
        let many_sources = (0..64).map(|id| source(id, key)).collect::<Vec<_>>();
        let many_visible = (0..64).collect::<Vec<_>>();
        let many_report =
            build_draw_packets_from_compacted_ids(&many_sources, &many_visible, indirect_options());

        assert_eq!(one_report.indirect_draw_count, 1);
        assert_eq!(many_report.indirect_draw_count, 1);
        assert!(many_report.draw_call_savings_vs_direct > one_report.draw_call_savings_vs_direct);
    }

    #[test]
    fn unique_materials_report_material_diversity_bottleneck() {
        let sources = (0..40)
            .map(|id| source(id, opaque_key(u64::from(id) + 10)))
            .collect::<Vec<_>>();
        let visible = (0..40).collect::<Vec<_>>();

        let report = build_draw_packets_from_compacted_ids(&sources, &visible, indirect_options());

        assert_eq!(report.bucket_count, 40);
        assert!(report.material_diversity_bottleneck);
        assert!(report.average_instances_per_draw <= 1.5);
    }

    #[test]
    fn transparent_and_viewmodel_paths_use_direct_fallback() {
        let mut transparent = opaque_key(3);
        transparent.phase = FunRenderPhaseKind::Transparent;
        transparent.transparency = FunTransparencyMode::AlphaBlend;
        let mut viewmodel = opaque_key(4);
        viewmodel.skeletal = FunSkeletalMode::Viewmodel;
        viewmodel.render_path = FunRenderPath::Viewmodel;
        let sources = [source(0, transparent), source(1, viewmodel)];

        let report = build_draw_packets_from_compacted_ids(&sources, &[0, 1], indirect_options());

        assert_eq!(report.direct_draw_count, 2);
        assert_eq!(
            report.fallback_draws_by_reason[&FunDrawFallbackReason::Transparent],
            1
        );
        assert_eq!(
            report.fallback_draws_by_reason[&FunDrawFallbackReason::Viewmodel],
            1
        );
    }

    #[test]
    fn unsupported_backend_uses_direct_fallback() {
        let key = opaque_key(5);
        let sources = (0..16).map(|id| source(id, key)).collect::<Vec<_>>();
        let visible = (0..16).collect::<Vec<_>>();
        let report = build_draw_packets_from_compacted_ids(
            &sources,
            &visible,
            FunDrawPacketBuildOptions {
                capabilities: FunDrawSubmissionCapabilities {
                    backend_supported: false,
                    indirect_supported: false,
                    multi_draw_indirect_supported: false,
                },
                tiny_scene_instance_threshold: 0,
                ..Default::default()
            },
        );

        assert_eq!(report.direct_draw_count, 16);
        assert_eq!(report.indirect_draw_count, 0);
        assert_eq!(
            report.fallback_draws_by_reason[&FunDrawFallbackReason::UnsupportedBackend],
            16
        );
    }

    #[test]
    fn multi_draw_groups_compatible_buckets() {
        let sources = (0..4)
            .map(|id| source(id, opaque_key(u64::from(id) + 1)))
            .collect::<Vec<_>>();
        let visible = (0..4).collect::<Vec<_>>();

        let report = build_draw_packets_from_compacted_ids(
            &sources,
            &visible,
            FunDrawPacketBuildOptions {
                capabilities: FunDrawSubmissionCapabilities {
                    multi_draw_indirect_supported: true,
                    ..Default::default()
                },
                tiny_scene_instance_threshold: 0,
                multi_draw_min_buckets: 2,
                ..Default::default()
            },
        );

        assert_eq!(report.bucket_count, 4);
        assert_eq!(report.indirect_draw_count, 0);
        assert_eq!(report.multi_draw_count, 1);
        assert_eq!(report.indirect_args_count, 4);
        assert!(
            report
                .packets
                .iter()
                .all(|packet| packet.submission_path == FunDrawSubmissionPath::MultiDrawIndirect)
        );
    }

    #[test]
    fn draw_budgets_are_lane_specific() {
        let presentation = draw_call_budget_for_lane(FunDrawBudgetLane::PresentationFloor);
        let large_battle = draw_call_budget_for_lane(FunDrawBudgetLane::LargeBattle);

        assert_ne!(
            presentation.static_opaque_bucket_draw_target,
            large_battle.static_opaque_bucket_draw_target
        );
        assert_ne!(
            presentation.min_instances_per_indirect_draw,
            large_battle.min_instances_per_indirect_draw
        );
    }

    #[test]
    fn report_marks_lane_budget_exceeded_without_hiding_actual_count() {
        let visible = (0..20).collect::<Vec<_>>();
        let compatible_sources = (0..20)
            .map(|id| source(id, opaque_key(u64::from(id) + 1)))
            .collect::<Vec<_>>();

        let compatible_report = build_draw_packets_from_compacted_ids(
            &compatible_sources,
            &visible,
            FunDrawPacketBuildOptions {
                lane: FunDrawBudgetLane::PresentationFloor,
                capabilities: FunDrawSubmissionCapabilities {
                    multi_draw_indirect_supported: true,
                    ..Default::default()
                },
                tiny_scene_instance_threshold: 0,
                multi_draw_min_buckets: 1,
                ..Default::default()
            },
        );
        assert_eq!(compatible_report.multi_draw_count, 1);
        assert!(!compatible_report.budget_exceeded);

        let incompatible_sources = (0..20)
            .map(|id| {
                let mut key = opaque_key(u64::from(id) + 1);
                key.shader_pipeline_signature = FunShaderPipelineSignature(u64::from(id) + 100);
                source(id, key)
            })
            .collect::<Vec<_>>();
        let incompatible_report = build_draw_packets_from_compacted_ids(
            &incompatible_sources,
            &visible,
            FunDrawPacketBuildOptions {
                lane: FunDrawBudgetLane::PresentationFloor,
                capabilities: FunDrawSubmissionCapabilities {
                    multi_draw_indirect_supported: true,
                    ..Default::default()
                },
                tiny_scene_instance_threshold: 0,
                multi_draw_min_buckets: 1,
                ..Default::default()
            },
        );

        assert_eq!(incompatible_report.multi_draw_count, 20);
        assert!(incompatible_report.budget_exceeded);
    }

    #[test]
    fn indirect_args_buffer_allocates_contiguous_ranges() {
        let mut buffer = FunIndirectArgsBuffer::new(9, 3);

        assert_eq!(
            buffer.allocate_range(2),
            Some(FunIndirectArgsRange {
                first_arg_index: 0,
                arg_count: 2
            })
        );
        assert_eq!(
            buffer.allocate_range(1),
            Some(FunIndirectArgsRange {
                first_arg_index: 2,
                arg_count: 1
            })
        );
        assert_eq!(buffer.allocate_range(1), None);
    }

    #[test]
    fn compute_compaction_output_fills_draw_packets() {
        let key = opaque_key(9);
        let sources = [source(42, key)];
        let culling_output = FunCullingCpuReferenceOutput {
            visible_instance_ids: vec![42],
            selected_lods: BTreeMap::new(),
            visible_meshlet_ids: vec![0, 1, 2],
            draw_args: Vec::new(),
            dispatch_args: Vec::new(),
        };

        let report =
            build_draw_packets_from_culling_output(&sources, &culling_output, indirect_options());

        assert_eq!(report.bucket_count, 1);
        assert_eq!(report.packets[0].bucket_key, key);
        assert_eq!(report.packets[0].visible_instances, 1);
    }

    #[derive(Default)]
    struct RecordingSubmissionSink {
        direct: u32,
        indirect: u32,
        multi_draw: u32,
    }

    impl FunDrawSubmissionSink for RecordingSubmissionSink {
        fn submit_direct_bucket(&mut self, _bucket: &FunDrawBucket) {
            self.direct = self.direct.saturating_add(1);
        }

        fn submit_indirect_packet(&mut self, _packet: &FunIndirectDrawPacket) {
            self.indirect = self.indirect.saturating_add(1);
        }

        fn submit_multi_draw_group(
            &mut self,
            _group_key: FunMultiDrawGroupKey,
            _packets: &[FunIndirectDrawPacket],
        ) {
            self.multi_draw = self.multi_draw.saturating_add(1);
        }
    }

    #[test]
    fn submission_sink_gets_one_indirect_submit_for_one_large_bucket() {
        let key = opaque_key(10);
        let sources = (0..32).map(|id| source(id, key)).collect::<Vec<_>>();
        let visible = (0..32).collect::<Vec<_>>();
        let report = build_draw_packets_from_compacted_ids(&sources, &visible, indirect_options());
        let mut sink = RecordingSubmissionSink::default();

        let summary = submit_draw_packet_report(&report, &mut sink);

        assert_eq!(sink.direct, 0);
        assert_eq!(sink.indirect, 1);
        assert_eq!(sink.multi_draw, 0);
        assert_eq!(summary.indirect_submit_calls, 1);
        assert_eq!(summary.instances_submitted, 32);
    }
}
