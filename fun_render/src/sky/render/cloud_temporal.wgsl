#import fun_render::sky::render::sky_common::{
    CloudParams,
    intersect_cloud_slab,
    reconstruct_world_ray,
    reproject_world_to_uv,
    runtime_temporal_enabled,
    saturate,
}

@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var current_cloud: texture_2d<f32>;
@group(0) @binding(2) var history_read: texture_2d<f32>;
@group(0) @binding(3) var history_write: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var debug_history: texture_storage_2d<rgba16float, write>;

fn clamp_history_pixel(pixel: vec2<i32>, size: vec2<u32>) -> vec2<i32> {
    return clamp(pixel, vec2<i32>(0), vec2<i32>(max(size, vec2<u32>(1u)) - vec2<u32>(1u)));
}

fn history_reprojection_uv(uv: vec2<f32>) -> vec2<f32> {
    let ray_origin = cloud.current_camera.xyz;
    let ray_dir = reconstruct_world_ray(cloud, uv);
    let segment = intersect_cloud_slab(cloud, ray_origin, ray_dir);
    let representative_distance = select(12000.0, mix(segment.x, segment.y, 0.42), segment.y > segment.x);
    let world_position = ray_origin + ray_dir * representative_distance;
    let wind_delta = cloud.wind_history.xy - cloud.wind_history.zw;
    return reproject_world_to_uv(cloud, world_position) - wind_delta;
}

fn current_neighborhood_bounds(coord: vec2<i32>, size: vec2<u32>) -> array<vec3<f32>, 2> {
    var bounds: array<vec3<f32>, 2>;
    var min_color = vec3<f32>(65504.0);
    var max_color = vec3<f32>(0.0);
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let sample_coord = clamp_history_pixel(coord + vec2<i32>(x, y), size);
            let sample_color = textureLoad(current_cloud, sample_coord, 0).rgb;
            min_color = min(min_color, sample_color);
            max_color = max(max_color, sample_color);
        }
    }
    bounds[0] = min_color;
    bounds[1] = max_color;
    return bounds;
}

@compute @workgroup_size(8, 8, 1)
fn resolve_cloud_history(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.xy;
    if (gid.x >= size.x || gid.y >= size.y) {
        return;
    }

    let coord = vec2<i32>(gid.xy);
    let current = textureLoad(current_cloud, coord, 0);
    let uv = (vec2<f32>(gid.xy) + vec2<f32>(0.5)) / vec2<f32>(size);
    let previous_uv = history_reprojection_uv(uv);
    let history_valid = all(previous_uv >= vec2<f32>(0.0)) && all(previous_uv <= vec2<f32>(1.0));
    let history_coord = clamp_history_pixel(vec2<i32>(previous_uv * vec2<f32>(size)), size);
    let history = textureLoad(history_read, history_coord, 0);
    let transmittance = saturate(current.a);
    let reset_this_frame = cloud.history.z != 0u;
    let can_reuse_history = runtime_temporal_enabled(cloud) && history_valid && !reset_this_frame;

    let temporal_alpha = select(1.0, mix(0.18, 0.08, saturate(cloud.profile0.x)), can_reuse_history);
    let bounds = current_neighborhood_bounds(coord, size);
    let clamped_history = clamp(history.rgb, bounds[0], bounds[1]);
    let color = mix(clamped_history, current.rgb, temporal_alpha);
    let alpha = mix(history.a, transmittance, temporal_alpha);
    let history_age = select(0.0, min(f32(cloud.history.w) / 32.0, 1.0), can_reuse_history);
    let rejected = select(1.0, 0.0, can_reuse_history);

    textureStore(history_write, coord, vec4<f32>(color, alpha));
    textureStore(debug_history, coord, vec4<f32>(
        vec3<f32>(select(0.0, 1.0, can_reuse_history), rejected, history_age),
        1.0,
    ));
}
