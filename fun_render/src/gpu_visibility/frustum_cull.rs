use super::types::{GpuObjectRecord, GpuVisibilityObjectId};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpuFrustumPlane {
    pub normal: [f32; 3],
    pub distance: f32,
}

impl GpuFrustumPlane {
    pub const fn new(normal: [f32; 3], distance: f32) -> Self {
        Self { normal, distance }
    }

    pub const fn from_coefficients(a: f32, b: f32, c: f32, d: f32) -> Self {
        Self {
            normal: [a, b, c],
            distance: d,
        }
    }

    pub const fn as_array(self) -> [f32; 4] {
        [
            self.normal[0],
            self.normal[1],
            self.normal[2],
            self.distance,
        ]
    }

    pub fn signed_distance(self, point: [f32; 3]) -> f32 {
        self.normal[0].mul_add(
            point[0],
            self.normal[1].mul_add(point[1], self.normal[2].mul_add(point[2], self.distance)),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GpuFrustumCullOutput {
    pub visible_object_ids: Vec<GpuVisibilityObjectId>,
    pub submitted_object_count: u32,
    pub visible_object_count: u32,
    pub submitted_instance_count: u32,
    pub visible_instance_count: u32,
    pub rejected_object_count: u32,
    pub rejected_instance_count: u32,
    pub excluded_non_static_opaque_count: u32,
}

pub fn conservative_sphere_visible(
    planes: &[GpuFrustumPlane],
    center_radius: [f32; 4],
    conservative_padding: f32,
) -> bool {
    let center = [center_radius[0], center_radius[1], center_radius[2]];
    let radius = center_radius[3].max(0.0) + conservative_padding.max(0.0);
    planes
        .iter()
        .all(|plane| plane.signed_distance(center) + radius >= 0.0)
}

pub fn frustum_cull_static_objects(
    objects: &[GpuObjectRecord],
    planes: &[GpuFrustumPlane],
    conservative_padding: f32,
) -> GpuFrustumCullOutput {
    let mut output = GpuFrustumCullOutput::default();

    for object in objects {
        if !object.is_static_opaque_world() {
            output.excluded_non_static_opaque_count =
                output.excluded_non_static_opaque_count.saturating_add(1);
            continue;
        }

        let instance_count = object.instance_range().count;
        output.submitted_object_count = output.submitted_object_count.saturating_add(1);
        output.submitted_instance_count = output
            .submitted_instance_count
            .saturating_add(instance_count);

        if conservative_sphere_visible(planes, object.bounds_center_radius, conservative_padding) {
            output.visible_object_ids.push(object.object_id());
            output.visible_object_count = output.visible_object_count.saturating_add(1);
            output.visible_instance_count =
                output.visible_instance_count.saturating_add(instance_count);
        } else {
            output.rejected_object_count = output.rejected_object_count.saturating_add(1);
            output.rejected_instance_count = output
                .rejected_instance_count
                .saturating_add(instance_count);
        }
    }

    output
}
