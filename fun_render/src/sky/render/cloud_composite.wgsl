#import fun_render::sky::render::sky_common::{
    CloudParams,
    saturate,
}

@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var history_cloud: texture_2d<f32>;
@group(0) @binding(2) var cloud_transmittance: texture_2d<f32>;
@group(0) @binding(3) var composite_output: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn composite_clouds(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.xy;
    if (gid.x >= size.x || gid.y >= size.y) {
        return;
    }

    let coord = vec2<i32>(gid.xy);
    let cloud_color = textureLoad(history_cloud, coord, 0);
    let cloud_data = textureLoad(cloud_transmittance, coord, 0);
    let exposure_guard = mix(1.0, 0.85, saturate(cloud.profile1.y));
    let color = max(cloud_color.rgb * exposure_guard, vec3<f32>(0.0));
    let alpha = saturate(1.0 - cloud_data.r);

    textureStore(composite_output, coord, vec4<f32>(color, alpha));
}
