//! Pass C7.4.6 — typed Lux shadow aux-layer registration.
//!
//! Pass C7.4.5 wired the typed `LuxCloudShadowRegisterLayer`
//! pass-recording function + the typed `CloudShadowAuxLayer`
//! resource type the typed register pass writes.  This
//! module lands the typed sidecar metadata the typed
//! register pass produces — the typed
//! [`LuxShadowAuxLayer`] record that maps a typed directional
//! Lux light to a typed cloud-shadow aux layer the typed
//! `LuxDirectLighting` + (future) `LuxVolumetricLightInject`
//! passes consume.
//!
//! Contract — typed cloud shadow MUST NOT be baked into
//! opaque virtual shadow depth.  The typed aux layer is a
//! typed separate sidecar that lives in its own typed
//! cloud-owned namespace; the typed
//! `is_not_opaque_shadow_depth` predicate audits the typed
//! contract at the typed record level.  Pass C7.3
//! `cloud_shadows_not_baked_into_opaque_depth` is the typed
//! resource-taxonomy audit; this module is the typed
//! record-shape audit.

use crate::cloud_shadow::{CloudShadowFrameDelayMode, CloudShadowProjectionConstants};
use crate::clouds::CloudRenderSettings;
use crate::frame_graph::FrameGraphResourceType;
use fun_lux::LuxLightId;

pub const FUN_RENDERER_LUX_SHADOW_AUX_LAYER_SCHEMA_VERSION: u16 = 1;

/// Typed maximum number of aux-layer slots the typed
/// [`LuxShadowAuxLayerRegistry`] holds.  Sized for typed
/// product cases — typical scenes have one typed directional
/// sun light, so a small fixed bound keeps the typed
/// registry typed `Copy + Eq + Hash` without an allocation.
pub const LUX_SHADOW_AUX_LAYER_MAX: usize = 4;

// ============================================================================
// Section 1 — typed LuxShadowAuxLayerKind
// ============================================================================

/// Typed Pass C7.4.6 Lux shadow aux-layer kind.  Names the
/// typed cloud-shadow products that can register a typed
/// aux layer.  Today only the typed cloud transmittance
/// product registers; future kinds (e.g., cloud Lux
/// scattering) extend this enum.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowAuxLayerKind {
    /// Typed cloud transmittance aux layer.  Produced by
    /// the typed C7.4 cloud-shadow chain (project → filter
    /// → register).  Sampled by typed `LuxDirectLighting`
    /// alongside the typed Lux virtual shadow pages.
    #[default]
    CloudTransmittance,
}

impl LuxShadowAuxLayerKind {
    pub const ALL: [Self; 1] = [Self::CloudTransmittance];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CloudTransmittance => "cloud_transmittance",
        }
    }

    /// Typed default resource type the typed kind writes /
    /// reads.  Drives the typed
    /// `LuxShadowAuxLayer::resource_type` builder default.
    #[must_use]
    pub const fn default_resource_type(self) -> FrameGraphResourceType {
        match self {
            Self::CloudTransmittance => FrameGraphResourceType::CloudWorldShadowFiltered,
        }
    }

    /// Typed default projection-constants resource type.
    #[must_use]
    pub const fn default_projection_constants(self) -> FrameGraphResourceType {
        match self {
            Self::CloudTransmittance => FrameGraphResourceType::CloudShadowProjectionConstants,
        }
    }
}

// ============================================================================
// Section 2 — typed LuxShadowAuxLayer
// ============================================================================

/// Typed Pass C7.4.6 Lux shadow aux-layer record.  Bundles
/// the typed light id, kind, resource handles, opacity /
/// softness knobs, and latency mode into a typed sidecar
/// the typed `LuxDirectLighting` pass reads alongside the
/// typed Lux virtual shadow pages.
///
/// Fields:
/// - `light_id` — typed Lux directional light id that owns
///   the typed aux layer.  MUST be valid; the typed
///   `matches_light` predicate audits the typed lookup.
/// - `kind` — typed [`LuxShadowAuxLayerKind`] discriminant.
/// - `resource_type` — typed [`FrameGraphResourceType`] the
///   typed direct-lighting pass samples (typed
///   `CloudWorldShadowFiltered` for the typed
///   `CloudTransmittance` kind).  MUST NOT be a typed Lux
///   opaque shadow resource — audited by typed
///   `is_not_opaque_shadow_depth`.
/// - `projection_constants` — typed [`FrameGraphResourceType`]
///   of the typed `CloudShadowProjectionConstants` uniform
///   buffer the typed sampler needs for the typed world →
///   shadow-UV transform.
/// - `opacity_q16` — Q16 typed opacity scale (mirrors the
///   typed `CloudWorldShadowSettings::opacity_scale_q16`).
/// - `softness_q16` — Q16 typed softness (mirrors the typed
///   `CloudWorldShadowSettings::softness_q16`).
/// - `latency` — typed [`CloudShadowFrameDelayMode`]
///   carrying the typed one-frame-delayed vs same-frame
///   intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxShadowAuxLayer {
    pub light_id: LuxLightId,
    pub kind: LuxShadowAuxLayerKind,
    pub resource_type: FrameGraphResourceType,
    pub projection_constants: FrameGraphResourceType,
    pub opacity_q16: u16,
    pub softness_q16: u16,
    pub latency: CloudShadowFrameDelayMode,
}

impl LuxShadowAuxLayer {
    /// Typed Pass C7.4.6 const predicate: does this typed
    /// aux layer record describe a typed non-opaque shadow
    /// source?  Returns `false` if the typed resource type
    /// or the typed projection-constants type reports
    /// `is_lux() == true` (typed Lux opaque shadow types
    /// like `LuxVirtualShadowPages` / `LuxShadowAtlas`).
    /// The typed C7.4.6 contract refuses baking cloud
    /// shadows into the typed opaque shadow depth.
    #[must_use]
    pub const fn is_not_opaque_shadow_depth(&self) -> bool {
        !self.resource_type.is_lux() && !self.projection_constants.is_lux()
    }

    /// Typed Pass C7.4.6 predicate: does this typed aux
    /// layer belong to the typed light_id?
    #[must_use]
    pub fn matches_light(&self, light_id: LuxLightId) -> bool {
        self.light_id == light_id && self.light_id.is_valid()
    }

    /// Typed Pass C7.4.6 predicate: is this typed aux
    /// layer the typed cloud-transmittance kind?
    #[must_use]
    pub const fn is_cloud_transmittance(&self) -> bool {
        matches!(self.kind, LuxShadowAuxLayerKind::CloudTransmittance)
    }

    /// Typed Pass C7.4.6 builder — derive a typed
    /// `CloudTransmittance` aux layer from typed
    /// `CloudRenderSettings` + `CloudShadowProjectionConstants`
    /// + typed latency mode.
    ///
    /// Returns `None` when the typed projection constants
    /// describe a typed disabled/gated-off projection
    /// (`projects_world_shadow == false`) or when the typed
    /// light id is invalid — in those typed cases the typed
    /// register pass produces no typed aux layer + the
    /// typed downstream lookup returns `None`.
    #[must_use]
    pub fn cloud_transmittance_from_inputs(
        settings: &CloudRenderSettings,
        constants: &CloudShadowProjectionConstants,
        latency: CloudShadowFrameDelayMode,
    ) -> Option<Self> {
        if !constants.projects_world_shadow() {
            return None;
        }
        if !constants.light_id.is_valid() {
            return None;
        }
        if !settings.registers_world_shadow_pass() {
            return None;
        }
        Some(Self {
            light_id: constants.light_id,
            kind: LuxShadowAuxLayerKind::CloudTransmittance,
            resource_type: LuxShadowAuxLayerKind::CloudTransmittance.default_resource_type(),
            projection_constants: LuxShadowAuxLayerKind::CloudTransmittance
                .default_projection_constants(),
            opacity_q16: settings.world_shadows.opacity_scale_q16,
            softness_q16: settings.world_shadows.softness_q16,
            latency,
        })
    }
}

// ============================================================================
// Section 3 — typed LuxShadowAuxLayerRegistry
// ============================================================================

/// Typed Pass C7.4.6 registry error.  Reported when the
/// typed `LuxShadowAuxLayerRegistry::register` cannot accept
/// a typed new layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowAuxLayerRegistryError {
    /// Typed registry is full (already holds
    /// `LUX_SHADOW_AUX_LAYER_MAX` layers).  The typed
    /// renderer-owned register pass either drops the typed
    /// new layer or evicts the typed oldest.
    RegistryFull,
    /// Typed duplicate `(light_id, kind)` already
    /// registered.  The typed register pass either overwrites
    /// or skips depending on the typed renderer policy.
    DuplicateLightAndKind,
    /// Typed invalid light id (typed
    /// `LuxLightId::INVALID`).  The typed contract refuses
    /// registering an aux layer without a typed owning
    /// directional Lux light.
    InvalidLightId,
}

impl LuxShadowAuxLayerRegistryError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegistryFull => "registry_full",
            Self::DuplicateLightAndKind => "duplicate_light_and_kind",
            Self::InvalidLightId => "invalid_light_id",
        }
    }
}

/// Typed Pass C7.4.6 Lux shadow aux-layer registry.
/// Fixed-size, typed `Copy + Eq + Hash`.  Holds up to
/// [`LUX_SHADOW_AUX_LAYER_MAX`] typed aux layers — typical
/// scenes have one typed directional sun light so the typed
/// bound is generous.
///
/// The typed `LuxCloudShadowRegisterLayer` pass calls typed
/// [`Self::register`] each frame to register the typed
/// current cloud-shadow aux layer; typed `LuxDirectLighting`
/// (and future typed `LuxVolumetricLightInject`) calls
/// typed [`Self::find`] to discover the typed layer by
/// typed light id.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxShadowAuxLayerRegistry {
    layers: [Option<LuxShadowAuxLayer>; LUX_SHADOW_AUX_LAYER_MAX],
    count: u8,
}

impl LuxShadowAuxLayerRegistry {
    /// Typed empty registry — no aux layers registered.
    pub const EMPTY: Self = Self {
        layers: [None; LUX_SHADOW_AUX_LAYER_MAX],
        count: 0,
    };

    /// Typed Pass C7.4.6 — register a typed aux layer.
    /// Refuses typed duplicate `(light_id, kind)` pairs +
    /// typed invalid light ids + typed full registry.
    pub fn register(
        &mut self,
        layer: LuxShadowAuxLayer,
    ) -> Result<(), LuxShadowAuxLayerRegistryError> {
        if !layer.light_id.is_valid() {
            return Err(LuxShadowAuxLayerRegistryError::InvalidLightId);
        }
        for existing in self.layers.iter().flatten() {
            if existing.light_id == layer.light_id && existing.kind == layer.kind {
                return Err(LuxShadowAuxLayerRegistryError::DuplicateLightAndKind);
            }
        }
        for slot in &mut self.layers {
            if slot.is_none() {
                *slot = Some(layer);
                self.count = self.count.saturating_add(1);
                return Ok(());
            }
        }
        Err(LuxShadowAuxLayerRegistryError::RegistryFull)
    }

    /// Typed Pass C7.4.6 — find the typed first aux layer
    /// registered for the typed light id.  Returns `None`
    /// when no typed aux layer matches.  Used by typed
    /// `LuxDirectLighting` to discover the typed cloud
    /// shadow layer + by typed `LuxVolumetricLightInject`
    /// (future) to discover the typed same layer.
    #[must_use]
    pub fn find(&self, light_id: LuxLightId) -> Option<&LuxShadowAuxLayer> {
        if !light_id.is_valid() {
            return None;
        }
        self.layers
            .iter()
            .flatten()
            .find(|layer| layer.matches_light(light_id))
    }

    /// Typed Pass C7.4.6 — find the typed aux layer
    /// registered for the typed light id + kind tuple.
    /// Useful when the typed `LuxDirectLighting` pass wants
    /// to discover only the typed cloud transmittance layer
    /// (ignoring typed future kinds).
    #[must_use]
    pub fn find_for_kind(
        &self,
        light_id: LuxLightId,
        kind: LuxShadowAuxLayerKind,
    ) -> Option<&LuxShadowAuxLayer> {
        if !light_id.is_valid() {
            return None;
        }
        self.layers
            .iter()
            .flatten()
            .find(|layer| layer.matches_light(light_id) && layer.kind == kind)
    }

    /// Typed count of registered typed aux layers.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Typed predicate: is the typed registry empty?
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Typed iterator over registered typed aux layers.
    /// Skips typed empty slots.
    pub fn iter(&self) -> impl Iterator<Item = &LuxShadowAuxLayer> {
        self.layers.iter().filter_map(|slot| slot.as_ref())
    }

    /// Typed Pass C7.4.6 — clear the typed registry.  Used
    /// at typed frame boundaries when the typed register
    /// pass rebuilds the typed registry from scratch each
    /// frame.
    pub fn clear(&mut self) {
        self.layers = [None; LUX_SHADOW_AUX_LAYER_MAX];
        self.count = 0;
    }
}

// ============================================================================
// Section 4 — typed integration helper
// ============================================================================

/// Typed Pass C7.4.6 — register the typed cloud-shadow
/// aux layer the typed `LuxCloudShadowRegisterLayer` pass
/// produces.  Composes the typed
/// `LuxShadowAuxLayer::cloud_transmittance_from_inputs`
/// builder + the typed `LuxShadowAuxLayerRegistry::register`
/// call.
///
/// Returns:
/// - `Ok(Some(layer))` — typed layer built + registered.
/// - `Ok(None)` — typed projection gated off (settings /
///   constants disabled); typed registry unchanged.
/// - `Err(...)` — typed registry rejected the typed layer
///   (typed duplicate / typed full / typed invalid id).
pub fn register_cloud_shadow_aux_layer(
    settings: &CloudRenderSettings,
    constants: &CloudShadowProjectionConstants,
    latency: CloudShadowFrameDelayMode,
    registry: &mut LuxShadowAuxLayerRegistry,
) -> Result<Option<LuxShadowAuxLayer>, LuxShadowAuxLayerRegistryError> {
    match LuxShadowAuxLayer::cloud_transmittance_from_inputs(settings, constants, latency) {
        Some(layer) => {
            registry.register(layer)?;
            Ok(Some(layer))
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clouds::{CloudQuality, CloudWeatherProfileId};

    fn make_live_constants(light_id: LuxLightId) -> CloudShadowProjectionConstants {
        CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            light_id,
            [0.0, 1.0, 0.0],
            0,
        )
    }

    /// Pass C7.4.6 acceptance — typed schema version is
    /// stable.
    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_LUX_SHADOW_AUX_LAYER_SCHEMA_VERSION, 1);
    }

    /// Pass C7.4.6 acceptance — typed
    /// `LuxShadowAuxLayerKind` taxonomy is dense.
    #[test]
    fn lux_shadow_aux_layer_kind_taxonomy_is_dense() {
        assert_eq!(LuxShadowAuxLayerKind::ALL.len(), 1);
        assert_eq!(
            LuxShadowAuxLayerKind::CloudTransmittance.as_str(),
            "cloud_transmittance",
        );
        // Typed default is the typed cloud-transmittance kind.
        assert_eq!(
            LuxShadowAuxLayerKind::default(),
            LuxShadowAuxLayerKind::CloudTransmittance,
        );
        // Typed default resource types are typed cloud-owned.
        assert_eq!(
            LuxShadowAuxLayerKind::CloudTransmittance.default_resource_type(),
            FrameGraphResourceType::CloudWorldShadowFiltered,
        );
        assert_eq!(
            LuxShadowAuxLayerKind::CloudTransmittance.default_projection_constants(),
            FrameGraphResourceType::CloudShadowProjectionConstants,
        );
    }

    /// Pass C7.4.6 acceptance — typed cloud shadow layer is
    /// associated with a typed directional Lux light.
    #[test]
    fn cloud_shadow_layer_is_associated_with_directional_lux_light() {
        let light_id = LuxLightId::new(42);
        let constants = make_live_constants(light_id);
        let layer = LuxShadowAuxLayer::cloud_transmittance_from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        )
        .expect("layer should build");
        assert_eq!(layer.light_id, light_id);
        assert!(layer.matches_light(light_id));
        assert!(!layer.matches_light(LuxLightId::new(7))); // wrong id
        assert!(!layer.matches_light(LuxLightId::INVALID)); // invalid id
        // Typed kind defaults to typed CloudTransmittance.
        assert!(layer.is_cloud_transmittance());
        // Typed opacity / softness mirror the typed settings.
        assert_eq!(
            layer.opacity_q16,
            CloudRenderSettings::PRODUCT_DEFAULT
                .world_shadows
                .opacity_scale_q16,
        );
        assert_eq!(
            layer.softness_q16,
            CloudRenderSettings::PRODUCT_DEFAULT.world_shadows.softness_q16,
        );
        assert_eq!(layer.latency, CloudShadowFrameDelayMode::OneFrameDelayed);
    }

    /// Pass C7.4.6 acceptance — typed cloud shadow layer is
    /// NOT treated as typed opaque shadow depth.  Audited
    /// at the typed record layer (resource_type +
    /// projection_constants both report typed
    /// `is_lux() == false`).
    #[test]
    fn cloud_shadow_layer_is_not_opaque_shadow_depth() {
        let light_id = LuxLightId::new(1);
        let constants = make_live_constants(light_id);
        let layer = LuxShadowAuxLayer::cloud_transmittance_from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
        )
        .expect("layer should build");
        assert!(layer.is_not_opaque_shadow_depth());
        assert!(!layer.resource_type.is_lux());
        assert!(!layer.projection_constants.is_lux());
        // Typed sanity: typed resource types are typed
        // cloud-owned (typed string prefix `cloud_`).
        assert!(layer.resource_type.as_str().starts_with("cloud_"));
        assert!(layer.projection_constants.as_str().starts_with("cloud_"));
        // Typed hand-constructed bad case: if a typed caller
        // tries to point the typed aux layer at a typed
        // typed Lux opaque shadow resource, the typed
        // predicate flags it.
        let mut bad = layer;
        bad.resource_type = FrameGraphResourceType::LuxVirtualShadowPages;
        assert!(!bad.is_not_opaque_shadow_depth());
        let mut bad2 = layer;
        bad2.projection_constants = FrameGraphResourceType::LuxShadowAtlas;
        assert!(!bad2.is_not_opaque_shadow_depth());
    }

    /// Pass C7.4.6 acceptance — typed direct lighting can
    /// discover the typed aux layer by typed Lux light id.
    /// (typed `LuxVolumetricLightInject` will use the typed
    /// same `find` API later.)
    #[test]
    fn direct_lighting_can_discover_aux_layer_by_lux_light_id() {
        let light_id = LuxLightId::new(99);
        let constants = make_live_constants(light_id);
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        let registered = register_cloud_shadow_aux_layer(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &mut registry,
        )
        .expect("registration should succeed")
        .expect("layer should build");
        assert_eq!(registry.len(), 1);
        // Typed lookup by typed light id returns the typed
        // layer.
        let found = registry.find(light_id).expect("lookup should find");
        assert_eq!(*found, registered);
        // Typed lookup by typed kind also returns the typed
        // layer.
        let found_kind = registry
            .find_for_kind(light_id, LuxShadowAuxLayerKind::CloudTransmittance)
            .expect("kind lookup should find");
        assert_eq!(*found_kind, registered);
        // Typed lookup by typed wrong light id returns
        // `None`.
        assert!(registry.find(LuxLightId::new(7)).is_none());
        // Typed lookup by typed invalid light id returns
        // `None`.
        assert!(registry.find(LuxLightId::INVALID).is_none());
    }

    /// Pass C7.4.6 acceptance — typed volumetric light
    /// injection can discover the typed layer by typed Lux
    /// light id later.  Mirrors the typed direct lighting
    /// lookup since both consumers use the typed same
    /// `find` API.
    #[test]
    fn volumetric_light_injection_can_discover_aux_layer_by_lux_light_id_later() {
        let light_id = LuxLightId::new(10);
        let constants = make_live_constants(light_id);
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        register_cloud_shadow_aux_layer(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &mut registry,
        )
        .expect("registration should succeed");
        // Typed volumetric light injection uses the typed
        // same lookup the typed direct lighting uses.  The
        // typed lookup is typed lookup-by-light-id, which
        // every typed Lux pass that wants the typed cloud
        // shadow can call.
        let layer = registry.find(light_id).expect("volumetric lookup");
        assert_eq!(layer.light_id, light_id);
        assert!(layer.is_cloud_transmittance());
    }

    /// Pass C7.4.6 acceptance — typed disabled settings
    /// produce typed `None` from the typed builder; typed
    /// registry remains empty.
    #[test]
    fn disabled_settings_produce_no_aux_layer() {
        let constants = CloudShadowProjectionConstants::DISABLED;
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let result = register_cloud_shadow_aux_layer(
            &CloudRenderSettings::DISABLED,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &mut registry,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
        assert!(registry.is_empty());
        // Typed `Off` quality cascade-disables.
        let mut settings_off = CloudRenderSettings::PRODUCT_DEFAULT;
        settings_off.quality = CloudQuality::Off;
        let result = register_cloud_shadow_aux_layer(
            &settings_off,
            &make_live_constants(LuxLightId::new(1)),
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &mut registry,
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
        assert!(registry.is_empty());
    }

    /// Pass C7.4.6 acceptance — typed registry refuses
    /// typed duplicate `(light_id, kind)` pairs.
    #[test]
    fn registry_refuses_duplicate_light_and_kind() {
        let light_id = LuxLightId::new(5);
        let constants = make_live_constants(light_id);
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        register_cloud_shadow_aux_layer(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
            &mut registry,
        )
        .expect("first registration should succeed");
        let second = register_cloud_shadow_aux_layer(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            CloudShadowFrameDelayMode::SameFrame,
            &mut registry,
        );
        assert_eq!(
            second,
            Err(LuxShadowAuxLayerRegistryError::DuplicateLightAndKind),
        );
        // Typed registry retains the typed first
        // registration.
        assert_eq!(registry.len(), 1);
        let layer = registry.find(light_id).expect("first layer survives");
        assert_eq!(layer.latency, CloudShadowFrameDelayMode::OneFrameDelayed);
    }

    /// Pass C7.4.6 acceptance — typed registry refuses
    /// typed invalid light id.
    #[test]
    fn registry_refuses_invalid_light_id() {
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        let bad_layer = LuxShadowAuxLayer {
            light_id: LuxLightId::INVALID,
            kind: LuxShadowAuxLayerKind::CloudTransmittance,
            resource_type: FrameGraphResourceType::CloudWorldShadowFiltered,
            projection_constants: FrameGraphResourceType::CloudShadowProjectionConstants,
            opacity_q16: 0x8000,
            softness_q16: 0x2000,
            latency: CloudShadowFrameDelayMode::OneFrameDelayed,
        };
        let result = registry.register(bad_layer);
        assert_eq!(result, Err(LuxShadowAuxLayerRegistryError::InvalidLightId));
        assert!(registry.is_empty());
    }

    /// Pass C7.4.6 acceptance — typed registry holds up to
    /// `LUX_SHADOW_AUX_LAYER_MAX` typed layers; the typed
    /// (N+1)-th typed register fails with `RegistryFull`.
    #[test]
    fn registry_reports_full_when_at_capacity() {
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        for i in 0..LUX_SHADOW_AUX_LAYER_MAX {
            let layer = LuxShadowAuxLayer {
                light_id: LuxLightId::new((i + 1) as u64),
                kind: LuxShadowAuxLayerKind::CloudTransmittance,
                resource_type: FrameGraphResourceType::CloudWorldShadowFiltered,
                projection_constants: FrameGraphResourceType::CloudShadowProjectionConstants,
                opacity_q16: 0x8000,
                softness_q16: 0x2000,
                latency: CloudShadowFrameDelayMode::OneFrameDelayed,
            };
            registry
                .register(layer)
                .expect("typed within-capacity register");
        }
        assert_eq!(registry.len(), LUX_SHADOW_AUX_LAYER_MAX);
        let overflow = LuxShadowAuxLayer {
            light_id: LuxLightId::new((LUX_SHADOW_AUX_LAYER_MAX + 1) as u64),
            kind: LuxShadowAuxLayerKind::CloudTransmittance,
            resource_type: FrameGraphResourceType::CloudWorldShadowFiltered,
            projection_constants: FrameGraphResourceType::CloudShadowProjectionConstants,
            opacity_q16: 0,
            softness_q16: 0,
            latency: CloudShadowFrameDelayMode::OneFrameDelayed,
        };
        assert_eq!(
            registry.register(overflow),
            Err(LuxShadowAuxLayerRegistryError::RegistryFull),
        );
    }

    /// Pass C7.4.6 acceptance — typed registry iter walks
    /// every typed registered layer.
    #[test]
    fn registry_iter_walks_every_layer() {
        let mut registry = LuxShadowAuxLayerRegistry::EMPTY;
        for i in 0..2 {
            let layer = LuxShadowAuxLayer {
                light_id: LuxLightId::new((i + 1) as u64),
                kind: LuxShadowAuxLayerKind::CloudTransmittance,
                resource_type: FrameGraphResourceType::CloudWorldShadowFiltered,
                projection_constants: FrameGraphResourceType::CloudShadowProjectionConstants,
                opacity_q16: 0x8000,
                softness_q16: 0x2000,
                latency: CloudShadowFrameDelayMode::SameFrame,
            };
            registry.register(layer).unwrap();
        }
        let ids: Vec<u64> = registry.iter().map(|l| l.light_id.0).collect();
        assert_eq!(ids, vec![1, 2]);
        // Typed `clear` resets the typed registry.
        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    /// Pass C7.4.6 acceptance — typed builder rejects a
    /// typed projection that names a typed invalid light
    /// id, even when the typed settings register the typed
    /// pass.
    #[test]
    fn builder_rejects_invalid_light_id() {
        // Hand-construct typed constants with the typed
        // valid path but a typed invalid light id slipped
        // in.  Typed `from_inputs` would already return
        // DISABLED for that case; typed test asserts the
        // typed `cloud_transmittance_from_inputs` builder
        // also short-circuits.
        let mut constants = make_live_constants(LuxLightId::new(1));
        constants.light_id = LuxLightId::INVALID;
        let layer = LuxShadowAuxLayer::cloud_transmittance_from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            &constants,
            CloudShadowFrameDelayMode::OneFrameDelayed,
        );
        assert_eq!(layer, None);
    }
}
