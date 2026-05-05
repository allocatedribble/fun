#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunPrefabId(pub &'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunPrefabPolicy {
    pub prefab_inheritance_enabled: bool,
    pub prefab_props_enabled: bool,
    pub editor_writes_validated_assets: bool,
}

impl FunPrefabPolicy {
    pub const DEFAULT: Self = Self {
        prefab_inheritance_enabled: true,
        prefab_props_enabled: true,
        editor_writes_validated_assets: true,
    };
}

impl Default for FunPrefabPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
