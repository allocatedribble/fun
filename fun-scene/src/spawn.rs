use core::marker::PhantomData;

use fun_ecs::{entity::Entity, system::Commands, world::World};

use crate::{FunScene, FunSceneList};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSpawnSceneError;

#[derive(Debug)]
pub struct EntityCommands<'world> {
    entity: Entity,
    marker: PhantomData<&'world mut ()>,
}

impl<'world> EntityCommands<'world> {
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        Self {
            entity,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn id(&self) -> Entity {
        self.entity
    }
}

#[derive(Debug)]
pub struct EntityWorldMut<'world> {
    entity: Entity,
    marker: PhantomData<&'world mut ()>,
}

impl<'world> EntityWorldMut<'world> {
    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        Self {
            entity,
            marker: PhantomData,
        }
    }

    #[must_use]
    pub const fn id(&self) -> Entity {
        self.entity
    }
}

pub trait CommandsFunSceneExt {
    fn spawn_fun_scene<S: FunScene>(&mut self, scene: S) -> EntityCommands<'_>;
    fn queue_spawn_fun_scene<S: FunScene>(&mut self, scene: S) -> EntityCommands<'_>;
    fn spawn_fun_scene_list<L: FunSceneList>(&mut self, scenes: L);
    fn queue_spawn_fun_scene_list<L: FunSceneList>(&mut self, scenes: L);
}

impl CommandsFunSceneExt for Commands {
    fn spawn_fun_scene<S: FunScene>(&mut self, scene: S) -> EntityCommands<'_> {
        let _ = scene;
        EntityCommands::new(Entity::INVALID)
    }

    fn queue_spawn_fun_scene<S: FunScene>(&mut self, scene: S) -> EntityCommands<'_> {
        self.spawn_fun_scene(scene)
    }

    fn spawn_fun_scene_list<L: FunSceneList>(&mut self, scenes: L) {
        let _ = scenes;
    }

    fn queue_spawn_fun_scene_list<L: FunSceneList>(&mut self, scenes: L) {
        self.spawn_fun_scene_list(scenes);
    }
}

pub trait WorldFunSceneExt {
    fn spawn_fun_scene<S: FunScene>(
        &mut self,
        scene: S,
    ) -> Result<EntityWorldMut<'_>, FunSpawnSceneError>;
    fn queue_spawn_fun_scene<S: FunScene>(&mut self, scene: S) -> EntityWorldMut<'_>;
    fn spawn_fun_scene_list<L: FunSceneList>(
        &mut self,
        scenes: L,
    ) -> Result<Vec<Entity>, FunSpawnSceneError>;
    fn queue_spawn_fun_scene_list<L: FunSceneList>(&mut self, scenes: L);
}

impl WorldFunSceneExt for World {
    fn spawn_fun_scene<S: FunScene>(
        &mut self,
        scene: S,
    ) -> Result<EntityWorldMut<'_>, FunSpawnSceneError> {
        let _ = scene;
        let entity = self.spawn(()).id();
        Ok(EntityWorldMut::new(entity))
    }

    fn queue_spawn_fun_scene<S: FunScene>(&mut self, scene: S) -> EntityWorldMut<'_> {
        self.spawn_fun_scene(scene)
            .unwrap_or_else(|_| EntityWorldMut::new(Entity::INVALID))
    }

    fn spawn_fun_scene_list<L: FunSceneList>(
        &mut self,
        scenes: L,
    ) -> Result<Vec<Entity>, FunSpawnSceneError> {
        let _ = scenes;
        Ok(Vec::new())
    }

    fn queue_spawn_fun_scene_list<L: FunSceneList>(&mut self, scenes: L) {
        let _ = self.spawn_fun_scene_list(scenes);
    }
}

pub trait EntityWorldMutFunSceneExt {
    fn queue_spawn_related_fun_scenes<T>(self, scenes: impl FunSceneList) -> Self;
    fn apply_fun_scene<S: FunScene>(&mut self, scene: S) -> Result<(), FunSpawnSceneError>;
    fn queue_apply_fun_scene<S: FunScene>(&mut self, scene: S);
}

impl EntityWorldMutFunSceneExt for EntityWorldMut<'_> {
    fn queue_spawn_related_fun_scenes<T>(self, scenes: impl FunSceneList) -> Self {
        let _ = (PhantomData::<T>, scenes);
        self
    }

    fn apply_fun_scene<S: FunScene>(&mut self, scene: S) -> Result<(), FunSpawnSceneError> {
        let _ = scene;
        Ok(())
    }

    fn queue_apply_fun_scene<S: FunScene>(&mut self, scene: S) {
        let _ = self.apply_fun_scene(scene);
    }
}

pub trait EntityCommandsFunSceneExt {
    fn queue_spawn_related_fun_scenes<T>(&mut self, scenes: impl FunSceneList) -> &mut Self;
    fn apply_fun_scene<S: FunScene>(&mut self, scene: S) -> &mut Self;
    fn queue_apply_fun_scene<S: FunScene>(&mut self, scene: S) -> &mut Self;
}

impl EntityCommandsFunSceneExt for EntityCommands<'_> {
    fn queue_spawn_related_fun_scenes<T>(&mut self, scenes: impl FunSceneList) -> &mut Self {
        let _ = (PhantomData::<T>, scenes);
        self
    }

    fn apply_fun_scene<S: FunScene>(&mut self, scene: S) -> &mut Self {
        let _ = scene;
        self
    }

    fn queue_apply_fun_scene<S: FunScene>(&mut self, scene: S) -> &mut Self {
        self.apply_fun_scene(scene)
    }
}
