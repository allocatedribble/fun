use std::collections::BTreeMap;

use super::types::{GpuObjectRecord, GpuVisibilityObjectId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GpuCompactionReport {
    pub visible_object_ids: Vec<u32>,
    pub visible_instance_ids: Vec<u32>,
    pub visible_object_count: u32,
    pub visible_instance_count: u32,
}

pub fn compact_visible_object_ids(
    objects: &[GpuObjectRecord],
    visible_object_ids: &[GpuVisibilityObjectId],
) -> Vec<u32> {
    let objects_by_id = objects
        .iter()
        .filter(|object| object.is_static_opaque_world())
        .map(|object| (object.object_id(), *object))
        .collect::<BTreeMap<_, _>>();

    visible_object_ids
        .iter()
        .copied()
        .filter(|object_id| objects_by_id.contains_key(object_id))
        .map(|object_id| object_id.0)
        .collect()
}

pub fn compact_visible_instance_ids(
    objects: &[GpuObjectRecord],
    visible_object_ids: &[GpuVisibilityObjectId],
) -> Vec<u32> {
    let objects_by_id = objects
        .iter()
        .filter(|object| object.is_static_opaque_world())
        .map(|object| (object.object_id(), *object))
        .collect::<BTreeMap<_, _>>();
    let mut visible_instance_ids = Vec::new();

    for object_id in visible_object_ids {
        let Some(object) = objects_by_id.get(object_id).copied() else {
            continue;
        };
        let range = object.instance_range();
        visible_instance_ids.extend(range.start..range.end());
    }

    visible_instance_ids
}

pub fn compact_visible_static_opaque(
    objects: &[GpuObjectRecord],
    visible_object_ids: &[GpuVisibilityObjectId],
) -> GpuCompactionReport {
    let visible_object_ids = compact_visible_object_ids(objects, visible_object_ids);
    let visible_instance_ids = compact_visible_instance_ids(
        objects,
        &visible_object_ids
            .iter()
            .copied()
            .map(GpuVisibilityObjectId)
            .collect::<Vec<_>>(),
    );

    GpuCompactionReport {
        visible_object_count: visible_object_ids.len() as u32,
        visible_instance_count: visible_instance_ids.len() as u32,
        visible_object_ids,
        visible_instance_ids,
    }
}
