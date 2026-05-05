#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneParserPolicy {
    pub accepts_macro_like_syntax: bool,
    pub rejects_rust_expressions: bool,
    pub validates_named_entities: bool,
}

impl FunSceneParserPolicy {
    pub const DEFAULT: Self = Self {
        accepts_macro_like_syntax: true,
        rejects_rust_expressions: true,
        validates_named_entities: true,
    };
}

impl Default for FunSceneParserPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
