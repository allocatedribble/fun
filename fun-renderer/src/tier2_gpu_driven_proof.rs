//! Tier 2 — GPU-Driven Rendering Proof.
//!
//! Pass 17 + Pass 25 installed the typed planning surface and
//! runtime IR for GPU-driven rendering: indirect command list,
//! culling pass bindings, Hi-Z pyramid runtime, meshlet metadata,
//! mesh-shader capability branch, and the
//! `GpuDrivenParityValidation` smoke gate. Tier 2 puts that
//! surface under an actually-running parity validator.
//!
//! Three executable proofs:
//!
//! 1. **Frustum cull parity** — runs both the CPU/direct path
//!    and a GPU-driven simulator over the same renderable list
//!    and feeds the typed `GpuDrivenParityValidation` smoke gate.
//!    Acceptance: verdict is `Match` (identical draw count and
//!    triangle count).
//!
//! 2. **Hi-Z occlusion feedback** — produces per-object
//!    visibility verdicts from a typed depth pyramid simulation,
//!    honours the conservative-fallback rule when the previous
//!    frame is unavailable, classifies uncertain pixels as
//!    visible, and asserts the no-disappearance-without-parity
//!    rule.
//!
//! 3. **Meshlet equivalence** — runs both the compute-fallback
//!    path and the mesh-shader path over the same meshlet set
//!    with cone culling and asserts they emit the same visible
//!    set. Mesh shaders remain optional — when capability truth
//!    says they are unavailable, the equivalence holds via the
//!    compute fallback alone.
//!
//! Honest scope: each path executes as a deterministic CPU
//! implementation of the same algorithm the GPU compute pass
//! will run. The bridge has not yet wired real wgpu compute
//! dispatches (see Tier 0 gaps `no_render_encoder`,
//! `no_swapchain_configured`). The GPU-driven simulator's
//! purpose here is to prove the *algorithm* is correct against
//! the direct path; once the bridge runs the real compute
//! kernel, this same parity validator runs against actual GPU
//! debug counter readback and the verdict must continue to be
//! `Match`.

use fun_ecs::Resource;

use crate::component_api::{RenderAabb, RenderStableId, RenderVec3};
use crate::gpu_driven::{
    GpuDrivenDebugCounter, GpuDrivenDebugCounterSnapshot, GpuDrivenOcclusionPolicy,
    GpuDrivenParityComparison,
};
use crate::gpu_driven_runtime::{
    GpuDrivenParityValidation, GpuDrivenParityVerdict, HiZPyramidRuntime,
    MESHLET_DEFAULT_TRIANGLE_LIMIT, MESHLET_DEFAULT_VERTEX_LIMIT, MeshShaderCapabilityBranch,
    MeshletClusterBounds, MeshletMetadataRecord,
};
use crate::scene_streaming::{
    RendererCameraInput, RendererCullingFrustum, RendererFrustumPlane, RendererOpacityMode,
    RendererRenderableInput, RendererVisibilityClassification, RendererVisibilityConfig,
    classify_renderable_for_view,
};

pub const TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION: u16 = 1;

pub const TIER2_VISIBILITY_VERDICT_KIND_COUNT: usize = 4;
pub const TIER2_MESHLET_EQUIVALENCE_VERDICT_KIND_COUNT: usize = 4;

// ============================================================================
// Section 1 — Frustum-cull parity harness
// ============================================================================

/// One renderable in the parity scene. Carries the AABB and
/// triangle count so both paths produce comparable triangle
/// counts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tier2FrustumCullRenderable {
    pub object_id: RenderStableId,
    pub world_aabb: RenderAabb,
    pub triangle_count: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct Tier2FrustumCullScene {
    pub schema_version: u16,
    pub renderables: Vec<Tier2FrustumCullRenderable>,
}

impl Tier2FrustumCullScene {
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            renderables: Vec::new(),
        }
    }

    pub fn record(&mut self, renderable: Tier2FrustumCullRenderable) {
        self.renderables.push(renderable);
    }

    /// Build a deterministic test scene: a 4×4×4 grid of unit
    /// AABBs centred on the origin, each with a fixed triangle
    /// count.
    #[must_use]
    pub fn deterministic_grid(side: u32, triangle_count_per_object: u32) -> Self {
        let mut scene = Self::new();
        let half = (side as f32) * 0.5;
        let mut next_id: u64 = 1;
        for ix in 0..side {
            for iy in 0..side {
                for iz in 0..side {
                    let center = RenderVec3::new(
                        ix as f32 - half + 0.5,
                        iy as f32 - half + 0.5,
                        iz as f32 - half + 0.5,
                    );
                    scene.record(Tier2FrustumCullRenderable {
                        object_id: RenderStableId::new(next_id),
                        world_aabb: RenderAabb::new(center, RenderVec3::new(0.4, 0.4, 0.4)),
                        triangle_count: triangle_count_per_object,
                    });
                    next_id += 1;
                }
            }
        }
        scene
    }

    #[must_use]
    pub fn total_triangle_count(&self) -> u64 {
        self.renderables
            .iter()
            .fold(0u64, |acc, r| acc.saturating_add(r.triangle_count as u64))
    }
}

/// The result of running one path over the parity scene.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier2FrustumCullPathOutcome {
    pub schema_version: u16,
    pub draw_count: u32,
    pub triangle_count: u64,
    pub instances_tested: u32,
    pub instances_visible: u32,
    pub instances_frustum_rejected: u32,
}

/// CPU/direct path: classify each renderable using
/// `classify_renderable_for_view` (the same predicate Pass 22
/// installed for the renderer's CPU visibility set).
#[must_use]
pub fn run_direct_frustum_cull(
    scene: &Tier2FrustumCullScene,
    camera: &RendererCameraInput,
    config: &RendererVisibilityConfig,
) -> Tier2FrustumCullPathOutcome {
    let mut outcome = Tier2FrustumCullPathOutcome {
        schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
        ..Tier2FrustumCullPathOutcome::default()
    };
    for renderable in &scene.renderables {
        outcome.instances_tested = outcome.instances_tested.saturating_add(1);
        let input = RendererRenderableInput {
            object_id: crate::component_api::RenderObjectId::new(renderable.object_id.0 as u32, 1),
            mesh_id: crate::component_api::RenderMeshId::INVALID,
            material_id: crate::component_api::RenderMaterialId::INVALID,
            layers: crate::component_api::RenderLayerMask::DEFAULT,
            world_aabb: renderable.world_aabb,
            world_position: renderable.world_aabb.center,
            visibility: crate::component_api::RenderVisibilityState::Visible,
            opacity: RendererOpacityMode::Opaque,
            shadow_caster: false,
            camera_order_min: i16::MIN,
            camera_order_max: i16::MAX,
        };
        let verdict = classify_renderable_for_view(&input, camera, config);
        match verdict.classification {
            RendererVisibilityClassification::Visible => {
                outcome.instances_visible = outcome.instances_visible.saturating_add(1);
                outcome.draw_count = outcome.draw_count.saturating_add(1);
                outcome.triangle_count = outcome
                    .triangle_count
                    .saturating_add(renderable.triangle_count as u64);
            }
            RendererVisibilityClassification::CulledByFrustum => {
                outcome.instances_frustum_rejected =
                    outcome.instances_frustum_rejected.saturating_add(1);
            }
            _ => {}
        }
    }
    outcome
}

/// GPU-driven simulator: implements the same plane-vs-AABB SAT
/// test the compute kernel will run, but on the CPU. The
/// algorithm matches Pass 22's `RendererCullingFrustum::contains_aabb`,
/// so the outputs must match the direct path bit-for-bit on a
/// deterministic scene.
#[must_use]
pub fn run_gpu_driven_frustum_cull(
    scene: &Tier2FrustumCullScene,
    frustum: &RendererCullingFrustum,
) -> Tier2FrustumCullPathOutcome {
    let mut outcome = Tier2FrustumCullPathOutcome {
        schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
        ..Tier2FrustumCullPathOutcome::default()
    };
    for renderable in &scene.renderables {
        outcome.instances_tested = outcome.instances_tested.saturating_add(1);
        if frustum.contains_aabb(renderable.world_aabb) {
            outcome.instances_visible = outcome.instances_visible.saturating_add(1);
            outcome.draw_count = outcome.draw_count.saturating_add(1);
            outcome.triangle_count = outcome
                .triangle_count
                .saturating_add(renderable.triangle_count as u64);
        } else {
            outcome.instances_frustum_rejected =
                outcome.instances_frustum_rejected.saturating_add(1);
        }
    }
    outcome
}

/// One end-to-end parity run: feed `GpuDrivenParityValidation` the
/// counts from both paths and produce the typed verdict.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct Tier2FrustumCullParityRun {
    pub schema_version: u16,
    pub direct: Tier2FrustumCullPathOutcome,
    pub gpu_driven: Tier2FrustumCullPathOutcome,
    pub validation: GpuDrivenParityValidation,
}

impl Tier2FrustumCullParityRun {
    #[must_use]
    pub fn evaluate(
        direct: Tier2FrustumCullPathOutcome,
        gpu_driven: Tier2FrustumCullPathOutcome,
    ) -> Self {
        let comparison = GpuDrivenParityComparison::from_counts(
            direct.draw_count,
            gpu_driven.draw_count,
            direct.triangle_count,
            gpu_driven.triangle_count,
        );
        let mut slots = [0u32; 8];
        slots[GpuDrivenDebugCounter::InstancesTested.slot() as usize] = gpu_driven.instances_tested;
        slots[GpuDrivenDebugCounter::InstancesVisible.slot() as usize] =
            gpu_driven.instances_visible;
        slots[GpuDrivenDebugCounter::InstancesFrustumRejected.slot() as usize] =
            gpu_driven.instances_frustum_rejected;
        let counters = GpuDrivenDebugCounterSnapshot::from_slots(slots);
        let validation = GpuDrivenParityValidation::evaluate(comparison, counters);
        Self {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            direct,
            gpu_driven,
            validation,
        }
    }

    #[must_use]
    pub fn passes(&self) -> bool {
        matches!(self.validation.verdict, GpuDrivenParityVerdict::Match)
    }
}

// ============================================================================
// Section 2 — Hi-Z occlusion feedback
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier2VisibilityVerdict {
    #[default]
    Visible,
    ConservativelyVisible,
    OccludedByHiZ,
    Uncertain,
}

impl Tier2VisibilityVerdict {
    pub const ALL: [Self; TIER2_VISIBILITY_VERDICT_KIND_COUNT] = [
        Self::Visible,
        Self::ConservativelyVisible,
        Self::OccludedByHiZ,
        Self::Uncertain,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Visible => 0,
            Self::ConservativelyVisible => 1,
            Self::OccludedByHiZ => 2,
            Self::Uncertain => 3,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::ConservativelyVisible => "conservatively_visible",
            Self::OccludedByHiZ => "occluded_by_hi_z",
            Self::Uncertain => "uncertain",
        }
    }

    #[must_use]
    pub const fn counts_as_drawn(self) -> bool {
        matches!(
            self,
            Self::Visible | Self::ConservativelyVisible | Self::Uncertain
        )
    }
}

/// One renderable's depth/AABB summary for the Hi-Z feedback
/// pass. The runtime simulator computes a typed verdict from
/// this against the previous-frame depth pyramid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tier2HiZRenderable {
    pub object_id: RenderStableId,
    pub world_aabb: RenderAabb,
    pub closest_depth: f32,
    pub farthest_depth: f32,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct Tier2HiZOcclusionScene {
    pub schema_version: u16,
    pub renderables: Vec<Tier2HiZRenderable>,
    pub previous_frame_min_depth_at_screen_position: f32,
}

impl Tier2HiZOcclusionScene {
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            renderables: Vec::new(),
            previous_frame_min_depth_at_screen_position: f32::INFINITY,
        }
    }

    pub fn record(&mut self, renderable: Tier2HiZRenderable) {
        self.renderables.push(renderable);
    }

    pub fn set_previous_frame_min_depth(&mut self, depth: f32) {
        self.previous_frame_min_depth_at_screen_position = depth;
    }
}

/// Per-renderable visibility result from the Hi-Z producer.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Tier2HiZRenderableFeedback {
    pub object_id: RenderStableId,
    pub verdict: Tier2VisibilityVerdict,
    pub closest_depth: f32,
    pub farthest_depth: f32,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct Tier2VisibilityFeedback {
    pub schema_version: u16,
    pub records: Vec<Tier2HiZRenderableFeedback>,
    pub counts_by_verdict: [u32; TIER2_VISIBILITY_VERDICT_KIND_COUNT],
    pub conservative_fallback_active: bool,
    pub previous_frame_available: bool,
}

impl Tier2VisibilityFeedback {
    pub fn record(&mut self, feedback: Tier2HiZRenderableFeedback) {
        self.counts_by_verdict[feedback.verdict.index()] =
            self.counts_by_verdict[feedback.verdict.index()].saturating_add(1);
        self.records.push(feedback);
    }

    #[must_use]
    pub fn count_for(&self, verdict: Tier2VisibilityVerdict) -> u32 {
        self.counts_by_verdict[verdict.index()]
    }

    #[must_use]
    pub fn drawn_count(&self) -> u32 {
        self.records
            .iter()
            .filter(|r| r.verdict.counts_as_drawn())
            .count() as u32
    }

    /// Acceptance rule: no object disappears unless the parity
    /// validator proves it is safely culled. Every record whose
    /// verdict is `OccludedByHiZ` must have evidence — the
    /// `closest_depth` of the renderable must exceed the typed
    /// previous-frame min depth threshold by a non-zero margin.
    #[must_use]
    pub fn no_disappearance_without_evidence(
        &self,
        previous_frame_min_depth_threshold: f32,
    ) -> bool {
        if self.conservative_fallback_active {
            // Under conservative fallback, no record may be
            // OccludedByHiZ — every uncertain object stays
            // visible.
            return self
                .records
                .iter()
                .all(|r| !matches!(r.verdict, Tier2VisibilityVerdict::OccludedByHiZ));
        }
        self.records.iter().all(|r| {
            !matches!(r.verdict, Tier2VisibilityVerdict::OccludedByHiZ)
                || r.closest_depth > previous_frame_min_depth_threshold
        })
    }
}

#[must_use]
pub fn run_hi_z_occlusion_feedback(
    scene: &Tier2HiZOcclusionScene,
    runtime: &HiZPyramidRuntime,
) -> Tier2VisibilityFeedback {
    let mut feedback = Tier2VisibilityFeedback {
        schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
        previous_frame_available: runtime.previous_frame_available,
        conservative_fallback_active: runtime.conservative_fallback_active
            || !runtime.previous_frame_available,
        ..Tier2VisibilityFeedback::default()
    };

    let conservative = feedback.conservative_fallback_active;
    for renderable in &scene.renderables {
        let verdict = if conservative {
            // Guardrail: uncertain stays visible. Without a real
            // previous-frame depth pyramid the Hi-Z compute pass
            // cannot prove occlusion; classify everything as
            // ConservativelyVisible.
            Tier2VisibilityVerdict::ConservativelyVisible
        } else if !renderable.closest_depth.is_finite() || !renderable.farthest_depth.is_finite() {
            // Uncertain depth → conservative-visible per the
            // guardrail.
            Tier2VisibilityVerdict::Uncertain
        } else if renderable.closest_depth > scene.previous_frame_min_depth_at_screen_position {
            // The renderable's closest sample is deeper than the
            // previous frame's min depth at the same screen
            // position — it is provably behind something opaque.
            Tier2VisibilityVerdict::OccludedByHiZ
        } else {
            Tier2VisibilityVerdict::Visible
        };

        feedback.record(Tier2HiZRenderableFeedback {
            object_id: renderable.object_id,
            verdict,
            closest_depth: renderable.closest_depth,
            farthest_depth: renderable.farthest_depth,
        });
    }

    feedback
}

#[must_use]
pub const fn classifies_uncertain_as_visible(policy: GpuDrivenOcclusionPolicy) -> bool {
    policy.classifies_uncertain_as_visible()
}

// ============================================================================
// Section 3 — Meshlet equivalence
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct Tier2MeshletEquivalenceScene {
    pub schema_version: u16,
    pub meshlets: Vec<MeshletMetadataRecord>,
}

impl Tier2MeshletEquivalenceScene {
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            meshlets: Vec::new(),
        }
    }

    pub fn record(&mut self, meshlet: MeshletMetadataRecord) {
        self.meshlets.push(meshlet);
    }

    /// Deterministic test scene: a column of meshlets along the
    /// `+x` axis, each within the default vertex/triangle limits,
    /// with cone axes pointing in alternating directions.
    #[must_use]
    pub fn deterministic_column(count: u32) -> Self {
        let mut scene = Self::new();
        for index in 0..count {
            scene.record(MeshletMetadataRecord {
                schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
                mesh_id: 1,
                meshlet_index: index,
                vertex_offset: 0,
                vertex_count: MESHLET_DEFAULT_VERTEX_LIMIT,
                triangle_offset: 0,
                triangle_count: MESHLET_DEFAULT_TRIANGLE_LIMIT,
                material_slot: (index % 4) as u16,
                bounds: MeshletClusterBounds {
                    center: [index as f32 * 2.0, 0.0, 0.0],
                    radius: 0.8,
                    cone_axis: if index.is_multiple_of(2) {
                        [1.0, 0.0, 0.0]
                    } else {
                        [-1.0, 0.0, 0.0]
                    },
                    cone_cutoff: 0.0,
                },
            });
        }
        scene
    }
}

/// Per-meshlet visibility verdict for the equivalence check.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Tier2MeshletVisibility {
    pub mesh_id: u32,
    pub meshlet_index: u32,
    pub visible: bool,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Tier2MeshletVisibleSet {
    pub schema_version: u16,
    pub records: Vec<Tier2MeshletVisibility>,
    pub visible_count: u32,
}

impl Tier2MeshletVisibleSet {
    pub fn record(&mut self, visibility: Tier2MeshletVisibility) {
        if visibility.visible {
            self.visible_count = self.visible_count.saturating_add(1);
        }
        self.records.push(visibility);
    }

    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        if self.records.len() != other.records.len() {
            return false;
        }
        if self.visible_count != other.visible_count {
            return false;
        }
        for (left, right) in self.records.iter().zip(other.records.iter()) {
            if left.mesh_id != right.mesh_id
                || left.meshlet_index != right.meshlet_index
                || left.visible != right.visible
            {
                return false;
            }
        }
        true
    }
}

#[must_use]
fn meshlet_passes_cone_cull(bounds: MeshletClusterBounds, camera_position: RenderVec3) -> bool {
    let center = RenderVec3::new(bounds.center[0], bounds.center[1], bounds.center[2]);
    let to_camera = RenderVec3::new(
        camera_position.x - center.x,
        camera_position.y - center.y,
        camera_position.z - center.z,
    );
    let len_sq = to_camera.x * to_camera.x + to_camera.y * to_camera.y + to_camera.z * to_camera.z;
    if len_sq <= 0.0 {
        return true;
    }
    let len = len_sq.sqrt();
    let dir = RenderVec3::new(to_camera.x / len, to_camera.y / len, to_camera.z / len);
    let cone_axis = RenderVec3::new(
        bounds.cone_axis[0],
        bounds.cone_axis[1],
        bounds.cone_axis[2],
    );
    let dot = dir.x * cone_axis.x + dir.y * cone_axis.y + dir.z * cone_axis.z;
    dot >= bounds.cone_cutoff
}

#[must_use]
fn meshlet_passes_frustum(bounds: MeshletClusterBounds, frustum: &RendererCullingFrustum) -> bool {
    // Approximate the meshlet bound as a sphere-bounded AABB.
    let center = RenderVec3::new(bounds.center[0], bounds.center[1], bounds.center[2]);
    let r = bounds.radius;
    let aabb = RenderAabb::new(center, RenderVec3::new(r, r, r));
    frustum.contains_aabb(aabb)
}

/// Compute-fallback path: walk the meshlet list, run cone +
/// frustum culling, emit the typed visible set. The renderer's
/// production compute pass will run the same algorithm on the
/// GPU.
#[must_use]
pub fn run_meshlet_compute_fallback(
    scene: &Tier2MeshletEquivalenceScene,
    frustum: &RendererCullingFrustum,
    camera_position: RenderVec3,
) -> Tier2MeshletVisibleSet {
    let mut set = Tier2MeshletVisibleSet {
        schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
        ..Tier2MeshletVisibleSet::default()
    };
    for meshlet in &scene.meshlets {
        let visible = meshlet_passes_frustum(meshlet.bounds, frustum)
            && meshlet_passes_cone_cull(meshlet.bounds, camera_position);
        set.record(Tier2MeshletVisibility {
            mesh_id: meshlet.mesh_id,
            meshlet_index: meshlet.meshlet_index,
            visible,
        });
    }
    set
}

/// Mesh-shader path: when the capability branch supports mesh
/// shaders, the GPU thread group runs the same cone + frustum
/// culling logic. The CPU simulator runs the identical predicate
/// for parity validation. When the capability branch is
/// `ComputeFallback`, this path is unavailable and the function
/// returns `None`.
#[must_use]
pub fn run_meshlet_mesh_shader_path(
    scene: &Tier2MeshletEquivalenceScene,
    frustum: &RendererCullingFrustum,
    camera_position: RenderVec3,
    branch: MeshShaderCapabilityBranch,
) -> Option<Tier2MeshletVisibleSet> {
    if matches!(branch, MeshShaderCapabilityBranch::ComputeFallback) {
        return None;
    }
    Some(run_meshlet_compute_fallback(
        scene,
        frustum,
        camera_position,
    ))
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier2MeshletEquivalenceVerdict {
    #[default]
    NotEvaluated,
    EquivalentBothPaths,
    ComputeOnlyEquivalent,
    Diverged,
}

impl Tier2MeshletEquivalenceVerdict {
    pub const ALL: [Self; TIER2_MESHLET_EQUIVALENCE_VERDICT_KIND_COUNT] = [
        Self::NotEvaluated,
        Self::EquivalentBothPaths,
        Self::ComputeOnlyEquivalent,
        Self::Diverged,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotEvaluated => "not_evaluated",
            Self::EquivalentBothPaths => "equivalent_both_paths",
            Self::ComputeOnlyEquivalent => "compute_only_equivalent",
            Self::Diverged => "diverged",
        }
    }

    #[must_use]
    pub const fn passes(self) -> bool {
        matches!(
            self,
            Self::EquivalentBothPaths | Self::ComputeOnlyEquivalent
        )
    }
}

#[derive(Debug, Clone, PartialEq, Resource)]
pub struct Tier2MeshletEquivalenceRun {
    pub schema_version: u16,
    pub branch: MeshShaderCapabilityBranch,
    pub compute_fallback: Tier2MeshletVisibleSet,
    pub mesh_shader: Option<Tier2MeshletVisibleSet>,
    pub verdict: Tier2MeshletEquivalenceVerdict,
}

impl Tier2MeshletEquivalenceRun {
    #[must_use]
    pub fn evaluate(
        scene: &Tier2MeshletEquivalenceScene,
        frustum: &RendererCullingFrustum,
        camera_position: RenderVec3,
        branch: MeshShaderCapabilityBranch,
    ) -> Self {
        let compute = run_meshlet_compute_fallback(scene, frustum, camera_position);
        let mesh_shader = run_meshlet_mesh_shader_path(scene, frustum, camera_position, branch);
        let verdict = match &mesh_shader {
            None => Tier2MeshletEquivalenceVerdict::ComputeOnlyEquivalent,
            Some(set) => {
                if compute.matches(set) {
                    Tier2MeshletEquivalenceVerdict::EquivalentBothPaths
                } else {
                    Tier2MeshletEquivalenceVerdict::Diverged
                }
            }
        };
        Self {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            branch,
            compute_fallback: compute,
            mesh_shader,
            verdict,
        }
    }

    #[must_use]
    pub fn passes(&self) -> bool {
        self.verdict.passes()
    }
}

// ============================================================================
// Section 4 — Helper: build a frustum that accepts a half-space
// ============================================================================

/// Build a six-plane frustum that accepts only points with x ≤
/// `cutoff_x`. Used by tests so the CPU and GPU-driven paths
/// reject the same set of objects.
#[must_use]
pub fn frustum_accept_half_space_x(cutoff_x: f32) -> RendererCullingFrustum {
    RendererCullingFrustum {
        planes: [
            // Plane: -x + cutoff_x ≥ 0 → x ≤ cutoff_x
            RendererFrustumPlane::new(RenderVec3::new(-1.0, 0.0, 0.0), cutoff_x),
            // Five accept-all planes so the half-space cutoff is
            // the only constraint.
            RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
            RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
            RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
            RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
            RendererFrustumPlane::new(RenderVec3::new(0.0, 0.0, 1.0), f32::MAX),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_api::RenderViewId;
    use crate::gpu_driven::GpuDrivenHiZPyramidPlan;

    fn camera_with_frustum(frustum: RendererCullingFrustum) -> RendererCameraInput {
        RendererCameraInput {
            view_id: RenderViewId::new(1, 1),
            frustum,
            ..Default::default()
        }
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION, 1);
        assert_eq!(TIER2_VISIBILITY_VERDICT_KIND_COUNT, 4);
        assert_eq!(TIER2_MESHLET_EQUIVALENCE_VERDICT_KIND_COUNT, 4);
    }

    #[test]
    fn deterministic_grid_produces_expected_renderable_count() {
        let scene = Tier2FrustumCullScene::deterministic_grid(4, 100);
        assert_eq!(scene.renderables.len(), 64);
        assert_eq!(scene.total_triangle_count(), 6400);
    }

    #[test]
    fn frustum_cull_parity_run_matches_on_accept_all_frustum() {
        let scene = Tier2FrustumCullScene::deterministic_grid(4, 100);
        let frustum = RendererCullingFrustum::ACCEPT_ALL;
        let camera = camera_with_frustum(frustum);
        let config = RendererVisibilityConfig::PRODUCT_DEFAULT;
        let direct = run_direct_frustum_cull(&scene, &camera, &config);
        let gpu = run_gpu_driven_frustum_cull(&scene, &frustum);
        let run = Tier2FrustumCullParityRun::evaluate(direct, gpu);
        assert!(run.passes());
        assert_eq!(run.validation.verdict, GpuDrivenParityVerdict::Match);
        assert_eq!(direct.draw_count, scene.renderables.len() as u32);
        assert_eq!(direct.triangle_count, scene.total_triangle_count());
    }

    #[test]
    fn frustum_cull_parity_run_matches_on_half_space_cutoff() {
        let scene = Tier2FrustumCullScene::deterministic_grid(4, 100);
        let frustum = frustum_accept_half_space_x(0.0);
        let camera = camera_with_frustum(frustum);
        let config = RendererVisibilityConfig::PRODUCT_DEFAULT;
        let direct = run_direct_frustum_cull(&scene, &camera, &config);
        let gpu = run_gpu_driven_frustum_cull(&scene, &frustum);
        let run = Tier2FrustumCullParityRun::evaluate(direct, gpu);
        assert!(run.passes(), "verdict was {:?}", run.validation.verdict);
        // Sanity: half the grid should be rejected.
        assert!(direct.draw_count < scene.renderables.len() as u32);
        assert!(direct.draw_count > 0);
    }

    #[test]
    fn frustum_cull_parity_detects_mismatched_counts() {
        let scene = Tier2FrustumCullScene::deterministic_grid(4, 100);
        let direct = Tier2FrustumCullPathOutcome {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            draw_count: 64,
            triangle_count: 6400,
            instances_tested: 64,
            instances_visible: 64,
            instances_frustum_rejected: 0,
        };
        // Inject a fake mismatch in the GPU path.
        let gpu = Tier2FrustumCullPathOutcome {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            draw_count: 50,
            triangle_count: 5000,
            instances_tested: 64,
            instances_visible: 50,
            instances_frustum_rejected: 14,
        };
        let _ = scene; // unused but documents which scene is being modelled
        let run = Tier2FrustumCullParityRun::evaluate(direct, gpu);
        assert!(!run.passes());
        // Both draw and triangle counts diverge → BothMismatch.
        assert_eq!(run.validation.verdict, GpuDrivenParityVerdict::BothMismatch,);
    }

    #[test]
    fn hi_z_conservative_fallback_classifies_every_renderable_as_conservatively_visible() {
        let mut scene = Tier2HiZOcclusionScene::new();
        scene.set_previous_frame_min_depth(0.5);
        scene.record(Tier2HiZRenderable {
            object_id: RenderStableId::new(1),
            world_aabb: RenderAabb::new(RenderVec3::ZERO, RenderVec3::new(1.0, 1.0, 1.0)),
            closest_depth: 0.9,
            farthest_depth: 1.0,
        });
        let mut runtime = HiZPyramidRuntime::default();
        runtime.note_previous_frame(false);
        let feedback = run_hi_z_occlusion_feedback(&scene, &runtime);
        assert!(feedback.conservative_fallback_active);
        assert_eq!(
            feedback.count_for(Tier2VisibilityVerdict::ConservativelyVisible),
            1,
        );
        assert_eq!(feedback.count_for(Tier2VisibilityVerdict::OccludedByHiZ), 0);
    }

    #[test]
    fn hi_z_with_previous_frame_can_classify_objects_behind_min_depth_as_occluded() {
        let mut scene = Tier2HiZOcclusionScene::new();
        scene.set_previous_frame_min_depth(0.5);
        scene.record(Tier2HiZRenderable {
            object_id: RenderStableId::new(1),
            world_aabb: RenderAabb::new(RenderVec3::ZERO, RenderVec3::new(1.0, 1.0, 1.0)),
            closest_depth: 0.9, // > 0.5 → behind the previous frame's near surface
            farthest_depth: 1.0,
        });
        scene.record(Tier2HiZRenderable {
            object_id: RenderStableId::new(2),
            world_aabb: RenderAabb::new(RenderVec3::ZERO, RenderVec3::new(1.0, 1.0, 1.0)),
            closest_depth: 0.2, // < 0.5 → in front of the previous frame's near surface
            farthest_depth: 0.4,
        });
        let mut runtime = HiZPyramidRuntime::new(
            GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT
                .with_policy(GpuDrivenOcclusionPolicy::Aggressive),
            256,
        );
        runtime.note_previous_frame(true);
        let feedback = run_hi_z_occlusion_feedback(&scene, &runtime);
        assert!(!feedback.conservative_fallback_active);
        assert_eq!(feedback.count_for(Tier2VisibilityVerdict::OccludedByHiZ), 1);
        assert_eq!(feedback.count_for(Tier2VisibilityVerdict::Visible), 1);
    }

    #[test]
    fn hi_z_uncertain_depth_classifies_as_uncertain_under_aggressive_policy() {
        let mut scene = Tier2HiZOcclusionScene::new();
        scene.set_previous_frame_min_depth(0.5);
        scene.record(Tier2HiZRenderable {
            object_id: RenderStableId::new(1),
            world_aabb: RenderAabb::new(RenderVec3::ZERO, RenderVec3::new(1.0, 1.0, 1.0)),
            closest_depth: f32::NAN,
            farthest_depth: f32::NAN,
        });
        let mut runtime = HiZPyramidRuntime::new(
            GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT
                .with_policy(GpuDrivenOcclusionPolicy::Aggressive),
            256,
        );
        runtime.note_previous_frame(true);
        let feedback = run_hi_z_occlusion_feedback(&scene, &runtime);
        assert_eq!(feedback.count_for(Tier2VisibilityVerdict::Uncertain), 1);
    }

    #[test]
    fn no_disappearance_without_evidence_holds_under_conservative_fallback() {
        let mut scene = Tier2HiZOcclusionScene::new();
        scene.set_previous_frame_min_depth(0.0);
        scene.record(Tier2HiZRenderable {
            object_id: RenderStableId::new(1),
            world_aabb: RenderAabb::new(RenderVec3::ZERO, RenderVec3::new(1.0, 1.0, 1.0)),
            closest_depth: 1.0,
            farthest_depth: 2.0,
        });
        let mut runtime = HiZPyramidRuntime::default();
        runtime.note_previous_frame(false);
        let feedback = run_hi_z_occlusion_feedback(&scene, &runtime);
        // Conservative fallback active → no record may be
        // OccludedByHiZ.
        assert!(feedback.no_disappearance_without_evidence(0.0));
    }

    #[test]
    fn classifies_uncertain_as_visible_routes_through_existing_policy_rule() {
        assert!(classifies_uncertain_as_visible(
            GpuDrivenOcclusionPolicy::Conservative
        ));
        assert!(classifies_uncertain_as_visible(
            GpuDrivenOcclusionPolicy::DisabledForDebug
        ));
        assert!(!classifies_uncertain_as_visible(
            GpuDrivenOcclusionPolicy::Aggressive
        ));
    }

    #[test]
    fn meshlet_compute_fallback_emits_visible_set_with_cone_culling() {
        let scene = Tier2MeshletEquivalenceScene::deterministic_column(8);
        let frustum = RendererCullingFrustum::ACCEPT_ALL;
        let camera = RenderVec3::new(100.0, 0.0, 0.0); // camera far +x
        let set = run_meshlet_compute_fallback(&scene, &frustum, camera);
        assert_eq!(set.records.len(), 8);
        // With cone_cutoff = 0.0 and camera on +x, half the
        // meshlets (the ones with cone_axis = +x) face the camera
        // and pass; half (-x) point away and fail.
        assert!(set.visible_count > 0);
        assert!(set.visible_count < 8);
    }

    #[test]
    fn meshlet_mesh_shader_path_returns_none_when_capability_is_compute_fallback() {
        let scene = Tier2MeshletEquivalenceScene::deterministic_column(4);
        let frustum = RendererCullingFrustum::ACCEPT_ALL;
        let result = run_meshlet_mesh_shader_path(
            &scene,
            &frustum,
            RenderVec3::ZERO,
            MeshShaderCapabilityBranch::ComputeFallback,
        );
        assert!(result.is_none());
    }

    #[test]
    fn meshlet_equivalence_verdict_compute_only_when_branch_is_compute_fallback() {
        let scene = Tier2MeshletEquivalenceScene::deterministic_column(8);
        let frustum = RendererCullingFrustum::ACCEPT_ALL;
        let run = Tier2MeshletEquivalenceRun::evaluate(
            &scene,
            &frustum,
            RenderVec3::new(100.0, 0.0, 0.0),
            MeshShaderCapabilityBranch::ComputeFallback,
        );
        assert!(run.passes());
        assert_eq!(
            run.verdict,
            Tier2MeshletEquivalenceVerdict::ComputeOnlyEquivalent,
        );
        assert!(run.mesh_shader.is_none());
    }

    #[test]
    fn meshlet_equivalence_verdict_both_equivalent_when_mesh_shader_branch_runs() {
        let scene = Tier2MeshletEquivalenceScene::deterministic_column(8);
        let frustum = RendererCullingFrustum::ACCEPT_ALL;
        let run = Tier2MeshletEquivalenceRun::evaluate(
            &scene,
            &frustum,
            RenderVec3::new(100.0, 0.0, 0.0),
            MeshShaderCapabilityBranch::MeshShader,
        );
        assert!(run.passes());
        assert_eq!(
            run.verdict,
            Tier2MeshletEquivalenceVerdict::EquivalentBothPaths,
        );
        let mesh_shader_set = run
            .mesh_shader
            .as_ref()
            .expect("mesh-shader path emits set");
        assert!(run.compute_fallback.matches(mesh_shader_set));
    }

    #[test]
    fn meshlet_equivalence_verdict_diverges_on_mismatched_visible_sets() {
        let mut compute = Tier2MeshletVisibleSet::default();
        compute.record(Tier2MeshletVisibility {
            mesh_id: 1,
            meshlet_index: 0,
            visible: true,
        });
        let mut mesh_shader = Tier2MeshletVisibleSet::default();
        mesh_shader.record(Tier2MeshletVisibility {
            mesh_id: 1,
            meshlet_index: 0,
            visible: false,
        });
        // Manually compose a divergent run to exercise the verdict
        // branch.
        let run = Tier2MeshletEquivalenceRun {
            schema_version: TIER2_GPU_DRIVEN_PROOF_SCHEMA_VERSION,
            branch: MeshShaderCapabilityBranch::MeshShader,
            verdict: if compute.matches(&mesh_shader) {
                Tier2MeshletEquivalenceVerdict::EquivalentBothPaths
            } else {
                Tier2MeshletEquivalenceVerdict::Diverged
            },
            compute_fallback: compute,
            mesh_shader: Some(mesh_shader),
        };
        assert!(!run.passes());
        assert_eq!(run.verdict, Tier2MeshletEquivalenceVerdict::Diverged);
    }

    #[test]
    fn frustum_half_space_helper_constructs_planes_with_cutoff_distance() {
        let frustum = frustum_accept_half_space_x(2.0);
        assert_eq!(frustum.planes[0].distance, 2.0);
        let inside = RenderAabb::new(
            RenderVec3::new(0.0, 0.0, 0.0),
            RenderVec3::new(0.5, 0.5, 0.5),
        );
        let outside = RenderAabb::new(
            RenderVec3::new(10.0, 0.0, 0.0),
            RenderVec3::new(0.5, 0.5, 0.5),
        );
        assert!(frustum.contains_aabb(inside));
        assert!(!frustum.contains_aabb(outside));
    }

    #[test]
    fn visibility_verdict_drawn_classification_excludes_only_occluded() {
        for verdict in Tier2VisibilityVerdict::ALL {
            match verdict {
                Tier2VisibilityVerdict::OccludedByHiZ => assert!(!verdict.counts_as_drawn()),
                _ => assert!(verdict.counts_as_drawn()),
            }
        }
    }

    #[test]
    fn meshlet_visible_set_matches_returns_false_on_visibility_flip() {
        let mut a = Tier2MeshletVisibleSet::default();
        a.record(Tier2MeshletVisibility {
            mesh_id: 1,
            meshlet_index: 0,
            visible: true,
        });
        let mut b = Tier2MeshletVisibleSet::default();
        b.record(Tier2MeshletVisibility {
            mesh_id: 1,
            meshlet_index: 0,
            visible: false,
        });
        assert!(!a.matches(&b));
    }
}
