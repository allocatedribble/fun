pub trait FunSceneList: bevy_scene::SceneList {}

impl<T: bevy_scene::SceneList> FunSceneList for T {}

pub use bevy_scene::{SceneList as BevySceneList, SceneListBox as FunSceneListBox};
