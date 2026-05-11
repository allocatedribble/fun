//! Typed lighting quality settings.
//!
//! `LuxQualityTier` is the dominant knob that drives every
//! per-feature decision: cluster bin counts, reservoir pool
//! size, virtual shadow page headroom, GI ray counts, denoiser
//! kernel strength, volumetric froxel resolution. The typed
//! `LuxQualitySettings` lets the renderer override a single
//! feature without disturbing the rest of the lighting policy.

pub const FUN_LUX_QUALITY_SCHEMA_VERSION: u16 = 1;
pub const LUX_QUALITY_TIER_COUNT: usize = 4;

/// Typed lighting quality tier.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxQualityTier {
    #[default]
    Low,
    Medium,
    High,
    Ultra,
}

impl LuxQualityTier {
    pub const ALL: [Self; LUX_QUALITY_TIER_COUNT] =
        [Self::Low, Self::Medium, Self::High, Self::Ultra];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    /// Typed ordering key. `Low = 0`, `Ultra = 3`.
    #[must_use]
    pub const fn order_key(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Ultra => 3,
        }
    }

    /// Typed predicate: `Medium+` permits per-pixel reservoir
    /// reuse for many-light direct shade.
    #[must_use]
    pub const fn permits_reservoir_reuse(self) -> bool {
        self.order_key() >= 1
    }

    /// Typed predicate: `High+` permits virtual shadow
    /// demand-page residency tracking.
    #[must_use]
    pub const fn permits_virtual_shadow_demand_pages(self) -> bool {
        self.order_key() >= 2
    }

    /// Typed predicate: `High+` permits volumetric fog +
    /// light scattering at full froxel resolution.
    #[must_use]
    pub const fn permits_volumetric_full_resolution(self) -> bool {
        self.order_key() >= 2
    }

    /// Typed predicate: `Ultra` permits GI second-bounce ray
    /// tracing + denoiser temporal upsampling.
    #[must_use]
    pub const fn permits_gi_second_bounce(self) -> bool {
        self.order_key() >= 3
    }
}

/// Typed per-feature quality override. Each feature can take
/// its own tier; the global `default_tier` is used when a
/// feature has no override. Renderers reading a
/// `LuxQualitySettings` consult `tier_for(...)` for a typed
/// resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxQualitySettings {
    pub schema_version: u16,
    pub default_tier: LuxQualityTier,
    pub direct_lighting: Option<LuxQualityTier>,
    pub many_light_reservoir: Option<LuxQualityTier>,
    pub shadows: Option<LuxQualityTier>,
    pub virtual_shadows: Option<LuxQualityTier>,
    pub gi: Option<LuxQualityTier>,
    pub reflections: Option<LuxQualityTier>,
    pub denoise: Option<LuxQualityTier>,
    pub volumetric: Option<LuxQualityTier>,
}

impl LuxQualitySettings {
    /// Typed product-default: every feature inherits the global
    /// tier (no per-feature override).
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_QUALITY_SCHEMA_VERSION,
        default_tier: LuxQualityTier::Medium,
        direct_lighting: None,
        many_light_reservoir: None,
        shadows: None,
        virtual_shadows: None,
        gi: None,
        reflections: None,
        denoise: None,
        volumetric: None,
    };

    /// Typed uniform tier: every feature pinned to the given
    /// tier (no per-feature variance).
    #[must_use]
    pub const fn uniform(tier: LuxQualityTier) -> Self {
        Self {
            schema_version: FUN_LUX_QUALITY_SCHEMA_VERSION,
            default_tier: tier,
            direct_lighting: Some(tier),
            many_light_reservoir: Some(tier),
            shadows: Some(tier),
            virtual_shadows: Some(tier),
            gi: Some(tier),
            reflections: Some(tier),
            denoise: Some(tier),
            volumetric: Some(tier),
        }
    }

    /// Typed feature accessor: returns the per-feature tier if
    /// set, else the global default.
    #[must_use]
    pub const fn tier_for(self, feature: LuxQualityFeature) -> LuxQualityTier {
        let override_value = match feature {
            LuxQualityFeature::DirectLighting => self.direct_lighting,
            LuxQualityFeature::ManyLightReservoir => self.many_light_reservoir,
            LuxQualityFeature::Shadows => self.shadows,
            LuxQualityFeature::VirtualShadows => self.virtual_shadows,
            LuxQualityFeature::Gi => self.gi,
            LuxQualityFeature::Reflections => self.reflections,
            LuxQualityFeature::Denoise => self.denoise,
            LuxQualityFeature::Volumetric => self.volumetric,
        };
        match override_value {
            Some(tier) => tier,
            None => self.default_tier,
        }
    }
}

/// Typed lighting feature handle used by per-feature
/// quality-tier lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxQualityFeature {
    DirectLighting,
    ManyLightReservoir,
    Shadows,
    VirtualShadows,
    Gi,
    Reflections,
    Denoise,
    Volumetric,
}

impl LuxQualityFeature {
    pub const ALL: [Self; 8] = [
        Self::DirectLighting,
        Self::ManyLightReservoir,
        Self::Shadows,
        Self::VirtualShadows,
        Self::Gi,
        Self::Reflections,
        Self::Denoise,
        Self::Volumetric,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLighting => "direct_lighting",
            Self::ManyLightReservoir => "many_light_reservoir",
            Self::Shadows => "shadows",
            Self::VirtualShadows => "virtual_shadows",
            Self::Gi => "gi",
            Self::Reflections => "reflections",
            Self::Denoise => "denoise",
            Self::Volumetric => "volumetric",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_QUALITY_SCHEMA_VERSION, 1);
        assert_eq!(LUX_QUALITY_TIER_COUNT, 4);
        assert_eq!(LuxQualityTier::ALL.len(), LUX_QUALITY_TIER_COUNT);
    }

    #[test]
    fn quality_tier_predicates_climb_with_order_key() {
        assert!(!LuxQualityTier::Low.permits_reservoir_reuse());
        assert!(LuxQualityTier::Medium.permits_reservoir_reuse());
        assert!(LuxQualityTier::High.permits_virtual_shadow_demand_pages());
        assert!(LuxQualityTier::Ultra.permits_gi_second_bounce());
        assert!(!LuxQualityTier::Low.permits_gi_second_bounce());
    }

    #[test]
    fn settings_product_default_inherits_global_tier_for_every_feature() {
        let settings = LuxQualitySettings::PRODUCT_DEFAULT;
        for feature in LuxQualityFeature::ALL {
            assert_eq!(
                settings.tier_for(feature),
                settings.default_tier,
                "{} should inherit default tier",
                feature.as_str(),
            );
        }
    }

    #[test]
    fn settings_uniform_pins_every_feature_to_one_tier() {
        let settings = LuxQualitySettings::uniform(LuxQualityTier::Ultra);
        for feature in LuxQualityFeature::ALL {
            assert_eq!(settings.tier_for(feature), LuxQualityTier::Ultra);
        }
    }

    #[test]
    fn settings_per_feature_override_takes_precedence() {
        let mut settings = LuxQualitySettings::PRODUCT_DEFAULT;
        settings.gi = Some(LuxQualityTier::Ultra);
        settings.denoise = Some(LuxQualityTier::Low);
        assert_eq!(
            settings.tier_for(LuxQualityFeature::Gi),
            LuxQualityTier::Ultra,
        );
        assert_eq!(
            settings.tier_for(LuxQualityFeature::Denoise),
            LuxQualityTier::Low,
        );
        // Unchanged features still inherit the default.
        assert_eq!(
            settings.tier_for(LuxQualityFeature::Shadows),
            LuxQualityTier::Medium,
        );
    }

    #[test]
    fn quality_feature_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for feature in LuxQualityFeature::ALL {
            assert!(
                seen.insert(feature.as_str()),
                "duplicate: {}",
                feature.as_str(),
            );
        }
    }
}
