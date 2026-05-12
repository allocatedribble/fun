use std::sync::{Arc, Mutex};

use fun_scheduler_core::{
    DeterministicSingleThreadedGraphExecutor, FunSchedulerConfig, GraphExecutor, GraphReport,
    GraphRunError, GraphSubmitError, NodeRunner, Runtime,
};
use fun_scheduler_types::{GraphExecutionMode, NodeOutcome};

use crate::{
    CollisionCookMode, ECS_SPATIAL_ARTIFACT_BUILD_CONSUMERS, ECS_SPATIAL_DEFAULT_DECODE_CHUNKS,
    EcsArtifactConsumer, EcsCameraTransformSample, EcsChunkKey, EcsCommandBufferId,
    EcsCrossDomainHandoff, EcsCrossDomainHandoffKind, EcsDecodeOverlay,
    EcsDerivedArtifactBuildSystem, EcsDerivedArtifactRecord, EcsHandoffQueueId,
    EcsPageResidencyRecord, EcsPageResidencyState, EcsProceduralTerrainSource, EcsSpatialCommand,
    EcsSpatialCommandApplyDigest, EcsSpatialCommandApplyReport, EcsSpatialCommandBarrierKind,
    EcsSpatialCommandBuffer, EcsSpatialCompilerBarrierKind, EcsSpatialPageKey,
    EcsSpatialScheduleBuildInput, EcsSpatialScheduleCompileError, EcsSpatialScheduleCompileOutput,
    EcsSpatialScheduleCompiler, EcsSpatialSourceId, EcsSpatialValidationError, EcsStreamCameraId,
    EcsStreamPriority, EcsStreamRequest, EcsStreamRequestId, EcsStreamSourceDescriptor,
    EcsStreamSourceId, EcsStreamWaveLedger, EcsStreamingSourceSnapshot, EcsSystemClass, EcsWork,
    EcsWorkKind, FUN_COMMAND_BUFFER_ARTIFACTS, FUN_COMMAND_BUFFER_DIRTY_PROPAGATION,
    FUN_COMMAND_BUFFER_HANDOFFS, FUN_COMMAND_BUFFER_SPATIAL_REQUESTS, FixedStepId, FunRevision,
    FunWorld, GraphInvariantError, ProductRegistry, RendererVisibilityHint, RevisionCategory,
    ScheduleDomain, ScheduleLane, SpatialStreamCamera, StreamWaveReason, WorkBlockingClass,
    WorkNode, WorkNodeId, WorkWaitToken, acquire_sources, apply_artifact_commands_with_revisions,
    apply_dirty_propagation_commands_with_revisions, apply_handoff_commands_with_revisions,
    build_derived_artifacts, build_interest, decode_pages_with_procedural_manifest, diff_requests,
    plan_stream_wave, publish_lux_handoffs, publish_physics_cooks, publish_renderer_handoffs,
    sense_sources,
};

const DECODE_CHUNK_BASE: u64 = 0x0dec_0000;
const ARTIFACT_CONSUMER_CHUNK_BASE: u64 = 0x0a7f_0000;
const DEFAULT_SOURCE_ID: EcsSpatialSourceId = EcsSpatialSourceId::new(1);
const DEFAULT_STREAM_SOURCE_ID: EcsStreamSourceId = EcsStreamSourceId::new(1);
const DEFAULT_CAMERA_ID: EcsStreamCameraId = EcsStreamCameraId::new(1);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsNodeExecutionMode {
    #[default]
    DeterministicSingleThread = 0,
    DeterministicParallelRequested = 1,
    FairParallelRequested = 2,
    ExperimentalThroughputRequested = 3,
}

impl EcsNodeExecutionMode {
    #[must_use]
    pub const fn requested_graph_mode(self) -> GraphExecutionMode {
        match self {
            Self::DeterministicSingleThread => GraphExecutionMode::SingleThreadDeterministic,
            Self::DeterministicParallelRequested => GraphExecutionMode::DeterministicParallel,
            Self::FairParallelRequested => GraphExecutionMode::FairParallel,
            Self::ExperimentalThroughputRequested => GraphExecutionMode::MaxThroughputExperimental,
        }
    }

    #[must_use]
    pub const fn executor_graph_mode(self) -> GraphExecutionMode {
        GraphExecutionMode::SingleThreadDeterministic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsNodeContext<'a> {
    pub node_id: WorkNodeId,
    pub domain: ScheduleDomain,
    pub lane: ScheduleLane,
    pub work: &'a EcsWork<ProductRegistry>,
    pub execution_mode: EcsNodeExecutionMode,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsChunkExecutionContext {
    pub chunk_key: EcsChunkKey,
    pub chunk_index: u16,
    pub artifact_consumer: Option<EcsArtifactConsumer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsCommandApplyContext {
    pub barrier: Option<EcsSpatialCompilerBarrierKind>,
    pub command_buffer: Option<EcsCommandBufferId>,
    pub observed_revision: FunRevision,
    pub pending_commands: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsNodeMetrics {
    pub rows_read: u64,
    pub rows_written: u64,
    pub commands_emitted: u64,
    pub commands_applied: u64,
    pub tokens_published: u64,
    pub cancellation_tokens_published: u64,
    pub failure_tokens_published: u64,
    pub chunks_processed: u64,
    pub blocking_lane_used: bool,
}

impl EcsNodeMetrics {
    fn merge(&mut self, other: Self) {
        self.rows_read = self.rows_read.saturating_add(other.rows_read);
        self.rows_written = self.rows_written.saturating_add(other.rows_written);
        self.commands_emitted = self.commands_emitted.saturating_add(other.commands_emitted);
        self.commands_applied = self.commands_applied.saturating_add(other.commands_applied);
        self.tokens_published = self.tokens_published.saturating_add(other.tokens_published);
        self.cancellation_tokens_published = self
            .cancellation_tokens_published
            .saturating_add(other.cancellation_tokens_published);
        self.failure_tokens_published = self
            .failure_tokens_published
            .saturating_add(other.failure_tokens_published);
        self.chunks_processed = self.chunks_processed.saturating_add(other.chunks_processed);
        self.blocking_lane_used |= other.blocking_lane_used;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcsNodeError {
    Compile(EcsSpatialScheduleCompileError),
    Graph(GraphInvariantError),
    Submit(GraphSubmitError),
    Run(GraphRunError),
    Spatial(EcsSpatialValidationError),
    BlockingOutsideSchedulerLane {
        node: WorkNodeId,
        lane: ScheduleLane,
        blocking: WorkBlockingClass,
    },
    DuplicateProducedToken {
        node: WorkNodeId,
        token: WorkWaitToken,
    },
    UnknownCommandBuffer {
        node: WorkNodeId,
        command_buffer: Option<EcsCommandBufferId>,
    },
    UnknownWork {
        node: WorkNodeId,
        kind: EcsWorkKind,
        class: EcsSystemClass,
    },
    RunnerOutcomeLockPoisoned,
}

impl From<EcsSpatialScheduleCompileError> for EcsNodeError {
    fn from(value: EcsSpatialScheduleCompileError) -> Self {
        Self::Compile(value)
    }
}

impl From<GraphInvariantError> for EcsNodeError {
    fn from(value: GraphInvariantError) -> Self {
        Self::Graph(value)
    }
}

impl From<GraphSubmitError> for EcsNodeError {
    fn from(value: GraphSubmitError) -> Self {
        Self::Submit(value)
    }
}

impl From<GraphRunError> for EcsNodeError {
    fn from(value: GraphRunError) -> Self {
        Self::Run(value)
    }
}

impl From<EcsSpatialValidationError> for EcsNodeError {
    fn from(value: EcsSpatialValidationError) -> Self {
        Self::Spatial(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsNodeOutcome {
    pub node_id: WorkNodeId,
    pub kind: EcsWorkKind,
    pub class: EcsSystemClass,
    pub scheduler_outcome: NodeOutcome,
    pub produced_tokens: Vec<WorkWaitToken>,
    pub cancellation_tokens: Vec<WorkWaitToken>,
    pub failure_tokens: Vec<WorkWaitToken>,
    pub metrics: EcsNodeMetrics,
    pub error: Option<EcsNodeError>,
    pub digest_after: EcsSpatialWorldDigest,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialWorldDigest {
    pub value: u64,
}

#[derive(Debug, Clone)]
pub struct EcsSpatialFrameRunReport {
    pub scheduler_report: GraphReport,
    pub requested_mode: GraphExecutionMode,
    pub executed_mode: GraphExecutionMode,
    pub digest: EcsSpatialWorldDigest,
    pub metrics: EcsNodeMetrics,
    pub outcomes: Vec<EcsNodeOutcome>,
}

type OutcomeSink = Arc<Mutex<Vec<EcsNodeOutcome>>>;

pub struct EcsNodeRunner<'a> {
    world: &'a mut FunWorld,
    execution_mode: EcsNodeExecutionMode,
    snapshot: EcsStreamingSourceSnapshot,
    stream_wave_ledger: EcsStreamWaveLedger,
    request_commands: EcsSpatialCommandBuffer,
    artifact_commands: EcsSpatialCommandBuffer,
    dirty_commands: EcsSpatialCommandBuffer,
    handoff_commands: EcsSpatialCommandBuffer,
    decode_overlays: Vec<EcsDecodeOverlay>,
    sources: Vec<EcsStreamSourceDescriptor>,
    cameras: Vec<(SpatialStreamCamera, EcsCameraTransformSample)>,
    next_artifact_id: u64,
    next_request_id: u64,
    decode_chunk_count: u16,
    frame: u64,
    published_tokens: Vec<(WorkNodeId, WorkWaitToken)>,
    outcomes: OutcomeSink,
}

impl<'a> EcsNodeRunner<'a> {
    #[must_use]
    pub fn new(world: &'a mut FunWorld) -> Self {
        let frame = world
            .revision_ledger
            .category_revision(RevisionCategory::Frame)
            .get()
            .saturating_add(1);
        let next_artifact_id = world
            .derived_artifact_registry
            .artifacts
            .iter()
            .map(|(id, _record)| id.get())
            .max()
            .unwrap_or(0);
        let next_request_id = world
            .stream_request_queue
            .requests
            .iter()
            .map(|request| request.id.get())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let spatial_revision = world
            .revision_ledger
            .category_revision(RevisionCategory::Spatial);
        let artifact_revision = world
            .revision_ledger
            .category_revision(RevisionCategory::Artifact);
        let handoff_revision = world
            .revision_ledger
            .category_revision(RevisionCategory::Handoff);

        Self {
            world,
            execution_mode: EcsNodeExecutionMode::DeterministicSingleThread,
            snapshot: EcsStreamingSourceSnapshot::default(),
            stream_wave_ledger: EcsStreamWaveLedger::default(),
            request_commands: EcsSpatialCommandBuffer::new(spatial_revision),
            artifact_commands: EcsSpatialCommandBuffer::new(artifact_revision),
            dirty_commands: EcsSpatialCommandBuffer::new(spatial_revision),
            handoff_commands: EcsSpatialCommandBuffer::new(handoff_revision),
            decode_overlays: Vec::new(),
            sources: Vec::new(),
            cameras: Vec::new(),
            next_artifact_id,
            next_request_id,
            decode_chunk_count: ECS_SPATIAL_DEFAULT_DECODE_CHUNKS,
            frame,
            published_tokens: Vec::new(),
            outcomes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[must_use]
    pub fn with_execution_mode(mut self, mode: EcsNodeExecutionMode) -> Self {
        self.execution_mode = mode;
        self
    }

    #[must_use]
    pub fn with_decode_chunk_count(mut self, decode_chunk_count: u16) -> Self {
        self.decode_chunk_count = decode_chunk_count.max(1);
        self
    }

    #[must_use]
    pub fn with_stream_source(mut self, source: EcsStreamSourceDescriptor) -> Self {
        self.sources.push(source);
        self
    }

    #[must_use]
    pub fn with_stream_camera(
        mut self,
        camera: SpatialStreamCamera,
        transform: EcsCameraTransformSample,
    ) -> Self {
        self.cameras.push((camera, transform));
        self
    }

    #[must_use]
    pub fn with_decode_overlay(mut self, overlay: EcsDecodeOverlay) -> Self {
        self.decode_overlays.push(overlay);
        self
    }

    #[must_use]
    pub fn outcome_sink(&self) -> OutcomeSink {
        Arc::clone(&self.outcomes)
    }

    pub fn run_node(
        &mut self,
        node: &WorkNode<EcsWork<ProductRegistry>>,
    ) -> Result<EcsNodeOutcome, EcsNodeError> {
        let context = EcsNodeContext {
            node_id: node.id,
            domain: node.domain,
            lane: node.lane,
            work: &node.work_descriptor,
            execution_mode: self.execution_mode,
        };
        self.validate_blocking_lane(context)?;

        let mut metrics = EcsNodeMetrics {
            blocking_lane_used: context.lane == ScheduleLane::Blocking,
            ..EcsNodeMetrics::default()
        };

        match context.work.kind {
            EcsWorkKind::ApplyCommands => {
                let apply_metrics = self.apply_commands(context)?;
                metrics.merge(apply_metrics);
            }
            EcsWorkKind::UpdateChangeTicks => {
                let frame_metrics = self.finalize_frame_revision();
                metrics.merge(frame_metrics);
            }
            EcsWorkKind::ScheduleDiagnostics => {
                metrics.rows_read = self.diagnostic_row_count();
            }
            EcsWorkKind::RunSystem | EcsWorkKind::RunSystemChunk => {
                let run_metrics = self.dispatch_system(context)?;
                metrics.merge(run_metrics);
            }
            _ => {
                return Err(EcsNodeError::UnknownWork {
                    node: context.node_id,
                    kind: context.work.kind,
                    class: context.work.class,
                });
            }
        }

        let produced_tokens = self.publish_tokens(context)?;
        metrics.tokens_published = produced_tokens.len() as u64;
        Ok(EcsNodeOutcome {
            node_id: context.node_id,
            kind: context.work.kind,
            class: context.work.class,
            scheduler_outcome: NodeOutcome::Completed,
            produced_tokens,
            cancellation_tokens: Vec::new(),
            failure_tokens: Vec::new(),
            metrics,
            error: None,
            digest_after: EcsSpatialWorldDigest::from_world(self.world),
        })
    }

    fn validate_blocking_lane(&self, context: EcsNodeContext<'_>) -> Result<(), EcsNodeError> {
        let blocking = context.work.execution.blocking_class;
        if blocking.can_block() && context.lane != ScheduleLane::Blocking {
            Err(EcsNodeError::BlockingOutsideSchedulerLane {
                node: context.node_id,
                lane: context.lane,
                blocking,
            })
        } else {
            Ok(())
        }
    }

    fn dispatch_system(
        &mut self,
        context: EcsNodeContext<'_>,
    ) -> Result<EcsNodeMetrics, EcsNodeError> {
        match context.work.class {
            EcsSystemClass::WorldQuery => Ok(self.run_sense_sources()),
            EcsSystemClass::StreamInterestBuild => self.run_build_interest(),
            EcsSystemClass::StreamPlan => Ok(self.run_plan_stream_wave()),
            EcsSystemClass::StreamRequestDiff => self.run_diff_requests(),
            EcsSystemClass::SourceAcquire => self.run_acquire_sources(),
            EcsSystemClass::Decode => self.run_decode_pages(context),
            EcsSystemClass::DerivedArtifactBuild => self.run_build_derived_artifacts(context),
            EcsSystemClass::DirtyPropagation => Ok(EcsNodeMetrics::default()),
            EcsSystemClass::RendererHandoff => self.run_publish_renderer_handoffs(),
            EcsSystemClass::LuxHandoff => self.run_publish_lux_handoffs(),
            EcsSystemClass::PhysicsHandoff => self.run_publish_physics_handoffs(),
            EcsSystemClass::NetworkHandoff => self.run_publish_network_handoffs(),
            EcsSystemClass::Eviction | EcsSystemClass::Diagnostics => Ok(EcsNodeMetrics::default()),
            _ => Err(EcsNodeError::UnknownWork {
                node: context.node_id,
                kind: context.work.kind,
                class: context.work.class,
            }),
        }
    }

    fn run_sense_sources(&mut self) -> EcsNodeMetrics {
        self.snapshot = sense_sources(self.frame, &self.cameras, &self.sources);
        EcsNodeMetrics {
            rows_read: (self.cameras.len() + self.sources.len()) as u64,
            rows_written: self.snapshot.active_cameras.len() as u64
                + self.snapshot.active_sources.len() as u64
                + self.snapshot.active_terrain_grids.len() as u64,
            ..EcsNodeMetrics::default()
        }
    }

    fn run_build_interest(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        let grid_registry = crate::EcsSpatialGridRegistry::default();
        let table = build_interest(
            &self.snapshot,
            &grid_registry,
            &self.world.spatial_page_table,
        )?;
        let rows_written = table.interests.len() as u64;
        self.world.stream_interest_table = table;
        Ok(EcsNodeMetrics {
            rows_read: self.snapshot.active_cameras.len() as u64
                + self.snapshot.active_terrain_grids.len() as u64
                + self.world.spatial_page_table.len() as u64,
            rows_written,
            ..EcsNodeMetrics::default()
        })
    }

    fn run_plan_stream_wave(&mut self) -> EcsNodeMetrics {
        let planned = plan_stream_wave(
            &self.snapshot,
            &mut self.stream_wave_ledger,
            0,
            4,
            StreamWaveReason::ColdStart,
        )
        .is_some();
        EcsNodeMetrics {
            rows_read: self.snapshot.active_cameras.len() as u64,
            rows_written: u64::from(planned),
            ..EcsNodeMetrics::default()
        }
    }

    fn run_diff_requests(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        let requested_pages: Vec<EcsSpatialPageKey> = self
            .world
            .stream_request_queue
            .requests
            .iter()
            .map(|request| request.page)
            .collect();
        let before = self.request_commands.commands.len();
        let report = diff_requests(
            &self.world.stream_interest_table,
            &self.world.spatial_page_table,
            &requested_pages,
            &[],
            &mut self.request_commands,
        )?;
        let emitted = self.request_commands.commands.len().saturating_sub(before);
        Ok(EcsNodeMetrics {
            rows_read: self.world.stream_interest_table.interests.len() as u64
                + self.world.spatial_page_table.len() as u64
                + requested_pages.len() as u64,
            commands_emitted: emitted as u64,
            rows_written: u64::from(report.requested)
                + u64::from(report.cancelled)
                + u64::from(report.pinned)
                + u64::from(report.unpinned),
            ..EcsNodeMetrics::default()
        })
    }

    fn run_acquire_sources(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        let pages: Vec<EcsSpatialPageKey> = self
            .world
            .stream_request_queue
            .requests
            .iter()
            .map(|request| request.page)
            .collect();
        if pages.is_empty() {
            return Ok(EcsNodeMetrics {
                blocking_lane_used: true,
                ..EcsNodeMetrics::default()
            });
        }
        let source = EcsProceduralTerrainSource {
            source_id: DEFAULT_SOURCE_ID,
            manifest: self.world.procedural_world_manifest,
        };
        let before = self.world.source_acquire_queue.rows.len();
        let report = acquire_sources(
            &source,
            &pages,
            self.world.procedural_world_manifest.region_edge_pages,
            &mut self.world.source_acquire_queue,
        )?;
        Ok(EcsNodeMetrics {
            rows_read: pages.len() as u64,
            rows_written: self
                .world
                .source_acquire_queue
                .rows
                .len()
                .saturating_sub(before) as u64,
            blocking_lane_used: true,
            commands_applied: u64::from(report.requested),
            ..EcsNodeMetrics::default()
        })
    }

    fn run_decode_pages(
        &mut self,
        context: EcsNodeContext<'_>,
    ) -> Result<EcsNodeMetrics, EcsNodeError> {
        let chunk = chunk_context(context.work, self.decode_chunk_count);
        let before = self.world.decoded_page_queue.rows.len();
        let mut acquired = crate::EcsSourceAcquireQueue::default();
        for (index, row) in self.world.source_acquire_queue.rows.iter().enumerate() {
            if (index % usize::from(self.decode_chunk_count)) == usize::from(chunk.chunk_index) {
                acquired.push(row.clone())?;
            }
        }
        let report = decode_pages_with_procedural_manifest(
            &acquired,
            &self.decode_overlays,
            self.frame as u32,
            self.world.procedural_world_manifest,
            &mut self.world.decoded_page_queue,
        )?;
        Ok(EcsNodeMetrics {
            rows_read: acquired.rows.len() as u64 + self.decode_overlays.len() as u64,
            rows_written: self
                .world
                .decoded_page_queue
                .rows
                .len()
                .saturating_sub(before) as u64,
            chunks_processed: 1,
            commands_applied: u64::from(report.decoded)
                + u64::from(report.procedural)
                + u64::from(report.failed),
            ..EcsNodeMetrics::default()
        })
    }

    fn run_build_derived_artifacts(
        &mut self,
        context: EcsNodeContext<'_>,
    ) -> Result<EcsNodeMetrics, EcsNodeError> {
        let chunk = chunk_context(context.work, self.decode_chunk_count);
        let systems: Vec<EcsDerivedArtifactBuildSystem> = EcsDerivedArtifactBuildSystem::all()
            .iter()
            .copied()
            .filter(|system| {
                chunk
                    .artifact_consumer
                    .is_none_or(|consumer| system.consumer() == consumer)
            })
            .collect();
        let before = self.artifact_commands.commands.len();
        let report = build_derived_artifacts(
            &self.world.decoded_page_queue.rows,
            &systems,
            &mut self.next_artifact_id,
            &mut self.artifact_commands,
        )?;
        Ok(EcsNodeMetrics {
            rows_read: self.world.decoded_page_queue.rows.len() as u64,
            rows_written: u64::from(report.artifacts_published),
            commands_emitted: self.artifact_commands.commands.len().saturating_sub(before) as u64,
            chunks_processed: 1,
            ..EcsNodeMetrics::default()
        })
    }

    fn run_publish_renderer_handoffs(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        let artifacts = ready_artifacts_for_consumer(self.world, EcsArtifactConsumer::Renderer);
        let before = self.handoff_commands.commands.len();
        let report = publish_renderer_handoffs(
            &artifacts,
            RendererVisibilityHint::VisibleNear,
            &mut self.handoff_commands,
        )?;
        Ok(EcsNodeMetrics {
            rows_read: artifacts.len() as u64,
            rows_written: u64::from(report.published),
            commands_emitted: self.handoff_commands.commands.len().saturating_sub(before) as u64,
            ..EcsNodeMetrics::default()
        })
    }

    fn run_publish_lux_handoffs(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        let artifacts = ready_artifacts_for_consumer(self.world, EcsArtifactConsumer::Lux);
        let before = self.handoff_commands.commands.len();
        let report = publish_lux_handoffs(
            &artifacts,
            crate::EcsAabbF32::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            &mut self.handoff_commands,
        )?;
        Ok(EcsNodeMetrics {
            rows_read: artifacts.len() as u64,
            rows_written: u64::from(report.published),
            commands_emitted: self.handoff_commands.commands.len().saturating_sub(before) as u64,
            ..EcsNodeMetrics::default()
        })
    }

    fn run_publish_physics_handoffs(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        let artifacts = ready_artifacts_for_consumer(self.world, EcsArtifactConsumer::AvisPhysics);
        let before = self.handoff_commands.commands.len();
        let report = publish_physics_cooks(
            &artifacts,
            CollisionCookMode::NarrowBandSdf,
            Option::<FixedStepId>::None,
            &mut self.handoff_commands,
        )?;
        Ok(EcsNodeMetrics {
            rows_read: artifacts.len() as u64,
            rows_written: u64::from(report.published),
            commands_emitted: self.handoff_commands.commands.len().saturating_sub(before) as u64,
            ..EcsNodeMetrics::default()
        })
    }

    fn run_publish_network_handoffs(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        let artifacts =
            ready_artifacts_for_consumer(self.world, EcsArtifactConsumer::ThunderNetwork);
        let before = self.world.cross_domain_handoff_queues.rows.len();
        for artifact in &artifacts {
            self.world
                .cross_domain_handoff_queues
                .push(EcsCrossDomainHandoff::new(
                    EcsHandoffQueueId::new(4),
                    EcsCrossDomainHandoffKind::NetworkRelevanceRow,
                    EcsArtifactConsumer::ThunderNetwork,
                    artifact.artifact_id,
                    artifact.source_page,
                    artifact.artifact_epoch,
                ))?;
        }
        Ok(EcsNodeMetrics {
            rows_read: artifacts.len() as u64,
            rows_written: self
                .world
                .cross_domain_handoff_queues
                .rows
                .len()
                .saturating_sub(before) as u64,
            ..EcsNodeMetrics::default()
        })
    }

    fn apply_commands(
        &mut self,
        context: EcsNodeContext<'_>,
    ) -> Result<EcsNodeMetrics, EcsNodeError> {
        match context.work.command_buffer {
            Some(command_buffer)
                if command_buffer == FUN_COMMAND_BUFFER_SPATIAL_REQUESTS.scheduler_id() =>
            {
                self.apply_request_commands()
            }
            Some(command_buffer)
                if command_buffer == FUN_COMMAND_BUFFER_ARTIFACTS.scheduler_id() =>
            {
                self.artifact_commands.set_observed_revision(
                    self.world
                        .revision_ledger
                        .category_revision(RevisionCategory::Artifact),
                );
                let report = apply_artifact_commands_with_revisions(
                    &mut self.artifact_commands,
                    &mut self.world.derived_artifact_registry,
                    &mut self.world.revision_ledger,
                )?;
                Ok(metrics_from_apply_report(report))
            }
            Some(command_buffer)
                if command_buffer == FUN_COMMAND_BUFFER_DIRTY_PROPAGATION.scheduler_id() =>
            {
                self.dirty_commands.set_observed_revision(
                    self.world
                        .revision_ledger
                        .category_revision(RevisionCategory::Spatial),
                );
                let report = apply_dirty_propagation_commands_with_revisions(
                    &mut self.dirty_commands,
                    &mut self.world.derived_artifact_registry,
                    &mut self.world.physics_cook_queue,
                    &mut self.world.revision_ledger,
                )?;
                Ok(metrics_from_apply_report(report))
            }
            Some(command_buffer)
                if command_buffer == FUN_COMMAND_BUFFER_HANDOFFS.scheduler_id() =>
            {
                self.handoff_commands.set_observed_revision(
                    self.world
                        .revision_ledger
                        .category_revision(RevisionCategory::Handoff),
                );
                let report = apply_handoff_commands_with_revisions(
                    &mut self.handoff_commands,
                    &mut self.world.renderer_handoff_queue,
                    &mut self.world.lux_handoff_queue,
                    &mut self.world.physics_cook_queue,
                    &mut self.world.revision_ledger,
                )?;
                Ok(metrics_from_apply_report(report))
            }
            command_buffer => Err(EcsNodeError::UnknownCommandBuffer {
                node: context.node_id,
                command_buffer,
            }),
        }
    }

    fn apply_request_commands(&mut self) -> Result<EcsNodeMetrics, EcsNodeError> {
        self.request_commands.set_observed_revision(
            self.world
                .revision_ledger
                .category_revision(RevisionCategory::Spatial),
        );
        let inspected = self.request_commands.commands.len() as u32;
        let selected = self
            .request_commands
            .drain_for_barrier(EcsSpatialCommandBarrierKind::ApplyRequestCommands);
        let digest = EcsSpatialCommandApplyDigest::from_commands(&selected);
        let mut applied = 0_u32;
        let mut ignored = 0_u32;
        for command in selected {
            match command {
                EcsSpatialCommand::RequestPage(page) => {
                    self.apply_request_page(page)?;
                    applied = applied.saturating_add(1);
                }
                EcsSpatialCommand::CancelPage(page) => {
                    let before = self.world.stream_request_queue.requests.len();
                    self.world
                        .stream_request_queue
                        .requests
                        .retain(|request| request.page != page);
                    if before != self.world.stream_request_queue.requests.len() {
                        applied = applied.saturating_add(1);
                    } else {
                        ignored = ignored.saturating_add(1);
                    }
                }
                EcsSpatialCommand::PinPage(_) | EcsSpatialCommand::UnpinPage(_) => {
                    applied = applied.saturating_add(1);
                }
                _ => ignored = ignored.saturating_add(1),
            }
        }
        if applied > 0 {
            let (_previous, new) = self
                .world
                .revision_ledger
                .advance_category(RevisionCategory::Spatial);
            self.world.revision = new;
        }
        let report = EcsSpatialCommandApplyReport {
            barrier: EcsSpatialCommandBarrierKind::ApplyRequestCommands,
            inspected,
            applied,
            ignored,
            retained: self.request_commands.commands.len() as u32,
            digest,
            revision: crate::FunCommandApplyRevisionReport::new(
                self.request_commands.observed_revision,
                self.world
                    .revision_ledger
                    .category_revision(RevisionCategory::Spatial),
            ),
        };
        Ok(metrics_from_apply_report(report))
    }

    fn apply_request_page(&mut self, page: EcsSpatialPageKey) -> Result<(), EcsNodeError> {
        if self
            .world
            .stream_request_queue
            .requests
            .iter()
            .any(|request| request.page == page)
        {
            return Ok(());
        }
        let request_id = EcsStreamRequestId::new(self.next_request_id);
        self.next_request_id = self.next_request_id.saturating_add(1);
        self.world.stream_request_queue.push(EcsStreamRequest::new(
            request_id,
            DEFAULT_STREAM_SOURCE_ID,
            DEFAULT_CAMERA_ID,
            page,
            EcsStreamPriority::default(),
        ))?;
        let requested_frame = self.frame.min(u64::from(u32::MAX)) as u32;

        if let Some(record) = self.world.spatial_page_table.get_mut_by_key(page) {
            record.state = EcsPageResidencyState::Requested;
            record.last_requested_frame = requested_frame;
        } else {
            let generation = self.world.spatial_page_table.len().saturating_add(1) as u32;
            let mut record =
                EcsPageResidencyRecord::new(page, EcsStreamPriority::default(), generation);
            record.state = EcsPageResidencyState::Requested;
            record.last_requested_frame = requested_frame;
            self.world.spatial_page_table.push(record)?;
        }
        self.world
            .residency_table
            .upsert(page, EcsPageResidencyState::Requested, 1);
        Ok(())
    }

    fn finalize_frame_revision(&mut self) -> EcsNodeMetrics {
        let (_previous, new) = self
            .world
            .revision_ledger
            .advance_category(RevisionCategory::Frame);
        self.world.revision = new;
        EcsNodeMetrics {
            rows_written: 1,
            ..EcsNodeMetrics::default()
        }
    }

    fn diagnostic_row_count(&self) -> u64 {
        self.world.spatial_page_table.len() as u64
            + self.world.stream_interest_table.interests.len() as u64
            + self.world.stream_request_queue.requests.len() as u64
            + self.world.source_acquire_queue.rows.len() as u64
            + self.world.decoded_page_queue.rows.len() as u64
            + self.world.derived_artifact_registry.len() as u64
            + self.world.renderer_handoff_queue.items.len() as u64
            + self.world.lux_handoff_queue.items.len() as u64
            + self.world.physics_cook_queue.items.len() as u64
            + self.world.cross_domain_handoff_queues.rows.len() as u64
    }

    fn publish_tokens(
        &mut self,
        context: EcsNodeContext<'_>,
    ) -> Result<Vec<WorkWaitToken>, EcsNodeError> {
        let mut published = Vec::new();
        for token in &context.work.produced_tokens {
            if published.contains(token) {
                return Err(EcsNodeError::DuplicateProducedToken {
                    node: context.node_id,
                    token: *token,
                });
            }
            published.push(*token);
            self.published_tokens.push((context.node_id, *token));
        }
        Ok(published)
    }
}

impl NodeRunner<EcsWork<ProductRegistry>> for EcsNodeRunner<'_> {
    fn run(&mut self, node: &WorkNode<EcsWork<ProductRegistry>>) -> NodeOutcome {
        let result = self.run_node(node);
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(error) => EcsNodeOutcome {
                node_id: node.id,
                kind: node.work_descriptor.kind,
                class: node.work_descriptor.class,
                scheduler_outcome: NodeOutcome::Failed,
                produced_tokens: Vec::new(),
                cancellation_tokens: Vec::new(),
                failure_tokens: node.work_descriptor.produced_tokens.clone(),
                metrics: EcsNodeMetrics {
                    failure_tokens_published: node.work_descriptor.produced_tokens.len() as u64,
                    ..EcsNodeMetrics::default()
                },
                error: Some(error),
                digest_after: EcsSpatialWorldDigest::from_world(self.world),
            },
        };
        let scheduler_outcome = outcome.scheduler_outcome;
        if let Ok(mut outcomes) = self.outcomes.lock() {
            outcomes.push(outcome);
        }
        scheduler_outcome
    }
}

impl EcsSpatialWorldDigest {
    #[must_use]
    pub fn from_world(world: &FunWorld) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = hash_u64(hash, world.revision.get());
        hash = hash_u64(hash, world.procedural_world_manifest.signature().value);
        hash = hash_u64(hash, world.spatial_page_table.len() as u64);
        hash = hash_u64(hash, world.stream_request_queue.requests.len() as u64);
        hash = hash_u64(hash, world.source_acquire_queue.rows.len() as u64);
        hash = hash_u64(hash, world.decoded_page_queue.rows.len() as u64);
        hash = hash_u64(hash, world.derived_artifact_registry.len() as u64);
        hash = hash_u64(hash, world.renderer_handoff_queue.items.len() as u64);
        hash = hash_u64(hash, world.lux_handoff_queue.items.len() as u64);
        hash = hash_u64(hash, world.physics_cook_queue.items.len() as u64);
        hash = hash_u64(hash, world.cross_domain_handoff_queues.rows.len() as u64);

        let mut requests: Vec<_> = world.stream_request_queue.requests.iter().collect();
        requests.sort_by_key(|request| page_sort_key(request.page));
        for request in requests {
            hash = hash_page(hash, request.page);
            hash = hash_u64(hash, request.id.get());
        }

        let mut decoded: Vec<_> = world.decoded_page_queue.rows.iter().collect();
        decoded.sort_by_key(|row| page_sort_key(row.key));
        for row in decoded {
            hash = hash_page(hash, row.key);
            hash = hash_u64(hash, u64::from(row.source_epoch));
            hash = hash_u8(hash, row.payload_kind as u8);
        }

        let mut artifacts: Vec<(u64, EcsDerivedArtifactRecord)> = world
            .derived_artifact_registry
            .artifacts
            .iter()
            .map(|(id, record)| (id.get(), *record))
            .collect();
        artifacts.sort_by_key(|(id, _record)| *id);
        for (id, artifact) in artifacts {
            hash = hash_u64(hash, id);
            hash = hash_page(hash, artifact.source_page);
            hash = hash_u8(hash, artifact.kind as u8);
            hash = hash_u8(hash, artifact.consumer as u8);
            hash = hash_u64(hash, u64::from(artifact.artifact_epoch));
        }

        for handoff in &world.renderer_handoff_queue.items {
            hash = hash_u64(hash, handoff.artifact_id.get());
            hash = hash_page(hash, handoff.source_page);
            hash = hash_u8(hash, handoff.kind as u8);
        }
        for handoff in &world.lux_handoff_queue.items {
            hash = hash_page(hash, handoff.source_page);
            hash = hash_u8(hash, handoff.kind as u8);
            hash = hash_u64(hash, u64::from(handoff.dirty_epoch));
        }
        for cook in &world.physics_cook_queue.items {
            hash = hash_page(hash, cook.source_page);
            hash = hash_u8(hash, cook.mode as u8);
            hash = hash_u64(hash, u64::from(cook.dirty_epoch));
        }
        for handoff in &world.cross_domain_handoff_queues.rows {
            hash = hash_u64(hash, handoff.artifact.get());
            hash = hash_page(hash, handoff.source_page);
            hash = hash_u8(hash, handoff.kind as u8);
            hash = hash_u8(hash, handoff.consumer as u8);
        }

        Self { value: hash }
    }
}

impl FunWorld {
    pub fn compile_spatial_frame_graph(
        &self,
    ) -> Result<EcsSpatialScheduleCompileOutput, EcsSpatialScheduleCompileError> {
        EcsSpatialScheduleCompiler::compile(
            EcsSpatialScheduleBuildInput::default().with_observed_revision(self.revision),
        )
    }

    pub fn submit_spatial_frame(
        &mut self,
        runtime: &Runtime<ProductRegistry>,
    ) -> Result<EcsSpatialFrameRunReport, EcsNodeError> {
        self.run_spatial_frame_with_runtime(
            runtime,
            EcsNodeExecutionMode::DeterministicSingleThread,
        )
    }

    pub fn run_spatial_frame_deterministic(
        &mut self,
    ) -> Result<EcsSpatialFrameRunReport, EcsNodeError> {
        let runtime = Runtime::new(FunSchedulerConfig::default());
        self.submit_spatial_frame(&runtime)
    }

    pub fn run_spatial_frame_deterministic_with_streaming_inputs(
        &mut self,
        cameras: &[(SpatialStreamCamera, EcsCameraTransformSample)],
        sources: &[EcsStreamSourceDescriptor],
    ) -> Result<EcsSpatialFrameRunReport, EcsNodeError> {
        let runtime = Runtime::new(FunSchedulerConfig::default());
        self.run_spatial_frame_with_runtime_and_inputs(
            &runtime,
            EcsNodeExecutionMode::DeterministicSingleThread,
            cameras,
            sources,
        )
    }

    pub fn run_spatial_frame_parallel(&mut self) -> Result<EcsSpatialFrameRunReport, EcsNodeError> {
        let runtime = Runtime::new(FunSchedulerConfig::default());
        self.run_spatial_frame_with_runtime(
            &runtime,
            EcsNodeExecutionMode::DeterministicParallelRequested,
        )
    }

    fn run_spatial_frame_with_runtime(
        &mut self,
        runtime: &Runtime<ProductRegistry>,
        mode: EcsNodeExecutionMode,
    ) -> Result<EcsSpatialFrameRunReport, EcsNodeError> {
        self.run_spatial_frame_with_runtime_and_inputs(runtime, mode, &[], &[])
    }

    fn run_spatial_frame_with_runtime_and_inputs(
        &mut self,
        runtime: &Runtime<ProductRegistry>,
        mode: EcsNodeExecutionMode,
        cameras: &[(SpatialStreamCamera, EcsCameraTransformSample)],
        sources: &[EcsStreamSourceDescriptor],
    ) -> Result<EcsSpatialFrameRunReport, EcsNodeError> {
        let mut output = self.compile_spatial_frame_graph()?;
        let requested_mode = mode.requested_graph_mode();
        output.graph.mode = requested_mode;
        output.graph.validate_topology()?;
        output.graph.mode = mode.executor_graph_mode();
        let executed_mode = output.graph.mode;

        let mut runner = EcsNodeRunner::new(self)
            .with_execution_mode(mode)
            .with_decode_chunk_count(output.chunk_plan.decode_chunks);
        for source in sources {
            runner = runner.with_stream_source(*source);
        }
        for (camera, transform) in cameras {
            runner = runner.with_stream_camera(*camera, *transform);
        }
        let outcome_sink = runner.outcome_sink();
        let executor =
            DeterministicSingleThreadedGraphExecutor::new(Arc::clone(runtime.clock()), runner);
        let handle = executor.submit_graph(output.graph)?;
        let scheduler_report = executor.run_graph_until_complete(handle)?;
        let outcomes = outcomes_from_sink(outcome_sink)?;
        let mut metrics = EcsNodeMetrics::default();
        for outcome in &outcomes {
            metrics.merge(outcome.metrics);
        }
        Ok(EcsSpatialFrameRunReport {
            scheduler_report,
            requested_mode,
            executed_mode,
            digest: EcsSpatialWorldDigest::from_world(self),
            metrics,
            outcomes,
        })
    }
}

fn outcomes_from_sink(sink: OutcomeSink) -> Result<Vec<EcsNodeOutcome>, EcsNodeError> {
    let outcomes = sink
        .lock()
        .map_err(|_error| EcsNodeError::RunnerOutcomeLockPoisoned)?;
    Ok(outcomes.clone())
}

fn metrics_from_apply_report(report: EcsSpatialCommandApplyReport) -> EcsNodeMetrics {
    EcsNodeMetrics {
        rows_read: u64::from(report.inspected),
        rows_written: u64::from(report.applied),
        commands_applied: u64::from(report.applied),
        ..EcsNodeMetrics::default()
    }
}

fn chunk_context(
    work: &EcsWork<ProductRegistry>,
    decode_chunk_count: u16,
) -> EcsChunkExecutionContext {
    let chunk_key = work.chunk_key;
    let raw = chunk_key.get();
    let artifact_consumer = artifact_consumer_from_chunk_key(chunk_key);
    let chunk_index = if raw > DECODE_CHUNK_BASE && raw <= DECODE_CHUNK_BASE + 64 {
        raw.saturating_sub(DECODE_CHUNK_BASE + 1) as u16
    } else {
        artifact_consumer.map_or(0, |consumer| consumer as u16)
    };
    EcsChunkExecutionContext {
        chunk_key,
        chunk_index: chunk_index.min(decode_chunk_count.saturating_sub(1)),
        artifact_consumer,
    }
}

fn artifact_consumer_from_chunk_key(chunk_key: EcsChunkKey) -> Option<EcsArtifactConsumer> {
    let raw = chunk_key.get();
    if raw <= ARTIFACT_CONSUMER_CHUNK_BASE {
        return None;
    }
    let index = raw.saturating_sub(ARTIFACT_CONSUMER_CHUNK_BASE + 1);
    ECS_SPATIAL_ARTIFACT_BUILD_CONSUMERS
        .iter()
        .copied()
        .find(|consumer| *consumer as u64 == index)
}

fn ready_artifacts_for_consumer(
    world: &FunWorld,
    consumer: EcsArtifactConsumer,
) -> Vec<EcsDerivedArtifactRecord> {
    let mut artifacts: Vec<EcsDerivedArtifactRecord> = world
        .derived_artifact_registry
        .artifacts
        .iter()
        .filter_map(|(_id, record)| {
            (record.consumer == consumer && record.state == crate::EcsArtifactState::Ready)
                .then_some(*record)
        })
        .collect();
    artifacts.sort_by_key(|artifact| artifact.artifact_id.get());
    artifacts
}

fn page_sort_key(page: EcsSpatialPageKey) -> (u8, u64, u8, i32, i32, i32, u8) {
    (
        page.domain as u8,
        page.grid_id.get(),
        page.level,
        page.x,
        page.y,
        page.z,
        page.channel as u8,
    )
}

fn hash_page(mut hash: u64, page: EcsSpatialPageKey) -> u64 {
    hash = hash_u8(hash, page.domain as u8);
    hash = hash_u64(hash, page.grid_id.get());
    hash = hash_u8(hash, page.level);
    hash = hash_u32(hash, page.x as u32);
    hash = hash_u32(hash, page.y as u32);
    hash = hash_u32(hash, page.z as u32);
    hash_u8(hash, page.channel as u8)
}

const fn hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn hash_u32(mut hash: u64, value: u32) -> u64 {
    hash = hash_u8(hash, (value & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 8) & 0xff) as u8);
    hash = hash_u8(hash, ((value >> 16) & 0xff) as u8);
    hash_u8(hash, ((value >> 24) & 0xff) as u8)
}

const fn hash_u64(mut hash: u64, value: u64) -> u64 {
    hash = hash_u32(hash, (value & 0xffff_ffff) as u32);
    hash_u32(hash, (value >> 32) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EcsDecodedPagePayloadKind, EcsDecodedPageRecord, EcsEntityId, EcsPageChannel,
        EcsSourceChecksum, EcsSpatialDomainKind, EcsSpatialGridId, EcsSystemExecutionContract,
        EcsViewFrustum, IVec3, RendererArtifactHandoffKind, StreamCameraRole,
        VOXEL_CLUSTER_SUMMARIES_PER_BRICK, VoxelClusterSummary,
    };
    use fun_scheduler_types::{
        DeterministicDescriptor, EcsLivenessClass, EcsWorldRevision, ScheduleBudget,
        ScheduleDeadline, SystemAccess, TaskPriority, WorkNodeId,
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

    fn decoded_page(key: EcsSpatialPageKey) -> EcsDecodedPageRecord {
        EcsDecodedPageRecord {
            key,
            source: DEFAULT_SOURCE_ID,
            source_epoch: 1,
            payload_kind: crate::EcsDecodedPagePayloadKind::Empty,
            voxel_brick: None,
            cluster_summaries: [VoxelClusterSummary::default(); VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
            telemetry: crate::EcsDecodeTelemetry {
                key,
                source_epoch: 1,
                decode_epoch: 7,
                source_bytes: 0,
                overlay_count: 0,
                cluster_summary_count: 0,
                checksum: EcsSourceChecksum::NONE,
                failure: None,
            },
        }
    }

    fn main_stream_camera(
        origin_world_ft: IVec3,
    ) -> (SpatialStreamCamera, EcsCameraTransformSample) {
        (
            SpatialStreamCamera {
                role: StreamCameraRole::MainView,
                enabled: true,
                priority: 10,
                required_shells: 0,
                desired_shells: 0,
                velocity_lookahead_s: 0.0,
            },
            EcsCameraTransformSample {
                camera_entity: EcsEntityId::new(1),
                camera_id: DEFAULT_CAMERA_ID,
                origin_world_ft,
                velocity_world_ft_s: IVec3::zero(),
                view_frustum: EcsViewFrustum::EMPTY,
                cut_or_teleport: false,
            },
        )
    }

    fn terrain_source() -> EcsStreamSourceDescriptor {
        EcsStreamSourceDescriptor {
            source: DEFAULT_STREAM_SOURCE_ID,
            domain: EcsSpatialDomainKind::Terrain,
            grid_id: EcsSpatialGridId::new(1),
            priority: 10,
        }
    }

    fn run_reference_artifact_and_handoff_path(
        world: &mut FunWorld,
    ) -> Result<EcsSpatialWorldDigest, EcsSpatialValidationError> {
        let mut next_artifact_id = 0;
        for consumer in ECS_SPATIAL_ARTIFACT_BUILD_CONSUMERS {
            let systems: Vec<EcsDerivedArtifactBuildSystem> = EcsDerivedArtifactBuildSystem::all()
                .iter()
                .copied()
                .filter(|system| system.consumer() == consumer)
                .collect();
            let mut artifact_commands = EcsSpatialCommandBuffer::new(
                world
                    .revision_ledger
                    .category_revision(RevisionCategory::Artifact),
            );
            build_derived_artifacts(
                &world.decoded_page_queue.rows,
                &systems,
                &mut next_artifact_id,
                &mut artifact_commands,
            )?;
            apply_artifact_commands_with_revisions(
                &mut artifact_commands,
                &mut world.derived_artifact_registry,
                &mut world.revision_ledger,
            )?;
        }

        let mut handoff_commands = EcsSpatialCommandBuffer::new(
            world
                .revision_ledger
                .category_revision(RevisionCategory::Handoff),
        );
        let renderer = ready_artifacts_for_consumer(world, EcsArtifactConsumer::Renderer);
        publish_renderer_handoffs(
            &renderer,
            RendererVisibilityHint::VisibleNear,
            &mut handoff_commands,
        )?;
        let lux = ready_artifacts_for_consumer(world, EcsArtifactConsumer::Lux);
        publish_lux_handoffs(
            &lux,
            crate::EcsAabbF32::new([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            &mut handoff_commands,
        )?;
        let physics = ready_artifacts_for_consumer(world, EcsArtifactConsumer::AvisPhysics);
        publish_physics_cooks(
            &physics,
            CollisionCookMode::NarrowBandSdf,
            Option::<FixedStepId>::None,
            &mut handoff_commands,
        )?;
        apply_handoff_commands_with_revisions(
            &mut handoff_commands,
            &mut world.renderer_handoff_queue,
            &mut world.lux_handoff_queue,
            &mut world.physics_cook_queue,
            &mut world.revision_ledger,
        )?;
        for artifact in ready_artifacts_for_consumer(world, EcsArtifactConsumer::ThunderNetwork) {
            world
                .cross_domain_handoff_queues
                .push(EcsCrossDomainHandoff::new(
                    EcsHandoffQueueId::new(4),
                    EcsCrossDomainHandoffKind::NetworkRelevanceRow,
                    EcsArtifactConsumer::ThunderNetwork,
                    artifact.artifact_id,
                    artifact.source_page,
                    artifact.artifact_epoch,
                ))?;
        }
        let (_previous, new) = world
            .revision_ledger
            .advance_category(RevisionCategory::Frame);
        world.revision = new;
        Ok(EcsSpatialWorldDigest::from_world(world))
    }

    #[test]
    fn noop_spatial_frame_runs_through_fun_scheduler() {
        let mut world = FunWorld::default();
        let report = world
            .run_spatial_frame_deterministic()
            .expect("no-op spatial frame runs");

        assert_eq!(
            report.executed_mode,
            GraphExecutionMode::SingleThreadDeterministic
        );
        assert_eq!(report.scheduler_report.metrics.rejected_count, 0);
        assert_eq!(
            report.scheduler_report.kernel_metrics.nodes_failed, 0,
            "runner outcomes: {:?}",
            report.outcomes
        );
        assert_eq!(
            report.scheduler_report.kernel_metrics.nodes_completed,
            report.scheduler_report.metrics.node_count
        );
        assert_eq!(report.digest, EcsSpatialWorldDigest::from_world(&world));
    }

    #[test]
    fn streaming_frame_generates_procedural_voxel_page_and_renderer_handoff() {
        let mut world = FunWorld::default();
        let camera = main_stream_camera(IVec3::zero());
        let source = terrain_source();
        let report = world
            .run_spatial_frame_deterministic_with_streaming_inputs(&[camera], &[source])
            .expect("streaming spatial frame runs");

        assert_eq!(report.scheduler_report.metrics.rejected_count, 0);
        assert_eq!(world.source_acquire_queue.rows.len(), 1);
        assert!(matches!(
            world.source_acquire_queue.rows[0].request.payload,
            crate::EcsSourcePayload::ProceduralRecipe(_)
        ));
        assert_eq!(world.decoded_page_queue.rows.len(), 1);
        let decoded = &world.decoded_page_queue.rows[0];
        assert_eq!(decoded.payload_kind, EcsDecodedPagePayloadKind::VoxelBrick);
        assert!(decoded.voxel_brick.is_some());
        assert!(
            world.renderer_handoff_queue.items.iter().any(|handoff| {
                handoff.kind == RendererArtifactHandoffKind::TerrainSurfacePackets
            })
        );
    }

    #[test]
    fn decoded_page_fixture_matches_direct_function_digest() {
        let key = page(3);
        let mut scheduled = FunWorld::default();
        scheduled
            .decoded_page_queue
            .push(decoded_page(key))
            .expect("push scheduled decoded page");
        let mut direct = FunWorld::default();
        direct
            .decoded_page_queue
            .push(decoded_page(key))
            .expect("push direct decoded page");

        let scheduled_report = scheduled
            .run_spatial_frame_deterministic()
            .expect("scheduled spatial frame runs");
        let direct_digest =
            run_reference_artifact_and_handoff_path(&mut direct).expect("direct path runs");

        assert_eq!(scheduled_report.scheduler_report.metrics.rejected_count, 0);
        assert_eq!(scheduled_report.digest, direct_digest);
        assert_eq!(
            scheduled.derived_artifact_registry.len(),
            direct.derived_artifact_registry.len()
        );
        assert_eq!(
            scheduled.renderer_handoff_queue.items.len(),
            direct.renderer_handoff_queue.items.len()
        );
    }

    #[test]
    fn blocking_source_acquire_is_rejected_off_blocking_lane() {
        let mut world = FunWorld::default();
        let mut runner = EcsNodeRunner::new(&mut world);
        let mut work = EcsWork {
            kind: EcsWorkKind::RunSystem,
            class: EcsSystemClass::SourceAcquire,
            system: None,
            access: SystemAccess::new(),
            chunk_key: EcsChunkKey::WHOLE_WORLD,
            command_buffer: None,
            deterministic_key: DeterministicDescriptor::new("source_acquire", 1),
            world_revision: EcsWorldRevision::new(0),
            awaited_tokens: Vec::new(),
            produced_tokens: Vec::new(),
            liveness_class: EcsLivenessClass::Normal,
            execution: EcsSystemExecutionContract {
                domain: ScheduleDomain::Blocking,
                lane: ScheduleLane::EcsSystem,
                blocking_class: WorkBlockingClass::BlockingPool,
                ..EcsSystemExecutionContract::default()
            },
            non_send: false,
            main_thread_only: false,
        };
        work.execution.lane = ScheduleLane::EcsSystem;
        let node = WorkNode::new(
            WorkNodeId::new(0),
            ScheduleDomain::FunEcs,
            ScheduleLane::EcsSystem,
            EcsWorkKind::RunSystem.phase(),
            TaskPriority::Normal,
            ScheduleBudget::UNBOUNDED,
            ScheduleDeadline::Stream,
            work,
        );

        let err = runner.run_node(&node).expect_err("blocking lane rejected");
        assert!(matches!(
            err,
            EcsNodeError::BlockingOutsideSchedulerLane { .. }
        ));
    }
}
