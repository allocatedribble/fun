use bevy_ecs::prelude::Component;

use crate::FunSceneOwner;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunSceneLightingDeclarationKind {
    DirectLight,
    EmissiveCandidate,
    VirtualShadowDemandPage,
    RadianceCacheSeed,
    SurfaceCacheSeed,
    ProbeCacheSeed,
}

impl FunSceneLightingDeclarationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLight => "direct_light",
            Self::EmissiveCandidate => "emissive_candidate",
            Self::VirtualShadowDemandPage => "virtual_shadow_demand_page",
            Self::RadianceCacheSeed => "radiance_cache_seed",
            Self::SurfaceCacheSeed => "surface_cache_seed",
            Self::ProbeCacheSeed => "probe_cache_seed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunSceneLightingDeclaration {
    pub kind: FunSceneLightingDeclarationKind,
    pub owner: FunSceneOwner,
}

impl FunSceneLightingDeclaration {
    #[must_use]
    pub const fn new(kind: FunSceneLightingDeclarationKind) -> Self {
        Self {
            kind,
            owner: FunSceneOwner::Lighting,
        }
    }
}
