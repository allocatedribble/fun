#![forbid(unsafe_code)]

use bevy_ecs::prelude::{Component, Message, Resource};

pub const FUN_LUX_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_PACKAGE_NAME: &str = "fun-lux";
pub const FUN_LUX_CRATE_NAME: &str = "fun_lux";
pub const FUN_LUX_ECS_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxSubsystem {
    DirectLighting,
    ManyLightSampling,
    VirtualShadowPolicy,
    GlobalIllumination,
    Reflections,
    DenoisingReconstruction,
    RadianceCache,
    SurfaceCache,
    ProbeCache,
}

impl FunLuxSubsystem {
    pub const ALL: [Self; 9] = [
        Self::DirectLighting,
        Self::ManyLightSampling,
        Self::VirtualShadowPolicy,
        Self::GlobalIllumination,
        Self::Reflections,
        Self::DenoisingReconstruction,
        Self::RadianceCache,
        Self::SurfaceCache,
        Self::ProbeCache,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLighting => "direct_lighting",
            Self::ManyLightSampling => "many_light_sampling",
            Self::VirtualShadowPolicy => "virtual_shadow_policy",
            Self::GlobalIllumination => "global_illumination",
            Self::Reflections => "reflections",
            Self::DenoisingReconstruction => "denoising_reconstruction",
            Self::RadianceCache => "radiance_cache",
            Self::SurfaceCache => "surface_cache",
            Self::ProbeCache => "probe_cache",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxPolicyKind {
    DirectLighting,
    ManyLightSampling,
    VirtualShadows,
    GlobalIllumination,
    Reflections,
    DenoisingReconstruction,
}

impl FunLuxPolicyKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLighting => "direct_lighting",
            Self::ManyLightSampling => "many_light_sampling",
            Self::VirtualShadows => "virtual_shadows",
            Self::GlobalIllumination => "global_illumination",
            Self::Reflections => "reflections",
            Self::DenoisingReconstruction => "denoising_reconstruction",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxCacheKind {
    Radiance,
    Surface,
    Probe,
}

impl FunLuxCacheKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Radiance => "radiance",
            Self::Surface => "surface",
            Self::Probe => "probe",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxDenoiseReconstructionPath {
    BalancedFast,
    BalancedQuality,
    DlssRayReconstruction,
    ModelAssisted,
    HeuristicFallback,
}

impl FunLuxDenoiseReconstructionPath {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BalancedFast => "balanced_fast",
            Self::BalancedQuality => "balanced_quality",
            Self::DlssRayReconstruction => "dlss_ray_reconstruction",
            Self::ModelAssisted => "model_assisted",
            Self::HeuristicFallback => "heuristic_fallback",
        }
    }

    #[must_use]
    pub const fn requires_fun_ai_runtime(self) -> bool {
        matches!(self, Self::ModelAssisted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunLuxPolicyDescriptor {
    pub stable_id: &'static str,
    pub subsystem: FunLuxSubsystem,
    pub policy_kind: FunLuxPolicyKind,
    pub default_path: &'static str,
    pub owner_crate: &'static str,
    pub accepts_identifiable_data: bool,
}

pub const FUN_LUX_POLICY_DESCRIPTORS: [FunLuxPolicyDescriptor; 6] = [
    FunLuxPolicyDescriptor {
        stable_id: "fun_lux.policy.direct_lighting",
        subsystem: FunLuxSubsystem::DirectLighting,
        policy_kind: FunLuxPolicyKind::DirectLighting,
        default_path: "many_light_reuse",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    FunLuxPolicyDescriptor {
        stable_id: "fun_lux.policy.many_light_sampling",
        subsystem: FunLuxSubsystem::ManyLightSampling,
        policy_kind: FunLuxPolicyKind::ManyLightSampling,
        default_path: "reservoir_sampling",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    FunLuxPolicyDescriptor {
        stable_id: "fun_lux.policy.virtual_shadows",
        subsystem: FunLuxSubsystem::VirtualShadowPolicy,
        policy_kind: FunLuxPolicyKind::VirtualShadows,
        default_path: "virtual_shadow_pages",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    FunLuxPolicyDescriptor {
        stable_id: "fun_lux.policy.global_illumination",
        subsystem: FunLuxSubsystem::GlobalIllumination,
        policy_kind: FunLuxPolicyKind::GlobalIllumination,
        default_path: "radiance_cache",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    FunLuxPolicyDescriptor {
        stable_id: "fun_lux.policy.reflections",
        subsystem: FunLuxSubsystem::Reflections,
        policy_kind: FunLuxPolicyKind::Reflections,
        default_path: "surface_cache",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    FunLuxPolicyDescriptor {
        stable_id: "fun_lux.policy.denoising_reconstruction",
        subsystem: FunLuxSubsystem::DenoisingReconstruction,
        policy_kind: FunLuxPolicyKind::DenoisingReconstruction,
        default_path: FunLuxDenoiseReconstructionPath::BalancedFast.as_str(),
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunLuxCacheDescriptor {
    pub stable_id: &'static str,
    pub kind: FunLuxCacheKind,
    pub subsystem: FunLuxSubsystem,
    pub owner_crate: &'static str,
    pub frame_local_default: bool,
    pub accepts_identifiable_data: bool,
}

pub const FUN_LUX_CACHE_DESCRIPTORS: [FunLuxCacheDescriptor; 3] = [
    FunLuxCacheDescriptor {
        stable_id: "fun_lux.cache.radiance",
        kind: FunLuxCacheKind::Radiance,
        subsystem: FunLuxSubsystem::RadianceCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
    FunLuxCacheDescriptor {
        stable_id: "fun_lux.cache.surface",
        kind: FunLuxCacheKind::Surface,
        subsystem: FunLuxSubsystem::SurfaceCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
    FunLuxCacheDescriptor {
        stable_id: "fun_lux.cache.probe",
        kind: FunLuxCacheKind::Probe,
        subsystem: FunLuxSubsystem::ProbeCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunLuxFeatureSet {
    pub direct_lighting: bool,
    pub many_light_sampling: bool,
    pub virtual_shadows: bool,
    pub global_illumination: bool,
    pub reflections: bool,
    pub denoising_reconstruction: bool,
    pub radiance_cache: bool,
    pub surface_cache: bool,
    pub probe_cache: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunLuxLightId(pub u64);

impl FunLuxLightId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxLightKind {
    Directional,
    Punctual,
    Area,
    EmissiveCandidate,
    Probe,
}

impl FunLuxLightKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Punctual => "punctual",
            Self::Area => "area",
            Self::EmissiveCandidate => "emissive_candidate",
            Self::Probe => "probe",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct FunLuxLight {
    pub light_id: FunLuxLightId,
    pub kind: FunLuxLightKind,
    pub intensity_lux: f32,
    pub casts_virtual_shadow: bool,
}

impl FunLuxLight {
    #[must_use]
    pub const fn new(
        light_id: FunLuxLightId,
        kind: FunLuxLightKind,
        intensity_lux: f32,
        casts_virtual_shadow: bool,
    ) -> Self {
        Self {
            light_id,
            kind,
            intensity_lux,
            casts_virtual_shadow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct FunLuxEmissive {
    pub light_id: FunLuxLightId,
    pub candidate_weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunLuxGiParticipant {
    pub cache_kind: FunLuxCacheKind,
    pub dynamic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunLuxProbeCacheParticipant {
    pub probe_id: u32,
    pub participates_in_relighting: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunLuxLightDatabase {
    pub revision: u64,
    pub direct_light_count: u32,
    pub emissive_candidate_count: u32,
    pub gi_participant_count: u32,
    pub virtual_shadow_caster_count: u32,
}

impl FunLuxLightDatabase {
    pub const EMPTY: Self = Self {
        revision: 0,
        direct_light_count: 0,
        emissive_candidate_count: 0,
        gi_participant_count: 0,
        virtual_shadow_caster_count: 0,
    };

    #[must_use]
    pub const fn with_revision(revision: u64) -> Self {
        Self {
            revision,
            ..Self::EMPTY
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunLuxLightEventKind {
    LightChanged,
    EmissiveCandidatePromoted,
    ShadowInvalidated,
    GiCacheInvalidated,
    ProbeCacheInvalidated,
}

impl FunLuxLightEventKind {
    pub const ALL: [Self; 5] = [
        Self::LightChanged,
        Self::EmissiveCandidatePromoted,
        Self::ShadowInvalidated,
        Self::GiCacheInvalidated,
        Self::ProbeCacheInvalidated,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LightChanged => "light_changed",
            Self::EmissiveCandidatePromoted => "emissive_candidate_promoted",
            Self::ShadowInvalidated => "shadow_invalidated",
            Self::GiCacheInvalidated => "gi_cache_invalidated",
            Self::ProbeCacheInvalidated => "probe_cache_invalidated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub struct FunLuxLightEvent {
    pub kind: FunLuxLightEventKind,
    pub light_id: FunLuxLightId,
    pub revision: u64,
}

impl FunLuxLightEvent {
    #[must_use]
    pub const fn new(kind: FunLuxLightEventKind, light_id: FunLuxLightId, revision: u64) -> Self {
        Self {
            kind,
            light_id,
            revision,
        }
    }
}

impl FunLuxFeatureSet {
    pub const DEFAULT_REALTIME: Self = Self {
        direct_lighting: true,
        many_light_sampling: true,
        virtual_shadows: true,
        global_illumination: true,
        reflections: true,
        denoising_reconstruction: true,
        radiance_cache: true,
        surface_cache: true,
        probe_cache: true,
    };

    pub const HEURISTIC_FALLBACK: Self = Self {
        direct_lighting: true,
        many_light_sampling: false,
        virtual_shadows: false,
        global_illumination: false,
        reflections: false,
        denoising_reconstruction: true,
        radiance_cache: false,
        surface_cache: false,
        probe_cache: false,
    };

    #[must_use]
    pub const fn uses_cache(self, kind: FunLuxCacheKind) -> bool {
        match kind {
            FunLuxCacheKind::Radiance => self.radiance_cache,
            FunLuxCacheKind::Surface => self.surface_cache,
            FunLuxCacheKind::Probe => self.probe_cache,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsystem_labels_are_stable() {
        assert_eq!(FunLuxSubsystem::ALL.len(), 9);
        assert_eq!(FunLuxSubsystem::DirectLighting.as_str(), "direct_lighting");
        assert_eq!(
            FunLuxSubsystem::DenoisingReconstruction.as_str(),
            "denoising_reconstruction"
        );
        assert_eq!(FunLuxSubsystem::ProbeCache.as_str(), "probe_cache");
    }

    #[test]
    fn policies_and_caches_are_fun_lux_owned_and_non_identifying() {
        for descriptor in FUN_LUX_POLICY_DESCRIPTORS {
            assert_eq!(descriptor.owner_crate, FUN_LUX_CRATE_NAME);
            assert!(!descriptor.accepts_identifiable_data);
        }

        for descriptor in FUN_LUX_CACHE_DESCRIPTORS {
            assert_eq!(descriptor.owner_crate, FUN_LUX_CRATE_NAME);
            assert!(!descriptor.accepts_identifiable_data);
            assert!(descriptor.frame_local_default);
        }
    }

    #[test]
    fn model_assisted_reconstruction_declares_fun_ai_runtime_dependency() {
        assert!(FunLuxDenoiseReconstructionPath::ModelAssisted.requires_fun_ai_runtime());
        assert!(!FunLuxDenoiseReconstructionPath::BalancedFast.requires_fun_ai_runtime());
        assert!(!FunLuxDenoiseReconstructionPath::DlssRayReconstruction.requires_fun_ai_runtime());
    }

    #[test]
    fn fallback_feature_set_remains_local_and_direct() {
        let features = FunLuxFeatureSet::HEURISTIC_FALLBACK;

        assert!(features.direct_lighting);
        assert!(features.denoising_reconstruction);
        assert!(!features.many_light_sampling);
        assert!(!features.uses_cache(FunLuxCacheKind::Radiance));
        assert!(!features.uses_cache(FunLuxCacheKind::Surface));
        assert!(!features.uses_cache(FunLuxCacheKind::Probe));
    }

    #[test]
    fn lux_light_components_are_ecs_authored_and_database_backed() {
        let light = FunLuxLight::new(
            FunLuxLightId::new(12),
            FunLuxLightKind::Punctual,
            1200.0,
            true,
        );
        assert!(light.light_id.is_valid());
        assert_eq!(light.kind.as_str(), "punctual");
        assert!(light.casts_virtual_shadow);

        let database = FunLuxLightDatabase::with_revision(9);
        assert_eq!(database.revision, 9);
        assert_eq!(database.direct_light_count, 0);
    }

    #[test]
    fn lux_light_events_cover_shadow_and_gi_invalidation() {
        assert_eq!(FunLuxLightEventKind::ALL.len(), 5);
        assert!(
            FunLuxLightEventKind::ALL
                .iter()
                .any(|kind| *kind == FunLuxLightEventKind::ShadowInvalidated)
        );
        assert!(
            FunLuxLightEventKind::ALL
                .iter()
                .any(|kind| *kind == FunLuxLightEventKind::GiCacheInvalidated)
        );
    }
}
