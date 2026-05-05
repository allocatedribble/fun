pub trait FunScene: bevy_scene::Scene {}

impl<T: bevy_scene::Scene> FunScene for T {}

use bevy_ecs::prelude::Resource;

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

pub use bevy_scene::{
    InheritSceneError as FunInheritSceneError, ResolveContext as FunResolveContext,
    ResolveSceneError as FunResolveSceneError, Scene as BevyScene,
    SceneComponent as FunSceneComponent, SceneDependencies as FunSceneDependencies, on as fun_on,
};
