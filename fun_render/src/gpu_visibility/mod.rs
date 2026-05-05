use std::collections::BTreeMap;

use bevy::prelude::Resource;

use crate::{FunDrawPacketBuildOptions, FunDrawPacketReport};

pub mod buffers;
pub mod compact;
pub mod frustum_cull;
pub mod indirect;
pub mod lod;
pub mod types;

pub use buffers::{
    GPU_VISIBILITY_BUFFER_PLAN, GpuVisibilityBufferDescriptor, GpuVisibilityBufferKind,
    GpuVisibilityBufferLifetime, gpu_visibility_buffer_descriptor,
};
pub use compact::{
    GpuCompactionReport, compact_visible_instance_ids, compact_visible_object_ids,
    compact_visible_static_opaque,
};
pub use frustum_cull::{
    GpuFrustumCullOutput, GpuFrustumPlane, conservative_sphere_visible, frustum_cull_static_objects,
};
pub use indirect::{GpuIndirectBuildOutput, build_indirect_buckets_for_static_opaque};
pub use lod::{
    GpuLodPolicy, GpuLodSelection, select_lod_with_hysteresis, select_lods_for_visible_objects,
};
pub use types::{
    FUN_GPU_VISIBILITY_SCHEMA_VERSION, GPU_VIS_OBJECT_CEF_UI, GPU_VIS_OBJECT_DEBUG,
    GPU_VIS_OBJECT_FOLIAGE, GPU_VIS_OBJECT_MESHLET, GPU_VIS_OBJECT_OCCLUDER,
    GPU_VIS_OBJECT_PARTICLE, GPU_VIS_OBJECT_RASTER, GPU_VIS_OBJECT_SKINNED,
    GPU_VIS_OBJECT_STATIC_OPAQUE, GPU_VIS_OBJECT_TRANSPARENT, GPU_VIS_OBJECT_VIEWMODEL,
    GPU_VISIBILITY_STATIC_OPAQUE_STAGES, GPU_VISIBILITY_WORKGROUP_SIZE, GpuBoundsRecord,
    GpuDrawBucket, GpuInstanceRecord, GpuMaterialBucket, GpuMeshClusterRange, GpuObjectRecord,
    GpuVisibilityBatchId, GpuVisibilityCellId, GpuVisibilityObjectId, GpuVisibilityStage,
    GpuVisibilityViewConstants, StaticOpaqueVisibilityFallbackReason, StaticOpaqueVisibilityPath,
    draw_bucket_key_for_object, fun_render_path_from_code, gpu_render_path_code,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Resource)]
pub struct StaticOpaqueGpuVisibilityConfig {
    pub enabled: bool,
    pub policy: StaticOpaqueGpuVisibilityPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticOpaqueGpuVisibilityPolicy {
    pub gpu_static_instance_threshold: u32,
    pub gpu_dense_batch_threshold: u32,
    pub conservative_bounds_padding_mm: u32,
    pub lod_policy: GpuLodPolicy,
}

impl Default for StaticOpaqueGpuVisibilityPolicy {
    fn default() -> Self {
        Self {
            gpu_static_instance_threshold: 2_000,
            gpu_dense_batch_threshold: 256,
            conservative_bounds_padding_mm: 50,
            lod_policy: GpuLodPolicy::default(),
        }
    }
}

impl StaticOpaqueGpuVisibilityPolicy {
    pub fn conservative_bounds_padding_meters(self) -> f32 {
        self.conservative_bounds_padding_mm as f32 * 0.001
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StaticOpaqueVisibilityDecisionInput {
    pub static_instance_count: u32,
    pub dense_batch_count: u32,
    pub static_buffers_resident: bool,
    pub backend_supported: bool,
    pub debug_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticOpaqueVisibilityDecision {
    pub path: StaticOpaqueVisibilityPath,
    pub fallback_reason: Option<StaticOpaqueVisibilityFallbackReason>,
}

pub fn decide_static_opaque_visibility_path(
    policy: StaticOpaqueGpuVisibilityPolicy,
    input: StaticOpaqueVisibilityDecisionInput,
) -> StaticOpaqueVisibilityDecision {
    if !input.backend_supported {
        return StaticOpaqueVisibilityDecision {
            path: StaticOpaqueVisibilityPath::FallbackDirect,
            fallback_reason: Some(StaticOpaqueVisibilityFallbackReason::UnsupportedBackend),
        };
    }
    if input.debug_mode {
        return StaticOpaqueVisibilityDecision {
            path: StaticOpaqueVisibilityPath::FallbackDirect,
            fallback_reason: Some(StaticOpaqueVisibilityFallbackReason::DebugMode),
        };
    }
    if input.static_instance_count == 0 && input.dense_batch_count == 0 {
        return StaticOpaqueVisibilityDecision {
            path: StaticOpaqueVisibilityPath::CpuStaticCellCulling,
            fallback_reason: Some(StaticOpaqueVisibilityFallbackReason::EmptyScene),
        };
    }
    if !input.static_buffers_resident {
        return StaticOpaqueVisibilityDecision {
            path: StaticOpaqueVisibilityPath::CpuStaticCellCulling,
            fallback_reason: Some(StaticOpaqueVisibilityFallbackReason::BuffersNotResident),
        };
    }
    if input.static_instance_count >= policy.gpu_static_instance_threshold
        || input.dense_batch_count >= policy.gpu_dense_batch_threshold
    {
        return StaticOpaqueVisibilityDecision {
            path: StaticOpaqueVisibilityPath::GpuDriven,
            fallback_reason: None,
        };
    }

    StaticOpaqueVisibilityDecision {
        path: StaticOpaqueVisibilityPath::CpuStaticCellCulling,
        fallback_reason: Some(StaticOpaqueVisibilityFallbackReason::TinyScene),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticOpaqueGpuVisibilityFrame {
    pub decision: StaticOpaqueVisibilityDecision,
    pub cull: GpuFrustumCullOutput,
    pub lods: Vec<GpuLodSelection>,
    pub compacted: GpuCompactionReport,
    pub indirect: GpuIndirectBuildOutput,
}

pub fn build_static_opaque_gpu_visibility_frame(
    objects: &[GpuObjectRecord],
    planes: &[GpuFrustumPlane],
    projected_radius_px_by_object: &BTreeMap<GpuVisibilityObjectId, f32>,
    previous_lod_by_object: &BTreeMap<GpuVisibilityObjectId, u8>,
    decision_input: StaticOpaqueVisibilityDecisionInput,
    policy: StaticOpaqueGpuVisibilityPolicy,
    draw_options: FunDrawPacketBuildOptions,
) -> StaticOpaqueGpuVisibilityFrame {
    let decision = decide_static_opaque_visibility_path(policy, decision_input);
    let cull =
        frustum_cull_static_objects(objects, planes, policy.conservative_bounds_padding_meters());
    let lods = select_lods_for_visible_objects(
        objects,
        &cull.visible_object_ids,
        projected_radius_px_by_object,
        previous_lod_by_object,
        policy.lod_policy,
    );
    let compacted = compact_visible_static_opaque(objects, &cull.visible_object_ids);
    let indirect = build_indirect_buckets_for_static_opaque(
        objects,
        &cull.visible_object_ids,
        &compacted.visible_instance_ids,
        draw_options,
    );

    StaticOpaqueGpuVisibilityFrame {
        decision,
        cull,
        lods,
        compacted,
        indirect,
    }
}

pub fn draw_packet_report_for_static_visibility_frame(
    frame: &StaticOpaqueGpuVisibilityFrame,
) -> &FunDrawPacketReport {
    &frame.indirect.draw_packet_report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FunDrawBucketKey, FunDrawPacketBuildOptions, FunRenderPath};

    fn test_planes() -> [GpuFrustumPlane; 6] {
        [
            GpuFrustumPlane::new([1.0, 0.0, 0.0], 10.0),
            GpuFrustumPlane::new([-1.0, 0.0, 0.0], 10.0),
            GpuFrustumPlane::new([0.0, 1.0, 0.0], 10.0),
            GpuFrustumPlane::new([0.0, -1.0, 0.0], 10.0),
            GpuFrustumPlane::new([0.0, 0.0, 1.0], 10.0),
            GpuFrustumPlane::new([0.0, 0.0, -1.0], 10.0),
        ]
    }

    fn object(
        object_id: u32,
        material_bucket: u32,
        mesh_start: u32,
        instance_start: u32,
        instance_count: u32,
        flags: u32,
        center_x: f32,
    ) -> GpuObjectRecord {
        GpuObjectRecord {
            ids_flags: [object_id, 1, object_id, flags],
            bounds_center_radius: [center_x, 0.0, 0.0, 1.0],
            instance_range_path: [
                instance_start,
                instance_count,
                gpu_render_path_code(FunRenderPath::GpuCulledIndirect),
                0,
            ],
            material_mesh_range: [material_bucket, mesh_start, 1, 0],
        }
    }

    fn static_opaque_object(
        object_id: u32,
        material_bucket: u32,
        mesh_start: u32,
        instance_start: u32,
        instance_count: u32,
    ) -> GpuObjectRecord {
        object(
            object_id,
            material_bucket,
            mesh_start,
            instance_start,
            instance_count,
            GPU_VIS_OBJECT_STATIC_OPAQUE | GPU_VIS_OBJECT_RASTER,
            0.0,
        )
    }

    fn indirect_options() -> FunDrawPacketBuildOptions {
        FunDrawPacketBuildOptions {
            tiny_scene_instance_threshold: 0,
            ..Default::default()
        }
    }

    #[test]
    fn gpu_visibility_stage_order_is_stable() {
        assert_eq!(
            GPU_VISIBILITY_STATIC_OPAQUE_STAGES
                .iter()
                .map(|stage| stage.as_str())
                .collect::<Vec<_>>(),
            [
                "frustum_cull_cells_and_batches",
                "cull_instances_or_meshlets",
                "select_lod_or_cluster_level",
                "compact_visible_ids",
                "build_indirect_draw_args"
            ]
        );
    }

    #[test]
    fn policy_uses_gpu_above_static_instance_threshold() {
        let policy = StaticOpaqueGpuVisibilityPolicy::default();
        let decision = decide_static_opaque_visibility_path(
            policy,
            StaticOpaqueVisibilityDecisionInput {
                static_instance_count: policy.gpu_static_instance_threshold,
                static_buffers_resident: true,
                backend_supported: true,
                ..Default::default()
            },
        );

        assert_eq!(decision.path, StaticOpaqueVisibilityPath::GpuDriven);
        assert_eq!(decision.fallback_reason, None);
    }

    #[test]
    fn policy_uses_gpu_above_dense_batch_threshold() {
        let policy = StaticOpaqueGpuVisibilityPolicy::default();
        let decision = decide_static_opaque_visibility_path(
            policy,
            StaticOpaqueVisibilityDecisionInput {
                dense_batch_count: policy.gpu_dense_batch_threshold,
                static_buffers_resident: true,
                backend_supported: true,
                ..Default::default()
            },
        );

        assert_eq!(decision.path, StaticOpaqueVisibilityPath::GpuDriven);
    }

    #[test]
    fn policy_uses_cpu_for_small_static_scene() {
        let decision = decide_static_opaque_visibility_path(
            StaticOpaqueGpuVisibilityPolicy::default(),
            StaticOpaqueVisibilityDecisionInput {
                static_instance_count: 128,
                dense_batch_count: 8,
                static_buffers_resident: true,
                backend_supported: true,
                ..Default::default()
            },
        );

        assert_eq!(
            decision.path,
            StaticOpaqueVisibilityPath::CpuStaticCellCulling
        );
        assert_eq!(
            decision.fallback_reason,
            Some(StaticOpaqueVisibilityFallbackReason::TinyScene)
        );
    }

    #[test]
    fn unsupported_or_debug_uses_direct_fallback() {
        let policy = StaticOpaqueGpuVisibilityPolicy::default();
        let unsupported = decide_static_opaque_visibility_path(
            policy,
            StaticOpaqueVisibilityDecisionInput {
                static_instance_count: 4_096,
                static_buffers_resident: true,
                backend_supported: false,
                ..Default::default()
            },
        );
        let debug = decide_static_opaque_visibility_path(
            policy,
            StaticOpaqueVisibilityDecisionInput {
                static_instance_count: 4_096,
                static_buffers_resident: true,
                backend_supported: true,
                debug_mode: true,
                ..Default::default()
            },
        );

        assert_eq!(unsupported.path, StaticOpaqueVisibilityPath::FallbackDirect);
        assert_eq!(
            unsupported.fallback_reason,
            Some(StaticOpaqueVisibilityFallbackReason::UnsupportedBackend)
        );
        assert_eq!(debug.path, StaticOpaqueVisibilityPath::FallbackDirect);
        assert_eq!(
            debug.fallback_reason,
            Some(StaticOpaqueVisibilityFallbackReason::DebugMode)
        );
    }

    #[test]
    fn non_static_opaque_records_are_excluded() {
        let objects = [
            static_opaque_object(1, 1, 1, 0, 4),
            object(
                2,
                2,
                2,
                4,
                4,
                GPU_VIS_OBJECT_STATIC_OPAQUE | GPU_VIS_OBJECT_TRANSPARENT,
                0.0,
            ),
        ];

        let cull = frustum_cull_static_objects(&objects, &test_planes(), 0.0);

        assert_eq!(cull.visible_object_ids, [GpuVisibilityObjectId(1)]);
        assert_eq!(cull.excluded_non_static_opaque_count, 1);
        assert_eq!(cull.visible_instance_count, 4);
    }

    #[test]
    fn frustum_culls_cells_before_instances() {
        let objects = [
            static_opaque_object(1, 1, 1, 0, 3),
            object(
                2,
                1,
                1,
                3,
                7,
                GPU_VIS_OBJECT_STATIC_OPAQUE | GPU_VIS_OBJECT_RASTER,
                30.0,
            ),
        ];

        let cull = frustum_cull_static_objects(&objects, &test_planes(), 0.0);

        assert_eq!(cull.visible_object_ids, [GpuVisibilityObjectId(1)]);
        assert_eq!(cull.rejected_object_count, 1);
        assert_eq!(cull.visible_instance_count, 3);
        assert_eq!(cull.rejected_instance_count, 7);
    }

    #[test]
    fn lod_hysteresis_prevents_flicker() {
        let policy = GpuLodPolicy::default();

        assert_eq!(
            select_lod_with_hysteresis(92.0, 0, policy),
            0,
            "near the lod0 threshold, previous LOD should hold"
        );
        assert_eq!(
            select_lod_with_hysteresis(80.0, 0, policy),
            1,
            "outside the hysteresis band the coarser LOD is selected"
        );
    }

    #[test]
    fn compaction_preserves_visible_order() {
        let objects = [
            static_opaque_object(1, 1, 1, 0, 2),
            static_opaque_object(2, 1, 1, 4, 3),
            static_opaque_object(3, 1, 1, 9, 1),
        ];
        let compacted = compact_visible_static_opaque(
            &objects,
            &[GpuVisibilityObjectId(2), GpuVisibilityObjectId(1)],
        );

        assert_eq!(compacted.visible_object_ids, [2, 1]);
        assert_eq!(compacted.visible_instance_ids, [4, 5, 6, 0, 1]);
    }

    #[test]
    fn indirect_buckets_group_by_material_mesh_path() {
        let shared = static_opaque_object(1, 7, 9, 0, 16);
        let same_bucket = static_opaque_object(2, 7, 9, 16, 16);
        let different_material = static_opaque_object(3, 8, 9, 32, 4);
        let objects = [shared, same_bucket, different_material];
        let visible = [
            GpuVisibilityObjectId(1),
            GpuVisibilityObjectId(2),
            GpuVisibilityObjectId(3),
        ];
        let compacted = compact_visible_instance_ids(&objects, &visible);

        let output = build_indirect_buckets_for_static_opaque(
            &objects,
            &visible,
            &compacted,
            indirect_options(),
        );

        assert_eq!(output.draw_packet_report.bucket_count, 2);
        assert_eq!(output.draw_packet_report.indirect_draw_count, 2);
        assert_eq!(output.draw_packet_report.visible_instance_count, 36);
        assert_eq!(output.indirect_args.len(), 2);
    }

    #[test]
    fn draw_count_scales_with_visible_buckets_not_objects() {
        let objects = (0..64)
            .map(|object_id| static_opaque_object(object_id, 4, 11, object_id * 4, 4))
            .collect::<Vec<_>>();
        let visible = objects
            .iter()
            .map(|object| object.object_id())
            .collect::<Vec<_>>();
        let compacted = compact_visible_instance_ids(&objects, &visible);

        let output = build_indirect_buckets_for_static_opaque(
            &objects,
            &visible,
            &compacted,
            indirect_options(),
        );

        assert_eq!(output.draw_packet_report.bucket_count, 1);
        assert_eq!(output.draw_packet_report.indirect_draw_count, 1);
        assert_eq!(output.draw_packet_report.visible_instance_count, 256);
        assert_eq!(output.cpu_submit_count, 1);
        assert_eq!(output.draw_packet_report.draw_call_savings_vs_direct, 255);
    }

    #[test]
    fn buffer_plan_marks_visibility_buffers_persistent_unread() {
        let compact = gpu_visibility_buffer_descriptor(GpuVisibilityBufferKind::CompactVisibleIds)
            .expect("compact visible ids descriptor should exist");
        let indirect = gpu_visibility_buffer_descriptor(GpuVisibilityBufferKind::IndirectArgs)
            .expect("indirect args descriptor should exist");
        let counters = gpu_visibility_buffer_descriptor(GpuVisibilityBufferKind::Counters)
            .expect("counters descriptor should exist");

        assert_eq!(compact.lifetime, GpuVisibilityBufferLifetime::Persistent);
        assert!(!compact.full_buffer_write_allowed);
        assert!(!compact.normal_readback_allowed);
        assert_eq!(indirect.lifetime, GpuVisibilityBufferLifetime::Persistent);
        assert!(!indirect.normal_readback_allowed);
        assert_eq!(counters.lifetime, GpuVisibilityBufferLifetime::PerFrame);
        assert!(counters.normal_readback_allowed);
    }

    #[test]
    fn frame_builder_runs_stage_outputs_in_order() {
        let objects = [static_opaque_object(1, 3, 5, 0, 12)];
        let mut projected = BTreeMap::new();
        projected.insert(GpuVisibilityObjectId(1), 128.0);

        let frame = build_static_opaque_gpu_visibility_frame(
            &objects,
            &test_planes(),
            &projected,
            &BTreeMap::new(),
            StaticOpaqueVisibilityDecisionInput {
                static_instance_count: 2_000,
                dense_batch_count: 1,
                static_buffers_resident: true,
                backend_supported: true,
                ..Default::default()
            },
            StaticOpaqueGpuVisibilityPolicy::default(),
            indirect_options(),
        );
        let report = draw_packet_report_for_static_visibility_frame(&frame);
        let key: FunDrawBucketKey = draw_bucket_key_for_object(objects[0]);

        assert_eq!(frame.decision.path, StaticOpaqueVisibilityPath::GpuDriven);
        assert_eq!(frame.cull.visible_object_ids, [GpuVisibilityObjectId(1)]);
        assert_eq!(frame.lods[0].selected_lod, 0);
        assert_eq!(frame.compacted.visible_instance_count, 12);
        assert_eq!(report.bucket_count, 1);
        assert_eq!(report.buckets[0].key, key);
    }
}
