#import fun_render::sky::render::sky_common::{
    CloudParams,
    runtime_debug_mode,
    saturate,
}

@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var history_cloud: texture_2d<f32>;
@group(0) @binding(2) var cloud_transmittance: texture_2d<f32>;
@group(0) @binding(3) var debug_output: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn write_cloud_debug_overlay(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.xy;
    if (gid.x >= size.x || gid.y >= size.y) {
        return;
    }

    let coord = vec2<i32>(gid.xy);
    let cloud_color = textureLoad(history_cloud, coord, 0);
    let cloud_data = textureLoad(cloud_transmittance, coord, 0);
    let mode = runtime_debug_mode(cloud);

    var debug_color = cloud_color.rgb;
    if (mode == 1u) {
        debug_color = vec3<f32>(cloud_data.b, cloud_data.b * 0.45, 1.0 - cloud_data.b);
    } else if (mode == 2u) {
        let density = saturate(cloud_color.a);
        debug_color = vec3<f32>(density, density * density, 0.15);
    } else if (mode == 3u) {
        debug_color = vec3<f32>(cloud_data.g, 1.0 - cloud_data.g, 0.1);
    } else if (mode == 4u) {
        debug_color = vec3<f32>(cloud_color.a, abs(cloud_color.a - (1.0 - cloud_data.r)), cloud_data.r);
    } else if (mode == 5u) {
        debug_color = vec3<f32>(cloud.profile0.x, cloud.profile0.y * 0.25, cloud.profile1.y);
    }

    textureStore(debug_output, coord, vec4<f32>(debug_color, 1.0));
}
