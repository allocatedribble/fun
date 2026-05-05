pub mod fun_asset;
pub mod hot_reload;
pub mod parser;
pub mod serializer;

pub use fun_asset::*;
pub use hot_reload::*;
pub use parser::*;
pub use serializer::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneFormatPolicy {
    pub extension: &'static str,
    pub compatible_with_fun_macro_syntax: bool,
    pub arbitrary_rust_expressions_allowed: bool,
}

impl FunSceneFormatPolicy {
    pub const DEFAULT: Self = Self {
        extension: "fun",
        compatible_with_fun_macro_syntax: true,
        arbitrary_rust_expressions_allowed: false,
    };
}

impl Default for FunSceneFormatPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
