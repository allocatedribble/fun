#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneValidationPolicy {
    pub validate_schema_before_spawn: bool,
    pub require_deterministic_stable_ids: bool,
    pub reject_arbitrary_rust_expressions: bool,
    pub retain_renderer_lux_component_authority: bool,
}

impl FunSceneValidationPolicy {
    pub const DEFAULT: Self = Self {
        validate_schema_before_spawn: true,
        require_deterministic_stable_ids: true,
        reject_arbitrary_rust_expressions: true,
        retain_renderer_lux_component_authority: true,
    };
}

impl Default for FunSceneValidationPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
