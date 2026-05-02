#import fun_render::sky::render::sky_common::{
    CloudParams,
    runtime_debug_mode,
    saturate,
}
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var source_texture: texture_2d<f32>;
@group(0) @binding(2) var auxiliary_texture: texture_2d<f32>;

#ifdef CLOUD_VIEW_COMPOSITE
#ifdef MULTISAMPLED_DEPTH
@group(0) @binding(3) var camera_depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(3) var camera_depth: texture_depth_2d;
#endif
#else
@group(0) @binding(3) var composite_output: texture_storage_2d<rgba16float, write>;
#endif

#ifndef CLOUD_VIEW_COMPOSITE
@compute @workgroup_size(8, 8, 1)
fn composite_clouds(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.xy;
    if (gid.x >= size.x || gid.y >= size.y) {
        return;
    }

    let coord = vec2<i32>(gid.xy);
    let cloud_color = textureLoad(source_texture, coord, 0);
    let cloud_data = textureLoad(auxiliary_texture, coord, 0);
    let exposure_guard = mix(1.0, 0.85, saturate(cloud.profile1.y));
    let color = max(cloud_color.rgb * exposure_guard, vec3<f32>(0.0));
    let alpha = saturate(1.0 - cloud_data.r);

    textureStore(composite_output, coord, vec4<f32>(color, alpha));
}
#endif

fn clamp_pixel(pixel: vec2<u32>, size: vec2<u32>) -> vec2<u32> {
    return min(pixel, max(size, vec2<u32>(1u, 1u)) - vec2<u32>(1u, 1u));
}

#ifdef CLOUD_VIEW_COMPOSITE
fn load_camera_depth(pixel: vec2<u32>) -> f32 {
#ifdef MULTISAMPLED_DEPTH
    return textureLoad(camera_depth, vec2<i32>(pixel), 0);
#else
    return textureLoad(camera_depth, vec2<i32>(pixel), 0);
#endif
}

fn load_resolved_cloud_for_camera_pixel(pixel: vec2<u32>, camera_size: vec2<u32>) -> vec4<f32> {
    let cloud_size = textureDimensions(auxiliary_texture);
    let uv = (vec2<f32>(pixel) + vec2<f32>(0.5, 0.5)) / vec2<f32>(max(camera_size, vec2<u32>(1u, 1u)));
    let cloud_pixel = clamp_pixel(vec2<u32>(uv * vec2<f32>(cloud_size)), cloud_size);
    return textureLoad(auxiliary_texture, vec2<i32>(cloud_pixel), 0);
}

@fragment
fn fragment_clouds_to_view(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let camera_size = textureDimensions(source_texture);
    let pixel = clamp_pixel(vec2<u32>(in.position.xy), camera_size);
    let scene = textureLoad(source_texture, vec2<i32>(pixel), 0);
    let cloud_color = load_resolved_cloud_for_camera_pixel(pixel, camera_size);

    if (runtime_debug_mode(cloud) != 0u) {
        return vec4<f32>(cloud_color.rgb, 1.0);
    }

    let reverse_z_depth = load_camera_depth(pixel);
    if (reverse_z_depth > 0.000001) {
        return scene;
    }

    return vec4<f32>(cloud_color.rgb, 1.0);
}
#endif
