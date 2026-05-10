//! Pass 4 — typed light-component schema.
//!
//! Encodes the user's "4.2 Light component schema" rule: every
//! light carries a typed kind + position + direction + range +
//! color + intensity + color temperature + cone angles + shadow
//! enabled + shadow quality + volumetric contribution +
//! bloom/emissive hint + lighting mode + layer mask + scene
//! mask + priority + dirty flags.
//!
//! `FunLuxLightComponent` is a `bevy_ecs::Component`; the
//! lighting extraction systems read it and project into the
//! Pass 2 dense `LuxLightRecord` table the renderer consumes.
//!
//! Pass 4's typed light dirty flags reuse Pass 2's
//! [`crate::dirty::LuxDirtyFlags`] 1:1 — the spec's
//! `TransformDirty` / `ColorDirty` / `IntensityDirty` /
//! `RangeDirty` / `ShadowDirty` / `StaticBakeDirty` /
//! `VolumetricDirty` / `Removed` / `Created` flags map exactly
//! to the existing bits.

use bevy_ecs::prelude::Component;

use crate::dirty::LuxDirtyFlags;

pub const FUN_LUX_LIGHT_COMPONENT_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_LIGHT_KIND_COUNT: usize = 10;
pub const FUN_LUX_LIGHTING_MODE_COUNT: usize = 3;
pub const FUN_LUX_SHADOW_QUALITY_COUNT: usize = 5;

// ============================================================================
// Section 1 — Typed light kind (4.1 stored kinds)
// ============================================================================

/// Typed light kind. Names every typed scene-lighting record
/// the Pass 4 "4.1 Scene lighting data" rule lists.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxLightKind {
    #[default]
    Directional,
    Point,
    Spot,
    Area,
    Emissive,
    ReflectionProbe,
    IrradianceProbe,
    FogVolume,
    LocalVolumetric,
    ShadowCaster,
}

impl FunLuxLightKind {
    pub const ALL: [Self; FUN_LUX_LIGHT_KIND_COUNT] = [
        Self::Directional,
        Self::Point,
        Self::Spot,
        Self::Area,
        Self::Emissive,
        Self::ReflectionProbe,
        Self::IrradianceProbe,
        Self::FogVolume,
        Self::LocalVolumetric,
        Self::ShadowCaster,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Point => "point",
            Self::Spot => "spot",
            Self::Area => "area",
            Self::Emissive => "emissive",
            Self::ReflectionProbe => "reflection_probe",
            Self::IrradianceProbe => "irradiance_probe",
            Self::FogVolume => "fog_volume",
            Self::LocalVolumetric => "local_volumetric",
            Self::ShadowCaster => "shadow_caster",
        }
    }

    /// Typed predicate: does this kind ever cast a shadow?
    /// Used by the planner to skip shadow request emission
    /// for kinds that never cast.
    #[must_use]
    pub const fn can_cast_shadow(self) -> bool {
        matches!(
            self,
            Self::Directional | Self::Point | Self::Spot | Self::Area | Self::ShadowCaster
        )
    }

    /// Typed predicate: does this kind ever contribute to
    /// the volumetric scattering pass?
    #[must_use]
    pub const fn contributes_to_volumetric(self) -> bool {
        matches!(
            self,
            Self::Directional
                | Self::Point
                | Self::Spot
                | Self::Area
                | Self::FogVolume
                | Self::LocalVolumetric
        )
    }

    /// Typed predicate: is this kind a probe (irradiance /
    /// reflection)? Probes drive GI / reflection caches.
    #[must_use]
    pub const fn is_probe(self) -> bool {
        matches!(self, Self::ReflectionProbe | Self::IrradianceProbe)
    }

    /// Typed predicate: is this kind a volumetric record
    /// (fog volume / local volumetric)?
    #[must_use]
    pub const fn is_volumetric_record(self) -> bool {
        matches!(self, Self::FogVolume | Self::LocalVolumetric)
    }

    /// Typed predicate: does this kind use the `direction`
    /// field? Directional + spot do; point / area / probes /
    /// volumes don't.
    #[must_use]
    pub const fn uses_direction(self) -> bool {
        matches!(self, Self::Directional | Self::Spot)
    }

    /// Typed predicate: does this kind use the inner/outer
    /// cone angles?
    #[must_use]
    pub const fn uses_cone_angles(self) -> bool {
        matches!(self, Self::Spot)
    }
}

// ============================================================================
// Section 2 — Typed lighting mode (4.2 static/dynamic/mixed)
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxLightingMode {
    /// Static lighting: baked into the static lighting
    /// cache; the renderer does not re-evaluate this light
    /// at runtime.
    Static,
    /// Dynamic lighting: re-evaluated every frame.
    #[default]
    Dynamic,
    /// Mixed: indirect bounce baked into static cache,
    /// direct shading evaluated at runtime.
    Mixed,
}

impl FunLuxLightingMode {
    pub const ALL: [Self; FUN_LUX_LIGHTING_MODE_COUNT] = [Self::Static, Self::Dynamic, Self::Mixed];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Dynamic => "dynamic",
            Self::Mixed => "mixed",
        }
    }

    /// Typed predicate: does this mode bake into the static
    /// lighting cache?
    #[must_use]
    pub const fn bakes_into_static_cache(self) -> bool {
        matches!(self, Self::Static | Self::Mixed)
    }

    /// Typed predicate: does this mode evaluate at runtime?
    #[must_use]
    pub const fn evaluates_at_runtime(self) -> bool {
        matches!(self, Self::Dynamic | Self::Mixed)
    }
}

// ============================================================================
// Section 3 — Typed shadow quality
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxShadowQuality {
    Off,
    Low,
    #[default]
    Medium,
    High,
    Ultra,
}

impl FunLuxShadowQuality {
    pub const ALL: [Self; FUN_LUX_SHADOW_QUALITY_COUNT] =
        [Self::Off, Self::Low, Self::Medium, Self::High, Self::Ultra];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    #[must_use]
    pub const fn order_key(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Low => 1,
            Self::Medium => 2,
            Self::High => 3,
            Self::Ultra => 4,
        }
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Off)
    }
}

// ============================================================================
// Section 4 — Typed layer + scene masks
// ============================================================================

/// Typed 32-bit layer mask. Lights only illuminate
/// renderers whose layer bit is set in the typed mask.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxLayerMask(pub u32);

impl FunLuxLayerMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u32::MAX);

    #[must_use]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    /// Typed predicate: does this mask include the given
    /// layer bit?
    #[must_use]
    pub const fn includes_layer(self, layer_bit: u32) -> bool {
        (self.0 & layer_bit) != 0
    }
}

/// Typed 32-bit scene mask. Lights only contribute to
/// scenes whose scene-id bit is set in the typed mask. The
/// renderer consults the mask when it walks the multi-scene
/// registry to know which scenes a light affects.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxSceneMask(pub u32);

impl FunLuxSceneMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u32::MAX);
    pub const MAIN_WORLD_ONLY: Self = Self(1 << 0);

    #[must_use]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn includes_scene_bit(self, scene_bit: u32) -> bool {
        (self.0 & scene_bit) != 0
    }
}

// ============================================================================
// Section 5 — Typed light component (4.2 full schema)
// ============================================================================

/// Typed light component. `bevy_ecs::Component` the
/// extraction system reads each frame; mirrors the user's
/// "4.2 Light component schema" field set.
#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct FunLuxLightComponent {
    pub schema_version: u16,
    pub stable_light_key: u64,
    pub kind: FunLuxLightKind,
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub range: f32,
    pub color_rgb: [f32; 3],
    pub intensity_lux: f32,
    pub color_temperature_kelvin: u32,
    pub inner_cone_angle_radians: f32,
    pub outer_cone_angle_radians: f32,
    pub shadow_enabled: bool,
    pub shadow_quality: FunLuxShadowQuality,
    pub volumetric_contribution_q8: u16,
    pub bloom_emissive_hint: bool,
    pub lighting_mode: FunLuxLightingMode,
    pub layer_mask: FunLuxLayerMask,
    pub scene_mask: FunLuxSceneMask,
    pub priority: u16,
    pub dirty_flags: LuxDirtyFlags,
}

impl FunLuxLightComponent {
    /// Typed directional-light constructor.
    #[must_use]
    pub const fn directional(
        stable_light_key: u64,
        direction: [f32; 3],
        color_rgb: [f32; 3],
        intensity_lux: f32,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_LIGHT_COMPONENT_SCHEMA_VERSION,
            stable_light_key,
            kind: FunLuxLightKind::Directional,
            position: [0.0, 0.0, 0.0],
            direction,
            range: f32::INFINITY,
            color_rgb,
            intensity_lux,
            color_temperature_kelvin: 6504, // D65
            inner_cone_angle_radians: 0.0,
            outer_cone_angle_radians: 0.0,
            shadow_enabled: true,
            shadow_quality: FunLuxShadowQuality::Medium,
            volumetric_contribution_q8: 256,
            bloom_emissive_hint: false,
            lighting_mode: FunLuxLightingMode::Dynamic,
            layer_mask: FunLuxLayerMask::ALL,
            scene_mask: FunLuxSceneMask::MAIN_WORLD_ONLY,
            priority: 128,
            dirty_flags: LuxDirtyFlags::CREATED,
        }
    }

    /// Typed point-light constructor.
    #[must_use]
    pub const fn point(
        stable_light_key: u64,
        position: [f32; 3],
        range: f32,
        color_rgb: [f32; 3],
        intensity_lux: f32,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_LIGHT_COMPONENT_SCHEMA_VERSION,
            stable_light_key,
            kind: FunLuxLightKind::Point,
            position,
            direction: [0.0, 0.0, 0.0],
            range,
            color_rgb,
            intensity_lux,
            color_temperature_kelvin: 6504,
            inner_cone_angle_radians: 0.0,
            outer_cone_angle_radians: 0.0,
            shadow_enabled: true,
            shadow_quality: FunLuxShadowQuality::Medium,
            volumetric_contribution_q8: 256,
            bloom_emissive_hint: false,
            lighting_mode: FunLuxLightingMode::Dynamic,
            layer_mask: FunLuxLayerMask::ALL,
            scene_mask: FunLuxSceneMask::MAIN_WORLD_ONLY,
            priority: 128,
            dirty_flags: LuxDirtyFlags::CREATED,
        }
    }

    /// Typed spot-light constructor.
    #[must_use]
    pub const fn spot(
        stable_light_key: u64,
        position: [f32; 3],
        direction: [f32; 3],
        range: f32,
        inner_cone_angle_radians: f32,
        outer_cone_angle_radians: f32,
        color_rgb: [f32; 3],
        intensity_lux: f32,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_LIGHT_COMPONENT_SCHEMA_VERSION,
            stable_light_key,
            kind: FunLuxLightKind::Spot,
            position,
            direction,
            range,
            color_rgb,
            intensity_lux,
            color_temperature_kelvin: 6504,
            inner_cone_angle_radians,
            outer_cone_angle_radians,
            shadow_enabled: true,
            shadow_quality: FunLuxShadowQuality::Medium,
            volumetric_contribution_q8: 256,
            bloom_emissive_hint: false,
            lighting_mode: FunLuxLightingMode::Dynamic,
            layer_mask: FunLuxLayerMask::ALL,
            scene_mask: FunLuxSceneMask::MAIN_WORLD_ONLY,
            priority: 128,
            dirty_flags: LuxDirtyFlags::CREATED,
        }
    }

    /// Typed predicate: does this light cast a shadow this
    /// frame? Combines `shadow_enabled` + `shadow_quality`
    /// + the typed kind's `can_cast_shadow` capability.
    #[must_use]
    pub const fn casts_shadow_this_frame(&self) -> bool {
        self.shadow_enabled && self.shadow_quality.is_active() && self.kind.can_cast_shadow()
    }

    /// Typed predicate: does this light contribute to the
    /// volumetric pass this frame?
    #[must_use]
    pub const fn contributes_to_volumetric_this_frame(&self) -> bool {
        self.kind.contributes_to_volumetric() && self.volumetric_contribution_q8 > 0
    }

    /// Typed predicate: is this light's dirty state
    /// quiescent (no flags set)?
    #[must_use]
    pub const fn is_quiescent(&self) -> bool {
        self.dirty_flags.is_empty()
    }

    /// Typed transition: mark this light's transform dirty
    /// after a position / direction move. Pass 2 acceptance:
    /// shadow maps must invalidate.
    pub fn mark_transform_dirty(&mut self, new_position: [f32; 3], new_direction: [f32; 3]) {
        self.position = new_position;
        self.direction = new_direction;
        self.dirty_flags.insert(LuxDirtyFlags::TRANSFORM);
    }

    /// Typed transition: mark color dirty. Pass 2
    /// acceptance: shadow maps must NOT invalidate.
    pub fn mark_color_dirty(&mut self, new_color_rgb: [f32; 3]) {
        self.color_rgb = new_color_rgb;
        self.dirty_flags.insert(LuxDirtyFlags::COLOR);
    }

    /// Typed acknowledge: clear all dirty flags.
    pub fn clear_dirty_flags(&mut self) {
        self.dirty_flags = LuxDirtyFlags::NONE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_LIGHT_COMPONENT_SCHEMA_VERSION, 1);
        assert_eq!(FUN_LUX_LIGHT_KIND_COUNT, 10);
        assert_eq!(FunLuxLightKind::ALL.len(), FUN_LUX_LIGHT_KIND_COUNT);
        assert_eq!(FUN_LUX_LIGHTING_MODE_COUNT, 3);
        assert_eq!(FunLuxLightingMode::ALL.len(), FUN_LUX_LIGHTING_MODE_COUNT);
        assert_eq!(FUN_LUX_SHADOW_QUALITY_COUNT, 5);
        assert_eq!(FunLuxShadowQuality::ALL.len(), FUN_LUX_SHADOW_QUALITY_COUNT);
    }

    #[test]
    fn light_kind_taxonomy_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in FunLuxLightKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn light_kind_capability_predicates() {
        assert!(FunLuxLightKind::Directional.can_cast_shadow());
        assert!(FunLuxLightKind::Point.can_cast_shadow());
        assert!(FunLuxLightKind::Spot.can_cast_shadow());
        assert!(FunLuxLightKind::Area.can_cast_shadow());
        assert!(!FunLuxLightKind::Emissive.can_cast_shadow());
        assert!(!FunLuxLightKind::ReflectionProbe.can_cast_shadow());
        assert!(!FunLuxLightKind::IrradianceProbe.can_cast_shadow());
        assert!(!FunLuxLightKind::FogVolume.can_cast_shadow());
        assert!(!FunLuxLightKind::LocalVolumetric.can_cast_shadow());
        assert!(FunLuxLightKind::ShadowCaster.can_cast_shadow());

        assert!(FunLuxLightKind::Directional.contributes_to_volumetric());
        assert!(FunLuxLightKind::FogVolume.contributes_to_volumetric());
        assert!(!FunLuxLightKind::Emissive.contributes_to_volumetric());

        assert!(FunLuxLightKind::ReflectionProbe.is_probe());
        assert!(FunLuxLightKind::IrradianceProbe.is_probe());
        assert!(!FunLuxLightKind::Directional.is_probe());

        assert!(FunLuxLightKind::FogVolume.is_volumetric_record());
        assert!(FunLuxLightKind::LocalVolumetric.is_volumetric_record());
        assert!(!FunLuxLightKind::Directional.is_volumetric_record());

        assert!(FunLuxLightKind::Directional.uses_direction());
        assert!(FunLuxLightKind::Spot.uses_direction());
        assert!(!FunLuxLightKind::Point.uses_direction());

        assert!(FunLuxLightKind::Spot.uses_cone_angles());
        assert!(!FunLuxLightKind::Directional.uses_cone_angles());
    }

    #[test]
    fn lighting_mode_predicates() {
        assert!(FunLuxLightingMode::Static.bakes_into_static_cache());
        assert!(FunLuxLightingMode::Mixed.bakes_into_static_cache());
        assert!(!FunLuxLightingMode::Dynamic.bakes_into_static_cache());

        assert!(FunLuxLightingMode::Dynamic.evaluates_at_runtime());
        assert!(FunLuxLightingMode::Mixed.evaluates_at_runtime());
        assert!(!FunLuxLightingMode::Static.evaluates_at_runtime());
    }

    #[test]
    fn shadow_quality_ordering_and_active_predicate() {
        let qualities = [
            FunLuxShadowQuality::Off,
            FunLuxShadowQuality::Low,
            FunLuxShadowQuality::Medium,
            FunLuxShadowQuality::High,
            FunLuxShadowQuality::Ultra,
        ];
        for window in qualities.windows(2) {
            assert!(window[0].order_key() < window[1].order_key());
        }
        assert!(!FunLuxShadowQuality::Off.is_active());
        assert!(FunLuxShadowQuality::Low.is_active());
        assert!(FunLuxShadowQuality::Ultra.is_active());
    }

    #[test]
    fn directional_constructor_sets_canonical_defaults() {
        let light =
            FunLuxLightComponent::directional(42, [0.0, -1.0, 0.0], [1.0, 1.0, 1.0], 100_000.0);
        assert_eq!(light.kind, FunLuxLightKind::Directional);
        assert!(light.casts_shadow_this_frame());
        assert!(light.contributes_to_volumetric_this_frame());
        assert!(!light.is_quiescent());
        assert!(light.dirty_flags.contains(LuxDirtyFlags::CREATED));
    }

    #[test]
    fn point_constructor_records_position_and_range() {
        let light =
            FunLuxLightComponent::point(7, [10.0, 20.0, 30.0], 50.0, [1.0, 0.5, 0.5], 5_000.0);
        assert_eq!(light.kind, FunLuxLightKind::Point);
        assert_eq!(light.position, [10.0, 20.0, 30.0]);
        assert_eq!(light.range, 50.0);
        assert!(light.casts_shadow_this_frame());
    }

    #[test]
    fn spot_constructor_records_cone_angles() {
        let light = FunLuxLightComponent::spot(
            13,
            [0.0, 5.0, 0.0],
            [0.0, -1.0, 0.0],
            20.0,
            0.5,
            0.8,
            [1.0, 1.0, 1.0],
            2_000.0,
        );
        assert_eq!(light.kind, FunLuxLightKind::Spot);
        assert_eq!(light.inner_cone_angle_radians, 0.5);
        assert_eq!(light.outer_cone_angle_radians, 0.8);
    }

    #[test]
    fn casts_shadow_predicate_combines_kind_and_settings() {
        let mut light = FunLuxLightComponent::directional(1, [0.0, -1.0, 0.0], [1.0; 3], 1000.0);
        assert!(light.casts_shadow_this_frame());
        light.shadow_enabled = false;
        assert!(!light.casts_shadow_this_frame());
        light.shadow_enabled = true;
        light.shadow_quality = FunLuxShadowQuality::Off;
        assert!(!light.casts_shadow_this_frame());
    }

    #[test]
    fn emissive_does_not_cast_shadow_even_with_quality_high() {
        let mut emissive = FunLuxLightComponent::directional(1, [0.0, -1.0, 0.0], [1.0; 3], 1000.0);
        emissive.kind = FunLuxLightKind::Emissive;
        emissive.shadow_quality = FunLuxShadowQuality::Ultra;
        emissive.shadow_enabled = true;
        // Emissive cannot cast a shadow (capability).
        assert!(!emissive.casts_shadow_this_frame());
    }

    #[test]
    fn mark_transform_dirty_sets_transform_flag_only() {
        let mut light = FunLuxLightComponent::point(1, [0.0, 0.0, 0.0], 10.0, [1.0; 3], 500.0);
        light.clear_dirty_flags();
        light.mark_transform_dirty([5.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        assert!(light.dirty_flags.contains(LuxDirtyFlags::TRANSFORM));
        assert!(!light.dirty_flags.contains(LuxDirtyFlags::COLOR));
    }

    /// Pass 4 acceptance: color change does NOT mark
    /// shadow-invalidating flags. Mirrors Pass 2 acceptance.
    #[test]
    fn mark_color_dirty_does_not_invalidate_shadow_maps() {
        let mut light = FunLuxLightComponent::point(1, [0.0, 0.0, 0.0], 10.0, [1.0; 3], 500.0);
        light.clear_dirty_flags();
        light.mark_color_dirty([0.0, 1.0, 0.0]);
        assert!(!light.dirty_flags.invalidates_shadow_maps());
    }

    #[test]
    fn layer_mask_predicates() {
        let mask = FunLuxLayerMask::new(0b1010);
        assert!(mask.includes_layer(0b1000));
        assert!(mask.includes_layer(0b0010));
        assert!(!mask.includes_layer(0b0001));
        assert!(FunLuxLayerMask::ALL.includes_layer(1));
        assert!(!FunLuxLayerMask::NONE.includes_layer(1));
    }

    #[test]
    fn scene_mask_predicates() {
        assert!(FunLuxSceneMask::MAIN_WORLD_ONLY.includes_scene_bit(1));
        assert!(!FunLuxSceneMask::MAIN_WORLD_ONLY.includes_scene_bit(2));
        assert!(FunLuxSceneMask::ALL.includes_scene_bit(0xFFFFFFFF));
        assert!(!FunLuxSceneMask::NONE.includes_scene_bit(1));
    }
}
