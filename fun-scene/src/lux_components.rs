use bevy_color::Color;
use bevy_ecs::prelude::Component;

use crate::{FunFromTemplate, FunSceneOwner};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLightKind {
    Directional,
    #[default]
    Punctual,
    Area,
    EmissiveCandidate,
    Probe,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxShadowPolicy {
    None,
    StaticMap,
    #[default]
    VirtualDemandPaged,
    RayTraced,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxImportance {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct LuxLight {
    pub kind: LuxLightKind,
    pub color: Color,
    pub intensity_lux: f32,
    pub range: f32,
    pub shadow_policy: LuxShadowPolicy,
    pub importance: LuxImportance,
}

impl LuxLight {
    #[must_use]
    pub fn directional(intensity_lux: f32) -> Self {
        Self {
            kind: LuxLightKind::Directional,
            color: Color::WHITE,
            intensity_lux,
            range: 0.0,
            shadow_policy: LuxShadowPolicy::VirtualDemandPaged,
            importance: LuxImportance::High,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmissiveCandidatePolicy {
    Never,
    #[default]
    AutoPromote,
    AlwaysPromote,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct LuxEmissive {
    pub luminance: f32,
    pub candidate_policy: EmissiveCandidatePolicy,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiBouncePolicy {
    Disabled,
    StaticSingleBounce,
    #[default]
    DynamicBudgeted,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GiCachePolicy {
    None,
    Radiance,
    Surface,
    #[default]
    Probe,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct LuxGiParticipant {
    pub bounce_policy: GiBouncePolicy,
    pub cache_policy: GiCachePolicy,
}

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
