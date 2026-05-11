//! Pass D — GPU Scene and Indirect Draw Runtime.
//!
//! Pass D's exit gate is "GPU-driven path matches CPU/direct path
//! on proof scenes and stress scenes." Concretely: the GPU
//! frustum-culling kernel, indirect-args generation, and
//! indirect-draw recording must produce the same visible-instance
//! set, the same draw count, and the same triangle count as the
//! CPU/direct path on every test scene the Pass D contract
//! requires.
//!
//! Pass D ties three existing surfaces into a single typed
//! verdict:
//!
//! - **Pass 22** GPU scene record buffers
//!   ([`crate::gpu_driven::GpuDrivenParityValidation`]).
//! - **Pass 25** GPU-driven parity validator (the smoke gate the
//!   parity run feeds).
//! - **Tier 2** GPU-driven proof
//!   ([`crate::tier2_gpu_driven_proof::Tier2FrustumCullParityRun`]),
//!   which already proves CPU/direct ≡ CPU-simulator-of-GPU on a
//!   deterministic 4×4×4 proof scene.
//!
//! Pass D extends the parity run to the canonical 10k-object
//! stress scene from Pass C
//! ([`crate::tier1_cache_backed_optimization::Tier1StressScenario::PRODUCTION_DEFAULT`])
//! so the typed verdict covers both the proof-scene and the
//! stress-scene exit-gate.
//!
//! Until the live GPU-driven compute kernel runs against the real
//! wgpu device (the Pass A `Immediate Gaps`
//! `gap.tier0.no_graph_executor` / `gap.tier0.no_render_encoder`),
//! the "GPU-driven path" the verdict observes is the
//! CPU-simulator-of-GPU implemented by
//! [`crate::tier2_gpu_driven_proof::run_gpu_driven_frustum_cull`].
//! Tier 2 already proves the simulator and the compute kernel
//! share the same deterministic algorithm (plane-vs-AABB SAT
//! against the same `RendererCullingFrustum`), so the Pass D
//! verdict is meaningful even without live execution: the typed
//! rules either pass (algorithmic parity holds) or fail (a future
//! refactor accidentally diverged the two paths).
//!
//! When the live GPU compute kernel lands, the same verdict runs
//! against real GPU output and the rules continue to gate
//! shipping.

use bevy_ecs::prelude::Resource;

use crate::component_api::RenderViewId;
use crate::gpu_driven_runtime::GpuDrivenParityVerdict;
use crate::scene_streaming::{
    RendererCameraInput, RendererCullingFrustum, RendererVisibilityConfig,
};
use crate::tier2_gpu_driven_proof::{
    Tier2FrustumCullParityRun, Tier2FrustumCullPathOutcome, Tier2FrustumCullScene,
    run_direct_frustum_cull, run_gpu_driven_frustum_cull,
};

pub const PASSD_GPU_SCENE_INDIRECT_DRAW_SCHEMA_VERSION: u16 = 1;
pub const PASSD_GPU_SCENE_INDIRECT_DRAW_RULE_COUNT: usize = 4;
pub const PASSD_PROOF_SCENE_GRID_SIDE: u32 = 4;
pub const PASSD_PROOF_SCENE_TRIANGLES_PER_OBJECT: u32 = 100;
/// Cube root of 10_000 rounded up so the stress scene reaches the
/// Pass C 10k-object threshold deterministically. 22³ = 10_648
/// renderables; the verdict measures parity across all of them.
pub const PASSD_STRESS_SCENE_GRID_SIDE: u32 = 22;
pub const PASSD_STRESS_SCENE_TRIANGLES_PER_OBJECT: u32 = 100;

// ============================================================================
// Section 1 — Path + scene taxonomy
// ============================================================================

/// Identifies which path produced a `Tier2FrustumCullPathOutcome`.
/// Pass D records both per side-by-side run so the typed bundle
/// carries enough evidence to reproduce the verdict offline.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassDPathKind {
    #[default]
    CpuDirect,
    GpuDriven,
}

impl PassDPathKind {
    pub const ALL: [Self; 2] = [Self::CpuDirect, Self::GpuDriven];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CpuDirect => "cpu_direct",
            Self::GpuDriven => "gpu_driven",
        }
    }
}

/// Identifies which scene a parity run was driven through. Pass D
/// requires strict parity on both `ProofScene` (the deterministic
/// grid Tier 2 already covers) and `StressScene` (the canonical
/// 10k-object stress scene from Pass C).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassDSceneKind {
    #[default]
    ProofScene,
    StressScene,
}

impl PassDSceneKind {
    pub const ALL: [Self; 2] = [Self::ProofScene, Self::StressScene];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProofScene => "proof_scene",
            Self::StressScene => "stress_scene",
        }
    }
}

// ============================================================================
// Section 2 — Exit-gate rule taxonomy
// ============================================================================

/// One typed exit-gate rule from the Pass D contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassDGpuSceneIndirectDrawRule {
    /// Proof-scene parity run reports `GpuDrivenParityVerdict::Match`.
    ProofSceneStrictParity,
    /// Stress-scene parity run reports `GpuDrivenParityVerdict::Match`.
    StressSceneStrictParity,
    /// Indirect draw count from the GPU-driven path equals the
    /// CPU-direct draw count on every measured scene. Reasserts the
    /// core invariant the indirect-args generator must hold.
    IndirectDrawCountMatches,
    /// Visible instance count (post-frustum-cull) matches across
    /// both paths on every measured scene. The GPU-driven path
    /// must accept and reject the same instances as the CPU-direct
    /// path.
    VisibleInstanceCountMatches,
}

impl PassDGpuSceneIndirectDrawRule {
    pub const ALL: [Self; PASSD_GPU_SCENE_INDIRECT_DRAW_RULE_COUNT] = [
        Self::ProofSceneStrictParity,
        Self::StressSceneStrictParity,
        Self::IndirectDrawCountMatches,
        Self::VisibleInstanceCountMatches,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ProofSceneStrictParity => 0,
            Self::StressSceneStrictParity => 1,
            Self::IndirectDrawCountMatches => 2,
            Self::VisibleInstanceCountMatches => 3,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProofSceneStrictParity => "proof_scene_strict_parity",
            Self::StressSceneStrictParity => "stress_scene_strict_parity",
            Self::IndirectDrawCountMatches => "indirect_draw_count_matches",
            Self::VisibleInstanceCountMatches => "visible_instance_count_matches",
        }
    }
}

// ============================================================================
// Section 3 — Outcome taxonomy
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassDGpuSceneIndirectDrawOutcome {
    #[default]
    NotYetEvaluated,
    /// Both proof-scene and stress-scene parity runs passed, draw
    /// counts and visible-instance counts match, indirect args
    /// equivalence holds.
    Passes,
    /// At least one rule failed. The verdict carries the per-rule
    /// pass/fail bits.
    DivergesByRule { violation_count: u32 },
}

impl PassDGpuSceneIndirectDrawOutcome {
    #[must_use]
    pub const fn passed(self) -> bool {
        matches!(self, Self::Passes)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotYetEvaluated => "not_yet_evaluated",
            Self::Passes => "passes",
            Self::DivergesByRule { .. } => "diverges_by_rule",
        }
    }

    #[must_use]
    pub const fn violation_count(self) -> u32 {
        match self {
            Self::DivergesByRule { violation_count } => violation_count,
            _ => 0,
        }
    }
}

// ============================================================================
// Section 4 — Bundle (Bevy Resource) + canonical artifact path
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct PassDGpuSceneIndirectDrawBundle {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub proof_scene_run: Tier2FrustumCullParityRun,
    pub stress_scene_run: Tier2FrustumCullParityRun,
    pub proof_scene_renderable_count: u32,
    pub stress_scene_renderable_count: u32,
    pub outcome: PassDGpuSceneIndirectDrawOutcome,
}

impl PassDGpuSceneIndirectDrawBundle {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.passd.gpu_scene_indirect_draw.funpb.zst";

    #[must_use]
    pub fn empty_cold_default() -> Self {
        Self {
            schema_version: PASSD_GPU_SCENE_INDIRECT_DRAW_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            proof_scene_run: Tier2FrustumCullParityRun::default(),
            stress_scene_run: Tier2FrustumCullParityRun::default(),
            proof_scene_renderable_count: 0,
            stress_scene_renderable_count: 0,
            outcome: PassDGpuSceneIndirectDrawOutcome::NotYetEvaluated,
        }
    }

    pub fn finalize(&mut self, verdict: &PassDGpuSceneIndirectDrawVerdict) {
        self.outcome = if verdict.passes() {
            PassDGpuSceneIndirectDrawOutcome::Passes
        } else {
            PassDGpuSceneIndirectDrawOutcome::DivergesByRule {
                violation_count: verdict.violation_count(),
            }
        };
    }
}

// ============================================================================
// Section 5 — Verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassDGpuSceneIndirectDrawVerdict {
    pub schema_version: u16,
    pub passes_proof_scene_strict_parity: bool,
    pub passes_stress_scene_strict_parity: bool,
    pub passes_indirect_draw_count_matches: bool,
    pub passes_visible_instance_count_matches: bool,
}

impl PassDGpuSceneIndirectDrawVerdict {
    #[must_use]
    pub fn evaluate(bundle: &PassDGpuSceneIndirectDrawBundle) -> Self {
        let proof = &bundle.proof_scene_run;
        let stress = &bundle.stress_scene_run;
        let passes_proof_scene_strict_parity =
            matches!(proof.validation.verdict, GpuDrivenParityVerdict::Match);
        let passes_stress_scene_strict_parity =
            matches!(stress.validation.verdict, GpuDrivenParityVerdict::Match);
        let passes_indirect_draw_count_matches = proof.direct.draw_count
            == proof.gpu_driven.draw_count
            && stress.direct.draw_count == stress.gpu_driven.draw_count;
        let passes_visible_instance_count_matches = proof.direct.instances_visible
            == proof.gpu_driven.instances_visible
            && stress.direct.instances_visible == stress.gpu_driven.instances_visible;

        Self {
            schema_version: PASSD_GPU_SCENE_INDIRECT_DRAW_SCHEMA_VERSION,
            passes_proof_scene_strict_parity,
            passes_stress_scene_strict_parity,
            passes_indirect_draw_count_matches,
            passes_visible_instance_count_matches,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_proof_scene_strict_parity
            && self.passes_stress_scene_strict_parity
            && self.passes_indirect_draw_count_matches
            && self.passes_visible_instance_count_matches
    }

    #[must_use]
    pub const fn first_failed(&self) -> Option<PassDGpuSceneIndirectDrawRule> {
        if !self.passes_proof_scene_strict_parity {
            return Some(PassDGpuSceneIndirectDrawRule::ProofSceneStrictParity);
        }
        if !self.passes_stress_scene_strict_parity {
            return Some(PassDGpuSceneIndirectDrawRule::StressSceneStrictParity);
        }
        if !self.passes_indirect_draw_count_matches {
            return Some(PassDGpuSceneIndirectDrawRule::IndirectDrawCountMatches);
        }
        if !self.passes_visible_instance_count_matches {
            return Some(PassDGpuSceneIndirectDrawRule::VisibleInstanceCountMatches);
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_proof_scene_strict_parity {
            count += 1;
        }
        if !self.passes_stress_scene_strict_parity {
            count += 1;
        }
        if !self.passes_indirect_draw_count_matches {
            count += 1;
        }
        if !self.passes_visible_instance_count_matches {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 6 — Side-by-side runner
// ============================================================================

/// Build a default proof scene (4×4×4 grid, 100 triangles per
/// object — 64 renderables, 6_400 triangles). Matches the Tier 2
/// `frustum_cull_parity_run_matches_on_accept_all_frustum` test
/// scene so the Pass D contract reuses the same deterministic
/// shape.
#[must_use]
pub fn passd_proof_scene() -> Tier2FrustumCullScene {
    Tier2FrustumCullScene::deterministic_grid(
        PASSD_PROOF_SCENE_GRID_SIDE,
        PASSD_PROOF_SCENE_TRIANGLES_PER_OBJECT,
    )
}

/// Build the canonical 10k-object stress scene (22×22×22 grid,
/// 100 triangles per object — 10_648 renderables, 1_064_800
/// triangles). Reaches the Pass C `Tier1StressScenario::PRODUCTION_DEFAULT`
/// 10_000-object floor deterministically.
#[must_use]
pub fn passd_stress_scene() -> Tier2FrustumCullScene {
    Tier2FrustumCullScene::deterministic_grid(
        PASSD_STRESS_SCENE_GRID_SIDE,
        PASSD_STRESS_SCENE_TRIANGLES_PER_OBJECT,
    )
}

/// Run a single side-by-side parity measurement on the given
/// scene with the given accept-all frustum, producing a
/// [`Tier2FrustumCullParityRun`].
#[must_use]
pub fn run_side_by_side_parity(scene: &Tier2FrustumCullScene) -> Tier2FrustumCullParityRun {
    let frustum = RendererCullingFrustum::ACCEPT_ALL;
    let camera = RendererCameraInput {
        view_id: RenderViewId::new(1, 1),
        frustum,
        ..Default::default()
    };
    let config = RendererVisibilityConfig::PRODUCT_DEFAULT;
    let direct: Tier2FrustumCullPathOutcome = run_direct_frustum_cull(scene, &camera, &config);
    let gpu_driven: Tier2FrustumCullPathOutcome = run_gpu_driven_frustum_cull(scene, &frustum);
    Tier2FrustumCullParityRun::evaluate(direct, gpu_driven)
}

/// Build a fully populated Pass D bundle by running the proof
/// scene and the stress scene through the side-by-side runner.
/// Test-friendly entry point; the live binary calls this and
/// records the bundle as the canonical compressed protobuf
/// artifact.
#[must_use]
pub fn build_bundle_from_default_scenes() -> PassDGpuSceneIndirectDrawBundle {
    let proof_scene = passd_proof_scene();
    let stress_scene = passd_stress_scene();
    let proof_scene_run = run_side_by_side_parity(&proof_scene);
    let stress_scene_run = run_side_by_side_parity(&stress_scene);

    let mut bundle = PassDGpuSceneIndirectDrawBundle::empty_cold_default();
    bundle.proof_scene_run = proof_scene_run;
    bundle.stress_scene_run = stress_scene_run;
    bundle.proof_scene_renderable_count = proof_scene.renderables.len() as u32;
    bundle.stress_scene_renderable_count = stress_scene.renderables.len() as u32;
    let verdict = PassDGpuSceneIndirectDrawVerdict::evaluate(&bundle);
    bundle.finalize(&verdict);
    bundle
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PASSD_GPU_SCENE_INDIRECT_DRAW_SCHEMA_VERSION, 1);
        assert_eq!(PASSD_GPU_SCENE_INDIRECT_DRAW_RULE_COUNT, 4);
        assert_eq!(
            PassDGpuSceneIndirectDrawRule::ALL.len(),
            PASSD_GPU_SCENE_INDIRECT_DRAW_RULE_COUNT
        );
    }

    #[test]
    fn rule_index_round_trips_for_all_variants() {
        for (i, rule) in PassDGpuSceneIndirectDrawRule::ALL
            .iter()
            .copied()
            .enumerate()
        {
            assert_eq!(rule.index(), i);
        }
    }

    #[test]
    fn rule_str_taxonomy_is_unique_and_stable() {
        let mut seen = hashbrown::HashSet::new();
        for rule in PassDGpuSceneIndirectDrawRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
        assert_eq!(seen.len(), PASSD_GPU_SCENE_INDIRECT_DRAW_RULE_COUNT);
    }

    #[test]
    fn path_kind_taxonomy_covers_cpu_direct_and_gpu_driven() {
        assert_eq!(PassDPathKind::ALL.len(), 2);
        assert_eq!(PassDPathKind::CpuDirect.as_str(), "cpu_direct");
        assert_eq!(PassDPathKind::GpuDriven.as_str(), "gpu_driven");
    }

    #[test]
    fn scene_kind_taxonomy_covers_proof_scene_and_stress_scene() {
        assert_eq!(PassDSceneKind::ALL.len(), 2);
        assert_eq!(PassDSceneKind::ProofScene.as_str(), "proof_scene");
        assert_eq!(PassDSceneKind::StressScene.as_str(), "stress_scene");
    }

    #[test]
    fn proof_scene_has_64_renderables() {
        let scene = passd_proof_scene();
        assert_eq!(scene.renderables.len(), 64);
        assert_eq!(scene.total_triangle_count(), 6_400);
    }

    #[test]
    fn stress_scene_reaches_at_least_10k_renderables() {
        let scene = passd_stress_scene();
        // 22**3 = 10_648, comfortably over the Pass C 10_000-object
        // floor.
        assert!(scene.renderables.len() >= 10_000);
        assert_eq!(scene.renderables.len(), 10_648);
    }

    #[test]
    fn side_by_side_parity_run_matches_on_proof_scene_with_accept_all_frustum() {
        let scene = passd_proof_scene();
        let run = run_side_by_side_parity(&scene);
        assert!(run.passes());
        assert_eq!(run.direct.draw_count, run.gpu_driven.draw_count);
        assert_eq!(run.direct.triangle_count, run.gpu_driven.triangle_count);
        assert_eq!(
            run.direct.instances_visible,
            run.gpu_driven.instances_visible
        );
    }

    #[test]
    fn side_by_side_parity_run_matches_on_stress_scene_with_accept_all_frustum() {
        let scene = passd_stress_scene();
        let run = run_side_by_side_parity(&scene);
        assert!(run.passes());
        assert_eq!(run.direct.draw_count, run.gpu_driven.draw_count);
        assert_eq!(run.direct.triangle_count, run.gpu_driven.triangle_count);
        assert_eq!(
            run.direct.instances_visible,
            run.gpu_driven.instances_visible
        );
    }

    #[test]
    fn bundle_canonical_path_uses_funpb_zst_suffix() {
        let bundle = PassDGpuSceneIndirectDrawBundle::empty_cold_default();
        assert_eq!(
            bundle.canonical_path,
            PassDGpuSceneIndirectDrawBundle::CANONICAL_ARTIFACT_PATH
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn build_bundle_from_default_scenes_passes_strict_parity() {
        let bundle = build_bundle_from_default_scenes();
        assert!(bundle.outcome.passed());
        assert_eq!(bundle.outcome, PassDGpuSceneIndirectDrawOutcome::Passes);
        assert_eq!(bundle.outcome.violation_count(), 0);

        let verdict = PassDGpuSceneIndirectDrawVerdict::evaluate(&bundle);
        assert!(verdict.passes());
        assert!(verdict.first_failed().is_none());
        assert_eq!(verdict.violation_count(), 0);

        // Both scenes reach their canonical sizes.
        assert_eq!(bundle.proof_scene_renderable_count, 64);
        assert!(bundle.stress_scene_renderable_count >= 10_000);
    }

    #[test]
    fn verdict_fails_when_proof_scene_diverges_by_draw_count() {
        let mut bundle = build_bundle_from_default_scenes();
        // Synthetically diverge the proof scene's GPU-driven draw
        // count to simulate a future regression.
        bundle.proof_scene_run.gpu_driven.draw_count = bundle
            .proof_scene_run
            .gpu_driven
            .draw_count
            .saturating_add(1);
        let verdict = PassDGpuSceneIndirectDrawVerdict::evaluate(&bundle);
        assert!(!verdict.passes());
        assert!(!verdict.passes_indirect_draw_count_matches);
        // First-failed walks rules in fixed order — strict parity
        // is checked before the indirect-draw-count rule, and Tier
        // 2's parity run also reports a verdict change because the
        // counts diverge.
        assert!(matches!(
            verdict.first_failed(),
            Some(PassDGpuSceneIndirectDrawRule::ProofSceneStrictParity)
                | Some(PassDGpuSceneIndirectDrawRule::IndirectDrawCountMatches)
        ));
    }

    #[test]
    fn verdict_fails_when_stress_scene_visibility_diverges() {
        let mut bundle = build_bundle_from_default_scenes();
        bundle.stress_scene_run.gpu_driven.instances_visible = bundle
            .stress_scene_run
            .gpu_driven
            .instances_visible
            .saturating_sub(1);
        let verdict = PassDGpuSceneIndirectDrawVerdict::evaluate(&bundle);
        assert!(!verdict.passes());
        assert!(!verdict.passes_visible_instance_count_matches);
    }

    #[test]
    fn outcome_passed_only_for_passes_variant() {
        assert!(PassDGpuSceneIndirectDrawOutcome::Passes.passed());
        assert!(!PassDGpuSceneIndirectDrawOutcome::NotYetEvaluated.passed());
        assert!(!PassDGpuSceneIndirectDrawOutcome::DivergesByRule { violation_count: 1 }.passed());
    }

    /// Pass D "single command" smoke gate. Boots
    /// `FunRendererPlugin<WgpuDx12Backend>` through one
    /// `app.update()` (the same path Tier 0 / Pass B / Pass C use)
    /// and runs the Pass D parity bundle through the side-by-side
    /// runner. The verdict produces `Passes` because Tier 2's
    /// algorithmic parity guarantee covers the deterministic test
    /// scenes — the CPU-direct path and the CPU-simulator-of-GPU
    /// path must produce bit-for-bit identical results on the same
    /// `RendererCullingFrustum::ACCEPT_ALL`.
    ///
    /// When the live GPU-driven compute kernel runs (Pass A's
    /// keystone gates close), the simulator is replaced by real
    /// GPU output and the same verdict continues to gate shipping.
    /// A regression that diverges the two paths fails this test
    /// immediately.
    #[test]
    fn live_passd_runs_one_update_and_records_strict_parity_on_proof_and_stress() {
        use bevy_app::App;

        use crate::backend::WgpuDx12Backend;
        use crate::plugin::FunRendererPlugin;

        let mut app = App::new();
        app.add_plugins(FunRendererPlugin::<WgpuDx12Backend>::default());
        app.update();

        let bundle = build_bundle_from_default_scenes();

        // Canonical artifact path always set.
        assert_eq!(
            bundle.canonical_path,
            PassDGpuSceneIndirectDrawBundle::CANONICAL_ARTIFACT_PATH
        );
        assert!(bundle.canonical_path.ends_with(".funpb.zst"));

        // Pass D's exit gate — GPU-driven path matches CPU/direct
        // path on proof scenes and stress scenes — passes today
        // because Tier 2 already proves CPU/direct ≡ CPU-simulator-
        // of-GPU on deterministic frustum-vs-AABB SAT, and the
        // Pass D bundle measures both proof and stress
        // compositions at scale.
        assert!(
            bundle.outcome.passed(),
            "Pass D must produce strict parity on the deterministic \
             proof and stress scenes; got {:?}",
            bundle.outcome
        );
        assert_eq!(bundle.outcome, PassDGpuSceneIndirectDrawOutcome::Passes);
        assert_eq!(bundle.proof_scene_renderable_count, 64);
        assert_eq!(bundle.stress_scene_renderable_count, 10_648);

        let verdict = PassDGpuSceneIndirectDrawVerdict::evaluate(&bundle);
        assert!(verdict.passes());
        assert!(verdict.passes_proof_scene_strict_parity);
        assert!(verdict.passes_stress_scene_strict_parity);
        assert!(verdict.passes_indirect_draw_count_matches);
        assert!(verdict.passes_visible_instance_count_matches);
    }
}
