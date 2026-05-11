// fun-renderer / Pass C9.3 — typed Lux direct-lighting
// cloud-compose WGSL.
//
// Bridges the typed CPU Pass C7.6
// `apply_cloud_layer_to_direct_visibility` contract into
// the typed GPU Lux direct-lighting shader path.  The
// typed Lux direct-lighting shader imports / inlines
// these typed functions to sample the typed cloud aux
// layer + compose with typed opaque Lux visibility:
//
//     let opaque_visibility = sample_lux_shadow(...);
//     let cloud_transmittance = sample_cloud_shadow_layer(...);
//     let final_visibility = compose_final_direct_visibility(
//         opaque_visibility, cloud_transmittance);
//
// Contract — typed cloud data is NEVER written into
// `LuxVirtualShadowPages` or `LuxShadowAtlas`.  This typed
// shader only READS from typed `CloudWorldShadowFiltered`
// + `CloudShadowProjectionConstants` + the typed
// `LuxShadowAuxLayer` metadata uniform.  Bind layout
// shape enforces the typed contract at the typed binding
// boundary; the typed source-text audit at
// `lux_direct_lighting_cloud_shader.rs` cross-checks at
// the typed source layer.
//
// No-layer behavior: when the typed `aux_layer.flags`
// `present` bit is zero OR the typed `aux_layer.light_id`
// does not match the typed shading light, the typed
// `sample_cloud_shadow_layer` function returns typed
// `1.0` (typed neutral) so the typed
// `compose_final_direct_visibility` reduces to typed
// `opaque_visibility * 1.0 = opaque_visibility`.

// ============================================================================
// CloudShadowProjectionConstants — typed GPU mirror of
// the typed CPU `CloudShadowProjectionConstants` (matches
// the typed Pass C7.4.3 / C7.4.4 layout).
// ============================================================================
struct CloudShadowProjectionConstants {
    header: vec4<u32>,
    world_from_shadow_uv: mat4x4<f32>,
    shadow_uv_from_world: mat4x4<f32>,
    sun_dir_ws: vec4<f32>,
    slab: vec4<f32>,
    knobs: vec4<f32>,
}

// ============================================================================
// LuxShadowAuxLayerEntry — typed GPU mirror of the typed
// CPU `LuxShadowAuxLayer`.  Compact 32-byte layout:
//
//     header.x = light_id low 32 bits
//     header.y = light_id high 32 bits
//     header.z = kind discriminant (0 = CloudTransmittance)
//     header.w = flags
//                bit 0 = present (1 = typed aux layer
//                                 registered this frame)
//                bit 1 = samples_current_frame (typed
//                        SameFrame mode)
//                bit 2 = samples_previous_frame (typed
//                        OneFrameDelayed mode)
//     knobs.x = opacity   (typed Q16 → f32 in [0, 1])
//     knobs.y = softness  (typed Q16 → f32 in [0, 1])
//     knobs.z = reserved
//     knobs.w = reserved
// ============================================================================
struct LuxShadowAuxLayerEntry {
    header: vec4<u32>,
    knobs: vec4<f32>,
}

// ============================================================================
// Bindings — typed user spec.  Caller (typed Lux direct-
// lighting pass) picks the typed bind group + binding
// indices; these declarations show the typed canonical
// layout the typed Pass C9.3 entry-point expects.  The
// typed group / binding indices below are placeholders;
// the typed Lux direct-lighting pass remaps via its own
// `pipeline_layout`.
// ============================================================================
@group(3) @binding(0) var<uniform> shadow_projection: CloudShadowProjectionConstants;
@group(3) @binding(1) var cloud_shadow_filtered: texture_2d<f32>;
@group(3) @binding(2) var cloud_shadow_sampler: sampler;
@group(3) @binding(3) var<uniform> aux_layer: LuxShadowAuxLayerEntry;

// ============================================================================
// Helpers
// ============================================================================
fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

fn aux_layer_is_present() -> bool {
    return (aux_layer.header.w & 1u) != 0u;
}

fn aux_layer_matches_light(light_id_lo: u32, light_id_hi: u32) -> bool {
    return aux_layer.header.x == light_id_lo && aux_layer.header.y == light_id_hi;
}

fn aux_layer_opacity() -> f32 {
    return saturate(aux_layer.knobs.x);
}

// ============================================================================
// sample_cloud_shadow_layer
//
// Samples the typed `CloudWorldShadowFiltered` texture at
// the typed world position projected into typed shadow
// UV space via `shadow_projection.shadow_uv_from_world`.
// Returns typed `1.0` when:
//
// - The typed aux layer is NOT present (flags bit 0 = 0).
// - The typed aux layer's typed `light_id` does NOT match
//   the typed shading light.
// - The typed sampled UV is outside the typed `[0, 1]`
//   range (typed projection footprint exceeded).
//
// Otherwise returns the typed sampled transmittance
// modulated by the typed `aux_layer.knobs.x` (opacity).
// ============================================================================
fn sample_cloud_shadow_layer(
    world_position: vec3<f32>,
    light_id_lo: u32,
    light_id_hi: u32,
) -> f32 {
    if (!aux_layer_is_present()) {
        return 1.0;
    }
    if (!aux_layer_matches_light(light_id_lo, light_id_hi)) {
        return 1.0;
    }

    let world = vec4<f32>(world_position, 1.0);
    let shadow_uv4 = shadow_projection.shadow_uv_from_world * world;
    let w = max(abs(shadow_uv4.w), 1e-5);
    let shadow_uv = vec2<f32>(shadow_uv4.x / w, shadow_uv4.z / w);

    // Typed footprint check — outside the typed projection
    // bounds → typed neutral transmittance (no cloud
    // shadow contribution).
    if (shadow_uv.x < 0.0 || shadow_uv.x > 1.0
        || shadow_uv.y < 0.0 || shadow_uv.y > 1.0) {
        return 1.0;
    }

    let raw = textureSampleLevel(
        cloud_shadow_filtered,
        cloud_shadow_sampler,
        shadow_uv,
        0.0,
    );
    let raw_transmittance = saturate(raw.r);

    // Typed opacity knob composes per Pass C7.4.3 +
    // C7.4.4 semantics: typed opacity=0 → typed neutral
    // (no shadow), typed opacity=1 → typed raw passes
    // through.
    let opacity = aux_layer_opacity();
    return saturate(1.0 - (1.0 - raw_transmittance) * opacity);
}

// ============================================================================
// compose_final_direct_visibility
//
// Typed canonical product compose (matches typed CPU
// `LuxDirectLightShadowMath::compose_final_direct_visibility`):
//
//     final_visibility = opaque_visibility * cloud_transmittance
//
// Both inputs typed clamped to `[0, 1]`.
// ============================================================================
fn compose_final_direct_visibility(
    opaque_visibility: f32,
    cloud_transmittance: f32,
) -> f32 {
    let opaque = saturate(opaque_visibility);
    let cloud = saturate(cloud_transmittance);
    return opaque * cloud;
}

// ============================================================================
// apply_cloud_layer_to_direct_visibility
//
// Typed top-level Lux direct-lighting entry point.  The
// typed Lux direct-lighting shader calls this typed
// function per typed shading pixel to compose typed
// opaque Lux visibility with typed cloud transmittance.
//
// Mirrors typed CPU
// `crate::lux_direct_lighting_cloud_layer::apply_cloud_layer_to_direct_visibility`
// from Pass C7.6 — typed same compose formula, typed
// same neutral fallback when no typed aux layer is
// present.
// ============================================================================
fn apply_cloud_layer_to_direct_visibility(
    world_position: vec3<f32>,
    opaque_lux_visibility: f32,
    light_id_lo: u32,
    light_id_hi: u32,
) -> f32 {
    let cloud_transmittance = sample_cloud_shadow_layer(
        world_position,
        light_id_lo,
        light_id_hi,
    );
    return compose_final_direct_visibility(
        opaque_lux_visibility,
        cloud_transmittance,
    );
}
