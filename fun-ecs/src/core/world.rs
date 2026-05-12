use bevy_ecs::world::World;

use crate::{
    EcsCrossDomainHandoffQueues, EcsDecodedPageQueue, EcsDerivedArtifactRegistry,
    EcsDirtyRegionLedger, EcsLuxHandoffQueue, EcsPageResidencyMap, EcsPageResidencyTable,
    EcsPhysicsCookQueue, EcsRendererHandoffQueue, EcsSourceAcquireQueue, EcsStreamInterestTable,
    EcsStreamRequestQueue, FunWorldRevision, RevisionCategory, WorldRevisionLedger,
};

pub const FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT: u16 = 12;

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
    BevyCompatibility = 0,
    FunNative = 1,
    #[default]
    Hybrid = 2,
}

impl FunWorldMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BevyCompatibility => "bevy_compatibility",
            Self::FunNative => "fun_native",
            Self::Hybrid => "hybrid",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunWorldStorageBackend {
    BevyWorld = 0,
    FunNative = 1,
    #[default]
    Hybrid = 2,
}

impl FunWorldStorageBackend {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BevyWorld => "bevy_world",
            Self::FunNative => "fun_native",
            Self::Hybrid => "hybrid",
        }
    }

    #[must_use]
    pub const fn hosts_bevy_world(self) -> bool {
        matches!(self, Self::BevyWorld | Self::Hybrid)
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
    pub bevy_resources_mirrored: u16,
    pub storage_backend: FunWorldStorageBackend,
    pub scheduler_authority: FunWorldSchedulerAuthority,
}

pub struct FunWorld {
    pub id: FunWorldId,
    pub revision: FunWorldRevision,
    pub revision_ledger: WorldRevisionLedger,
    pub mode: FunWorldMode,
    pub storage_backend: FunWorldStorageBackend,
    pub diagnostics: FunWorldDiagnostics,
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
    bevy_world: Option<World>,
}

impl Default for FunWorld {
    fn default() -> Self {
        Self::hybrid(FunWorldId::ROOT)
    }
}

impl FunWorld {
    #[must_use]
    pub fn hybrid(id: FunWorldId) -> Self {
        Self::new(id, FunWorldMode::Hybrid, FunWorldStorageBackend::Hybrid)
    }

    #[must_use]
    pub fn bevy_compatibility(id: FunWorldId) -> Self {
        Self::new(
            id,
            FunWorldMode::BevyCompatibility,
            FunWorldStorageBackend::BevyWorld,
        )
    }

    #[must_use]
    pub fn fun_native(id: FunWorldId) -> Self {
        Self::new(
            id,
            FunWorldMode::FunNative,
            FunWorldStorageBackend::FunNative,
        )
    }

    #[must_use]
    pub fn new(
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
            bevy_world: storage_backend.hosts_bevy_world().then(World::new),
        };
        world.initialize_spatial_resources();
        world
    }

    pub fn initialize_spatial_resources(&mut self) {
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
        self.diagnostics.bevy_resources_mirrored = 0;
        if let Some(bevy_world) = &mut self.bevy_world {
            insert_spatial_resources_into_bevy(
                bevy_world,
                &self.spatial_page_table,
                &self.residency_table,
                &self.dirty_ledger,
                &self.stream_interest_table,
                &self.stream_request_queue,
                &self.source_acquire_queue,
                &self.decoded_page_queue,
                &self.derived_artifact_registry,
                &self.cross_domain_handoff_queues,
                &self.renderer_handoff_queue,
                &self.lux_handoff_queue,
                &self.physics_cook_queue,
            );
            self.diagnostics.bevy_resources_mirrored = FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT;
        }
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
    pub fn bevy_world(&self) -> Option<&World> {
        self.bevy_world.as_ref()
    }

    #[must_use]
    pub fn bevy_world_mut(&mut self) -> Option<&mut World> {
        self.bevy_world.as_mut()
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "mirrors the explicit acceptance resource set and keeps initialization grep-able"
)]
fn insert_spatial_resources_into_bevy(
    bevy_world: &mut World,
    spatial_page_table: &EcsPageResidencyTable,
    residency_table: &EcsPageResidencyMap,
    dirty_ledger: &EcsDirtyRegionLedger,
    stream_interest_table: &EcsStreamInterestTable,
    stream_request_queue: &EcsStreamRequestQueue,
    source_acquire_queue: &EcsSourceAcquireQueue,
    decoded_page_queue: &EcsDecodedPageQueue,
    derived_artifact_registry: &EcsDerivedArtifactRegistry,
    cross_domain_handoff_queues: &EcsCrossDomainHandoffQueues,
    renderer_handoff_queue: &EcsRendererHandoffQueue,
    lux_handoff_queue: &EcsLuxHandoffQueue,
    physics_cook_queue: &EcsPhysicsCookQueue,
) {
    bevy_world.insert_resource(spatial_page_table.clone());
    bevy_world.insert_resource(residency_table.clone());
    bevy_world.insert_resource(dirty_ledger.clone());
    bevy_world.insert_resource(stream_interest_table.clone());
    bevy_world.insert_resource(stream_request_queue.clone());
    bevy_world.insert_resource(source_acquire_queue.clone());
    bevy_world.insert_resource(decoded_page_queue.clone());
    bevy_world.insert_resource(derived_artifact_registry.clone());
    bevy_world.insert_resource(cross_domain_handoff_queues.clone());
    bevy_world.insert_resource(renderer_handoff_queue.clone());
    bevy_world.insert_resource(lux_handoff_queue.clone());
    bevy_world.insert_resource(physics_cook_queue.clone());
}

#[cfg(test)]
mod tests {
    use crate::{
        EcsCrossDomainHandoffQueues, EcsDecodedPageQueue, EcsDerivedArtifactRegistry,
        EcsDirtyRegionLedger, EcsLuxHandoffQueue, EcsPageResidencyMap, EcsPageResidencyTable,
        EcsPhysicsCookQueue, EcsRendererHandoffQueue, EcsSourceAcquireQueue,
        EcsStreamInterestTable, EcsStreamRequestQueue,
    };

    use super::*;

    #[test]
    fn hybrid_fun_world_initializes_spatial_resource_tables() {
        let world = FunWorld::default();

        assert_eq!(world.mode, FunWorldMode::Hybrid);
        assert_eq!(world.storage_backend, FunWorldStorageBackend::Hybrid);
        assert_eq!(
            world.diagnostics.initialized_spatial_resources,
            FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT
        );
        assert_eq!(
            world.scheduler_authority(),
            FunWorldSchedulerAuthority::FunScheduler
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
    fn hybrid_fun_world_hosts_bevy_for_compatibility_not_scheduler_authority() {
        let world = FunWorld::hybrid(FunWorldId::new(12));
        let bevy_world = world.bevy_world().expect("hybrid hosts Bevy world");

        assert_eq!(
            world.scheduler_authority(),
            FunWorldSchedulerAuthority::FunScheduler
        );
        assert_eq!(
            world.diagnostics.bevy_resources_mirrored,
            FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT
        );
        assert!(bevy_world.contains_resource::<EcsPageResidencyTable>());
        assert!(bevy_world.contains_resource::<EcsPageResidencyMap>());
        assert!(bevy_world.contains_resource::<EcsDirtyRegionLedger>());
        assert!(bevy_world.contains_resource::<EcsStreamInterestTable>());
        assert!(bevy_world.contains_resource::<EcsStreamRequestQueue>());
        assert!(bevy_world.contains_resource::<EcsSourceAcquireQueue>());
        assert!(bevy_world.contains_resource::<EcsDecodedPageQueue>());
        assert!(bevy_world.contains_resource::<EcsDerivedArtifactRegistry>());
        assert!(bevy_world.contains_resource::<EcsCrossDomainHandoffQueues>());
        assert!(bevy_world.contains_resource::<EcsRendererHandoffQueue>());
        assert!(bevy_world.contains_resource::<EcsLuxHandoffQueue>());
        assert!(bevy_world.contains_resource::<EcsPhysicsCookQueue>());
    }

    #[test]
    fn native_fun_world_keeps_bevy_absent_but_resource_tables_ready() {
        let world = FunWorld::fun_native(FunWorldId::new(44));

        assert_eq!(world.storage_backend, FunWorldStorageBackend::FunNative);
        assert!(world.bevy_world().is_none());
        assert_eq!(
            world.diagnostics.initialized_spatial_resources,
            FUN_WORLD_INITIALIZED_SPATIAL_RESOURCE_COUNT
        );
        assert_eq!(world.diagnostics.bevy_resources_mirrored, 0);
    }
}
