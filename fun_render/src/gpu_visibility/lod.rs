use std::collections::BTreeMap;

use super::types::{GpuObjectRecord, GpuVisibilityObjectId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuLodPolicy {
    pub lod0_min_projected_radius_px: f32,
    pub lod1_min_projected_radius_px: f32,
    pub hysteresis_px: f32,
    pub max_lod: u8,
}

impl Default for GpuLodPolicy {
    fn default() -> Self {
        Self {
            lod0_min_projected_radius_px: 96.0,
            lod1_min_projected_radius_px: 32.0,
            hysteresis_px: 8.0,
            max_lod: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GpuLodSelection {
    pub object_id: GpuVisibilityObjectId,
    pub selected_lod: u8,
}

pub fn select_lod_with_hysteresis(
    projected_radius_px: f32,
    previous_lod: u8,
    policy: GpuLodPolicy,
) -> u8 {
    let desired_lod = desired_lod(projected_radius_px, policy);
    let previous_lod = previous_lod.min(policy.max_lod);
    if desired_lod == previous_lod {
        return desired_lod;
    }

    let threshold = if desired_lod > previous_lod {
        match previous_lod {
            0 => policy.lod0_min_projected_radius_px,
            _ => policy.lod1_min_projected_radius_px,
        }
    } else {
        match desired_lod {
            0 => policy.lod0_min_projected_radius_px,
            _ => policy.lod1_min_projected_radius_px,
        }
    };

    if (projected_radius_px - threshold).abs() <= policy.hysteresis_px {
        previous_lod
    } else {
        desired_lod
    }
}

pub fn select_lods_for_visible_objects(
    objects: &[GpuObjectRecord],
    visible_object_ids: &[GpuVisibilityObjectId],
    projected_radius_px_by_object: &BTreeMap<GpuVisibilityObjectId, f32>,
    previous_lod_by_object: &BTreeMap<GpuVisibilityObjectId, u8>,
    policy: GpuLodPolicy,
) -> Vec<GpuLodSelection> {
    let objects_by_id = objects
        .iter()
        .filter(|object| object.is_static_opaque_world())
        .map(|object| (object.object_id(), *object))
        .collect::<BTreeMap<_, _>>();

    visible_object_ids
        .iter()
        .copied()
        .filter(|object_id| objects_by_id.contains_key(object_id))
        .map(|object_id| {
            let projected_radius_px = projected_radius_px_by_object
                .get(&object_id)
                .copied()
                .unwrap_or(policy.lod1_min_projected_radius_px);
            let previous_lod = previous_lod_by_object
                .get(&object_id)
                .copied()
                .unwrap_or_default();
            GpuLodSelection {
                object_id,
                selected_lod: select_lod_with_hysteresis(projected_radius_px, previous_lod, policy),
            }
        })
        .collect()
}

fn desired_lod(projected_radius_px: f32, policy: GpuLodPolicy) -> u8 {
    if projected_radius_px >= policy.lod0_min_projected_radius_px {
        0
    } else if projected_radius_px >= policy.lod1_min_projected_radius_px {
        1_u8.min(policy.max_lod)
    } else {
        2_u8.min(policy.max_lod)
    }
}
