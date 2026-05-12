use crate::{
    AgentManifest, AgentMutationProposal, AgentProposalValidationReport, AgentProposalWorldState,
    EcsAabbF32, EcsArtifactConsumer, EcsCrossDomainWaitClass, EcsCrossDomainWaitDecision,
    EcsCrossDomainWaitPolicy, EcsCrossDomainWaitRejectReason, EcsDerivedArtifactId,
    EcsDerivedArtifactKind, EcsLuxHandoffQueue, EcsPhysicsCookQueue, EcsRendererHandoffQueue,
    EcsSpatialPageKey, EcsSpatialRegionKey, FixedStepId, FunCommandDigest, FunEntity, FunFrameId,
    FunRevision, FunSystemAccess, LuxArtifactHandoff, LuxArtifactHandoffKind,
    RendererArtifactHandoff, SnapshotGeneration, VoxelPhysicsCookRequest, WorkRequiredness,
    WorldSnapshot, validate_agent_proposal,
};
use fun_scheduler_types::ScheduleDeadline;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubsystemContractError {
    RendererPresentDependencyRejected(EcsCrossDomainWaitRejectReason),
    RendererGpuMutationAttempt,
    LuxWaitRejected(EcsCrossDomainWaitRejectReason),
    RequiredLightingMissingFallbackOrDeadline,
    PhysicsFixedStepRejected,
    RvelteWaitRejected(EcsCrossDomainWaitRejectReason),
    RequiredHudMissingFallbackOrDeadline,
    SecurityManifestScopeRequired,
    AgentProposalRejected,
    NonDeterministicAuthoritativeDelta,
}

impl SubsystemContractError {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RendererPresentDependencyRejected(_) => "renderer_present_dependency_rejected",
            Self::RendererGpuMutationAttempt => "renderer_gpu_mutation_attempt",
            Self::LuxWaitRejected(_) => "lux_wait_rejected",
            Self::RequiredLightingMissingFallbackOrDeadline => {
                "required_lighting_missing_fallback_or_deadline"
            }
            Self::PhysicsFixedStepRejected => "physics_fixed_step_rejected",
            Self::RvelteWaitRejected(_) => "rvelte_wait_rejected",
            Self::RequiredHudMissingFallbackOrDeadline => {
                "required_hud_missing_fallback_or_deadline"
            }
            Self::SecurityManifestScopeRequired => "security_manifest_scope_required",
            Self::AgentProposalRejected => "agent_proposal_rejected",
            Self::NonDeterministicAuthoritativeDelta => "non_deterministic_authoritative_delta",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderableDeclaration {
    pub entity: FunEntity,
    pub artifact_kind: EcsDerivedArtifactKind,
    pub requiredness: WorkRequiredness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererArtifactImportPlan {
    pub snapshot_generation: SnapshotGeneration,
    pub handoff_rows: Vec<RendererArtifactHandoff>,
    pub required_rows: u32,
    pub optional_rows: u32,
}

impl RendererArtifactImportPlan {
    #[must_use]
    pub fn from_snapshot_and_queue(
        snapshot: &WorldSnapshot,
        queue: &EcsRendererHandoffQueue,
    ) -> Self {
        let mut handoff_rows = queue.items.clone();
        handoff_rows.sort_by_key(|row| {
            (
                row.artifact_id,
                row.source_page.chunk_key(),
                row.source_epoch,
                row.artifact_epoch,
                row.kind as u8,
            )
        });
        let required_rows = handoff_rows
            .iter()
            .filter(|row| !row.requiredness.is_optional())
            .count() as u32;
        let optional_rows = handoff_rows.len() as u32 - required_rows;
        Self {
            snapshot_generation: snapshot.generation,
            handoff_rows,
            required_rows,
            optional_rows,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererArtifactRetirePlan {
    pub artifacts: Vec<EcsDerivedArtifactId>,
    pub intent_revision: FunRevision,
}

impl RendererArtifactRetirePlan {
    #[must_use]
    pub fn new(mut artifacts: Vec<EcsDerivedArtifactId>, intent_revision: FunRevision) -> Self {
        artifacts.sort_unstable();
        artifacts.dedup();
        Self {
            artifacts,
            intent_revision,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererFrameSnapshot {
    pub frame: FunFrameId,
    pub generation: SnapshotGeneration,
    pub world_revision: FunRevision,
    pub digest: u64,
}

impl RendererFrameSnapshot {
    #[must_use]
    pub fn from_world_snapshot(snapshot: &WorldSnapshot) -> Self {
        Self {
            frame: snapshot.frame,
            generation: snapshot.generation,
            world_revision: snapshot.world_revision,
            digest: snapshot.digest(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererPresentDependencyValidator;

impl RendererPresentDependencyValidator {
    pub fn validate(
        policy: EcsCrossDomainWaitPolicy,
    ) -> Result<EcsCrossDomainWaitDecision, SubsystemContractError> {
        if matches!(policy.class, EcsCrossDomainWaitClass::OptionalUiDiagnostics) {
            return Err(SubsystemContractError::RendererPresentDependencyRejected(
                EcsCrossDomainWaitRejectReason::OptionalUiDiagnosticsGatePresent,
            ));
        }
        if let EcsCrossDomainWaitDecision::Reject(reason) =
            policy.renderer_lux_refinement_decision()
        {
            return Err(SubsystemContractError::RendererPresentDependencyRejected(
                reason,
            ));
        }
        match policy.renderer_present_decision() {
            EcsCrossDomainWaitDecision::Reject(reason) => Err(
                SubsystemContractError::RendererPresentDependencyRejected(reason),
            ),
            decision => Ok(decision),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererEcsBridge {
    pub ecs_owns_renderable_declarations: bool,
    pub ecs_owns_artifact_manifests: bool,
    pub ecs_never_mutates_gpu_resources: bool,
}

impl RendererEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ecs_owns_renderable_declarations: true,
            ecs_owns_artifact_manifests: true,
            ecs_never_mutates_gpu_resources: true,
        }
    }

    #[must_use]
    pub fn import_plan(
        &self,
        snapshot: &WorldSnapshot,
        queue: &EcsRendererHandoffQueue,
    ) -> RendererArtifactImportPlan {
        RendererArtifactImportPlan::from_snapshot_and_queue(snapshot, queue)
    }

    #[must_use]
    pub fn retire_plan(
        &self,
        artifacts: Vec<EcsDerivedArtifactId>,
        intent_revision: FunRevision,
    ) -> RendererArtifactRetirePlan {
        RendererArtifactRetirePlan::new(artifacts, intent_revision)
    }

    pub fn validate_present_dependency(
        &self,
        policy: EcsCrossDomainWaitPolicy,
    ) -> Result<EcsCrossDomainWaitDecision, SubsystemContractError> {
        RendererPresentDependencyValidator::validate(policy)
    }

    pub const fn reject_gpu_resource_mutation(&self) -> Result<(), SubsystemContractError> {
        Err(SubsystemContractError::RendererGpuMutationAttempt)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxFallbackPolicy {
    #[default]
    None,
    PreviousLightingCache,
    CoarseProbe,
    BoundedWait(ScheduleDeadline),
}

impl LuxFallbackPolicy {
    #[must_use]
    pub const fn has_fallback_or_deadline(self) -> bool {
        match self {
            Self::None => false,
            Self::PreviousLightingCache | Self::CoarseProbe => true,
            Self::BoundedWait(deadline) => !matches!(deadline, ScheduleDeadline::None),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuxDirtyRow {
    pub source_page: EcsSpatialPageKey,
    pub kind: LuxArtifactHandoffKind,
    pub bounds_world: EcsAabbF32,
    pub revision: FunRevision,
    pub requiredness: WorkRequiredness,
}

impl LuxDirtyRow {
    #[must_use]
    pub fn from_handoff(row: LuxArtifactHandoff) -> Self {
        Self {
            source_page: row.source_page,
            kind: row.kind,
            bounds_world: row.bounds_world,
            revision: FunRevision::new(u64::from(row.dirty_epoch)),
            requiredness: row.requiredness,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct LuxDirtyRowTable {
    pub rows: Vec<LuxDirtyRow>,
}

impl LuxDirtyRowTable {
    #[must_use]
    pub fn from_handoffs(handoff_rows: &[LuxArtifactHandoff]) -> Self {
        let mut rows = handoff_rows
            .iter()
            .copied()
            .map(LuxDirtyRow::from_handoff)
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| (row.source_page.chunk_key(), row.revision, row.kind as u8));
        Self { rows }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LuxHandoffImportPlan {
    pub handoff_rows: Vec<LuxArtifactHandoff>,
    pub dirty_rows: LuxDirtyRowTable,
    pub fallback_policy: LuxFallbackPolicy,
}

impl LuxHandoffImportPlan {
    #[must_use]
    pub fn from_queue(queue: &EcsLuxHandoffQueue, fallback_policy: LuxFallbackPolicy) -> Self {
        let mut handoff_rows = queue.items.clone();
        handoff_rows
            .sort_by_key(|row| (row.source_page.chunk_key(), row.dirty_epoch, row.kind as u8));
        let dirty_rows = LuxDirtyRowTable::from_handoffs(&handoff_rows);
        Self {
            handoff_rows,
            dirty_rows,
            fallback_policy,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxEcsBridge {
    pub ecs_owns_light_declarations: bool,
    pub ecs_owns_dirty_rows: bool,
    pub lux_owns_lighting_caches: bool,
}

impl LuxEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ecs_owns_light_declarations: true,
            ecs_owns_dirty_rows: true,
            lux_owns_lighting_caches: true,
        }
    }

    #[must_use]
    pub fn import_plan(
        &self,
        queue: &EcsLuxHandoffQueue,
        fallback_policy: LuxFallbackPolicy,
    ) -> LuxHandoffImportPlan {
        LuxHandoffImportPlan::from_queue(queue, fallback_policy)
    }

    pub fn validate_wait(
        &self,
        policy: EcsCrossDomainWaitPolicy,
    ) -> Result<EcsCrossDomainWaitDecision, SubsystemContractError> {
        match policy.lux_wait_decision() {
            EcsCrossDomainWaitDecision::Reject(reason) => {
                Err(SubsystemContractError::LuxWaitRejected(reason))
            }
            decision => Ok(decision),
        }
    }

    pub const fn validate_required_lighting(
        &self,
        requiredness: WorkRequiredness,
        fallback_policy: LuxFallbackPolicy,
    ) -> Result<(), SubsystemContractError> {
        if matches!(requiredness, WorkRequiredness::Required)
            && !fallback_policy.has_fallback_or_deadline()
        {
            return Err(SubsystemContractError::RequiredLightingMissingFallbackOrDeadline);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicsProxyManifest {
    pub source_page: EcsSpatialPageKey,
    pub artifact: Option<EcsDerivedArtifactId>,
    pub generation: u32,
    pub collision_critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicsCookImportPlan {
    pub cook_requests: Vec<VoxelPhysicsCookRequest>,
    pub collision_critical: u32,
    pub optional: u32,
}

impl PhysicsCookImportPlan {
    #[must_use]
    pub fn from_queue(queue: &EcsPhysicsCookQueue) -> Self {
        let mut cook_requests = queue.items.clone();
        cook_requests.sort_by_key(|row| {
            (
                row.source_page.chunk_key(),
                row.dirty_epoch,
                row.mode as u8,
                row.requiredness.is_optional(),
            )
        });
        let collision_critical = cook_requests
            .iter()
            .filter(|row| !row.requiredness.is_optional())
            .count() as u32;
        let optional = cook_requests.len() as u32 - collision_critical;
        Self {
            cook_requests,
            collision_critical,
            optional,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicsWritebackApplyPlan {
    pub proxy_rows: Vec<PhysicsProxyManifest>,
    pub apply_revision: FunRevision,
}

impl PhysicsWritebackApplyPlan {
    #[must_use]
    pub fn new(mut proxy_rows: Vec<PhysicsProxyManifest>, apply_revision: FunRevision) -> Self {
        proxy_rows.sort_by_key(|row| {
            (
                row.source_page.chunk_key(),
                row.generation,
                row.artifact,
                row.collision_critical,
            )
        });
        Self {
            proxy_rows,
            apply_revision,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConservativePhysicsFallback {
    pub enabled: bool,
    pub fixed_step_deadline: Option<FixedStepId>,
}

impl ConservativePhysicsFallback {
    #[must_use]
    pub const fn enabled(fixed_step_deadline: Option<FixedStepId>) -> Self {
        Self {
            enabled: true,
            fixed_step_deadline,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AvisEcsBridge {
    pub ecs_owns_declarations: bool,
    pub avis_owns_hot_physics_state: bool,
    pub hot_physics_state_in_ecs: bool,
}

impl AvisEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ecs_owns_declarations: true,
            avis_owns_hot_physics_state: true,
            hot_physics_state_in_ecs: false,
        }
    }

    #[must_use]
    pub fn import_plan(&self, queue: &EcsPhysicsCookQueue) -> PhysicsCookImportPlan {
        PhysicsCookImportPlan::from_queue(queue)
    }

    pub fn fixed_step_decision(
        &self,
        policy: EcsCrossDomainWaitPolicy,
    ) -> Result<EcsCrossDomainWaitDecision, SubsystemContractError> {
        match policy.physics_fixed_step_decision() {
            EcsCrossDomainWaitDecision::Reject(_) => {
                Err(SubsystemContractError::PhysicsFixedStepRejected)
            }
            decision => Ok(decision),
        }
    }

    #[must_use]
    pub fn writeback_plan(
        &self,
        proxy_rows: Vec<PhysicsProxyManifest>,
        apply_revision: FunRevision,
    ) -> PhysicsWritebackApplyPlan {
        PhysicsWritebackApplyPlan::new(proxy_rows, apply_revision)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetworkRelevanceRow {
    pub region: EcsSpatialRegionKey,
    pub source_revision: FunRevision,
    pub relevance: u16,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NetworkRelevanceTable {
    pub rows: Vec<NetworkRelevanceRow>,
}

impl NetworkRelevanceTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, row: NetworkRelevanceRow) {
        if !self.rows.contains(&row) {
            self.rows.push(row);
            self.rows.sort_unstable();
        }
    }

    #[must_use]
    pub fn digest(&self) -> FunCommandDigest {
        let mut value = 0xcbf2_9ce4_8422_2325_u64;
        for row in &self.rows {
            value = hash_u8(value, row.region.domain as u8);
            value = hash_u64(value, row.region.grid_id.get());
            value = hash_u8(value, row.region.level);
            value = hash_i32(value, row.region.x);
            value = hash_i32(value, row.region.y);
            value = hash_i32(value, row.region.z);
            value = hash_u64(value, row.source_revision.get());
            value = hash_u64(value, u64::from(row.relevance));
        }
        FunCommandDigest { value }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkDeltaManifest {
    pub source_revision: FunRevision,
    pub rows: Vec<NetworkRelevanceRow>,
    pub digest: FunCommandDigest,
    pub authoritative: bool,
}

impl NetworkDeltaManifest {
    #[must_use]
    pub fn from_table(
        table: &NetworkRelevanceTable,
        source_revision: FunRevision,
        authoritative: bool,
    ) -> Self {
        let mut rows = table.rows.clone();
        rows.sort_unstable();
        let sorted = NetworkRelevanceTable { rows: rows.clone() };
        Self {
            source_revision,
            rows,
            digest: sorted.digest(),
            authoritative,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetworkRollbackApplyPlan {
    pub command_journal: FunCommandDigest,
    pub target_revision: FunRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PerPeerEcsBudgetBridge {
    pub peer_id: u64,
    pub requested_rows: u32,
    pub admitted_rows: u32,
    pub pressure: u8,
}

impl PerPeerEcsBudgetBridge {
    #[must_use]
    pub fn from_budget(peer_id: u64, requested_rows: u32, budget_rows: u32) -> Self {
        let admitted_rows = if requested_rows < budget_rows {
            requested_rows
        } else {
            budget_rows
        };
        let shed_rows = requested_rows.saturating_sub(admitted_rows);
        let pressure = u64::from(shed_rows)
            .saturating_mul(255)
            .checked_div(u64::from(requested_rows))
            .unwrap_or(0)
            .min(u64::from(u8::MAX));
        let pressure = u8::try_from(pressure).unwrap_or(u8::MAX);
        Self {
            peer_id,
            requested_rows,
            admitted_rows,
            pressure,
        }
    }

    #[must_use]
    pub const fn feeds_scheduler_admission(self) -> bool {
        self.requested_rows > self.admitted_rows || self.pressure > 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThunderEcsBridge {
    pub deterministic_authoritative_delta: bool,
}

impl ThunderEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            deterministic_authoritative_delta: true,
        }
    }

    pub fn delta_manifest(
        &self,
        table: &NetworkRelevanceTable,
        source_revision: FunRevision,
        authoritative: bool,
    ) -> Result<NetworkDeltaManifest, SubsystemContractError> {
        if authoritative && !self.deterministic_authoritative_delta {
            return Err(SubsystemContractError::NonDeterministicAuthoritativeDelta);
        }
        Ok(NetworkDeltaManifest::from_table(
            table,
            source_revision,
            authoritative,
        ))
    }

    #[must_use]
    pub const fn rollback_plan(
        &self,
        command_journal: FunCommandDigest,
        target_revision: FunRevision,
    ) -> NetworkRollbackApplyPlan {
        NetworkRollbackApplyPlan {
            command_journal,
            target_revision,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RvelteStateResource {
    pub generation: SnapshotGeneration,
    pub state_revision: FunRevision,
    pub exposed_to_ui: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RvelteInputEnvelope {
    pub input_sequence: u64,
    pub routing_revision: FunRevision,
    pub snapshot_generation: SnapshotGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RveltePaintPacketManifest {
    pub packet_generation: SnapshotGeneration,
    pub wait_class: EcsCrossDomainWaitClass,
    pub requiredness: WorkRequiredness,
    pub has_fallback: bool,
    pub bounded_deadline: bool,
}

impl RveltePaintPacketManifest {
    #[must_use]
    pub const fn wait_policy(self) -> EcsCrossDomainWaitPolicy {
        let mut policy = EcsCrossDomainWaitPolicy::new(
            crate::EcsCrossDomainWaitTokenKind::RveltePaintPacketReady,
            self.wait_class,
            self.requiredness,
            self.has_fallback,
        );
        if self.bounded_deadline {
            policy = policy.with_bounded_deadline();
        }
        policy
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RvelteRendererHandoffQueue {
    pub packets: Vec<RveltePaintPacketManifest>,
}

impl RvelteRendererHandoffQueue {
    pub fn push(&mut self, packet: RveltePaintPacketManifest) {
        if !self.packets.contains(&packet) {
            self.packets.push(packet);
            self.packets
                .sort_by_key(|packet| (packet.packet_generation, packet.wait_class as u8));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RvelteGenerationSnapshot {
    pub generation: SnapshotGeneration,
    pub retained_tree_revision: FunRevision,
    pub immutable_for_renderer: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RvelteEcsBridge {
    pub ecs_owns_mount_declarations: bool,
    pub rvelte_owns_retained_tree: bool,
    pub renderer_imports_immutable_packets: bool,
}

impl RvelteEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ecs_owns_mount_declarations: true,
            rvelte_owns_retained_tree: true,
            renderer_imports_immutable_packets: true,
        }
    }

    pub fn validate_wait(
        &self,
        policy: EcsCrossDomainWaitPolicy,
    ) -> Result<EcsCrossDomainWaitDecision, SubsystemContractError> {
        match policy.rvelte_wait_decision() {
            EcsCrossDomainWaitDecision::Reject(reason) => {
                Err(SubsystemContractError::RvelteWaitRejected(reason))
            }
            decision => Ok(decision),
        }
    }

    pub fn validate_renderer_present_packet(
        &self,
        packet: RveltePaintPacketManifest,
    ) -> Result<EcsCrossDomainWaitDecision, SubsystemContractError> {
        if matches!(
            packet.wait_class,
            EcsCrossDomainWaitClass::OptionalUiDiagnostics
        ) {
            return Err(SubsystemContractError::RendererPresentDependencyRejected(
                EcsCrossDomainWaitRejectReason::OptionalUiDiagnosticsGatePresent,
            ));
        }
        match packet.wait_policy().renderer_present_decision() {
            EcsCrossDomainWaitDecision::Reject(
                EcsCrossDomainWaitRejectReason::RequiredHudPaintMissingFallbackOrDeadline,
            ) => Err(SubsystemContractError::RequiredHudMissingFallbackOrDeadline),
            EcsCrossDomainWaitDecision::Reject(reason) => Err(
                SubsystemContractError::RendererPresentDependencyRejected(reason),
            ),
            decision => Ok(decision),
        }
    }

    #[must_use]
    pub const fn generation_snapshot(
        &self,
        generation: SnapshotGeneration,
        retained_tree_revision: FunRevision,
    ) -> RvelteGenerationSnapshot {
        RvelteGenerationSnapshot {
            generation,
            retained_tree_revision,
            immutable_for_renderer: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationPoseArtifactManifest {
    pub entity: FunEntity,
    pub artifact_id: EcsDerivedArtifactId,
    pub generation: SnapshotGeneration,
    pub consumer: EcsArtifactConsumer,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationEcsBridge {
    pub ecs_owns_animation_declarations: bool,
    pub animation_owns_pose_buffers: bool,
}

impl AnimationEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ecs_owns_animation_declarations: true,
            animation_owns_pose_buffers: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProposalPlan {
    pub manifest: AgentManifest,
    pub requested_access: FunSystemAccess,
    pub validation_required: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AiEcsBridge {
    pub ecs_owns_actor_declarations: bool,
    pub ai_owns_inference_queues: bool,
    pub proposals_use_agent_validation: bool,
}

impl AiEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ecs_owns_actor_declarations: true,
            ai_owns_inference_queues: true,
            proposals_use_agent_validation: true,
        }
    }

    pub fn validate_proposal<C: crate::core::FunCommandPayload>(
        &self,
        manifest: &AgentManifest,
        proposal: &AgentMutationProposal<C>,
        state: AgentProposalWorldState,
    ) -> Result<AgentProposalValidationReport, SubsystemContractError> {
        let report = validate_agent_proposal(manifest, proposal, state);
        if report.accepted {
            Ok(report)
        } else {
            Err(SubsystemContractError::AgentProposalRejected)
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WardenEcsBridge {
    pub minimal_session_integrity_state: bool,
    pub policy_outside_gameplay_mutation_authority: bool,
    pub explicit_manifest_scope_required: bool,
}

impl WardenEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            minimal_session_integrity_state: true,
            policy_outside_gameplay_mutation_authority: true,
            explicit_manifest_scope_required: true,
        }
    }

    pub const fn validate_sensitive_mutation_scope(
        &self,
        has_explicit_manifest_scope: bool,
    ) -> Result<(), SubsystemContractError> {
        if self.explicit_manifest_scope_required && !has_explicit_manifest_scope {
            return Err(SubsystemContractError::SecurityManifestScopeRequired);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TelemetryReportKind {
    Storage = 0,
    Queries = 1,
    ScheduleGraphs = 2,
    CommandBarriers = 3,
    CrossDomainWaits = 4,
    ArtifactDag = 5,
    HandoffQueues = 6,
    FrameDigest = 7,
}

impl TelemetryReportKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Storage => "storage",
            Self::Queries => "queries",
            Self::ScheduleGraphs => "schedule_graphs",
            Self::CommandBarriers => "command_barriers",
            Self::CrossDomainWaits => "cross_domain_waits",
            Self::ArtifactDag => "artifact_dag",
            Self::HandoffQueues => "handoff_queues",
            Self::FrameDigest => "frame_digest",
        }
    }
}

pub const TELEMETRY_REPORT_KINDS: [TelemetryReportKind; 8] = [
    TelemetryReportKind::Storage,
    TelemetryReportKind::Queries,
    TelemetryReportKind::ScheduleGraphs,
    TelemetryReportKind::CommandBarriers,
    TelemetryReportKind::CrossDomainWaits,
    TelemetryReportKind::ArtifactDag,
    TelemetryReportKind::HandoffQueues,
    TelemetryReportKind::FrameDigest,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TelemetryReport {
    pub kind: TelemetryReportKind,
    pub revision: FunRevision,
    pub digest: FunCommandDigest,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TelemetryEcsBridge {
    pub emits_typed_reports: bool,
}

impl TelemetryEcsBridge {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            emits_typed_reports: true,
        }
    }

    #[must_use]
    pub const fn typed_report_kinds(&self) -> &'static [TelemetryReportKind] {
        &TELEMETRY_REPORT_KINDS
    }
}

const fn hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn hash_u64(mut hash: u64, value: u64) -> u64 {
    hash = hash_u8(hash, (value & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 8) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 16) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 24) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 32) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 40) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 48) & 0xff) as u8);
    hash_u8(hash, ((value >> 56) & 0xff) as u8)
}

const fn hash_i32(hash: u64, value: i32) -> u64 {
    hash_u64(hash, value as u32 as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AgentAccessScope, EcsCrossDomainFallbackKind, EcsCrossDomainWaitTokenKind, EcsPageChannel,
        EcsSpatialDomainKind, EcsSpatialGridId, FunCommandBufferId, FunCommandDeterministicKey,
        FunCommandEnvelope, FunCommandKind, FunCommandPhase, FunResourceId, FunSubsystemCommand,
        FunSystemAccessMode, FunSystemAccessRow, FunSystemAccessTarget, FunSystemId,
        RendererArtifactHandoffKind, RendererVisibilityHint,
    };
    use fun_scheduler_types::EcsChunkKey;

    fn terrain_page(x: i32) -> EcsSpatialPageKey {
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

    fn wait_policy(
        class: EcsCrossDomainWaitClass,
        requiredness: WorkRequiredness,
        has_fallback: bool,
    ) -> EcsCrossDomainWaitPolicy {
        EcsCrossDomainWaitPolicy::new(
            EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished,
            class,
            requiredness,
            has_fallback,
        )
    }

    #[test]
    fn renderer_contract_rejects_non_present_dependencies_and_gpu_mutation() {
        let bridge = RendererEcsBridge::new();
        let optional_foliage = bridge.validate_present_dependency(wait_policy(
            EcsCrossDomainWaitClass::OptionalFoliage,
            WorkRequiredness::Optional,
            false,
        ));
        let optional_lux = bridge.validate_present_dependency(wait_policy(
            EcsCrossDomainWaitClass::LuxRefinement,
            WorkRequiredness::Optional,
            false,
        ));
        let diagnostics = bridge.validate_present_dependency(wait_policy(
            EcsCrossDomainWaitClass::OptionalUiDiagnostics,
            WorkRequiredness::Optional,
            false,
        ));
        let required = bridge
            .validate_present_dependency(wait_policy(
                EcsCrossDomainWaitClass::RequiredRenderArtifact,
                WorkRequiredness::Required,
                false,
            ))
            .expect("required render artifact can gate present");

        assert!(matches!(
            optional_foliage,
            Err(SubsystemContractError::RendererPresentDependencyRejected(
                EcsCrossDomainWaitRejectReason::NonRenderPresentDependency
            ))
        ));
        assert!(matches!(
            optional_lux,
            Err(SubsystemContractError::RendererPresentDependencyRejected(
                EcsCrossDomainWaitRejectReason::RendererWaitsOnOptionalLuxRefinement
            ))
        ));
        assert!(matches!(
            diagnostics,
            Err(SubsystemContractError::RendererPresentDependencyRejected(
                EcsCrossDomainWaitRejectReason::OptionalUiDiagnosticsGatePresent
            ))
        ));
        assert_eq!(required, EcsCrossDomainWaitDecision::Wait);
        assert_eq!(
            bridge.reject_gpu_resource_mutation(),
            Err(SubsystemContractError::RendererGpuMutationAttempt)
        );
    }

    #[test]
    fn renderer_import_plan_uses_immutable_snapshot_generation() {
        let snapshot = WorldSnapshot::new(
            SnapshotGeneration::new(9),
            FunFrameId::new(3),
            FunRevision::new(7),
        )
        .expect("snapshot");
        let mut queue = EcsRendererHandoffQueue::default();
        queue
            .push(RendererArtifactHandoff {
                artifact_id: EcsDerivedArtifactId::new(2),
                source_page: terrain_page(2),
                kind: RendererArtifactHandoffKind::TerrainSurfacePackets,
                source_epoch: 1,
                artifact_epoch: 1,
                requiredness: WorkRequiredness::Required,
                visibility_hint: RendererVisibilityHint::VisibleNear,
            })
            .expect("push handoff");
        queue
            .push(RendererArtifactHandoff {
                artifact_id: EcsDerivedArtifactId::new(1),
                source_page: terrain_page(1),
                kind: RendererArtifactHandoffKind::FoliageCardsImpostors,
                source_epoch: 1,
                artifact_epoch: 1,
                requiredness: WorkRequiredness::Optional,
                visibility_hint: RendererVisibilityHint::VisibleFar,
            })
            .expect("push handoff");

        let plan = RendererEcsBridge::new().import_plan(&snapshot, &queue);

        assert_eq!(plan.snapshot_generation, SnapshotGeneration::new(9));
        assert_eq!(plan.required_rows, 1);
        assert_eq!(plan.optional_rows, 1);
        assert_eq!(
            plan.handoff_rows[0].artifact_id,
            EcsDerivedArtifactId::new(1)
        );
    }

    #[test]
    fn lux_contract_blocks_present_waits_and_requires_fallback_for_required_artifacts() {
        let bridge = LuxEcsBridge::new();
        let wait = bridge.validate_wait(wait_policy(
            EcsCrossDomainWaitClass::RendererPresent,
            WorkRequiredness::Required,
            false,
        ));

        assert!(matches!(
            wait,
            Err(SubsystemContractError::LuxWaitRejected(
                EcsCrossDomainWaitRejectReason::LuxWaitsOnRendererPresent
            ))
        ));
        assert_eq!(
            bridge.validate_required_lighting(WorkRequiredness::Required, LuxFallbackPolicy::None),
            Err(SubsystemContractError::RequiredLightingMissingFallbackOrDeadline)
        );
        assert_eq!(
            bridge.validate_required_lighting(
                WorkRequiredness::Required,
                LuxFallbackPolicy::BoundedWait(ScheduleDeadline::Frame)
            ),
            Ok(())
        );
    }

    #[test]
    fn avis_contract_keeps_hot_physics_out_of_ecs_and_uses_conservative_fallbacks() {
        let bridge = AvisEcsBridge::new();
        let noncritical = bridge
            .fixed_step_decision(wait_policy(
                EcsCrossDomainWaitClass::NonCriticalPhysicsCook,
                WorkRequiredness::Optional,
                false,
            ))
            .expect("noncritical physics uses fallback");
        let critical = bridge
            .fixed_step_decision(wait_policy(
                EcsCrossDomainWaitClass::CollisionCriticalTerrainProxy,
                WorkRequiredness::Required,
                false,
            ))
            .expect("critical proxy can wait");

        assert!(!bridge.hot_physics_state_in_ecs);
        assert_eq!(
            noncritical,
            EcsCrossDomainWaitDecision::UseFallback(
                EcsCrossDomainFallbackKind::ConservativePhysicsProxy
            )
        );
        assert_eq!(critical, EcsCrossDomainWaitDecision::Wait);
    }

    #[test]
    fn thunder_delta_manifest_is_authoritative_and_deterministic() {
        let mut first = NetworkRelevanceTable::new();
        first.push(NetworkRelevanceRow {
            region: EcsSpatialRegionKey::from_page(terrain_page(2), 4),
            source_revision: FunRevision::new(11),
            relevance: 70,
        });
        first.push(NetworkRelevanceRow {
            region: EcsSpatialRegionKey::from_page(terrain_page(1), 4),
            source_revision: FunRevision::new(11),
            relevance: 90,
        });

        let mut second = NetworkRelevanceTable::new();
        second.push(first.rows[1]);
        second.push(first.rows[0]);

        let bridge = ThunderEcsBridge::new();
        let first_manifest = bridge
            .delta_manifest(&first, FunRevision::new(11), true)
            .expect("deterministic authoritative manifest");
        let second_manifest = bridge
            .delta_manifest(&second, FunRevision::new(11), true)
            .expect("deterministic authoritative manifest");
        let budget = PerPeerEcsBudgetBridge::from_budget(42, 100, 25);

        assert_eq!(first_manifest.digest, second_manifest.digest);
        assert!(first_manifest.authoritative);
        assert!(budget.feeds_scheduler_admission());
        assert!(budget.pressure > 0);
        assert_eq!(
            ThunderEcsBridge {
                deterministic_authoritative_delta: false
            }
            .delta_manifest(&first, FunRevision::new(11), true),
            Err(SubsystemContractError::NonDeterministicAuthoritativeDelta)
        );
    }

    #[test]
    fn rvelte_contract_separates_mutation_from_renderer_present() {
        let bridge = RvelteEcsBridge::new();
        let retained_mutation = bridge.validate_wait(wait_policy(
            EcsCrossDomainWaitClass::RendererPresent,
            WorkRequiredness::Required,
            false,
        ));
        let required_hud = RveltePaintPacketManifest {
            packet_generation: SnapshotGeneration::new(4),
            wait_class: EcsCrossDomainWaitClass::RequiredHudPaintPacket,
            requiredness: WorkRequiredness::Required,
            has_fallback: false,
            bounded_deadline: false,
        };
        let bounded_hud = RveltePaintPacketManifest {
            bounded_deadline: true,
            ..required_hud
        };
        let diagnostics = RveltePaintPacketManifest {
            packet_generation: SnapshotGeneration::new(5),
            wait_class: EcsCrossDomainWaitClass::OptionalUiDiagnostics,
            requiredness: WorkRequiredness::Optional,
            has_fallback: false,
            bounded_deadline: false,
        };
        let snapshot = bridge.generation_snapshot(SnapshotGeneration::new(6), FunRevision::new(20));

        assert!(matches!(
            retained_mutation,
            Err(SubsystemContractError::RvelteWaitRejected(
                EcsCrossDomainWaitRejectReason::RvelteWaitsOnRendererPresent
            ))
        ));
        assert_eq!(
            bridge.validate_renderer_present_packet(required_hud),
            Err(SubsystemContractError::RequiredHudMissingFallbackOrDeadline)
        );
        assert_eq!(
            bridge
                .validate_renderer_present_packet(bounded_hud)
                .expect("bounded HUD can gate present"),
            EcsCrossDomainWaitDecision::Wait
        );
        assert!(matches!(
            bridge.validate_renderer_present_packet(diagnostics),
            Err(SubsystemContractError::RendererPresentDependencyRejected(
                EcsCrossDomainWaitRejectReason::OptionalUiDiagnosticsGatePresent
            ))
        ));
        assert!(snapshot.immutable_for_renderer);
    }

    #[test]
    fn animation_ai_warden_and_telemetry_contracts_are_explicit() {
        let animation = AnimationEcsBridge::new();
        let ai = AiEcsBridge::new();
        let warden = WardenEcsBridge::new();
        let telemetry = TelemetryEcsBridge::new();

        assert!(animation.ecs_owns_animation_declarations);
        assert!(animation.animation_owns_pose_buffers);
        assert!(ai.proposals_use_agent_validation);
        assert_eq!(
            warden.validate_sensitive_mutation_scope(false),
            Err(SubsystemContractError::SecurityManifestScopeRequired)
        );
        assert_eq!(warden.validate_sensitive_mutation_scope(true), Ok(()));
        assert_eq!(telemetry.typed_report_kinds().len(), 8);
        assert!(
            telemetry
                .typed_report_kinds()
                .contains(&TelemetryReportKind::CrossDomainWaits)
        );
    }

    #[test]
    fn ai_bridge_uses_agent_mutation_validation() {
        let resource = FunResourceId::new(77);
        let buffer = FunCommandBufferId::new(3);
        let access = FunSystemAccess::new().with_row(FunSystemAccessRow::new(
            FunSystemAccessTarget::Resource(resource),
            FunSystemAccessMode::Write,
        ));
        let manifest = AgentManifest {
            id: crate::EcsAgentManifestId::new(1),
            system: crate::EcsSystemId::new(2),
            scope: AgentAccessScope::new(access.clone())
                .with_command_buffer(buffer)
                .allow_required_path_mutation(),
        };
        let key = FunCommandDeterministicKey::new(
            FunRevision::new(10),
            1,
            FunSystemId::new(4),
            EcsChunkKey::new(5),
            1,
        )
        .with_resource(resource);
        let command = FunSubsystemCommand {
            kind: FunCommandKind::AgentProposal,
            target_resource: Some(resource),
            payload_digest: 9,
        };
        let envelope = FunCommandEnvelope::new(
            buffer,
            FunRevision::new(10),
            key,
            FunCommandPhase::Emit,
            command,
        );
        let proposal = AgentMutationProposal {
            observed_revision: FunRevision::new(10),
            requested_access: access,
            phase: FunCommandPhase::Emit,
            command: envelope,
        };
        let state = AgentProposalWorldState {
            revision: FunRevision::new(10),
            entity_generation: None,
            page_generation: None,
            artifact_generation: None,
        };

        let report = AiEcsBridge::new()
            .validate_proposal(&manifest, &proposal, state)
            .expect("agent validation accepts scoped proposal");

        assert!(report.accepted);
    }
}
