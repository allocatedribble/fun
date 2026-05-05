pub const FUN_SCENE_ASSET_EXTENSION: &str = "fun";
pub const FUN_SCENE_ASSET_MIME: &str = "application/x-fun-scene";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSceneAssetId(pub &'static str);
