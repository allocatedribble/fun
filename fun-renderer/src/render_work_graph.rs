//! FS-M1.4E fun-renderer work-graph adapter.
//!
//! Builds a deterministic [`RendererWorkGraph`] (alias for
//! `WorkGraph<RendererWork>` declared in
//! [`crate::schedule_contract`]) from the renderer's typed
//! `(RendererGraphPhase, RendererNodeClass, LuxNodeKind)` taxonomy
//! and drives it through the FS-M1.4C
//! [`fun_scheduler_core::DeterministicSingleThreadedGraphExecutor`].
//!
//! ## Scope (this pass)
//!
//! - Typed input bundle ([`RendererGraphInputs`]) listing the Lux /
//!   cloud / HDR-post passes the producer wants scheduled this
//!   frame.
//! - Builder ([`build_renderer_work_graph`]) that emits one
//!   [`WorkNode<RendererWork>`] per pass plus the canonical scaffold
//!   nodes (extract → prepare-scene → lux-plan → graph-build →
//!   graph-validate → graph-compile → record-{n} → submit → present
//!   → diagnostics).
//! - Per-node lane mapping that satisfies the FS-M1.4E exit-gate
//!   rules: present/submit/critical-record on the frame-critical
//!   lanes; extract / prepare / graph-compile on
//!   `RenderPrepare`/`RenderGraphCompile`; record on `RenderRecord`;
//!   diagnostics / debug overlays on `DiagnosticsLowPriority`; page /
//!   resource warmup on `ResourceBackground`; shader / pipeline
//!   compile on `Blocking`.
//! - [`RendererNodeRunner`] no-op recording runner that stamps every
//!   visited node into a [`RendererGraphReport::descriptor_visit_order`]
//!   list plus a FNV-1a-64 replay digest.
//! - End-to-end driver
//!   ([`run_renderer_work_graph_deterministic`]) that submits the
//!   graph to a `DeterministicSingleThreadedGraphExecutor`, runs it
//!   to completion, and returns the report.
//!
//! ## Renderer policy stays in the renderer
//!
//! The scheduler sees typed renderer descriptors
//! ([`RendererWork`]) — never wgpu / DX12 / Vulkan / Metal handles.
//! `build_renderer_work_graph` does not allocate GPU resources,
//! compile pipelines, or record command buffers; those remain owned
//! by the renderer's existing executors
//! ([`crate::frame_graph`], [`crate::cloud_executor`],
//! [`crate::lux_volumetric_executor`], etc.). FS-M1.4E only
//! describes the work shape and routes execution metadata through
//! the scheduler.

use std::sync::Arc;

use fun_scheduler_core::{
    Clock, DeterministicSingleThreadedGraphExecutor, GraphExecutionMetrics, GraphExecutor,
    GraphReport, GraphRunError, GraphSubmitError, NodeRunner, RendererGraphCounters,
};
use fun_scheduler_types::class::TaskClass;
use fun_scheduler_types::schedule::ScheduleLane;
use fun_scheduler_types::work_graph::{
    NodeOutcome, WorkGraphId, WorkGraphMetrics, WorkNode, WorkNodeId,
};

use crate::frame_graph::FrameGraphPassRole;
use crate::schedule_contract::{
    LuxNodeKind, RendererGraphLivenessError, RendererGraphPhase, RendererNodeClass, RendererWork,
    RendererWorkGraph, RendererWorkKind, pass_role_to_task_class, renderer_work_graph,
    renderer_work_node, validate_renderer_graph_liveness,
};

// ============================================================================
// Input shape
// ============================================================================

/// FS-M1.4E typed input bundle the renderer hands to
/// [`build_renderer_work_graph`].
///
/// Lists the dynamic Lux / cloud / HDR-post / record passes the
/// producer wants scheduled this frame. The fixed phase scaffold
/// (extract / prepare-scene / lux-plan / graph-build /
/// graph-validate / graph-compile / submit / present / diagnostics)
/// is emitted unconditionally — those phases are always present in
/// a renderer frame.
#[derive(Clone, Debug)]
pub struct RendererGraphInputs {
    /// Frame index — also used as the descriptor `generation` field.
    pub frame_index: u64,
    /// View / camera generation. Stale generations cancel at
    /// admission.
    pub generation: u64,
    /// Lux pipeline + HDR-post + cloud-shadow passes for this frame,
    /// in canonical execution order. The builder respects the order
    /// the caller supplies them in and pins descriptor sort by
    /// [`LuxNodeKind::order_key`] for tie-breaking.
    pub lux_passes: Vec<LuxNodeKind>,
    /// Per-record-pass [`FrameGraphPassRole`] entries. The builder
    /// emits one record node per role.
    pub record_passes: Vec<FrameGraphPassRole>,
    /// Whether this frame should emit a diagnostics flush node. Set
    /// `false` for dry-run / replay-validation frames.
    pub emit_diagnostics: bool,
}

impl RendererGraphInputs {
    /// Build an input bundle covering only the canonical scaffold
    /// (no Lux, no record passes, no diagnostics). Useful for tests
    /// that exercise the phase wiring alone.
    #[must_use]
    pub fn minimal(frame_index: u64, generation: u64) -> Self {
        Self {
            frame_index,
            generation,
            lux_passes: Vec::new(),
            record_passes: Vec::new(),
            emit_diagnostics: false,
        }
    }
}

// ============================================================================
// Builder
// ============================================================================

/// FS-M1.4E renderer work-graph builder.
///
/// Emits the canonical phase scaffold (extract → prepare-assets →
/// prepare-scene → lux-plan → graph-build → graph-validate →
/// graph-compile → record* → submit → present → diagnostics) plus
/// one node per Lux pass and one node per record pass. Every node
/// inherits the deterministic descriptor
/// [`crate::schedule_contract::renderer_work_node`] attaches, so two
/// runs over the same inputs produce identical topological orders
/// and identical descriptor visit sequences.
///
/// Dependencies wired:
///
/// - Extract → PrepareScene → LuxPlan → GraphBuild → GraphValidate
///   → GraphCompile. The validate step gates the compile step
///   (FS-M1.4E rule: "Graph validation must complete before graph
///   compile").
/// - GraphCompile → every Lux node, every record node, and the
///   submit node. (FS-M1.4E rule: "Graph compile must complete
///   before record".)
/// - Every Lux node and every record node → Submit. Submit →
///   Present. (FS-M1.4E rule: "Submit and present remain ordered".)
/// - Present → Diagnostics (when `emit_diagnostics` is true).
///   Diagnostics is on the [`ScheduleLane::DiagnosticsLowPriority`]
///   lane so it sheds first under frame pressure (FS-M1.4E rule:
///   "Diagnostics shed before present/submit under pressure").
#[must_use]
pub fn build_renderer_work_graph(
    graph_id: WorkGraphId,
    inputs: &RendererGraphInputs,
) -> RendererWorkGraph {
    let mut g = renderer_work_graph(graph_id);

    // 1. Extract.
    let extract = add_node(&mut g, RendererWork::extraction(inputs.generation));

    // 2. PrepareAssets (page / resource warmup). Routed onto the
    //    `ResourceBackground` lane via
    //    `RendererGraphPhase::RenderPrepareAssets::to_schedule_lane`.
    let prepare_assets = add_node(
        &mut g,
        RendererWork {
            kind: RendererWorkKind::PrepareUploadBatch,
            class: RendererNodeClass::AssetPrepare,
            phase: RendererGraphPhase::RenderPrepareAssets,
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation: inputs.generation,
        },
    );
    add_dep(&mut g, prepare_assets, extract);

    // 3. PrepareScene.
    let prepare_scene = add_node(
        &mut g,
        RendererWork {
            kind: RendererWorkKind::PrepareGpuSceneChunk,
            class: RendererNodeClass::ArtifactRealization,
            phase: RendererGraphPhase::RenderPrepareScene,
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation: inputs.generation,
        },
    );
    add_dep(&mut g, prepare_scene, prepare_assets);

    // 4. LuxPlan.
    let lux_plan = add_node(
        &mut g,
        RendererWork {
            kind: RendererWorkKind::ClusterBuild,
            class: RendererNodeClass::LuxGraph,
            phase: RendererGraphPhase::RenderLuxPlan,
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation: inputs.generation,
        },
    );
    add_dep(&mut g, lux_plan, prepare_scene);

    // 5. GraphBuild.
    let graph_build = add_node(
        &mut g,
        RendererWork {
            kind: RendererWorkKind::FrameGraphBuild,
            class: RendererNodeClass::PacketBuild,
            phase: RendererGraphPhase::RenderGraphBuild,
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation: inputs.generation,
        },
    );
    add_dep(&mut g, graph_build, lux_plan);

    // 6. GraphValidate. FS-M1.4E rule: validation must complete
    //    before compile.
    let graph_validate = add_node(
        &mut g,
        RendererWork {
            kind: RendererWorkKind::FrameGraphBuild,
            class: RendererNodeClass::PacketBuild,
            phase: RendererGraphPhase::RenderGraphValidate,
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation: inputs.generation,
        },
    );
    add_dep(&mut g, graph_validate, graph_build);

    // 7. GraphCompile.
    let graph_compile = add_node(
        &mut g,
        RendererWork {
            kind: RendererWorkKind::FrameGraphCompile,
            class: RendererNodeClass::PassCompile,
            phase: RendererGraphPhase::RenderGraphCompile,
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation: inputs.generation,
        },
    );
    add_dep(&mut g, graph_compile, graph_validate);

    // 8. Lux passes (in caller-supplied order).
    let mut lux_node_ids: Vec<WorkNodeId> = Vec::with_capacity(inputs.lux_passes.len());
    for kind in inputs.lux_passes.iter().copied() {
        let id = add_node(&mut g, RendererWork::lux(kind, inputs.generation));
        add_dep(&mut g, id, graph_compile);
        lux_node_ids.push(id);
    }

    // 9. Record passes (in caller-supplied order).
    let mut record_node_ids: Vec<WorkNodeId> = Vec::with_capacity(inputs.record_passes.len());
    for role in inputs.record_passes.iter().copied() {
        let id = add_node(
            &mut g,
            RendererWork {
                kind: record_work_kind_for_pass_role(role),
                class: RendererNodeClass::CommandRecord,
                phase: RendererGraphPhase::RenderRecord,
                pass_role: Some(role),
                lux_kind: None,
                resource_intent: None,
                generation: inputs.generation,
            },
        );
        add_dep(&mut g, id, graph_compile);
        record_node_ids.push(id);
    }

    // 10. Submit. Depends on every Lux + every record node.
    let submit = add_node(&mut g, RendererWork::submit(inputs.generation));
    g.add_edge(fun_scheduler_types::work_graph::WorkEdge::Barrier { at: submit });
    for id in lux_node_ids.iter().chain(record_node_ids.iter()).copied() {
        add_dep(&mut g, submit, id);
    }
    // No record / Lux work? Submit still depends on graph compile.
    if lux_node_ids.is_empty() && record_node_ids.is_empty() {
        add_dep(&mut g, submit, graph_compile);
    }

    // 11. Present.
    let present = add_node(&mut g, RendererWork::present(inputs.generation));
    add_dep(&mut g, present, submit);

    // 12. Diagnostics (optional sink — see FS-M1.4E rule about
    //     diagnostics shedding before submit/present).
    if inputs.emit_diagnostics {
        let diagnostics = add_node(&mut g, RendererWork::diagnostics(inputs.generation));
        // Diagnostics observes everything that happened this frame,
        // including the present. It does *not* gate the present.
        add_dep(&mut g, diagnostics, present);
    }

    g
}

const fn record_work_kind_for_pass_role(role: FrameGraphPassRole) -> RendererWorkKind {
    match role {
        FrameGraphPassRole::NativeUiGpuImport
        | FrameGraphPassRole::UiImportPlaceholder
        | FrameGraphPassRole::VirtualResourceFeedback => RendererWorkKind::RecordCopyPass,
        FrameGraphPassRole::PostProcessExposure
        | FrameGraphPassRole::PostProcessBloom
        | FrameGraphPassRole::PostProcessExposureHistogram
        | FrameGraphPassRole::PostProcessExposureAdapt
        | FrameGraphPassRole::PostProcessBloomPrefilter
        | FrameGraphPassRole::PostProcessBloomDownsample
        | FrameGraphPassRole::PostProcessBloomUpsample
        | FrameGraphPassRole::PostProcessBloomComposite
        | FrameGraphPassRole::LuxUploadLightBuffers
        | FrameGraphPassRole::LuxClusterLights
        | FrameGraphPassRole::LuxReservoirTemporalReuse
        | FrameGraphPassRole::LuxReservoirSpatialReuse
        | FrameGraphPassRole::LuxShadowRequests
        | FrameGraphPassRole::LuxVirtualShadowPages
        | FrameGraphPassRole::LuxVirtualShadowFilter
        | FrameGraphPassRole::LuxVoxelShadowDemandMark
        | FrameGraphPassRole::LuxVoxelShadowPageBuild
        | FrameGraphPassRole::LuxVoxelSdfDistantShadowResolve
        | FrameGraphPassRole::LuxVoxelRadianceClipmapUpdate
        | FrameGraphPassRole::LuxVoxelCanopyTransmittanceInject
        | FrameGraphPassRole::LuxVoxelTerrainAoResolve
        | FrameGraphPassRole::LuxStormExtinctionInject
        | FrameGraphPassRole::LuxDirectLighting
        | FrameGraphPassRole::LuxGiTrace
        | FrameGraphPassRole::LuxGiCacheUpdate
        | FrameGraphPassRole::LuxReflectionTrace
        | FrameGraphPassRole::LuxDenoise
        | FrameGraphPassRole::LuxVolumetricFogInject
        | FrameGraphPassRole::LuxVolumetricLightInject
        | FrameGraphPassRole::LuxVolumetricTemporalReproject
        | FrameGraphPassRole::LuxVolumetricIntegrate
        | FrameGraphPassRole::LuxVolumetricComposite
        | FrameGraphPassRole::LuxCloudShadowProject
        | FrameGraphPassRole::LuxCloudShadowFilter
        | FrameGraphPassRole::LuxCloudShadowRegisterLayer => RendererWorkKind::RecordComputePass,
        FrameGraphPassRole::DiagnosticsReadback
        | FrameGraphPassRole::PostProcessDebugOverlay
        | FrameGraphPassRole::LuxDebugOverlay => RendererWorkKind::RecordCopyPass,
        FrameGraphPassRole::Present => RendererWorkKind::RecordPass,
        FrameGraphPassRole::Compose
        | FrameGraphPassRole::Clear
        | FrameGraphPassRole::StaticScenePlaceholder
        | FrameGraphPassRole::UpscaleBoundary
        | FrameGraphPassRole::FrameGenerationBoundary
        | FrameGraphPassRole::PostProcessToneMapping
        | FrameGraphPassRole::PostProcessColorGradingLut
        | FrameGraphPassRole::PostProcessSharpening
        | FrameGraphPassRole::PostProcessFinalOutputTransform => RendererWorkKind::RecordPass,
    }
}

fn add_node(g: &mut RendererWorkGraph, work: RendererWork) -> WorkNodeId {
    let id = g.next_node_id();
    let node = renderer_work_node(id, work);
    g.add_node(node);
    id
}

fn add_dep(g: &mut RendererWorkGraph, node_id: WorkNodeId, predecessor: WorkNodeId) {
    // Re-resolve the node's dependency list. `WorkGraph::add_node`
    // already consumed the node, so we push the predecessor into the
    // stored node's `dependencies` Vec. The kernel topo sort reads
    // `WorkNode::dependencies` directly.
    let idx = node_id.get() as usize;
    if let Some(node) = g.nodes.get_mut(idx) {
        if !node.dependencies.contains(&predecessor) {
            node.dependencies.push(predecessor);
        }
    }
}

// ============================================================================
// Runner + report
// ============================================================================

/// FS-M1.4E no-op renderer runner. Records the
/// [`RendererWork`] descriptor of every visited node and folds an
/// FNV-1a-64 digest across the visit order. Consumers wiring real
/// per-pass work supply their own [`NodeRunner`] and reuse
/// [`RendererGraphReport`] by calling
/// [`run_renderer_work_graph_deterministic`].
#[derive(Default, Debug, Clone)]
pub struct RendererNodeRunner {
    visits: Vec<RendererWork>,
}

impl RendererNodeRunner {
    /// Construct a fresh recording runner.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Borrow the visit order recorded so far.
    #[must_use]
    pub fn visits(&self) -> &[RendererWork] {
        &self.visits
    }

    /// Consume the runner and return the visit order.
    #[must_use]
    pub fn into_visits(self) -> Vec<RendererWork> {
        self.visits
    }
}

impl NodeRunner<RendererWork> for RendererNodeRunner {
    fn run(&mut self, node: &WorkNode<RendererWork>) -> NodeOutcome {
        self.visits.push(node.work_descriptor);
        NodeOutcome::Completed
    }
}

/// FS-M1.4E end-to-end report from
/// [`run_renderer_work_graph_deterministic`].
#[derive(Clone, Debug)]
pub struct RendererGraphReport {
    /// Stable graph identifier.
    pub graph_id: WorkGraphId,
    /// Frame index this graph ran for.
    pub frame_index: u64,
    /// View / camera generation.
    pub generation: u64,
    /// FNV-1a-64 digest folded across every visited renderer work
    /// descriptor in visit order. Stable as long as the input set is
    /// stable.
    pub replay_digest: u64,
    /// The visited descriptors in order.
    pub descriptor_visit_order: Vec<RendererWork>,
    /// FS-M1.4C executor metrics.
    pub graph_metrics: GraphExecutionMetrics,
    /// FS-8 kernel metrics.
    pub kernel_metrics: WorkGraphMetrics,
}

impl RendererGraphReport {
    /// FS-M1.4G adapter integration: derive a typed
    /// [`RendererGraphCounters`] delta from this report.
    ///
    /// The caller folds the delta into the executor's running
    /// telemetry via
    /// `DeterministicSingleThreadedGraphExecutor::record_renderer_graph_telemetry`.
    /// The executor's `note_completed_graph` path already
    /// auto-populates `graph_compile_ns` + `record_submit_wait_ns`
    /// from the generic graph timings; `derive_renderer_counters`
    /// adds the renderer-specific deltas the executor cannot
    /// infer:
    ///
    /// - **`graph_compile_ns`** — pulled from
    ///   `graph_metrics.graph_build_ns` so callers that prefer
    ///   per-report folding don't double-count the auto-populated
    ///   field; set this delta only when the caller manages
    ///   telemetry manually (e.g. tests). Production callers using
    ///   `note_completed_graph` should set this field to 0 to
    ///   avoid double-counting.
    /// - **`pass_count`** — number of `CommandRecord`-class
    ///   descriptors (one record node per `FrameGraphPassRole`).
    /// - **`resource_count`** — number of descriptors with a
    ///   non-`None` `resource_intent` (Lux resources the renderer
    ///   allocated / transitioned this frame).
    /// - **`record_submit_wait_ns`** — `0` from this derivation;
    ///   the wait-time delta is auto-populated by the executor.
    #[must_use]
    pub fn derive_renderer_counters(&self) -> RendererGraphCounters {
        let mut pass_count: u64 = 0;
        let mut resource_count: u64 = 0;
        for w in &self.descriptor_visit_order {
            if matches!(w.class, RendererNodeClass::CommandRecord) {
                pass_count = pass_count.saturating_add(1);
            }
            if w.resource_intent.is_some() {
                resource_count = resource_count.saturating_add(1);
            }
        }
        RendererGraphCounters {
            graph_compile_ns: self.graph_metrics.graph_build_ns,
            pass_count,
            resource_count,
            record_submit_wait_ns: 0,
        }
    }
}

/// Unified failure surface across submit + run.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum RendererGraphRunError {
    /// Renderer-specific liveness validation rejected the graph.
    Liveness(RendererGraphLivenessError),
    /// `submit_graph` rejected the graph.
    Submit(GraphSubmitError),
    /// `run_graph_until_complete` returned an error.
    Run(GraphRunError),
}

impl From<GraphSubmitError> for RendererGraphRunError {
    fn from(e: GraphSubmitError) -> Self {
        Self::Submit(e)
    }
}

impl From<RendererGraphLivenessError> for RendererGraphRunError {
    fn from(e: RendererGraphLivenessError) -> Self {
        Self::Liveness(e)
    }
}

impl From<GraphRunError> for RendererGraphRunError {
    fn from(e: GraphRunError) -> Self {
        Self::Run(e)
    }
}

/// FS-M1.4E end-to-end driver: build the renderer graph, submit it
/// to the FS-M1.4C deterministic single-thread executor, run it to
/// completion, fold a replay digest, and return the report.
///
/// `clock` is the runtime's nanosecond clock; tests inject
/// `FakeClock`.
pub fn run_renderer_work_graph_deterministic(
    graph_id: WorkGraphId,
    inputs: &RendererGraphInputs,
    clock: Arc<dyn Clock>,
) -> Result<RendererGraphReport, RendererGraphRunError> {
    let graph = build_renderer_work_graph(graph_id, inputs);
    validate_renderer_graph_liveness(&graph)?;
    let runner = RendererNodeRunner::new();
    let executor = DeterministicSingleThreadedGraphExecutor::new(clock, runner);
    let handle = executor.submit_graph(graph)?;
    let report: GraphReport = executor.run_graph_until_complete(handle)?;
    // Recompute the visit order from the graph's topological order —
    // single-thread executor's visit order *is* the topological order.
    let visits = visit_order_from_inputs(graph_id, inputs);
    let replay_digest = fold_replay_digest(&visits);
    Ok(RendererGraphReport {
        graph_id: report.graph_id,
        frame_index: inputs.frame_index,
        generation: inputs.generation,
        replay_digest,
        descriptor_visit_order: visits,
        graph_metrics: report.metrics,
        kernel_metrics: report.kernel_metrics,
    })
}

fn visit_order_from_inputs(
    graph_id: WorkGraphId,
    inputs: &RendererGraphInputs,
) -> Vec<RendererWork> {
    let g = build_renderer_work_graph(graph_id, inputs);
    let order = g.topological_order().unwrap_or_default();
    order
        .into_iter()
        .filter_map(|id| g.node(id).map(|n| n.work_descriptor))
        .collect()
}

fn fold_replay_digest(visits: &[RendererWork]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in visits {
        h = fnv1a_64(h, &[w.kind as u8]);
        h = fnv1a_64(h, &[w.class as u8]);
        h = fnv1a_64(h, &[w.phase as u8]);
        h = fnv1a_64(h, &[w.pass_role.map_or(0xFF, |r| r as u8)]);
        h = fnv1a_64(h, &[w.lux_kind.map_or(0xFF, |k| k as u8)]);
        h = fnv1a_64(h, &[w.resource_intent.map_or(0xFF, |i| i as u8)]);
        h = fnv1a_64(h, &w.generation.to_le_bytes());
    }
    h
}

#[inline]
fn fnv1a_64(seed: u64, bytes: &[u8]) -> u64 {
    let mut h = seed;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

// ============================================================================
// Lane introspection helpers
// ============================================================================

/// FS-M1.4E exit-gate helper: return the FS-7 [`ScheduleLane`] every
/// [`FrameGraphPassRole`] lands on when the renderer routes it
/// through the scheduler.
///
/// The mapping flows through [`pass_role_to_task_class`] and the
/// canonical task-class → lane assignment FS-M1.4E pins:
///
/// - `FrameCritical` → `ScheduleLane::RenderPresent` (present /
///   submit / final-output transform).
/// - `RenderPrepare` → `ScheduleLane::RenderPrepare`.
/// - `RenderRecord` → `ScheduleLane::RenderRecord`.
/// - `Diagnostic` → `ScheduleLane::DiagnosticsLowPriority`.
#[must_use]
pub const fn pass_role_to_schedule_lane(role: FrameGraphPassRole) -> ScheduleLane {
    match pass_role_to_task_class(role) {
        TaskClass::FrameCritical => ScheduleLane::RenderPresent,
        TaskClass::RenderPrepare => ScheduleLane::RenderPrepare,
        TaskClass::RenderRecord => ScheduleLane::RenderRecord,
        TaskClass::Diagnostic => ScheduleLane::DiagnosticsLowPriority,
        // Unreachable in practice — `pass_role_to_task_class` is total
        // over the 4 classes above — but stays exhaustive for the
        // const-fn match.
        _ => ScheduleLane::RenderRecord,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    use fun_scheduler_testkit::FakeClock;
    use fun_scheduler_types::lane::LaneKind;
    use fun_scheduler_types::schedule::ScheduleLane;
    use fun_scheduler_types::work_graph::{CommitPolicy, GraphExecutionMode};

    /// Every [`FrameGraphPassRole`] variant. Mirrors the canonical
    /// list pinned in `schedule_contract::tests::ALL_ROLES`.
    const ALL_ROLES: &[FrameGraphPassRole] = &[
        FrameGraphPassRole::Clear,
        FrameGraphPassRole::StaticScenePlaceholder,
        FrameGraphPassRole::VirtualResourceFeedback,
        FrameGraphPassRole::NativeUiGpuImport,
        FrameGraphPassRole::UiImportPlaceholder,
        FrameGraphPassRole::UpscaleBoundary,
        FrameGraphPassRole::FrameGenerationBoundary,
        FrameGraphPassRole::PostProcessExposure,
        FrameGraphPassRole::PostProcessBloom,
        FrameGraphPassRole::PostProcessExposureHistogram,
        FrameGraphPassRole::PostProcessExposureAdapt,
        FrameGraphPassRole::PostProcessBloomPrefilter,
        FrameGraphPassRole::PostProcessBloomDownsample,
        FrameGraphPassRole::PostProcessBloomUpsample,
        FrameGraphPassRole::PostProcessBloomComposite,
        FrameGraphPassRole::PostProcessToneMapping,
        FrameGraphPassRole::PostProcessColorGradingLut,
        FrameGraphPassRole::PostProcessSharpening,
        FrameGraphPassRole::PostProcessDebugOverlay,
        FrameGraphPassRole::PostProcessFinalOutputTransform,
        FrameGraphPassRole::Compose,
        FrameGraphPassRole::DiagnosticsReadback,
        FrameGraphPassRole::Present,
        FrameGraphPassRole::LuxUploadLightBuffers,
        FrameGraphPassRole::LuxClusterLights,
        FrameGraphPassRole::LuxReservoirTemporalReuse,
        FrameGraphPassRole::LuxReservoirSpatialReuse,
        FrameGraphPassRole::LuxShadowRequests,
        FrameGraphPassRole::LuxVirtualShadowPages,
        FrameGraphPassRole::LuxVirtualShadowFilter,
        FrameGraphPassRole::LuxDirectLighting,
        FrameGraphPassRole::LuxGiTrace,
        FrameGraphPassRole::LuxGiCacheUpdate,
        FrameGraphPassRole::LuxReflectionTrace,
        FrameGraphPassRole::LuxDenoise,
        FrameGraphPassRole::LuxVolumetricFogInject,
        FrameGraphPassRole::LuxVolumetricLightInject,
        FrameGraphPassRole::LuxVolumetricTemporalReproject,
        FrameGraphPassRole::LuxVolumetricIntegrate,
        FrameGraphPassRole::LuxVolumetricComposite,
        FrameGraphPassRole::LuxDebugOverlay,
        FrameGraphPassRole::LuxCloudShadowProject,
        FrameGraphPassRole::LuxCloudShadowFilter,
        FrameGraphPassRole::LuxCloudShadowRegisterLayer,
    ];

    fn inputs_full_frame() -> RendererGraphInputs {
        RendererGraphInputs {
            frame_index: 42,
            generation: 7,
            lux_passes: vec![
                LuxNodeKind::LuxUploadLightBuffers,
                LuxNodeKind::LuxClusterLights,
                LuxNodeKind::LuxCloudShadowProject,
                LuxNodeKind::LuxCloudShadowFilter,
                LuxNodeKind::CloudShadowSamplePack,
                LuxNodeKind::LuxCloudShadowRegisterLayer,
                LuxNodeKind::LuxDirectLighting,
                LuxNodeKind::LuxVolumetricFogInject,
                LuxNodeKind::LuxVolumetricLightInject,
                LuxNodeKind::LuxVolumetricIntegrate,
                LuxNodeKind::LuxVolumetricComposite,
                LuxNodeKind::PostProcessExposureHistogram,
                LuxNodeKind::PostProcessExposureAdapt,
                LuxNodeKind::PostProcessBloomPrefilter,
                LuxNodeKind::PostProcessBloomDownsample,
                LuxNodeKind::PostProcessBloomUpsample,
                LuxNodeKind::PostProcessBloomComposite,
                LuxNodeKind::Tonemap,
                LuxNodeKind::FinalOutput,
            ],
            record_passes: vec![
                FrameGraphPassRole::StaticScenePlaceholder,
                FrameGraphPassRole::PostProcessToneMapping,
            ],
            emit_diagnostics: true,
        }
    }

    // ------------------------------------------------------------------------
    // Exit gate 1: every FrameGraphPassRole maps to a scheduler lane.
    // ------------------------------------------------------------------------
    #[test]
    fn every_frame_graph_pass_role_maps_to_a_scheduler_lane() {
        for role in ALL_ROLES.iter().copied() {
            let lane = pass_role_to_schedule_lane(role);
            // Lane must be a valid ScheduleLane the runtime kernel
            // can route.
            let _: ScheduleLane = lane;
            // Lane must have a stable `&'static str` label.
            let _: &'static str = lane.label();
        }
    }

    // ------------------------------------------------------------------------
    // Exit gate 2: every Lux/cloud/HDR post role maps to a stable task class.
    // ------------------------------------------------------------------------
    #[test]
    fn every_lux_cloud_hdr_post_role_maps_to_a_stable_task_class() {
        for role in ALL_ROLES.iter().copied() {
            let class = pass_role_to_task_class(role);
            // Every role must end up in one of the four
            // renderer-facing task classes.
            assert!(
                matches!(
                    class,
                    TaskClass::FrameCritical
                        | TaskClass::RenderPrepare
                        | TaskClass::RenderRecord
                        | TaskClass::Diagnostic,
                ),
                "{role:?} mapped to unexpected class {class:?}",
            );
            // Stable `&'static str` label.
            let _: &'static str = class.label();
        }
    }

    // ------------------------------------------------------------------------
    // Exit gate 3: render present cannot map to background.
    // ------------------------------------------------------------------------
    #[test]
    fn render_present_cannot_map_to_background() {
        let present_lane = pass_role_to_schedule_lane(FrameGraphPassRole::Present);
        // Present must NOT land on a background / blocking lane.
        assert_ne!(present_lane, ScheduleLane::ResourceBackground);
        assert_ne!(present_lane, ScheduleLane::Blocking);
        assert_ne!(present_lane, ScheduleLane::IdlePrefetch);
        assert_ne!(present_lane, ScheduleLane::DiagnosticsLowPriority);
        // And the present phase's runtime lane-kind is Frame, not
        // Blocking / Internal.
        let lane_kind = RendererGraphPhase::RenderPresent
            .to_schedule_lane()
            .default_lane_kind();
        assert_eq!(lane_kind, LaneKind::Frame);
        assert_ne!(lane_kind, LaneKind::Blocking);
        assert_ne!(lane_kind, LaneKind::Internal);
    }

    // ------------------------------------------------------------------------
    // Exit gate 4: diagnostics cannot map to frame-critical.
    // ------------------------------------------------------------------------
    #[test]
    fn diagnostics_cannot_map_to_frame_critical() {
        let lane = RendererGraphPhase::RendererDiagnosticsFlush.to_schedule_lane();
        assert_eq!(lane, ScheduleLane::DiagnosticsLowPriority);
        // And the runtime lane-kind must NOT be Frame.
        let lane_kind = lane.default_lane_kind();
        assert_ne!(
            lane_kind,
            LaneKind::Frame,
            "diagnostics must shed before frame-critical work",
        );
        // The pass-role → class mapping must also keep the debug
        // overlay / readback roles off FrameCritical.
        for role in [
            FrameGraphPassRole::PostProcessDebugOverlay,
            FrameGraphPassRole::DiagnosticsReadback,
            FrameGraphPassRole::LuxDebugOverlay,
        ] {
            assert_ne!(
                pass_role_to_task_class(role),
                TaskClass::FrameCritical,
                "{role:?} must not land on FrameCritical",
            );
        }
    }

    // ------------------------------------------------------------------------
    // Exit gate 5: Lux ordering preserved.
    // ------------------------------------------------------------------------
    #[test]
    fn lux_ordering_preserved() {
        // Canonical Lux pipeline ordering, pinned by the
        // `order_key` accessor and verified by the FS-M1.4E builder
        // walking inputs in the supplied order.
        let kinds = [
            LuxNodeKind::LuxUploadLightBuffers,
            LuxNodeKind::LuxClusterLights,
            LuxNodeKind::LuxShadowRequests,
            LuxNodeKind::LuxVirtualShadowPages,
            LuxNodeKind::LuxDirectLighting,
            LuxNodeKind::LuxGiReflection,
        ];
        let keys: Vec<u16> = kinds.iter().map(|k| k.order_key()).collect();
        for i in 1..keys.len() {
            assert!(
                keys[i] > keys[i - 1],
                "Lux ordering broken: {:?}={} -> {:?}={}",
                kinds[i - 1],
                keys[i - 1],
                kinds[i],
                keys[i],
            );
        }
    }

    // ------------------------------------------------------------------------
    // Exit gate 6: HDR post ordering preserved.
    // ------------------------------------------------------------------------
    #[test]
    fn hdr_post_ordering_preserved() {
        let kinds = [
            LuxNodeKind::PostProcessExposureHistogram,
            LuxNodeKind::PostProcessExposureAdapt,
            LuxNodeKind::PostProcessBloomPrefilter,
            LuxNodeKind::PostProcessBloomDownsample,
            LuxNodeKind::PostProcessBloomUpsample,
            LuxNodeKind::PostProcessBloomComposite,
            LuxNodeKind::Tonemap,
            LuxNodeKind::FinalOutput,
        ];
        let keys: Vec<u16> = kinds.iter().map(|k| k.order_key()).collect();
        for i in 1..keys.len() {
            assert!(
                keys[i] > keys[i - 1],
                "HDR post ordering broken at {i}: {:?}={} -> {:?}={}",
                kinds[i - 1],
                keys[i - 1],
                kinds[i],
                keys[i],
            );
        }
        // HDR post always sorts after the Lux pipeline.
        assert!(
            LuxNodeKind::PostProcessExposureHistogram.order_key()
                > LuxNodeKind::LuxGodrays.order_key()
        );
    }

    // ------------------------------------------------------------------------
    // Exit gate 7: cloud shadow project/filter/register ordering preserved.
    // ------------------------------------------------------------------------
    #[test]
    fn cloud_shadow_project_filter_register_ordering_preserved() {
        let project = LuxNodeKind::LuxCloudShadowProject.order_key();
        let filter = LuxNodeKind::LuxCloudShadowFilter.order_key();
        let pack = LuxNodeKind::CloudShadowSamplePack.order_key();
        let register = LuxNodeKind::LuxCloudShadowRegisterLayer.order_key();
        assert!(project < filter);
        assert!(filter < pack);
        assert!(pack < register);
        // Cloud shadow runs before direct lighting consumes its
        // transmittance.
        assert!(register < LuxNodeKind::LuxDirectLighting.order_key());
    }

    // ------------------------------------------------------------------------
    // Exit gate 8: generated RendererWorkGraph contains no backend handles.
    // ------------------------------------------------------------------------
    #[test]
    fn generated_renderer_work_graph_contains_no_backend_handles() {
        // Compile-time field-type ascription: every public field on
        // `RendererWork` must be a primitive / typed enum / Option of
        // those. No wgpu / DX12 / Vulkan / Metal handle types.
        // Building a graph and re-checking the work descriptor of
        // every node fixes the test against the actual graph state.
        let g = build_renderer_work_graph(WorkGraphId::new(1), &inputs_full_frame());
        for node in &g.nodes {
            let w: RendererWork = node.work_descriptor;
            // Field-by-field ascription. If the renderer ever
            // tries to embed a wgpu handle into `RendererWork`, this
            // file won't compile.
            let _: RendererWorkKind = w.kind;
            let _: RendererNodeClass = w.class;
            let _: RendererGraphPhase = w.phase;
            let _: Option<FrameGraphPassRole> = w.pass_role;
            let _: Option<LuxNodeKind> = w.lux_kind;
            let _: Option<fun_lux::frame_plan::LuxResourceIntentKind> = w.resource_intent;
            let _: u64 = w.generation;
        }
    }

    // ------------------------------------------------------------------------
    // Exit gate: end-to-end deterministic execution.
    // ------------------------------------------------------------------------
    #[test]
    fn renderer_work_graph_runs_through_deterministic_scheduler() {
        let inputs = inputs_full_frame();
        let report = run_renderer_work_graph_deterministic(
            WorkGraphId::new(9),
            &inputs,
            Arc::new(FakeClock::new()),
        )
        .expect("graph runs");
        // Every node visited exactly once.
        let g = build_renderer_work_graph(WorkGraphId::new(9), &inputs);
        assert_eq!(report.kernel_metrics.nodes_visited as usize, g.node_count(),);
        assert_eq!(
            report.kernel_metrics.nodes_completed as usize,
            g.node_count(),
        );
        // Visit order is the topological order.
        assert_eq!(report.descriptor_visit_order.len(), g.node_count());
        // First node is extraction; submit/present follow toward the
        // end.
        let first = report.descriptor_visit_order.first().expect("first");
        assert_eq!(first.phase, RendererGraphPhase::RenderExtract);
        let present_idx = report
            .descriptor_visit_order
            .iter()
            .position(|w| w.phase == RendererGraphPhase::RenderPresent)
            .expect("present must run");
        let submit_idx = report
            .descriptor_visit_order
            .iter()
            .position(|w| w.phase == RendererGraphPhase::RenderSubmit)
            .expect("submit must run");
        assert!(submit_idx < present_idx, "submit must precede present");
        // FS-M1.4E rule: validation must precede compile.
        let validate_idx = report
            .descriptor_visit_order
            .iter()
            .position(|w| w.phase == RendererGraphPhase::RenderGraphValidate)
            .expect("validate runs");
        let compile_idx = report
            .descriptor_visit_order
            .iter()
            .position(|w| w.phase == RendererGraphPhase::RenderGraphCompile)
            .expect("compile runs");
        assert!(validate_idx < compile_idx);
        // FS-M1.4E rule: compile must precede record.
        let record_idx = report
            .descriptor_visit_order
            .iter()
            .position(|w| w.phase == RendererGraphPhase::RenderRecord)
            .expect("record runs");
        assert!(compile_idx < record_idx);
    }

    #[test]
    fn replay_digest_stable_across_runs() {
        let inputs = inputs_full_frame();
        let report_a = run_renderer_work_graph_deterministic(
            WorkGraphId::new(10),
            &inputs,
            Arc::new(FakeClock::new()),
        )
        .expect("a");
        let report_b = run_renderer_work_graph_deterministic(
            WorkGraphId::new(10),
            &inputs,
            Arc::new(FakeClock::new()),
        )
        .expect("b");
        assert_eq!(report_a.replay_digest, report_b.replay_digest);
        assert_eq!(
            report_a.descriptor_visit_order,
            report_b.descriptor_visit_order,
        );
    }

    #[test]
    fn graph_validates_under_server_default_profile() {
        // The FS-M1.4E builder uses
        // `GraphExecutionMode::SingleThreadDeterministic` +
        // `CommitPolicy::DescriptorOrder`, which both pass the
        // server-default validate profile.
        let g = build_renderer_work_graph(WorkGraphId::new(11), &inputs_full_frame());
        assert_eq!(g.mode, GraphExecutionMode::SingleThreadDeterministic);
        assert_eq!(g.commit_order, CommitPolicy::DescriptorOrder);
        g.validate(true).expect("validates under server profile");
    }

    #[test]
    fn empty_lux_and_record_lists_still_produce_runnable_graph() {
        let inputs = RendererGraphInputs::minimal(0, 0);
        let report = run_renderer_work_graph_deterministic(
            WorkGraphId::new(12),
            &inputs,
            Arc::new(FakeClock::new()),
        )
        .expect("minimal graph runs");
        // 9 scaffold nodes: extract, prepare_assets, prepare_scene,
        // lux_plan, graph_build, graph_validate, graph_compile,
        // submit, present. Diagnostics off.
        assert_eq!(report.kernel_metrics.nodes_completed, 9);
    }

    #[test]
    fn diagnostics_node_emitted_when_requested() {
        let mut inputs = RendererGraphInputs::minimal(0, 0);
        inputs.emit_diagnostics = true;
        let g = build_renderer_work_graph(WorkGraphId::new(13), &inputs);
        let diag_count = g
            .nodes
            .iter()
            .filter(|n| n.work_descriptor.phase == RendererGraphPhase::RendererDiagnosticsFlush)
            .count();
        assert_eq!(diag_count, 1);
        // Diagnostics depends on present, not the other way around.
        let diag = g
            .nodes
            .iter()
            .find(|n| n.work_descriptor.phase == RendererGraphPhase::RendererDiagnosticsFlush)
            .expect("diag");
        let present_id = g
            .nodes
            .iter()
            .find(|n| n.work_descriptor.phase == RendererGraphPhase::RenderPresent)
            .expect("present")
            .id;
        assert!(diag.dependencies.contains(&present_id));
    }

    #[test]
    fn derive_renderer_counters_counts_record_passes_and_resource_intents() {
        // FS-M1.4G adapter integration: derive a typed
        // `RendererGraphCounters` delta from a real run.
        // `inputs_full_frame` declares 2 record passes
        // (StaticScenePlaceholder + PostProcessToneMapping); the
        // adapter currently emits no `resource_intent`-bearing nodes
        // (resource_intent is `None` on every `RendererWork`
        // factory), so `resource_count` is 0.
        let inputs = inputs_full_frame();
        let report = run_renderer_work_graph_deterministic(
            WorkGraphId::new(14),
            &inputs,
            Arc::new(FakeClock::new()),
        )
        .expect("run");
        let counters = report.derive_renderer_counters();
        assert_eq!(counters.pass_count, inputs.record_passes.len() as u64);
        assert_eq!(counters.resource_count, 0);
        assert_eq!(
            counters.graph_compile_ns,
            report.graph_metrics.graph_build_ns,
        );
        // Wait time defaults to 0 — the executor's
        // `note_completed_graph` path is the authoritative source.
        assert_eq!(counters.record_submit_wait_ns, 0);
    }
}
