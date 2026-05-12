//! Pass C9.4 — typed Lux volumetric-light-inject cloud-
//! compose shader scaffold.
//!
//! Pass C7.7 landed the typed CPU
//! `apply_cloud_layer_to_volumetric_scattering` contract.
//! Pass C9.4 lands the typed GPU shader path that mirrors
//! it:
//!
//!     directional_scattering *= cloud_transmittance;
//!
//! Local Lux lights (typed `Punctual` / `Area` /
//! `EmissiveCandidate` / `Probe`) pass through unchanged
//! per typed Pass C7.7 contract.
//!
//! The typed WGSL source lives at
//! `clouds/shaders/lux_volumetric_light_inject_cloud_compose.wgsl`
//! and exposes typed
//! `apply_cloud_layer_to_volumetric_scattering` +
//! `sample_cloud_shadow_layer_at_froxel` +
//! `light_kind_attenuates_with_cloud_shadow` functions
//! that the typed Lux volumetric-light-inject pipeline
//! imports/inlines.
//!
//! This Rust module:
//! - exposes the typed WGSL source as a typed const
//!   string + typed entry-point names;
//! - declares the typed `LuxLightKindGpu` discriminant
//!   that the typed WGSL reads;
//! - provides a typed CPU simulator the typed tests
//!   cross-check against the typed Pass C7.7 reference
//!   path;
//! - audits typed source-text contracts ("no writes to
//!   `LuxVirtualShadowPages` / `LuxShadowAtlas`", no
//!   typed forbidden identifiers);
//! - exposes typed `CloudShadowDebugBreakdown` that
//!   bundles the typed Pass C7.6 direct + typed Pass
//!   C7.7 volumetric compose records so the typed debug
//!   overlay can display typed direct + typed volumetric
//!   cloud effects separately (typed user-spec
//!   acceptance).

use crate::lux_direct_lighting_cloud_layer::DirectLightingCloudCompose;
use crate::lux_direct_lighting_cloud_shader::LuxShadowAuxLayerGpu;
use crate::lux_volumetric_cloud_layer::VolumetricCloudCompose;
use fun_lux::{LuxLightId, LuxLightKind};

pub const FUN_RENDERER_LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_SHADER_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C9.4 — typed WGSL source for the typed Lux
/// volumetric-light-inject cloud-compose helper functions.
pub const LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_COMPOSE_WGSL: &str =
    include_str!("clouds/shaders/lux_volumetric_light_inject_cloud_compose.wgsl");

/// Typed Pass C9.4 — typed entry-point function name in
/// the typed WGSL source.  The typed Lux volumetric
/// shader calls this typed function per froxel × light
/// to attenuate the typed directional scattering
/// contribution.
pub const LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_COMPOSE_ENTRY_POINT: &str =
    "apply_cloud_layer_to_volumetric_scattering";

/// Typed Pass C9.4 — typed helper function names exposed
/// by the typed WGSL source.
pub const LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_SAMPLE_FN: &str = "sample_cloud_shadow_layer_at_froxel";
pub const LUX_VOLUMETRIC_LIGHT_INJECT_KIND_GATE_FN: &str =
    "light_kind_attenuates_with_cloud_shadow";

// ============================================================================
// Section 1 — typed LuxLightKindGpu discriminant
// ============================================================================

/// Typed Pass C9.4 — typed GPU discriminant for the typed
/// `fun_lux::LuxLightKind` enum.  Matches the typed WGSL
/// `LUX_LIGHT_KIND_*` constants.
///
/// Value mapping:
///   0 = Directional       (typed attenuates with cloud shadow)
///   1 = Punctual          (typed local; passes through)
///   2 = Area              (typed local; passes through)
///   3 = EmissiveCandidate (typed local; passes through)
///   4 = Probe             (typed local; passes through)
#[repr(u32)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLightKindGpu {
    #[default]
    Directional = 0,
    Punctual = 1,
    Area = 2,
    EmissiveCandidate = 3,
    Probe = 4,
}

impl LuxLightKindGpu {
    pub const ALL: [Self; 5] = [
        Self::Directional,
        Self::Punctual,
        Self::Area,
        Self::EmissiveCandidate,
        Self::Probe,
    ];

    /// Typed Pass C9.4 — typed CPU → typed GPU mapping.
    /// Matches the typed WGSL discriminant table.
    #[must_use]
    pub const fn from_cpu(kind: LuxLightKind) -> Self {
        match kind {
            LuxLightKind::Directional => Self::Directional,
            LuxLightKind::Punctual => Self::Punctual,
            LuxLightKind::Area => Self::Area,
            LuxLightKind::EmissiveCandidate => Self::EmissiveCandidate,
            LuxLightKind::Probe => Self::Probe,
        }
    }

    /// Typed Pass C9.4 — typed GPU u32 representation.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// Typed Pass C9.4 — typed predicate: does this typed
    /// kind attenuate against typed cloud shadows?
    /// Mirrors typed CPU
    /// `crate::lux_volumetric_cloud_layer::light_kind_attenuates_with_cloud_shadow`.
    #[must_use]
    pub const fn attenuates_with_cloud_shadow(self) -> bool {
        matches!(self, Self::Directional)
    }
}

// ============================================================================
// Section 2 — typed CPU simulators (mirror WGSL)
// ============================================================================

/// Typed Pass C9.4 — typed CPU simulator of the typed
/// `sample_cloud_shadow_layer_at_froxel` WGSL function.
/// Bitwise-identical math to the typed Pass C9.3
/// `simulate_sample_cloud_shadow_layer` (both call into
/// the typed same WGSL function shape; the typed only
/// difference is the typed world-space coordinate
/// interpretation).
#[must_use]
pub fn simulate_sample_cloud_shadow_layer_at_froxel<F>(
    aux_layer_gpu: &LuxShadowAuxLayerGpu,
    light_id: LuxLightId,
    shadow_uv_from_world: &[[f32; 4]; 4],
    froxel_world_pos: [f32; 3],
    sample_cloud_transmittance: F,
) -> f32
where
    F: FnMut([f32; 2]) -> Option<f32>,
{
    crate::lux_direct_lighting_cloud_shader::simulate_sample_cloud_shadow_layer(
        aux_layer_gpu,
        light_id,
        shadow_uv_from_world,
        froxel_world_pos,
        sample_cloud_transmittance,
    )
}

/// Typed Pass C9.4 — typed CPU simulator of the typed
/// `apply_cloud_layer_to_volumetric_scattering` WGSL
/// function.  Mirrors typed CPU
/// `crate::lux_volumetric_cloud_layer::apply_cloud_layer_to_volumetric_scattering`
/// but driven from typed shader-side inputs (typed packed
/// GPU aux-layer uniform + typed projection matrix +
/// typed sampler callback).
#[must_use]
pub fn simulate_apply_cloud_layer_to_volumetric_scattering<F>(
    aux_layer_gpu: &LuxShadowAuxLayerGpu,
    light_kind_gpu: LuxLightKindGpu,
    light_id: LuxLightId,
    shadow_uv_from_world: &[[f32; 4]; 4],
    froxel_world_pos: [f32; 3],
    directional_scattering: f32,
    sample_cloud_transmittance: F,
) -> f32
where
    F: FnMut([f32; 2]) -> Option<f32>,
{
    // Typed local lights pass through unchanged.
    if !light_kind_gpu.attenuates_with_cloud_shadow() {
        return directional_scattering.max(0.0);
    }
    let scattering = directional_scattering.max(0.0);
    let cloud_transmittance = simulate_sample_cloud_shadow_layer_at_froxel(
        aux_layer_gpu,
        light_id,
        shadow_uv_from_world,
        froxel_world_pos,
        sample_cloud_transmittance,
    );
    scattering * cloud_transmittance
}

// ============================================================================
// Section 3 — typed source-text audit predicates
// ============================================================================

/// Typed Pass C9.4 — typed predicate: the typed WGSL
/// source does NOT reference typed Lux opaque shadow
/// write paths.  Audits the typed contract "No cloud
/// data is written into `LuxVirtualShadowPages` or
/// `LuxShadowAtlas`."
#[must_use]
pub fn lux_volumetric_light_inject_cloud_compose_does_not_write_opaque_shadow() -> bool {
    let src = LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_COMPOSE_WGSL;
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

/// Typed Pass C9.4 — typed predicate: the typed shader
/// has zero typed `textureStore(` calls (typed shader is
/// typed pure-read).
#[must_use]
pub fn lux_volumetric_light_inject_cloud_compose_has_no_texture_stores() -> bool {
    let src = LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_COMPOSE_WGSL;
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

// ============================================================================
// Section 4 — typed CloudShadowDebugBreakdown
// ============================================================================

/// Typed Pass C9.4 — typed bundled debug record for the
/// typed direct + typed volumetric cloud-shadow paths.
/// Drives the typed user-spec acceptance bullet
/// "`CloudDebugOverlay::LuxLighting` can display direct
/// and volumetric cloud effects separately."
///
/// The typed renderer collects one typed
/// `DirectLightingCloudCompose` (Pass C7.6) + one typed
/// `VolumetricCloudCompose` (Pass C7.7) per typed
/// directional Lux light per frame and bundles them
/// into a typed breakdown.  Typed debug section emitter
/// produces typed two separate text sections so the
/// typed operator can visualize each contribution
/// independently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudShadowDebugBreakdown {
    pub schema_version: u16,
    pub light_id: LuxLightId,
    pub direct: DirectLightingCloudCompose,
    pub volumetric: VolumetricCloudCompose,
}

impl CloudShadowDebugBreakdown {
    /// Typed Pass C9.4 — typed builder: bundle a typed
    /// direct + volumetric pair into a typed breakdown
    /// record.
    #[must_use]
    pub fn from_pair(
        direct: DirectLightingCloudCompose,
        volumetric: VolumetricCloudCompose,
    ) -> Self {
        Self {
            schema_version: FUN_RENDERER_LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_SHADER_SCHEMA_VERSION,
            light_id: direct.light_id,
            direct,
            volumetric,
        }
    }

    /// Typed Pass C9.4 acceptance predicate — does the
    /// typed direct path + the typed volumetric path
    /// share the typed same Lux directional light id?
    /// Audits the typed user-spec bullet "Same Lux
    /// directional light ID is used by direct lighting
    /// and volumetric injection."
    #[must_use]
    pub fn uses_same_lux_light_id_across_direct_and_volumetric(&self) -> bool {
        self.direct.light_id == self.volumetric.light_id
            && self.direct.light_id == self.light_id
            && self.light_id.is_valid()
    }

    /// Typed predicate: did the typed direct path darken
    /// world pixels this frame?
    #[must_use]
    pub fn direct_path_darkened(&self) -> bool {
        self.direct.cloud_darkens_final_visibility()
    }

    /// Typed predicate: did the typed volumetric path
    /// dim scattering this frame?
    #[must_use]
    pub fn volumetric_path_dimmed(&self) -> bool {
        self.volumetric.cloud_dims_volumetrics()
    }

    /// Typed predicate: are both the typed direct + typed
    /// volumetric paths displaying typed cloud effects
    /// this frame?  Used by the typed debug overlay to
    /// confirm both paths fired.
    #[must_use]
    pub fn both_paths_show_cloud_effects(&self) -> bool {
        self.direct_path_darkened() && self.volumetric_path_dimmed()
    }
}

/// Typed Pass C9.4 — emit the typed debug section that
/// displays typed direct + typed volumetric cloud effects
/// separately.  Returns a typed multi-line text section
/// the typed `CloudDebugOverlay::LuxLighting` renders to
/// the typed debug artifact.
#[must_use]
pub fn cloud_shadow_debug_separate_sections(breakdown: &CloudShadowDebugBreakdown) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "Cloud Shadow Lighting Breakdown");
    let _ = writeln!(content, "-------------------------------");
    let _ = writeln!(content, "light_id: {}", breakdown.light_id.0);
    let _ = writeln!(
        content,
        "uses_same_lux_light_id: {}",
        breakdown.uses_same_lux_light_id_across_direct_and_volumetric(),
    );
    let _ = writeln!(content);

    // Typed Direct section.
    let _ = writeln!(content, "[Direct Lighting]");
    let _ = writeln!(
        content,
        "  opaque_visibility: {:.4}",
        breakdown.direct.opaque_visibility,
    );
    let _ = writeln!(
        content,
        "  cloud_transmittance: {:.4}",
        breakdown.direct.cloud_transmittance,
    );
    let _ = writeln!(
        content,
        "  final_visibility: {:.4}",
        breakdown.direct.final_visibility,
    );
    let _ = writeln!(
        content,
        "  cloud_darkens: {}",
        breakdown.direct_path_darkened(),
    );
    let _ = writeln!(content);

    // Typed Volumetric section.
    let _ = writeln!(content, "[Volumetric Light Inject]");
    let _ = writeln!(
        content,
        "  directional_scattering: {:.4}",
        breakdown.volumetric.directional_scattering,
    );
    let _ = writeln!(
        content,
        "  cloud_transmittance: {:.4}",
        breakdown.volumetric.cloud_transmittance,
    );
    let _ = writeln!(
        content,
        "  final_scattering: {:.4}",
        breakdown.volumetric.final_scattering,
    );
    let _ = writeln!(
        content,
        "  attenuation_applied: {}",
        breakdown.volumetric.attenuation_applied,
    );
    let _ = writeln!(
        content,
        "  cloud_dims: {}",
        breakdown.volumetric_path_dimmed(),
    );
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_shadow::{CloudShadowFrameDelayMode, CloudShadowProjectionConstants};
    use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};
    use crate::lux_direct_lighting_cloud_layer::apply_cloud_layer_to_direct_visibility;
    use crate::lux_shadow_aux_layer::{LuxShadowAuxLayerRegistry, register_cloud_shadow_aux_layer};
    use crate::lux_volumetric_cloud_layer::apply_cloud_layer_to_volumetric_scattering;

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

    /// Pass C9.4 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(
            FUN_RENDERER_LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_SHADER_SCHEMA_VERSION,
            1,
        );
    }

    /// Pass C9.4 — typed `LuxLightKindGpu` discriminants
    /// match the typed WGSL `LUX_LIGHT_KIND_*` constants.
    #[test]
    fn light_kind_gpu_discriminants_match_wgsl() {
        assert_eq!(LuxLightKindGpu::Directional.as_u32(), 0);
        assert_eq!(LuxLightKindGpu::Punctual.as_u32(), 1);
        assert_eq!(LuxLightKindGpu::Area.as_u32(), 2);
        assert_eq!(LuxLightKindGpu::EmissiveCandidate.as_u32(), 3);
        assert_eq!(LuxLightKindGpu::Probe.as_u32(), 4);
        // Typed CPU → GPU mapping.
        assert_eq!(
            LuxLightKindGpu::from_cpu(LuxLightKind::Directional),
            LuxLightKindGpu::Directional,
        );
        assert_eq!(
            LuxLightKindGpu::from_cpu(LuxLightKind::Punctual),
            LuxLightKindGpu::Punctual,
        );
        // Typed only Directional attenuates.
        assert!(LuxLightKindGpu::Directional.attenuates_with_cloud_shadow());
        for k in [
            LuxLightKindGpu::Punctual,
            LuxLightKindGpu::Area,
            LuxLightKindGpu::EmissiveCandidate,
            LuxLightKindGpu::Probe,
        ] {
            assert!(!k.attenuates_with_cloud_shadow(), "{:?}", k);
        }
    }

    /// Pass C9.4 acceptance — typed shader source carries
    /// the typed user-spec contract shape.
    #[test]
    fn shader_source_carries_user_spec_shape() {
        let src = LUX_VOLUMETRIC_LIGHT_INJECT_CLOUD_COMPOSE_WGSL;
        assert!(!src.is_empty());
        assert!(src.contains("fn apply_cloud_layer_to_volumetric_scattering"));
        assert!(src.contains("fn sample_cloud_shadow_layer_at_froxel"));
        assert!(src.contains("fn light_kind_attenuates_with_cloud_shadow"));
        // Typed user-spec formula.
        assert!(src.contains("return scattering * cloud_transmittance;"));
        // Typed local-lights-pass-through path.
        assert!(src.contains("if (!light_kind_attenuates_with_cloud_shadow(light_kind))"));
        // Typed LuxLightKind discriminant constants.
        assert!(src.contains("const LUX_LIGHT_KIND_DIRECTIONAL: u32 = 0u;"));
        assert!(src.contains("const LUX_LIGHT_KIND_PUNCTUAL: u32 = 1u;"));
        // Typed bindings.
        assert!(src.contains("var<uniform> shadow_projection: CloudShadowProjectionConstants"));
        assert!(src.contains("var cloud_shadow_filtered: texture_2d<f32>"));
        assert!(src.contains("var<uniform> aux_layer: LuxShadowAuxLayerEntry"));
    }

    /// Pass C9.4 acceptance — no cloud data is written
    /// into LuxVirtualShadowPages or LuxShadowAtlas.
    #[test]
    fn no_cloud_data_written_to_lux_virtual_shadow_or_atlas() {
        assert!(lux_volumetric_light_inject_cloud_compose_does_not_write_opaque_shadow());
        assert!(lux_volumetric_light_inject_cloud_compose_has_no_texture_stores());
    }

    /// Pass C9.4 acceptance — fog/godrays dim under dense
    /// clouds.
    #[test]
    fn fog_godrays_dim_under_dense_clouds() {
        let light_id = LuxLightId::new(7);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let aux_layer_gpu = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);
        // Typed scattering = 10.0, typed dense cloud = 0.2
        // → typed final < typed half.
        let final_scattering = simulate_apply_cloud_layer_to_volumetric_scattering(
            &aux_layer_gpu,
            LuxLightKindGpu::Directional,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            10.0,
            |_uv| Some(0.2),
        );
        assert!(
            final_scattering < 5.0,
            "expected dim, got {}",
            final_scattering,
        );
    }

    /// Pass C9.4 acceptance — clear / no-cloud profile
    /// leaves volumetric scattering unchanged.
    #[test]
    fn clear_profile_leaves_volumetric_scattering_unchanged() {
        let light_id = LuxLightId::new(1);
        let matrix = live_projection_matrix(light_id);

        // Typed empty registry → typed NO_LAYER → typed
        // final = directional_scattering.
        let empty_registry = LuxShadowAuxLayerRegistry::EMPTY;
        let no_layer = LuxShadowAuxLayerGpu::from_registry(&empty_registry, light_id);
        let final_no_layer = simulate_apply_cloud_layer_to_volumetric_scattering(
            &no_layer,
            LuxLightKindGpu::Directional,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            8.0,
            |_uv| Some(0.0),
        );
        assert!((final_no_layer - 8.0).abs() < 1e-6);

        // Typed registered layer + typed clear cloud
        // sample = 1.0 → typed final = directional.
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let final_clear = simulate_apply_cloud_layer_to_volumetric_scattering(
            &aux,
            LuxLightKindGpu::Directional,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            8.0,
            |_uv| Some(1.0),
        );
        assert!((final_clear - 8.0).abs() < 1e-6);
    }

    /// Pass C9.4 acceptance — same Lux directional light
    /// ID is used by direct lighting and volumetric
    /// injection.
    #[test]
    fn same_lux_directional_light_id_in_direct_and_volumetric() {
        let light_id = LuxLightId::new(42);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        // Typed direct path lookup.
        let direct_compose = apply_cloud_layer_to_direct_visibility(&registry, light_id, 1.0, 0.4);
        assert!(direct_compose.layer_found);
        // Typed volumetric path lookup (same registry,
        // same light id).
        let volumetric_compose = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            10.0,
            0.4,
        );
        assert!(volumetric_compose.layer_found);

        // Typed both paths discovered the typed SAME
        // aux layer at the typed SAME light id.
        assert_eq!(direct_compose.light_id, volumetric_compose.light_id);
        assert_eq!(direct_compose.light_id, light_id);
        assert_eq!(direct_compose.latency, volumetric_compose.latency);

        // Typed breakdown record cross-audits the typed
        // same-id contract.
        let breakdown = CloudShadowDebugBreakdown::from_pair(direct_compose, volumetric_compose);
        assert!(breakdown.uses_same_lux_light_id_across_direct_and_volumetric());
    }

    /// Pass C9.4 acceptance — local Lux lights do not
    /// cast world-scale cloud shadows.
    #[test]
    fn local_lux_lights_do_not_cast_world_scale_cloud_shadows() {
        let light_id = LuxLightId::new(7);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);

        // Typed every local kind passes through unchanged
        // even with a typed matching aux layer + typed
        // dense cloud sample.
        for kind in [
            LuxLightKindGpu::Punctual,
            LuxLightKindGpu::Area,
            LuxLightKindGpu::EmissiveCandidate,
            LuxLightKindGpu::Probe,
        ] {
            let final_scattering = simulate_apply_cloud_layer_to_volumetric_scattering(
                &aux,
                kind,
                light_id,
                &matrix,
                [0.0, 0.0, 0.0],
                5.0,
                |_uv| Some(0.1), // dense cloud sample
            );
            assert!(
                (final_scattering - 5.0).abs() < 1e-6,
                "{:?} altered scattering: {}",
                kind,
                final_scattering,
            );
        }

        // Typed sanity — typed Directional at typed same
        // light id DOES attenuate.
        let directional_final = simulate_apply_cloud_layer_to_volumetric_scattering(
            &aux,
            LuxLightKindGpu::Directional,
            light_id,
            &matrix,
            [0.0, 0.0, 0.0],
            5.0,
            |_uv| Some(0.1),
        );
        assert!(
            directional_final < 5.0,
            "directional did not attenuate: {}",
            directional_final,
        );
    }

    /// Pass C9.4 acceptance — `CloudDebugOverlay::LuxLighting`
    /// can display direct and volumetric cloud effects
    /// separately.
    #[test]
    fn debug_overlay_displays_direct_and_volumetric_separately() {
        let light_id = LuxLightId::new(13);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let direct_compose = apply_cloud_layer_to_direct_visibility(&registry, light_id, 1.0, 0.3);
        let volumetric_compose = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            8.0,
            0.3,
        );

        let breakdown = CloudShadowDebugBreakdown::from_pair(direct_compose, volumetric_compose);
        assert!(breakdown.both_paths_show_cloud_effects());

        let section = cloud_shadow_debug_separate_sections(&breakdown);
        // Typed two separate sections.
        assert!(section.contains("[Direct Lighting]"));
        assert!(section.contains("[Volumetric Light Inject]"));
        // Typed direct path values.
        assert!(section.contains("opaque_visibility:"));
        assert!(section.contains("final_visibility:"));
        // Typed volumetric path values.
        assert!(section.contains("directional_scattering:"));
        assert!(section.contains("final_scattering:"));
        assert!(section.contains("attenuation_applied:"));
        // Typed light id appears in the typed header.
        assert!(section.contains("light_id: 13"));
        // Typed same-light-id audit appears.
        assert!(section.contains("uses_same_lux_light_id: true"));
    }

    /// Pass C9.4 — typed shader-vs-CPU agreement
    /// (parallel to C9.3 audit).
    #[test]
    fn shader_and_cpu_agree_within_tolerance() {
        let light_id = LuxLightId::new(99);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, light_id);
        let matrix = live_projection_matrix(light_id);
        // Cross-check typed shader vs typed CPU C7.7 for
        // a typed range of inputs.
        for scattering in [0.0, 1.0, 5.0, 10.0] {
            for cloud in [0.0, 0.3, 0.7, 1.0] {
                let shader_final = simulate_apply_cloud_layer_to_volumetric_scattering(
                    &aux,
                    LuxLightKindGpu::Directional,
                    light_id,
                    &matrix,
                    [0.0, 0.0, 0.0],
                    scattering,
                    |_uv| Some(cloud),
                );
                // Typed CPU C7.7 reference (computes typed
                // pre-opacity cloud sample as input).
                let opacity = aux.opacity();
                let effective_cloud = (1.0 - (1.0 - cloud) * opacity).clamp(0.0, 1.0);
                let cpu_final = scattering.max(0.0) * effective_cloud;
                assert!(
                    (shader_final - cpu_final).abs() < 1e-5,
                    "scattering={} cloud={} shader={} cpu={}",
                    scattering,
                    cloud,
                    shader_final,
                    cpu_final,
                );
            }
        }
    }

    /// Pass C9.4 — typed invalid light id falls back to
    /// typed neutral.
    #[test]
    fn wrong_light_id_falls_back_to_neutral() {
        let registered = LuxLightId::new(42);
        let registry = registry_with_layer(registered, CloudShadowFrameDelayMode::SameFrame);
        let aux = LuxShadowAuxLayerGpu::from_registry(&registry, registered);
        let matrix = live_projection_matrix(registered);
        // Typed query for typed different light id.
        let final_scattering = simulate_apply_cloud_layer_to_volumetric_scattering(
            &aux,
            LuxLightKindGpu::Directional,
            LuxLightId::new(7),
            &matrix,
            [0.0, 0.0, 0.0],
            5.0,
            |_uv| Some(0.2),
        );
        // Typed mismatched id → typed neutral cloud →
        // typed final = directional input.
        assert!((final_scattering - 5.0).abs() < 1e-6);
    }

    /// Pass C9.4 — typed breakdown predicates report
    /// typed correct state.
    #[test]
    fn breakdown_predicates_report_correct_state() {
        let light_id = LuxLightId::new(13);
        let registry = registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        // Typed dense cloud → both paths darken / dim.
        let direct = apply_cloud_layer_to_direct_visibility(&registry, light_id, 1.0, 0.3);
        let vol = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            10.0,
            0.3,
        );
        let breakdown = CloudShadowDebugBreakdown::from_pair(direct, vol);
        assert!(breakdown.direct_path_darkened());
        assert!(breakdown.volumetric_path_dimmed());
        assert!(breakdown.both_paths_show_cloud_effects());
        assert!(breakdown.uses_same_lux_light_id_across_direct_and_volumetric());

        // Typed clear cloud → neither darkens.
        let direct_clear = apply_cloud_layer_to_direct_visibility(&registry, light_id, 1.0, 1.0);
        let vol_clear = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            10.0,
            1.0,
        );
        let clear = CloudShadowDebugBreakdown::from_pair(direct_clear, vol_clear);
        assert!(!clear.direct_path_darkened());
        assert!(!clear.volumetric_path_dimmed());
        assert!(!clear.both_paths_show_cloud_effects());
    }
}
