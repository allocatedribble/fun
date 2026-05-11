//! Pass C7.7 — typed Lux volumetric light-inject cloud
//! aux-layer consumption.
//!
//! Wires the typed `LuxVolumetricLightInject` pass to
//! discover the typed cloud transmittance layer by typed
//! directional Lux light id (via the typed
//! [`crate::lux_shadow_aux_layer::LuxShadowAuxLayerRegistry`])
//! and attenuate the typed directional volumetric
//! scattering contribution:
//!
//!     final_scattering = directional_scattering * cloud_transmittance
//!
//! Mirrors the typed Pass C7.6
//! `apply_cloud_layer_to_direct_visibility` contract for
//! the typed direct-lighting path.  Both passes look up
//! the typed aux layer by the typed SAME Lux light id, so
//! the typed cloud shadow is consistent between typed
//! direct lighting and typed volumetric scattering.
//!
//! Local Lux lights (`Punctual`, `Area`, `EmissiveCandidate`,
//! `Probe`) MUST NOT cast typed world-scale cloud shadows
//! by default.  The typed
//! [`light_kind_attenuates_with_cloud_shadow`] predicate
//! gates the typed attenuation to typed `Directional` only;
//! the typed [`local_lights_skip_cloud_shadow`] const
//! predicate audits the typed contract at the typed const
//! layer.

use crate::cloud_shadow::CloudShadowFrameDelayMode;
use crate::lux_shadow_aux_layer::{
    LuxShadowAuxLayer, LuxShadowAuxLayerKind, LuxShadowAuxLayerRegistry,
};
use fun_lux::{LuxLightId, LuxLightKind};

pub const FUN_RENDERER_LUX_VOLUMETRIC_CLOUD_LAYER_SCHEMA_VERSION: u16 = 1;

// ============================================================================
// Section 1 — typed VolumetricCloudCompose record
// ============================================================================

/// Typed Pass C7.7 — typed volumetric-cloud-compose result.
/// Bundles the typed per-light compose inputs + the typed
/// final attenuated scattering + the typed lookup +
/// per-light-kind gates.  The typed renderer diagnostics
/// aggregate these records into a typed
/// [`VolumetricCloudComposeSummary`] for the typed cloud
/// debug overlay.
///
/// Field meaning:
/// - `schema_version` — typed schema marker.
/// - `light_id` — typed Lux light id the typed compose ran
///   for.
/// - `light_kind` — typed [`LuxLightKind`].  Drives the
///   typed `attenuation_applied` gate — only typed
///   `Directional` lights attenuate against typed cloud
///   shadows.
/// - `directional_scattering` — typed input directional
///   scattering contribution (clamped to typed `[0, +∞)`;
///   non-negative).  Represents the typed scalar
///   intensity the typed renderer would inject into the
///   typed froxel before typed cloud attenuation.
/// - `cloud_transmittance` — typed input cloud
///   transmittance sampled from the typed aux layer
///   (clamped to typed `[0, 1]`).  Defaults to typed `1.0`
///   when no typed aux layer is registered or when the
///   typed light is typed local (no cloud-shadow
///   contribution).
/// - `final_scattering` — typed
///   `directional_scattering * cloud_transmittance` when
///   typed attenuation applies; typed
///   `directional_scattering` otherwise.
/// - `layer_found` — `true` when the typed registry
///   returned a typed aux layer for the typed light id.
/// - `attenuation_applied` — `true` when the typed compose
///   actually multiplied the typed cloud transmittance
///   into the typed final scattering.  Requires typed
///   directional kind AND typed layer found.
/// - `latency` — typed [`CloudShadowFrameDelayMode`] from
///   the typed registered aux layer (or
///   `CloudShadowFrameDelayMode::default()` when no layer
///   is found).
/// - `samples_current_frame` / `samples_previous_frame` —
///   typed delay-mode-derived flags.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumetricCloudCompose {
    pub schema_version: u16,
    pub light_id: LuxLightId,
    pub light_kind: LuxLightKind,
    pub directional_scattering: f32,
    pub cloud_transmittance: f32,
    pub final_scattering: f32,
    pub layer_found: bool,
    pub attenuation_applied: bool,
    pub latency: CloudShadowFrameDelayMode,
    pub samples_current_frame: bool,
    pub samples_previous_frame: bool,
}

impl Default for VolumetricCloudCompose {
    fn default() -> Self {
        // Typed `LuxLightKind` does not derive `Default` in
        // `fun_lux`; pick the typed `Directional` variant
        // since the typed C7.7 contract gates the typed
        // attenuation on it (the typed default record is
        // typed "directional light, no scattering, no
        // attenuation").
        Self {
            schema_version: FUN_RENDERER_LUX_VOLUMETRIC_CLOUD_LAYER_SCHEMA_VERSION,
            light_id: LuxLightId::INVALID,
            light_kind: LuxLightKind::Directional,
            directional_scattering: 0.0,
            cloud_transmittance: 1.0,
            final_scattering: 0.0,
            layer_found: false,
            attenuation_applied: false,
            latency: CloudShadowFrameDelayMode::default(),
            samples_current_frame: false,
            samples_previous_frame: false,
        }
    }
}

impl VolumetricCloudCompose {
    /// Typed "no attenuation" outcome — typed final
    /// scattering equals the typed directional scattering
    /// (no cloud-shadow contribution).  Used when:
    /// - The typed light kind is local (not typed
    ///   directional).
    /// - No typed aux layer is registered for the typed
    ///   light id.
    /// - The typed registry returned `None` for the typed
    ///   typed kind lookup.
    #[must_use]
    pub fn no_attenuation(
        light_id: LuxLightId,
        light_kind: LuxLightKind,
        directional_scattering: f32,
    ) -> Self {
        let scattering = directional_scattering.max(0.0);
        Self {
            schema_version: FUN_RENDERER_LUX_VOLUMETRIC_CLOUD_LAYER_SCHEMA_VERSION,
            light_id,
            light_kind,
            directional_scattering: scattering,
            cloud_transmittance: 1.0,
            final_scattering: scattering,
            layer_found: false,
            attenuation_applied: false,
            latency: CloudShadowFrameDelayMode::default(),
            samples_current_frame: false,
            samples_previous_frame: false,
        }
    }

    /// Typed predicate: did the typed cloud transmittance
    /// dim the typed volumetric scattering?  Used by the
    /// typed "fog/godrays dim under dense cloud cover"
    /// acceptance audit.
    #[must_use]
    pub fn cloud_dims_volumetrics(&self) -> bool {
        self.attenuation_applied
            && self.cloud_transmittance < 1.0
            && self.final_scattering < self.directional_scattering
    }

    /// Typed predicate: did the typed compose preserve the
    /// typed directional scattering (no typed cloud dimming)?
    /// True when:
    ///
    /// - No typed attenuation applied, OR
    /// - The typed cloud transmittance is typed `1.0`.
    ///
    /// Used by the typed "clear profile leaves volumetric
    /// lighting unchanged" acceptance audit.
    #[must_use]
    pub fn preserves_volumetrics(&self) -> bool {
        (self.final_scattering - self.directional_scattering).abs() < 1e-6
    }

    /// Typed predicate: was the typed attenuation applied
    /// ONLY to a typed directional light?  Returns `false`
    /// for any typed local light kind even if the typed
    /// `attenuation_applied` flag is somehow `true` (typed
    /// defense-in-depth audit).
    #[must_use]
    pub fn applied_to_directional_only(&self) -> bool {
        if !self.attenuation_applied {
            return true;
        }
        matches!(self.light_kind, LuxLightKind::Directional)
    }

    /// Typed predicate: typed multiplication semantics —
    /// when typed attenuation applies, typed
    /// `final = directional * cloud`.
    #[must_use]
    pub fn composes_via_multiplication(&self) -> bool {
        if !self.attenuation_applied {
            return (self.final_scattering - self.directional_scattering).abs() < 1e-6;
        }
        let expected = self.directional_scattering * self.cloud_transmittance;
        (self.final_scattering - expected).abs() < 1e-6
    }

    /// Typed Pass C7.7 debug helper — typed attenuation
    /// factor `final / directional`.  Used by the typed
    /// debug overlay (`CloudDebugOverlay::LuxLighting`) to
    /// visualize per-light cloud attenuation.  Clamped to
    /// typed `[0, 1]`; returns typed `1.0` when typed
    /// directional scattering is typed near-zero (avoids
    /// typed divide-by-zero).
    #[must_use]
    pub fn debug_attenuation_factor(&self) -> f32 {
        if self.directional_scattering <= f32::EPSILON {
            return 1.0;
        }
        (self.final_scattering / self.directional_scattering).clamp(0.0, 1.0)
    }
}

// ============================================================================
// Section 2 — typed light-kind gating predicates
// ============================================================================

/// Typed Pass C7.7 — typed predicate: does this typed
/// `LuxLightKind` attenuate against typed cloud shadows?
/// Only typed `Directional` lights do; local kinds skip
/// (per user spec "Local Lux lights do not cast world-scale
/// cloud shadows by default").
#[must_use]
pub const fn light_kind_attenuates_with_cloud_shadow(kind: LuxLightKind) -> bool {
    matches!(kind, LuxLightKind::Directional)
}

/// Typed Pass C7.7 const audit — every typed local light
/// kind (`Punctual`, `Area`, `EmissiveCandidate`, `Probe`)
/// skips typed cloud shadow attenuation.  Audited at the
/// typed const layer so callers can grep + assert the
/// typed contract without running typed runtime tests.
#[must_use]
pub const fn local_lights_skip_cloud_shadow() -> bool {
    !light_kind_attenuates_with_cloud_shadow(LuxLightKind::Punctual)
        && !light_kind_attenuates_with_cloud_shadow(LuxLightKind::Area)
        && !light_kind_attenuates_with_cloud_shadow(LuxLightKind::EmissiveCandidate)
        && !light_kind_attenuates_with_cloud_shadow(LuxLightKind::Probe)
}

/// Typed Pass C7.7 const audit — typed `Directional`
/// lights DO attenuate against typed cloud shadows.
#[must_use]
pub const fn directional_lights_attenuate_with_cloud_shadow() -> bool {
    light_kind_attenuates_with_cloud_shadow(LuxLightKind::Directional)
}

// ============================================================================
// Section 3 — typed compose helpers
// ============================================================================

/// Typed Pass C7.7 — discover the typed cloud aux layer
/// for the typed directional Lux light id and attenuate
/// the typed volumetric scattering contribution.
///
/// Inputs:
/// - `registry` — typed
///   [`LuxShadowAuxLayerRegistry`] the typed
///   `LuxCloudShadowRegisterLayer` pass populated this
///   frame (or the typed PREVIOUS frame for the typed
///   one-frame-delayed mode).  Typed C7.6 uses the typed
///   same registry — the typed cloud shadow is shared
///   between typed direct lighting + typed volumetric
///   light injection.
/// - `light_id` — typed Lux light id (typed same id the
///   typed direct-lighting pass uses for the typed lookup).
/// - `light_kind` — typed Lux light kind.  Typed local
///   kinds skip the typed attenuation.
/// - `directional_scattering` — typed scalar intensity the
///   typed volumetric pass would inject into the typed
///   froxel before typed cloud attenuation.
/// - `cloud_transmittance_sample` — typed cloud
///   transmittance sampled from the typed aux layer's
///   typed resource at the typed froxel→sun ray.  Caller
///   produces this value (the typed GPU sampler does the
///   typed shadow-UV transform via the typed projection
///   constants).
///
/// Output: typed
/// [`VolumetricCloudCompose`] record.  When no typed aux
/// layer is registered for the typed light id OR the typed
/// light kind is local, the typed record reports typed
/// `layer_found = false` / `attenuation_applied = false`
/// and typed `final_scattering = directional_scattering`
/// so the typed renderer audit can prove "Clear profile
/// leaves volumetric lighting unchanged" / "Local Lux
/// lights do not cast world-scale cloud shadows".
#[must_use]
pub fn apply_cloud_layer_to_volumetric_scattering(
    registry: &LuxShadowAuxLayerRegistry,
    light_id: LuxLightId,
    light_kind: LuxLightKind,
    directional_scattering: f32,
    cloud_transmittance_sample: f32,
) -> VolumetricCloudCompose {
    if !light_kind_attenuates_with_cloud_shadow(light_kind) {
        return VolumetricCloudCompose::no_attenuation(
            light_id,
            light_kind,
            directional_scattering,
        );
    }
    let layer = registry.find_for_kind(light_id, LuxShadowAuxLayerKind::CloudTransmittance);
    let Some(layer) = layer else {
        return VolumetricCloudCompose::no_attenuation(
            light_id,
            light_kind,
            directional_scattering,
        );
    };

    let scattering = directional_scattering.max(0.0);
    let cloud = cloud_transmittance_sample.clamp(0.0, 1.0);
    let final_scattering = scattering * cloud;
    VolumetricCloudCompose {
        schema_version: FUN_RENDERER_LUX_VOLUMETRIC_CLOUD_LAYER_SCHEMA_VERSION,
        light_id,
        light_kind,
        directional_scattering: scattering,
        cloud_transmittance: cloud,
        final_scattering,
        layer_found: true,
        attenuation_applied: true,
        latency: layer.latency,
        samples_current_frame: layer.latency.samples_current_frame_filtered_shadow(),
        samples_previous_frame: layer.latency.samples_previous_frame_filtered_shadow(),
    }
}

/// Typed Pass C7.7 — convenience: discover + compose
/// using a typed full `LuxShadowAuxLayer` reference.
/// Useful when the typed renderer already iterated the
/// typed registry and wants to apply the typed layer to
/// multiple typed froxels without re-running the typed
/// lookup.
#[must_use]
pub fn apply_cloud_layer_to_volumetric_scattering_with_layer(
    layer: &LuxShadowAuxLayer,
    light_id: LuxLightId,
    light_kind: LuxLightKind,
    directional_scattering: f32,
    cloud_transmittance_sample: f32,
) -> VolumetricCloudCompose {
    if !light_kind_attenuates_with_cloud_shadow(light_kind) {
        return VolumetricCloudCompose::no_attenuation(
            light_id,
            light_kind,
            directional_scattering,
        );
    }
    if !layer.matches_light(light_id) {
        return VolumetricCloudCompose::no_attenuation(
            light_id,
            light_kind,
            directional_scattering,
        );
    }
    let scattering = directional_scattering.max(0.0);
    let cloud = cloud_transmittance_sample.clamp(0.0, 1.0);
    let final_scattering = scattering * cloud;
    VolumetricCloudCompose {
        schema_version: FUN_RENDERER_LUX_VOLUMETRIC_CLOUD_LAYER_SCHEMA_VERSION,
        light_id,
        light_kind,
        directional_scattering: scattering,
        cloud_transmittance: cloud,
        final_scattering,
        layer_found: true,
        attenuation_applied: true,
        latency: layer.latency,
        samples_current_frame: layer.latency.samples_current_frame_filtered_shadow(),
        samples_previous_frame: layer.latency.samples_previous_frame_filtered_shadow(),
    }
}

// ============================================================================
// Section 4 — typed debug summary + debug section
// ============================================================================

/// Typed Pass C7.7 volumetric cloud compose summary.
/// Aggregates the typed per-light compose outcomes the
/// typed renderer collected this frame for the typed
/// `CloudDebugOverlay::LuxLighting` debug view.  Drives
/// the typed "debug view can show cloud attenuation
/// applied to Lux volumetrics" acceptance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumetricCloudComposeSummary {
    pub schema_version: u16,
    /// Typed count of typed directional lights that ran
    /// through the typed compose with typed attenuation
    /// applied.
    pub directional_attenuated_count: u32,
    /// Typed count of typed directional lights that did
    /// NOT attenuate this frame (typed no aux layer
    /// registered for the typed light id).
    pub directional_skipped_count: u32,
    /// Typed count of typed local lights that skipped
    /// typed cloud shadow attenuation (typed Punctual /
    /// Area / EmissiveCandidate / Probe).
    pub local_skipped_count: u32,
    /// Typed sum of typed `directional_scattering` across
    /// every typed compose this frame.  Drives the typed
    /// `mean_attenuation_factor` denominator.
    pub total_directional_scattering: f32,
    /// Typed sum of typed `final_scattering` across every
    /// typed compose this frame.
    pub total_final_scattering: f32,
    /// Typed minimum typed `debug_attenuation_factor`
    /// observed (the typed darkest froxel's typed cloud
    /// attenuation).  Starts at typed `1.0`; decreases as
    /// typed dense cloud cover dims the typed volumetrics.
    pub min_attenuation_factor: f32,
    /// Typed maximum typed `debug_attenuation_factor`
    /// observed.  Starts at typed `0.0`; saturates at
    /// typed `1.0` under typed clear skies.
    pub max_attenuation_factor: f32,
}

impl Default for VolumetricCloudComposeSummary {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl VolumetricCloudComposeSummary {
    /// Typed empty summary — no typed composes recorded.
    pub const EMPTY: Self = Self {
        schema_version: FUN_RENDERER_LUX_VOLUMETRIC_CLOUD_LAYER_SCHEMA_VERSION,
        directional_attenuated_count: 0,
        directional_skipped_count: 0,
        local_skipped_count: 0,
        total_directional_scattering: 0.0,
        total_final_scattering: 0.0,
        min_attenuation_factor: 1.0,
        max_attenuation_factor: 0.0,
    };

    /// Typed Pass C7.7 — record a typed compose outcome
    /// into the typed summary.  Updates the typed counters
    /// + the typed min/max attenuation factors.
    pub fn record(&mut self, compose: &VolumetricCloudCompose) {
        match compose.light_kind {
            LuxLightKind::Directional => {
                if compose.attenuation_applied {
                    self.directional_attenuated_count =
                        self.directional_attenuated_count.saturating_add(1);
                } else {
                    self.directional_skipped_count =
                        self.directional_skipped_count.saturating_add(1);
                }
            }
            LuxLightKind::Punctual
            | LuxLightKind::Area
            | LuxLightKind::EmissiveCandidate
            | LuxLightKind::Probe => {
                self.local_skipped_count = self.local_skipped_count.saturating_add(1);
            }
        }
        self.total_directional_scattering += compose.directional_scattering.max(0.0);
        self.total_final_scattering += compose.final_scattering.max(0.0);
        let factor = compose.debug_attenuation_factor();
        if factor < self.min_attenuation_factor {
            self.min_attenuation_factor = factor;
        }
        if factor > self.max_attenuation_factor {
            self.max_attenuation_factor = factor;
        }
    }

    /// Typed mean attenuation factor across the typed
    /// recorded composes.  Returns typed `1.0` when no
    /// typed directional scattering was recorded (avoids
    /// typed divide-by-zero).
    #[must_use]
    pub fn mean_attenuation_factor(&self) -> f32 {
        if self.total_directional_scattering <= f32::EPSILON {
            return 1.0;
        }
        (self.total_final_scattering / self.total_directional_scattering).clamp(0.0, 1.0)
    }

    /// Typed total count of typed composes recorded.
    #[must_use]
    pub const fn total_compose_count(&self) -> u32 {
        self.directional_attenuated_count
            .saturating_add(self.directional_skipped_count)
            .saturating_add(self.local_skipped_count)
    }

    /// Typed predicate: did typed any cloud attenuation
    /// run this frame?
    #[must_use]
    pub const fn any_attenuation_applied(&self) -> bool {
        self.directional_attenuated_count > 0
    }
}

/// Typed Pass C7.7 debug section.  Emits the typed
/// "Volumetric Cloud Attenuation" multi-line section the
/// typed cloud debug overlay (`CloudDebugOverlay::LuxLighting`)
/// appends to the typed
/// `RendererFrameGraphDebugArtifact.content`.  Lists the
/// typed counts + the typed attenuation factors so the
/// typed operator + the typed AI agent can confirm typed
/// cloud attenuation reached the typed volumetric path.
#[must_use]
pub fn volumetric_cloud_debug_section(summary: &VolumetricCloudComposeSummary) -> String {
    use core::fmt::Write as _;
    let mut content = String::new();
    let _ = writeln!(content, "Volumetric Cloud Attenuation");
    let _ = writeln!(content, "----------------------------");
    let _ = writeln!(
        content,
        "total_compose_count: {}",
        summary.total_compose_count(),
    );
    let _ = writeln!(
        content,
        "directional_attenuated: {}",
        summary.directional_attenuated_count,
    );
    let _ = writeln!(
        content,
        "directional_skipped: {}",
        summary.directional_skipped_count,
    );
    let _ = writeln!(content, "local_skipped: {}", summary.local_skipped_count);
    let _ = writeln!(
        content,
        "min_attenuation_factor: {:.4}",
        summary.min_attenuation_factor,
    );
    let _ = writeln!(
        content,
        "max_attenuation_factor: {:.4}",
        summary.max_attenuation_factor,
    );
    let _ = writeln!(
        content,
        "mean_attenuation_factor: {:.4}",
        summary.mean_attenuation_factor(),
    );
    let _ = writeln!(
        content,
        "any_attenuation_applied: {}",
        summary.any_attenuation_applied(),
    );
    content
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_shadow::CloudShadowProjectionConstants;
    use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};
    use crate::lux_shadow_aux_layer::register_cloud_shadow_aux_layer;

    fn make_registry_with_layer(
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
        .expect("registration succeeds");
        registry
    }

    /// Pass C7.7 schema is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_LUX_VOLUMETRIC_CLOUD_LAYER_SCHEMA_VERSION, 1);
    }

    /// Pass C7.7 acceptance — fog/godrays dim under dense
    /// cloud cover.  Path: directional scattering + low
    /// cloud transmittance → low final scattering.
    #[test]
    fn fog_godrays_dim_under_dense_cloud_cover() {
        let light_id = LuxLightId::new(42);
        let registry =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        // Typed scattering = 10.0, typed dense cloud = 0.2
        // → typed final = 2.0 < typed 10.0.
        let compose = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            10.0,
            0.2,
        );
        assert!(compose.attenuation_applied);
        assert!(compose.layer_found);
        assert!((compose.final_scattering - 2.0).abs() < 1e-6);
        assert!(compose.cloud_dims_volumetrics());
        assert!(!compose.preserves_volumetrics());
        // Typed debug factor reflects the typed dimming.
        assert!((compose.debug_attenuation_factor() - 0.2).abs() < 1e-6);
    }

    /// Pass C7.7 acceptance — Clear profile (typed cloud
    /// transmittance ≈ 1.0) leaves volumetric lighting
    /// unchanged.
    #[test]
    fn clear_profile_leaves_volumetric_lighting_unchanged() {
        let light_id = LuxLightId::new(1);
        let registry =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        // Typed clear → typed cloud transmittance = 1.0.
        let compose = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            8.0,
            1.0,
        );
        assert!(compose.attenuation_applied);
        // Typed final = typed input (typed cloud is typed
        // neutral).
        assert!((compose.final_scattering - 8.0).abs() < 1e-6);
        assert!(compose.preserves_volumetrics());
        assert!(!compose.cloud_dims_volumetrics());
        // Typed debug factor is typed 1.0 (no typed
        // dimming).
        assert!((compose.debug_attenuation_factor() - 1.0).abs() < 1e-6);

        // Typed full "no layer registered" path: typed
        // empty registry → typed final = typed
        // directional scattering.
        let empty = LuxShadowAuxLayerRegistry::EMPTY;
        let no_layer = apply_cloud_layer_to_volumetric_scattering(
            &empty,
            light_id,
            LuxLightKind::Directional,
            8.0,
            0.5, // Doesn't matter — no layer registered.
        );
        assert!(!no_layer.attenuation_applied);
        assert!((no_layer.final_scattering - 8.0).abs() < 1e-6);
        assert!(no_layer.preserves_volumetrics());
    }

    /// Pass C7.7 acceptance — cloud shadow layer is looked
    /// up by the typed SAME Lux light id used by typed
    /// direct lighting.  Audited by registering one layer
    /// + asserting both typed direct-lighting and typed
    /// volumetric paths find the typed same layer.
    #[test]
    fn same_light_id_lookup_as_direct_lighting() {
        use crate::lux_direct_lighting_cloud_layer::apply_cloud_layer_to_direct_visibility;
        let light_id = LuxLightId::new(123);
        let registry =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);

        // Typed direct lighting path.
        let direct_compose = apply_cloud_layer_to_direct_visibility(
            &registry, light_id, /* opaque */ 1.0, /* cloud */ 0.5,
        );
        assert!(direct_compose.layer_found);

        // Typed volumetric path uses the typed SAME
        // registry + the typed SAME light id.
        let volumetric_compose = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            1.0,
            0.5,
        );
        assert!(volumetric_compose.layer_found);

        // Typed both paths discovered the typed same
        // layer; typed latency mode matches (since they
        // read the typed same aux record).
        assert_eq!(direct_compose.latency, volumetric_compose.latency);
        assert_eq!(direct_compose.light_id, volumetric_compose.light_id);
        // Typed cloud transmittance composes the typed
        // same way in both paths (typed product).
        assert!(
            (direct_compose.final_visibility - volumetric_compose.final_scattering).abs() < 1e-6,
        );
    }

    /// Pass C7.7 acceptance — local Lux lights do not cast
    /// world-scale cloud shadows by default.  Every typed
    /// local kind (`Punctual`, `Area`, `EmissiveCandidate`,
    /// `Probe`) skips the typed attenuation.
    #[test]
    fn local_lux_lights_do_not_cast_world_scale_cloud_shadows_by_default() {
        // Typed const-layer audit.
        assert!(local_lights_skip_cloud_shadow());
        assert!(directional_lights_attenuate_with_cloud_shadow());
        assert!(!light_kind_attenuates_with_cloud_shadow(LuxLightKind::Punctual));
        assert!(!light_kind_attenuates_with_cloud_shadow(LuxLightKind::Area));
        assert!(!light_kind_attenuates_with_cloud_shadow(
            LuxLightKind::EmissiveCandidate,
        ));
        assert!(!light_kind_attenuates_with_cloud_shadow(LuxLightKind::Probe));
        assert!(light_kind_attenuates_with_cloud_shadow(LuxLightKind::Directional));

        // Typed runtime audit — register a typed layer
        // under a typed light id, then run typed every
        // local kind through the typed compose at the
        // typed same light id + assert NO typed
        // attenuation applies even though the typed layer
        // would be found.
        let light_id = LuxLightId::new(7);
        let registry = make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        for kind in [
            LuxLightKind::Punctual,
            LuxLightKind::Area,
            LuxLightKind::EmissiveCandidate,
            LuxLightKind::Probe,
        ] {
            let compose = apply_cloud_layer_to_volumetric_scattering(
                &registry, light_id, kind, 5.0, /* cloud */ 0.1,
            );
            assert!(
                !compose.attenuation_applied,
                "{:?} unexpectedly attenuated",
                kind,
            );
            // Typed local light path → typed final =
            // typed directional scattering input.
            assert!(
                (compose.final_scattering - 5.0).abs() < 1e-6,
                "{:?} altered final scattering",
                kind,
            );
            // Typed `layer_found` reflects whether the
            // typed registry lookup would have matched —
            // but we typed short-circuit BEFORE the
            // lookup for typed local kinds, so it stays
            // false.
            assert!(!compose.layer_found, "{:?} reported layer_found", kind);
            // Typed defense-in-depth audit.
            assert!(compose.applied_to_directional_only());
        }

        // Typed sanity: typed Directional at the typed
        // same light id DOES attenuate.
        let directional = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            5.0,
            0.1,
        );
        assert!(directional.attenuation_applied);
        assert!((directional.final_scattering - 0.5).abs() < 1e-6);
    }

    /// Pass C7.7 acceptance — debug view can show cloud
    /// attenuation applied to Lux volumetrics.  Audited by
    /// emitting the typed
    /// `volumetric_cloud_debug_section` against a typed
    /// summary that captured typed multiple composes.
    #[test]
    fn debug_view_shows_cloud_attenuation_applied_to_volumetrics() {
        let light_id = LuxLightId::new(42);
        let registry =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);

        let mut summary = VolumetricCloudComposeSummary::EMPTY;
        assert!(!summary.any_attenuation_applied());
        assert_eq!(summary.total_compose_count(), 0);

        // Typed three composes: one directional attenuated
        // (cloud=0.3), one directional unattenuated (no
        // layer for typed wrong light id), one local light
        // skipped.
        let c1 = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            10.0,
            0.3,
        );
        summary.record(&c1);
        let c2 = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            LuxLightId::new(99), // wrong id
            LuxLightKind::Directional,
            5.0,
            0.5,
        );
        summary.record(&c2);
        let c3 = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Punctual,
            3.0,
            0.1,
        );
        summary.record(&c3);

        assert_eq!(summary.directional_attenuated_count, 1);
        assert_eq!(summary.directional_skipped_count, 1);
        assert_eq!(summary.local_skipped_count, 1);
        assert_eq!(summary.total_compose_count(), 3);
        assert!(summary.any_attenuation_applied());
        // Typed min factor came from the typed attenuated
        // compose (0.3); typed max factor is 1.0 from the
        // typed unattenuated composes.
        assert!((summary.min_attenuation_factor - 0.3).abs() < 1e-6);
        assert!((summary.max_attenuation_factor - 1.0).abs() < 1e-6);

        // Typed mean = typed (10*0.3 + 5*1 + 3*1) /
        // (10+5+3) = typed 11/18 ≈ 0.611.
        let expected_mean = (10.0 * 0.3 + 5.0 + 3.0) / 18.0;
        assert!((summary.mean_attenuation_factor() - expected_mean).abs() < 1e-5);

        let section = volumetric_cloud_debug_section(&summary);
        assert!(section.contains("Volumetric Cloud Attenuation"));
        assert!(section.contains("total_compose_count: 3"));
        assert!(section.contains("directional_attenuated: 1"));
        assert!(section.contains("directional_skipped: 1"));
        assert!(section.contains("local_skipped: 1"));
        assert!(section.contains("min_attenuation_factor: 0.3000"));
        assert!(section.contains("max_attenuation_factor: 1.0000"));
        assert!(section.contains("any_attenuation_applied: true"));
    }

    /// Pass C7.7 — typed `no_attenuation` baseline.
    #[test]
    fn no_attenuation_baseline_matches_input() {
        let light_id = LuxLightId::new(1);
        let compose =
            VolumetricCloudCompose::no_attenuation(light_id, LuxLightKind::Punctual, 4.0);
        assert!(!compose.attenuation_applied);
        assert!(!compose.layer_found);
        assert_eq!(compose.cloud_transmittance, 1.0);
        assert!((compose.final_scattering - 4.0).abs() < 1e-6);
        assert!(compose.preserves_volumetrics());
        assert!(!compose.cloud_dims_volumetrics());
        assert!(compose.applied_to_directional_only());
        // Typed negative typed scattering input clamps to
        // typed 0.0.
        let clamped =
            VolumetricCloudCompose::no_attenuation(light_id, LuxLightKind::Directional, -2.0);
        assert_eq!(clamped.directional_scattering, 0.0);
        assert_eq!(clamped.final_scattering, 0.0);
    }

    /// Pass C7.7 — typed multiplication contract: when
    /// typed attenuation applies, typed
    /// `final = directional * cloud`.
    #[test]
    fn compose_via_multiplication_audit() {
        let light_id = LuxLightId::new(2);
        let registry = make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        // Typed (scattering, cloud) → typed final = typed
        // product.
        for (scattering, cloud) in [(10.0, 0.5), (4.0, 0.25), (1.0, 1.0), (0.0, 0.7)] {
            let compose = apply_cloud_layer_to_volumetric_scattering(
                &registry,
                light_id,
                LuxLightKind::Directional,
                scattering,
                cloud,
            );
            assert!(compose.composes_via_multiplication());
            let expected = scattering.max(0.0) * cloud.clamp(0.0, 1.0);
            assert!((compose.final_scattering - expected).abs() < 1e-6);
        }
    }

    /// Pass C7.7 — typed both delay modes are explicit at
    /// the typed record layer.
    #[test]
    fn both_delay_modes_are_explicit_on_volumetric_records() {
        let light_id = LuxLightId::new(1);
        let same =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let same_compose = apply_cloud_layer_to_volumetric_scattering(
            &same,
            light_id,
            LuxLightKind::Directional,
            1.0,
            0.5,
        );
        assert!(same_compose.samples_current_frame);
        assert!(!same_compose.samples_previous_frame);

        let delayed =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let delayed_compose = apply_cloud_layer_to_volumetric_scattering(
            &delayed,
            light_id,
            LuxLightKind::Directional,
            1.0,
            0.5,
        );
        assert!(!delayed_compose.samples_current_frame);
        assert!(delayed_compose.samples_previous_frame);
    }

    /// Pass C7.7 — typed `_with_layer` convenience matches
    /// the typed registry-lookup path.
    #[test]
    fn convenience_with_layer_matches_registry_path() {
        let light_id = LuxLightId::new(13);
        let registry = make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let layer = *registry.find(light_id).expect("registered");
        let from_registry = apply_cloud_layer_to_volumetric_scattering(
            &registry,
            light_id,
            LuxLightKind::Directional,
            6.0,
            0.4,
        );
        let from_layer = apply_cloud_layer_to_volumetric_scattering_with_layer(
            &layer,
            light_id,
            LuxLightKind::Directional,
            6.0,
            0.4,
        );
        assert_eq!(from_registry.final_scattering, from_layer.final_scattering);
        assert_eq!(from_registry.latency, from_layer.latency);
        assert_eq!(from_registry.attenuation_applied, from_layer.attenuation_applied);

        // Typed `_with_layer` path also short-circuits for
        // typed local kinds.
        let local_with_layer = apply_cloud_layer_to_volumetric_scattering_with_layer(
            &layer,
            light_id,
            LuxLightKind::Punctual,
            6.0,
            0.4,
        );
        assert!(!local_with_layer.attenuation_applied);
        assert!((local_with_layer.final_scattering - 6.0).abs() < 1e-6);

        // Typed mismatched light id short-circuits.
        let mismatched = apply_cloud_layer_to_volumetric_scattering_with_layer(
            &layer,
            LuxLightId::new(999),
            LuxLightKind::Directional,
            6.0,
            0.4,
        );
        assert!(!mismatched.attenuation_applied);
    }
}
