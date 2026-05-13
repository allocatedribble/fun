pub trait FunSceneList {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEmptySceneList;

impl FunSceneList for FunEmptySceneList {}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSceneListBox {
    pub scene_count_hint: u16,
}

impl FunSceneListBox {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            scene_count_hint: 0,
        }
    }
}

impl FunSceneList for FunSceneListBox {}
