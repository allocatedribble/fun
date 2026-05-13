#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunScenePlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunScenePluginPolicy {
    pub installs_fun_scene_spawner: bool,
    pub fun_scene_is_authoring_surface: bool,
}

impl FunScenePluginPolicy {
    pub const DEFAULT: Self = Self {
        installs_fun_scene_spawner: true,
        fun_scene_is_authoring_surface: true,
    };
}

impl Default for FunScenePluginPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
