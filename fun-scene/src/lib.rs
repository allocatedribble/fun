#![forbid(unsafe_code)]

extern crate self as fun_scene;

pub use bevy_scene;
pub use bevy_scene::{
    CommandsSceneExt, EntityCommandsSceneExt, EntityWorldMutSceneExt, PatchFromTemplate,
    PatchTemplate, Scene, SceneComponent, SceneList, ScenePlugin, WorldSceneExt, template_value,
};
pub use fun_scene_macros::{fun, fun_list};

use bevy_ecs::prelude::{Component, Resource};

pub const FUN_SCENE_SCHEMA_VERSION: u16 = 1;
pub const FUN_SCENE_PACKAGE_NAME: &str = "fun-scene";
pub const FUN_SCENE_CRATE_NAME: &str = "fun_scene";
pub const FUN_SCENE_MACRO_PACKAGE_NAME: &str = "fun-scene-macros";
pub const FUN_SCENE_AUTHORING_MACRO: &str = "fun";
pub const FUN_SCENE_LIST_AUTHORING_MACRO: &str = "fun_list";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunSceneOwner {
    SceneAuthoring,
    Renderer,
    Lighting,
    ServerAuthority,
    Editor,
}

impl FunSceneOwner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SceneAuthoring => FUN_SCENE_CRATE_NAME,
            Self::Renderer => "fun_renderer",
            Self::Lighting => "fun_lux",
            Self::ServerAuthority => "game_server",
            Self::Editor => "fun_host",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub struct FunSceneEntityId(pub u64);

impl FunSceneEntityId {
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
pub enum FunSceneRendererDeclarationKind {
    GpuSceneObject,
    VirtualGeometrySource,
    VirtualShadowCaster,
    StreamingPageSource,
    RuntimeProceduralGeometry,
}

impl FunSceneRendererDeclarationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuSceneObject => "gpu_scene_object",
            Self::VirtualGeometrySource => "virtual_geometry_source",
            Self::VirtualShadowCaster => "virtual_shadow_caster",
            Self::StreamingPageSource => "streaming_page_source",
            Self::RuntimeProceduralGeometry => "runtime_procedural_geometry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunSceneRendererDeclaration {
    pub kind: FunSceneRendererDeclarationKind,
    pub owner: FunSceneOwner,
}

impl FunSceneRendererDeclaration {
    #[must_use]
    pub const fn new(kind: FunSceneRendererDeclarationKind) -> Self {
        Self {
            kind,
            owner: FunSceneOwner::Renderer,
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunSceneAuthoringPolicy {
    pub bevy_scene_bsn_reuse_confirmed: bool,
    pub fun_macro_is_primary_authoring_surface: bool,
    pub deterministic_manifest_required: bool,
    pub server_editor_renderer_shared_authority: bool,
    pub renderer_consumes_ecs_archetypes: bool,
    pub lighting_consumes_ecs_declarations: bool,
}

impl FunSceneAuthoringPolicy {
    pub const DEFAULT: Self = Self {
        bevy_scene_bsn_reuse_confirmed: true,
        fun_macro_is_primary_authoring_surface: true,
        deterministic_manifest_required: true,
        server_editor_renderer_shared_authority: true,
        renderer_consumes_ecs_archetypes: true,
        lighting_consumes_ecs_declarations: true,
    };
}

impl Default for FunSceneAuthoringPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneProductTopology {
    pub scene_package: &'static str,
    pub scene_crate: &'static str,
    pub macro_package: &'static str,
    pub authoring_macro: &'static str,
    pub list_authoring_macro: &'static str,
    pub source_model: &'static str,
}

pub const FUN_SCENE_PRODUCT_TOPOLOGY: FunSceneProductTopology = FunSceneProductTopology {
    scene_package: FUN_SCENE_PACKAGE_NAME,
    scene_crate: FUN_SCENE_CRATE_NAME,
    macro_package: FUN_SCENE_MACRO_PACKAGE_NAME,
    authoring_macro: FUN_SCENE_AUTHORING_MACRO,
    list_authoring_macro: FUN_SCENE_LIST_AUTHORING_MACRO,
    source_model: "bevy_scene_bsn_reused_then_fun_owned",
};

#[cfg(test)]
mod tests {
    use bevy_ecs::world::World;

    use super::*;

    #[test]
    fn topology_names_match_fun_scene_decision() {
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.scene_package, "fun-scene");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.scene_crate, "fun_scene");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.macro_package, "fun-scene-macros");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.authoring_macro, "fun");
        assert_eq!(FUN_SCENE_PRODUCT_TOPOLOGY.list_authoring_macro, "fun_list");
        assert_eq!(
            FUN_SCENE_PRODUCT_TOPOLOGY.source_model,
            "bevy_scene_bsn_reused_then_fun_owned"
        );
    }

    #[test]
    fn policy_confirms_ecs_first_scene_authority() {
        let policy = core::hint::black_box(FunSceneAuthoringPolicy::DEFAULT);

        assert!(policy.bevy_scene_bsn_reuse_confirmed);
        assert!(policy.fun_macro_is_primary_authoring_surface);
        assert!(policy.deterministic_manifest_required);
        assert!(policy.server_editor_renderer_shared_authority);
        assert!(policy.renderer_consumes_ecs_archetypes);
        assert!(policy.lighting_consumes_ecs_declarations);
    }

    #[test]
    fn renderer_and_lighting_declarations_are_ecs_components() {
        let mut world = World::new();
        let entity = world
            .spawn((
                FunSceneEntityId::new(42),
                FunSceneRendererDeclaration::new(
                    FunSceneRendererDeclarationKind::RuntimeProceduralGeometry,
                ),
                FunSceneLightingDeclaration::new(FunSceneLightingDeclarationKind::DirectLight),
            ))
            .id();

        let id = world
            .get::<FunSceneEntityId>(entity)
            .expect("scene entity id should be an ECS component");
        assert!(id.is_valid());

        let render = world
            .get::<FunSceneRendererDeclaration>(entity)
            .expect("renderer declaration should be an ECS component");
        assert_eq!(render.owner, FunSceneOwner::Renderer);
        assert_eq!(
            render.kind.as_str(),
            FunSceneRendererDeclarationKind::RuntimeProceduralGeometry.as_str()
        );

        let lighting = world
            .get::<FunSceneLightingDeclaration>(entity)
            .expect("lighting declaration should be an ECS component");
        assert_eq!(lighting.owner, FunSceneOwner::Lighting);
        assert_eq!(
            lighting.kind.as_str(),
            FunSceneLightingDeclarationKind::DirectLight.as_str()
        );
    }
}
