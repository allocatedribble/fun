#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneAssetPolicy {
    pub macro_only_short_term: bool,
    pub fun_asset_format_mid_term: bool,
    pub arbitrary_rust_expressions_allowed_in_assets: bool,
    pub hot_reload_for_editor: bool,
}

impl FunSceneAssetPolicy {
    pub const DEFAULT: Self = Self {
        macro_only_short_term: true,
        fun_asset_format_mid_term: true,
        arbitrary_rust_expressions_allowed_in_assets: false,
        hot_reload_for_editor: true,
    };
}

impl Default for FunSceneAssetPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
