#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneHotReloadPolicy {
    pub editor_hot_reload_enabled: bool,
    pub runtime_product_hot_reload_requires_validation: bool,
}

impl FunSceneHotReloadPolicy {
    pub const DEFAULT: Self = Self {
        editor_hot_reload_enabled: true,
        runtime_product_hot_reload_requires_validation: true,
    };
}

impl Default for FunSceneHotReloadPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
