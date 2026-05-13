#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunScenePatch {
    pub operation_count: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunSceneListPatch {
    pub scene_patch_count: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunScenePatchInstance {
    pub applied_operation_count: u16,
}

pub trait FunPatchFromTemplate {}
pub trait FunPatchTemplate {}
