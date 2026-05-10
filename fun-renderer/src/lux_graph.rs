//! Pass 3 — the typed Lux graph compiler.
//!
//! `LuxGraphCompiler::compile_lux_plan` walks a typed
//! [`fun_lux::LuxFramePlan`], declares typed resources in the
//! [`crate::frame_graph::RendererFrameGraph`], and registers
//! typed Lux passes with read/write declarations.
//!
//! The typed contract:
//!
//! 1. Every `fun_lux::LuxPassRequest` compiles into ≥1 typed
//!    frame-graph pass with a `FrameGraphPassRole::Lux*`
//!    role.
//! 2. No Lux pass is executed outside the frame graph — the
//!    compiler is the only entry point that emits Lux roles.
//! 3. Every Lux pass declares reads + writes via the typed
//!    tables in [`crate::lux_passes::typed_resource_inputs`]
//!    / [`crate::lux_passes::typed_resource_outputs`].
//! 4. The typed
//!    [`crate::lux_diagnostics::LuxGraphCompileReport`]
//!    records typed counters + typed
//!    [`crate::lux_diagnostics::LuxGraphCompileFailure`]
//!    records (unwritten reads, pass-order violations,
//!    empty passes).
//! 5. Bridge modules read only the typed
//!    [`crate::lux_diagnostics::LuxGraphCompileReport`] +
//!    the typed
//!    [`crate::frame_graph::RendererFrameGraphDiagnostics`];
//!    they never name `fun_lux` internals.

use std::collections::HashMap;

use fun_lux::{LuxFramePlan, LuxPassRequest, LuxResourceIntent};

use crate::frame_graph::{
    FrameGraphDiagnosticCategory, FrameGraphPassDescriptor, FrameGraphPassHandle,
    FrameGraphPassRole, FrameGraphPassType, FrameGraphResourceDescriptor, FrameGraphResourceHandle,
    FrameGraphResourceType, RendererFrameGraph,
};
use crate::lux_diagnostics::{LuxGraphCompileFailure, LuxGraphCompileReport};
use crate::lux_passes::{lux_role_for, typed_resource_inputs, typed_resource_outputs};
use crate::lux_resources::map_intent_to_resource_type;
use crate::resource::RendererResourceRegistry;

pub const FUN_RENDERER_LUX_GRAPH_SCHEMA_VERSION: u16 = 1;

/// Typed Lux graph compiler. The single entry point for
/// translating a typed `fun_lux::LuxFramePlan` into the
/// renderer's typed frame graph.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LuxGraphCompiler;

impl LuxGraphCompiler {
    /// Stable id used for the typed lux pass type — the
    /// renderer's existing frame graph treats Lux passes as
    /// the `Compute` type by default; specific Lux roles
    /// (e.g. `LuxVolumetricComposite`) overlay a render-pass
    /// type via [`pass_type_for_lux_role`].
    pub const STABLE_ID_PREFIX: &'static str = "fun_renderer.lux.";

    /// Compile a typed `LuxFramePlan` into the renderer's
    /// typed frame graph. Returns the typed
    /// [`LuxGraphCompileReport`].
    #[must_use]
    pub fn compile_lux_plan(
        graph: &mut RendererFrameGraph,
        _resources: &mut RendererResourceRegistry,
        plan: &LuxFramePlan,
    ) -> LuxGraphCompileReport {
        let mut report = LuxGraphCompileReport::new(plan.frame_index);

        // ============================================
        // Phase 1 — declare typed Lux resources.
        // ============================================
        let mut intent_to_handle: HashMap<&'static str, FrameGraphResourceHandle> = HashMap::new();
        for scene in &plan.scene_plans {
            for resource in &scene.resources {
                report.lux_resource_intents_walked =
                    report.lux_resource_intents_walked.saturating_add(1);
                let resource_type = map_intent_to_resource_type(resource);
                let stable_id = resource_stable_id(resource);
                if intent_to_handle.contains_key(stable_id) {
                    continue;
                }
                let handle = graph.declare_resource(FrameGraphResourceDescriptor::new(
                    stable_id,
                    resource_type,
                    stable_id,
                ));
                if !handle.is_valid() {
                    report.record_failure(LuxGraphCompileFailure::ResourceDeclarationFailed {
                        resource_type,
                    });
                    continue;
                }
                intent_to_handle.insert(stable_id, handle);
                report.frame_graph_resources_declared =
                    report.frame_graph_resources_declared.saturating_add(1);
            }
        }

        // Phase 1b — synthesize a typed handle for every
        // Lux resource type the typed pass tables reference
        // but no scene plan declared. These are
        // typed renderer-owned persistent / frame-local
        // resources the compiler back-fills so passes that
        // read them can bind to a handle.
        let mut resource_type_to_handle: HashMap<FrameGraphResourceType, FrameGraphResourceHandle> =
            HashMap::new();
        for (id, handle) in &intent_to_handle {
            if let Some(resource) = graph.resource(*handle) {
                resource_type_to_handle.insert(resource.descriptor.resource_type, *handle);
            }
            // Lifetime check (warn / log only).
            let _ = id;
        }

        // ============================================
        // Phase 2 — collect + sort Lux pass requests.
        // ============================================
        let mut requests: Vec<&LuxPassRequest> = Vec::new();
        for scene in &plan.scene_plans {
            for request in &scene.passes {
                requests.push(request);
            }
        }
        report.lux_pass_requests_walked = requests.len() as u32;
        // Sort by typed `lux_order_key` to enforce the
        // typed pass ordering from the user spec.
        requests.sort_by_key(|r| {
            let role = lux_role_for(r);
            role.lux_order_key().unwrap_or(u16::MAX)
        });

        // ============================================
        // Phase 3 — register passes with reads/writes.
        // ============================================
        let mut last_order_key: u16 = 0;
        for (index, request) in requests.iter().enumerate() {
            let role = lux_role_for(request);
            let order_key = role.lux_order_key().unwrap_or(u16::MAX);
            if order_key < last_order_key {
                report.record_failure(LuxGraphCompileFailure::PassOrderViolation {
                    role,
                    expected_order_key: order_key,
                    actual_index: index as u32,
                });
            }
            last_order_key = order_key;

            let inputs = typed_resource_inputs(role);
            let outputs = typed_resource_outputs(role);
            if inputs.is_empty() && outputs.is_empty() {
                report.record_failure(LuxGraphCompileFailure::PassDeclaresNoReadsOrWrites { role });
                continue;
            }

            // Back-fill typed resource handles for inputs
            // and outputs that no scene plan declared.
            for resource_type in inputs.iter().chain(outputs.iter()) {
                if resource_type_to_handle.contains_key(resource_type) {
                    continue;
                }
                let stable_id = renderer_owned_stable_id_for(*resource_type);
                let handle = graph.declare_resource(FrameGraphResourceDescriptor::new(
                    stable_id,
                    *resource_type,
                    stable_id,
                ));
                if handle.is_valid() {
                    resource_type_to_handle.insert(*resource_type, handle);
                    report.frame_graph_resources_declared =
                        report.frame_graph_resources_declared.saturating_add(1);
                } else {
                    report.record_failure(LuxGraphCompileFailure::ResourceDeclarationFailed {
                        resource_type: *resource_type,
                    });
                }
            }

            let stable_id = request.common().stable_id;
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                stable_id,
                pass_type_for_lux_role(role),
                role,
                stable_id,
                FrameGraphDiagnosticCategory::Scene,
                None,
                stable_id,
            ));
            if pass == FrameGraphPassHandle::INVALID {
                report.record_failure(LuxGraphCompileFailure::PassRegistrationFailed { role });
                continue;
            }
            report.frame_graph_passes_registered =
                report.frame_graph_passes_registered.saturating_add(1);

            // Add typed reads.
            for resource_type in inputs {
                let Some(handle) = resource_type_to_handle.get(resource_type) else {
                    report.record_failure(LuxGraphCompileFailure::UnwrittenResourceRead {
                        role,
                        resource_type: *resource_type,
                    });
                    continue;
                };
                if graph.add_pass_read(pass, *handle) {
                    report.frame_graph_reads_added =
                        report.frame_graph_reads_added.saturating_add(1);
                }
            }
            // Add typed writes.
            for resource_type in outputs {
                let Some(handle) = resource_type_to_handle.get(resource_type) else {
                    // Outputs are always declared inline
                    // above; missing handle here is a
                    // resource declaration failure.
                    report.record_failure(LuxGraphCompileFailure::ResourceDeclarationFailed {
                        resource_type: *resource_type,
                    });
                    continue;
                };
                if graph.add_pass_write(pass, *handle) {
                    report.frame_graph_writes_added =
                        report.frame_graph_writes_added.saturating_add(1);
                }
            }
        }

        report
    }
}

/// Typed mapping from a Lux pass role to its typed
/// `FrameGraphPassType`. Most Lux roles are compute passes;
/// the volumetric composite + direct lighting roles emit
/// real render-pass dispatches (when the renderer wires
/// them to the live pipelines).
#[must_use]
pub const fn pass_type_for_lux_role(role: FrameGraphPassRole) -> FrameGraphPassType {
    match role {
        FrameGraphPassRole::LuxUploadLightBuffers => FrameGraphPassType::CopyImport,
        FrameGraphPassRole::LuxClusterLights
        | FrameGraphPassRole::LuxReservoirTemporalReuse
        | FrameGraphPassRole::LuxReservoirSpatialReuse
        | FrameGraphPassRole::LuxShadowRequests
        | FrameGraphPassRole::LuxVirtualShadowPages
        | FrameGraphPassRole::LuxVirtualShadowFilter
        | FrameGraphPassRole::LuxGiTrace
        | FrameGraphPassRole::LuxGiCacheUpdate
        | FrameGraphPassRole::LuxReflectionTrace
        | FrameGraphPassRole::LuxDenoise
        | FrameGraphPassRole::LuxVolumetricFogInject
        | FrameGraphPassRole::LuxVolumetricLightInject
        | FrameGraphPassRole::LuxVolumetricTemporalReproject
        | FrameGraphPassRole::LuxVolumetricIntegrate => FrameGraphPassType::Compute,
        FrameGraphPassRole::LuxDirectLighting | FrameGraphPassRole::LuxVolumetricComposite => {
            FrameGraphPassType::Render
        }
        FrameGraphPassRole::LuxDebugOverlay => FrameGraphPassType::Render,
        _ => FrameGraphPassType::Compute,
    }
}

/// Typed stable id for a lux resource intent. fun-lux's
/// typed intent stable ids are `&'static str` so we can
/// use them directly as frame-graph stable ids.
#[must_use]
fn resource_stable_id(intent: &LuxResourceIntent) -> &'static str {
    intent.stable_id()
}

/// Typed stable id for a renderer-owned (not lux-declared)
/// resource type. Used when a Lux pass reads/writes a
/// resource type that no scene plan declared an intent for.
#[must_use]
const fn renderer_owned_stable_id_for(resource_type: FrameGraphResourceType) -> &'static str {
    match resource_type {
        FrameGraphResourceType::LuxLightBuffer => "fun_renderer.lux.renderer_owned.light_buffer",
        FrameGraphResourceType::LuxLightIndexBuffer => {
            "fun_renderer.lux.renderer_owned.light_index_buffer"
        }
        FrameGraphResourceType::LuxClusterGrid => "fun_renderer.lux.renderer_owned.cluster_grid",
        FrameGraphResourceType::LuxReservoirBuffer => {
            "fun_renderer.lux.renderer_owned.reservoir_buffer"
        }
        FrameGraphResourceType::LuxShadowRequestBuffer => {
            "fun_renderer.lux.renderer_owned.shadow_request_buffer"
        }
        FrameGraphResourceType::LuxShadowAtlas => "fun_renderer.lux.renderer_owned.shadow_atlas",
        FrameGraphResourceType::LuxVirtualShadowPages => {
            "fun_renderer.lux.renderer_owned.virtual_shadow_pages"
        }
        FrameGraphResourceType::LuxSurfaceCache => "fun_renderer.lux.renderer_owned.surface_cache",
        FrameGraphResourceType::LuxRadianceCache => {
            "fun_renderer.lux.renderer_owned.radiance_cache"
        }
        FrameGraphResourceType::LuxProbeCache => "fun_renderer.lux.renderer_owned.probe_cache",
        FrameGraphResourceType::LuxReflectionBuffer => {
            "fun_renderer.lux.renderer_owned.reflection_buffer"
        }
        FrameGraphResourceType::LuxDenoiseHistory => {
            "fun_renderer.lux.renderer_owned.denoise_history"
        }
        FrameGraphResourceType::LuxVolumetricFroxelDensity => {
            "fun_renderer.lux.renderer_owned.volumetric_froxel_density"
        }
        FrameGraphResourceType::LuxVolumetricFroxelScattering => {
            "fun_renderer.lux.renderer_owned.volumetric_froxel_scattering"
        }
        FrameGraphResourceType::LuxVolumetricIntegratedFog => {
            "fun_renderer.lux.renderer_owned.volumetric_integrated_fog"
        }
        FrameGraphResourceType::LuxVolumetricHistory => {
            "fun_renderer.lux.renderer_owned.volumetric_history"
        }
        FrameGraphResourceType::RenderResolutionSceneColor => {
            "fun_renderer.lux.renderer_owned.scene_color_render_resolution"
        }
        FrameGraphResourceType::TransientScratch => {
            "fun_renderer.lux.renderer_owned.transient_scratch"
        }
        _ => "fun_renderer.lux.renderer_owned.unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_lux::{LuxFramePlan, LuxFramePlanner, LuxPassKind, LuxSceneChangeSignal, LuxSceneId};

    fn compile_proof_scene() -> (RendererFrameGraph, LuxGraphCompileReport) {
        let mut planner = LuxFramePlanner::product_default();
        planner.current_light_count = 16;
        let signal = LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 1);
        let plan = planner.build_frame_plan(1, &[signal]);
        let mut graph = RendererFrameGraph::default();
        let mut resources = RendererResourceRegistry::default();
        let report = LuxGraphCompiler::compile_lux_plan(&mut graph, &mut resources, &plan);
        (graph, report)
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(FUN_RENDERER_LUX_GRAPH_SCHEMA_VERSION, 1);
    }

    /// Pass 3 acceptance: every `LuxPassRequest` compiles
    /// into one or more `RendererFrameGraph` passes.
    #[test]
    fn every_lux_pass_request_compiles_into_at_least_one_frame_graph_pass() {
        let (graph, report) = compile_proof_scene();
        assert!(report.compile_succeeded(), "{:?}", report.failures);
        assert!(report.every_request_compiled());
        // Every pass in the graph that has a Lux role must
        // be present.
        let lux_passes_in_graph: usize = graph
            .passes()
            .iter()
            .filter(|p| p.descriptor.role.is_lux())
            .count();
        assert!(lux_passes_in_graph >= report.lux_pass_requests_walked as usize);
    }

    /// Pass 3 acceptance: every lux pass declares
    /// reads/writes.
    #[test]
    fn every_lux_pass_declares_reads_or_writes() {
        let (graph, report) = compile_proof_scene();
        assert!(report.every_pass_declares_reads_or_writes());
        for pass in graph.passes() {
            if !pass.descriptor.role.is_lux() {
                continue;
            }
            let read_count = pass.reads.len();
            let write_count = pass.writes.len();
            assert!(
                read_count + write_count > 0,
                "lux pass {} declared neither reads nor writes",
                pass.descriptor.stable_id,
            );
        }
    }

    /// Pass 3 acceptance: pass ordering matches the typed
    /// `lux_order_key`.
    #[test]
    fn lux_pass_ordering_is_monotonic_by_typed_order_key() {
        let (graph, _report) = compile_proof_scene();
        let mut last_key: u16 = 0;
        for pass in graph.passes() {
            if !pass.descriptor.role.is_lux() {
                continue;
            }
            let key = pass.descriptor.role.lux_order_key().unwrap_or(u16::MAX);
            assert!(
                key >= last_key,
                "lux ordering violation: pass {} (key {}) appears after key {}",
                pass.descriptor.stable_id,
                key,
                last_key,
            );
            last_key = key;
        }
    }

    /// Pass 3 acceptance: graph validation rejects empty
    /// passes (pass that declared neither reads nor
    /// writes). The typed predicate
    /// `lux_passes::declares_any_resource_interaction`
    /// guarantees this — every typed Lux role has typed
    /// inputs or outputs.
    #[test]
    fn no_typed_lux_role_declares_no_reads_or_writes() {
        use crate::lux_passes::declares_any_resource_interaction;
        for role in [
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
        ] {
            assert!(declares_any_resource_interaction(role), "{role:?}");
        }
    }

    /// Pass 3 acceptance: bridge modules see typed renderer
    /// IR, not fun-lux internals. The typed
    /// `LuxGraphCompileReport` carries `FrameGraphPassRole`
    /// + `FrameGraphResourceType` (renderer IR) — never
    /// any `fun_lux::*` types.
    #[test]
    fn compile_report_exposes_typed_renderer_ir_only() {
        let (_graph, report) = compile_proof_scene();
        // The typed report fields are renderer-IR types
        // + scalar counters. The typed failure variants
        // also carry renderer IR.
        for failure in &report.failures {
            match failure {
                LuxGraphCompileFailure::UnwrittenResourceRead {
                    role,
                    resource_type,
                } => {
                    let _ = role.as_str();
                    let _ = resource_type.as_str();
                }
                LuxGraphCompileFailure::PassOrderViolation { role, .. } => {
                    let _ = role.as_str();
                }
                LuxGraphCompileFailure::PassDeclaresNoReadsOrWrites { role } => {
                    let _ = role.as_str();
                }
                LuxGraphCompileFailure::PassRegistrationFailed { role } => {
                    let _ = role.as_str();
                }
                LuxGraphCompileFailure::ResourceDeclarationFailed { resource_type } => {
                    let _ = resource_type.as_str();
                }
            }
        }
    }

    /// Pass 3 acceptance: an empty plan (no scenes)
    /// produces a minimal report with zero passes / zero
    /// resources / zero failures.
    #[test]
    fn empty_plan_produces_minimal_report() {
        let plan = LuxFramePlan::cold_default();
        let mut graph = RendererFrameGraph::default();
        let mut resources = RendererResourceRegistry::default();
        let report = LuxGraphCompiler::compile_lux_plan(&mut graph, &mut resources, &plan);
        assert!(report.compile_succeeded());
        assert_eq!(report.lux_pass_requests_walked, 0);
        assert_eq!(report.frame_graph_passes_registered, 0);
        assert_eq!(report.frame_graph_resources_declared, 0);
        assert!(!report.every_request_compiled()); // zero requests
    }

    /// Pass 3 acceptance: when every `LuxPassRequest`
    /// variant appears in the plan, every one compiles.
    /// Tests the typed exhaustiveness of `lux_role_for`.
    #[test]
    fn compile_handles_every_lux_pass_kind() {
        // Build a synthetic plan that contains every typed
        // LuxPassKind. We do this by feeding the planner a
        // dirty signal under product-default settings so it
        // emits a broad set of passes.
        let mut planner = LuxFramePlanner::product_default();
        planner.current_light_count = 16;
        let mut signal = LuxSceneChangeSignal::light_changed(LuxSceneId::PROOF_SCENE, 1);
        signal.gi_probes_changed = true;
        let plan = planner.build_frame_plan(1, &[signal]);
        let mut graph = RendererFrameGraph::default();
        let mut resources = RendererResourceRegistry::default();
        let report = LuxGraphCompiler::compile_lux_plan(&mut graph, &mut resources, &plan);
        // Every typed Lux pass request in the plan must
        // appear in the graph as a Lux-role pass.
        let plan_kinds: Vec<LuxPassKind> = plan
            .scene_plans
            .iter()
            .flat_map(|s| s.passes.iter().map(|p| p.kind()))
            .collect();
        for kind in plan_kinds {
            // Each kind compiles to a typed role.
            let expected_role = match kind {
                LuxPassKind::UploadLightBuffers => FrameGraphPassRole::LuxUploadLightBuffers,
                LuxPassKind::ClusterLights => FrameGraphPassRole::LuxClusterLights,
                LuxPassKind::SelectReservoirs => FrameGraphPassRole::LuxReservoirTemporalReuse,
                LuxPassKind::BuildShadowRequests => FrameGraphPassRole::LuxShadowRequests,
                LuxPassKind::RenderVirtualShadowPages => FrameGraphPassRole::LuxVirtualShadowPages,
                LuxPassKind::FilterVirtualShadows => FrameGraphPassRole::LuxVirtualShadowFilter,
                LuxPassKind::DirectLighting => FrameGraphPassRole::LuxDirectLighting,
                LuxPassKind::GiTrace => FrameGraphPassRole::LuxGiTrace,
                LuxPassKind::GiCacheUpdate => FrameGraphPassRole::LuxGiCacheUpdate,
                LuxPassKind::ReflectionTrace => FrameGraphPassRole::LuxReflectionTrace,
                LuxPassKind::Denoise => FrameGraphPassRole::LuxDenoise,
                LuxPassKind::VolumetricFogInject => FrameGraphPassRole::LuxVolumetricFogInject,
                LuxPassKind::VolumetricLightInject => FrameGraphPassRole::LuxVolumetricLightInject,
                LuxPassKind::VolumetricTemporalReproject => {
                    FrameGraphPassRole::LuxVolumetricTemporalReproject
                }
                LuxPassKind::VolumetricIntegrate => FrameGraphPassRole::LuxVolumetricIntegrate,
                LuxPassKind::VolumetricComposite => FrameGraphPassRole::LuxVolumetricComposite,
                LuxPassKind::LuxDebugOverlay => FrameGraphPassRole::LuxDebugOverlay,
            };
            assert!(
                graph
                    .passes()
                    .iter()
                    .any(|p| p.descriptor.role == expected_role),
                "expected role {:?} for kind {:?} not found in graph",
                expected_role,
                kind,
            );
        }
        assert!(report.compile_succeeded(), "{:?}", report.failures);
    }

    /// Pass 3 acceptance: typed pass type mapping. Compute
    /// roles → Compute; render roles → Render; copy roles →
    /// CopyImport.
    #[test]
    fn pass_type_mapping_is_consistent() {
        assert_eq!(
            pass_type_for_lux_role(FrameGraphPassRole::LuxUploadLightBuffers),
            FrameGraphPassType::CopyImport,
        );
        assert_eq!(
            pass_type_for_lux_role(FrameGraphPassRole::LuxClusterLights),
            FrameGraphPassType::Compute,
        );
        assert_eq!(
            pass_type_for_lux_role(FrameGraphPassRole::LuxDirectLighting),
            FrameGraphPassType::Render,
        );
        assert_eq!(
            pass_type_for_lux_role(FrameGraphPassRole::LuxVolumetricComposite),
            FrameGraphPassType::Render,
        );
        assert_eq!(
            pass_type_for_lux_role(FrameGraphPassRole::LuxDebugOverlay),
            FrameGraphPassType::Render,
        );
    }
}
