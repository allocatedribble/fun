#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunSceneSerializerPolicy {
    pub writes_stable_order: bool,
    pub preserves_named_entities: bool,
}

impl FunSceneSerializerPolicy {
    pub const DEFAULT: Self = Self {
        writes_stable_order: true,
        preserves_named_entities: true,
    };
}

impl Default for FunSceneSerializerPolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
