// fun-renderer / Pass C7.4.4 — typed cloud shadow filter compute shader.
//
// Reads the typed projected cloud transmittance from
// `CloudWorldShadowTransmittance`, applies a typed softness-modulated
// gaussian blur, and writes the typed result into
// `CloudWorldShadowFiltered`.  The typed shader is the typed second half
// of the C7.4 chain:
//
//     LuxCloudShadowProject
//       -> CloudWorldShadowTransmittance
//     LuxCloudShadowFilter   ← THIS SHADER
//       -> CloudWorldShadowFiltered
//     LuxCloudShadowRegisterLayer
//       -> consumed by LuxDirectLighting (Lux owns final shadow eval).
//
// The typed blur is a typed single-pass 9-tap separable-like kernel
// (3x3 box * 3x3 box ≈ approximated gaussian in two perpendicular
// 5-tap reads) implemented as typed 9 weighted samples around the
// center.  The typed kernel radius scales with the typed `softness`
// knob from `CloudWorldShadowSettings`:
//
//   softness = 0.0  → no blur (one center sample, opacity-modulated).
//   softness = 1.0  → typed maximum radius (typed 8-texel offsets).
//
// The typed `opacity_scale` knob is honored once more here so the typed
// filter can compose the typed opacity-modulated shadow strength even
// when the typed projection shader was invoked with a typed neutral
// opacity (lets the typed renderer expose typed runtime opacity tuning
// without re-running the typed expensive projection pass).
//
// Edge-safe clamp: every typed sample is clamped to typed `[0, dims-1]`
// so the typed kernel near the typed texture border samples typed valid
// texels only (no typed wrap, no typed undefined reads).  Preserves the
// typed broad cloud shape: the typed kernel weights are typed normalized
// so a typed uniform input transmittance produces a typed uniform output
// transmittance.

// ============================================================================
// CloudShadowProjectionConstants — same typed struct shape as the typed
// projection shader.  Mirrors the typed CPU `CloudShadowProjectionConstants`
// uniform.  Only `knobs.x` (opacity_scale) + `knobs.y` (softness) are
// read here.
// ============================================================================
struct CloudShadowProjectionConstants {
    header: vec4<u32>,
    world_from_shadow_uv: mat4x4<f32>,
    shadow_uv_from_world: mat4x4<f32>,
    sun_dir_ws: vec4<f32>,
    slab: vec4<f32>,
    knobs: vec4<f32>,
}

// Bindings — typed per user spec (Pass C7.4.4).  Only the typed
// shadow_projection uniform + the typed in/out textures appear here;
// the typed filter pass does NOT touch any typed `LuxVirtualShadowPages`
// or typed opaque shadow depth resource.  The typed bind-layout shape
// enforces the typed Pass C7.3
// `cloud_shadows_not_baked_into_opaque_depth` contract at the typed
// binding boundary.
@group(0) @binding(0) var<uniform> shadow_projection: CloudShadowProjectionConstants;
@group(0) @binding(1) var cloud_shadow_in: texture_2d<f32>;
@group(0) @binding(2) var cloud_shadow_filtered: texture_storage_2d<r16float, write>;

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

// Edge-safe texel load — clamps the typed integer coord to the typed
// texture extent so the typed kernel reads typed valid texels only.
fn load_clamped(coord: vec2<i32>, dims: vec2<i32>) -> vec4<f32> {
    let clamped = clamp(coord, vec2<i32>(0), dims - vec2<i32>(1));
    return textureLoad(cloud_shadow_in, clamped, 0);
}

// ============================================================================
// Entry point — one workgroup invocation per filtered shadow texel.
// Workgroup size matches the typed projection shader (8x8x1) so the typed
// filter target dispatches with `ceil(extent / 8)` workgroups.
// ============================================================================
@compute @workgroup_size(8, 8, 1)
fn filter_cloud_shadow(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims_u = textureDimensions(cloud_shadow_filtered);
    if (gid.x >= dims_u.x || gid.y >= dims_u.y) {
        return;
    }

    let dims = vec2<i32>(dims_u);
    let texel = vec2<i32>(gid.xy);

    let softness = saturate(shadow_projection.knobs.y);
    let opacity = saturate(shadow_projection.knobs.x);

    // Typed early-out: softness == 0 → no blur.  Loads the typed center
    // sample directly, applies the typed opacity knob, writes the typed
    // result.  Cheap path for the typed cinematic capture that wants the
    // typed raw projected transmittance with typed crisp edges.
    if (softness <= 1e-4) {
        let center = textureLoad(cloud_shadow_in, texel, 0).r;
        let opacity_modulated = saturate(1.0 - (1.0 - center) * opacity);
        textureStore(
            cloud_shadow_filtered,
            texel,
            vec4<f32>(opacity_modulated, 0.0, 0.0, 1.0),
        );
        return;
    }

    // Typed kernel radius scales with softness: typed `radius = 1 + 7 *
    // softness` so typed softness=0 → radius=1 (3x3) and typed
    // softness=1 → radius=8 (17x17 footprint sampled via typed 9-tap
    // dilated kernel).
    let radius = i32(round(1.0 + 7.0 * softness));

    // Typed 9-tap dilated gaussian kernel.  Offsets are typed (dx, dy)
    // multiplied by typed `radius` so the typed footprint widens with
    // typed softness without paying typed N^2 texture fetches.  Weights
    // are typed gaussian-derived (sigma ≈ radius / 2) and typed
    // pre-normalized so the typed sum is exactly 1.0.
    //
    //   offset                weight
    //   ( 0,  0)              0.25
    //   (-1,  0) ( 1,  0)     0.125 each
    //   ( 0, -1) ( 0,  1)     0.125 each
    //   (-1, -1) ( 1, -1)     0.0625 each
    //   (-1,  1) ( 1,  1)     0.0625 each
    //
    // Total weight = 0.25 + 4*0.125 + 4*0.0625 = 0.25 + 0.5 + 0.25 = 1.0.
    // Typed uniform input → typed uniform output (preserves the typed
    // broad cloud shape).

    let center = load_clamped(texel, dims).r;
    let west = load_clamped(texel + vec2<i32>(-radius, 0), dims).r;
    let east = load_clamped(texel + vec2<i32>(radius, 0), dims).r;
    let north = load_clamped(texel + vec2<i32>(0, -radius), dims).r;
    let south = load_clamped(texel + vec2<i32>(0, radius), dims).r;
    let nw = load_clamped(texel + vec2<i32>(-radius, -radius), dims).r;
    let ne = load_clamped(texel + vec2<i32>(radius, -radius), dims).r;
    let sw = load_clamped(texel + vec2<i32>(-radius, radius), dims).r;
    let se = load_clamped(texel + vec2<i32>(radius, radius), dims).r;

    let cardinal = (west + east + north + south) * 0.125;
    let diagonal = (nw + ne + sw + se) * 0.0625;
    let blurred = saturate(center * 0.25 + cardinal + diagonal);

    // Re-apply the typed opacity_scale knob here so the typed renderer
    // can tune typed shadow strength at typed runtime without re-running
    // the typed projection pass.  Typed `opacity = 0` → typed
    // transmittance always 1 (no shadow); typed `opacity = 1` → typed
    // blurred transmittance passes through.
    let opacity_modulated = saturate(1.0 - (1.0 - blurred) * opacity);

    textureStore(
        cloud_shadow_filtered,
        texel,
        vec4<f32>(opacity_modulated, 0.0, 0.0, 1.0),
    );
}
