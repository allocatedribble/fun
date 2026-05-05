use bevy_ecs::prelude::{Component, Resource};

use crate::{FunRendererBackend, FunRendererFrameGraphStage, FunRendererSubsystem};

pub const FUN_RENDERER_ECS_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunRendererGpuObjectId(pub u64);

impl FunRendererGpuObjectId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunRendererGpuSceneObject {
    pub object_id: FunRendererGpuObjectId,
    pub subsystem: FunRendererSubsystem,
}

impl FunRendererGpuSceneObject {
    #[must_use]
    pub const fn new(object_id: FunRendererGpuObjectId, subsystem: FunRendererSubsystem) -> Self {
        Self {
            object_id,
            subsystem,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FunRendererFrameGraphNode {
    pub stage: FunRendererFrameGraphStage,
    pub order_key: u16,
}

impl FunRendererFrameGraphNode {
    #[must_use]
    pub const fn new(stage: FunRendererFrameGraphStage) -> Self {
        Self {
            stage,
            order_key: stage.order_key(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRendererEcsSchedulePolicy {
    pub extraction_uses_bevy_ecs: bool,
    pub gpu_scene_database_uses_components: bool,
    pub frame_graph_uses_schedule_sets: bool,
    pub backend: FunRendererBackend,
}

impl FunRendererEcsSchedulePolicy {
    pub const DEFAULT: Self = Self {
        extraction_uses_bevy_ecs: true,
        gpu_scene_database_uses_components: true,
        frame_graph_uses_schedule_sets: true,
        backend: default_backend_for_target(),
    };
}

impl Default for FunRendererEcsSchedulePolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[must_use]
pub const fn default_backend_for_target() -> FunRendererBackend {
    if cfg!(target_os = "windows") {
        FunRendererBackend::Dx12
    } else {
        FunRendererBackend::Vulkan
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::world::World;

    use super::*;

    #[test]
    fn renderer_core_has_working_bevy_ecs_surface() {
        let mut world = World::new();
        world.insert_resource(FunRendererEcsSchedulePolicy::DEFAULT);

        let entity = world
            .spawn((
                FunRendererGpuSceneObject::new(
                    FunRendererGpuObjectId::new(7),
                    FunRendererSubsystem::GpuSceneDatabase,
                ),
                FunRendererFrameGraphNode::new(FunRendererFrameGraphStage::GpuSceneDatabase),
            ))
            .id();

        let scene_object = world
            .get::<FunRendererGpuSceneObject>(entity)
            .expect("spawned entity should carry renderer scene object component");
        assert!(scene_object.object_id.is_valid());
        assert_eq!(
            scene_object.subsystem,
            FunRendererSubsystem::GpuSceneDatabase
        );

        let node = world
            .get::<FunRendererFrameGraphNode>(entity)
            .expect("spawned entity should carry frame graph node component");
        assert_eq!(
            node.order_key,
            FunRendererFrameGraphStage::GpuSceneDatabase.order_key()
        );

        let policy = world.resource::<FunRendererEcsSchedulePolicy>();
        assert!(policy.extraction_uses_bevy_ecs);
        assert!(policy.gpu_scene_database_uses_components);
        assert!(policy.frame_graph_uses_schedule_sets);
    }
}
