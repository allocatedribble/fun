struct CloudParams {
    view_size: vec4<u32>,
    quality: vec4<u32>,
    profile0: vec4<f32>,
    profile1: vec4<f32>,
    sky_zenith: vec4<f32>,
    sky_horizon: vec4<f32>,
    ambient: vec4<f32>,
    wind: vec4<f32>,
    sun: vec4<f32>,
    history: vec4<u32>,
    current_camera: vec4<f32>,
    previous_camera: vec4<f32>,
    wind_history: vec4<f32>,
    current_world_from_clip: mat4x4<f32>,
    previous_clip_from_world: mat4x4<f32>,
}

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

fn runtime_temporal_enabled(params: CloudParams) -> bool {
    return (params.quality.z & 1u) != 0u;
}

fn runtime_debug_mode(params: CloudParams) -> u32 {
    return (params.quality.z >> 8u) & 255u;
}

fn hash12(p: vec2<f32>) -> f32 {
    let q = vec3<f32>(p.x, p.y, p.x) * 0.1031;
    let r = fract(q);
    let s = r + dot(r, r.yzx + vec3<f32>(33.33));
    return fract((s.x + s.y) * s.z);
}

fn hash13(p: vec3<f32>) -> f32 {
    let q = fract(p * 0.1031);
    let r = q + dot(q, q.zyx + vec3<f32>(31.32));
    return fract((r.x + r.y) * r.z);
}

fn value_noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i + vec2<f32>(0.0, 0.0));
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn value_noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let n000 = hash13(i + vec3<f32>(0.0, 0.0, 0.0));
    let n100 = hash13(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash13(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash13(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash13(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash13(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash13(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash13(i + vec3<f32>(1.0, 1.0, 1.0));
    let x00 = mix(n000, n100, u.x);
    let x10 = mix(n010, n110, u.x);
    let x01 = mix(n001, n101, u.x);
    let x11 = mix(n011, n111, u.x);
    return mix(mix(x00, x10, u.y), mix(x01, x11, u.y), u.z);
}

fn fbm2(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var octave = 0u; octave < 5u; octave = octave + 1u) {
        value = value + value_noise2(p * frequency) * amplitude;
        frequency = frequency * 2.03;
        amplitude = amplitude * 0.5;
    }
    return value;
}

fn fbm3(p: vec3<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    for (var octave = 0u; octave < 4u; octave = octave + 1u) {
        value = value + value_noise3(p * frequency) * amplitude;
        frequency = frequency * 2.07;
        amplitude = amplitude * 0.5;
    }
    return value;
}

fn blue_noise_jitter(pixel: vec2<u32>, frame_index: u32, seed: f32) -> f32 {
    let p = vec2<f32>(pixel) + vec2<f32>(f32(frame_index & 1023u), seed * 4096.0);
    return hash12(p);
}

fn sky_gradient(params: CloudParams, uv: vec2<f32>) -> vec3<f32> {
    let horizon = saturate(pow(1.0 - abs(uv.y * 2.0 - 1.0), 0.72));
    let base = mix(params.sky_zenith.rgb, params.sky_horizon.rgb, horizon);
    let sun_warmth = saturate(params.sky_zenith.w / 120000.0);
    return base * mix(0.72, 1.12, sun_warmth);
}

fn height_gradient(height01: f32, edge_softness: f32) -> f32 {
    let base = smoothstep(0.02, 0.18 + edge_softness * 0.12, height01);
    let top = 1.0 - smoothstep(0.74 - edge_softness * 0.18, 1.0, height01);
    return saturate(base * top);
}

fn reconstruct_world_ray(params: CloudParams, uv: vec2<f32>) -> vec3<f32> {
    let clip = vec4<f32>(uv * 2.0 - vec2<f32>(1.0), 1.0, 1.0);
    let world = params.current_world_from_clip * clip;
    let world_position = world.xyz / max(abs(world.w), 0.00001);
    return normalize(world_position - params.current_camera.xyz);
}

fn reproject_world_to_uv(params: CloudParams, world_position: vec3<f32>) -> vec2<f32> {
    let previous_clip = params.previous_clip_from_world * vec4<f32>(world_position, 1.0);
    let previous_ndc = previous_clip.xyz / max(abs(previous_clip.w), 0.00001);
    return previous_ndc.xy * 0.5 + vec2<f32>(0.5);
}

fn intersect_cloud_slab(params: CloudParams, ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> vec2<f32> {
    let base = params.profile0.z;
    let top = max(params.profile0.w, base + 1.0);
    if (abs(ray_dir.y) < 0.0001) {
        return vec2<f32>(1.0, 0.0);
    }
    let t0 = (base - ray_origin.y) / ray_dir.y;
    let t1 = (top - ray_origin.y) / ray_dir.y;
    return vec2<f32>(max(min(t0, t1), 0.0), max(t0, t1));
}
