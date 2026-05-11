//! Typed volumetric fog + light scattering settings.
//!
//! The lighting policy for volumetric effects: froxel grid
//! resolution, density model, scattering anisotropy, temporal
//! reproject alpha, integration mode. `fun-renderer` reads
//! these typed records and allocates the actual froxel
//! textures + dispatches the volumetric compute passes.
//!
//! Pass V2.6 — compatibility shim
//! ------------------------------
//! The typed canonical home for product-facing fog +
//! volumetric policy moves to [`crate::fog_volumetric`]
//! (`LuxVolumetricSettings`, `LuxFroxelGridSettings`,
//! `LuxVolumetricPassRole`, `LuxVolumetricResourceIntent`).
//! This module stays as a typed compatibility shim because
//! existing runtime code (`crate::runtime::LuxFramePlanner`)
//! and the typed frame-plan (`crate::frame_plan::LuxFramePlan`)
//! still consume the legacy `FunLuxVolumetricSettings` shape.
//!
//! Callers SHOULD prefer the typed canonical names — they
//! are re-exported at the crate root via `pub use
//! fog_volumetric::*` in `lib.rs`.  The legacy `FunLux*`
//! types remain so the existing typed planner + frame-plan
//! tests + the renderer's compatibility callers continue to
//! work without a single sweeping refactor.

// Pass V2.6 migration aliases — typed canonical re-exports
// from `fog_volumetric` for callers that import directly
// from this typed module.  The typed canonical surface
// always wins; the typed legacy `FunLuxVolumetricSettings`
// stays defined below for the typed runtime + frame-plan
// callers that depend on its concrete shape today.
pub use crate::fog_volumetric::{
    LuxFroxelGridSettings, LuxVolumetricPassRole, LuxVolumetricResourceIntent,
    LuxVolumetricSettings,
};

pub const FUN_LUX_VOLUMETRIC_SCHEMA_VERSION: u16 = 1;

/// Typed froxel grid extent. The renderer maps the extent to
/// real 3D textures via the typed
/// [`crate::frame_plan::LuxResourceIntent::VolumetricFroxelDensity`]
/// + `VolumetricFroxelScattering` variants.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxFroxelGridExtent {
    pub schema_version: u16,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
}

impl FunLuxFroxelGridExtent {
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_VOLUMETRIC_SCHEMA_VERSION,
        width: 1,
        height: 1,
        depth: 1,
    };

    pub const LOW_QUALITY: Self = Self {
        schema_version: FUN_LUX_VOLUMETRIC_SCHEMA_VERSION,
        width: 64,
        height: 36,
        depth: 64,
    };

    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_VOLUMETRIC_SCHEMA_VERSION,
        width: 160,
        height: 90,
        depth: 64,
    };

    pub const HIGH_QUALITY: Self = Self {
        schema_version: FUN_LUX_VOLUMETRIC_SCHEMA_VERSION,
        width: 240,
        height: 135,
        depth: 96,
    };

    #[must_use]
    pub const fn cell_count(self) -> u64 {
        (self.width as u64) * (self.height as u64) * (self.depth as u64)
    }

    /// Typed predicate: is this extent the cold-default
    /// (1×1×1, equivalent to volumetric disabled)?
    #[must_use]
    pub const fn is_cold_default(self) -> bool {
        self.width <= 1 && self.height <= 1 && self.depth <= 1
    }
}

/// Typed volumetric density model. The renderer maps the model
/// to its concrete fog-density compute shader; the typed value
/// names the algorithm.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxVolumetricDensityModel {
    /// Constant density per cell — debug / cold default.
    #[default]
    Constant,
    /// Exponential height fog (single layer).
    ExponentialHeight,
    /// Layered height fog (multiple stacked layers).
    LayeredHeight,
    /// Procedural noise-driven density (Perlin / curl).
    Procedural,
    /// Authored volume (artist-painted SDF).
    AuthoredVolume,
}

impl FunLuxVolumetricDensityModel {
    pub const ALL: [Self; 5] = [
        Self::Constant,
        Self::ExponentialHeight,
        Self::LayeredHeight,
        Self::Procedural,
        Self::AuthoredVolume,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Constant => "constant",
            Self::ExponentialHeight => "exponential_height",
            Self::LayeredHeight => "layered_height",
            Self::Procedural => "procedural",
            Self::AuthoredVolume => "authored_volume",
        }
    }
}

/// Typed scattering anisotropy mode. Drives the Henyey-Greenstein
/// `g` parameter for the renderer's scattering shader.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxScatteringAnisotropy {
    /// Isotropic — `g = 0`.
    #[default]
    Isotropic,
    /// Strong forward scatter — `g ≈ 0.8` (sunlit fog).
    StrongForward,
    /// Weak forward scatter — `g ≈ 0.4` (general atmospheric).
    WeakForward,
    /// Backward scatter — `g ≈ -0.4` (rare; volumetric haze).
    Backward,
}

impl FunLuxScatteringAnisotropy {
    pub const ALL: [Self; 4] = [
        Self::Isotropic,
        Self::StrongForward,
        Self::WeakForward,
        Self::Backward,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Isotropic => "isotropic",
            Self::StrongForward => "strong_forward",
            Self::WeakForward => "weak_forward",
            Self::Backward => "backward",
        }
    }

    /// Typed Henyey-Greenstein `g` in Q8 fixed-point form
    /// (so the typed value stays Hash-stable). Divide by 256
    /// for the float value.
    #[must_use]
    pub const fn henyey_greenstein_g_q8(self) -> i16 {
        match self {
            Self::Isotropic => 0,
            Self::StrongForward => 204, // 0.8 × 256 ≈ 204
            Self::WeakForward => 102,   // 0.4 × 256 ≈ 102
            Self::Backward => -102,
        }
    }
}

/// Typed volumetric integration mode. The renderer maps the
/// mode to its compute integration pipeline.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxVolumetricIntegrationMode {
    /// Disabled — no integration pass.
    #[default]
    Disabled,
    /// Single-bounce ray-march along view ray.
    SingleBounceRayMarch,
    /// Multi-tap importance-sampled integration.
    MultiTapImportanceSampled,
}

impl FunLuxVolumetricIntegrationMode {
    pub const ALL: [Self; 3] = [
        Self::Disabled,
        Self::SingleBounceRayMarch,
        Self::MultiTapImportanceSampled,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SingleBounceRayMarch => "single_bounce_ray_march",
            Self::MultiTapImportanceSampled => "multi_tap_importance_sampled",
        }
    }

    /// Typed predicate: is the integration pass active?
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

/// Typed volumetric policy. Carried in
/// `LuxFramePlan::look_profile` indirectly through the typed
/// `LuxFramePlanner`'s settings; the renderer reads the
/// settings to allocate froxel resources + dispatch passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxVolumetricSettings {
    pub schema_version: u16,
    pub froxel_grid: FunLuxFroxelGridExtent,
    pub density_model: FunLuxVolumetricDensityModel,
    pub scattering: FunLuxScatteringAnisotropy,
    pub integration: FunLuxVolumetricIntegrationMode,
    /// Temporal reproject blend alpha in Q8 fixed-point.
    /// `0` = no reproject; `255` = full reproject.
    pub temporal_reproject_alpha_q8: u8,
    /// Receives shadows from the typed
    /// `RenderVirtualShadowPages` pass when enabled.
    pub receives_virtual_shadow: bool,
}

impl FunLuxVolumetricSettings {
    /// Cold-default: volumetric disabled.
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_LUX_VOLUMETRIC_SCHEMA_VERSION,
        froxel_grid: FunLuxFroxelGridExtent::COLD_DEFAULT,
        density_model: FunLuxVolumetricDensityModel::Constant,
        scattering: FunLuxScatteringAnisotropy::Isotropic,
        integration: FunLuxVolumetricIntegrationMode::Disabled,
        temporal_reproject_alpha_q8: 0,
        receives_virtual_shadow: false,
    };

    /// Product-default: low-quality exponential height fog +
    /// weak forward scatter + temporal reproject enabled.
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_VOLUMETRIC_SCHEMA_VERSION,
        froxel_grid: FunLuxFroxelGridExtent::PRODUCT_DEFAULT,
        density_model: FunLuxVolumetricDensityModel::ExponentialHeight,
        scattering: FunLuxScatteringAnisotropy::WeakForward,
        integration: FunLuxVolumetricIntegrationMode::SingleBounceRayMarch,
        temporal_reproject_alpha_q8: 200,
        receives_virtual_shadow: true,
    };

    /// Typed predicate: is the volumetric pipeline active?
    #[must_use]
    pub const fn is_active(self) -> bool {
        self.integration.is_active() && !self.froxel_grid.is_cold_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_LUX_VOLUMETRIC_SCHEMA_VERSION, 1);
        assert_eq!(FunLuxVolumetricDensityModel::ALL.len(), 5);
        assert_eq!(FunLuxScatteringAnisotropy::ALL.len(), 4);
        assert_eq!(FunLuxVolumetricIntegrationMode::ALL.len(), 3);
    }

    #[test]
    fn cold_default_froxel_grid_is_minimal() {
        let g = FunLuxFroxelGridExtent::COLD_DEFAULT;
        assert!(g.is_cold_default());
        assert_eq!(g.cell_count(), 1);
    }

    #[test]
    fn product_default_froxel_grid_has_meaningful_cell_count() {
        let g = FunLuxFroxelGridExtent::PRODUCT_DEFAULT;
        assert!(!g.is_cold_default());
        assert_eq!(g.cell_count(), 160 * 90 * 64);
    }

    #[test]
    fn density_model_taxonomy_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for model in FunLuxVolumetricDensityModel::ALL {
            assert!(seen.insert(model.as_str()), "duplicate: {}", model.as_str());
        }
    }

    #[test]
    fn scattering_anisotropy_henyey_greenstein_g_signs_match_taxonomy() {
        assert_eq!(
            FunLuxScatteringAnisotropy::Isotropic.henyey_greenstein_g_q8(),
            0
        );
        assert!(FunLuxScatteringAnisotropy::StrongForward.henyey_greenstein_g_q8() > 0);
        assert!(FunLuxScatteringAnisotropy::WeakForward.henyey_greenstein_g_q8() > 0);
        assert!(FunLuxScatteringAnisotropy::Backward.henyey_greenstein_g_q8() < 0);
    }

    #[test]
    fn integration_mode_is_active_predicate() {
        assert!(!FunLuxVolumetricIntegrationMode::Disabled.is_active());
        assert!(FunLuxVolumetricIntegrationMode::SingleBounceRayMarch.is_active());
        assert!(FunLuxVolumetricIntegrationMode::MultiTapImportanceSampled.is_active());
    }

    #[test]
    fn cold_default_volumetric_settings_are_disabled() {
        let v = FunLuxVolumetricSettings::COLD_DEFAULT;
        assert!(!v.is_active());
    }

    #[test]
    fn product_default_volumetric_settings_are_active() {
        let v = FunLuxVolumetricSettings::PRODUCT_DEFAULT;
        assert!(v.is_active());
        assert!(v.receives_virtual_shadow);
    }
}
