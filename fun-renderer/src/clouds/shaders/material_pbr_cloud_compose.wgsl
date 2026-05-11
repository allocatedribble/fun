// fun-renderer / Pass C9.5 — typed material / PBR cloud-
// compose WGSL.
//
// Bridges the typed CPU Pass C7.10
// `apply_cloud_layer_to_material_direct_lighting`
// contract into the typed GPU material lighting path.
// The typed material shader imports/inlines these typed
// functions to sample the typed cloud aux layer + compose
// with typed opaque Lux × typed material visibility:
//
//     // Per shading pixel × material kind:
//     final_direct = opaque_visibility * material_visibility * cloud_transmittance;
//     final_pixel  = final_direct + emissive + indirect;
//
// Material eligibility (typed user spec):
//
//     Receives cloud shadow:
//       Terrain         (typed kind 0)
//       StaticMesh      (typed kind 1)
//       DynamicMesh     (typed kind 2)
//       Foliage         (typed kind 3; typed softened)
//       Water           (typed kind 4)
//
//     Skips cloud shadow:
//       EmissiveOnly    (typed kind 5)
//       Unlit           (typed kind 6)
//       Skybox          (typed kind 7)
//
// Cloud shadows ONLY multiply into the typed direct
// lighting term.  The typed emissive + typed indirect
// contributions pass through unchanged so typed self-
// illumination and typed sky-bounce do NOT dim with
// cloud cover.
//
// Foliage softening: typed Foliage materials receive a
// typed softened cloud shadow (typed blend toward typed
// 1.0 by typed FOLIAGE_SOFTENING_FACTOR) so typed grass /
// leaves / instanced vegetation get typed dappled-light
// look rather than typed hard cloud-shadow edges.
//
// Contract — typed cloud data is NEVER written into
// `LuxVirtualShadowPages` or `LuxShadowAtlas`.

// ============================================================================
// CloudShadowProjectionConstants + LuxShadowAuxLayerEntry
// -- typed GPU mirrors shared with typed Pass C9.3 +
// C9.4.  See those WGSL files for typed field docs.
// ============================================================================
struct CloudShadowProjectionConstants {
    header: vec4<u32>,
    world_from_shadow_uv: mat4x4<f32>,
    shadow_uv_from_world: mat4x4<f32>,
    sun_dir_ws: vec4<f32>,
    slab: vec4<f32>,
    knobs: vec4<f32>,
}

struct LuxShadowAuxLayerEntry {
    header: vec4<u32>,
    knobs: vec4<f32>,
}

// ============================================================================
// CloudShadowMaterialKind discriminant — typed shader
// mirror of the typed CPU
// `crate::lux_material_cloud_layer::CloudShadowMaterialKind`
// enum.  Values match typed C7.10 enum index order.
// ============================================================================
const MATERIAL_KIND_TERRAIN: u32 = 0u;
const MATERIAL_KIND_STATIC_MESH: u32 = 1u;
const MATERIAL_KIND_DYNAMIC_MESH: u32 = 2u;
const MATERIAL_KIND_FOLIAGE: u32 = 3u;
const MATERIAL_KIND_WATER: u32 = 4u;
const MATERIAL_KIND_EMISSIVE_ONLY: u32 = 5u;
const MATERIAL_KIND_UNLIT: u32 = 6u;
const MATERIAL_KIND_SKYBOX: u32 = 7u;

// Typed Foliage softening factor — typed shader blends
// the typed sampled cloud transmittance toward typed 1.0
// (typed full sunlight) by this typed factor.  Typed
// FOLIAGE_SOFTENING_FACTOR = 0.7 means typed 30% of the
// typed cloud-shadow dimming is typed retained on typed
// foliage (typed dappled-light look).  Other material
// kinds use typed 1.0 (typed no softening; typed raw
// cloud passes through).
const FOLIAGE_SOFTENING_FACTOR: f32 = 0.7;

// ============================================================================
// Bindings — typed canonical @group(3) placeholders
// matching typed Pass C9.3 / C9.4 binding shape so the
// typed PBR material pipeline shares typed same uniforms
// + texture views with typed direct-lighting + typed
// volumetric paths.
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
// material_kind_receives_cloud_shadow
//
// Typed gating predicate per typed Pass C7.10 contract.
// Returns `true` for typed Terrain / StaticMesh /
// DynamicMesh / Foliage / Water; typed `false` for typed
// EmissiveOnly / Unlit / Skybox.
// ============================================================================
fn material_kind_receives_cloud_shadow(kind: u32) -> bool {
    return kind <= MATERIAL_KIND_WATER;
}

// ============================================================================
// material_softening_factor
//
// Typed per-material softening factor.  Typed Foliage
// returns typed FOLIAGE_SOFTENING_FACTOR (typed 0.7);
// other material kinds return typed 1.0 (typed no
// softening).  Used by typed
// `sample_cloud_shadow_for_material` to typed blend the
// typed sampled cloud transmittance toward typed 1.0
// (typed full sunlight) for typed Foliage.
// ============================================================================
fn material_softening_factor(kind: u32) -> f32 {
    if (kind == MATERIAL_KIND_FOLIAGE) {
        return FOLIAGE_SOFTENING_FACTOR;
    }
    return 1.0;
}

// ============================================================================
// sample_cloud_shadow_for_material
//
// Samples the typed cloud shadow layer at the typed
// world position with typed material-specific softening.
// Identical math to typed Pass C9.3
// `sample_cloud_shadow_layer` plus typed material kind
// gating + typed Foliage softening blend.
//
// Returns typed 1.0 (typed neutral) for:
// - typed material kinds that skip cloud shadows,
// - typed aux layer absent,
// - typed light_id mismatched,
// - typed shadow UV outside [0, 1].
//
// For typed Foliage, the typed sampled transmittance is
// typed blended toward typed 1.0 by typed
// `(1.0 - FOLIAGE_SOFTENING_FACTOR)` so the typed cloud
// shadow contribution is typed softened.
// ============================================================================
fn sample_cloud_shadow_for_material(
    world_pos: vec3<f32>,
    material_kind: u32,
    light_id_lo: u32,
    light_id_hi: u32,
) -> f32 {
    if (!material_kind_receives_cloud_shadow(material_kind)) {
        return 1.0;
    }
    if (!aux_layer_is_present()) {
        return 1.0;
    }
    if (!aux_layer_matches_light(light_id_lo, light_id_hi)) {
        return 1.0;
    }

    let world = vec4<f32>(world_pos, 1.0);
    let shadow_uv4 = shadow_projection.shadow_uv_from_world * world;
    let w = max(abs(shadow_uv4.w), 1e-5);
    let shadow_uv = vec2<f32>(shadow_uv4.x / w, shadow_uv4.z / w);

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
    let opacity = aux_layer_opacity();
    let opacity_modulated = saturate(1.0 - (1.0 - raw_transmittance) * opacity);

    // Typed material softening blend: typed final =
    // mix(1.0, opacity_modulated, softening).  Typed
    // softening = 1.0 → typed opacity_modulated passes
    // through.  Typed softening < 1.0 → typed blend
    // toward typed 1.0 (typed softer).
    let softening = material_softening_factor(material_kind);
    return mix(1.0, opacity_modulated, softening);
}

// ============================================================================
// apply_cloud_layer_to_material_direct_lighting
//
// Composes the typed PBR direct-lighting term with typed
// cloud transmittance.  Mirrors typed CPU
// `crate::lux_material_cloud_layer::apply_cloud_layer_to_material_direct_lighting`.
//
// Formula:
//     final_direct = opaque * material * cloud
//
// Inputs are typed clamped to typed [0, 1].
// ============================================================================
fn apply_cloud_layer_to_material_direct_lighting(
    opaque_lux_visibility: f32,
    material_visibility: f32,
    cloud_transmittance: f32,
) -> f32 {
    let opaque = saturate(opaque_lux_visibility);
    let material = saturate(material_visibility);
    let cloud = saturate(cloud_transmittance);
    return opaque * material * cloud;
}

// ============================================================================
// compose_pbr_pixel_with_cloud
//
// Top-level PBR pixel compose.  Multiplies typed cloud
// transmittance into the typed direct-lighting term ONLY.
// Typed emissive + typed indirect contributions pass
// through unchanged so:
//
// - typed self-illumination (emissive) does NOT dim with
//   cloud cover;
// - typed sky-bounce / GI (indirect) does NOT dim with
//   cloud cover (typed indirect lighting already
//   incorporates typed cloud-coverage attenuation at
//   the typed probe layer).
//
// Drives the typed user-spec acceptance "cloud shadows
// only affect direct lighting, not emissive or purely
// indirect output."
//
// Returns the typed final RGB pixel color.
// ============================================================================
fn compose_pbr_pixel_with_cloud(
    direct_lighting: vec3<f32>,
    emissive: vec3<f32>,
    indirect: vec3<f32>,
    cloud_transmittance: f32,
) -> vec3<f32> {
    let cloud = saturate(cloud_transmittance);
    let direct = max(direct_lighting, vec3<f32>(0.0));
    let em = max(emissive, vec3<f32>(0.0));
    let ind = max(indirect, vec3<f32>(0.0));
    return direct * cloud + em + ind;
}
