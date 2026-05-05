use bevy_ecs::{
    entity::Entity,
    relationship::RelationshipTarget,
    system::{Commands, EntityCommands},
    world::{EntityWorldMut, World},
};
use bevy_scene::{
    CommandsSceneExt as BevyCommandsSceneExt, EntityCommandsSceneExt as BevyEntityCommandsSceneExt,
    EntityWorldMutSceneExt as BevyEntityWorldMutSceneExt, Scene, SceneList, SpawnSceneError,
    WorldSceneExt as BevyWorldSceneExt,
};

pub use bevy_scene::SpawnSceneError as FunSpawnSceneError;

pub trait CommandsFunSceneExt {
    fn spawn_fun_scene<S: Scene>(&mut self, scene: S) -> EntityCommands<'_>;
    fn queue_spawn_fun_scene<S: Scene>(&mut self, scene: S) -> EntityCommands<'_>;
    fn spawn_fun_scene_list<L: SceneList>(&mut self, scenes: L);
    fn queue_spawn_fun_scene_list<L: SceneList>(&mut self, scenes: L);
}

impl<'w, 's> CommandsFunSceneExt for Commands<'w, 's> {
    fn spawn_fun_scene<S: Scene>(&mut self, scene: S) -> EntityCommands<'_> {
        BevyCommandsSceneExt::spawn_scene(self, scene)
    }

    fn queue_spawn_fun_scene<S: Scene>(&mut self, scene: S) -> EntityCommands<'_> {
        BevyCommandsSceneExt::queue_spawn_scene(self, scene)
    }

    fn spawn_fun_scene_list<L: SceneList>(&mut self, scenes: L) {
        BevyCommandsSceneExt::spawn_scene_list(self, scenes);
    }

    fn queue_spawn_fun_scene_list<L: SceneList>(&mut self, scenes: L) {
        BevyCommandsSceneExt::queue_spawn_scene_list(self, scenes);
    }
}

pub trait WorldFunSceneExt {
    fn spawn_fun_scene<S: Scene>(
        &mut self,
        scene: S,
    ) -> Result<EntityWorldMut<'_>, SpawnSceneError>;
    fn queue_spawn_fun_scene<S: Scene>(&mut self, scene: S) -> EntityWorldMut<'_>;
    fn spawn_fun_scene_list<L: SceneList>(
        &mut self,
        scenes: L,
    ) -> Result<Vec<Entity>, SpawnSceneError>;
    fn queue_spawn_fun_scene_list<L: SceneList>(&mut self, scenes: L);
}

impl WorldFunSceneExt for World {
    fn spawn_fun_scene<S: Scene>(
        &mut self,
        scene: S,
    ) -> Result<EntityWorldMut<'_>, SpawnSceneError> {
        BevyWorldSceneExt::spawn_scene(self, scene)
    }

    fn queue_spawn_fun_scene<S: Scene>(&mut self, scene: S) -> EntityWorldMut<'_> {
        BevyWorldSceneExt::queue_spawn_scene(self, scene)
    }

    fn spawn_fun_scene_list<L: SceneList>(
        &mut self,
        scenes: L,
    ) -> Result<Vec<Entity>, SpawnSceneError> {
        BevyWorldSceneExt::spawn_scene_list(self, scenes)
    }

    fn queue_spawn_fun_scene_list<L: SceneList>(&mut self, scenes: L) {
        BevyWorldSceneExt::queue_spawn_scene_list(self, scenes);
    }
}

pub trait EntityWorldMutFunSceneExt {
    fn queue_spawn_related_fun_scenes<T: RelationshipTarget>(self, scenes: impl SceneList) -> Self;
    fn apply_fun_scene<S: Scene>(&mut self, scene: S) -> Result<(), SpawnSceneError>;
    fn queue_apply_fun_scene<S: Scene>(&mut self, scene: S);
}

impl EntityWorldMutFunSceneExt for EntityWorldMut<'_> {
    fn queue_spawn_related_fun_scenes<T: RelationshipTarget>(self, scenes: impl SceneList) -> Self {
        BevyEntityWorldMutSceneExt::queue_spawn_related_scenes::<T>(self, scenes)
    }

    fn apply_fun_scene<S: Scene>(&mut self, scene: S) -> Result<(), SpawnSceneError> {
        BevyEntityWorldMutSceneExt::apply_scene(self, scene)
    }

    fn queue_apply_fun_scene<S: Scene>(&mut self, scene: S) {
        BevyEntityWorldMutSceneExt::queue_apply_scene(self, scene);
    }
}

pub trait EntityCommandsFunSceneExt {
    fn queue_spawn_related_fun_scenes<T: RelationshipTarget>(
        &mut self,
        scenes: impl SceneList,
    ) -> &mut Self;
    fn apply_fun_scene<S: Scene>(&mut self, scene: S) -> &mut Self;
    fn queue_apply_fun_scene<S: Scene>(&mut self, scene: S) -> &mut Self;
}

impl EntityCommandsFunSceneExt for EntityCommands<'_> {
    fn queue_spawn_related_fun_scenes<T: RelationshipTarget>(
        &mut self,
        scenes: impl SceneList,
    ) -> &mut Self {
        BevyEntityCommandsSceneExt::queue_spawn_related_scenes::<T>(self, scenes)
    }

    fn apply_fun_scene<S: Scene>(&mut self, scene: S) -> &mut Self {
        BevyEntityCommandsSceneExt::apply_scene(self, scene)
    }

    fn queue_apply_fun_scene<S: Scene>(&mut self, scene: S) -> &mut Self {
        BevyEntityCommandsSceneExt::queue_apply_scene(self, scene)
    }
}
