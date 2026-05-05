struct FunGpuCullingInstanceMetadata {
    bounds_center_radius: vec4<f32>,
    bounds_extent_lod: vec4<f32>,
    meshlet_range: vec4<u32>,
    bucket_key: vec4<u32>,
    table_indices_flags: vec4<u32>,
};

struct FunGpuMeshletClusterMetadata {
    bounds_center_radius: vec4<f32>,
    normal_cone: vec4<f32>,
    instance_meshlet_range: vec4<u32>,
    flags: vec4<u32>,
};

struct FunGpuCullingViewConstants {
    frustum_planes: array<vec4<f32>, 6>,
    camera_position_lod: vec4<f32>,
    viewport_hysteresis: vec4<f32>,
    counts: vec4<u32>,
};

struct FunGpuDrawIndirectArgs {
    vertex_count_per_instance: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
};

@group(0) @binding(0) var<uniform> view: FunGpuCullingViewConstants;
@group(0) @binding(1) var<storage, read> instances: array<FunGpuCullingInstanceMetadata>;
@group(0) @binding(2) var<storage, read> clusters: array<FunGpuMeshletClusterMetadata>;
@group(0) @binding(3) var<storage, read_write> visibility_flags: array<atomic<u32>>;
@group(0) @binding(4) var<storage, read_write> compact_visible_ids: array<u32>;
@group(0) @binding(5) var<storage, read_write> indirect_args: array<FunGpuDrawIndirectArgs>;
@group(0) @binding(6) var<storage, read_write> counters: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read> hiz_depth: array<f32>;

const VISIBILITY_VISIBLE: u32 = 1u;
const VISIBILITY_OCCLUDED: u32 = 1u << 1u;
const VISIBILITY_LOD_SHIFT: u32 = 8u;
const WORKGROUP_SIZE: u32 = 64u;

fn sphere_visible(center_radius: vec4<f32>) -> bool {
    for (var plane_index: u32 = 0u; plane_index < 6u; plane_index = plane_index + 1u) {
        let plane = view.frustum_planes[plane_index];
        let distance = dot(plane.xyz, center_radius.xyz) + plane.w;
        if (distance + center_radius.w < 0.0) {
            return false;
        }
    }
    return true;
}

fn selected_lod(screen_space_size: f32, previous_lod: u32, lod_count: u32, hysteresis: f32) -> u32 {
    if (lod_count <= 1u) {
        return 0u;
    }

    var desired_lod = 0u;
    if (screen_space_size < 0.20) {
        desired_lod = 2u;
    } else if (screen_space_size < 0.45) {
        desired_lod = 1u;
    }
    desired_lod = min(desired_lod, lod_count - 1u);

    if (desired_lod != previous_lod) {
        let threshold = select(0.20, 0.45, desired_lod > previous_lod);
        if (abs(screen_space_size - threshold) <= hysteresis) {
            return min(previous_lod, lod_count - 1u);
        }
    }
    return desired_lod;
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn reset_culling_state(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let id = global_id.x;
    if (id < view.counts.x) {
        atomicStore(&visibility_flags[id], 0u);
    }
    if (id < 4u) {
        atomicStore(&counters[id], 0u);
    }
    if (id == 0u) {
        indirect_args[0].vertex_count_per_instance = 0u;
        indirect_args[0].instance_count = 0u;
        indirect_args[0].first_vertex = 0u;
        indirect_args[0].first_instance = 0u;
    }
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn instance_frustum_cull(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;
    if (instance_index >= view.counts.x) {
        return;
    }

    let instance = instances[instance_index];
    if (sphere_visible(instance.bounds_center_radius)) {
        atomicStore(&visibility_flags[instance_index], VISIBILITY_VISIBLE);
    } else {
        atomicStore(&visibility_flags[instance_index], 0u);
    }
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn lod_select(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;
    if (instance_index >= view.counts.x) {
        return;
    }
    if ((atomicLoad(&visibility_flags[instance_index]) & VISIBILITY_VISIBLE) == 0u) {
        return;
    }

    let instance = instances[instance_index];
    let screen_space_size = instance.bounds_extent_lod.w;
    let previous_lod = instance.table_indices_flags.w;
    let lod_count = max(instance.meshlet_range.z, 1u);
    let lod = selected_lod(screen_space_size, previous_lod, lod_count, view.viewport_hysteresis.z);
    let current_flags = atomicLoad(&visibility_flags[instance_index]) & 0x000000ffu;
    atomicStore(&visibility_flags[instance_index], current_flags | (lod << VISIBILITY_LOD_SHIFT));
    _ = atomicAdd(&counters[1], 1u);
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn meshlet_cluster_cull(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let cluster_index = global_id.x;
    if (cluster_index >= view.counts.y) {
        return;
    }

    let cluster = clusters[cluster_index];
    let instance_index = cluster.instance_meshlet_range.x;
    if ((atomicLoad(&visibility_flags[instance_index]) & VISIBILITY_VISIBLE) == 0u) {
        return;
    }

    let cone_axis = cluster.normal_cone.xyz;
    let cone_cutoff = cluster.normal_cone.w;
    let to_camera = normalize(view.camera_position_lod.xyz - cluster.bounds_center_radius.xyz);
    if (dot(cone_axis, to_camera) < cone_cutoff) {
        return;
    }

    let compact_index = atomicAdd(&counters[2], 1u);
    compact_visible_ids[compact_index] = cluster.instance_meshlet_range.y;
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn hiz_occlusion_cull(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;
    if (instance_index >= view.counts.x) {
        return;
    }
    if ((atomicLoad(&visibility_flags[instance_index]) & VISIBILITY_VISIBLE) == 0u) {
        return;
    }

    let hiz_index = min(instance_index, max(view.counts.z, 1u) - 1u);
    let conservative_depth = hiz_depth[hiz_index];
    let instance_depth = instances[instance_index].bounds_center_radius.z;
    if (instance_depth > conservative_depth) {
        let current_flags = atomicLoad(&visibility_flags[instance_index]);
        atomicStore(&visibility_flags[instance_index], (current_flags & ~VISIBILITY_VISIBLE) | VISIBILITY_OCCLUDED);
    }
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn compact_visible_instances(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let instance_index = global_id.x;
    if (instance_index >= view.counts.x) {
        return;
    }
    if ((atomicLoad(&visibility_flags[instance_index]) & VISIBILITY_VISIBLE) == 0u) {
        return;
    }

    let compact_index = atomicAdd(&counters[0], 1u);
    compact_visible_ids[compact_index] = instance_index;
}

@compute @workgroup_size(1)
fn generate_indirect_args(@builtin(global_invocation_id) global_id: vec3<u32>) {
    if (global_id.x != 0u) {
        return;
    }

    let visible_count = atomicLoad(&counters[0]);
    if (visible_count == 0u) {
        indirect_args[0].vertex_count_per_instance = 0u;
        indirect_args[0].instance_count = 0u;
        indirect_args[0].first_vertex = 0u;
        indirect_args[0].first_instance = 0u;
        atomicStore(&counters[3], 0u);
        return;
    }

    indirect_args[0].vertex_count_per_instance = 3u;
    indirect_args[0].instance_count = visible_count;
    indirect_args[0].first_vertex = 0u;
    indirect_args[0].first_instance = 0u;
    atomicStore(&counters[3], 1u);
}
