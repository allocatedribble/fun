#import fun_render::sky::render::sky_common::{
    CloudParams,
    runtime_temporal_enabled,
    saturate,
}

@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var current_cloud: texture_2d<f32>;
@group(0) @binding(2) var history_read: texture_2d<f32>;
@group(0) @binding(3) var history_write: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var debug_history: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn resolve_cloud_history(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.xy;
    if (gid.x >= size.x || gid.y >= size.y) {
        return;
    }

    let coord = vec2<i32>(gid.xy);
    let current = textureLoad(current_cloud, coord, 0);
    let history = textureLoad(history_read, coord, 0);
    let transmittance = saturate(current.a);

    let temporal_alpha = select(1.0, mix(0.18, 0.08, saturate(cloud.profile0.x)), runtime_temporal_enabled(cloud));
    let color = mix(history.rgb, current.rgb, temporal_alpha);
    let alpha = mix(history.a, transmittance, temporal_alpha);
    let history_age = min(history.a + 1.0 / 32.0, 1.0);

    textureStore(history_write, coord, vec4<f32>(color, alpha));
    textureStore(debug_history, coord, vec4<f32>(
        vec3<f32>(history_age, temporal_alpha, abs(current.a - history.a)),
        1.0,
    ));
}
