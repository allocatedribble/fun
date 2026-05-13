use std::{
    any::{Any, TypeId, type_name},
    collections::{BTreeMap, HashMap},
};

use crate::{
    EcsCrossDomainHandoffQueues, EcsDecodedPageQueue, EcsDerivedArtifactRegistry,
    EcsDirtyRegionLedger, EcsLuxHandoffQueue, EcsPageResidencyMap, EcsPageResidencyTable,
    EcsPhysicsCookQueue, EcsProceduralWorldManifest, EcsRendererHandoffQueue,
    EcsSourceAcquireQueue, EcsStreamInterestTable, EcsStreamRequestQueue, FunEntity,
    FunEntityGeneration, FunWorldRevision, RevisionCategory, WorldRevisionLedger,
};

pub const FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT: u16 = 14;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunWorldId(pub u64);

impl FunWorldId {
    pub const ROOT: Self = Self(1);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunWorldMode {
    #[default]
    FunNative = 0,
}

impl FunWorldMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FunNative => "fun_native",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunWorldStorageBackend {
    #[default]
    FunNative = 0,
}

impl FunWorldStorageBackend {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FunNative => "fun_native",
        }
    }

    #[must_use]
    pub const fn is_fun_native(self) -> bool {
        matches!(self, Self::FunNative)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunWorldSchedulerAuthority {
    #[default]
    FunScheduler = 0,
}

impl FunWorldSchedulerAuthority {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FunScheduler => "fun_scheduler",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunWorldDiagnostics {
    pub initialized_spatial_resources: u16,
    pub native_resources_registered: u16,
    pub storage_backend: FunWorldStorageBackend,
    pub scheduler_authority: FunWorldSchedulerAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunWorldBuilder {
    pub id: FunWorldId,
    pub mode: FunWorldMode,
    pub storage_backend: FunWorldStorageBackend,
}

impl Default for FunWorldBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FunWorldBuilder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            id: FunWorldId::ROOT,
            mode: FunWorldMode::FunNative,
            storage_backend: FunWorldStorageBackend::FunNative,
        }
    }

    #[must_use]
    pub const fn with_id(mut self, id: FunWorldId) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub const fn with_mode(mut self, mode: FunWorldMode) -> Self {
        self.mode = mode;
        self
    }

    #[must_use]
    pub const fn with_storage_backend(mut self, storage_backend: FunWorldStorageBackend) -> Self {
        self.storage_backend = storage_backend;
        self
    }

    #[must_use]
    pub const fn fun_native(mut self) -> Self {
        self.mode = FunWorldMode::FunNative;
        self.storage_backend = FunWorldStorageBackend::FunNative;
        self
    }

    #[must_use]
    pub fn build(self) -> FunWorld {
        FunWorld::with_parts(self.id, self.mode, self.storage_backend)
    }
}

#[derive(Default)]
struct FunEntityRecord {
    components: HashMap<TypeId, Box<dyn Any>>,
}

pub struct SpawnedEntity {
    entity: FunEntity,
}

impl SpawnedEntity {
    #[must_use]
    pub const fn id(&self) -> FunEntity {
        self.entity
    }
}

pub struct FunEntityMut<'world> {
    entity: FunEntity,
    record: &'world mut FunEntityRecord,
}

impl<'world> FunEntityMut<'world> {
    pub fn insert<T: 'static>(&mut self, component: T) -> &mut Self {
        self.record
            .components
            .insert(TypeId::of::<T>(), Box::new(component) as Box<dyn Any>);
        self
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.record
            .components
            .get_mut(&TypeId::of::<T>())?
            .downcast_mut::<T>()
    }

    pub fn remove<T: 'static>(&mut self) -> Option<T> {
        self.record
            .components
            .remove(&TypeId::of::<T>())
            .and_then(|component| component.downcast::<T>().ok())
            .map(|component| *component)
    }

    pub fn despawn(self) {
        let _ = self.entity;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunWorldQuery<T> {
    marker: std::marker::PhantomData<T>,
}

impl<T> FunWorldQuery<T> {
    pub fn iter<'world>(&'world mut self, _world: &'world FunWorld) -> std::iter::Empty<T> {
        std::iter::empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunEntityLookupError {
    pub entity: FunEntity,
}

pub struct FunWorld {
    pub id: FunWorldId,
    pub revision: FunWorldRevision,
    pub revision_ledger: WorldRevisionLedger,
    pub mode: FunWorldMode,
    pub storage_backend: FunWorldStorageBackend,
    pub diagnostics: FunWorldDiagnostics,
    pub procedural_world_manifest: EcsProceduralWorldManifest,
    pub spatial_page_table: EcsPageResidencyTable,
    pub residency_table: EcsPageResidencyMap,
    pub dirty_ledger: EcsDirtyRegionLedger,
    pub stream_interest_table: EcsStreamInterestTable,
    pub stream_request_queue: EcsStreamRequestQueue,
    pub source_acquire_queue: EcsSourceAcquireQueue,
    pub decoded_page_queue: EcsDecodedPageQueue,
    pub derived_artifact_registry: EcsDerivedArtifactRegistry,
    pub cross_domain_handoff_queues: EcsCrossDomainHandoffQueues,
    pub renderer_handoff_queue: EcsRendererHandoffQueue,
    pub lux_handoff_queue: EcsLuxHandoffQueue,
    pub physics_cook_queue: EcsPhysicsCookQueue,
    resources: HashMap<TypeId, Box<dyn Any>>,
    entities: BTreeMap<FunEntity, FunEntityRecord>,
    next_entity_slot: u64,
}

impl Default for FunWorld {
    fn default() -> Self {
        Self::fun_native(FunWorldId::ROOT)
    }
}

impl FunWorld {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn fun_native(id: FunWorldId) -> Self {
        Self::with_parts(
            id,
            FunWorldMode::FunNative,
            FunWorldStorageBackend::FunNative,
        )
    }

    #[must_use]
    pub fn with_parts(
        id: FunWorldId,
        mode: FunWorldMode,
        storage_backend: FunWorldStorageBackend,
    ) -> Self {
        let mut world = Self {
            id,
            revision: FunWorldRevision::default(),
            revision_ledger: WorldRevisionLedger::default(),
            mode,
            storage_backend,
            diagnostics: FunWorldDiagnostics {
                storage_backend,
                scheduler_authority: FunWorldSchedulerAuthority::FunScheduler,
                ..FunWorldDiagnostics::default()
            },
            procedural_world_manifest: EcsProceduralWorldManifest::default(),
            spatial_page_table: EcsPageResidencyTable::default(),
            residency_table: EcsPageResidencyMap::default(),
            dirty_ledger: EcsDirtyRegionLedger::default(),
            stream_interest_table: EcsStreamInterestTable::default(),
            stream_request_queue: EcsStreamRequestQueue::default(),
            source_acquire_queue: EcsSourceAcquireQueue::default(),
            decoded_page_queue: EcsDecodedPageQueue::default(),
            derived_artifact_registry: EcsDerivedArtifactRegistry::default(),
            cross_domain_handoff_queues: EcsCrossDomainHandoffQueues::default(),
            renderer_handoff_queue: EcsRendererHandoffQueue::default(),
            lux_handoff_queue: EcsLuxHandoffQueue::default(),
            physics_cook_queue: EcsPhysicsCookQueue::default(),
            resources: HashMap::new(),
            entities: BTreeMap::new(),
            next_entity_slot: 1,
        };
        world.initialize_spatial_resources();
        world
    }

    pub fn initialize_spatial_resources(&mut self) {
        self.procedural_world_manifest = EcsProceduralWorldManifest::default();
        self.spatial_page_table = EcsPageResidencyTable::default();
        self.residency_table = EcsPageResidencyMap::default();
        self.dirty_ledger = EcsDirtyRegionLedger::default();
        self.stream_interest_table = EcsStreamInterestTable::default();
        self.stream_request_queue = EcsStreamRequestQueue::default();
        self.source_acquire_queue = EcsSourceAcquireQueue::default();
        self.decoded_page_queue = EcsDecodedPageQueue::default();
        self.derived_artifact_registry = EcsDerivedArtifactRegistry::default();
        self.cross_domain_handoff_queues = EcsCrossDomainHandoffQueues::default();
        self.renderer_handoff_queue = EcsRendererHandoffQueue::default();
        self.lux_handoff_queue = EcsLuxHandoffQueue::default();
        self.physics_cook_queue = EcsPhysicsCookQueue::default();
        self.diagnostics.initialized_spatial_resources =
            FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT;
        self.diagnostics.native_resources_registered = FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT;
        self.register_spatial_resources();
        let (_previous, new) = self
            .revision_ledger
            .advance_category(RevisionCategory::Structure);
        self.revision = new;
    }

    #[must_use]
    pub const fn scheduler_authority(&self) -> FunWorldSchedulerAuthority {
        self.diagnostics.scheduler_authority
    }

    #[must_use]
    pub fn contains_resource<T: 'static>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<T>())
    }

    pub fn insert_resource<T: 'static>(&mut self, resource: T) -> &mut Self {
        self.resources
            .insert(TypeId::of::<T>(), Box::new(resource) as Box<dyn Any>);
        self
    }

    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources
            .remove(&TypeId::of::<T>())
            .and_then(|resource| resource.downcast::<T>().ok())
            .map(|resource| *resource)
    }

    pub fn init_resource<T: Default + 'static>(&mut self) -> &mut Self {
        if !self.contains_resource::<T>() {
            self.insert_resource(T::default());
        }
        self
    }

    #[must_use]
    pub fn resource<T: 'static>(&self) -> &T {
        self.resources
            .get(&TypeId::of::<T>())
            .and_then(|resource| resource.downcast_ref::<T>())
            .unwrap_or_else(|| panic!("missing FUN ECS resource {}", type_name::<T>()))
    }

    pub fn resource_mut<T: 'static>(&mut self) -> &mut T {
        self.resources
            .get_mut(&TypeId::of::<T>())
            .and_then(|resource| resource.downcast_mut::<T>())
            .unwrap_or_else(|| panic!("missing FUN ECS resource {}", type_name::<T>()))
    }

    pub fn spawn<T: 'static>(&mut self, bundle: T) -> SpawnedEntity {
        let entity = FunEntity::new(self.next_entity_slot, FunEntityGeneration::new(1));
        self.next_entity_slot = self.next_entity_slot.saturating_add(1);

        let mut record = FunEntityRecord::default();
        record
            .components
            .insert(TypeId::of::<T>(), Box::new(bundle) as Box<dyn Any>);
        self.entities.insert(entity, record);
        SpawnedEntity { entity }
    }

    pub fn despawn(&mut self, entity: FunEntity) -> bool {
        self.entities.remove(&entity).is_some()
    }

    pub fn entity_mut(&mut self, entity: FunEntity) -> FunEntityMut<'_> {
        let record = self.entities.entry(entity).or_default();
        FunEntityMut { entity, record }
    }

    pub fn get_mut<T: 'static>(&mut self, entity: FunEntity) -> Option<&mut T> {
        self.entities
            .get_mut(&entity)?
            .components
            .get_mut(&TypeId::of::<T>())?
            .downcast_mut::<T>()
    }

    pub fn get_entity(&self, entity: FunEntity) -> Result<FunEntity, FunEntityLookupError> {
        self.entities
            .contains_key(&entity)
            .then_some(entity)
            .ok_or(FunEntityLookupError { entity })
    }

    #[must_use]
    pub fn query<T>(&self) -> FunWorldQuery<T> {
        FunWorldQuery {
            marker: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn get<T: 'static>(&self, entity: FunEntity) -> Option<&T> {
        self.entities
            .get(&entity)?
            .components
            .get(&TypeId::of::<T>())?
            .downcast_ref::<T>()
    }

    pub fn set_procedural_world_manifest(
        &mut self,
        manifest: EcsProceduralWorldManifest,
    ) -> Result<(), crate::EcsSpatialValidationError> {
        manifest.validate()?;
        self.procedural_world_manifest = manifest;
        self.insert_resource(manifest);
        let (_previous, new) = self
            .revision_ledger
            .advance_category(RevisionCategory::Structure);
        self.revision = new;
        Ok(())
    }

    fn register_spatial_resources(&mut self) {
        self.insert_resource(self.procedural_world_manifest);
        self.insert_resource(self.spatial_page_table.clone());
        self.insert_resource(self.residency_table.clone());
        self.insert_resource(self.dirty_ledger.clone());
        self.insert_resource(self.stream_interest_table.clone());
        self.insert_resource(self.stream_request_queue.clone());
        self.insert_resource(self.source_acquire_queue.clone());
        self.insert_resource(self.decoded_page_queue.clone());
        self.insert_resource(self.derived_artifact_registry.clone());
        self.insert_resource(self.cross_domain_handoff_queues.clone());
        self.insert_resource(self.renderer_handoff_queue.clone());
        self.insert_resource(self.lux_handoff_queue.clone());
        self.insert_resource(self.physics_cook_queue.clone());
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        EcsCrossDomainHandoffQueues, EcsDecodedPageQueue, EcsDerivedArtifactRegistry,
        EcsDirtyRegionLedger, EcsLuxHandoffQueue, EcsPageResidencyMap, EcsPageResidencyTable,
        EcsPhysicsCookQueue, EcsProceduralWorldManifest, EcsRendererHandoffQueue,
        EcsSourceAcquireQueue, EcsStreamInterestTable, EcsStreamRequestQueue,
    };

    use super::*;

    #[test]
    fn native_fun_world_initializes_spatial_resource_tables() {
        let world = FunWorld::default();

        assert_eq!(world.mode, FunWorldMode::FunNative);
        assert_eq!(world.storage_backend, FunWorldStorageBackend::FunNative);
        assert_eq!(
            world.diagnostics.initialized_spatial_resources,
            FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT
        );
        assert_eq!(
            world.scheduler_authority(),
            FunWorldSchedulerAuthority::FunScheduler
        );
        assert_eq!(
            world.procedural_world_manifest,
            EcsProceduralWorldManifest::default()
        );
        assert!(world.spatial_page_table.is_empty());
        assert!(world.residency_table.is_consistent());
        assert!(world.dirty_ledger.regions.is_empty());
        assert!(world.stream_interest_table.interests.is_empty());
        assert!(world.stream_request_queue.requests.is_empty());
        assert!(world.source_acquire_queue.rows.is_empty());
        assert!(world.decoded_page_queue.rows.is_empty());
        assert!(world.derived_artifact_registry.is_empty());
        assert!(world.cross_domain_handoff_queues.rows.is_empty());
    }

    #[test]
    fn native_fun_world_registers_spatial_resources_in_fun_storage() {
        let world = FunWorld::fun_native(FunWorldId::new(12));

        assert_eq!(
            world.diagnostics.native_resources_registered,
            FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT
        );
        assert!(world.contains_resource::<EcsProceduralWorldManifest>());
        assert!(world.contains_resource::<EcsPageResidencyTable>());
        assert!(world.contains_resource::<EcsPageResidencyMap>());
        assert!(world.contains_resource::<EcsDirtyRegionLedger>());
        assert!(world.contains_resource::<EcsStreamInterestTable>());
        assert!(world.contains_resource::<EcsStreamRequestQueue>());
        assert!(world.contains_resource::<EcsSourceAcquireQueue>());
        assert!(world.contains_resource::<EcsDecodedPageQueue>());
        assert!(world.contains_resource::<EcsDerivedArtifactRegistry>());
        assert!(world.contains_resource::<EcsCrossDomainHandoffQueues>());
        assert!(world.contains_resource::<EcsRendererHandoffQueue>());
        assert!(world.contains_resource::<EcsLuxHandoffQueue>());
        assert!(world.contains_resource::<EcsPhysicsCookQueue>());
    }

    #[test]
    fn native_fun_world_can_store_resources_and_spawn_entities() {
        #[derive(Debug, Default, PartialEq, Eq)]
        struct TestResource(u32);
        #[derive(Debug, PartialEq, Eq)]
        struct TestComponent(u32);

        let mut world = FunWorld::default();
        world.insert_resource(TestResource(7));
        let entity = world.spawn(TestComponent(11)).id();

        assert_eq!(world.resource::<TestResource>().0, 7);
        assert_eq!(world.get::<TestComponent>(entity), Some(&TestComponent(11)));
    }
}
