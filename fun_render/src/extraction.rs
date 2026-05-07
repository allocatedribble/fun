use bevy::{
    ecs::lifecycle::RemovedComponents,
    prelude::{Added, Changed, Component, Entity, Or, Query, ResMut, Resource, Transform, Without},
};
use fun_renderer::{
    GpuSceneDatabase, GpuSceneDatabaseDiagnostics, GpuSceneUploadReport, GpuTransform,
    StableInstanceId,
    fun_scene::{LuxLight, Renderable, SceneStableIdentity, VirtualGeometryAuthoring},
};

use crate::FunRendererUploadArena;

pub const FUN_RENDER_SCENE_EXTRACTION_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct FunRenderSceneExtractionBridge {
    entity_stable_ids: Vec<(Entity, StableInstanceId)>,
    next_synthetic_local: u64,
}

impl FunRenderSceneExtractionBridge {
    #[must_use]
    pub fn stable_id_for(
        &mut self,
        entity: Entity,
        scene_identity: Option<&SceneStableIdentity>,
    ) -> StableInstanceId {
        if let Some(identity) = scene_identity
            && identity.is_valid()
        {
            let stable = StableInstanceId::from_scene_identity(*identity);
            self.upsert_entity_mapping(entity, stable);
            return stable;
        }

        if let Some((_, stable)) = self
            .entity_stable_ids
            .iter()
            .find(|(candidate, _)| *candidate == entity)
        {
            return *stable;
        }

        self.next_synthetic_local = self.next_synthetic_local.saturating_add(1);
        let stable = StableInstanceId::synthetic(self.next_synthetic_local);
        self.entity_stable_ids.push((entity, stable));
        stable
    }

    pub fn remove_entity_mapping(&mut self, entity: Entity) -> Option<StableInstanceId> {
        let index = self
            .entity_stable_ids
            .iter()
            .position(|(candidate, _)| *candidate == entity)?;
        Some(self.entity_stable_ids.swap_remove(index).1)
    }

    #[must_use]
    pub fn tracked_entity_count(&self) -> usize {
        self.entity_stable_ids.len()
    }

    fn upsert_entity_mapping(&mut self, entity: Entity, stable: StableInstanceId) {
        if let Some((_, existing)) = self
            .entity_stable_ids
            .iter_mut()
            .find(|(candidate, _)| *candidate == entity)
        {
            *existing = stable;
        } else {
            self.entity_stable_ids.push((entity, stable));
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunRenderSceneExtractionReport {
    pub schema_version: u16,
    pub renderable_records: u32,
    pub light_records: u32,
    pub removed_renderables: u32,
    pub upload_bytes: u64,
    pub upload_record_count: u32,
    pub renderer_owned_records_ready: bool,
    pub database: GpuSceneDatabaseDiagnostics,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct ExtractedToRendererScene;

#[allow(clippy::type_complexity)]
pub fn extract_fun_scene_renderables_to_renderer_database(
    query: Query<
        (
            Entity,
            &Renderable,
            &Transform,
            Option<&SceneStableIdentity>,
            Option<&VirtualGeometryAuthoring>,
        ),
        Or<(
            Added<Renderable>,
            Changed<Renderable>,
            Changed<Transform>,
            Changed<VirtualGeometryAuthoring>,
        )>,
    >,
    mut removed: RemovedComponents<Renderable>,
    mut bridge: ResMut<FunRenderSceneExtractionBridge>,
    mut database: ResMut<GpuSceneDatabase>,
    mut upload_arena: ResMut<FunRendererUploadArena>,
    mut report: ResMut<FunRenderSceneExtractionReport>,
) {
    let mut renderable_records = 0u32;
    for (entity, renderable, transform, scene_identity, virtual_geometry) in query.iter() {
        let stable = bridge.stable_id_for(entity, scene_identity);
        database.upsert_renderable_from_fun_scene(
            stable,
            renderable,
            GpuTransform::from(transform),
            virtual_geometry,
        );
        renderable_records = renderable_records.saturating_add(1);
    }

    let mut removed_renderables = 0u32;
    for entity in removed.read() {
        if let Some(stable) = bridge.remove_entity_mapping(entity)
            && database.remove_instance_by_stable(stable)
        {
            removed_renderables = removed_renderables.saturating_add(1);
        }
    }

    let upload = database.upload_dirty_records(&mut upload_arena.diagnostics);
    upload_arena.bytes_reserved = upload_arena
        .bytes_reserved
        .saturating_add(upload.upload_bytes);
    upload_arena.bytes_written = upload_arena
        .bytes_written
        .saturating_add(upload.upload_bytes);

    report.schema_version = FUN_RENDER_SCENE_EXTRACTION_SCHEMA_VERSION;
    report.renderable_records = report.renderable_records.saturating_add(renderable_records);
    report.removed_renderables = report
        .removed_renderables
        .saturating_add(removed_renderables);
    merge_upload_report(&mut report, upload);
    report.database = database.diagnostics();
    report.renderer_owned_records_ready = report.database.instance_count != 0
        || report.database.light_count != 0
        || removed_renderables != 0;
}

#[allow(clippy::type_complexity)]
pub fn extract_fun_scene_lights_to_renderer_database(
    query: Query<
        (
            Entity,
            &LuxLight,
            Option<&Transform>,
            Option<&SceneStableIdentity>,
        ),
        (
            Or<(Added<LuxLight>, Changed<LuxLight>, Changed<Transform>)>,
            Without<Renderable>,
        ),
    >,
    mut bridge: ResMut<FunRenderSceneExtractionBridge>,
    mut database: ResMut<GpuSceneDatabase>,
    mut upload_arena: ResMut<FunRendererUploadArena>,
    mut report: ResMut<FunRenderSceneExtractionReport>,
) {
    let mut light_records = 0u32;
    for (entity, light, transform, scene_identity) in query.iter() {
        let stable = bridge.stable_id_for(entity, scene_identity);
        let transform = transform.map_or(GpuTransform::IDENTITY, GpuTransform::from);
        database.upsert_light_from_fun_scene(stable, light, transform);
        light_records = light_records.saturating_add(1);
    }

    let upload = database.upload_dirty_records(&mut upload_arena.diagnostics);
    upload_arena.bytes_reserved = upload_arena
        .bytes_reserved
        .saturating_add(upload.upload_bytes);
    upload_arena.bytes_written = upload_arena
        .bytes_written
        .saturating_add(upload.upload_bytes);

    report.schema_version = FUN_RENDER_SCENE_EXTRACTION_SCHEMA_VERSION;
    report.light_records = report.light_records.saturating_add(light_records);
    merge_upload_report(&mut report, upload);
    report.database = database.diagnostics();
    report.renderer_owned_records_ready = report.database.instance_count != 0
        || report.database.light_count != 0
        || light_records != 0;
}

fn merge_upload_report(report: &mut FunRenderSceneExtractionReport, upload: GpuSceneUploadReport) {
    report.upload_bytes = report.upload_bytes.saturating_add(upload.upload_bytes);
    report.upload_record_count = report
        .upload_record_count
        .saturating_add(upload.upload_record_count);
}

#[cfg(test)]
mod tests {
    use bevy::prelude::{Schedule, World};
    use fun_renderer::fun_scene::{GeometryRef, MaterialRef, RenderableFlags, VirtualGeometryMode};
    use fun_renderer::{GpuInstanceFlags, GpuLightType};
    use thunder::prelude::NetEntity;

    use super::*;

    fn install_extraction_resources(world: &mut World) {
        world.insert_resource(FunRenderSceneExtractionBridge::default());
        world.insert_resource(FunRenderSceneExtractionReport::default());
        world.insert_resource(GpuSceneDatabase::default());
        world.insert_resource(FunRendererUploadArena::default());
    }

    #[test]
    fn extraction_bridges_fun_scene_renderables_into_renderer_database() {
        let mut world = World::new();
        install_extraction_resources(&mut world);
        world.spawn((
            SceneStableIdentity::new(NetEntity::from_parts(1, 44)),
            Renderable::new(
                GeometryRef::new(3),
                MaterialRef::new(9),
                RenderableFlags::STATIC_WORLD,
            ),
            VirtualGeometryAuthoring {
                mode: VirtualGeometryMode::StaticClusterPages,
                ..Default::default()
            },
            Transform::from_xyz(1.0, 2.0, 3.0),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_fun_scene_renderables_to_renderer_database);
        schedule.run(&mut world);

        let report = world.resource::<FunRenderSceneExtractionReport>();
        assert_eq!(report.renderable_records, 1);
        assert!(report.renderer_owned_records_ready);
        assert!(report.upload_bytes > 0);
        assert_eq!(report.database.instance_count, 1);
        assert_eq!(report.database.mesh_count, 1);
        assert_eq!(report.database.material_count, 1);
        assert_eq!(report.database.page_metadata_count, 1);

        let database = world.resource::<GpuSceneDatabase>();
        let stable = StableInstanceId::from_scene_identity(SceneStableIdentity::new(
            NetEntity::from_parts(1, 44),
        ));
        let handle = database
            .instance_handle_for_stable(stable)
            .expect("stable instance should map to renderer handle");
        let instance = database.instance(handle).expect("record should exist");
        assert!(instance.flags.contains(GpuInstanceFlags::VIRTUAL_GEOMETRY));
        assert!(world.resource::<FunRendererUploadArena>().bytes_written > 0);
    }

    #[test]
    fn extraction_updates_previous_transform_for_existing_stable_instance() {
        let mut world = World::new();
        install_extraction_resources(&mut world);
        let entity = world
            .spawn((
                SceneStableIdentity::new(NetEntity::from_parts(1, 55)),
                Renderable::new(
                    GeometryRef::new(4),
                    MaterialRef::new(8),
                    RenderableFlags::DYNAMIC,
                ),
                Transform::from_xyz(0.0, 0.0, 0.0),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_fun_scene_renderables_to_renderer_database);
        schedule.run(&mut world);
        world
            .entity_mut(entity)
            .get_mut::<Transform>()
            .expect("transform")
            .translation
            .x = 10.0;
        schedule.run(&mut world);

        let database = world.resource::<GpuSceneDatabase>();
        let stable = StableInstanceId::from_scene_identity(SceneStableIdentity::new(
            NetEntity::from_parts(1, 55),
        ));
        let handle = database.instance_handle_for_stable(stable).expect("handle");
        let instance = database.instance(handle).expect("instance");
        assert_eq!(instance.previous_transform.translation.x, 0.0);
        assert_eq!(instance.transform.translation.x, 10.0);
        assert_eq!(
            world
                .resource::<FunRenderSceneExtractionReport>()
                .renderable_records,
            2
        );
    }

    #[test]
    fn extraction_removes_renderer_record_when_bevy_entity_despawns() {
        let mut world = World::new();
        install_extraction_resources(&mut world);
        let entity = world
            .spawn((
                SceneStableIdentity::new(NetEntity::from_parts(1, 66)),
                Renderable::new(
                    GeometryRef::new(1),
                    MaterialRef::new(1),
                    RenderableFlags::STATIC,
                ),
                Transform::IDENTITY,
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_fun_scene_renderables_to_renderer_database);
        schedule.run(&mut world);
        assert_eq!(
            world
                .resource::<GpuSceneDatabase>()
                .diagnostics()
                .instance_count,
            1
        );

        let _ = world.despawn(entity);
        schedule.run(&mut world);

        let report = world.resource::<FunRenderSceneExtractionReport>();
        assert_eq!(report.removed_renderables, 1);
        assert_eq!(report.database.instance_count, 0);
        assert_eq!(
            world
                .resource::<FunRenderSceneExtractionBridge>()
                .tracked_entity_count(),
            0
        );
    }

    #[test]
    fn extraction_bridges_fun_scene_lights_into_renderer_database() {
        let mut world = World::new();
        install_extraction_resources(&mut world);
        world.spawn((
            SceneStableIdentity::new(NetEntity::from_parts(2, 77)),
            LuxLight::directional(80_000.0),
            Transform::from_xyz(0.0, 10.0, 0.0),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_fun_scene_lights_to_renderer_database);
        schedule.run(&mut world);

        let report = world.resource::<FunRenderSceneExtractionReport>();
        assert_eq!(report.light_records, 1);
        assert_eq!(report.database.light_count, 1);
        assert!(report.upload_bytes > 0);
        let database = world.resource::<GpuSceneDatabase>();
        let stable = StableInstanceId::from_scene_identity(SceneStableIdentity::new(
            NetEntity::from_parts(2, 77),
        ));
        let handle = database
            .light_handle_for_stable(stable)
            .expect("stable light should map to renderer handle");
        let record = database.light(handle).expect("light should be addressable");
        assert_eq!(record.stable_id, stable);
        assert_eq!(record.light_type, GpuLightType::Directional);
    }
}
