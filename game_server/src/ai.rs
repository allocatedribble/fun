use bevy::prelude::*;
use fun_ai_core::{
    AiActorId, AiCancellationKey, AiDeadline, AiJobId, AiLodDecision, AiLodPolicy, AiLodSignals,
    AiLodTier, AiPrivacyClass, AiTaskKind, AiTaskPriority, AuthorityTier, ModelBackend, ModelId,
    ModelRoutePolicy, TokenBudget,
};
use fun_ai_inference::{
    AdmissionController, InferenceProvider, InferenceQueue, InferenceRequest, InferenceResponse,
    InferenceResponseStatus, MockInferenceProvider, ProviderHealth, QueueAdmission,
};
use fun_ai_tactics::{MoraleState, SquadRole, TacticalIntent};

const DEFAULT_SERVER_AI_MODEL_ID: &str = "fun.server.ai.mock.v1";
const SERVER_AI_MODEL_INPUT_TOKENS: u32 = 256;
const SERVER_AI_MODEL_OUTPUT_TOKENS: u32 = 64;
const SERVER_AI_MODEL_QUEUE_HARD_CAPACITY: usize = 32;
const SERVER_AI_MODEL_QUEUE_SOFT_CAPACITY: usize = 24;
const SERVER_AI_MAX_COMPLETED_RESULTS: usize = 64;
const SERVER_AI_MAX_VALIDATED_PROPOSALS: usize = 64;
const SERVER_AI_MAX_JOBS_PER_TICK: usize = 2;
const SERVER_AI_MODEL_JOB_DEADLINE_TICKS: u64 = 4;
#[cfg(test)]
const SERVER_AI_DEFAULT_SEED: u64 = 0x5A17_5EED;
const SERVER_AI_ACCEPTED_INPUTS: [ServerAiBoundaryInputKind; 4] = [
    ServerAiBoundaryInputKind::TypedPlayerInput,
    ServerAiBoundaryInputKind::ValidatedEditorCommand,
    ServerAiBoundaryInputKind::ValidatedModelProposal,
    ServerAiBoundaryInputKind::ServerOwnedStateChange,
];
const SERVER_AI_REJECTED_INPUTS: [ServerAiBoundaryInputKind; 4] = [
    ServerAiBoundaryInputKind::ClientModelOutput,
    ServerAiBoundaryInputKind::RawSpeechTranscript,
    ServerAiBoundaryInputKind::RawUserText,
    ServerAiBoundaryInputKind::RawModelOutput,
];

pub struct FunAiServerPlugin;

impl Plugin for FunAiServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerAiAuthority>()
            .init_resource::<ServerAiFrame>()
            .init_resource::<ServerAiModelRuntime>()
            .init_resource::<ServerAiModelResults>()
            .init_resource::<ServerAiValidatedProposals>()
            .init_resource::<ServerAiSquadCoordinatorState>()
            .add_systems(Startup, assert_server_ai_runtime_contract)
            .add_systems(
                Update,
                (
                    advance_server_ai_frame,
                    assign_ai_lod,
                    update_ai_perception,
                    score_ai_goals,
                    advance_ai_plans,
                    coordinate_squads,
                    collect_server_ai_jobs,
                    poll_server_ai_results,
                    validate_server_ai_proposals,
                    apply_validated_ai_state,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerAiBoundaryInputKind {
    TypedPlayerInput,
    ValidatedEditorCommand,
    ValidatedModelProposal,
    ServerOwnedStateChange,
    ClientModelOutput,
    RawSpeechTranscript,
    RawUserText,
    RawModelOutput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerAiAuthorityInput {
    TypedPlayerInput,
    ValidatedEditorCommand,
    ValidatedModelProposal,
    ServerOwnedStateChange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerAiInputRejection {
    UntrustedClientModelOutput,
    RawSpeechTranscript,
    RawUserText,
    RawModelOutput,
}

pub fn validate_server_ai_boundary(
    input_kind: ServerAiBoundaryInputKind,
) -> Result<ServerAiAuthorityInput, ServerAiInputRejection> {
    match input_kind {
        ServerAiBoundaryInputKind::TypedPlayerInput => Ok(ServerAiAuthorityInput::TypedPlayerInput),
        ServerAiBoundaryInputKind::ValidatedEditorCommand => {
            Ok(ServerAiAuthorityInput::ValidatedEditorCommand)
        }
        ServerAiBoundaryInputKind::ValidatedModelProposal => {
            Ok(ServerAiAuthorityInput::ValidatedModelProposal)
        }
        ServerAiBoundaryInputKind::ServerOwnedStateChange => {
            Ok(ServerAiAuthorityInput::ServerOwnedStateChange)
        }
        ServerAiBoundaryInputKind::ClientModelOutput => {
            Err(ServerAiInputRejection::UntrustedClientModelOutput)
        }
        ServerAiBoundaryInputKind::RawSpeechTranscript => {
            Err(ServerAiInputRejection::RawSpeechTranscript)
        }
        ServerAiBoundaryInputKind::RawUserText => Err(ServerAiInputRejection::RawUserText),
        ServerAiBoundaryInputKind::RawModelOutput => Err(ServerAiInputRejection::RawModelOutput),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Resource)]
pub struct ServerAiAuthority {
    owns_npc_goals: bool,
    owns_combat_tactics: bool,
    owns_squad_coordination: bool,
    owns_life_simulation: bool,
    owns_memory_writes: bool,
    owns_dialogue_eligibility: bool,
    owns_model_proposal_validation: bool,
}

impl ServerAiAuthority {
    pub const fn owns_authoritative_state(self) -> bool {
        self.owns_npc_goals
            && self.owns_combat_tactics
            && self.owns_squad_coordination
            && self.owns_life_simulation
            && self.owns_memory_writes
            && self.owns_dialogue_eligibility
            && self.owns_model_proposal_validation
    }
}

impl Default for ServerAiAuthority {
    fn default() -> Self {
        Self {
            owns_npc_goals: true,
            owns_combat_tactics: true,
            owns_squad_coordination: true,
            owns_life_simulation: true,
            owns_memory_writes: true,
            owns_dialogue_eligibility: true,
            owns_model_proposal_validation: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Resource)]
pub struct ServerAiFrame {
    tick: u64,
}

impl ServerAiFrame {
    pub const fn tick(self) -> u64 {
        self.tick
    }
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct ServerAiActor {
    actor_id: AiActorId,
    lod_signals: AiLodSignals,
    seed: u64,
    despawned: bool,
}

impl ServerAiActor {
    #[allow(dead_code)]
    pub const fn new(actor_id: AiActorId, lod_signals: AiLodSignals, seed: u64) -> Self {
        Self {
            actor_id,
            lod_signals,
            seed,
            despawned: false,
        }
    }

    pub fn actor_id(&self) -> &AiActorId {
        &self.actor_id
    }

    pub const fn lod_signals(&self) -> AiLodSignals {
        self.lod_signals
    }

    pub const fn seed(&self) -> u64 {
        self.seed
    }

    pub const fn despawned(&self) -> bool {
        self.despawned
    }

    #[cfg(test)]
    const fn mark_despawned(mut self) -> Self {
        self.despawned = true;
        self
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiLod {
    tier: AiLodTier,
    decision: AiLodDecision,
    last_evaluated_tick: u64,
}

impl ServerAiLod {
    pub const fn new(tier: AiLodTier) -> Self {
        Self {
            tier,
            decision: AiLodDecision::new(tier, tier.update_interval_frames()),
            last_evaluated_tick: 0,
        }
    }

    pub const fn tier(self) -> AiLodTier {
        self.tier
    }

    pub const fn allows_model_queue(self) -> bool {
        self.decision.model_queue_allowed()
    }
}

impl Default for ServerAiLod {
    fn default() -> Self {
        Self::new(AiLodTier::Lod4Dormant)
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiPerception {
    threat_level: u16,
    visible_enemy_count: u16,
    last_observed_tick: u64,
}

impl ServerAiPerception {
    pub const fn new(threat_level: u16, visible_enemy_count: u16, last_observed_tick: u64) -> Self {
        Self {
            threat_level,
            visible_enemy_count,
            last_observed_tick,
        }
    }

    pub const fn threat_level(self) -> u16 {
        self.threat_level
    }
}

impl Default for ServerAiPerception {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ServerAiGoalKind {
    #[default]
    Hold,
    Patrol,
    Engage,
    SeekCover,
    Regroup,
    Retreat,
}

#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerAiGoalState {
    active_goal: ServerAiGoalKind,
    score: u16,
    last_scored_tick: u64,
}

impl ServerAiGoalState {
    pub const fn active_goal(self) -> ServerAiGoalKind {
        self.active_goal
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiTacticalState {
    intent: TacticalIntent,
    morale: MoraleState,
    utility_score: u16,
    last_decision_tick: u64,
}

impl ServerAiTacticalState {
    pub const fn new(intent: TacticalIntent, morale: MoraleState) -> Self {
        Self {
            intent,
            morale,
            utility_score: 0,
            last_decision_tick: 0,
        }
    }

    pub const fn intent(self) -> TacticalIntent {
        self.intent
    }

    pub const fn morale(self) -> MoraleState {
        self.morale
    }
}

impl Default for ServerAiTacticalState {
    fn default() -> Self {
        Self::new(TacticalIntent::Hold, MoraleState::Steady)
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiPlanState {
    current_intent: TacticalIntent,
    last_advanced_tick: u64,
}

impl Default for ServerAiPlanState {
    fn default() -> Self {
        Self {
            current_intent: TacticalIntent::Hold,
            last_advanced_tick: 0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiLifeState {
    need_pressure: u16,
    last_simulated_tick: u64,
}

impl Default for ServerAiLifeState {
    fn default() -> Self {
        Self {
            need_pressure: 250,
            last_simulated_tick: 0,
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerAiDialogueEligibility {
    eligible: bool,
    last_changed_tick: u64,
}

impl ServerAiDialogueEligibility {
    pub const fn eligible(self) -> bool {
        self.eligible
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiSquadMembership {
    squad_index: u16,
    role: SquadRole,
}

impl ServerAiSquadMembership {
    #[allow(dead_code)]
    pub const fn new(squad_index: u16, role: SquadRole) -> Self {
        Self { squad_index, role }
    }

    pub const fn role(self) -> SquadRole {
        self.role
    }
}

#[derive(Component, Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiLastValidatedProposal {
    raw_text_hash: u64,
    authority_tier: AuthorityTier,
    applied_tick: u64,
}

impl ServerAiLastValidatedProposal {
    #[allow(dead_code)]
    pub const fn authority_tier(self) -> AuthorityTier {
        self.authority_tier
    }
}

#[derive(Clone, Debug, Resource)]
pub struct ServerAiModelRuntime {
    provider: MockInferenceProvider,
    model_id: ModelId,
    queue: InferenceQueue,
    pending_jobs: Vec<ServerAiPendingJob>,
    dialogue_generation_enabled: bool,
    max_jobs_per_tick: usize,
}

impl ServerAiModelRuntime {
    pub fn health(&self) -> ProviderHealth {
        self.provider.health()
    }

    #[cfg(test)]
    fn enable_dialogue_generation(&mut self) {
        self.dialogue_generation_enabled = true;
    }

    fn pending_job_for(&self, job_id: &AiJobId) -> Option<&ServerAiPendingJob> {
        self.pending_jobs
            .iter()
            .find(|pending| pending.job_id == *job_id)
    }

    fn remove_pending_job(&mut self, job_id: &AiJobId) -> Option<ServerAiPendingJob> {
        let index = self
            .pending_jobs
            .iter()
            .position(|pending| pending.job_id == *job_id)?;
        Some(self.pending_jobs.swap_remove(index))
    }
}

impl Default for ServerAiModelRuntime {
    fn default() -> Self {
        let model_id =
            ModelId::new(DEFAULT_SERVER_AI_MODEL_ID).expect("static server AI model id is valid");
        let mut provider = MockInferenceProvider::new(model_id.clone());
        provider
            .load_model(&model_id)
            .expect("mock server AI model loads once during resource initialization");
        Self {
            provider,
            model_id,
            queue: InferenceQueue::new(AdmissionController::new(
                SERVER_AI_MODEL_QUEUE_HARD_CAPACITY,
                SERVER_AI_MODEL_QUEUE_SOFT_CAPACITY,
                SERVER_AI_MODEL_INPUT_TOKENS,
                SERVER_AI_MODEL_OUTPUT_TOKENS,
            )),
            pending_jobs: Vec::with_capacity(SERVER_AI_MODEL_QUEUE_HARD_CAPACITY),
            dialogue_generation_enabled: false,
            max_jobs_per_tick: SERVER_AI_MAX_JOBS_PER_TICK,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ServerAiPendingJob {
    job_id: AiJobId,
    actor_entity: Entity,
    actor_id_hash: u64,
    task_kind: AiTaskKind,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct ServerAiModelResults {
    results: Vec<ServerAiModelResult>,
}

impl ServerAiModelResults {
    fn push_bounded(&mut self, result: ServerAiModelResult) {
        if self.results.len() >= SERVER_AI_MAX_COMPLETED_RESULTS {
            self.results.remove(0);
        }
        self.results.push(result);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerAiModelResult {
    actor_entity: Entity,
    actor_id_hash: u64,
    task_kind: AiTaskKind,
    response: InferenceResponse,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct ServerAiValidatedProposals {
    proposals: Vec<ServerAiValidatedProposal>,
}

impl ServerAiValidatedProposals {
    fn push_bounded(&mut self, proposal: ServerAiValidatedProposal) {
        if self.proposals.len() >= SERVER_AI_MAX_VALIDATED_PROPOSALS {
            self.proposals.remove(0);
        }
        self.proposals.push(proposal);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerAiValidatedProposal {
    actor_entity: Entity,
    actor_id_hash: u64,
    task_kind: AiTaskKind,
    raw_text_hash: u64,
    authority_tier: AuthorityTier,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Resource)]
pub struct ServerAiSquadCoordinatorState {
    coordinated_members: u32,
    regroup_orders: u32,
    suppress_orders: u32,
}

fn advance_server_ai_frame(mut frame: ResMut<ServerAiFrame>) {
    frame.tick = frame.tick.saturating_add(1);
}

fn assert_server_ai_runtime_contract(
    authority: Res<ServerAiAuthority>,
    runtime: Res<ServerAiModelRuntime>,
) {
    assert!(authority.owns_authoritative_state());
    assert_eq!(runtime.health().backend(), ModelBackend::Mock);
    assert_eq!(runtime.model_id.as_str(), DEFAULT_SERVER_AI_MODEL_ID);
    for input_kind in SERVER_AI_ACCEPTED_INPUTS {
        assert!(validate_server_ai_boundary(input_kind).is_ok());
    }
    for input_kind in SERVER_AI_REJECTED_INPUTS {
        assert!(validate_server_ai_boundary(input_kind).is_err());
    }
}

pub fn assign_ai_lod(
    frame: Res<ServerAiFrame>,
    policy: Local<AiLodPolicy>,
    mut actors: Query<(&ServerAiActor, &mut ServerAiLod)>,
) {
    for (actor, mut lod) in &mut actors {
        if actor.despawned() {
            lod.tier = AiLodTier::Lod4Dormant;
            lod.decision = AiLodDecision::new(AiLodTier::Lod4Dormant, u16::MAX);
            lod.last_evaluated_tick = frame.tick();
            continue;
        }
        let decision = policy.decide(lod.tier, actor.lod_signals());
        lod.tier = decision.tier();
        lod.decision = decision;
        lod.last_evaluated_tick = frame.tick();
    }
}

pub fn update_ai_perception(
    frame: Res<ServerAiFrame>,
    mut actors: Query<(&ServerAiActor, &ServerAiLod, &mut ServerAiPerception)>,
) {
    for (actor, lod, mut perception) in &mut actors {
        let base_threat: u16 = if actor.lod_signals().combat_threat() {
            760
        } else if matches!(
            lod.tier(),
            AiLodTier::Lod0Spotlight | AiLodTier::Lod1NearActive
        ) {
            240
        } else {
            0
        };
        let jitter = deterministic_jitter(actor.seed(), frame.tick(), 90);
        perception.threat_level = base_threat.saturating_add(jitter).min(1000);
        perception.visible_enemy_count = if perception.threat_level >= 500 { 1 } else { 0 };
        perception.last_observed_tick = frame.tick();
    }
}

pub fn score_ai_goals(
    frame: Res<ServerAiFrame>,
    mut query: Query<(
        &ServerAiLod,
        &ServerAiPerception,
        &ServerAiTacticalState,
        &mut ServerAiGoalState,
    )>,
) {
    for (lod, perception, tactics, mut goals) in &mut query {
        let (active_goal, score) = deterministic_goal(lod.tier(), perception, tactics.morale());
        goals.active_goal = active_goal;
        goals.score = score;
        goals.last_scored_tick = frame.tick();
    }
}

pub fn advance_ai_plans(
    frame: Res<ServerAiFrame>,
    mut query: Query<(
        &ServerAiLod,
        &ServerAiGoalState,
        &mut ServerAiTacticalState,
        &mut ServerAiPlanState,
        &mut ServerAiLifeState,
        &mut ServerAiDialogueEligibility,
    )>,
) {
    for (lod, goals, mut tactics, mut plan, mut life, mut dialogue) in &mut query {
        let intent = deterministic_intent(goals.active_goal(), tactics.morale());
        tactics.intent = intent;
        tactics.utility_score = goals.score;
        tactics.last_decision_tick = frame.tick();
        plan.current_intent = intent;
        plan.last_advanced_tick = frame.tick();
        life.need_pressure = life.need_pressure.saturating_add(1).min(1000);
        life.last_simulated_tick = frame.tick();
        let eligible = lod.decision.dialogue_queue_allowed()
            && !matches!(
                intent,
                TacticalIntent::Retreat | TacticalIntent::Suppress | TacticalIntent::Flank
            );
        if dialogue.eligible != eligible {
            dialogue.eligible = eligible;
            dialogue.last_changed_tick = frame.tick();
        }
    }
}

pub fn coordinate_squads(
    mut state: ResMut<ServerAiSquadCoordinatorState>,
    mut members: Query<(&ServerAiSquadMembership, &mut ServerAiTacticalState)>,
) {
    let mut coordinated_members = 0_u32;
    let mut regroup_orders = 0_u32;
    let mut suppress_orders = 0_u32;
    for (membership, mut tactics) in &mut members {
        coordinated_members = coordinated_members.saturating_add(1);
        if tactics.morale().should_retreat() {
            tactics.intent = TacticalIntent::Regroup;
            regroup_orders = regroup_orders.saturating_add(1);
            continue;
        }
        if matches!(membership.role(), SquadRole::Support | SquadRole::Overwatch)
            && matches!(
                tactics.intent(),
                TacticalIntent::Hold | TacticalIntent::SeekCover
            )
        {
            tactics.intent = TacticalIntent::Suppress;
            suppress_orders = suppress_orders.saturating_add(1);
        }
    }
    state.coordinated_members = coordinated_members;
    state.regroup_orders = regroup_orders;
    state.suppress_orders = suppress_orders;
}

pub fn collect_server_ai_jobs(
    frame: Res<ServerAiFrame>,
    mut runtime: ResMut<ServerAiModelRuntime>,
    actors: Query<(
        Entity,
        &ServerAiActor,
        &ServerAiLod,
        &ServerAiDialogueEligibility,
    )>,
) {
    if !runtime.dialogue_generation_enabled {
        return;
    }
    let now_tick = frame.tick();
    for (entity, actor, lod, dialogue) in &actors {
        if !dialogue.eligible() || !lod.allows_model_queue() {
            continue;
        }
        let actor_id_hash = stable_actor_hash(actor.actor_id());
        if runtime
            .pending_jobs
            .iter()
            .any(|pending| pending.actor_id_hash == actor_id_hash)
        {
            continue;
        }
        let request = match server_dialogue_request(actor, now_tick) {
            Ok(request) => request,
            Err(_) => continue,
        };
        let job_id = request.job_id().clone();
        match runtime.queue.enqueue(request, now_tick) {
            Ok(
                QueueAdmission::Accepted { .. }
                | QueueAdmission::Downgraded { .. }
                | QueueAdmission::Preempted { .. },
            ) => runtime.pending_jobs.push(ServerAiPendingJob {
                job_id,
                actor_entity: entity,
                actor_id_hash,
                task_kind: AiTaskKind::NpcDialogue,
            }),
            Err(_) => continue,
        }
    }
}

pub fn poll_server_ai_results(
    frame: Res<ServerAiFrame>,
    mut runtime: ResMut<ServerAiModelRuntime>,
    mut results: ResMut<ServerAiModelResults>,
) {
    for _ in 0..runtime.max_jobs_per_tick {
        let Some(request) = runtime.queue.pop_next(frame.tick()) else {
            break;
        };
        let Some(pending) = runtime.pending_job_for(request.job_id()).cloned() else {
            continue;
        };
        let Ok(response) = runtime.provider.generate(&request) else {
            runtime.remove_pending_job(request.job_id());
            continue;
        };
        runtime.remove_pending_job(request.job_id());
        results.push_bounded(ServerAiModelResult {
            actor_entity: pending.actor_entity,
            actor_id_hash: pending.actor_id_hash,
            task_kind: pending.task_kind,
            response,
        });
    }
}

pub fn validate_server_ai_proposals(
    mut results: ResMut<ServerAiModelResults>,
    mut proposals: ResMut<ServerAiValidatedProposals>,
) {
    let mut pending_results = Vec::new();
    std::mem::swap(&mut pending_results, &mut results.results);
    for result in pending_results {
        if !valid_model_result_for_server(&result) {
            continue;
        }
        proposals.push_bounded(ServerAiValidatedProposal {
            actor_entity: result.actor_entity,
            actor_id_hash: result.actor_id_hash,
            task_kind: result.task_kind,
            raw_text_hash: result.response.raw_text_hash(),
            authority_tier: AuthorityTier::Proposal,
        });
    }
}

pub fn apply_validated_ai_state(
    frame: Res<ServerAiFrame>,
    mut commands: Commands,
    mut proposals: ResMut<ServerAiValidatedProposals>,
) {
    let mut pending_proposals = Vec::new();
    std::mem::swap(&mut pending_proposals, &mut proposals.proposals);
    for proposal in pending_proposals {
        commands
            .entity(proposal.actor_entity)
            .insert(ServerAiLastValidatedProposal {
                raw_text_hash: proposal.raw_text_hash,
                authority_tier: proposal.authority_tier,
                applied_tick: frame.tick(),
            });
    }
}

fn deterministic_goal(
    tier: AiLodTier,
    perception: &ServerAiPerception,
    morale: MoraleState,
) -> (ServerAiGoalKind, u16) {
    if morale.should_retreat() {
        return (ServerAiGoalKind::Retreat, 1000);
    }
    if matches!(morale, MoraleState::Shaken) {
        return (ServerAiGoalKind::Regroup, 640);
    }
    if perception.threat_level() >= 700 {
        return (ServerAiGoalKind::Engage, perception.threat_level());
    }
    if perception.threat_level() >= 450 {
        return (ServerAiGoalKind::SeekCover, perception.threat_level());
    }
    match tier {
        AiLodTier::Lod0Spotlight | AiLodTier::Lod1NearActive => (ServerAiGoalKind::Patrol, 380),
        AiLodTier::Lod2AreaSim => (ServerAiGoalKind::Hold, 220),
        AiLodTier::Lod3FarAggregate | AiLodTier::Lod4Dormant => (ServerAiGoalKind::Hold, 80),
    }
}

fn deterministic_intent(goal: ServerAiGoalKind, morale: MoraleState) -> TacticalIntent {
    if morale.should_retreat() {
        return TacticalIntent::Retreat;
    }
    match goal {
        ServerAiGoalKind::Hold => TacticalIntent::Hold,
        ServerAiGoalKind::Patrol => TacticalIntent::Investigate,
        ServerAiGoalKind::Engage => TacticalIntent::Suppress,
        ServerAiGoalKind::SeekCover => TacticalIntent::SeekCover,
        ServerAiGoalKind::Regroup => TacticalIntent::Regroup,
        ServerAiGoalKind::Retreat => TacticalIntent::Retreat,
    }
}

fn valid_model_result_for_server(result: &ServerAiModelResult) -> bool {
    result.response.status() == InferenceResponseStatus::Completed
        && result
            .response
            .validation_status()
            .authoritative_payload_allowed()
        && validate_server_ai_boundary(ServerAiBoundaryInputKind::ValidatedModelProposal).is_ok()
}

fn server_dialogue_request(
    actor: &ServerAiActor,
    now_tick: u64,
) -> Result<InferenceRequest, fun_ai_inference::ProviderError> {
    let job_id = AiJobId::new(format!(
        "server.ai.dialogue.{}.{}",
        actor.actor_id().as_str(),
        now_tick
    ))?;
    let cancellation_key =
        AiCancellationKey::new(format!("server.ai.actor.{}", actor.actor_id().as_str()))?;
    InferenceRequest::builder(
        job_id,
        AiTaskKind::NpcDialogue,
        "server-owned npc dialogue proposal context",
        cancellation_key,
    )
    .priority(AiTaskPriority::Low)
    .deadline(AiDeadline::new(
        now_tick.saturating_add(SERVER_AI_MODEL_JOB_DEADLINE_TICKS),
    ))
    .route_policy(ModelRoutePolicy::local_only())
    .privacy_class(AiPrivacyClass::RuntimeLocal)
    .max_input_tokens(TokenBudget::new(SERVER_AI_MODEL_INPUT_TOKENS)?)
    .max_output_tokens(TokenBudget::new(SERVER_AI_MODEL_OUTPUT_TOKENS)?)
    .temperature_milli(0)
    .top_p_milli(1000)
    .seed(actor.seed())
    .build()
}

fn deterministic_jitter(seed: u64, tick: u64, max_exclusive: u16) -> u16 {
    if max_exclusive == 0 {
        return 0;
    }
    let value = seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(tick.rotate_left(17))
        .wrapping_mul(0xBF58_476D_1CE4_E5B9);
    u16::try_from(value % u64::from(max_exclusive)).expect("jitter is bounded")
}

fn stable_actor_hash(actor_id: &AiActorId) -> u64 {
    actor_id
        .as_str()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
}

trait ServerStructuredValidation {
    fn authoritative_payload_allowed(self) -> bool;
}

impl ServerStructuredValidation for fun_ai_inference::StructuredValidationStatus {
    fn authoritative_payload_allowed(self) -> bool {
        matches!(self, Self::NotRequired | Self::Valid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor_id(value: &str) -> AiActorId {
        AiActorId::new(String::from(value)).expect("test actor id")
    }

    fn server_actor(value: &str, signals: AiLodSignals) -> ServerAiActor {
        ServerAiActor::new(actor_id(value), signals, SERVER_AI_DEFAULT_SEED)
    }

    #[test]
    fn server_ai_authority_owns_all_authoritative_surfaces() {
        assert!(ServerAiAuthority::default().owns_authoritative_state());
    }

    #[test]
    fn server_ai_boundary_rejects_raw_inputs() {
        assert_eq!(
            validate_server_ai_boundary(ServerAiBoundaryInputKind::TypedPlayerInput),
            Ok(ServerAiAuthorityInput::TypedPlayerInput)
        );
        assert_eq!(
            validate_server_ai_boundary(ServerAiBoundaryInputKind::ValidatedEditorCommand),
            Ok(ServerAiAuthorityInput::ValidatedEditorCommand)
        );
        assert_eq!(
            validate_server_ai_boundary(ServerAiBoundaryInputKind::ValidatedModelProposal),
            Ok(ServerAiAuthorityInput::ValidatedModelProposal)
        );
        assert_eq!(
            validate_server_ai_boundary(ServerAiBoundaryInputKind::ClientModelOutput),
            Err(ServerAiInputRejection::UntrustedClientModelOutput)
        );
        assert_eq!(
            validate_server_ai_boundary(ServerAiBoundaryInputKind::RawSpeechTranscript),
            Err(ServerAiInputRejection::RawSpeechTranscript)
        );
        assert_eq!(
            validate_server_ai_boundary(ServerAiBoundaryInputKind::RawUserText),
            Err(ServerAiInputRejection::RawUserText)
        );
        assert_eq!(
            validate_server_ai_boundary(ServerAiBoundaryInputKind::RawModelOutput),
            Err(ServerAiInputRejection::RawModelOutput)
        );
    }

    #[test]
    fn lod_only_spotlight_can_queue_models() {
        assert!(ServerAiLod::new(AiLodTier::Lod0Spotlight).allows_model_queue());
        assert!(!ServerAiLod::new(AiLodTier::Lod1NearActive).allows_model_queue());
        assert!(!ServerAiLod::new(AiLodTier::Lod2AreaSim).allows_model_queue());
        assert!(!ServerAiLod::new(AiLodTier::Lod3FarAggregate).allows_model_queue());
        assert!(!ServerAiLod::new(AiLodTier::Lod4Dormant).allows_model_queue());
    }

    #[test]
    fn deterministic_tactics_retreat_when_morale_collapses() {
        let perception = ServerAiPerception::new(1000, 1, 7);
        let (goal, score) = deterministic_goal(
            AiLodTier::Lod0Spotlight,
            &perception,
            MoraleState::Collapsed,
        );
        assert_eq!(goal, ServerAiGoalKind::Retreat);
        assert_eq!(score, 1000);
        assert_eq!(
            deterministic_intent(goal, MoraleState::Collapsed),
            TacticalIntent::Retreat
        );
    }

    #[test]
    fn mock_model_runtime_is_local_loaded_and_disabled_for_dialogue_by_default() {
        let runtime = ServerAiModelRuntime::default();
        let health = runtime.health();
        assert_eq!(health.backend(), fun_ai_core::ModelBackend::Mock);
        assert_eq!(health.loaded_model_count(), 1);
        assert!(!runtime.dialogue_generation_enabled);
    }

    #[test]
    fn model_dialogue_jobs_require_spotlight_lod_and_explicit_enablement() {
        let mut app = App::new();
        app.init_resource::<ServerAiFrame>()
            .init_resource::<ServerAiModelRuntime>();
        let actor = server_actor(
            "npc.model.enabled",
            AiLodSignals::new(1).with_active_conversation(true),
        );
        app.world_mut().spawn((
            actor,
            ServerAiLod::new(AiLodTier::Lod0Spotlight),
            ServerAiDialogueEligibility {
                eligible: true,
                last_changed_tick: 0,
            },
        ));

        app.update();
        app.world_mut()
            .run_system_cached(collect_server_ai_jobs)
            .expect("system runs");
        assert_eq!(
            app.world()
                .resource::<ServerAiModelRuntime>()
                .queue
                .pending_len(),
            0
        );

        app.world_mut()
            .resource_mut::<ServerAiModelRuntime>()
            .enable_dialogue_generation();
        app.world_mut()
            .run_system_cached(collect_server_ai_jobs)
            .expect("system runs");
        assert_eq!(
            app.world()
                .resource::<ServerAiModelRuntime>()
                .queue
                .pending_len(),
            1
        );
    }

    #[test]
    fn despawned_actor_demotes_to_dormant_lod() {
        let mut app = App::new();
        app.insert_resource(ServerAiFrame { tick: 42 });
        app.world_mut().spawn((
            server_actor(
                "npc.despawned",
                AiLodSignals::new(1).with_combat_threat(true),
            )
            .mark_despawned(),
            ServerAiLod::new(AiLodTier::Lod0Spotlight),
        ));
        app.world_mut()
            .run_system_cached(assign_ai_lod)
            .expect("system runs");
        let mut query = app.world_mut().query::<&ServerAiLod>();
        let lod = query.single(app.world()).expect("lod exists");
        assert_eq!(lod.tier(), AiLodTier::Lod4Dormant);
    }
}
