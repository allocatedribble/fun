use fun_scheduler_types::{
    EcsChunkKey, EcsSystemClass, EcsSystemExecutionContract, WorkRequiredness,
};

use crate::EcsHandoffQueueId;

pub const FUN_ECS_MAX_SYSTEM_DECLARATIONS: usize = 4_096;
pub const FUN_ECS_MAX_RESOURCE_CHUNKS: usize = 1_048_576;
pub const FUN_ECS_REQUIRED_HANDOFF_CONSUMER_COUNT: usize = 9;

pub const FUN_ECS_REQUIRED_HANDOFF_CONSUMERS: [FunEcsSubsystem;
    FUN_ECS_REQUIRED_HANDOFF_CONSUMER_COUNT] = [
    FunEcsSubsystem::Renderer,
    FunEcsSubsystem::Lux,
    FunEcsSubsystem::Avis,
    FunEcsSubsystem::Thunder,
    FunEcsSubsystem::Rvelte,
    FunEcsSubsystem::Animation,
    FunEcsSubsystem::Ai,
    FunEcsSubsystem::Warden,
    FunEcsSubsystem::Telemetry,
];

pub const FUN_ECS_SUBSYSTEM_HANDOFF_CONTRACTS: [FunEcsSubsystemHandoffContract;
    FUN_ECS_REQUIRED_HANDOFF_CONSUMER_COUNT] = [
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Renderer,
        FunEcsResourceKind::RendererHandoffQueue,
        EcsHandoffQueueId::new(1),
        WorkRequiredness::Required,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Lux,
        FunEcsResourceKind::LuxHandoffQueue,
        EcsHandoffQueueId::new(2),
        WorkRequiredness::Optional,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Avis,
        FunEcsResourceKind::PhysicsCookQueue,
        EcsHandoffQueueId::new(3),
        WorkRequiredness::Required,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Thunder,
        FunEcsResourceKind::NetworkHandoffQueue,
        EcsHandoffQueueId::new(4),
        WorkRequiredness::Required,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Rvelte,
        FunEcsResourceKind::RvelteUiPacketQueue,
        EcsHandoffQueueId::new(5),
        WorkRequiredness::Required,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Animation,
        FunEcsResourceKind::AnimationIntentQueue,
        EcsHandoffQueueId::new(6),
        WorkRequiredness::Required,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Ai,
        FunEcsResourceKind::AiIntentQueue,
        EcsHandoffQueueId::new(7),
        WorkRequiredness::Optional,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Warden,
        FunEcsResourceKind::WardenEvidenceQueue,
        EcsHandoffQueueId::new(8),
        WorkRequiredness::Required,
    ),
    FunEcsSubsystemHandoffContract::new(
        FunEcsSubsystem::FunEcs,
        FunEcsSubsystem::Telemetry,
        FunEcsResourceKind::TelemetryEventQueue,
        EcsHandoffQueueId::new(9),
        WorkRequiredness::Optional,
    ),
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunEcsWorldId(pub u64);

impl FunEcsWorldId {
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunEcsRevision(pub u64);

impl FunEcsRevision {
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunEcsSystemId(pub u32);

impl FunEcsSystemId {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunEcsSubsystem {
    #[default]
    FunEcs = 0,
    Renderer = 1,
    Lux = 2,
    Avis = 3,
    Thunder = 4,
    Rvelte = 5,
    Animation = 6,
    Ai = 7,
    Warden = 8,
    Telemetry = 9,
}

impl FunEcsSubsystem {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FunEcs => "fun_ecs",
            Self::Renderer => "renderer",
            Self::Lux => "lux",
            Self::Avis => "avis",
            Self::Thunder => "thunder",
            Self::Rvelte => "rvelte",
            Self::Animation => "animation",
            Self::Ai => "ai",
            Self::Warden => "warden",
            Self::Telemetry => "telemetry",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunEcsComponentKind {
    #[default]
    SpatialVolume = 0,
    StreamCamera = 1,
    StreamSource = 2,
    AuthoringTool = 3,
    DebugPin = 4,
    HighLevelWorldObject = 5,
    GameComponent = 6,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunEcsResourceKind {
    #[default]
    SpatialGridRegistry = 0,
    SpatialPageTable = 1,
    PageResidencyTable = 2,
    StreamInterestTable = 3,
    StreamRequestQueue = 4,
    SourceAcquireQueue = 5,
    DecodedPageQueue = 6,
    DerivedArtifactRegistry = 7,
    DirtyRegionLedger = 8,
    RendererHandoffQueue = 9,
    LuxHandoffQueue = 10,
    PhysicsCookQueue = 11,
    NetworkHandoffQueue = 12,
    RvelteUiPacketQueue = 13,
    AnimationIntentQueue = 14,
    AiIntentQueue = 15,
    WardenEvidenceQueue = 16,
    TelemetryEventQueue = 17,
    ArtifactManifest = 18,
    StreamWaveLedger = 19,
}

impl FunEcsResourceKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SpatialGridRegistry => "spatial_grid_registry",
            Self::SpatialPageTable => "spatial_page_table",
            Self::PageResidencyTable => "page_residency_table",
            Self::StreamInterestTable => "stream_interest_table",
            Self::StreamRequestQueue => "stream_request_queue",
            Self::SourceAcquireQueue => "source_acquire_queue",
            Self::DecodedPageQueue => "decoded_page_queue",
            Self::DerivedArtifactRegistry => "derived_artifact_registry",
            Self::DirtyRegionLedger => "dirty_region_ledger",
            Self::RendererHandoffQueue => "renderer_handoff_queue",
            Self::LuxHandoffQueue => "lux_handoff_queue",
            Self::PhysicsCookQueue => "physics_cook_queue",
            Self::NetworkHandoffQueue => "network_handoff_queue",
            Self::RvelteUiPacketQueue => "rvelte_ui_packet_queue",
            Self::AnimationIntentQueue => "animation_intent_queue",
            Self::AiIntentQueue => "ai_intent_queue",
            Self::WardenEvidenceQueue => "warden_evidence_queue",
            Self::TelemetryEventQueue => "telemetry_event_queue",
            Self::ArtifactManifest => "artifact_manifest",
            Self::StreamWaveLedger => "stream_wave_ledger",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunEcsAccessMode {
    #[default]
    Read = 0,
    Write = 1,
    AppendCommand = 2,
}

impl FunEcsAccessMode {
    #[must_use]
    pub const fn conflicts_with(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::Write, _) | (_, Self::Write) | (Self::AppendCommand, Self::AppendCommand)
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsSystemAccess {
    pub resource: FunEcsResourceKind,
    pub mode: FunEcsAccessMode,
}

impl FunEcsSystemAccess {
    #[must_use]
    pub const fn read(resource: FunEcsResourceKind) -> Self {
        Self {
            resource,
            mode: FunEcsAccessMode::Read,
        }
    }

    #[must_use]
    pub const fn write(resource: FunEcsResourceKind) -> Self {
        Self {
            resource,
            mode: FunEcsAccessMode::Write,
        }
    }

    #[must_use]
    pub const fn append_command(resource: FunEcsResourceKind) -> Self {
        Self {
            resource,
            mode: FunEcsAccessMode::AppendCommand,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunEcsSystemDeclaration {
    pub id: FunEcsSystemId,
    pub label: &'static str,
    pub subsystem: FunEcsSubsystem,
    pub class: EcsSystemClass,
    pub contract: EcsSystemExecutionContract,
    pub access: Vec<FunEcsSystemAccess>,
}

impl FunEcsSystemDeclaration {
    #[must_use]
    pub fn new(
        id: FunEcsSystemId,
        label: &'static str,
        subsystem: FunEcsSubsystem,
        class: EcsSystemClass,
        contract: EcsSystemExecutionContract,
        access: Vec<FunEcsSystemAccess>,
    ) -> Self {
        Self {
            id,
            label,
            subsystem,
            class,
            contract,
            access,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsResourceChunk {
    pub resource: FunEcsResourceKind,
    pub chunk_key: EcsChunkKey,
    pub first_row: u32,
    pub row_count: u32,
    pub revision: FunEcsRevision,
}

impl FunEcsResourceChunk {
    #[must_use]
    pub const fn new(
        resource: FunEcsResourceKind,
        chunk_key: EcsChunkKey,
        first_row: u32,
        row_count: u32,
        revision: FunEcsRevision,
    ) -> Self {
        Self {
            resource,
            chunk_key,
            first_row,
            row_count,
            revision,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.row_count != 0 && self.chunk_key.get() != 0
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunEcsControlPlane {
    pub world: FunEcsWorldId,
    pub revision: FunEcsRevision,
    pub systems: Vec<FunEcsSystemDeclaration>,
    pub resource_chunks: Vec<FunEcsResourceChunk>,
}

impl FunEcsControlPlane {
    #[must_use]
    pub const fn new(world: FunEcsWorldId) -> Self {
        Self {
            world,
            revision: FunEcsRevision(0),
            systems: Vec::new(),
            resource_chunks: Vec::new(),
        }
    }

    pub fn declare_system(
        &mut self,
        declaration: FunEcsSystemDeclaration,
    ) -> Result<(), FunEcsValidationError> {
        if declaration.label.is_empty() {
            return Err(FunEcsValidationError::EmptySystemLabel);
        }
        if self.systems.len() >= FUN_ECS_MAX_SYSTEM_DECLARATIONS {
            return Err(FunEcsValidationError::SystemDeclarationTableFull);
        }
        if self
            .systems
            .iter()
            .any(|system| system.id == declaration.id)
        {
            return Err(FunEcsValidationError::DuplicateSystemId);
        }
        self.systems.push(declaration);
        self.systems.sort_by_key(|system| system.id);
        self.revision = self.revision.next();
        Ok(())
    }

    #[must_use]
    pub fn system_access(&self, id: FunEcsSystemId) -> Option<&[FunEcsSystemAccess]> {
        self.systems
            .iter()
            .find(|system| system.id == id)
            .map(|system| system.access.as_slice())
    }

    pub fn upsert_resource_chunk(
        &mut self,
        chunk: FunEcsResourceChunk,
    ) -> Result<(), FunEcsValidationError> {
        if !chunk.is_valid() {
            return Err(FunEcsValidationError::InvalidResourceChunk);
        }
        if let Some(existing) = self.resource_chunks.iter_mut().find(|candidate| {
            candidate.resource == chunk.resource && candidate.chunk_key == chunk.chunk_key
        }) {
            *existing = chunk;
            self.revision = self.revision.next();
            return Ok(());
        }
        if self.resource_chunks.len() >= FUN_ECS_MAX_RESOURCE_CHUNKS {
            return Err(FunEcsValidationError::ResourceChunkTableFull);
        }
        self.resource_chunks.push(chunk);
        self.resource_chunks
            .sort_by_key(|chunk| (chunk.resource, chunk.chunk_key));
        self.revision = self.revision.next();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsSubsystemHandoffContract {
    pub producer: FunEcsSubsystem,
    pub consumer: FunEcsSubsystem,
    pub queue_resource: FunEcsResourceKind,
    pub queue: EcsHandoffQueueId,
    pub requiredness: WorkRequiredness,
}

impl FunEcsSubsystemHandoffContract {
    #[must_use]
    pub const fn new(
        producer: FunEcsSubsystem,
        consumer: FunEcsSubsystem,
        queue_resource: FunEcsResourceKind,
        queue: EcsHandoffQueueId,
        requiredness: WorkRequiredness,
    ) -> Self {
        Self {
            producer,
            consumer,
            queue_resource,
            queue,
            requiredness,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunEcsLivenessReport {
    pub contracts: u32,
    pub required_consumers_covered: u32,
}

pub fn validate_subsystem_liveness(
    contracts: &[FunEcsSubsystemHandoffContract],
) -> Result<FunEcsLivenessReport, FunEcsValidationError> {
    if contracts.is_empty() {
        return Err(FunEcsValidationError::HandoffContractTableEmpty);
    }
    for contract in contracts {
        if contract.producer == contract.consumer {
            return Err(FunEcsValidationError::HandoffProducerConsumerCycle);
        }
    }
    let mut report = FunEcsLivenessReport {
        contracts: contracts.len() as u32,
        ..FunEcsLivenessReport::default()
    };
    for consumer in FUN_ECS_REQUIRED_HANDOFF_CONSUMERS {
        if contracts
            .iter()
            .any(|contract| contract.consumer == consumer && contract.queue.get() != 0)
        {
            report.required_consumers_covered += 1;
        } else {
            return Err(FunEcsValidationError::MissingRequiredHandoffConsumer);
        }
    }
    Ok(report)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunEcsValidationError {
    EmptySystemLabel = 0,
    DuplicateSystemId = 1,
    SystemDeclarationTableFull = 2,
    ResourceChunkTableFull = 3,
    InvalidResourceChunk = 4,
    HandoffContractTableEmpty = 5,
    HandoffProducerConsumerCycle = 6,
    MissingRequiredHandoffConsumer = 7,
}

impl FunEcsValidationError {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EmptySystemLabel => "empty_system_label",
            Self::DuplicateSystemId => "duplicate_system_id",
            Self::SystemDeclarationTableFull => "system_declaration_table_full",
            Self::ResourceChunkTableFull => "resource_chunk_table_full",
            Self::InvalidResourceChunk => "invalid_resource_chunk",
            Self::HandoffContractTableEmpty => "handoff_contract_table_empty",
            Self::HandoffProducerConsumerCycle => "handoff_producer_consumer_cycle",
            Self::MissingRequiredHandoffConsumer => "missing_required_handoff_consumer",
        }
    }
}

#[cfg(test)]
mod tests {
    use fun_scheduler_types::{EcsSystemClass, EcsSystemExecutionContract, ScheduleDomain};

    use super::*;

    #[test]
    fn control_plane_declares_systems_and_extracts_access() {
        let mut control = FunEcsControlPlane::new(FunEcsWorldId::ROOT);
        let system = FunEcsSystemDeclaration::new(
            FunEcsSystemId::new(7),
            "build_derived_artifacts",
            FunEcsSubsystem::FunEcs,
            EcsSystemClass::DerivedArtifactBuild,
            EcsSystemExecutionContract::DEFAULT,
            vec![
                FunEcsSystemAccess::read(FunEcsResourceKind::DecodedPageQueue),
                FunEcsSystemAccess::append_command(FunEcsResourceKind::DerivedArtifactRegistry),
            ],
        );

        control.declare_system(system).expect("declare system");

        let access = control
            .system_access(FunEcsSystemId::new(7))
            .expect("system access");
        assert_eq!(access.len(), 2);
        assert_eq!(
            access[1],
            FunEcsSystemAccess::append_command(FunEcsResourceKind::DerivedArtifactRegistry)
        );
        assert_eq!(control.revision.get(), 1);
        assert_eq!(
            control.declare_system(FunEcsSystemDeclaration::new(
                FunEcsSystemId::new(7),
                "duplicate",
                FunEcsSubsystem::FunEcs,
                EcsSystemClass::WorldQuery,
                EcsSystemExecutionContract::DEFAULT,
                Vec::new(),
            )),
            Err(FunEcsValidationError::DuplicateSystemId)
        );
    }

    #[test]
    fn resource_table_chunks_are_sorted_and_revisioned() {
        let mut control = FunEcsControlPlane::new(FunEcsWorldId::ROOT);
        control
            .upsert_resource_chunk(FunEcsResourceChunk::new(
                FunEcsResourceKind::RendererHandoffQueue,
                EcsChunkKey::new(9),
                16,
                4,
                FunEcsRevision(1),
            ))
            .expect("renderer chunk");
        control
            .upsert_resource_chunk(FunEcsResourceChunk::new(
                FunEcsResourceKind::DerivedArtifactRegistry,
                EcsChunkKey::new(3),
                0,
                16,
                FunEcsRevision(1),
            ))
            .expect("artifact chunk");

        assert_eq!(control.resource_chunks.len(), 2);
        assert_eq!(
            control.resource_chunks[0].resource,
            FunEcsResourceKind::DerivedArtifactRegistry
        );
        assert_eq!(control.revision.get(), 2);
        assert_eq!(
            control.upsert_resource_chunk(FunEcsResourceChunk::new(
                FunEcsResourceKind::TelemetryEventQueue,
                EcsChunkKey::WHOLE_WORLD,
                0,
                0,
                FunEcsRevision(1),
            )),
            Err(FunEcsValidationError::InvalidResourceChunk)
        );
    }

    #[test]
    fn subsystem_handoff_contracts_cover_cross_domain_liveness() {
        let report = validate_subsystem_liveness(&FUN_ECS_SUBSYSTEM_HANDOFF_CONTRACTS)
            .expect("default liveness contracts");

        assert_eq!(
            report.required_consumers_covered as usize,
            FUN_ECS_REQUIRED_HANDOFF_CONSUMER_COUNT
        );
        assert!(FUN_ECS_SUBSYSTEM_HANDOFF_CONTRACTS.iter().any(|contract| {
            contract.consumer == FunEcsSubsystem::Renderer
                && contract.queue_resource == FunEcsResourceKind::RendererHandoffQueue
                && contract.requiredness == WorkRequiredness::Required
        }));
        assert!(FUN_ECS_SUBSYSTEM_HANDOFF_CONTRACTS.iter().any(|contract| {
            contract.consumer == FunEcsSubsystem::Rvelte
                && contract.queue_resource == FunEcsResourceKind::RvelteUiPacketQueue
        }));
        assert!(FUN_ECS_SUBSYSTEM_HANDOFF_CONTRACTS.iter().any(|contract| {
            contract.consumer == FunEcsSubsystem::Warden
                && contract.queue_resource == FunEcsResourceKind::WardenEvidenceQueue
        }));
        assert_eq!(
            EcsSystemExecutionContract::DEFAULT.domain,
            ScheduleDomain::FunEcs
        );
    }
}
