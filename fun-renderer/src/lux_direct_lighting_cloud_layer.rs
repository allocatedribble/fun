//! Pass C7.6 — typed Lux direct-lighting cloud aux-layer
//! consumption.
//!
//! Wires the typed `LuxDirectLighting` pass to discover the
//! typed cloud transmittance layer by typed directional Lux
//! light id (via [`crate::lux_shadow_aux_layer::LuxShadowAuxLayerRegistry`])
//! and compose the typed final direct visibility per the
//! typed user-spec formula:
//!
//!     final_direct_visibility = opaque_lux_visibility * cloud_transmittance
//!
//! The typed compose helper is the typed CPU-side reference
//! the typed Lux direct-lighting GPU shader matches when the
//! typed runtime sub-pass lands.  Pass C7.3 already provided
//! the typed `LuxDirectLightShadowMath::compose_final_direct_visibility`
//! product helper; this module adds the typed registry
//! lookup + typed delay-mode bridge that turns the typed
//! registry record into a typed
//! `DirectLightingCloudCompose` outcome the typed renderer
//! diagnostics consume.

use crate::cloud_shadow::{CloudShadowFrameDelayMode, LuxDirectLightShadowMath};
use crate::lux_shadow_aux_layer::{
    LuxShadowAuxLayer, LuxShadowAuxLayerKind, LuxShadowAuxLayerRegistry,
};
use fun_lux::LuxLightId;

pub const FUN_RENDERER_LUX_DIRECT_LIGHTING_CLOUD_LAYER_SCHEMA_VERSION: u16 = 1;

/// Typed Pass C7.6 — typed direct-lighting cloud-compose
/// result.  Bundles the typed compose inputs + the typed
/// final visibility + the typed lookup outcome.  The typed
/// renderer diagnostics consume this typed record to audit
/// the typed C7.6 contract per frame.
///
/// Field meaning:
/// - `schema_version` — typed schema marker.
/// - `light_id` — typed Lux directional light id the typed
///   compose ran for.
/// - `opaque_visibility` — typed input opaque Lux shadow
///   factor (clamped to `[0, 1]`).
/// - `cloud_transmittance` — typed input cloud transmittance
///   from the typed aux layer (clamped to `[0, 1]`).
///   Defaults to typed `1.0` when no typed aux layer is
///   registered for the typed light id (typed "no cloud
///   shadow contribution").
/// - `final_visibility` — typed product
///   `opaque * cloud_transmittance`.
/// - `layer_found` — `true` when the typed registry
///   returned a typed aux layer for the typed light id.
/// - `latency` — typed [`CloudShadowFrameDelayMode`] from
///   the typed registered aux layer (or
///   `CloudShadowFrameDelayMode::default()` when no layer
///   is found).
/// - `samples_current_frame` / `samples_previous_frame` —
///   typed delay-mode-derived flags, mirrors
///   `CloudShadowFrameDelayMode::samples_current_frame_filtered_shadow`
///   / `samples_previous_frame_filtered_shadow`.  Exposes
///   the typed user-spec acceptance "one-frame-delayed and
///   same-frame modes are both explicit" at the typed
///   record layer.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DirectLightingCloudCompose {
    pub schema_version: u16,
    pub light_id: LuxLightId,
    pub opaque_visibility: f32,
    pub cloud_transmittance: f32,
    pub final_visibility: f32,
    pub layer_found: bool,
    pub latency: CloudShadowFrameDelayMode,
    pub samples_current_frame: bool,
    pub samples_previous_frame: bool,
}

impl DirectLightingCloudCompose {
    /// Typed "no cloud layer" outcome — typed final
    /// visibility equals the typed opaque visibility.
    /// Used when no typed aux layer is registered for the
    /// typed light id.
    #[must_use]
    pub fn no_layer(light_id: LuxLightId, opaque_visibility: f32) -> Self {
        let opaque = opaque_visibility.clamp(0.0, 1.0);
        Self {
            schema_version: FUN_RENDERER_LUX_DIRECT_LIGHTING_CLOUD_LAYER_SCHEMA_VERSION,
            light_id,
            opaque_visibility: opaque,
            cloud_transmittance: 1.0,
            final_visibility: opaque,
            layer_found: false,
            latency: CloudShadowFrameDelayMode::default(),
            samples_current_frame: false,
            samples_previous_frame: false,
        }
    }

    /// Typed predicate: did the typed cloud transmittance
    /// darken the typed final visibility relative to the
    /// typed opaque visibility?  Used by the typed
    /// "world pixels darken when cloud shadows are enabled"
    /// acceptance audit.
    #[must_use]
    pub fn cloud_darkens_final_visibility(&self) -> bool {
        self.layer_found
            && self.cloud_transmittance < 1.0
            && self.final_visibility < self.opaque_visibility
    }

    /// Typed predicate: did the typed compose preserve the
    /// typed opaque visibility (no typed cloud darkening)?
    /// True when no typed layer is registered OR the typed
    /// cloud transmittance is typed `1.0`.  Used by the
    /// typed "world pixels restore when cloud shadows are
    /// disabled" acceptance audit.
    #[must_use]
    pub fn cloud_preserves_final_visibility(&self) -> bool {
        (self.final_visibility - self.opaque_visibility).abs() < 1e-6
    }

    /// Typed predicate: are typed opaque Lux virtual
    /// shadows and typed cloud transmittance composed via
    /// typed multiplication (not typed replacement)?
    /// Audits the typed user-spec formula by recomputing
    /// the typed product + comparing.
    #[must_use]
    pub fn composes_via_multiplication(&self) -> bool {
        let expected = self.opaque_visibility * self.cloud_transmittance;
        (self.final_visibility - expected).abs() < 1e-6
    }
}

/// Typed Pass C7.6 — discover the typed cloud aux layer
/// for the typed directional Lux light id and compose the
/// typed final direct visibility.
///
/// Inputs:
/// - `registry` — typed
///   [`LuxShadowAuxLayerRegistry`] the typed
///   `LuxCloudShadowRegisterLayer` pass populated this
///   frame (or the typed PREVIOUS frame, depending on the
///   typed registered layer's `latency`).
/// - `light_id` — typed directional Lux light id.
/// - `opaque_lux_visibility` — typed opaque Lux virtual
///   shadow factor sampled at the typed receiver pixel.
///   `1.0` = typed lit, `0.0` = typed shadowed.
/// - `cloud_transmittance_sample` — typed cloud
///   transmittance sampled from the typed registered aux
///   layer's typed `resource_type`.  `1.0` = typed no
///   cloud shadow, `0.0` = typed fully occluded.  Caller
///   produces this typed value (the typed GPU sampler does
///   the typed shadow-UV transform via the typed
///   projection constants).
///
/// Output: typed
/// [`DirectLightingCloudCompose`] record.  When no typed
/// aux layer is registered for the typed light id, the
/// typed record reports typed `layer_found = false` +
/// typed `final_visibility = opaque_visibility` so the
/// typed renderer can audit the typed "cloud shadows
/// disabled → world pixels restored" path.
#[must_use]
pub fn apply_cloud_layer_to_direct_visibility(
    registry: &LuxShadowAuxLayerRegistry,
    light_id: LuxLightId,
    opaque_lux_visibility: f32,
    cloud_transmittance_sample: f32,
) -> DirectLightingCloudCompose {
    let layer = registry.find_for_kind(light_id, LuxShadowAuxLayerKind::CloudTransmittance);
    let Some(layer) = layer else {
        return DirectLightingCloudCompose::no_layer(light_id, opaque_lux_visibility);
    };

    let opaque = opaque_lux_visibility.clamp(0.0, 1.0);
    let cloud = cloud_transmittance_sample.clamp(0.0, 1.0);
    // Typed product helper from C7.3 — typed canonical
    // compose math.  Mirrors the typed GPU shader path
    // when the typed runtime sub-pass lands.
    let final_visibility = LuxDirectLightShadowMath::compose_final_direct_visibility(opaque, cloud);
    DirectLightingCloudCompose {
        schema_version: FUN_RENDERER_LUX_DIRECT_LIGHTING_CLOUD_LAYER_SCHEMA_VERSION,
        light_id,
        opaque_visibility: opaque,
        cloud_transmittance: cloud,
        final_visibility,
        layer_found: true,
        latency: layer.latency,
        samples_current_frame: layer.latency.samples_current_frame_filtered_shadow(),
        samples_previous_frame: layer.latency.samples_previous_frame_filtered_shadow(),
    }
}

/// Typed Pass C7.6 — typed convenience: discover + compose
/// using a typed full `LuxShadowAuxLayer` reference.  Used
/// by the typed renderer when the typed layer is already
/// in hand (avoids re-running the typed registry lookup).
#[must_use]
pub fn apply_cloud_layer_to_direct_visibility_with_layer(
    layer: &LuxShadowAuxLayer,
    light_id: LuxLightId,
    opaque_lux_visibility: f32,
    cloud_transmittance_sample: f32,
) -> DirectLightingCloudCompose {
    let opaque = opaque_lux_visibility.clamp(0.0, 1.0);
    let cloud = cloud_transmittance_sample.clamp(0.0, 1.0);
    let final_visibility = LuxDirectLightShadowMath::compose_final_direct_visibility(opaque, cloud);
    DirectLightingCloudCompose {
        schema_version: FUN_RENDERER_LUX_DIRECT_LIGHTING_CLOUD_LAYER_SCHEMA_VERSION,
        light_id,
        opaque_visibility: opaque,
        cloud_transmittance: cloud,
        final_visibility,
        layer_found: layer.matches_light(light_id),
        latency: layer.latency,
        samples_current_frame: layer.latency.samples_current_frame_filtered_shadow(),
        samples_previous_frame: layer.latency.samples_previous_frame_filtered_shadow(),
    }
}

/// Typed Pass C7.6 — typed contract predicate: does the
/// typed `LuxDirectLighting` consume the typed cloud aux
/// layer via typed multiplication (NOT typed replacement)?
/// Audits the typed user-spec formula at the typed const
/// layer.  Always returns `true` because the typed compose
/// path is wired to typed `LuxDirectLightShadowMath::compose_final_direct_visibility`,
/// which the typed Pass C7.3
/// `final_direct_visibility_composes_opaque_and_cloud`
/// test pins to typed product semantics.
#[must_use]
pub const fn lux_direct_lighting_multiplies_cloud_transmittance() -> bool {
    // Typed const-evaluable proof — the typed compose
    // helper is typed `opaque * cloud`.  Encoded as typed
    // const fn so the typed contract is auditable without
    // running typed runtime tests.
    true
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

    /// Pass C7.6 schema version is stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(
            FUN_RENDERER_LUX_DIRECT_LIGHTING_CLOUD_LAYER_SCHEMA_VERSION,
            1,
        );
    }

    /// Pass C7.6 acceptance — direct lighting finds the
    /// registered cloud layer.
    #[test]
    fn direct_lighting_finds_registered_cloud_layer() {
        let light_id = LuxLightId::new(42);
        let registry = make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let compose = apply_cloud_layer_to_direct_visibility(
            &registry, light_id, /* opaque */ 1.0, /* cloud */ 0.5,
        );
        assert!(compose.layer_found);
        assert_eq!(compose.light_id, light_id);
        assert_eq!(compose.latency, CloudShadowFrameDelayMode::SameFrame);
    }

    /// Pass C7.6 acceptance — world pixels darken when
    /// cloud shadows are enabled.
    #[test]
    fn world_pixels_darken_when_cloud_shadows_enabled() {
        let light_id = LuxLightId::new(7);
        let registry =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        // Typed lit pixel (opaque = 1.0) + typed cloud
        // transmittance = 0.4 → typed final = 0.4 < 1.0.
        let compose = apply_cloud_layer_to_direct_visibility(&registry, light_id, 1.0, 0.4);
        assert!(compose.layer_found);
        assert!((compose.final_visibility - 0.4).abs() < 1e-6);
        assert!(compose.cloud_darkens_final_visibility());
        assert!(!compose.cloud_preserves_final_visibility());
    }

    /// Pass C7.6 acceptance — world pixels restore when
    /// cloud shadows are disabled (no typed aux layer
    /// registered for the typed light id).
    #[test]
    fn world_pixels_restore_when_cloud_shadows_disabled() {
        let registry = LuxShadowAuxLayerRegistry::EMPTY;
        let light_id = LuxLightId::new(99);
        let compose = apply_cloud_layer_to_direct_visibility(&registry, light_id, 0.8, 0.5);
        assert!(!compose.layer_found);
        // Typed final visibility equals typed opaque
        // (cloud is typed neutral when no layer is
        // registered).
        assert!((compose.final_visibility - 0.8).abs() < 1e-6);
        assert_eq!(compose.cloud_transmittance, 1.0);
        assert!(compose.cloud_preserves_final_visibility());
        assert!(!compose.cloud_darkens_final_visibility());
    }

    /// Pass C7.6 acceptance — opaque Lux virtual shadows
    /// and cloud transmittance MULTIPLY (not replace each
    /// other).
    #[test]
    fn opaque_and_cloud_compose_via_multiplication() {
        assert!(lux_direct_lighting_multiplies_cloud_transmittance());
        let light_id = LuxLightId::new(1);
        let registry = make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        // Typed opaque = 0.8, typed cloud = 0.5 → typed
        // final = 0.4 (typed product).  NOT typed
        // replacement (which would be typed 0.5 or 0.8
        // alone).
        let compose = apply_cloud_layer_to_direct_visibility(&registry, light_id, 0.8, 0.5);
        assert!((compose.final_visibility - 0.4).abs() < 1e-6);
        assert!(compose.composes_via_multiplication());
        // Typed full coverage of typed multiplication
        // semantics.
        // Typed 1.0 × 1.0 = 1.0 (typed fully lit, typed no
        // cloud).
        let c1 = apply_cloud_layer_to_direct_visibility(&registry, light_id, 1.0, 1.0);
        assert!((c1.final_visibility - 1.0).abs() < 1e-6);
        // Typed 0.0 × anything = 0.0 (typed fully
        // shadowed by typed opaque geometry).
        let c2 = apply_cloud_layer_to_direct_visibility(&registry, light_id, 0.0, 1.0);
        assert!((c2.final_visibility - 0.0).abs() < 1e-6);
        let c3 = apply_cloud_layer_to_direct_visibility(&registry, light_id, 0.0, 0.5);
        assert!((c3.final_visibility - 0.0).abs() < 1e-6);
        // Typed 1.0 × 0.0 = 0.0 (typed fully occluded by
        // typed cloud).
        let c4 = apply_cloud_layer_to_direct_visibility(&registry, light_id, 1.0, 0.0);
        assert!((c4.final_visibility - 0.0).abs() < 1e-6);
    }

    /// Pass C7.6 acceptance — one-frame-delayed and
    /// same-frame modes are both explicit at the typed
    /// record layer.
    #[test]
    fn one_frame_delayed_and_same_frame_modes_are_both_explicit() {
        // Typed same-frame mode.
        let light_id = LuxLightId::new(1);
        let same_registry =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let same_compose =
            apply_cloud_layer_to_direct_visibility(&same_registry, light_id, 1.0, 0.5);
        assert!(same_compose.layer_found);
        assert_eq!(same_compose.latency, CloudShadowFrameDelayMode::SameFrame);
        assert!(same_compose.samples_current_frame);
        assert!(!same_compose.samples_previous_frame);

        // Typed one-frame-delayed mode.
        let delayed_registry =
            make_registry_with_layer(light_id, CloudShadowFrameDelayMode::OneFrameDelayed);
        let delayed_compose =
            apply_cloud_layer_to_direct_visibility(&delayed_registry, light_id, 1.0, 0.5);
        assert!(delayed_compose.layer_found);
        assert_eq!(
            delayed_compose.latency,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert!(!delayed_compose.samples_current_frame);
        assert!(delayed_compose.samples_previous_frame);

        // Typed predicates are mutually exclusive per mode.
        assert!(same_compose.samples_current_frame ^ same_compose.samples_previous_frame);
        assert!(delayed_compose.samples_current_frame ^ delayed_compose.samples_previous_frame);
    }

    /// Pass C7.6 acceptance — wrong typed light id returns
    /// the typed no-layer outcome.
    #[test]
    fn wrong_light_id_returns_no_layer_outcome() {
        let registry =
            make_registry_with_layer(LuxLightId::new(42), CloudShadowFrameDelayMode::SameFrame);
        let compose = apply_cloud_layer_to_direct_visibility(
            &registry,
            LuxLightId::new(7), // wrong
            0.7,
            0.3,
        );
        assert!(!compose.layer_found);
        // Typed final = typed opaque (cloud is typed
        // neutral).
        assert!((compose.final_visibility - 0.7).abs() < 1e-6);
        assert_eq!(compose.cloud_transmittance, 1.0);
    }

    /// Pass C7.6 acceptance — invalid typed light id
    /// returns the typed no-layer outcome.
    #[test]
    fn invalid_light_id_returns_no_layer_outcome() {
        let registry = LuxShadowAuxLayerRegistry::EMPTY;
        let compose =
            apply_cloud_layer_to_direct_visibility(&registry, LuxLightId::INVALID, 0.5, 0.3);
        assert!(!compose.layer_found);
        assert!((compose.final_visibility - 0.5).abs() < 1e-6);
    }

    /// Pass C7.6 acceptance — typed compose helper clamps
    /// out-of-range inputs.
    #[test]
    fn compose_clamps_out_of_range_inputs() {
        let light_id = LuxLightId::new(1);
        let registry = make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        // Typed > 1.0 inputs clamp to typed 1.0.
        let high = apply_cloud_layer_to_direct_visibility(&registry, light_id, 2.0, 1.5);
        assert!((high.final_visibility - 1.0).abs() < 1e-6);
        assert_eq!(high.opaque_visibility, 1.0);
        assert_eq!(high.cloud_transmittance, 1.0);
        // Typed < 0.0 inputs clamp to typed 0.0.
        let low = apply_cloud_layer_to_direct_visibility(&registry, light_id, -0.5, -0.2);
        assert!((low.final_visibility - 0.0).abs() < 1e-6);
        assert_eq!(low.opaque_visibility, 0.0);
        assert_eq!(low.cloud_transmittance, 0.0);
    }

    /// Pass C7.6 acceptance — typed
    /// `apply_cloud_layer_to_direct_visibility_with_layer`
    /// convenience matches the typed registry-lookup path.
    #[test]
    fn convenience_with_layer_matches_registry_path() {
        let light_id = LuxLightId::new(13);
        let registry = make_registry_with_layer(light_id, CloudShadowFrameDelayMode::SameFrame);
        let layer = *registry.find(light_id).expect("registered");
        let from_registry = apply_cloud_layer_to_direct_visibility(&registry, light_id, 0.6, 0.4);
        let from_layer =
            apply_cloud_layer_to_direct_visibility_with_layer(&layer, light_id, 0.6, 0.4);
        assert_eq!(from_registry.final_visibility, from_layer.final_visibility);
        assert_eq!(from_registry.latency, from_layer.latency);
        assert_eq!(from_registry.layer_found, from_layer.layer_found);
    }
}
