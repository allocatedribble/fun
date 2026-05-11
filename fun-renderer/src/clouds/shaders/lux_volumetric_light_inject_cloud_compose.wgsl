// fun-renderer / Pass C9.4 — typed Lux volumetric-light-
// inject cloud-compose WGSL.
//
// Bridges the typed CPU Pass C7.7
// `apply_cloud_layer_to_volumetric_scattering` contract
// into the typed GPU Lux volumetric-light-inject shader
// path.  The typed Lux volumetric shader imports/inlines
// these typed functions to attenuate typed directional
// scattering by typed cloud transmittance:
//
//     // Per froxel × light:
//     directional_scattering *= cloud_transmittance;
//
// Gated on typed `LuxLightKind::Directional` only — typed
// local lights (typed Punctual / Area /
// EmissiveCandidate / Probe) pass through unchanged.
// Mirrors typed C7.7
// `light_kind_attenuates_with_cloud_shadow` gating.
//
// Contract — typed cloud data is NEVER written into
// `LuxVirtualShadowPages` or `LuxShadowAtlas`.  This typed
// shader only READS from typed `CloudWorldShadowFiltered`
// + typed `CloudShadowProjectionConstants` + typed
// `LuxShadowAuxLayer` metadata uniform.

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
// CPU `LuxShadowAuxLayer`.  Identical layout to typed
// Pass C9.3 entry.  See
// `clouds/shaders/lux_direct_lighting_cloud_compose.wgsl`
// for typed field documentation.
// ============================================================================
struct LuxShadowAuxLayerEntry {
    header: vec4<u32>,
    knobs: vec4<f32>,
}

// ============================================================================
// LuxLightKind discriminant — typed shader mirror of the
// typed fun_lux `LuxLightKind` enum.  Values:
//
//   0 = Directional      (typed attenuates with cloud shadow)
//   1 = Punctual         (typed local; passes through)
//   2 = Area             (typed local; passes through)
//   3 = EmissiveCandidate(typed local; passes through)
//   4 = Probe            (typed local; passes through)
//
// Only typed `Directional` attenuates per typed Pass C7.7
// `light_kind_attenuates_with_cloud_shadow`.
// ============================================================================
const LUX_LIGHT_KIND_DIRECTIONAL: u32 = 0u;
const LUX_LIGHT_KIND_PUNCTUAL: u32 = 1u;
const LUX_LIGHT_KIND_AREA: u32 = 2u;
const LUX_LIGHT_KIND_EMISSIVE_CANDIDATE: u32 = 3u;
const LUX_LIGHT_KIND_PROBE: u32 = 4u;

// ============================================================================
// Bindings — typed canonical @group(3) placeholders the
// typed Lux volumetric-light-inject pipeline remaps via
// its typed `pipeline_layout`.  Matches the typed Pass
// C9.3 binding shape so the typed direct-lighting + typed
// volumetric paths share the typed same uniforms +
// texture views per frame.
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
// light_kind_attenuates_with_cloud_shadow
//
// Typed gating predicate per typed Pass C7.7 contract.
// Returns `true` only for typed `Directional` lights.
// Mirrors typed CPU
// `crate::lux_volumetric_cloud_layer::light_kind_attenuates_with_cloud_shadow`.
// ============================================================================
fn light_kind_attenuates_with_cloud_shadow(kind: u32) -> bool {
    return kind == LUX_LIGHT_KIND_DIRECTIONAL;
}

// ============================================================================
// sample_cloud_shadow_layer_at_froxel
//
// Samples the typed `CloudWorldShadowFiltered` texture at
// the typed froxel center projected into typed shadow UV
// space.  Identical math to typed Pass C9.3
// `sample_cloud_shadow_layer` (which samples at typed
// pixel-shading-point world position); reused here for
// typed froxel-center world position.  Returns typed
// 1.0 (typed neutral) for:
//
// - typed aux layer absent,
// - typed light_id mismatched,
// - typed shadow UV outside [0, 1].
//
// Otherwise returns typed sampled transmittance ×
// opacity-modulation per typed C7.4.3 / C7.4.4 semantics.
// ============================================================================
fn sample_cloud_shadow_layer_at_froxel(
    froxel_world_pos: vec3<f32>,
    light_id_lo: u32,
    light_id_hi: u32,
) -> f32 {
    if (!aux_layer_is_present()) {
        return 1.0;
    }
    if (!aux_layer_matches_light(light_id_lo, light_id_hi)) {
        return 1.0;
    }

    let world = vec4<f32>(froxel_world_pos, 1.0);
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
    return saturate(1.0 - (1.0 - raw_transmittance) * opacity);
}

// ============================================================================
// apply_cloud_layer_to_volumetric_scattering
//
// Typed top-level entry point.  The typed Lux volumetric-
// light-inject shader calls this typed function per
// typed froxel × typed light to attenuate the typed
// directional scattering contribution.  Local lights pass
// through unchanged per typed Pass C7.7 contract.
//
// Inputs:
// - `light_kind` — typed `LuxLightKind` discriminant
//   (0 = Directional).
// - `light_id_lo` / `light_id_hi` — typed packed
//   `LuxLightId`.
// - `froxel_world_pos` — typed froxel center in world
//   space.
// - `directional_scattering` — typed scalar scattering
//   contribution the typed renderer would inject before
//   typed cloud attenuation.
//
// Returns the typed attenuated scattering contribution
// (or the typed input directional_scattering when the
// typed light is typed local OR the typed aux layer is
// typed absent / mismatched).
// ============================================================================
fn apply_cloud_layer_to_volumetric_scattering(
    light_kind: u32,
    light_id_lo: u32,
    light_id_hi: u32,
    froxel_world_pos: vec3<f32>,
    directional_scattering: f32,
) -> f32 {
    // Typed local lights pass through unchanged — typed
    // local lights MUST NOT cast world-scale cloud
    // shadows per typed Pass C7.7 contract.
    if (!light_kind_attenuates_with_cloud_shadow(light_kind)) {
        return max(directional_scattering, 0.0);
    }
    let scattering = max(directional_scattering, 0.0);
    let cloud_transmittance = sample_cloud_shadow_layer_at_froxel(
        froxel_world_pos,
        light_id_lo,
        light_id_hi,
    );
    return scattering * cloud_transmittance;
}
