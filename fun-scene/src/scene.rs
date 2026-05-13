use fun_ecs::Resource;

pub trait FunScene: 'static {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSceneDeclaration {
    pub node_count_hint: u16,
}

impl FunSceneDeclaration {
    #[must_use]
    pub const fn new() -> Self {
        Self { node_count_hint: 0 }
    }
}

impl FunScene for FunSceneDeclaration {}

pub trait FunSceneComponent: fun_ecs::Component {}

impl<T: fun_ecs::Component> FunSceneComponent for T {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunResolveContext {
    pub strict_validation: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSceneDependencies {
    pub component_count: u16,
    pub resource_count: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunInheritSceneError;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunResolveSceneError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunSceneAuthoringPolicy {
    pub fun_scene_macros_are_native: bool,
    pub fun_macro_is_primary_authoring_surface: bool,
    pub deterministic_manifest_required: bool,
    pub server_editor_renderer_shared_authority: bool,
    pub renderer_consumes_ecs_archetypes: bool,
    pub lighting_consumes_ecs_declarations: bool,
}

impl FunSceneAuthoringPolicy {
    pub const DEFAULT: Self = Self {
        fun_scene_macros_are_native: true,
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
