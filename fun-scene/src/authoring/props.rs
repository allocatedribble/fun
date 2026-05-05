#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunScenePropName(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunScenePropsPolicy {
    pub typed_props_only: bool,
    pub dynamic_expressions_validated: bool,
}

impl FunScenePropsPolicy {
    pub const DEFAULT: Self = Self {
        typed_props_only: true,
        dynamic_expressions_validated: true,
    };
}

impl Default for FunScenePropsPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
