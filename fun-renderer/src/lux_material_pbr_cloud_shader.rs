//! Pass C9.5 — typed material / PBR cloud-compose shader
//! scaffold.
//!
//! Pass C7.10 landed the typed CPU
//! `apply_cloud_layer_to_material_direct_lighting` contract.
//! Pass C9.5 lands the typed GPU shader path that mirrors
//! the typed CPU contract per shading pixel × material
//! kind:
//!
//!     final_direct = opaque_visibility
//!                  * material_visibility
//!                  * cloud_transmittance;
//!     final_pixel  = final_direct + emissive + indirect;
//!
//! Material eligibility (typed user spec):
//! - typed `Terrain` / `StaticMesh` / `DynamicMesh` /
//!   `Foliage` / `Water` receive typed cloud shadows;
//! - typed `EmissiveOnly` / `Unlit` / `Skybox` skip typed
//!   cloud shadows.
//!
//! Cloud shadows ONLY multiply into the typed direct
//! lighting term.  The typed emissive + typed indirect
//! contributions pass through unchanged so typed self-
//! illumination and typed sky-bounce do NOT dim with cloud
//! cover.
//!
//! Foliage softening: typed Foliage materials receive a
//! typed softened cloud shadow (typed blend toward typed
//! 1.0 by typed `FOLIAGE_SOFTENING_FACTOR`) so typed grass
//! / leaves / instanced vegetation get typed dappled-light
//! look rather than typed hard cloud-shadow edges.
//!
//! The typed WGSL source lives at
//! `clouds/shaders/material_pbr_cloud_compose.wgsl` and
//! exposes the typed top-level
//! `apply_cloud_layer_to_material_direct_lighting` entry
//! point along with the typed `sample_cloud_shadow_for_material`
//! and typed `compose_pbr_pixel_with_cloud` helpers that
//! the typed material lighting shader imports/inlines.
//!
//! This Rust module:
//! - exposes the typed WGSL source as a typed const string
//!   + typed entry-point names;
//! - declares the typed `CloudShadowMaterialKindGpu`
//!   discriminant that the typed WGSL reads;
//! - mirrors the typed `FOLIAGE_SOFTENING_FACTOR`;
//! - provides typed CPU simulators the typed tests
//!   cross-check against the typed Pass C7.10 reference
//!   path;
//! - audits typed source-text contracts ("no writes to
//!   `LuxVirtualShadowPages` / `LuxShadowAtlas`", no typed
//!   forbidden identifiers, no typed `textureStore` calls).

use crate::lux_direct_lighting_cloud_shader::{
    LuxShadowAuxLayerGpu, simulate_sample_cloud_shadow_layer,
};
use crate::lux_material_cloud_layer::CloudShadowMaterialKind;
use fun_lux::LuxLightId;

pub const FUN_RENDERER_LUX_MATERIAL_PBR_CLOUD_SHADER_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C9.5 — typed WGSL source for the typed
/// material / PBR cloud-compose helper functions.
pub const LUX_MATERIAL_PBR_CLOUD_COMPOSE_WGSL: &str =
    include_str!("clouds/shaders/material_pbr_cloud_compose.wgsl");

/// Typed Pass C9.5 — typed top-level entry-point function
/// name in the typed WGSL source.  The typed material
/// shader calls this typed function per shading pixel ×
/// material kind to attenuate the typed direct-lighting
/// term by typed cloud transmittance.
pub const LUX_MATERIAL_PBR_CLOUD_COMPOSE_ENTRY_POINT: &str =
    "apply_cloud_layer_to_material_direct_lighting";

/// Typed Pass C9.5 — typed helper function names exposed
/// by the typed WGSL source.
pub const LUX_MATERIAL_PBR_CLOUD_SAMPLE_FN: &str = "sample_cloud_shadow_for_material";
pub const LUX_MATERIAL_PBR_CLOUD_PIXEL_COMPOSE_FN: &str = "compose_pbr_pixel_with_cloud";
pub const LUX_MATERIAL_PBR_KIND_GATE_FN: &str = "material_kind_receives_cloud_shadow";
pub const LUX_MATERIAL_PBR_SOFTENING_FN: &str = "material_softening_factor";

/// Typed Pass C9.5 — typed Foliage softening factor.  Typed
/// shader mirror of the typed WGSL `FOLIAGE_SOFTENING_FACTOR`
/// constant.  Typed 0.7 means typed 30% of the typed cloud-
/// shadow dimming is typed retained on typed foliage
/// (typed dappled-light look); typed other material kinds
/// use typed 1.0 (typed no softening).
pub const FOLIAGE_SOFTENING_FACTOR: f32 = 0.7;

// ============================================================================
// Section 1 — typed CloudShadowMaterialKindGpu discriminant
// ============================================================================

/// Typed Pass C9.5 — typed GPU discriminant for the typed
/// `CloudShadowMaterialKind` enum.  Matches the typed WGSL
/// `MATERIAL_KIND_*` constants.
///
/// Value mapping:
///   0 = Terrain          (typed receives cloud shadow)
///   1 = StaticMesh       (typed receives cloud shadow)
///   2 = DynamicMesh      (typed receives cloud shadow)
///   3 = Foliage          (typed receives; typed softened)
///   4 = Water            (typed receives cloud shadow)
///   5 = EmissiveOnly     (typed skips cloud shadow)
///   6 = Unlit            (typed skips cloud shadow)
///   7 = Skybox           (typed skips cloud shadow)
#[repr(u32)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CloudShadowMaterialKindGpu {
    #[default]
    Terrain = 0,
    StaticMesh = 1,
    DynamicMesh = 2,
    Foliage = 3,
    Water = 4,
    EmissiveOnly = 5,
    Unlit = 6,
    Skybox = 7,
}

impl CloudShadowMaterialKindGpu {
    pub const ALL: [Self; 8] = [
        Self::Terrain,
        Self::StaticMesh,
        Self::DynamicMesh,
        Self::Foliage,
        Self::Water,
        Self::EmissiveOnly,
        Self::Unlit,
        Self::Skybox,
    ];

    /// Typed Pass C9.5 — typed CPU → typed GPU mapping.
    /// Matches the typed WGSL discriminant table.
    #[must_use]
    pub const fn from_cpu(kind: CloudShadowMaterialKind) -> Self {
        match kind {
            CloudShadowMaterialKind::Terrain => Self::Terrain,
            CloudShadowMaterialKind::StaticMesh => Self::StaticMesh,
            CloudShadowMaterialKind::DynamicMesh => Self::DynamicMesh,
            CloudShadowMaterialKind::Foliage => Self::Foliage,
            CloudShadowMaterialKind::Water => Self::Water,
            CloudShadowMaterialKind::EmissiveOnly => Self::EmissiveOnly,
            CloudShadowMaterialKind::Unlit => Self::Unlit,
            CloudShadowMaterialKind::Skybox => Self::Skybox,
        }
    }

    /// Typed Pass C9.5 — typed GPU u32 representation.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Typed Pass C9.5 — typed predicate: does this typed
    /// kind receive typed cloud shadows?  Mirrors the typed
    /// WGSL `material_kind_receives_cloud_shadow` predicate
    /// (typed kind <= MATERIAL_KIND_WATER).
    #[must_use]
    pub const fn receives_cloud_shadow(self) -> bool {
        matches!(
            self,
            Self::Terrain
                | Self::StaticMesh
                | Self::DynamicMesh
                | Self::Foliage
                | Self::Water,
        )
    }

    /// Typed Pass C9.5 — typed per-material softening
    /// factor.  Typed `Foliage` returns typed
    /// `FOLIAGE_SOFTENING_FACTOR`; typed other material
    /// kinds return typed 1.0.
    #[must_use]
    pub const fn softening_factor(self) -> f32 {
        match self {
            Self::Foliage => FOLIAGE_SOFTENING_FACTOR,
            _ => 1.0,
        }
    }
}

// ============================================================================
// Section 2 — typed CPU simulators (mirror WGSL)
// ============================================================================

/// Typed Pass C9.5 — typed CPU simulator of the typed
/// `sample_cloud_shadow_for_material` WGSL function.
/// Builds on the typed Pass C9.3 sample simulator + typed
/// material kind gating + typed Foliage softening blend.
///
/// Inputs:
/// - `aux_layer_gpu` — typed packed aux-layer uniform
///   the typed shader reads.
/// - `material_kind_gpu` — typed material kind
///   discriminant.
/// - `light_id` — typed shading light id.
/// - `shadow_uv_from_world` — typed 4×4 row-major matrix
///   from the typed projection constants.
/// - `world_position` — typed shading-pixel world XYZ.
/// - `sample_cloud_transmittance` — typed callback that
///   simulates the typed `textureSampleLevel`.
///
/// Returns the typed cloud transmittance the typed WGSL
/// function would produce.
#[must_use]
pub fn simulate_sample_cloud_shadow_for_material<F>(
    aux_layer_gpu: &LuxShadowAuxLayerGpu,
    material_kind_gpu: CloudShadowMaterialKindGpu,
    light_id: LuxLightId,
    shadow_uv_from_world: &[[f32; 4]; 4],
    world_position: [f32; 3],
    sample_cloud_transmittance: F,
) -> f32
where
    F: FnMut([f32; 2]) -> Option<f32>,
{
    // Typed material kinds that skip cloud shadow → typed
    // neutral 1.0.
    if !material_kind_gpu.receives_cloud_shadow() {
        return 1.0;
    }
    let opacity_modulated = simulate_sample_cloud_shadow_layer(
        aux_layer_gpu,
        light_id,
        shadow_uv_from_world,
        world_position,
        sample_cloud_transmittance,
    );
    // Typed material softening blend: typed Foliage blends
    // toward typed 1.0 by typed (1 - FOLIAGE_SOFTENING_FACTOR);
    // typed other kinds use typed softening = 1.0 → typed
    // opacity_modulated passes through.
    let softening = material_kind_gpu.softening_factor();
    mix(1.0, opacity_modulated, softening)
}

/// Typed Pass C9.5 — typed CPU simulator of the typed
/// `apply_cloud_layer_to_material_direct_lighting` WGSL
/// function.  Mirrors typed CPU
/// `crate::lux_material_cloud_layer::apply_cloud_layer_to_material_direct_lighting`
/// but driven from typed shader-side inputs (typed scalar
/// visibilities + typed cloud transmittance, all clamped
/// to typed `[0, 1]`).
///
/// Formula:
///     final_direct = opaque × material × cloud
#[must_use]
pub fn simulate_apply_cloud_layer_to_material_direct_lighting(
    opaque_lux_visibility: f32,
    material_visibility: f32,
    cloud_transmittance: f32,
) -> f32 {
    let opaque = opaque_lux_visibility.clamp(0.0, 1.0);
    let material = material_visibility.clamp(0.0, 1.0);
    let cloud = cloud_transmittance.clamp(0.0, 1.0);
    opaque * material * cloud
}

/// Typed Pass C9.5 — typed CPU simulator of the typed
/// `compose_pbr_pixel_with_cloud` WGSL function.  Mirrors
/// the typed top-level PBR pixel compose:
///
///     final_pixel = direct × cloud + emissive + indirect
///
/// Typed emissive + typed indirect bypass cloud
/// attenuation per typed user-spec acceptance
/// "cloud shadows only affect direct lighting, not
/// emissive or purely indirect output."
#[must_use]
pub fn simulate_compose_pbr_pixel_with_cloud(
    direct_lighting: [f32; 3],
    emissive: [f32; 3],
    indirect: [f32; 3],
    cloud_transmittance: f32,
) -> [f32; 3] {
    let cloud = cloud_transmittance.clamp(0.0, 1.0);
    let direct = [
        direct_lighting[0].max(0.0),
        direct_lighting[1].max(0.0),
        direct_lighting[2].max(0.0),
    ];
    let em = [
        emissive[0].max(0.0),
        emissive[1].max(0.0),
        emissive[2].max(0.0),
    ];
    let ind = [
        indirect[0].max(0.0),
        indirect[1].max(0.0),
        indirect[2].max(0.0),
    ];
    [
        direct[0] * cloud + em[0] + ind[0],
        direct[1] * cloud + em[1] + ind[1],
        direct[2] * cloud + em[2] + ind[2],
    ]
}

/// Typed Pass C9.5 helper — typed `mix(a, b, t)` matching
/// the typed WGSL `mix(1.0, opacity_modulated, softening)`
/// blend.
#[inline]
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

// ============================================================================
// Section 3 — typed source-text audit predicates
// ============================================================================

/// Typed Pass C9.5 — typed predicate: the typed WGSL
/// source does NOT reference typed Lux opaque shadow
/// write paths.  Audits the typed contract "No cloud data
/// is written into `LuxVirtualShadowPages` or
/// `LuxShadowAtlas`."
#[must_use]
pub fn lux_material_pbr_cloud_compose_does_not_write_opaque_shadow() -> bool {
    let src = LUX_MATERIAL_PBR_CLOUD_COMPOSE_WGSL;
    let forbidden = [
        "LuxVirtualShadowPages",
        "LuxShadowAtlas",
        "lux_virtual_shadow_pages",
        "lux_shadow_atlas",
    ];
    for raw_line in src.lines() {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        for needle in forbidden {
            if trimmed.contains(needle) {
                return false;
            }
        }
    }
    true
}

/// Typed Pass C9.5 — typed predicate: the typed shader has
/// zero typed `textureStore(` calls (typed shader is typed
/// pure-read).
#[must_use]
pub fn lux_material_pbr_cloud_compose_has_no_texture_stores() -> bool {
    let src = LUX_MATERIAL_PBR_CLOUD_COMPOSE_WGSL;
    for raw_line in src.lines() {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.contains("textureStore(") {
            return false;
        }
    }
    true
}

/// Typed Pass C9.5 — typed predicate: the typed WGSL
/// source has the typed canonical contract shape:
/// - typed top-level compose entry point;
/// - typed per-material sampling fn;
/// - typed per-pixel pbr-compose fn;
/// - typed material-kind gate;
/// - typed Foliage softening blend.
#[must_use]
pub fn lux_material_pbr_cloud_compose_carries_user_spec_shape() -> bool {
    let src = LUX_MATERIAL_PBR_CLOUD_COMPOSE_WGSL;
    src.contains("fn apply_cloud_layer_to_material_direct_lighting")
        && src.contains("fn sample_cloud_shadow_for_material")
        && src.contains("fn compose_pbr_pixel_with_cloud")
        && src.contains("fn material_kind_receives_cloud_shadow")
        && src.contains("fn material_softening_factor")
        && src.contains("const FOLIAGE_SOFTENING_FACTOR")
        && src.contains("return opaque * material * cloud;")
        && src.contains("return direct * cloud + em + ind;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_shadow::{CloudShadowFrameDelayMode, CloudShadowProjectionConstants};
    use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};
    use crate::lux_material_cloud_layer::{
        CloudShadowMaterialKind, apply_cloud_layer_to_material_direct_lighting,
    };
    use crate::lux_shadow_aux_layer::{LuxShadowAuxLayerRegistry, register_cloud_shadow_aux_layer};

    fn registry_with_layer(
        light_id: LuxLightId,
        latency: CloudShadowFrameDelayMode,
    ) -> LuxShadowAuxLayerRegistry {
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let constants = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            light_id,
            [0.0, 1.0, 0.0],
            0,
        );
        register_cloud_shadow_aux_layer(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            latency,
            &mut registry,
        )
        .expect("registration");
        registry
    }

    fn live_projection_matrix(light_id: LuxLightId) -> [[f32; 4]; 4] {
        let constants = CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            light_id,
            [0.0, 1.0, 0.0],
            0,
        );
        constants.shadow_uv_from_world
    }

    /// Pass C9.5 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(
            FUN_RENDERER_LUX_MATERIAL_PBR_CLOUD_SHADER_SCHEMA_VERSION,
            1,
        );
        // Typed Foliage softening factor matches WGSL.
        assert!((FOLIAGE_SOFTENING_FACTOR - 0.7).abs() < 1e-6);
    }

    /// Pass C9.5 — typed `CloudShadowMaterialKindGpu`
    /// discriminants match the typed WGSL `MATERIAL_KIND_*`
    /// constants.
    #[test]
    fn material_kind_gpu_discriminants_match_wgsl() {
        assert_eq!(CloudShadowMaterialKindGpu::Terrain.as_u32(), 0);
        assert_eq!(CloudShadowMaterialKindGpu::StaticMesh.as_u32(), 1);
        assert_eq!(CloudShadowMaterialKindGpu::DynamicMesh.as_u32(), 2);
        assert_eq!(CloudShadowMaterialKindGpu::Foliage.as_u32(), 3);
        assert_eq!(CloudShadowMaterialKindGpu::Water.as_u32(), 4);
        assert_eq!(CloudShadowMaterialKindGpu::EmissiveOnly.as_u32(), 5);
        assert_eq!(CloudShadowMaterialKindGpu::Unlit.as_u32(), 6);
        assert_eq!(CloudShadowMaterialKindGpu::Skybox.as_u32(), 7);
        // Typed CPU → GPU mapping.
        assert_eq!(
            CloudShadowMaterialKindGpu::from_cpu(CloudShadowMaterialKind::Terrain),
            CloudShadowMaterialKindGpu::Terrain,
        );
        assert_eq!(
            CloudShadowMaterialKindGpu::from_cpu(CloudShadowMaterialKind::Skybox),
            CloudShadowMaterialKindGpu::Skybox,
        );
        // Typed receiver predicates.
        for kind in [
            CloudShadowMaterialKindGpu::Terrain,
            CloudShadowMaterialKindGpu::StaticMesh,
            CloudShadowMaterialKindGpu::DynamicMesh,
            CloudShadowMaterialKindGpu::Foliage,
            CloudShadowMaterialKindGpu::Water,
        ] {
            assert!(kind.receives_cloud_shadow(), "{:?}", kind);
        }
        for kind in [
            CloudShadowMaterialKindGpu::EmissiveOnly,
            CloudShadowMaterialKindGpu::Unlit,
            CloudShadowMaterialKindGpu::Skybox,
        ] {
            assert!(!kind.receives_cloud_shadow(), "{:?}", kind);
        }
    }

    /// Pass C9.5 acceptance — typed shader source carries
    /// the typed user-spec contract shape.
    #[test]
    fn shader_source_carries_user_spec_shape() {
        let src = LUX_MATERIAL_PBR_CLOUD_COMPOSE_WGSL;
        assert!(!src.is_empty());
        assert!(lux_material_pbr_cloud_compose_carries_user_spec_shape());
        // Typed material kind discriminant constants.
        assert!(src.contains("const MATERIAL_KIND_TERRAIN: u32 = 0u;"));
        assert!(src.contains("const MATERIAL_KIND_STATIC_MESH: u32 = 1u;"));
        assert!(src.contains("const MATERIAL_KIND_DYNAMIC_MESH: u32 = 2u;"));
        assert!(src.contains("const MATERIAL_KIND_FOLIAGE: u32 = 3u;"));
        assert!(src.contains("const MATERIAL_KIND_WATER: u32 = 4u;"));
        assert!(src.contains("const MATERIAL_KIND_EMISSIVE_ONLY: u32 = 5u;"));
        assert!(src.contains("const MATERIAL_KIND_UNLIT: u32 = 6u;"));
        assert!(src.contains("const MATERIAL_KIND_SKYBOX: u32 = 7u;"));
        // Typed bindings (shared with C9.3 / C9.4).
        assert!(src.contains("var<uniform> shadow_projection: CloudShadowProjectionConstants"));
        assert!(src.contains("var cloud_shadow_filtered: texture_2d<f32>"));
        assert!(src.contains("var<uniform> aux_layer: LuxShadowAuxLayerEntry"));
        // Typed Foliage softening constant.
        assert!(src.contains("const FOLIAGE_SOFTENING_FACTOR: f32 = 0.7;"));
    }

    /// Pass C9.5 acceptance — no cloud data is written
    /// into `LuxVirtualShadowPages` or `LuxShadowAtlas`.
    #[test]
    fn no_cloud_data_written_to_lux_virtual_shadow_or_atlas() {
        assert!(lux_material_pbr_cloud_compose_does_not_write_opaque_shadow());
        assert!(lux_material_pbr_cloud_compose_has_no_texture_stores());
    }

    /// Pass C9.5 acceptance — terrain receives cloud
    /// shadows.
    #[test]
    fn terrain_receives_cloud_shadows() {
        let light_id = LuxLightId::new(42);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);

        // Typed sample = 0.4 (dim cloud) at typed terrain
        // pixel.  Typed Terrain has typed softening = 1.0
        // (no softening), so typed transmittance matches
        // typed sample × opacity-modulation directly.
        let cloud = simulate_sample_cloud_shadow_for_material(
            &aux,
            CloudShadowMaterialKindGpu::Terrain,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            |_uv| Some(0.4),
        );
        // Typed terrain at typed dim cloud → typed cloud <
        // typed 1.0 (typed darkening applied).
        assert!(cloud < 1.0, "terrain cloud transmittance: {}", cloud);

        // Typed final = typed opaque × typed material ×
        // typed cloud — typed final dimmer than typed
        // opaque × typed material baseline.
        let final_direct = simulate_apply_cloud_layer_to_material_direct_lighting(1.0, 1.0, cloud);
        assert!(final_direct < 1.0, "terrain final: {}", final_direct);
        assert!((final_direct - cloud).abs() < 1e-6);
    }

    /// Pass C9.5 acceptance — static and dynamic meshes
    /// receive cloud shadows.
    #[test]
    fn static_and_dynamic_meshes_receive_cloud_shadows() {
        let light_id = LuxLightId::new(7);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);

        for kind in [
            CloudShadowMaterialKindGpu::StaticMesh,
            CloudShadowMaterialKindGpu::DynamicMesh,
            CloudShadowMaterialKindGpu::Water,
        ] {
            let cloud = simulate_sample_cloud_shadow_for_material(
                &aux,
                kind,
                light_id,
                &matrix,
                [0.0, 0.0, 0.0],
                |_uv| Some(0.3),
            );
            assert!(cloud < 1.0, "{:?} cloud: {}", kind, cloud);
            // Typed receiving kinds use typed softening =
            // 1.0; their typed transmittance equals typed
            // sample × opacity-modulation.
            let opacity = aux.opacity();
            let expected_modulated = (1.0 - (1.0 - 0.3) * opacity).clamp(0.0, 1.0);
            assert!(
                (cloud - expected_modulated).abs() < 1e-5,
                "{:?} cloud={} expected={}",
                kind,
                cloud,
                expected_modulated,
            );
        }
    }

    /// Pass C9.5 acceptance — foliage receives softened
    /// cloud shadows.
    #[test]
    fn foliage_receives_softened_cloud_shadows() {
        let light_id = LuxLightId::new(11);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);

        // Typed dense cloud sample.
        let dense_sample = 0.2;

        // Typed terrain (no softening) — typed
        // transmittance = typed opacity_modulated.
        let terrain_cloud = simulate_sample_cloud_shadow_for_material(
            &aux,
            CloudShadowMaterialKindGpu::Terrain,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            |_uv| Some(dense_sample),
        );

        // Typed foliage (softened) — typed transmittance
        // = mix(1.0, opacity_modulated, 0.7).
        let foliage_cloud = simulate_sample_cloud_shadow_for_material(
            &aux,
            CloudShadowMaterialKindGpu::Foliage,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            |_uv| Some(dense_sample),
        );

        // Typed foliage typed lighter than typed terrain
        // under typed dense cloud (typed softening blends
        // toward typed 1.0).
        assert!(
            foliage_cloud > terrain_cloud,
            "foliage={} terrain={}",
            foliage_cloud,
            terrain_cloud,
        );
        // Typed both still darken below typed full
        // sunlight.
        assert!(foliage_cloud < 1.0);
        assert!(terrain_cloud < 1.0);
        // Typed exact softening formula.
        let expected = mix(1.0, terrain_cloud, FOLIAGE_SOFTENING_FACTOR);
        assert!(
            (foliage_cloud - expected).abs() < 1e-5,
            "foliage={} expected={}",
            foliage_cloud,
            expected,
        );
    }

    /// Pass C9.5 acceptance — emissive / unlit / skybox
    /// do not receive cloud shadows.
    #[test]
    fn emissive_unlit_skybox_do_not_receive_cloud_shadows() {
        let light_id = LuxLightId::new(13);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);

        // Typed every skipping kind returns typed neutral
        // 1.0 EVEN with typed matching aux layer + typed
        // dense cloud sample.
        for kind in [
            CloudShadowMaterialKindGpu::EmissiveOnly,
            CloudShadowMaterialKindGpu::Unlit,
            CloudShadowMaterialKindGpu::Skybox,
        ] {
            let cloud = simulate_sample_cloud_shadow_for_material(
                &aux,
                kind,
                light_id,
                &matrix,
                [0.0, 0.0, 0.0],
                |_uv| Some(0.1), // dense cloud sample
            );
            assert!(
                (cloud - 1.0).abs() < 1e-6,
                "{:?} cloud transmittance: {}",
                kind,
                cloud,
            );
            // Typed final = typed opaque × typed material
            // × typed 1.0 = typed opaque × typed material
            // (typed no cloud darkening).
            let final_direct = simulate_apply_cloud_layer_to_material_direct_lighting(1.0, 1.0, cloud);
            assert!((final_direct - 1.0).abs() < 1e-6, "{:?}", kind);
        }
    }

    /// Pass C9.5 acceptance — PBR path matches the C7.6
    /// direct-lighting debug path within tolerance.
    #[test]
    fn pbr_path_matches_c7_6_debug_path_within_tolerance() {
        let light_id = LuxLightId::new(42);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);

        // Typed full grid of typed receiver kinds × typed
        // inputs.
        for kind_cpu in [
            CloudShadowMaterialKind::Terrain,
            CloudShadowMaterialKind::StaticMesh,
            CloudShadowMaterialKind::DynamicMesh,
            CloudShadowMaterialKind::Water,
        ] {
            for opaque in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for material in [0.0, 0.3, 0.6, 1.0] {
                    for cloud in [0.0, 0.5, 1.0] {
                        let cpu_compose = apply_cloud_layer_to_material_direct_lighting(
                            &registry, kind_cpu, light_id, opaque, material, cloud,
                        );
                        let shader_final = simulate_apply_cloud_layer_to_material_direct_lighting(
                            opaque,
                            material,
                            cpu_compose.cloud_transmittance,
                        );
                        // Typed shader's typed scalar path
                        // and typed CPU PBR path produce
                        // typed identical results.
                        assert!(
                            (shader_final - cpu_compose.final_visibility).abs() < 1e-5,
                            "{:?} opaque={} material={} cloud={} shader={} cpu={}",
                            kind_cpu,
                            opaque,
                            material,
                            cloud,
                            shader_final,
                            cpu_compose.final_visibility,
                        );
                        assert!(cpu_compose.pbr_and_debug_paths_agree());
                    }
                }
            }
        }
    }

    /// Pass C9.5 acceptance — cloud shadows only affect
    /// direct lighting, not emissive or purely indirect
    /// output.
    #[test]
    fn cloud_shadows_only_affect_direct_lighting() {
        // Typed dense cloud → typed direct darkens but
        // typed emissive + typed indirect pass through
        // unchanged.
        let direct = [1.0, 1.0, 1.0];
        let emissive = [2.0, 2.0, 2.0];
        let indirect = [0.5, 0.5, 0.5];
        let cloud = 0.2;

        let pixel = simulate_compose_pbr_pixel_with_cloud(direct, emissive, indirect, cloud);
        // Typed direct path × cloud = 0.2; typed emissive
        // + typed indirect = 2.5; typed total = 2.7.
        let expected = 0.2 + 2.5;
        for c in pixel.iter() {
            assert!((c - expected).abs() < 1e-5, "channel={} expected={}", c, expected);
        }

        // Typed clear cloud → typed direct passes through
        // unchanged.
        let clear_pixel =
            simulate_compose_pbr_pixel_with_cloud(direct, emissive, indirect, 1.0);
        for c in clear_pixel.iter() {
            assert!((c - 3.5).abs() < 1e-5);
        }

        // Typed zero cloud → typed direct fully blocked
        // but typed emissive + typed indirect still appear.
        let dark_pixel =
            simulate_compose_pbr_pixel_with_cloud(direct, emissive, indirect, 0.0);
        for c in dark_pixel.iter() {
            assert!((c - 2.5).abs() < 1e-5);
        }

        // Typed pure-emissive case (typed direct = 0) → typed
        // pixel = typed emissive even under typed dense cloud.
        let emissive_only =
            simulate_compose_pbr_pixel_with_cloud([0.0; 3], emissive, [0.0; 3], 0.0);
        for c in emissive_only.iter() {
            assert!((c - 2.0).abs() < 1e-5);
        }

        // Typed pure-indirect case (typed direct = 0) →
        // typed pixel = typed indirect even under typed
        // dense cloud.
        let indirect_only =
            simulate_compose_pbr_pixel_with_cloud([0.0; 3], [0.0; 3], indirect, 0.0);
        for c in indirect_only.iter() {
            assert!((c - 0.5).abs() < 1e-5);
        }
    }

    /// Pass C9.5 — typed shader-vs-CPU agreement (parallel
    /// to C9.3 / C9.4 audits).
    #[test]
    fn shader_and_cpu_agree_within_tolerance() {
        let light_id = LuxLightId::new(99);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);
        for cloud_sample in [0.0, 0.3, 0.7, 1.0] {
            for material_kind in [
                CloudShadowMaterialKindGpu::Terrain,
                CloudShadowMaterialKindGpu::StaticMesh,
                CloudShadowMaterialKindGpu::DynamicMesh,
                CloudShadowMaterialKindGpu::Foliage,
                CloudShadowMaterialKindGpu::Water,
            ] {
                let shader_cloud = simulate_sample_cloud_shadow_for_material(
                    &aux,
                    material_kind,
                    light_id,
                    &matrix,
                    [0.0, 0.0, 0.0],
                    |_uv| Some(cloud_sample),
                );
                // Typed CPU C9.3 reference + typed
                // softening blend.
                let opacity = aux.opacity();
                let opacity_modulated =
                    (1.0 - (1.0 - cloud_sample) * opacity).clamp(0.0, 1.0);
                let softening = material_kind.softening_factor();
                let cpu_cloud = mix(1.0, opacity_modulated, softening);
                assert!(
                    (shader_cloud - cpu_cloud).abs() < 1e-5,
                    "{:?} sample={} shader={} cpu={}",
                    material_kind,
                    cloud_sample,
                    shader_cloud,
                    cpu_cloud,
                );
            }
        }
    }

    /// Pass C9.5 — typed wrong light id falls back to typed
    /// neutral.
    #[test]
    fn wrong_light_id_falls_back_to_neutral() {
        let registered = LuxLightId::new(42);
        let registry = registry_with_layer(registered, CloudShadowFrameDelayMode::SameFrame);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, registered);
        let matrix = live_projection_matrix(registered);
        // Typed query for typed different light id.
        let cloud = simulate_sample_cloud_shadow_for_material(
            &aux,
            CloudShadowMaterialKindGpu::Terrain,
            LuxLightId::new(7),
            &matrix,
            [0.0, 0.0, 0.0],
            |_uv| Some(0.2),
        );
        assert!((cloud - 1.0).abs() < 1e-6);
    }

    /// Pass C9.5 — typed unregistered layer treats cloud
    /// as neutral.
    #[test]
    fn unregistered_layer_treats_cloud_as_neutral() {
        let empty = LuxShadowAuxLayerRegistry::EMPTY;
        let aux = LuxShadowAuxLayerGpu::from_registry(&empty, LuxLightId::new(1));
        let matrix = live_projection_matrix(LuxLightId::new(1));
        // Typed dense cloud sample.  Typed aux absent →
        // typed neutral 1.0.
        for kind in CloudShadowMaterialKindGpu::ALL {
            let cloud = simulate_sample_cloud_shadow_for_material(
                &aux,
                kind,
                LuxLightId::new(1),
                &matrix,
                [0.0, 0.0, 0.0],
                |_uv| Some(0.0),
            );
            assert!((cloud - 1.0).abs() < 1e-6, "{:?}", kind);
        }
    }

    /// Pass C9.5 — typed compose clamps out-of-range
    /// inputs.
    #[test]
    fn material_compose_clamps_out_of_range_inputs() {
        let high = simulate_apply_cloud_layer_to_material_direct_lighting(2.0, 1.5, 1.5);
        assert!((high - 1.0).abs() < 1e-6);
        let low = simulate_apply_cloud_layer_to_material_direct_lighting(-1.0, -0.5, -0.2);
        assert!((low - 0.0).abs() < 1e-6);
        // Typed product preserves typed monotonicity.
        let a = simulate_apply_cloud_layer_to_material_direct_lighting(0.5, 0.6, 0.7);
        let b = simulate_apply_cloud_layer_to_material_direct_lighting(0.5, 0.6, 0.8);
        assert!(b > a);
    }
}
