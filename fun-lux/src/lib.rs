#![forbid(unsafe_code)]

use bevy_ecs::prelude::{Component, Message, Resource};

pub const FUN_LUX_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_PACKAGE_NAME: &str = "fun-lux";
pub const FUN_LUX_CRATE_NAME: &str = "fun_lux";
pub const FUN_LUX_ECS_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxSubsystem {
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

impl LuxSubsystem {
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
pub enum LuxPolicyKind {
    DirectLighting,
    ManyLightSampling,
    VirtualShadows,
    GlobalIllumination,
    Reflections,
    DenoisingReconstruction,
}

impl LuxPolicyKind {
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
pub enum LuxCacheKind {
    Radiance,
    Surface,
    Probe,
}

impl LuxCacheKind {
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
pub enum LuxDenoiseReconstructionPath {
    BalancedFast,
    BalancedQuality,
    DlssRayReconstruction,
    ModelAssisted,
    HeuristicFallback,
}

impl LuxDenoiseReconstructionPath {
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
pub struct LuxPolicyDescriptor {
    pub stable_id: &'static str,
    pub subsystem: LuxSubsystem,
    pub policy_kind: LuxPolicyKind,
    pub default_path: &'static str,
    pub owner_crate: &'static str,
    pub accepts_identifiable_data: bool,
}

pub const FUN_LUX_POLICY_DESCRIPTORS: [LuxPolicyDescriptor; 6] = [
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.direct_lighting",
        subsystem: LuxSubsystem::DirectLighting,
        policy_kind: LuxPolicyKind::DirectLighting,
        default_path: "many_light_reuse",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.many_light_sampling",
        subsystem: LuxSubsystem::ManyLightSampling,
        policy_kind: LuxPolicyKind::ManyLightSampling,
        default_path: "reservoir_sampling",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.virtual_shadows",
        subsystem: LuxSubsystem::VirtualShadowPolicy,
        policy_kind: LuxPolicyKind::VirtualShadows,
        default_path: "virtual_shadow_pages",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.global_illumination",
        subsystem: LuxSubsystem::GlobalIllumination,
        policy_kind: LuxPolicyKind::GlobalIllumination,
        default_path: "radiance_cache",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.reflections",
        subsystem: LuxSubsystem::Reflections,
        policy_kind: LuxPolicyKind::Reflections,
        default_path: "surface_cache",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.denoising_reconstruction",
        subsystem: LuxSubsystem::DenoisingReconstruction,
        policy_kind: LuxPolicyKind::DenoisingReconstruction,
        default_path: LuxDenoiseReconstructionPath::BalancedFast.as_str(),
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxCacheDescriptor {
    pub stable_id: &'static str,
    pub kind: LuxCacheKind,
    pub subsystem: LuxSubsystem,
    pub owner_crate: &'static str,
    pub frame_local_default: bool,
    pub accepts_identifiable_data: bool,
}

pub const FUN_LUX_CACHE_DESCRIPTORS: [LuxCacheDescriptor; 3] = [
    LuxCacheDescriptor {
        stable_id: "fun_lux.cache.radiance",
        kind: LuxCacheKind::Radiance,
        subsystem: LuxSubsystem::RadianceCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
    LuxCacheDescriptor {
        stable_id: "fun_lux.cache.surface",
        kind: LuxCacheKind::Surface,
        subsystem: LuxSubsystem::SurfaceCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
    LuxCacheDescriptor {
        stable_id: "fun_lux.cache.probe",
        kind: LuxCacheKind::Probe,
        subsystem: LuxSubsystem::ProbeCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxFeatureSet {
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
pub struct LuxLightId(pub u64);

impl LuxLightId {
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
pub enum LuxLightKind {
    Directional,
    Punctual,
    Area,
    EmissiveCandidate,
    Probe,
}

impl LuxLightKind {
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
pub struct LuxLight {
    pub light_id: LuxLightId,
    pub kind: LuxLightKind,
    pub intensity_lux: f32,
    pub casts_virtual_shadow: bool,
}

impl LuxLight {
    #[must_use]
    pub const fn new(
        light_id: LuxLightId,
        kind: LuxLightKind,
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
pub struct LuxEmissive {
    pub light_id: LuxLightId,
    pub candidate_weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LuxGiParticipant {
    pub cache_kind: LuxCacheKind,
    pub dynamic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LuxProbeCacheParticipant {
    pub probe_id: u32,
    pub participates_in_relighting: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct LuxLightDatabase {
    pub revision: u64,
    pub direct_light_count: u32,
    pub emissive_candidate_count: u32,
    pub gi_participant_count: u32,
    pub virtual_shadow_caster_count: u32,
}

impl LuxLightDatabase {
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
pub enum LuxLightEventKind {
    LightChanged,
    EmissiveCandidatePromoted,
    ShadowInvalidated,
    GiCacheInvalidated,
    ProbeCacheInvalidated,
}

impl LuxLightEventKind {
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
pub struct LuxLightEvent {
    pub kind: LuxLightEventKind,
    pub light_id: LuxLightId,
    pub revision: u64,
}

impl LuxLightEvent {
    #[must_use]
    pub const fn new(kind: LuxLightEventKind, light_id: LuxLightId, revision: u64) -> Self {
        Self {
            kind,
            light_id,
            revision,
        }
    }
}

impl LuxFeatureSet {
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
    pub const fn uses_cache(self, kind: LuxCacheKind) -> bool {
        match kind {
            LuxCacheKind::Radiance => self.radiance_cache,
            LuxCacheKind::Surface => self.surface_cache,
            LuxCacheKind::Probe => self.probe_cache,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsystem_labels_are_stable() {
        assert_eq!(LuxSubsystem::ALL.len(), 9);
        assert_eq!(LuxSubsystem::DirectLighting.as_str(), "direct_lighting");
        assert_eq!(
            LuxSubsystem::DenoisingReconstruction.as_str(),
            "denoising_reconstruction"
        );
        assert_eq!(LuxSubsystem::ProbeCache.as_str(), "probe_cache");
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
        assert!(LuxDenoiseReconstructionPath::ModelAssisted.requires_fun_ai_runtime());
        assert!(!LuxDenoiseReconstructionPath::BalancedFast.requires_fun_ai_runtime());
        assert!(!LuxDenoiseReconstructionPath::DlssRayReconstruction.requires_fun_ai_runtime());
    }

    #[test]
    fn fallback_feature_set_remains_local_and_direct() {
        let features = LuxFeatureSet::HEURISTIC_FALLBACK;

        assert!(features.direct_lighting);
        assert!(features.denoising_reconstruction);
        assert!(!features.many_light_sampling);
        assert!(!features.uses_cache(LuxCacheKind::Radiance));
        assert!(!features.uses_cache(LuxCacheKind::Surface));
        assert!(!features.uses_cache(LuxCacheKind::Probe));
    }

    #[test]
    fn lux_light_components_are_ecs_authored_and_database_backed() {
        let light = LuxLight::new(LuxLightId::new(12), LuxLightKind::Punctual, 1200.0, true);
        assert!(light.light_id.is_valid());
        assert_eq!(light.kind.as_str(), "punctual");
        assert!(light.casts_virtual_shadow);

        let database = LuxLightDatabase::with_revision(9);
        assert_eq!(database.revision, 9);
        assert_eq!(database.direct_light_count, 0);
    }

    #[test]
    fn lux_light_events_cover_shadow_and_gi_invalidation() {
        assert_eq!(LuxLightEventKind::ALL.len(), 5);
        assert!(
            LuxLightEventKind::ALL
                .iter()
                .any(|kind| *kind == LuxLightEventKind::ShadowInvalidated)
        );
        assert!(
            LuxLightEventKind::ALL
                .iter()
                .any(|kind| *kind == LuxLightEventKind::GiCacheInvalidated)
        );
    }
}
