use fun_ecs::{EcsAabbF32, EcsLuxHandoffQueue, LuxArtifactHandoff, LuxArtifactHandoffKind};

use crate::{
    FUN_LUX_FRAME_PLAN_SCHEMA_VERSION, LuxAabb, LuxDirtyFlags, LuxDirtyRegion, LuxPassCommon,
    LuxPassRequest, LuxQualityTier, LuxResourceIntent, LuxSceneFramePlan, LuxSceneId,
    LuxScenePriority, StormExtinctionInjectPass, VoxelCanopyTransmittanceInjectPass,
    VoxelRadianceClipmapUpdatePass, VoxelSdfDistantShadowResolvePass, VoxelShadowDemandMarkPass,
    VoxelShadowPageBuildPass, VoxelTerrainAoResolvePass,
};

pub const FUN_LUX_ECS_HANDOFF_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxEcsOwnershipContract {
    pub fun_lux_decides_virtual_shadow_policy: bool,
    pub fun_lux_decides_many_light_policy: bool,
    pub fun_lux_decides_gi_probe_radiance_policy: bool,
    pub fun_lux_decides_canopy_transmittance_policy: bool,
    pub fun_lux_decides_storm_extinction_policy: bool,
    pub fun_lux_decides_quality_tier: bool,
    pub fun_renderer_executes_gpu_realization: bool,
    pub fun_ecs_provides_dirty_rows_bounds_epochs_and_requiredness: bool,
}

impl LuxEcsOwnershipContract {
    pub const PRODUCT_DEFAULT: Self = Self {
        fun_lux_decides_virtual_shadow_policy: true,
        fun_lux_decides_many_light_policy: true,
        fun_lux_decides_gi_probe_radiance_policy: true,
        fun_lux_decides_canopy_transmittance_policy: true,
        fun_lux_decides_storm_extinction_policy: true,
        fun_lux_decides_quality_tier: true,
        fun_renderer_executes_gpu_realization: true,
        fun_ecs_provides_dirty_rows_bounds_epochs_and_requiredness: true,
    };

    #[must_use]
    pub const fn contract_holds(self) -> bool {
        self.fun_lux_decides_virtual_shadow_policy
            && self.fun_lux_decides_many_light_policy
            && self.fun_lux_decides_gi_probe_radiance_policy
            && self.fun_lux_decides_canopy_transmittance_policy
            && self.fun_lux_decides_storm_extinction_policy
            && self.fun_lux_decides_quality_tier
            && self.fun_renderer_executes_gpu_realization
            && self.fun_ecs_provides_dirty_rows_bounds_epochs_and_requiredness
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LuxEcsHandoffPlanReport {
    pub inspected: u32,
    pub dirty_regions: u32,
    pub passes: u32,
    pub resources: u32,
    pub required_rows: u32,
    pub optional_rows: u32,
    pub max_dirty_epoch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HandoffCounts {
    shadow_rows: u32,
    sdf_rows: u32,
    canopy_rows: u32,
    radiance_rows: u32,
    storm_rows: u32,
    required_rows: u32,
    optional_rows: u32,
    max_dirty_epoch: u32,
}

impl HandoffCounts {
    fn observe(&mut self, handoff: LuxArtifactHandoff) {
        self.max_dirty_epoch = self.max_dirty_epoch.max(handoff.dirty_epoch);
        if handoff.requiredness.is_optional() {
            self.optional_rows = self.optional_rows.saturating_add(1);
        } else {
            self.required_rows = self.required_rows.saturating_add(1);
        }
        match handoff.kind {
            LuxArtifactHandoffKind::VoxelShadowInvalidationRows => {
                self.shadow_rows = self.shadow_rows.saturating_add(1);
            }
            LuxArtifactHandoffKind::TerrainSdfForDistantShadow => {
                self.sdf_rows = self.sdf_rows.saturating_add(1);
            }
            LuxArtifactHandoffKind::CanopyOpacityForTransmittance => {
                self.canopy_rows = self.canopy_rows.saturating_add(1);
            }
            LuxArtifactHandoffKind::RadianceClipmapDirtyRows => {
                self.radiance_rows = self.radiance_rows.saturating_add(1);
            }
            LuxArtifactHandoffKind::StormExtinctionDirtyRows => {
                self.storm_rows = self.storm_rows.saturating_add(1);
            }
        }
    }
}

pub fn plan_scene_from_ecs_lux_handoffs(
    queue: &EcsLuxHandoffQueue,
    scene_id: LuxSceneId,
    quality_tier: LuxQualityTier,
) -> (LuxSceneFramePlan, LuxEcsHandoffPlanReport) {
    let mut counts = HandoffCounts {
        shadow_rows: 0,
        sdf_rows: 0,
        canopy_rows: 0,
        radiance_rows: 0,
        storm_rows: 0,
        required_rows: 0,
        optional_rows: 0,
        max_dirty_epoch: 0,
    };
    let mut dirty_regions = Vec::new();

    for handoff in &queue.items {
        counts.observe(*handoff);
        dirty_regions.push(dirty_region_from_handoff(scene_id, *handoff));
    }

    let mut resources = Vec::new();
    append_resources(&mut resources, counts);

    let mut passes = Vec::new();
    append_passes(&mut passes, counts, quality_tier);

    let report = LuxEcsHandoffPlanReport {
        inspected: queue.items.len().min(u32::MAX as usize) as u32,
        dirty_regions: dirty_regions.len().min(u32::MAX as usize) as u32,
        passes: passes.len().min(u32::MAX as usize) as u32,
        resources: resources.len().min(u32::MAX as usize) as u32,
        required_rows: counts.required_rows,
        optional_rows: counts.optional_rows,
        max_dirty_epoch: counts.max_dirty_epoch,
    };
    let scene = LuxSceneFramePlan {
        schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
        scene_id,
        scene_revision: u64::from(counts.max_dirty_epoch),
        visible: !queue.items.is_empty(),
        priority: LuxScenePriority::World,
        passes,
        resources,
        dirty_regions,
    };

    (scene, report)
}

fn dirty_region_from_handoff(scene_id: LuxSceneId, handoff: LuxArtifactHandoff) -> LuxDirtyRegion {
    let priority = if handoff.requiredness.is_optional() {
        96
    } else {
        224
    };
    LuxDirtyRegion::new(
        scene_id,
        lux_aabb_from_ecs(handoff.bounds_world),
        dirty_flags_for_handoff(handoff.kind),
        priority,
        estimated_cost_micros(handoff.kind),
    )
    .with_deadline_frame(u64::from(handoff.dirty_epoch))
}

#[must_use]
pub const fn dirty_flags_for_handoff(kind: LuxArtifactHandoffKind) -> LuxDirtyFlags {
    match kind {
        LuxArtifactHandoffKind::VoxelShadowInvalidationRows => LuxDirtyFlags::SHADOW_POLICY,
        LuxArtifactHandoffKind::TerrainSdfForDistantShadow => {
            LuxDirtyFlags::SHADOW_POLICY.with(LuxDirtyFlags::GI_CACHE)
        }
        LuxArtifactHandoffKind::CanopyOpacityForTransmittance => {
            LuxDirtyFlags::VOLUMETRIC.with(LuxDirtyFlags::SHADOW_POLICY)
        }
        LuxArtifactHandoffKind::RadianceClipmapDirtyRows => LuxDirtyFlags::GI_CACHE,
        LuxArtifactHandoffKind::StormExtinctionDirtyRows => LuxDirtyFlags::VOLUMETRIC,
    }
}

#[must_use]
pub const fn lux_aabb_from_ecs(bounds: EcsAabbF32) -> LuxAabb {
    LuxAabb::new(bounds.min, bounds.max)
}

fn append_resources(resources: &mut Vec<LuxResourceIntent>, counts: HandoffCounts) {
    if counts.shadow_rows != 0 {
        resources.push(LuxResourceIntent::VoxelShadowPageTable {
            stable_id: "fun_lux.voxel.shadow_page_table",
            page_count: counts.shadow_rows,
        });
    }
    if counts.sdf_rows != 0 {
        resources.push(LuxResourceIntent::VoxelTerrainSdfPool {
            stable_id: "fun_lux.voxel.terrain_sdf_pool",
            page_count: counts.sdf_rows,
        });
    }
    if counts.radiance_rows != 0 {
        resources.push(LuxResourceIntent::VoxelTerrainRadianceClipmap {
            stable_id: "fun_lux.voxel.radiance_clipmap",
            voxel_count: u64::from(counts.radiance_rows).saturating_mul(32 * 32 * 32),
        });
    }
    if counts.canopy_rows != 0 {
        resources.push(LuxResourceIntent::VoxelCanopyOpacityClipmap {
            stable_id: "fun_lux.voxel.canopy_opacity_clipmap",
            voxel_count: u64::from(counts.canopy_rows).saturating_mul(32 * 32 * 32),
        });
    }
    if counts.storm_rows != 0 {
        resources.push(LuxResourceIntent::StormExtinctionClipmap {
            stable_id: "fun_lux.storm.extinction_clipmap",
            voxel_count: u64::from(counts.storm_rows).saturating_mul(32 * 32 * 32),
        });
    }
}

fn append_passes(
    passes: &mut Vec<LuxPassRequest>,
    counts: HandoffCounts,
    quality_tier: LuxQualityTier,
) {
    if counts.shadow_rows != 0 {
        passes.push(LuxPassRequest::VoxelShadowDemandMark(
            VoxelShadowDemandMarkPass {
                common: LuxPassCommon::new("fun_lux.voxel.shadow_demand_mark", quality_tier),
                dirty_row_count: counts.shadow_rows,
            },
        ));
        passes.push(LuxPassRequest::VoxelShadowPageBuild(
            VoxelShadowPageBuildPass {
                common: LuxPassCommon::new("fun_lux.voxel.shadow_page_build", quality_tier)
                    .depends_on("fun_lux.voxel.shadow_demand_mark"),
                max_pages_per_frame: counts.shadow_rows,
            },
        ));
    }
    if counts.sdf_rows != 0 {
        passes.push(LuxPassRequest::VoxelSdfDistantShadowResolve(
            VoxelSdfDistantShadowResolvePass {
                common: LuxPassCommon::new("fun_lux.voxel.sdf_distant_shadow", quality_tier),
                sdf_page_count: counts.sdf_rows,
            },
        ));
        passes.push(LuxPassRequest::VoxelTerrainAoResolve(
            VoxelTerrainAoResolvePass {
                common: LuxPassCommon::new("fun_lux.voxel.terrain_ao", quality_tier),
                sdf_page_count: counts.sdf_rows,
            },
        ));
    }
    if counts.radiance_rows != 0 {
        passes.push(LuxPassRequest::VoxelRadianceClipmapUpdate(
            VoxelRadianceClipmapUpdatePass {
                common: LuxPassCommon::new("fun_lux.voxel.radiance_clipmap", quality_tier),
                dirty_row_count: counts.radiance_rows,
            },
        ));
    }
    if counts.canopy_rows != 0 {
        passes.push(LuxPassRequest::VoxelCanopyTransmittanceInject(
            VoxelCanopyTransmittanceInjectPass {
                common: LuxPassCommon::new("fun_lux.voxel.canopy_transmittance", quality_tier),
                opacity_page_count: counts.canopy_rows,
            },
        ));
    }
    if counts.storm_rows != 0 {
        passes.push(LuxPassRequest::StormExtinctionInject(
            StormExtinctionInjectPass {
                common: LuxPassCommon::new("fun_lux.storm.extinction", quality_tier),
                dirty_row_count: counts.storm_rows,
            },
        ));
    }
}

#[must_use]
pub const fn estimated_cost_micros(kind: LuxArtifactHandoffKind) -> u32 {
    match kind {
        LuxArtifactHandoffKind::VoxelShadowInvalidationRows => 24,
        LuxArtifactHandoffKind::TerrainSdfForDistantShadow => 32,
        LuxArtifactHandoffKind::CanopyOpacityForTransmittance => 24,
        LuxArtifactHandoffKind::RadianceClipmapDirtyRows => 40,
        LuxArtifactHandoffKind::StormExtinctionDirtyRows => 32,
    }
}

#[must_use]
pub const fn handoff_is_frame_required(handoff: LuxArtifactHandoff) -> bool {
    !handoff.requiredness.is_optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_ecs::{
        EcsPageChannel, EcsSpatialDomainKind, EcsSpatialGridId, EcsSpatialPageKey, WorkRequiredness,
    };

    fn page(channel: EcsPageChannel) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            1,
            2,
            3,
            channel,
        )
    }

    fn handoff(kind: LuxArtifactHandoffKind, requiredness: WorkRequiredness) -> LuxArtifactHandoff {
        LuxArtifactHandoff {
            source_page: page(EcsPageChannel::Radiance),
            kind,
            dirty_epoch: 9,
            bounds_world: EcsAabbF32::new([1.0, 2.0, 3.0], [33.0, 34.0, 35.0]),
            requiredness,
        }
    }

    #[test]
    fn ownership_contract_keeps_ecs_lux_renderer_split_explicit() {
        assert!(LuxEcsOwnershipContract::PRODUCT_DEFAULT.contract_holds());
    }

    #[test]
    fn ecs_lux_handoffs_plan_backend_neutral_passes_resources_and_dirty_bounds() {
        let mut queue = EcsLuxHandoffQueue::default();
        queue
            .push(handoff(
                LuxArtifactHandoffKind::VoxelShadowInvalidationRows,
                WorkRequiredness::Required,
            ))
            .expect("shadow handoff");
        queue
            .push(handoff(
                LuxArtifactHandoffKind::TerrainSdfForDistantShadow,
                WorkRequiredness::Required,
            ))
            .expect("sdf handoff");
        queue
            .push(handoff(
                LuxArtifactHandoffKind::CanopyOpacityForTransmittance,
                WorkRequiredness::Optional,
            ))
            .expect("canopy handoff");
        queue
            .push(handoff(
                LuxArtifactHandoffKind::RadianceClipmapDirtyRows,
                WorkRequiredness::Optional,
            ))
            .expect("radiance handoff");
        queue
            .push(handoff(
                LuxArtifactHandoffKind::StormExtinctionDirtyRows,
                WorkRequiredness::Optional,
            ))
            .expect("storm handoff");

        let (scene, report) =
            plan_scene_from_ecs_lux_handoffs(&queue, LuxSceneId::PROOF_SCENE, LuxQualityTier::High);
        let pass_kinds: Vec<_> = scene.passes.iter().map(LuxPassRequest::kind).collect();
        let resource_kinds: Vec<_> = scene
            .resources
            .iter()
            .map(LuxResourceIntent::kind)
            .collect();

        assert_eq!(report.inspected, 5);
        assert_eq!(report.dirty_regions, 5);
        assert_eq!(report.required_rows, 2);
        assert_eq!(report.optional_rows, 3);
        assert_eq!(report.max_dirty_epoch, 9);
        assert!(scene.visible);
        assert_eq!(scene.scene_revision, 9);
        assert!(pass_kinds.contains(&crate::LuxPassKind::VoxelShadowDemandMark));
        assert!(pass_kinds.contains(&crate::LuxPassKind::VoxelShadowPageBuild));
        assert!(pass_kinds.contains(&crate::LuxPassKind::VoxelSdfDistantShadowResolve));
        assert!(pass_kinds.contains(&crate::LuxPassKind::VoxelTerrainAoResolve));
        assert!(pass_kinds.contains(&crate::LuxPassKind::VoxelRadianceClipmapUpdate));
        assert!(pass_kinds.contains(&crate::LuxPassKind::VoxelCanopyTransmittanceInject));
        assert!(pass_kinds.contains(&crate::LuxPassKind::StormExtinctionInject));
        assert!(resource_kinds.contains(&crate::LuxResourceIntentKind::VoxelShadowPageTable));
        assert!(resource_kinds.contains(&crate::LuxResourceIntentKind::VoxelTerrainSdfPool));
        assert!(
            resource_kinds.contains(&crate::LuxResourceIntentKind::VoxelTerrainRadianceClipmap)
        );
        assert!(resource_kinds.contains(&crate::LuxResourceIntentKind::VoxelCanopyOpacityClipmap));
        assert!(resource_kinds.contains(&crate::LuxResourceIntentKind::StormExtinctionClipmap));
        assert_eq!(scene.dirty_regions[0].bounds.min, [1.0, 2.0, 3.0]);
        assert_eq!(scene.dirty_regions[0].deadline_frame, Some(9));
    }

    #[test]
    fn handoff_flags_preserve_shadow_gi_canopy_and_storm_policy() {
        assert!(
            dirty_flags_for_handoff(LuxArtifactHandoffKind::VoxelShadowInvalidationRows)
                .invalidates_shadow_maps()
        );
        assert!(
            dirty_flags_for_handoff(LuxArtifactHandoffKind::TerrainSdfForDistantShadow)
                .invalidates_gi_cache()
        );
        assert!(
            dirty_flags_for_handoff(LuxArtifactHandoffKind::CanopyOpacityForTransmittance)
                .invalidates_shadow_maps()
        );
        assert!(
            dirty_flags_for_handoff(LuxArtifactHandoffKind::StormExtinctionDirtyRows)
                .touches_volumetric_only()
        );
    }
}
