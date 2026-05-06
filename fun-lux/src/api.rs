use crate::LuxDenoiseReconstructionPath;

pub const LUX_API_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxFeatureToggles {
    pub many_light: bool,
    pub virtual_shadows: bool,
    pub hybrid_gi: bool,
}

impl LuxFeatureToggles {
    pub const COMPILED: Self = Self {
        many_light: cfg!(feature = "fun_lux_many_light"),
        virtual_shadows: cfg!(feature = "fun_lux_virtual_shadows"),
        hybrid_gi: cfg!(feature = "fun_lux_hybrid_gi"),
    };

    #[must_use]
    pub const fn compiled() -> Self {
        Self::COMPILED
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectLightingMode {
    Disabled,
    SingleLight,
    TiledClustered,
    ReservoirManyLight,
}

impl DirectLightingMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SingleLight => "single_light",
            Self::TiledClustered => "tiled_clustered",
            Self::ReservoirManyLight => "reservoir_many_light",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowMode {
    Disabled,
    StaticMaps,
    VirtualDemandPaged,
}

impl ShadowMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::StaticMaps => "static_maps",
            Self::VirtualDemandPaged => "virtual_demand_paged",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiMode {
    Disabled,
    ScreenSpace,
    RadianceCache,
    Hybrid,
}

impl GiMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ScreenSpace => "screen_space",
            Self::RadianceCache => "radiance_cache",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReflectionMode {
    Disabled,
    ScreenSpace,
    SurfaceCache,
    Hybrid,
}

impl ReflectionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ScreenSpace => "screen_space",
            Self::SurfaceCache => "surface_cache",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconstructionHook {
    pub stable_id: &'static str,
    pub path: LuxDenoiseReconstructionPath,
    pub model_assisted: bool,
}

impl ReconstructionHook {
    #[must_use]
    pub const fn new(stable_id: &'static str, path: LuxDenoiseReconstructionPath) -> Self {
        Self {
            stable_id,
            path,
            model_assisted: path.requires_fun_ai_runtime(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxSettings {
    pub direct_lighting: DirectLightingMode,
    pub shadows: ShadowMode,
    pub gi: GiMode,
    pub reflections: ReflectionMode,
    pub reconstruction: ReconstructionHook,
    pub features: LuxFeatureToggles,
}

impl LuxSettings {
    #[must_use]
    pub const fn compiled_default() -> Self {
        Self {
            direct_lighting: if cfg!(feature = "fun_lux_many_light") {
                DirectLightingMode::ReservoirManyLight
            } else {
                DirectLightingMode::TiledClustered
            },
            shadows: if cfg!(feature = "fun_lux_virtual_shadows") {
                ShadowMode::VirtualDemandPaged
            } else {
                ShadowMode::StaticMaps
            },
            gi: if cfg!(feature = "fun_lux_hybrid_gi") {
                GiMode::Hybrid
            } else {
                GiMode::ScreenSpace
            },
            reflections: ReflectionMode::SurfaceCache,
            reconstruction: ReconstructionHook::new(
                "fun_lux.reconstruction.balanced_fast",
                LuxDenoiseReconstructionPath::BalancedFast,
            ),
            features: LuxFeatureToggles::COMPILED,
        }
    }
}

impl Default for LuxSettings {
    fn default() -> Self {
        Self::compiled_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightDatabaseRecord {
    pub stable_light_key: u64,
    pub intensity_lux: f32,
    pub casts_shadow: bool,
    pub emissive_candidate: bool,
}

pub trait LightDatabase {
    fn upsert_light(&mut self, light: LightDatabaseRecord) -> u32;
    fn remove_light(&mut self, stable_light_key: u64) -> bool;
    fn light_count(&self) -> u32;
}

pub trait LuxRendererHooks {
    fn direct_lighting_mode(&self) -> DirectLightingMode;
    fn shadow_mode(&self) -> ShadowMode;
    fn gi_mode(&self) -> GiMode;
    fn reflection_mode(&self) -> ReflectionMode;
    fn reconstruction_hook(&self) -> ReconstructionHook;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxBootReport {
    pub direct_lighting: DirectLightingMode,
    pub shadows: ShadowMode,
    pub gi: GiMode,
    pub reflections: ReflectionMode,
    pub many_light_compiled: bool,
    pub virtual_shadows_compiled: bool,
    pub hybrid_gi_compiled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoopLuxCore {
    settings: LuxSettings,
    lights: Vec<LightDatabaseRecord>,
}

impl NoopLuxCore {
    #[must_use]
    pub fn boot(settings: LuxSettings) -> (Self, LuxBootReport) {
        let report = LuxBootReport {
            direct_lighting: settings.direct_lighting,
            shadows: settings.shadows,
            gi: settings.gi,
            reflections: settings.reflections,
            many_light_compiled: settings.features.many_light,
            virtual_shadows_compiled: settings.features.virtual_shadows,
            hybrid_gi_compiled: settings.features.hybrid_gi,
        };
        (
            Self {
                settings,
                lights: Vec::new(),
            },
            report,
        )
    }

    #[must_use]
    pub const fn settings(&self) -> LuxSettings {
        self.settings
    }
}

impl LightDatabase for NoopLuxCore {
    fn upsert_light(&mut self, light: LightDatabaseRecord) -> u32 {
        if let Some(index) = self
            .lights
            .iter()
            .position(|candidate| candidate.stable_light_key == light.stable_light_key)
        {
            self.lights[index] = light;
            return index as u32;
        }
        let Ok(index) = u32::try_from(self.lights.len()) else {
            return u32::MAX;
        };
        self.lights.push(light);
        index
    }

    fn remove_light(&mut self, stable_light_key: u64) -> bool {
        if let Some(index) = self
            .lights
            .iter()
            .position(|candidate| candidate.stable_light_key == stable_light_key)
        {
            self.lights.swap_remove(index);
            return true;
        }
        false
    }

    fn light_count(&self) -> u32 {
        self.lights.len() as u32
    }
}

impl LuxRendererHooks for NoopLuxCore {
    fn direct_lighting_mode(&self) -> DirectLightingMode {
        self.settings.direct_lighting
    }

    fn shadow_mode(&self) -> ShadowMode {
        self.settings.shadows
    }

    fn gi_mode(&self) -> GiMode {
        self.settings.gi
    }

    fn reflection_mode(&self) -> ReflectionMode {
        self.settings.reflections
    }

    fn reconstruction_hook(&self) -> ReconstructionHook {
        self.settings.reconstruction
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_feature_toggles_expose_lux_flags() {
        let toggles = LuxFeatureToggles::compiled();

        assert_eq!(toggles.many_light, cfg!(feature = "fun_lux_many_light"));
        assert_eq!(
            toggles.virtual_shadows,
            cfg!(feature = "fun_lux_virtual_shadows")
        );
        assert_eq!(toggles.hybrid_gi, cfg!(feature = "fun_lux_hybrid_gi"));
    }

    #[test]
    fn noop_lux_core_boots_lighting_surface_without_gpu_work() {
        let (core, report) = NoopLuxCore::boot(LuxSettings::default());

        assert_eq!(core.direct_lighting_mode(), report.direct_lighting);
        assert_eq!(core.shadow_mode(), report.shadows);
        assert_eq!(core.gi_mode(), report.gi);
        assert_eq!(core.reflection_mode(), report.reflections);
        assert!(!core.reconstruction_hook().model_assisted);
    }

    #[test]
    fn light_database_interface_tracks_records() {
        let (mut core, _) = NoopLuxCore::boot(LuxSettings::default());

        let light = LightDatabaseRecord {
            stable_light_key: 42,
            intensity_lux: 80_000.0,
            casts_shadow: true,
            emissive_candidate: false,
        };

        assert_eq!(core.upsert_light(light), 0);
        assert_eq!(core.light_count(), 1);
        assert_eq!(core.upsert_light(light), 0);
        assert_eq!(core.light_count(), 1);
        assert!(core.remove_light(42));
        assert_eq!(core.light_count(), 0);
    }
}
