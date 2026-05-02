#import fun_render::sky::render::sky_common::{
    CloudParams,
    blue_noise_jitter,
    height_gradient,
    intersect_cloud_slab,
    reconstruct_world_ray,
    saturate,
    sky_gradient,
}

@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var weather_map: texture_2d<f32>;
@group(0) @binding(2) var shape_noise: texture_3d<f32>;
@group(0) @binding(3) var cloud_color: texture_storage_2d<rgba16float, write>;
@group(0) @binding(4) var cloud_transmittance: texture_storage_2d<rgba16float, write>;

fn sample_weather(uv: vec2<f32>) -> vec4<f32> {
    let dims = textureDimensions(weather_map);
    let coord = vec2<u32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.9999)) * vec2<f32>(dims));
    return textureLoad(weather_map, vec2<i32>(coord), 0);
}

fn sample_shape(uv: vec3<f32>) -> vec4<f32> {
    let dims = textureDimensions(shape_noise);
    let wrapped = fract(uv);
    let coord = vec3<u32>(wrapped * vec3<f32>(dims));
    return textureLoad(shape_noise, vec3<i32>(coord), 0);
}

fn phase_approximation(cos_theta: f32) -> f32 {
    let g = 0.58;
    let denom = max(1.0 + g * g - 2.0 * g * cos_theta, 0.08);
    return (1.0 - g * g) / pow(denom, 1.5);
}

@compute @workgroup_size(8, 8, 1)
fn raymarch_cloud_layer(@builtin(global_invocation_id) gid: vec3<u32>) {
    let size = cloud.view_size.xy;
    if (gid.x >= size.x || gid.y >= size.y) {
        return;
    }

    let uv = (vec2<f32>(gid.xy) + vec2<f32>(0.5)) / vec2<f32>(size);
    let jitter = blue_noise_jitter(gid.xy, cloud.quality.w, cloud.wind.w);
    let sky = sky_gradient(cloud, uv);
    let ray_origin = cloud.current_camera.xyz;
    let view_dir = reconstruct_world_ray(cloud, uv);
    let ray_segment = intersect_cloud_slab(cloud, ray_origin, view_dir);
    let weather_uv = ray_origin.xz / cloud.profile1.x + cloud.wind_history.xy;
    let weather = sample_weather(weather_uv);

    if (ray_segment.y <= ray_segment.x || weather.r < 0.015 || cloud.profile0.x < 0.015 || cloud.profile0.y <= 0.0) {
        textureStore(cloud_color, vec2<i32>(gid.xy), vec4<f32>(sky, 0.0));
        textureStore(cloud_transmittance, vec2<i32>(gid.xy), vec4<f32>(1.0, 0.0, weather.r, 1.0));
        return;
    }

    let primary_steps = max(cloud.quality.x, 1u);
    let visible_steps = min(primary_steps, 64u);
    let horizon_fade = smoothstep(-0.04, 0.24, view_dir.y);
    let sun_dir = normalize(cloud.sun.xyz);
    let phase = phase_approximation(dot(sun_dir, view_dir));
    let sun_color = mix(vec3<f32>(1.0, 0.92, 0.78), vec3<f32>(1.0, 0.78, 0.55), cloud.sky_horizon.a);
    let ambient = cloud.ambient.rgb;
    let cloud_base = cloud.profile0.z;
    let cloud_depth = max(cloud.profile0.w - cloud.profile0.z, 1.0);
    let segment_length = min(ray_segment.y - ray_segment.x, 200000.0);
    let weather_scale = max(cloud.profile1.x, 1000.0);

    var color = vec3<f32>(0.0);
    var transmittance = 1.0;
    var executed_steps = 0u;

    for (var i = 0u; i < 64u; i = i + 1u) {
        if (i >= visible_steps) {
            break;
        }

        let t = (f32(i) + jitter) / f32(visible_steps);
        let sample_distance = ray_segment.x + t * segment_length;
        let world_position = ray_origin + view_dir * sample_distance;
        let height = saturate((world_position.y - cloud_base) / cloud_depth);
        let height_mask = height_gradient(height, cloud.ambient.a);
        let sample_weather_uv = world_position.xz / weather_scale + cloud.wind_history.xy;
        let local_weather = sample_weather(sample_weather_uv);
        let motion = cloud.wind.xy * height * 0.13;
        let noise = sample_shape(vec3<f32>(sample_weather_uv * 3.2 + motion, height * 1.7 + cloud.wind.w));
        let body = saturate(noise.r * 0.7 + noise.g * 0.45 - noise.b * cloud.profile1.w * 0.38);
        let coverage_cut = saturate(local_weather.r - (1.0 - body) * (0.82 - cloud.profile1.y * 0.24));
        let density = coverage_cut
            * local_weather.a
            * cloud.profile0.y
            * height_mask
            * horizon_fade
            * mix(0.55, 1.35, local_weather.g);

        if (density > 0.002) {
            var light_density = 0.0;
            let light_steps = min(cloud.quality.y, 8u);
            for (var light_step = 0u; light_step < 8u; light_step = light_step + 1u) {
                if (light_step >= light_steps) {
                    break;
                }
                let light_world_position = world_position + sun_dir * (f32(light_step) + 1.0) * 650.0;
                let light_height = saturate((light_world_position.y - cloud_base) / cloud_depth);
                if (light_height > 0.0 && light_height < 1.0) {
                    let light_weather_uv = light_world_position.xz / weather_scale + cloud.wind_history.xy;
                    let light_weather = sample_weather(light_weather_uv);
                    let light_noise = sample_shape(vec3<f32>(light_weather_uv * 3.2 + motion, light_height * 1.7 + cloud.wind.w));
                    let light_body = saturate(light_noise.r * 0.7 + light_noise.g * 0.45 - light_noise.b * cloud.profile1.w * 0.38);
                    let light_cut = saturate(light_weather.r - (1.0 - light_body) * (0.82 - cloud.profile1.y * 0.24));
                    light_density = light_density + light_cut * light_weather.a * height_gradient(light_height, cloud.ambient.a);
                }
            }
            let light_shadow = light_density / max(f32(light_steps), 1.0);
            let local_shadow = exp(-(density * mix(0.7, 1.55, local_weather.b) + light_shadow * 0.42));
            let powder = 1.0 - exp(-density * 2.15);
            let lighting = ambient * 0.55 + sun_color * (0.22 + phase * 0.09 + powder * 0.33) * local_shadow;
            let alpha = saturate(1.0 - exp(-density * 0.18));
            color = color + transmittance * lighting * alpha;
            transmittance = transmittance * (1.0 - alpha);
        }

        executed_steps = executed_steps + 1u;
        if (transmittance < 0.02) {
            break;
        }
    }

    let cloud_alpha = saturate(1.0 - transmittance);
    let lit_sky = sky * transmittance + color;
    textureStore(cloud_color, vec2<i32>(gid.xy), vec4<f32>(lit_sky, cloud_alpha));
    textureStore(cloud_transmittance, vec2<i32>(gid.xy), vec4<f32>(
        transmittance,
        f32(executed_steps) / f32(primary_steps),
        weather.r,
        1.0,
    ));
}
