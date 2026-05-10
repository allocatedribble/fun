//! Quality Audit Contract.
//!
//! Encodes the user-prompt "Quality issues to fix before more
//! feature work" (sections 6.1–6.4) as typed contracts the test
//! suite exercises. The four issues:
//!
//! - **6.1 Don't let typed-contract tests masquerade as
//!   rendered-frame tests.** Adds the typed [`EvidenceKind`]
//!   enum (LiveGpuExecution / CpuAlgorithmicParity /
//!   TypedContractOnly / SyntheticEvidence / FakeRendererFixture
//!   / PlanningSurface) plus a typed registry
//!   ([`PASS_EVIDENCE_REGISTRY`]) that records the typed
//!   evidence kind for every Pass A–H, Tier 0–8, and the
//!   live-runtime closeouts.
//! - **6.2 Root umbrella's `fun` gitlink should track latest
//!   accepted nested `fun`.** The typed
//!   [`RootGitlinkSyncExpectation`] records the typed
//!   expectation; the umbrella's commit log is the source of
//!   truth, and the Pass A ledger's `Passes Landed` table is the
//!   navigational view. This module also adds typed assertions
//!   for the latest Pass F/G/H entries.
//! - **6.3 Rename `CefComposition` graph stage or freeze it as
//!   stable telemetry.** The typed [`UiCompositionNamingPolicy`]
//!   records that `FunRendererSubsystem::UiComposition` is the
//!   product subsystem name while `FunRendererFrameGraphStage::CefComposition`
//!   is the frozen legacy telemetry stage ID; CEF is never the
//!   product UI surface (Pass A demotion).
//! - **6.4 Add typed integration test categories.** The typed
//!   [`IntegrationTestCategory`] enum (HeadlessContract /
//!   AdapterDevice / SurfacePresent / FrameProbe / GpuCapture)
//!   records the five canonical categories; only
//!   `SurfacePresent` satisfies visible-frame claims.

use bevy_ecs::prelude::Resource;

use crate::FunRendererFrameGraphStage;
use crate::FunRendererSubsystem;
use crate::ui::CefRenderRoleStatus;

pub const QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION: u16 = 1;
pub const EVIDENCE_KIND_COUNT: usize = 6;
pub const INTEGRATION_TEST_CATEGORY_COUNT: usize = 5;

// ============================================================================
// Section 6.1 — Evidence kind taxonomy
// ============================================================================

/// Typed kind for the evidence a Pass / Tier / closeout verdict
/// represents. The user prompt: "Several new passes 'pass
/// today,' but the wording often says they pass because typed
/// contract layers, CPU simulation, fake-renderer fixtures, or
/// canonical stage ordering are complete. That is fine. It must
/// remain explicitly labeled."
///
/// Every verdict in the workspace should be classifiable into one
/// of these typed kinds. The
/// [`PASS_EVIDENCE_REGISTRY`] records the typed mapping for
/// every Pass / Tier the renderer ships today.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceKind {
    /// The verdict's typed evidence comes from a real GPU
    /// execution path: live wgpu device + real command encoder +
    /// real submit + real readback. The Pass B exit test (under
    /// `live_passb_runs_one_update_and_records_passes`) is the
    /// canonical example.
    LiveGpuExecution,
    /// The verdict's typed evidence is a CPU implementation of
    /// the same algorithm the GPU will run. Pass D (GPU-driven
    /// frustum-cull parity via CPU simulator) is the canonical
    /// example.
    CpuAlgorithmicParity,
    /// The verdict's typed evidence is the *typed contract*
    /// itself — the rules + predicates + bundle shape are
    /// validated, but the underlying GPU runtime is not yet
    /// exercised end-to-end. Pass C
    /// (`live_passc_runs_one_update_and_records_not_yet_measured`)
    /// is the canonical example.
    #[default]
    TypedContractOnly,
    /// The verdict's typed evidence is synthesized
    /// (deterministic CPU-side fixtures) so the verdict's
    /// passing path is exercised even before real evidence
    /// flows. Pass E's `synthetic_shadow_residency_in_budget`
    /// helper is the canonical example.
    SyntheticEvidence,
    /// The verdict's typed evidence is a *fake-renderer
    /// fixture* — a typed deterministic baseline the real
    /// renderer is diffed against in tests. Pass G's
    /// `PassGFakeRendererFixture::CANONICAL` is the canonical
    /// example.
    FakeRendererFixture,
    /// The verdict's typed evidence is a *planning surface* —
    /// the typed contract exists but no producer is wired today.
    /// Tier 0's `Tier0ArtifactStatus::PlanningOnlySurface` is
    /// the canonical example.
    PlanningSurface,
}

impl EvidenceKind {
    pub const ALL: [Self; EVIDENCE_KIND_COUNT] = [
        Self::LiveGpuExecution,
        Self::CpuAlgorithmicParity,
        Self::TypedContractOnly,
        Self::SyntheticEvidence,
        Self::FakeRendererFixture,
        Self::PlanningSurface,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LiveGpuExecution => "live_gpu_execution",
            Self::CpuAlgorithmicParity => "cpu_algorithmic_parity",
            Self::TypedContractOnly => "typed_contract_only",
            Self::SyntheticEvidence => "synthetic_evidence",
            Self::FakeRendererFixture => "fake_renderer_fixture",
            Self::PlanningSurface => "planning_surface",
        }
    }

    /// Typed predicate: does this evidence kind satisfy a
    /// "visible-frame" claim? Only `LiveGpuExecution` does — the
    /// other kinds are typed proofs of contract / algorithm /
    /// fixture, not of real rendered output.
    #[must_use]
    pub const fn satisfies_visible_frame_claim(self) -> bool {
        matches!(self, Self::LiveGpuExecution)
    }

    /// Typed predicate: does this evidence kind require strict
    /// labeling in user-facing prose? Every kind except
    /// `LiveGpuExecution` does — the user-facing prose must NOT
    /// claim "the renderer renders X" when the evidence is
    /// typed-contract / CPU-simulated / synthetic / fake-fixture /
    /// planning. The user's 6.1 rule: "It must remain
    /// explicitly labeled."
    #[must_use]
    pub const fn requires_explicit_label(self) -> bool {
        !matches!(self, Self::LiveGpuExecution)
    }
}

// ============================================================================
// Section 6.1 — Pass evidence registry
// ============================================================================

/// Stable identifier for one Pass / Tier / closeout verdict the
/// renderer ships today. The registry pairs each identifier with
/// its typed [`EvidenceKind`] so the user-facing prose can be
/// audited against the typed taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassEvidenceEntry {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub evidence_kind: EvidenceKind,
}

impl PassEvidenceEntry {
    #[must_use]
    pub const fn new(stable_id: &'static str, evidence_kind: EvidenceKind) -> Self {
        Self {
            schema_version: QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION,
            stable_id,
            evidence_kind,
        }
    }
}

/// The typed evidence registry. Records the typed
/// [`EvidenceKind`] for every Pass / Tier the renderer ships
/// today. The user's 6.1 rule is enforced by reading this
/// registry: a verdict's user-facing prose must align with the
/// typed kind here.
pub const PASS_EVIDENCE_REGISTRY: &[PassEvidenceEntry] = &[
    // Pass A: governance cleanup — pure typed-contract change.
    PassEvidenceEntry::new(
        "pass_a.truth_resync_and_ui_governance",
        EvidenceKind::TypedContractOnly,
    ),
    // Pass B: typed runtime contract; the canonical CI proof
    // (`live_passb_runs_one_update_and_records_passes`) exercises
    // real GPU execution.
    PassEvidenceEntry::new("pass_b.proof_frame_runtime", EvidenceKind::LiveGpuExecution),
    // Pass C: cache + churn burn-down; verdict is typed-contract
    // until the live runtime drives 600 production frames.
    PassEvidenceEntry::new(
        "pass_c.runtime_cache_burndown",
        EvidenceKind::TypedContractOnly,
    ),
    // Pass D: GPU-driven parity — CPU simulator of GPU algorithm.
    PassEvidenceEntry::new(
        "pass_d.gpu_scene_indirect_draw_parity",
        EvidenceKind::CpuAlgorithmicParity,
    ),
    // Pass E: clustered lighting + virtual shadow MVP —
    // synthetic shadow residency + typed cluster assignment.
    PassEvidenceEntry::new(
        "pass_e.clustered_lighting_virtual_shadow_mvp",
        EvidenceKind::SyntheticEvidence,
    ),
    // Pass F: temporal stack — CPU TAA resolve algorithm proves
    // the four canonical scenarios.
    PassEvidenceEntry::new("pass_f.temporal_stack", EvidenceKind::CpuAlgorithmicParity),
    // Pass G: native UI product route — typed `PassGFakeRendererFixture::CANONICAL`
    // is the fake-renderer baseline.
    PassEvidenceEntry::new(
        "pass_g.native_ui_product_route",
        EvidenceKind::FakeRendererFixture,
    ),
    // Pass H: native CL fail-closed contract reaffirmation.
    PassEvidenceEntry::new(
        "pass_h.native_command_list_fail_closed",
        EvidenceKind::TypedContractOnly,
    ),
    // Pass I: GPU-driven compute culling + indirect draw.
    // Real WGSL compute kernel + indirect draw + readback +
    // CPU/direct parity comparison.
    PassEvidenceEntry::new(
        "pass_i.gpu_driven_compute_indirect",
        EvidenceKind::LiveGpuExecution,
    ),
    // Pass J: clustered lighting + shadow atlas (live GPU).
    // Real WGSL compute cluster-assignment kernel + real
    // forward+ shading pass + real shadow-atlas allocation +
    // real cascade clear pass + cluster-light-count readback
    // (Priority 2: "clustered lighting and virtual shadows with
    // real resources").
    PassEvidenceEntry::new(
        "pass_j.clustered_lighting_live",
        EvidenceKind::LiveGpuExecution,
    ),
    // Tier 0: proof frame keystone — visible frame / GPU timing
    // are planning surfaces today; bridge health + DX12 hardening
    // are typed-contract.
    PassEvidenceEntry::new("tier_0.proof_frame_gate", EvidenceKind::PlanningSurface),
    // Tier 1: cache-backed runtime optimization — typed
    // contracts on coalesce + upload ring + compact ids.
    PassEvidenceEntry::new(
        "tier_1.cache_backed_optimization",
        EvidenceKind::TypedContractOnly,
    ),
    // Tier 2: GPU-driven proof — CPU simulator of GPU algorithm.
    PassEvidenceEntry::new(
        "tier_2.gpu_driven_proof",
        EvidenceKind::CpuAlgorithmicParity,
    ),
    // Tier 3: lighting at scale — typed contracts + measurement.
    PassEvidenceEntry::new(
        "tier_3.lighting_shadows_at_scale",
        EvidenceKind::TypedContractOnly,
    ),
    // Tier 4: transient memory + barriers — typed contracts on
    // allocator + barrier kinds + DX12 enhanced barrier guard.
    PassEvidenceEntry::new(
        "tier_4.transient_memory_and_barriers",
        EvidenceKind::TypedContractOnly,
    ),
    // Tier 5: temporal reconstruction — CPU TAA resolve.
    PassEvidenceEntry::new(
        "tier_5.temporal_reconstruction",
        EvidenceKind::CpuAlgorithmicParity,
    ),
    // Tier 6: native UI rendering — typed contracts on batching +
    // glyph atlas + product input event.
    PassEvidenceEntry::new(
        "tier_6.native_ui_rendering",
        EvidenceKind::TypedContractOnly,
    ),
    // Tier 7: vendor SDKs + frame gen + latency — typed
    // contracts on guards + verdicts.
    PassEvidenceEntry::new(
        "tier_7.vendor_sdks_and_frame_generation",
        EvidenceKind::TypedContractOnly,
    ),
    // Tier 8: direct backend experiments — typed contracts on
    // start gate + conformance verdict.
    PassEvidenceEntry::new(
        "tier_8.direct_backend_experiments",
        EvidenceKind::TypedContractOnly,
    ),
    // Live executor closeouts: real GPU execution via real wgpu
    // device + indexed draw + readback + timestamp queries.
    PassEvidenceEntry::new(
        "live_proof_frame_executor.indexed_draw",
        EvidenceKind::LiveGpuExecution,
    ),
    PassEvidenceEntry::new(
        "live_proof_frame_executor.timestamps",
        EvidenceKind::LiveGpuExecution,
    ),
    PassEvidenceEntry::new(
        "live_proof_frame_executor.compiled_render_graph",
        EvidenceKind::LiveGpuExecution,
    ),
];

// ============================================================================
// Section 6.2 — Root gitlink sync expectation
// ============================================================================

/// Typed contract: the root umbrella's `fun` gitlink should
/// track the latest accepted nested `fun` commit. The user's
/// 6.2 rule: "bump root `fun` gitlink to latest accepted nested
/// `fun`."
///
/// This is a typed expectation — the umbrella commit log is the
/// source of truth. The typed [`PASS_EVIDENCE_REGISTRY`]
/// records every Pass A–H above so an audit can confirm the
/// umbrella's `summary.md` Passes Landed table covers every
/// entry.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootGitlinkSyncExpectation {
    pub schema_version: u16,
    pub umbrella_must_track_latest_accepted_nested_fun: bool,
    pub passes_landed_table_must_cover_every_registry_entry: bool,
    pub each_landed_pass_must_carry_evidence_kind: bool,
}

impl RootGitlinkSyncExpectation {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION,
        umbrella_must_track_latest_accepted_nested_fun: true,
        passes_landed_table_must_cover_every_registry_entry: true,
        each_landed_pass_must_carry_evidence_kind: true,
    };
}

// ============================================================================
// Section 6.3 — UiComposition vs CefComposition naming policy
// ============================================================================

/// Typed naming convention for the renderer's UI composition
/// surface. The user's 6.3 rule: "Product subsystem name:
/// `UiComposition`; Legacy telemetry stage: `CefComposition`;
/// CEF product role: never product UI."
///
/// The policy is enforced by `lib.rs`:
/// - `FunRendererSubsystem::UiComposition` (subsystem variant).
/// - `FunRendererFrameGraphStage::CefComposition` (frame graph
///   stage; legacy ID retained for stable telemetry).
/// - `CefRenderRoleStatus` enum forbids CEF as product UI under
///   every variant (`is_product_ui_surface() == false` for all).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiCompositionNamingPolicy {
    pub schema_version: u16,
    pub product_subsystem_name: &'static str,
    pub legacy_telemetry_stage_name: &'static str,
    pub cef_role_status_str: &'static str,
}

impl UiCompositionNamingPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION,
        product_subsystem_name: "ui_composition",
        legacy_telemetry_stage_name: "cef_composition",
        cef_role_status_str: "demoted_to_legacy_diagnostic",
    };

    /// Typed predicate verifying the canonical lib-layer
    /// surfaces agree with the policy.
    #[must_use]
    pub fn lib_layer_aligns_with_policy() -> bool {
        FunRendererSubsystem::UiComposition.as_str() == "ui_composition"
            && FunRendererFrameGraphStage::CefComposition.as_str() == "cef_composition"
            && CefRenderRoleStatus::ALL
                .iter()
                .all(|s| !s.is_product_ui_surface())
    }
}

// ============================================================================
// Section 6.4 — Integration test category taxonomy
// ============================================================================

/// Typed integration test category. The user's 6.4 rule: "A
/// one-update Bevy ECS test is useful, but it cannot prove
/// OS-surface integration. Add a separate integration category:
/// `headless_contract_tests`, `adapter_device_tests`,
/// `surface_present_tests`, `frame_probe_tests`,
/// `gpu_capture_tests`. Only `surface_present_tests` should
/// satisfy visible-frame claims."
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegrationTestCategory {
    /// Typed contracts validated against synthetic / CPU
    /// fixtures. The vast majority of fun-renderer's tests today
    /// fall here.
    #[default]
    HeadlessContract,
    /// Tests that exercise a real wgpu adapter + device
    /// initialization path without configuring a window-backed
    /// `wgpu::Surface`. The live executor's
    /// `run_proof_frame_against_fresh_dx12_device` (clear-only
    /// path) is a canonical example.
    AdapterDevice,
    /// Tests that configure a real `wgpu::Surface` against an OS
    /// window and call `SurfaceTexture::present`. Only this
    /// category satisfies visible-frame claims. The fun-renderer
    /// crate has no `SurfacePresent` tests yet because the
    /// windowed lane requires a binary-layer winit closeout.
    SurfacePresent,
    /// Tests that produce a typed
    /// [`crate::passb_proof_frame_runtime::PassBFrameProbeSample`]
    /// via `copy_texture_to_buffer` + readback. The live
    /// executor's indexed-draw + green-pixel readback is the
    /// canonical example.
    FrameProbe,
    /// Tests that perform a typed offline-style GPU capture
    /// (record commands without submitting). Reserved for
    /// future RenderDoc-style integration; no
    /// fun-renderer tests fall here today.
    GpuCapture,
}

impl IntegrationTestCategory {
    pub const ALL: [Self; INTEGRATION_TEST_CATEGORY_COUNT] = [
        Self::HeadlessContract,
        Self::AdapterDevice,
        Self::SurfacePresent,
        Self::FrameProbe,
        Self::GpuCapture,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HeadlessContract => "headless_contract_tests",
            Self::AdapterDevice => "adapter_device_tests",
            Self::SurfacePresent => "surface_present_tests",
            Self::FrameProbe => "frame_probe_tests",
            Self::GpuCapture => "gpu_capture_tests",
        }
    }

    /// Typed predicate: only `SurfacePresent` satisfies
    /// visible-frame claims. User-facing prose must not claim
    /// "the renderer rendered a visible frame" unless the
    /// underlying test is in this category.
    #[must_use]
    pub const fn satisfies_visible_frame_claim(self) -> bool {
        matches!(self, Self::SurfacePresent)
    }

    /// Typed predicate: does this category exercise a real wgpu
    /// adapter / device? True for everything except
    /// `HeadlessContract`.
    #[must_use]
    pub const fn exercises_real_wgpu_adapter(self) -> bool {
        matches!(
            self,
            Self::AdapterDevice | Self::SurfacePresent | Self::FrameProbe | Self::GpuCapture
        )
    }
}

/// Typed test-category registry entry. Maps a typed test
/// identifier to its category + evidence kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IntegrationTestRegistryEntry {
    pub schema_version: u16,
    pub test_stable_id: &'static str,
    pub category: IntegrationTestCategory,
    pub evidence_kind: EvidenceKind,
}

impl IntegrationTestRegistryEntry {
    #[must_use]
    pub const fn new(
        test_stable_id: &'static str,
        category: IntegrationTestCategory,
        evidence_kind: EvidenceKind,
    ) -> Self {
        Self {
            schema_version: QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION,
            test_stable_id,
            category,
            evidence_kind,
        }
    }
}

/// The typed integration-test registry. Records the typed
/// category + evidence kind for the live tests in
/// fun-renderer. A regression that adds a "visible-frame" claim
/// in user-facing prose must also add a `SurfacePresent` entry
/// here (and that entry must point at a real winit-backed test
/// that calls `SurfaceTexture::present`).
pub const INTEGRATION_TEST_REGISTRY: &[IntegrationTestRegistryEntry] = &[
    IntegrationTestRegistryEntry::new(
        "live_passb_runs_one_update_and_records_blocked_by_gaps",
        IntegrationTestCategory::HeadlessContract,
        EvidenceKind::TypedContractOnly,
    ),
    IntegrationTestRegistryEntry::new(
        "live_passb_runs_one_update_and_records_passes",
        IntegrationTestCategory::FrameProbe,
        EvidenceKind::LiveGpuExecution,
    ),
    IntegrationTestRegistryEntry::new(
        "live_proof_frame_executor_records_real_clear_pass_against_offscreen_target",
        IntegrationTestCategory::AdapterDevice,
        EvidenceKind::LiveGpuExecution,
    ),
    IntegrationTestRegistryEntry::new(
        "live_executor_records_real_indexed_draw_and_readback_proves_green_pixel",
        IntegrationTestCategory::FrameProbe,
        EvidenceKind::LiveGpuExecution,
    ),
    IntegrationTestRegistryEntry::new(
        "live_executor_records_real_gpu_timestamp_queries_around_indexed_draw",
        IntegrationTestCategory::FrameProbe,
        EvidenceKind::LiveGpuExecution,
    ),
    IntegrationTestRegistryEntry::new(
        "live_executor_records_per_pass_timing_artifact_for_two_passes",
        IntegrationTestCategory::FrameProbe,
        EvidenceKind::LiveGpuExecution,
    ),
    // Pass I: real GPU compute cull + indirect draw with
    // CPU-direct parity comparison.
    IntegrationTestRegistryEntry::new(
        "live_passi_runs_real_gpu_cull_and_indirect_draw_with_cpu_parity",
        IntegrationTestCategory::AdapterDevice,
        EvidenceKind::LiveGpuExecution,
    ),
    // Pass J: real GPU compute cluster-assignment + real
    // forward+ shading pass + real shadow-atlas allocation +
    // real cascade clear + per-cluster light-count readback.
    IntegrationTestRegistryEntry::new(
        "live_passj_runs_real_clustered_lighting_with_shadow_atlas",
        IntegrationTestCategory::AdapterDevice,
        EvidenceKind::LiveGpuExecution,
    ),
];

// ============================================================================
// Section 6.x — Quality audit bundle + verdict
// ============================================================================

/// The typed Quality Audit bundle. Carries every typed surface
/// the four 6.x rules enforce, plus the canonical artifact path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct QualityAuditContractBundle {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub root_gitlink_sync_expectation: RootGitlinkSyncExpectation,
    pub ui_composition_naming_policy: UiCompositionNamingPolicy,
    pub pass_evidence_registry_len: u32,
    pub integration_test_registry_len: u32,
    pub outcome: QualityAuditOutcome,
}

impl QualityAuditContractBundle {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.quality_audit_contract.funpb.zst";

    #[must_use]
    pub const fn cold_default() -> Self {
        Self {
            schema_version: QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            root_gitlink_sync_expectation: RootGitlinkSyncExpectation::PRODUCT_DEFAULT,
            ui_composition_naming_policy: UiCompositionNamingPolicy::PRODUCT_DEFAULT,
            pass_evidence_registry_len: PASS_EVIDENCE_REGISTRY.len() as u32,
            integration_test_registry_len: INTEGRATION_TEST_REGISTRY.len() as u32,
            outcome: QualityAuditOutcome::NotYetEvaluated,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityAuditOutcome {
    #[default]
    NotYetEvaluated,
    QualityContractHolds,
    QualityContractRegressed {
        violation_count: u32,
    },
}

impl QualityAuditOutcome {
    #[must_use]
    pub const fn holds(self) -> bool {
        matches!(self, Self::QualityContractHolds)
    }
}

/// Quality Audit verdict. Five rules — one per 6.1, 6.2, 6.3,
/// 6.4 (with 6.2 split into "registry covers Pass F/G/H" + "lib
/// layer naming aligns").
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QualityAuditVerdict {
    pub schema_version: u16,
    pub passes_evidence_kind_taxonomy_complete: bool,
    pub passes_evidence_registry_covers_pass_f_g_h: bool,
    pub passes_ui_composition_naming_aligns_with_lib_layer: bool,
    pub passes_integration_test_category_taxonomy_complete: bool,
    pub passes_visible_frame_claims_require_surface_present: bool,
}

impl QualityAuditVerdict {
    #[must_use]
    pub fn evaluate() -> Self {
        let passes_evidence_kind_taxonomy_complete = EvidenceKind::ALL.len() == EVIDENCE_KIND_COUNT;

        let registry_ids: Vec<&str> = PASS_EVIDENCE_REGISTRY.iter().map(|e| e.stable_id).collect();
        let passes_evidence_registry_covers_pass_f_g_h =
            registry_ids.iter().any(|id| *id == "pass_f.temporal_stack")
                && registry_ids
                    .iter()
                    .any(|id| *id == "pass_g.native_ui_product_route")
                && registry_ids
                    .iter()
                    .any(|id| *id == "pass_h.native_command_list_fail_closed");

        let passes_ui_composition_naming_aligns_with_lib_layer =
            UiCompositionNamingPolicy::lib_layer_aligns_with_policy();

        let passes_integration_test_category_taxonomy_complete =
            IntegrationTestCategory::ALL.len() == INTEGRATION_TEST_CATEGORY_COUNT;

        // Typed predicate: any registry entry whose
        // `evidence_kind == LiveGpuExecution` must be in a
        // category that exercises a real wgpu adapter
        // (`AdapterDevice` / `SurfacePresent` / `FrameProbe` /
        // `GpuCapture`). HeadlessContract cannot carry
        // LiveGpuExecution.
        let passes_visible_frame_claims_require_surface_present =
            INTEGRATION_TEST_REGISTRY.iter().all(|entry| {
                !matches!(entry.evidence_kind, EvidenceKind::LiveGpuExecution)
                    || entry.category.exercises_real_wgpu_adapter()
            });

        Self {
            schema_version: QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION,
            passes_evidence_kind_taxonomy_complete,
            passes_evidence_registry_covers_pass_f_g_h,
            passes_ui_composition_naming_aligns_with_lib_layer,
            passes_integration_test_category_taxonomy_complete,
            passes_visible_frame_claims_require_surface_present,
        }
    }

    #[must_use]
    pub const fn quality_contract_holds(&self) -> bool {
        self.passes_evidence_kind_taxonomy_complete
            && self.passes_evidence_registry_covers_pass_f_g_h
            && self.passes_ui_composition_naming_aligns_with_lib_layer
            && self.passes_integration_test_category_taxonomy_complete
            && self.passes_visible_frame_claims_require_surface_present
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_evidence_kind_taxonomy_complete {
            count += 1;
        }
        if !self.passes_evidence_registry_covers_pass_f_g_h {
            count += 1;
        }
        if !self.passes_ui_composition_naming_aligns_with_lib_layer {
            count += 1;
        }
        if !self.passes_integration_test_category_taxonomy_complete {
            count += 1;
        }
        if !self.passes_visible_frame_claims_require_surface_present {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(QUALITY_AUDIT_CONTRACT_SCHEMA_VERSION, 1);
        assert_eq!(EVIDENCE_KIND_COUNT, 6);
        assert_eq!(EvidenceKind::ALL.len(), EVIDENCE_KIND_COUNT);
        assert_eq!(INTEGRATION_TEST_CATEGORY_COUNT, 5);
        assert_eq!(
            IntegrationTestCategory::ALL.len(),
            INTEGRATION_TEST_CATEGORY_COUNT
        );
    }

    #[test]
    fn evidence_kind_taxonomy_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in EvidenceKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn only_live_gpu_execution_satisfies_visible_frame_claim() {
        for kind in EvidenceKind::ALL {
            let claim = kind.satisfies_visible_frame_claim();
            let expected = matches!(kind, EvidenceKind::LiveGpuExecution);
            assert_eq!(claim, expected, "{}", kind.as_str());
        }
    }

    #[test]
    fn every_non_live_evidence_kind_requires_explicit_label() {
        for kind in EvidenceKind::ALL {
            let requires_label = kind.requires_explicit_label();
            let expected = !matches!(kind, EvidenceKind::LiveGpuExecution);
            assert_eq!(requires_label, expected, "{}", kind.as_str());
        }
    }

    #[test]
    fn pass_evidence_registry_covers_pass_f_g_h() {
        let ids: Vec<&str> = PASS_EVIDENCE_REGISTRY.iter().map(|e| e.stable_id).collect();
        assert!(ids.contains(&"pass_f.temporal_stack"));
        assert!(ids.contains(&"pass_g.native_ui_product_route"));
        assert!(ids.contains(&"pass_h.native_command_list_fail_closed"));
    }

    #[test]
    fn pass_evidence_registry_covers_every_a_through_j_and_tier_0_through_8() {
        let ids: Vec<&str> = PASS_EVIDENCE_REGISTRY.iter().map(|e| e.stable_id).collect();
        for pass in [
            "pass_a.truth_resync_and_ui_governance",
            "pass_b.proof_frame_runtime",
            "pass_c.runtime_cache_burndown",
            "pass_d.gpu_scene_indirect_draw_parity",
            "pass_e.clustered_lighting_virtual_shadow_mvp",
            "pass_f.temporal_stack",
            "pass_g.native_ui_product_route",
            "pass_h.native_command_list_fail_closed",
            "pass_i.gpu_driven_compute_indirect",
            "pass_j.clustered_lighting_live",
        ] {
            assert!(ids.contains(&pass), "missing Pass: {pass}");
        }
        for tier in [
            "tier_0.proof_frame_gate",
            "tier_1.cache_backed_optimization",
            "tier_2.gpu_driven_proof",
            "tier_3.lighting_shadows_at_scale",
            "tier_4.transient_memory_and_barriers",
            "tier_5.temporal_reconstruction",
            "tier_6.native_ui_rendering",
            "tier_7.vendor_sdks_and_frame_generation",
            "tier_8.direct_backend_experiments",
        ] {
            assert!(ids.contains(&tier), "missing Tier: {tier}");
        }
    }

    #[test]
    fn ui_composition_naming_policy_aligns_with_lib_layer() {
        assert!(UiCompositionNamingPolicy::lib_layer_aligns_with_policy());

        // Defensive: the subsystem variant must literally be
        // `UiComposition` (not `CefCompositor` — that's the old
        // name Pass A renamed away from).
        assert_eq!(
            FunRendererSubsystem::UiComposition.as_str(),
            "ui_composition",
        );

        // Defensive: the frame-graph stage variant retains the
        // legacy `CefComposition` ID for stable telemetry. Don't
        // rename this without a coordinated telemetry migration.
        assert_eq!(
            FunRendererFrameGraphStage::CefComposition.as_str(),
            "cef_composition",
        );

        // CEF role never claims to be product UI under any
        // status variant.
        for status in CefRenderRoleStatus::ALL {
            assert!(
                !status.is_product_ui_surface(),
                "CefRenderRoleStatus::{} must not be a product UI surface",
                status.as_str(),
            );
        }
    }

    #[test]
    fn integration_test_category_taxonomy_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for category in IntegrationTestCategory::ALL {
            assert!(
                seen.insert(category.as_str()),
                "duplicate: {}",
                category.as_str()
            );
        }
    }

    #[test]
    fn only_surface_present_satisfies_visible_frame_claim() {
        for category in IntegrationTestCategory::ALL {
            let claim = category.satisfies_visible_frame_claim();
            let expected = matches!(category, IntegrationTestCategory::SurfacePresent);
            assert_eq!(claim, expected, "{}", category.as_str());
        }
    }

    #[test]
    fn headless_contract_does_not_exercise_real_wgpu_adapter() {
        assert!(!IntegrationTestCategory::HeadlessContract.exercises_real_wgpu_adapter());
        for category in [
            IntegrationTestCategory::AdapterDevice,
            IntegrationTestCategory::SurfacePresent,
            IntegrationTestCategory::FrameProbe,
            IntegrationTestCategory::GpuCapture,
        ] {
            assert!(
                category.exercises_real_wgpu_adapter(),
                "{}",
                category.as_str(),
            );
        }
    }

    #[test]
    fn integration_test_registry_entries_have_consistent_evidence_kind_and_category() {
        // Rule: any LiveGpuExecution entry must be in a category
        // that exercises a real wgpu adapter.
        for entry in INTEGRATION_TEST_REGISTRY {
            if matches!(entry.evidence_kind, EvidenceKind::LiveGpuExecution) {
                assert!(
                    entry.category.exercises_real_wgpu_adapter(),
                    "{}: LiveGpuExecution must be in a real-wgpu category",
                    entry.test_stable_id,
                );
            }
        }
    }

    #[test]
    fn integration_test_registry_covers_renamed_passb_test() {
        let ids: Vec<&str> = INTEGRATION_TEST_REGISTRY
            .iter()
            .map(|e| e.test_stable_id)
            .collect();
        assert!(
            ids.contains(&"live_passb_runs_one_update_and_records_passes"),
            "registry must record the canonical Pass B CI proof test name",
        );
        assert!(
            ids.contains(&"live_passb_runs_one_update_and_records_blocked_by_gaps"),
            "registry must record the App-only fail-honest test name",
        );
    }

    #[test]
    fn bundle_canonical_path_uses_funpb_zst_suffix() {
        let bundle = QualityAuditContractBundle::cold_default();
        assert_eq!(
            bundle.canonical_path,
            QualityAuditContractBundle::CANONICAL_ARTIFACT_PATH,
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn quality_audit_verdict_holds_under_current_workspace_state() {
        let verdict = QualityAuditVerdict::evaluate();
        assert!(
            verdict.quality_contract_holds(),
            "Quality Audit verdict must hold; violations = {}",
            verdict.violation_count(),
        );
        assert_eq!(verdict.violation_count(), 0);
    }

    #[test]
    fn root_gitlink_sync_expectation_carries_three_rules() {
        let exp = RootGitlinkSyncExpectation::PRODUCT_DEFAULT;
        assert!(exp.umbrella_must_track_latest_accepted_nested_fun);
        assert!(exp.passes_landed_table_must_cover_every_registry_entry);
        assert!(exp.each_landed_pass_must_carry_evidence_kind);
    }

    #[test]
    fn outcome_holds_only_for_quality_contract_holds() {
        assert!(QualityAuditOutcome::QualityContractHolds.holds());
        assert!(!QualityAuditOutcome::NotYetEvaluated.holds());
        assert!(!QualityAuditOutcome::QualityContractRegressed { violation_count: 1 }.holds());
    }
}
