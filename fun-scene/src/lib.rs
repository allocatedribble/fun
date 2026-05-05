#![forbid(unsafe_code)]

extern crate self as fun_scene;

pub mod asset;
pub mod authoring;
pub mod diagnostics;
pub mod format;
pub mod lux_components;
pub mod manifest;
pub mod observers;
pub mod patch;
pub mod plugin;
pub mod prelude;
pub mod renderer_components;
pub mod resolved;
pub mod scene;
pub mod scene_list;
pub mod schedule;
pub mod spawn;
pub mod stable_identity;
pub mod streaming;
pub mod template;
pub mod template_value;
pub mod validation;

pub use bevy_scene;
#[deprecated(note = "use fun_scene::fun instead of the temporary bsn alias")]
pub use fun_scene_macros::fun as bsn;
#[deprecated(note = "use fun_scene::fun_list instead of the temporary bsn_list alias")]
pub use fun_scene_macros::fun_list as bsn_list;
pub use fun_scene_macros::{fun, fun_list};

pub use asset::*;
pub use authoring::*;
pub use diagnostics::*;
pub use format::*;
pub use lux_components::*;
pub use manifest::*;
pub use observers::*;
pub use patch::*;
pub use plugin::*;
pub use renderer_components::*;
pub use resolved::*;
pub use scene::*;
pub use scene_list::*;
pub use schedule::*;
pub use spawn::*;
pub use stable_identity::*;
pub use streaming::*;
pub use template::*;
pub use template_value::*;
pub use validation::*;

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

    #[test]
    fn validation_policy_requires_safe_asset_boundary() {
        let policy = FunSceneValidationPolicy::DEFAULT;

        assert!(policy.reject_arbitrary_rust_expressions);
        assert!(policy.validate_schema_before_spawn);
        assert!(policy.require_deterministic_stable_ids);
        assert!(policy.retain_renderer_lux_component_authority);
    }
}
