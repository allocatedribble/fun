#import fun_render::sky::render::sky_common::{
    CloudParams,
    fbm2,
    fbm3,
    saturate,
}

@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var weather_map: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var shape_noise: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn generate_weather_map(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.z;
    if (gid.x >= size || gid.y >= size) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + vec2<f32>(0.5)) / f32(size);
    let time = f32(cloud.quality.w) * 0.00045;
    let wind = cloud.wind.xy * cloud.wind.z * time;
    let seed = vec2<f32>(cloud.wind.w * 29.13, cloud.wind.w * 71.87);
    let scale = max(cloud.profile1.x / 32000.0, 0.25);
    let p = uv * scale * 8.0 + wind + seed;

    let broad = fbm2(p);
    let cells = fbm2(p * 2.8 + vec2<f32>(7.1, 3.8));
    let front = fbm2(p * 0.58 + vec2<f32>(19.2, 2.7));

    let storm = cloud.profile1.y;
    let base_coverage = cloud.profile0.x;
    let coverage = saturate(base_coverage + (broad - 0.5) * 0.55 + (front - 0.5) * storm * 0.7);
    let cloud_type = saturate(0.22 + cells * 0.62 + storm * 0.26);
    let precipitation = saturate(storm * 0.84 + coverage * coverage * 0.18);
    let density_multiplier = saturate(0.28 + cloud.profile0.y * 0.24 + cells * 0.38);

    textureStore(weather_map, vec2<i32>(gid.xy), vec4<f32>(
        coverage,
        cloud_type,
        precipitation,
        density_multiplier,
    ));
}

@compute @workgroup_size(4, 4, 4)
fn generate_shape_noise(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.w;
    if (gid.x >= size || gid.y >= size || gid.z >= size) {
        return;
    }

    let uvw = (vec3<f32>(gid) + vec3<f32>(0.5)) / f32(size);
    let seed = vec3<f32>(cloud.wind.w * 13.0, cloud.wind.w * 37.0, cloud.wind.w * 91.0);
    let body = fbm3(uvw * 5.0 + seed);
    let billow = fbm3(uvw * 12.0 + seed.yzx);
    let erosion = fbm3(uvw * 28.0 + seed.zxy);
    let wisps = fbm3(vec3<f32>(uvw.xy * 18.0, uvw.z * 3.0) + seed.xzy);

    textureStore(shape_noise, vec3<i32>(gid), vec4<f32>(
        saturate(body),
        saturate(billow),
        saturate(erosion),
        saturate(wisps),
    ));
}
