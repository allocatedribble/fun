use fun_scheduler_types::{EcsAgentManifestId, EcsChunkKey, EcsEntityId, EcsSystemId};

use crate::{
    EcsSpatialCommand, EcsSpatialCommandApplyDigest, FunArtifactId, FunCommandBufferId,
    FunEcsResourceKind, FunEcsSubsystem, FunEntityGeneration, FunResourceId, FunResourceTableId,
    FunRevision, FunSystemAccess, FunSystemAccessMode, FunSystemAccessRow, FunSystemAccessTarget,
    FunSystemId,
};

pub const FUN_COMMAND_BUFFER_DEFAULT_CAPACITY: usize = 65_536;
pub const FUN_COMMAND_JOURNAL_MAX_ROWS: usize = 262_144;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunCommandBufferClass {
    #[default]
    WorldStructure = 0,
    ResourceTableMutation = 1,
    ArtifactPublication = 2,
    HandoffPublication = 3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunCommandDrainPolicy {
    #[default]
    DeterministicSortThenApply = 0,
    PreserveProducerOrder = 1,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunCommandKind {
    #[default]
    Unknown = 0,
    SpatialRequestPage = 1,
    SpatialCancelPage = 2,
    SpatialPinPage = 3,
    SpatialUnpinPage = 4,
    SpatialMarkDirty = 5,
    SpatialApplyVoxelEdit = 6,
    ArtifactPublish = 7,
    ArtifactRetire = 8,
    HandoffRenderer = 9,
    HandoffLux = 10,
    PhysicsCook = 11,
    HandoffNetwork = 12,
    RendererCommand = 13,
    NetworkCommand = 14,
    UiCommand = 15,
    AuthoringCommand = 16,
    AgentProposal = 17,
}

impl FunCommandKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::SpatialRequestPage => "spatial_request_page",
            Self::SpatialCancelPage => "spatial_cancel_page",
            Self::SpatialPinPage => "spatial_pin_page",
            Self::SpatialUnpinPage => "spatial_unpin_page",
            Self::SpatialMarkDirty => "spatial_mark_dirty",
            Self::SpatialApplyVoxelEdit => "spatial_apply_voxel_edit",
            Self::ArtifactPublish => "artifact_publish",
            Self::ArtifactRetire => "artifact_retire",
            Self::HandoffRenderer => "handoff_renderer",
            Self::HandoffLux => "handoff_lux",
            Self::PhysicsCook => "physics_cook",
            Self::HandoffNetwork => "handoff_network",
            Self::RendererCommand => "renderer_command",
            Self::NetworkCommand => "network_command",
            Self::UiCommand => "ui_command",
            Self::AuthoringCommand => "authoring_command",
            Self::AgentProposal => "agent_proposal",
        }
    }

    #[must_use]
    pub const fn default_buffer_class(self) -> FunCommandBufferClass {
        match self {
            Self::ArtifactPublish | Self::ArtifactRetire => {
                FunCommandBufferClass::ArtifactPublication
            }
            Self::HandoffRenderer | Self::HandoffLux | Self::HandoffNetwork | Self::PhysicsCook => {
                FunCommandBufferClass::HandoffPublication
            }
            Self::SpatialRequestPage
            | Self::SpatialCancelPage
            | Self::SpatialPinPage
            | Self::SpatialUnpinPage
            | Self::SpatialMarkDirty
            | Self::SpatialApplyVoxelEdit
            | Self::AuthoringCommand
            | Self::AgentProposal => FunCommandBufferClass::ResourceTableMutation,
            Self::RendererCommand | Self::NetworkCommand | Self::UiCommand | Self::Unknown => {
                FunCommandBufferClass::WorldStructure
            }
        }
    }

    #[must_use]
    pub const fn mutates_required_path(self) -> bool {
        matches!(
            self,
            Self::SpatialRequestPage
                | Self::SpatialCancelPage
                | Self::SpatialPinPage
                | Self::SpatialUnpinPage
                | Self::SpatialMarkDirty
                | Self::SpatialApplyVoxelEdit
                | Self::ArtifactPublish
                | Self::ArtifactRetire
                | Self::HandoffRenderer
                | Self::HandoffLux
                | Self::PhysicsCook
                | Self::HandoffNetwork
                | Self::AuthoringCommand
                | Self::AgentProposal
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum FunCommandPhase {
    #[default]
    Emit = 0,
    ApplyRequestCommands = 1,
    ApplyArtifactCommands = 2,
    ApplyDirtyPropagationCommands = 3,
    ApplyHandoffCommands = 4,
    Diagnostics = 5,
    OptionalPresentation = 6,
}

impl FunCommandPhase {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Emit => "emit",
            Self::ApplyRequestCommands => "apply_request_commands",
            Self::ApplyArtifactCommands => "apply_artifact_commands",
            Self::ApplyDirtyPropagationCommands => "apply_dirty_propagation_commands",
            Self::ApplyHandoffCommands => "apply_handoff_commands",
            Self::Diagnostics => "diagnostics",
            Self::OptionalPresentation => "optional_presentation",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunCommandMergePolicy {
    #[default]
    StableDeterministicReduction = 0,
    PreserveProducerOrder = 1,
    CompletionOrderOptionalPresentation = 2,
}

impl FunCommandMergePolicy {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::StableDeterministicReduction => "stable_deterministic_reduction",
            Self::PreserveProducerOrder => "preserve_producer_order",
            Self::CompletionOrderOptionalPresentation => "completion_order_optional_presentation",
        }
    }

    #[must_use]
    pub const fn allows_completion_order_commit(
        self,
        optional_presentation_feature_enabled: bool,
    ) -> bool {
        matches!(self, Self::CompletionOrderOptionalPresentation)
            && optional_presentation_feature_enabled
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FunCommandDeterministicKey {
    pub frame_revision: FunRevision,
    pub schedule_set_ordinal: u16,
    pub system_id: FunSystemId,
    pub chunk_key: EcsChunkKey,
    pub entity_id: EcsEntityId,
    pub resource_key: FunResourceId,
    pub command_sequence: u64,
}

impl FunCommandDeterministicKey {
    #[must_use]
    pub const fn new(
        frame_revision: FunRevision,
        schedule_set_ordinal: u16,
        system_id: FunSystemId,
        chunk_key: EcsChunkKey,
        command_sequence: u64,
    ) -> Self {
        Self {
            frame_revision,
            schedule_set_ordinal,
            system_id,
            chunk_key,
            entity_id: EcsEntityId::new(0),
            resource_key: FunResourceId::INVALID,
            command_sequence,
        }
    }

    #[must_use]
    pub const fn with_entity(mut self, entity_id: EcsEntityId) -> Self {
        self.entity_id = entity_id;
        self
    }

    #[must_use]
    pub const fn with_resource(mut self, resource_key: FunResourceId) -> Self {
        self.resource_key = resource_key;
        self
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.system_id.is_valid() && self.command_sequence != 0
    }
}

pub trait FunCommandPayload: Clone {
    fn command_kind(&self) -> FunCommandKind;
    fn payload_digest(&self) -> u64;

    fn target_resource(&self) -> Option<FunResourceId> {
        None
    }

    fn target_artifact(&self) -> Option<FunArtifactId> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunCommandEnvelope<C> {
    pub buffer: FunCommandBufferId,
    pub kind: FunCommandKind,
    pub observed_revision: FunRevision,
    pub deterministic_key: Option<FunCommandDeterministicKey>,
    pub phase: FunCommandPhase,
    pub target_entity: Option<EcsEntityId>,
    pub observed_entity_generation: Option<FunEntityGeneration>,
    pub target_resource: Option<FunResourceId>,
    pub target_artifact: Option<FunArtifactId>,
    pub observed_page_generation: Option<u32>,
    pub observed_artifact_generation: Option<u32>,
    pub optional_presentation: bool,
    pub command: C,
}

impl<C: FunCommandPayload> FunCommandEnvelope<C> {
    #[must_use]
    pub fn new(
        buffer: FunCommandBufferId,
        observed_revision: FunRevision,
        deterministic_key: FunCommandDeterministicKey,
        phase: FunCommandPhase,
        command: C,
    ) -> Self {
        Self {
            buffer,
            kind: command.command_kind(),
            observed_revision,
            deterministic_key: Some(deterministic_key),
            phase,
            target_entity: None,
            observed_entity_generation: None,
            target_resource: command.target_resource(),
            target_artifact: command.target_artifact(),
            observed_page_generation: None,
            observed_artifact_generation: None,
            optional_presentation: false,
            command,
        }
    }

    #[must_use]
    pub fn without_deterministic_key(
        buffer: FunCommandBufferId,
        observed_revision: FunRevision,
        phase: FunCommandPhase,
        command: C,
    ) -> Self {
        Self {
            buffer,
            kind: command.command_kind(),
            observed_revision,
            deterministic_key: None,
            phase,
            target_entity: None,
            observed_entity_generation: None,
            target_resource: command.target_resource(),
            target_artifact: command.target_artifact(),
            observed_page_generation: None,
            observed_artifact_generation: None,
            optional_presentation: false,
            command,
        }
    }

    #[must_use]
    pub fn with_entity_generation(
        mut self,
        entity: EcsEntityId,
        generation: FunEntityGeneration,
    ) -> Self {
        self.target_entity = Some(entity);
        self.observed_entity_generation = Some(generation);
        self
    }

    #[must_use]
    pub const fn with_page_generation(mut self, generation: u32) -> Self {
        self.observed_page_generation = Some(generation);
        self
    }

    #[must_use]
    pub const fn with_artifact_generation(mut self, generation: u32) -> Self {
        self.observed_artifact_generation = Some(generation);
        self
    }

    #[must_use]
    pub const fn optional_presentation(mut self) -> Self {
        self.optional_presentation = true;
        self
    }

    #[must_use = "command application must validate the deterministic key before staging"]
    pub fn deterministic_key(
        &self,
    ) -> Result<FunCommandDeterministicKey, FunCommandValidationError> {
        self.deterministic_key
            .filter(|key| key.is_valid())
            .ok_or(FunCommandValidationError::MissingDeterministicKey)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunCommandBuffer<C> {
    pub id: FunCommandBufferId,
    pub merge_policy: FunCommandMergePolicy,
    pub capacity: usize,
    pub commands: Vec<FunCommandEnvelope<C>>,
}

impl<C> FunCommandBuffer<C> {
    #[must_use]
    pub fn new(id: FunCommandBufferId) -> Self {
        Self {
            id,
            merge_policy: FunCommandMergePolicy::StableDeterministicReduction,
            capacity: FUN_COMMAND_BUFFER_DEFAULT_CAPACITY,
            commands: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_merge_policy(mut self, merge_policy: FunCommandMergePolicy) -> Self {
        self.merge_policy = merge_policy;
        self
    }
}

impl<C: FunCommandPayload> FunCommandBuffer<C> {
    pub fn push(
        &mut self,
        envelope: FunCommandEnvelope<C>,
    ) -> Result<(), FunCommandValidationError> {
        if self.commands.len() >= self.capacity {
            return Err(FunCommandValidationError::CommandBufferFull);
        }
        if envelope.buffer != self.id {
            return Err(FunCommandValidationError::CommandBufferDenied);
        }
        envelope.deterministic_key()?;
        if envelope.optional_presentation && envelope.kind.mutates_required_path() {
            return Err(FunCommandValidationError::OptionalCommandMutatesRequiredPath);
        }
        self.commands.push(envelope);
        Ok(())
    }

    #[must_use]
    pub fn journal(&self) -> FunCommandJournal<C> {
        FunCommandJournal::from_commands(self.commands.clone(), self.merge_policy)
    }

    #[must_use]
    pub fn deterministic_commands(&self) -> Vec<FunCommandEnvelope<C>> {
        deterministic_order(self.commands.clone())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunCommandDigest {
    pub value: u64,
}

impl FunCommandDigest {
    #[must_use]
    pub fn from_envelopes<C: FunCommandPayload>(commands: &[FunCommandEnvelope<C>]) -> Self {
        if commands.is_empty() {
            return Self { value: 0 };
        }
        let mut ordered = commands.to_vec();
        ordered = deterministic_order(ordered);
        let mut value = 0xcbf2_9ce4_8422_2325_u64;
        for command in &ordered {
            value = command_hash_u8(value, command.kind as u8);
            value = command_hash_u64(value, command.buffer.get() as u64);
            value = command_hash_u64(value, command.observed_revision.get());
            value = command_hash_u8(value, command.phase as u8);
            if let Some(key) = command.deterministic_key {
                value = command_hash_u64(value, key.frame_revision.get());
                value = command_hash_u64(value, u64::from(key.schedule_set_ordinal));
                value = command_hash_u64(value, key.system_id.get() as u64);
                value = command_hash_u64(value, key.chunk_key.get());
                value = command_hash_u64(value, key.entity_id.get());
                value = command_hash_u64(value, key.resource_key.get());
                value = command_hash_u64(value, key.command_sequence);
            }
            value = command_hash_u64(value, command.command.payload_digest());
        }
        Self { value }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunCommandJournal<C> {
    pub merge_policy: FunCommandMergePolicy,
    pub commands: Vec<FunCommandEnvelope<C>>,
    pub digest: FunCommandDigest,
}

impl<C: FunCommandPayload> FunCommandJournal<C> {
    #[must_use]
    pub fn from_commands(
        commands: Vec<FunCommandEnvelope<C>>,
        merge_policy: FunCommandMergePolicy,
    ) -> Self {
        let commands = match merge_policy {
            FunCommandMergePolicy::StableDeterministicReduction => deterministic_order(commands),
            FunCommandMergePolicy::PreserveProducerOrder
            | FunCommandMergePolicy::CompletionOrderOptionalPresentation => commands,
        };
        let digest = FunCommandDigest::from_envelopes(&commands);
        Self {
            merge_policy,
            commands,
            digest,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FunCommandApplyReport {
    pub inspected: u32,
    pub applied: u32,
    pub rejected: u32,
    pub retained: u32,
    pub digest: FunCommandDigest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunCommandValidationError {
    StaleWorldRevision,
    StaleEntityGeneration,
    StalePageGeneration,
    StaleArtifactGeneration,
    AccessDenied,
    CommandBufferDenied,
    OptionalCommandMutatesRequiredPath,
    MissingDeterministicKey,
    AppliedOutsidePhase,
    CommandBufferFull,
}

impl FunCommandValidationError {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::StaleWorldRevision => "stale_world_revision",
            Self::StaleEntityGeneration => "stale_entity_generation",
            Self::StalePageGeneration => "stale_page_generation",
            Self::StaleArtifactGeneration => "stale_artifact_generation",
            Self::AccessDenied => "access_denied",
            Self::CommandBufferDenied => "command_buffer_denied",
            Self::OptionalCommandMutatesRequiredPath => "optional_command_mutates_required_path",
            Self::MissingDeterministicKey => "missing_deterministic_key",
            Self::AppliedOutsidePhase => "applied_outside_phase",
            Self::CommandBufferFull => "command_buffer_full",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunSubsystemCommand {
    pub kind: FunCommandKind,
    pub target_resource: Option<FunResourceId>,
    pub payload_digest: u64,
}

impl FunCommandPayload for FunSubsystemCommand {
    fn command_kind(&self) -> FunCommandKind {
        self.kind
    }

    fn payload_digest(&self) -> u64 {
        self.payload_digest
    }

    fn target_resource(&self) -> Option<FunResourceId> {
        self.target_resource
    }
}

pub type SpatialCommandBuffer = FunCommandBuffer<EcsSpatialCommand>;
pub type ArtifactCommandBuffer = FunCommandBuffer<EcsSpatialCommand>;
pub type HandoffCommandBuffer = FunCommandBuffer<EcsSpatialCommand>;
pub type PhysicsCommandBuffer = FunCommandBuffer<EcsSpatialCommand>;
pub type RendererCommandBuffer = FunCommandBuffer<FunSubsystemCommand>;
pub type NetworkCommandBuffer = FunCommandBuffer<FunSubsystemCommand>;
pub type UiCommandBuffer = FunCommandBuffer<FunSubsystemCommand>;
pub type AuthoringCommandBuffer = FunCommandBuffer<FunSubsystemCommand>;
pub type AgentProposalCommandBuffer<C = EcsSpatialCommand> =
    FunCommandBuffer<AgentCommandEnvelope<C>>;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AgentAccessScope {
    pub access: FunSystemAccess,
    pub command_buffers: Vec<FunCommandBufferId>,
    pub phases: Vec<FunCommandPhase>,
    pub allow_required_path_mutation: bool,
}

impl AgentAccessScope {
    #[must_use]
    pub fn new(access: FunSystemAccess) -> Self {
        Self {
            access,
            command_buffers: Vec::new(),
            phases: vec![FunCommandPhase::Emit],
            allow_required_path_mutation: false,
        }
    }

    #[must_use]
    pub fn with_command_buffer(mut self, buffer: FunCommandBufferId) -> Self {
        if !self.command_buffers.contains(&buffer) {
            self.command_buffers.push(buffer);
            self.command_buffers.sort_unstable();
        }
        self
    }

    #[must_use]
    pub fn with_phase(mut self, phase: FunCommandPhase) -> Self {
        if !self.phases.contains(&phase) {
            self.phases.push(phase);
            self.phases.sort_unstable();
        }
        self
    }

    #[must_use]
    pub const fn allow_required_path_mutation(mut self) -> Self {
        self.allow_required_path_mutation = true;
        self
    }

    #[must_use]
    pub fn permits_access(&self, requested: &FunSystemAccess) -> bool {
        requested
            .rows
            .iter()
            .all(|row| self.access.rows.contains(row))
    }

    #[must_use]
    pub fn permits_buffer(&self, buffer: FunCommandBufferId) -> bool {
        self.command_buffers.contains(&buffer)
            || self.access.rows.iter().any(|row| {
                row.target == FunSystemAccessTarget::CommandBuffer(buffer)
                    && row.mode == FunSystemAccessMode::Output
            })
    }

    #[must_use]
    pub fn permits_resource_write(&self, resource: FunResourceId) -> bool {
        let table = FunResourceTableId::new(resource.get());
        self.access.rows.iter().any(|row| {
            row == &FunSystemAccessRow::new(
                FunSystemAccessTarget::Resource(resource),
                FunSystemAccessMode::Write,
            ) || row
                == &FunSystemAccessRow::new(
                    FunSystemAccessTarget::ResourceTable(table),
                    FunSystemAccessMode::Write,
                )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentManifest {
    pub id: EcsAgentManifestId,
    pub system: EcsSystemId,
    pub scope: AgentAccessScope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentMutationProposal<C> {
    pub observed_revision: FunRevision,
    pub requested_access: FunSystemAccess,
    pub phase: FunCommandPhase,
    pub command: FunCommandEnvelope<C>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCommandEnvelope<C> {
    pub manifest: EcsAgentManifestId,
    pub system: EcsSystemId,
    pub proposal_digest: FunCommandDigest,
    pub command: FunCommandEnvelope<C>,
}

impl<C: FunCommandPayload> FunCommandPayload for AgentCommandEnvelope<C> {
    fn command_kind(&self) -> FunCommandKind {
        FunCommandKind::AgentProposal
    }

    fn payload_digest(&self) -> u64 {
        let mut value = 0xcbf2_9ce4_8422_2325_u64;
        value = command_hash_u64(value, self.manifest.get());
        value = command_hash_u64(value, u64::from(self.system.get()));
        command_hash_u64(value, self.proposal_digest.value)
    }

    fn target_resource(&self) -> Option<FunResourceId> {
        self.command.target_resource
    }

    fn target_artifact(&self) -> Option<FunArtifactId> {
        self.command.target_artifact
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AgentProposalWorldState {
    pub revision: FunRevision,
    pub entity_generation: Option<FunEntityGeneration>,
    pub page_generation: Option<u32>,
    pub artifact_generation: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentProposalDiagnosticCode {
    Accepted,
    StaleWorldRevision,
    StaleEntityGeneration,
    StalePageGeneration,
    StaleArtifactGeneration,
    AccessDenied,
    CommandBufferDenied,
    OptionalCommandMutatesRequiredPath,
    MissingDeterministicKey,
    PhaseDenied,
}

impl AgentProposalDiagnosticCode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::StaleWorldRevision => "stale_world_revision",
            Self::StaleEntityGeneration => "stale_entity_generation",
            Self::StalePageGeneration => "stale_page_generation",
            Self::StaleArtifactGeneration => "stale_artifact_generation",
            Self::AccessDenied => "access_denied",
            Self::CommandBufferDenied => "command_buffer_denied",
            Self::OptionalCommandMutatesRequiredPath => "optional_command_mutates_required_path",
            Self::MissingDeterministicKey => "missing_deterministic_key",
            Self::PhaseDenied => "phase_denied",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentProposalDiagnostic {
    pub code: AgentProposalDiagnosticCode,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AgentProposalValidationReport {
    pub accepted: bool,
    pub diagnostics: Vec<AgentProposalDiagnostic>,
    pub digest: FunCommandDigest,
}

impl AgentProposalValidationReport {
    fn reject(code: AgentProposalDiagnosticCode, digest: FunCommandDigest) -> Self {
        Self {
            accepted: false,
            diagnostics: vec![AgentProposalDiagnostic { code }],
            digest,
        }
    }

    fn accept(digest: FunCommandDigest) -> Self {
        Self {
            accepted: true,
            diagnostics: vec![AgentProposalDiagnostic {
                code: AgentProposalDiagnosticCode::Accepted,
            }],
            digest,
        }
    }
}

pub fn validate_agent_proposal<C: FunCommandPayload>(
    manifest: &AgentManifest,
    proposal: &AgentMutationProposal<C>,
    state: AgentProposalWorldState,
) -> AgentProposalValidationReport {
    let digest = FunCommandDigest::from_envelopes(core::slice::from_ref(&proposal.command));
    if proposal.observed_revision != state.revision
        || proposal.command.observed_revision != state.revision
    {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::StaleWorldRevision,
            digest,
        );
    }
    if proposal.command.deterministic_key().is_err() {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::MissingDeterministicKey,
            digest,
        );
    }
    if !manifest.scope.permits_access(&proposal.requested_access) {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::AccessDenied,
            digest,
        );
    }
    if !manifest.scope.permits_buffer(proposal.command.buffer) {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::CommandBufferDenied,
            digest,
        );
    }
    if !manifest.scope.phases.contains(&proposal.phase) || proposal.command.phase != proposal.phase
    {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::PhaseDenied,
            digest,
        );
    }
    if let Some(resource) = proposal.command.target_resource
        && !manifest.scope.permits_resource_write(resource)
    {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::AccessDenied,
            digest,
        );
    }
    if proposal.command.optional_presentation
        && proposal.command.kind.mutates_required_path()
        && !manifest.scope.allow_required_path_mutation
    {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::OptionalCommandMutatesRequiredPath,
            digest,
        );
    }
    if let Some(observed) = proposal.command.observed_entity_generation
        && Some(observed) != state.entity_generation
    {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::StaleEntityGeneration,
            digest,
        );
    }
    if let Some(observed) = proposal.command.observed_page_generation
        && Some(observed) != state.page_generation
    {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::StalePageGeneration,
            digest,
        );
    }
    if let Some(observed) = proposal.command.observed_artifact_generation
        && Some(observed) != state.artifact_generation
    {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::StaleArtifactGeneration,
            digest,
        );
    }
    AgentProposalValidationReport::accept(digest)
}

pub fn stage_agent_proposal<C: FunCommandPayload>(
    manifest: &AgentManifest,
    proposal: AgentMutationProposal<C>,
    state: AgentProposalWorldState,
    output: &mut AgentProposalCommandBuffer<C>,
) -> AgentProposalValidationReport {
    let report = validate_agent_proposal(manifest, &proposal, state);
    if !report.accepted {
        return report;
    }
    let agent_command = AgentCommandEnvelope {
        manifest: manifest.id,
        system: manifest.system,
        proposal_digest: report.digest,
        command: proposal.command,
    };
    let Some(key) = agent_command.command.deterministic_key else {
        return AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::MissingDeterministicKey,
            report.digest,
        );
    };
    let envelope = FunCommandEnvelope::new(
        output.id,
        state.revision,
        key,
        FunCommandPhase::Emit,
        agent_command,
    );
    match output.push(envelope) {
        Ok(()) => report,
        Err(FunCommandValidationError::CommandBufferDenied) => {
            AgentProposalValidationReport::reject(
                AgentProposalDiagnosticCode::CommandBufferDenied,
                report.digest,
            )
        }
        Err(FunCommandValidationError::OptionalCommandMutatesRequiredPath) => {
            AgentProposalValidationReport::reject(
                AgentProposalDiagnosticCode::OptionalCommandMutatesRequiredPath,
                report.digest,
            )
        }
        Err(FunCommandValidationError::MissingDeterministicKey) => {
            AgentProposalValidationReport::reject(
                AgentProposalDiagnosticCode::MissingDeterministicKey,
                report.digest,
            )
        }
        Err(_error) => AgentProposalValidationReport::reject(
            AgentProposalDiagnosticCode::AccessDenied,
            report.digest,
        ),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpatialCommandJournalContext {
    pub buffer: FunCommandBufferId,
    pub observed_revision: FunRevision,
    pub frame_revision: FunRevision,
    pub schedule_set_ordinal: u16,
    pub system_id: FunSystemId,
    pub chunk_key: EcsChunkKey,
    pub phase: FunCommandPhase,
}

impl SpatialCommandJournalContext {
    #[must_use]
    pub const fn new(
        buffer: FunCommandBufferId,
        observed_revision: FunRevision,
        frame_revision: FunRevision,
        schedule_set_ordinal: u16,
        system_id: FunSystemId,
        chunk_key: EcsChunkKey,
        phase: FunCommandPhase,
    ) -> Self {
        Self {
            buffer,
            observed_revision,
            frame_revision,
            schedule_set_ordinal,
            system_id,
            chunk_key,
            phase,
        }
    }
}

#[must_use]
pub fn spatial_command_journal_from_commands(
    commands: &[EcsSpatialCommand],
    context: SpatialCommandJournalContext,
) -> FunCommandJournal<EcsSpatialCommand> {
    let mut sorted = commands.to_vec();
    sorted.sort_by_key(|command| (command.command_kind(), command.payload_digest()));
    let envelopes: Vec<FunCommandEnvelope<EcsSpatialCommand>> = sorted
        .into_iter()
        .enumerate()
        .map(|(index, command)| {
            let key = FunCommandDeterministicKey::new(
                context.frame_revision,
                context.schedule_set_ordinal,
                context.system_id,
                context.chunk_key,
                index as u64 + 1,
            )
            .with_resource(command.target_resource().unwrap_or(FunResourceId::INVALID));
            FunCommandEnvelope::new(
                context.buffer,
                context.observed_revision,
                key,
                context.phase,
                command,
            )
        })
        .collect();
    FunCommandJournal::from_commands(
        envelopes,
        FunCommandMergePolicy::StableDeterministicReduction,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunCommandBufferDeclaration {
    pub id: FunCommandBufferId,
    pub owner: FunEcsSubsystem,
    pub target_resource: FunEcsResourceKind,
    pub class: FunCommandBufferClass,
    pub drain_policy: FunCommandDrainPolicy,
}

impl FunCommandBufferDeclaration {
    #[must_use]
    pub const fn deterministic(
        id: FunCommandBufferId,
        owner: FunEcsSubsystem,
        target_resource: FunEcsResourceKind,
        class: FunCommandBufferClass,
    ) -> Self {
        Self {
            id,
            owner,
            target_resource,
            class,
            drain_policy: FunCommandDrainPolicy::DeterministicSortThenApply,
        }
    }
}

impl FunCommandPayload for EcsSpatialCommand {
    fn command_kind(&self) -> FunCommandKind {
        match self {
            Self::RequestPage(_) => FunCommandKind::SpatialRequestPage,
            Self::CancelPage(_) => FunCommandKind::SpatialCancelPage,
            Self::PinPage(_) => FunCommandKind::SpatialPinPage,
            Self::UnpinPage(_) => FunCommandKind::SpatialUnpinPage,
            Self::PublishArtifact(_) => FunCommandKind::ArtifactPublish,
            Self::RetireArtifact(_) => FunCommandKind::ArtifactRetire,
            Self::MarkDirty(_) => FunCommandKind::SpatialMarkDirty,
            Self::ApplyVoxelEdit(_) => FunCommandKind::SpatialApplyVoxelEdit,
            Self::PublishRendererHandoff(_) => FunCommandKind::HandoffRenderer,
            Self::PublishLuxHandoff(_) => FunCommandKind::HandoffLux,
            Self::PublishPhysicsCook(_) => FunCommandKind::PhysicsCook,
        }
    }

    fn payload_digest(&self) -> u64 {
        EcsSpatialCommandApplyDigest::from_commands(&[*self]).value
    }

    fn target_resource(&self) -> Option<FunResourceId> {
        let resource = match self {
            Self::RequestPage(_) | Self::CancelPage(_) | Self::PinPage(_) | Self::UnpinPage(_) => {
                FunEcsResourceKind::StreamRequestQueue
            }
            Self::PublishArtifact(_) | Self::RetireArtifact(_) => {
                FunEcsResourceKind::DerivedArtifactRegistry
            }
            Self::MarkDirty(_) => FunEcsResourceKind::DirtyRegionLedger,
            Self::ApplyVoxelEdit(_) => FunEcsResourceKind::PageResidencyTable,
            Self::PublishRendererHandoff(_) => FunEcsResourceKind::RendererHandoffQueue,
            Self::PublishLuxHandoff(_) => FunEcsResourceKind::LuxHandoffQueue,
            Self::PublishPhysicsCook(_) => FunEcsResourceKind::PhysicsCookQueue,
        };
        Some(FunResourceId::from_resource_kind(resource))
    }

    fn target_artifact(&self) -> Option<FunArtifactId> {
        match self {
            Self::PublishArtifact(artifact) => Some(FunArtifactId::from_derived_artifact_id(
                artifact.artifact_id,
            )),
            Self::RetireArtifact(artifact_id) => {
                Some(FunArtifactId::from_derived_artifact_id(*artifact_id))
            }
            Self::PublishRendererHandoff(handoff) => {
                Some(FunArtifactId::from_derived_artifact_id(handoff.artifact_id))
            }
            _ => None,
        }
    }
}

fn deterministic_order<C: FunCommandPayload>(
    mut commands: Vec<FunCommandEnvelope<C>>,
) -> Vec<FunCommandEnvelope<C>> {
    commands.sort_by_key(|command| {
        (
            command.deterministic_key.unwrap_or_default(),
            command.kind,
            command.command.payload_digest(),
        )
    });
    commands
}

const fn command_hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn command_hash_u64(mut hash: u64, value: u64) -> u64 {
    hash = command_hash_u8(hash, (value & 0xff) as u8);
    hash = command_hash_u8(hash, ((value >> 8) & 0xff) as u8);
    hash = command_hash_u8(hash, ((value >> 16) & 0xff) as u8);
    hash = command_hash_u8(hash, ((value >> 24) & 0xff) as u8);
    hash = command_hash_u8(hash, ((value >> 32) & 0xff) as u8);
    hash = command_hash_u8(hash, ((value >> 40) & 0xff) as u8);
    hash = command_hash_u8(hash, ((value >> 48) & 0xff) as u8);
    command_hash_u8(hash, ((value >> 56) & 0xff) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EcsAabbI64, EcsDirtyRegionLedger, EcsPageChannel, EcsPageResidencyRecord,
        EcsPageResidencyTable, EcsSpatialCommandBuffer, EcsSpatialDomainKind, EcsSpatialGridId,
        EcsSpatialPageKey, EcsStreamInterestKind, EcsStreamPriority,
        EcsVoxelEditPropagationOptions, FUN_COMMAND_BUFFER_DIRTY_PROPAGATION,
        FUN_COMMAND_BUFFER_SPATIAL_REQUESTS, FixedStepId, TerrainMaterialId, VoxelEditOp,
        propagate_voxel_edit,
    };

    fn page(x: i32) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            x,
            0,
            0,
            EcsPageChannel::Surface,
        )
    }

    fn deterministic_key(sequence: u64, chunk: u64) -> FunCommandDeterministicKey {
        FunCommandDeterministicKey::new(
            FunRevision::new(9),
            3,
            FunSystemId::new(7),
            EcsChunkKey::new(chunk),
            sequence,
        )
    }

    fn request_envelope(
        sequence: u64,
        chunk: u64,
        page: EcsSpatialPageKey,
    ) -> FunCommandEnvelope<EcsSpatialCommand> {
        FunCommandEnvelope::new(
            FUN_COMMAND_BUFFER_SPATIAL_REQUESTS.buffer,
            FunRevision::new(2),
            deterministic_key(sequence, chunk),
            FunCommandPhase::Emit,
            EcsSpatialCommand::RequestPage(page),
        )
    }

    #[test]
    fn deterministic_command_journal_digest_ignores_chunk_completion_order() {
        let mut first = SpatialCommandBuffer::new(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS.buffer);
        first
            .push(request_envelope(2, 200, page(2)))
            .expect("push first late chunk");
        first
            .push(request_envelope(1, 100, page(1)))
            .expect("push first early chunk");

        let mut second = SpatialCommandBuffer::new(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS.buffer);
        second
            .push(request_envelope(1, 100, page(1)))
            .expect("push second early chunk");
        second
            .push(request_envelope(2, 200, page(2)))
            .expect("push second late chunk");

        let first_journal = first.journal();
        let second_journal = second.journal();
        assert_eq!(first_journal.digest, second_journal.digest);
        assert_eq!(
            first_journal.commands[0]
                .deterministic_key
                .expect("key")
                .command_sequence,
            1
        );
        assert_eq!(
            second_journal.commands[0]
                .deterministic_key
                .expect("key")
                .command_sequence,
            1
        );
    }

    #[test]
    fn invalid_agent_proposal_reports_diagnostic_without_mutating_buffer() {
        let access = FunSystemAccess::write_resource_kind(FunEcsResourceKind::StreamRequestQueue)
            .merge(FunSystemAccess::command_output(
                FUN_COMMAND_BUFFER_SPATIAL_REQUESTS,
            ));
        let manifest = AgentManifest {
            id: EcsAgentManifestId::new(4),
            system: EcsSystemId::new(99),
            scope: AgentAccessScope::new(access.clone())
                .with_command_buffer(FUN_COMMAND_BUFFER_SPATIAL_REQUESTS.buffer)
                .with_phase(FunCommandPhase::Emit)
                .allow_required_path_mutation(),
        };
        let proposal = AgentMutationProposal {
            observed_revision: FunRevision::new(1),
            requested_access: access,
            phase: FunCommandPhase::Emit,
            command: request_envelope(1, 10, page(1)),
        };
        let mut output = AgentProposalCommandBuffer::new(FunCommandBufferId::new(81));

        let report = stage_agent_proposal(
            &manifest,
            proposal,
            AgentProposalWorldState {
                revision: FunRevision::new(2),
                ..AgentProposalWorldState::default()
            },
            &mut output,
        );

        assert!(!report.accepted);
        assert_eq!(
            report.diagnostics[0].code,
            AgentProposalDiagnosticCode::StaleWorldRevision
        );
        assert!(output.commands.is_empty());
    }

    #[test]
    fn voxel_edit_propagation_has_deterministic_command_journal_digest() {
        let source_page = page(0);
        let build_commands = || {
            let mut page_table = EcsPageResidencyTable::default();
            page_table
                .push(EcsPageResidencyRecord::new(
                    source_page,
                    EcsStreamPriority::new(EcsStreamInterestKind::CollisionCriticalNear, 0, 0),
                    1,
                ))
                .expect("page insert");
            let mut dirty_ledger = EcsDirtyRegionLedger::default();
            let mut next_artifact_id = 100;
            let mut commands = EcsSpatialCommandBuffer::default();
            let mut options = EcsVoxelEditPropagationOptions::collision_critical(
                source_page,
                22,
                FixedStepId::new(7),
            );
            options.navigation_enabled = true;
            options.audio_enabled = true;
            options.network_enabled = true;
            propagate_voxel_edit(
                VoxelEditOp::FillAabb {
                    bounds: EcsAabbI64::new([0, 0, 0], [32, 32, 32]),
                    material: TerrainMaterialId::new(3),
                },
                options,
                &mut page_table,
                &mut dirty_ledger,
                &mut next_artifact_id,
                &mut commands,
            )
            .expect("propagate edit");
            commands.commands
        };

        let first = build_commands();
        let mut second = build_commands();
        second.reverse();

        let first_journal = spatial_command_journal_from_commands(
            &first,
            SpatialCommandJournalContext::new(
                FUN_COMMAND_BUFFER_DIRTY_PROPAGATION.buffer,
                FunRevision::new(22),
                FunRevision::new(2),
                8,
                FunSystemId::new(9),
                source_page.chunk_key(),
                FunCommandPhase::ApplyDirtyPropagationCommands,
            ),
        );
        let second_journal = spatial_command_journal_from_commands(
            &second,
            SpatialCommandJournalContext::new(
                FUN_COMMAND_BUFFER_DIRTY_PROPAGATION.buffer,
                FunRevision::new(22),
                FunRevision::new(2),
                8,
                FunSystemId::new(9),
                source_page.chunk_key(),
                FunCommandPhase::ApplyDirtyPropagationCommands,
            ),
        );

        assert_eq!(first_journal.digest, second_journal.digest);
        assert!(
            first_journal
                .commands
                .iter()
                .any(|command| command.kind == FunCommandKind::SpatialMarkDirty)
        );
        assert!(
            first_journal
                .commands
                .iter()
                .any(|command| command.kind == FunCommandKind::ArtifactPublish)
        );
        assert!(
            first_journal
                .commands
                .iter()
                .any(|command| command.kind == FunCommandKind::PhysicsCook)
        );
    }
}
