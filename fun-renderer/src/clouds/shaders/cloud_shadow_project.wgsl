// fun-renderer / Pass C7.4.3 — typed cloud shadow projection compute shader.
//
// Marches every typed shadow texel toward the sun through the typed cloud
// slab, accumulates optical depth via Beer-Lambert, and writes the typed
// transmittance into `CloudWorldShadowTransmittance`.  The typed shader is
// the typed first half of the C7.4 chain:
//
//     LuxCloudShadowProject  ← THIS SHADER
//       -> CloudWorldShadowTransmittance
//     LuxCloudShadowFilter
//       -> CloudWorldShadowFiltered
//     LuxCloudShadowRegisterLayer
//       -> consumed by LuxDirectLighting (Lux owns final shadow eval).
//
// Lux owns final shadow evaluation: this shader produces typed atmospheric
// transmittance (1.0 = no cloud shadow, 0.0 = fully occluded), NEVER typed
// opaque shadow depth.  It MUST NOT write to typed `LuxVirtualShadowPages`
// or typed `LuxShadowAtlas` — the bind layout intentionally exposes only
// `cloud_shadow_out` so the contract is enforced at the typed binding layer.

// ============================================================================
// CloudParams — mirrors the typed CPU `CloudParams` uniform shape used by
// the typed cloud raymarch.  Only the fields the projection shader reads
// are referenced; the rest are kept in the typed struct so the typed
// uniform buffer is binary-compatible across the typed cloud pass chain.
//
// Field layout (matches the typed CPU `fun_renderer::clouds::CloudParams`
// when uploaded):
//   - profile0.x = density scale
//   - profile0.y = coverage scale
//   - profile0.z = cloud_base altitude (meters)
//   - profile0.w = cloud_top altitude (meters)
//   - profile1.x = weather map world scale (meters per UV unit)
//   - profile1.w = detail noise scale
//   - sun.xyz   = world-space sun direction (normalized; matches the typed
//                 `CloudShadowProjectionConstants::sun_direction_ws` field)
//   - wind.xy   = wind drift in world units per second (xz projection)
//   - wind.w    = wind seed
//   - wind_history.xy = accumulated wind offset (matches the legacy
//                 `wind_history` field).
// ============================================================================
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

// ============================================================================
// CloudShadowProjectionConstants — typed GPU mirror of the CPU
// `fun_renderer::cloud_shadow::CloudShadowProjectionConstants`.  Field
// order matches the typed Rust struct; padding lines up matrices on
// 16-byte boundaries per WGSL std140-ish layout rules.
//
// Field meaning (see CPU doc for full contract):
//   - header.x     = (mode << 16) | schema_version
//   - header.y     = light_id low 32 bits  (LuxLightId is u64 — split into
//                    lo / hi so the typed packing is portable across
//                    backends that lack a typed u64 uniform)
//   - header.z     = light_id high 32 bits
//   - header.w     = frame_index
//   - world_from_shadow_uv = row-major 4x4 (multiply on the LEFT in WGSL
//                            since WGSL matrices are column-major by default —
//                            the typed CPU side uploads the typed transpose,
//                            so `m * v` here equals row-major-on-CPU * v).
//   - shadow_uv_from_world = row-major 4x4 inverse projection.
//   - sun_dir_ws.xyz       = normalized sun direction in world space.
//                            sun_dir_ws.w   reserved / 0.
//   - slab.x = cloud_base_meters
//   - slab.y = cloud_top_meters
//   - slab.z = max_distance_meters
//   - slab.w = 0 (reserved)
//   - knobs.x = opacity_scale  (0..=1)
//   - knobs.y = softness       (0..=1)
//   - knobs.z = 0 (reserved)
//   - knobs.w = 0 (reserved)
// ============================================================================
struct CloudShadowProjectionConstants {
    header: vec4<u32>,
    world_from_shadow_uv: mat4x4<f32>,
    shadow_uv_from_world: mat4x4<f32>,
    sun_dir_ws: vec4<f32>,
    slab: vec4<f32>,
    knobs: vec4<f32>,
}

// Bindings — typed per user spec (Pass C7.4.3).
@group(0) @binding(0) var<uniform> cloud: CloudParams;
@group(0) @binding(1) var<uniform> shadow_projection: CloudShadowProjectionConstants;
@group(0) @binding(2) var weather_map: texture_2d<f32>;
@group(0) @binding(3) var shape_noise: texture_3d<f32>;
@group(0) @binding(4) var cloud_shadow_out: texture_storage_2d<r16float, write>;

// ============================================================================
// Helpers — minimal self-contained set so the typed shader compiles without
// an external common module.  Naming mirrors the typed legacy
// `fun_render::sky::render::sky_common` helpers for readability.
// ============================================================================

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
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

fn sample_weather(uv: vec2<f32>) -> vec4<f32> {
    let dims = vec2<i32>(textureDimensions(weather_map));
    let coord = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.9999)) * vec2<f32>(dims));
    return textureLoad(weather_map, coord, 0);
}

fn sample_shape(uv: vec3<f32>) -> vec4<f32> {
    let dims = vec3<i32>(textureDimensions(shape_noise));
    let wrapped = fract(uv);
    let coord = vec3<i32>(wrapped * vec3<f32>(dims));
    return textureLoad(shape_noise, coord, 0);
}

// Height gradient — same shape as the typed legacy raymarch's
// `height_gradient`, simplified to a typed fixed edge softness for the
// shadow path (the typed `cloud.ambient.a` slot drives raymarch softness
// but is not threaded into the typed shadow projection per the user spec).
fn shadow_height_gradient(height01: f32) -> f32 {
    let base = smoothstep(0.02, 0.22, height01);
    let top = 1.0 - smoothstep(0.74, 1.0, height01);
    return saturate(base * top);
}

// Reconstruct typed world XZ on the typed cloud-base plane from a typed
// shadow UV using the typed `world_from_shadow_uv` matrix.  The typed
// shadow UV is in [0, 1]; the typed matrix produces the typed world-space
// receiver footprint.  Y is replaced with cloud_base since the typed
// shadow plane sits at the typed cloud-base altitude.
fn shadow_uv_to_world_receiver(uv: vec2<f32>) -> vec3<f32> {
    let v = vec4<f32>(uv, 0.0, 1.0);
    let world = shadow_projection.world_from_shadow_uv * v;
    let w = max(abs(world.w), 1e-5);
    return vec3<f32>(world.x / w, shadow_projection.slab.x, world.z / w);
}

// Predicate: does the typed projection constants record describe a typed
// fully-live projection?  Mirrors the typed Rust `projects_world_shadow`
// predicate so the typed shader can early-out cleanly when the typed CPU
// gate would have produced typed `DISABLED` constants.
fn projection_is_live() -> bool {
    let cloud_base = shadow_projection.slab.x;
    let cloud_top = shadow_projection.slab.y;
    let max_dist = shadow_projection.slab.z;
    let opacity = shadow_projection.knobs.x;
    let sun_norm_sq = dot(shadow_projection.sun_dir_ws.xyz, shadow_projection.sun_dir_ws.xyz);
    return cloud_top > cloud_base
        && cloud_base >= 0.0
        && max_dist > 0.0
        && opacity > 0.0
        && sun_norm_sq > 0.0;
}

// ============================================================================
// Entry point — one workgroup invocation per shadow texel.  Workgroup size
// matches the typed cloud raymarch (8x8x1) so the typed shadow target
// dispatches with `ceil(extent / 8)` workgroups.
// ============================================================================
@compute @workgroup_size(8, 8, 1)
fn project_cloud_shadow(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(cloud_shadow_out);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }

    let texel = vec2<i32>(gid.xy);

    // Typed early-out for the typed `DISABLED` projection constants.
    // Writes a typed `transmittance = 1.0` (no shadow) so downstream
    // consumers default to typed "no cloud shadow" without sampling stale
    // data.
    if (!projection_is_live()) {
        textureStore(cloud_shadow_out, texel, vec4<f32>(1.0, 0.0, 0.0, 1.0));
        return;
    }

    let extent = vec2<f32>(dims);
    let uv = (vec2<f32>(gid.xy) + vec2<f32>(0.5)) / extent;

    let receiver = shadow_uv_to_world_receiver(uv);
    let sun_dir = normalize(shadow_projection.sun_dir_ws.xyz);

    // March from typed receiver (at cloud-base altitude) along the typed
    // sun direction up through the typed cloud slab.  Step count is fixed
    // at 16 — typed quality knob can scale this later via the typed
    // `cloud.quality` field when the typed renderer wires per-tier
    // budgets.
    let cloud_base = shadow_projection.slab.x;
    let cloud_top = shadow_projection.slab.y;
    let slab_thickness = max(cloud_top - cloud_base, 1.0);

    // Distance along the sun ray to escape the typed cloud slab.  If the
    // typed sun is at the horizon the typed ray length explodes, so we
    // clamp to a typed reasonable maximum (the typed `max_distance_meters`).
    let max_dist = shadow_projection.slab.z;
    let sun_up = max(sun_dir.y, 0.05);
    let ray_length = min(slab_thickness / sun_up, max_dist);

    let steps = 16u;
    let step_length = ray_length / f32(steps);

    // Typed jitter per texel breaks slice banding without introducing
    // typed temporal crawl (frame_index seeds the typed hash so the
    // pattern advances by one texel each frame).
    let frame_index = shadow_projection.header.w;
    let jitter = hash12(vec2<f32>(gid.xy) + vec2<f32>(f32(frame_index & 1023u), 0.0));

    let weather_scale = max(cloud.profile1.x, 1000.0);
    let wind_offset = cloud.wind_history.xy;
    let density_scale = max(cloud.profile0.x, 0.0);
    let coverage_scale = max(cloud.profile0.y, 0.0);
    let detail_strength = saturate(cloud.profile1.w);

    var optical_depth = 0.0;
    var coverage_accum = 0.0;
    var coverage_weight = 0.0;

    for (var i = 0u; i < 16u; i = i + 1u) {
        let t = (f32(i) + jitter) / f32(steps);
        let world_position = receiver + sun_dir * (t * ray_length);
        let height = saturate((world_position.y - cloud_base) / slab_thickness);
        let height_mask = shadow_height_gradient(height);

        let weather_uv = world_position.xz / weather_scale + wind_offset;
        let weather = sample_weather(weather_uv);

        let motion = cloud.wind.xy * height * 0.13;
        let noise_uv = vec3<f32>(weather_uv * 3.2 + motion, height * 1.7 + cloud.wind.w);
        let noise = sample_shape(noise_uv);

        let body = saturate(noise.r * 0.7 + noise.g * 0.45 - noise.b * detail_strength * 0.38);
        let coverage = saturate(weather.r * coverage_scale);
        let coverage_cut = saturate(coverage - (1.0 - body) * 0.7);

        let density = coverage_cut
            * max(weather.a, 0.1)
            * density_scale
            * height_mask;

        optical_depth = optical_depth + max(density, 0.0) * step_length;

        // Aggregate typed cloud coverage along the typed sun ray for the
        // typed packed-format `coverage` channel.  Weight each sample by
        // the typed step length so the typed result is unit-normalized.
        coverage_accum = coverage_accum + coverage * step_length;
        coverage_weight = coverage_weight + step_length;
    }

    // Beer-Lambert: T = exp(-tau).  Clamp tau to a typed sane upper bound
    // so the typed `exp` does not collapse to a typed denormal on the
    // typed dense-storm path.
    let tau = min(optical_depth, 10.0);
    let raw_transmittance = exp(-tau);

    // Typed `opacity_scale` (0..=1) dims the typed cloud shadow effect:
    //   t = 1 - (1 - raw_t) * opacity_scale
    // opacity = 0 → typed transmittance always 1 (no shadow contribution).
    // opacity = 1 → typed raw transmittance passes through.
    let opacity = saturate(shadow_projection.knobs.x);
    let transmittance = saturate(1.0 - (1.0 - raw_transmittance) * opacity);

    // Typed full sample fields for the typed `Rgba16FloatPacked` variant.
    // The typed R16Float output binding only stores `.r`, so the typed
    // extra channels are still computed for the typed packed-output
    // variant of this shader (see `cloud_shadow_project_packed.wgsl` —
    // identical except for the typed binding format + the typed
    // `textureStore` payload).
    let coverage_out = coverage_accum / max(coverage_weight, 1e-5);
    let confidence = 1.0; // typed current-frame sample; typed history
                          // fallback path is owned by the typed filter
                          // shader's temporal reproject (later sub-pass).

    textureStore(
        cloud_shadow_out,
        texel,
        vec4<f32>(transmittance, tau, coverage_out, confidence),
    );
}
