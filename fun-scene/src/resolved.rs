#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResolvedFunScene {
    pub node_count: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResolvedFunSceneRoot {
    pub node_count: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ResolvedFunSceneListRoot {
    pub scene_count: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunApplySceneError;
