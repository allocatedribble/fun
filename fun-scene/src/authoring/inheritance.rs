#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneInheritancePolicy {
    pub macro_inheritance_enabled: bool,
    pub asset_inheritance_enabled_after_validation: bool,
}

impl FunSceneInheritancePolicy {
    pub const DEFAULT: Self = Self {
        macro_inheritance_enabled: true,
        asset_inheritance_enabled_after_validation: true,
    };
}

impl Default for FunSceneInheritancePolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
