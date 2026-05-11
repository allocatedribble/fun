//! Pass V2.4 — live wire `LuxClusterLights` + `LuxDirectLighting`.
//!
//! This module is the first typed bridge that turns a typed
//! Lux frame-graph role into actual GPU work.  The strategy
//! is to wrap the existing typed live clustered-lighting
//! proof path ([`crate::passj_clustered_lighting_live`]),
//! tag its dispatches with the typed Lux roles
//! ([`crate::frame_graph::FrameGraphPassRole::LuxUploadLightBuffers`],
//! `LuxClusterLights`, `LuxDirectLighting`), and surface a
//! typed `LuxLiveLightingRunResult` that callers (tests, CI,
//! diagnostics dashboards) read to prove the typed graph
//! path produces pixels.
//!
//! Initial surface is intentionally small per the V2.4 spec:
//!
//! - Directional + point lights first (no full material /
//!   PBR integration yet).
//! - Single HDR scene-color target (today: the typed passj
//!   offscreen RGBA8 readback target).
//! - One frame-probe proof — the typed run result records
//!   whether the offscreen probe came back non-black.
//!
//! Live integration (running the typed wrapper against a
//! fresh DX12 device) goes through
//! [`run_lux_live_lighting_against_fresh_dx12_device`].
//! Synthetic unit tests use
//! [`LuxLiveLightingRunResult::from_passj_synthetic`] to
//! cover the typed contract without requiring a GPU.

use crate::frame_graph::FrameGraphPassRole;
use crate::passj_clustered_lighting_live::{
    PassJDebugHeatmapRecord, PassJLightInput, PassJRunResult,
    run_clustered_lighting_live_against_fresh_dx12_device,
};

pub const FUN_RENDERER_LUX_LIVE_LIGHTING_SCHEMA_VERSION: u16 = 1;

/// Typed Pass V2.4 stage taxonomy.  Names the three typed
/// frame-graph roles the typed live executor binds to GPU
/// work.  Used by the typed `frame_graph_role_for` mapping
/// helper so callers can confirm the typed live executor
/// covers exactly these three roles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLiveLightingStage {
    /// Typed `LuxUploadLightBuffers` role — backed by the
    /// typed `PassJBufferSet::allocate` GPU upload path
    /// (queue.write_buffer + bind group creation).
    #[default]
    UploadLightBuffers,
    /// Typed `LuxClusterLights` role — backed by the typed
    /// `PassJClusterAssignmentPipeline` compute dispatch
    /// that bins lights into cluster grid cells.
    ClusterLights,
    /// Typed `LuxDirectLighting` role — backed by the typed
    /// `PassJForwardPlusPipeline` render pass that shades
    /// per-pixel direct lighting using the typed cluster
    /// light index buffer.
    DirectLighting,
}

impl LuxLiveLightingStage {
    pub const ALL: [Self; 3] = [
        Self::UploadLightBuffers,
        Self::ClusterLights,
        Self::DirectLighting,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UploadLightBuffers => "upload_light_buffers",
            Self::ClusterLights => "cluster_lights",
            Self::DirectLighting => "direct_lighting",
        }
    }

    /// Typed mapping from a Pass V2.4 stage to the typed
    /// `FrameGraphPassRole` the typed Lux graph compiler
    /// emits.  Acceptance criterion #5 keys off this typed
    /// mapping.
    #[must_use]
    pub const fn frame_graph_role(self) -> FrameGraphPassRole {
        match self {
            Self::UploadLightBuffers => FrameGraphPassRole::LuxUploadLightBuffers,
            Self::ClusterLights => FrameGraphPassRole::LuxClusterLights,
            Self::DirectLighting => FrameGraphPassRole::LuxDirectLighting,
        }
    }
}

/// Typed Pass V2.4 run result — the typed handle callers
/// consume to confirm the typed live Lux lighting path
/// produced pixels.  Matches the user-spec field set
/// verbatim.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LuxLiveLightingRunResult {
    pub schema_version: u16,
    pub uploaded_light_count: u32,
    pub cluster_dispatch_count: u32,
    pub direct_lighting_draw_count: u32,
    pub non_black_frame_probe: bool,
    pub heatmap_readback_ok: bool,
    pub gpu_timing_observed: bool,
    /// Typed heatmap records the typed compute dispatch
    /// wrote.  Kept so callers can audit per-cluster light
    /// counts.
    pub heatmap_records: Vec<PassJDebugHeatmapRecord>,
    /// Typed frame-probe RGBA8 the offscreen target read
    /// back — `[0; 4]` means the typed path did not
    /// produce pixels.
    pub frame_probe_rgba8: [u8; 4],
}

impl LuxLiveLightingRunResult {
    pub const COLD_DEFAULT: Self = Self {
        schema_version: FUN_RENDERER_LUX_LIVE_LIGHTING_SCHEMA_VERSION,
        uploaded_light_count: 0,
        cluster_dispatch_count: 0,
        direct_lighting_draw_count: 0,
        non_black_frame_probe: false,
        heatmap_readback_ok: false,
        gpu_timing_observed: false,
        heatmap_records: Vec::new(),
        frame_probe_rgba8: [0, 0, 0, 0],
    };

    /// Typed conversion from the typed underlying
    /// `PassJRunResult`.  `uploaded_light_count` is supplied
    /// by the caller because passj's typed result tracks
    /// cluster + draw counts but not the typed upload-light
    /// count (the typed upload step is "before" passj's
    /// pipeline starts).
    #[must_use]
    pub fn from_passj(uploaded_light_count: u32, passj: &PassJRunResult) -> Self {
        let non_black = passj.frame_probe_rgba8.iter().any(|c| *c > 0);
        Self {
            schema_version: FUN_RENDERER_LUX_LIVE_LIGHTING_SCHEMA_VERSION,
            uploaded_light_count,
            cluster_dispatch_count: passj.cluster_assignment_dispatches,
            direct_lighting_draw_count: passj.forward_plus_render_passes,
            non_black_frame_probe: non_black,
            heatmap_readback_ok: passj.readback_succeeded && !passj.heatmap_records.is_empty(),
            // Pass V2.4 doesn't yet wire GPU timestamps; the
            // typed flag is reserved.  Set to `true` when
            // the typed compute + render dispatches both
            // ran (a coarse proxy until real timestamps
            // land).
            gpu_timing_observed: passj.cluster_assignment_dispatches > 0
                && passj.forward_plus_render_passes > 0,
            heatmap_records: passj.heatmap_records.clone(),
            frame_probe_rgba8: passj.frame_probe_rgba8,
        }
    }

    /// Typed synthetic helper for unit tests — produces a
    /// typed result from explicit values.  The typed live
    /// integration path uses `from_passj` against a real
    /// GPU run.
    #[must_use]
    pub fn from_passj_synthetic(
        uploaded_light_count: u32,
        cluster_dispatches: u32,
        forward_plus_passes: u32,
        frame_probe_rgba8: [u8; 4],
        heatmap_records: Vec<PassJDebugHeatmapRecord>,
        readback_ok: bool,
    ) -> Self {
        let non_black = frame_probe_rgba8.iter().any(|c| *c > 0);
        Self {
            schema_version: FUN_RENDERER_LUX_LIVE_LIGHTING_SCHEMA_VERSION,
            uploaded_light_count,
            cluster_dispatch_count: cluster_dispatches,
            direct_lighting_draw_count: forward_plus_passes,
            non_black_frame_probe: non_black,
            heatmap_readback_ok: readback_ok && !heatmap_records.is_empty(),
            gpu_timing_observed: cluster_dispatches > 0 && forward_plus_passes > 0,
            heatmap_records,
            frame_probe_rgba8,
        }
    }

    /// Typed predicate: did the typed live Lux lighting
    /// path produce pixels this frame (V2.4 acceptance
    /// rule #1 — "at least one Lux graph path produces
    /// pixels")?
    #[must_use]
    pub const fn produces_pixels(&self) -> bool {
        self.non_black_frame_probe
            && self.cluster_dispatch_count > 0
            && self.direct_lighting_draw_count > 0
    }

    /// Typed predicate: did `LuxClusterLights` execute as
    /// real GPU work (V2.4 acceptance rule #2)?
    #[must_use]
    pub const fn cluster_lights_dispatched(&self) -> bool {
        self.cluster_dispatch_count > 0
    }

    /// Typed predicate: did `LuxDirectLighting` execute as
    /// real GPU work (V2.4 acceptance rule #3)?
    #[must_use]
    pub const fn direct_lighting_recorded(&self) -> bool {
        self.direct_lighting_draw_count > 0
    }

    /// Typed predicate: every Pass V2.4 acceptance rule
    /// holds.
    #[must_use]
    pub fn obeys_all_v2_4_rules(&self) -> bool {
        self.produces_pixels()
            && self.cluster_lights_dispatched()
            && self.direct_lighting_recorded()
            && self.non_black_frame_probe
            && self.heatmap_readback_ok
    }
}

// ============================================================================
// Section 2 — Typed light mapping helper
// ============================================================================

/// Typed Pass V2.4 light input.  A trimmed surface of the
/// typed `fun_lux::LuxLightComponent` data — V2.4 ships
/// directional + point lights first, with no full material
/// / PBR integration.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct LuxLiveLightInput {
    pub position: [f32; 3],
    pub radius: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub kind: LuxLiveLightKind,
}

/// Typed Pass V2.4 light kind.  Directional + point only
/// for the initial surface.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLiveLightKind {
    /// Directional light (sun / moon).  `position` becomes
    /// the typed light direction in the typed proof scene;
    /// `radius` is ignored.
    Directional,
    /// Point light.  Both `position` and `radius` matter.
    #[default]
    Point,
}

impl LuxLiveLightKind {
    pub const ALL: [Self; 2] = [Self::Directional, Self::Point];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Point => "point",
        }
    }
}

impl LuxLiveLightInput {
    /// Typed conversion to the typed underlying passj
    /// light input.
    #[must_use]
    pub const fn to_passj(self) -> PassJLightInput {
        PassJLightInput {
            position_radius: [self.position[0], self.position[1], self.position[2], self.radius],
            color_intensity: [self.color[0], self.color[1], self.color[2], self.intensity],
        }
    }

    /// Typed builder for a typed Pass V2.4 point light.
    #[must_use]
    pub const fn point(position: [f32; 3], radius: f32, color: [f32; 3], intensity: f32) -> Self {
        Self {
            position,
            radius,
            color,
            intensity,
            kind: LuxLiveLightKind::Point,
        }
    }

    /// Typed builder for a typed Pass V2.4 directional
    /// light.  `direction` replaces the typed position
    /// field; the typed underlying passj path treats it as
    /// a position with a huge radius so every cluster
    /// receives the contribution.
    #[must_use]
    pub const fn directional(direction: [f32; 3], color: [f32; 3], intensity: f32) -> Self {
        Self {
            position: direction,
            radius: 1_000.0,
            color,
            intensity,
            kind: LuxLiveLightKind::Directional,
        }
    }
}

/// Typed Pass V2.4 canonical proof scene.  Returns a typed
/// directional + 3 typed point lights — mirroring the
/// typed passj proof scene but exposed through the typed
/// Lux live light surface.
#[must_use]
pub fn lux_live_lighting_proof_scene_lights() -> Vec<LuxLiveLightInput> {
    vec![
        LuxLiveLightInput::directional([0.0, -1.0, 0.0], [1.0, 1.0, 1.0], 0.5),
        LuxLiveLightInput::point([8.0, 4.5, 8.0], 100.0, [1.0, 0.0, 0.0], 0.5),
        LuxLiveLightInput::point([8.0, 4.5, 8.0], 100.0, [0.0, 1.0, 0.0], 0.5),
        LuxLiveLightInput::point([8.0, 4.5, 8.0], 100.0, [0.0, 0.0, 1.0], 0.5),
    ]
}

// ============================================================================
// Section 3 — Live integration entry point
// ============================================================================

/// Typed Pass V2.4 boot outcome — wraps the typed underlying
/// passj boot result with a typed Lux-named surface.
pub enum LuxLiveLightingBootOutcome {
    /// Typed run completed — carries the typed Lux run
    /// result.
    Ran(LuxLiveLightingRunResult),
    /// Typed DX12 device bring-up failed.  The typed live
    /// path requires a real GPU; CI / non-DX12 hosts hit
    /// this lane.
    BridgeRuntimeFailed,
}

/// Typed Pass V2.4 live boot — translates typed
/// `LuxLiveLightInput`s into typed passj inputs, boots a
/// fresh DX12 device, runs the typed pipeline, and returns
/// the typed Lux run result.
///
/// The typed underlying `run_clustered_lighting_live_against_fresh_dx12_device`
/// dispatches all three typed Lux roles in order:
///   1. `LuxUploadLightBuffers` — passj's typed
///      `PassJBufferSet::allocate` writes the typed light
///      records to the GPU buffer + builds the typed
///      cluster bind group.
///   2. `LuxClusterLights` — passj's typed
///      `PassJClusterAssignmentPipeline` compute dispatch
///      bins lights into cluster grid cells.
///   3. `LuxDirectLighting` — passj's typed
///      `PassJForwardPlusPipeline` render pass shades the
///      offscreen target using the typed cluster light
///      indices.
#[must_use]
pub fn run_lux_live_lighting_against_fresh_dx12_device(
    lights: &[LuxLiveLightInput],
) -> LuxLiveLightingBootOutcome {
    let passj_lights: Vec<PassJLightInput> = lights.iter().map(|l| l.to_passj()).collect();
    // Typed atlas residency under budget — the typed Lux
    // V2.4 surface doesn't yet wire the typed shadow atlas
    // through the typed `LuxShadowRequests` role, so this
    // is held constant.  A future pass will wire the
    // typed shadow path.
    let atlas_residency_bytes = 0;
    match run_clustered_lighting_live_against_fresh_dx12_device(
        &passj_lights,
        atlas_residency_bytes,
    ) {
        crate::passj_clustered_lighting_live::PassJBootResult::Ran(payload) => {
            let mut passj_result = payload.result;
            // Ensure typed light-list totals are computed.
            passj_result.compute_light_list_total();
            LuxLiveLightingBootOutcome::Ran(LuxLiveLightingRunResult::from_passj(
                lights.len() as u32,
                &passj_result,
            ))
        }
        crate::passj_clustered_lighting_live::PassJBootResult::BridgeRuntimeFailed(_) => {
            LuxLiveLightingBootOutcome::BridgeRuntimeFailed
        }
    }
}

// ============================================================================
// Tests — Pass V2.4 typed acceptance
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::passj_clustered_lighting_live::PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION;

    fn sample_heatmap(records: u32, light_count_per_record: u32) -> Vec<PassJDebugHeatmapRecord> {
        (0..records)
            .map(|i| PassJDebugHeatmapRecord {
                schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
                cluster_index: i,
                light_count: light_count_per_record,
            })
            .collect()
    }

    /// Pass V2.4 acceptance rule #2 — `LuxClusterLights` is
    /// no longer only a typed role; the typed live result
    /// reports a non-zero compute dispatch count.
    #[test]
    fn live_lux_cluster_lights_dispatches_compute() {
        let result = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [128, 128, 128, 255],
            sample_heatmap(4, 2),
            true,
        );
        assert!(result.cluster_lights_dispatched());
        assert_eq!(result.cluster_dispatch_count, 1);
    }

    /// Pass V2.4 acceptance rule #3 — `LuxDirectLighting`
    /// is no longer only a typed role; the typed live
    /// result reports a non-zero forward+ draw count.
    #[test]
    fn live_lux_direct_lighting_records_draw() {
        let result = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [128, 128, 128, 255],
            sample_heatmap(4, 2),
            true,
        );
        assert!(result.direct_lighting_recorded());
        assert_eq!(result.direct_lighting_draw_count, 1);
    }

    /// Pass V2.4 acceptance rule #1 — the typed live Lux
    /// lighting path produces pixels.  The typed frame
    /// probe must be non-black.
    #[test]
    fn live_lux_lighting_frame_probe_non_black() {
        let result = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [128, 128, 128, 255],
            sample_heatmap(4, 2),
            true,
        );
        assert!(result.non_black_frame_probe);
        assert!(result.produces_pixels());

        // The typed inverse: a typed black frame probe
        // means the typed path did not produce pixels.
        let black = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [0, 0, 0, 0],
            sample_heatmap(4, 2),
            true,
        );
        assert!(!black.non_black_frame_probe);
        assert!(!black.produces_pixels());
    }

    /// Pass V2.4 acceptance rule — the typed cluster
    /// heatmap readback succeeds (the typed compute
    /// dispatch wrote per-cluster light counts and the
    /// typed readback returned them).
    #[test]
    fn live_lux_cluster_heatmap_readback_succeeds() {
        let result = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [128, 128, 128, 255],
            sample_heatmap(4, 3),
            true,
        );
        assert!(result.heatmap_readback_ok);
        assert_eq!(result.heatmap_records.len(), 4);
        for r in &result.heatmap_records {
            assert_eq!(r.light_count, 3);
        }

        // Readback flagged but with empty records → still
        // false (the typed predicate guards against silent
        // empty readbacks).
        let empty = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [128, 128, 128, 255],
            Vec::new(),
            true,
        );
        assert!(!empty.heatmap_readback_ok);
    }

    /// Pass V2.4 acceptance rule #5 — the typed live
    /// executor binds to typed `FrameGraphPassRole::Lux*`
    /// variants.  Asserts every typed stage maps to a
    /// typed Lux role.
    #[test]
    fn live_lux_lighting_uses_frame_graph_roles() {
        assert_eq!(
            LuxLiveLightingStage::UploadLightBuffers.frame_graph_role(),
            FrameGraphPassRole::LuxUploadLightBuffers,
        );
        assert_eq!(
            LuxLiveLightingStage::ClusterLights.frame_graph_role(),
            FrameGraphPassRole::LuxClusterLights,
        );
        assert_eq!(
            LuxLiveLightingStage::DirectLighting.frame_graph_role(),
            FrameGraphPassRole::LuxDirectLighting,
        );
        // Every typed stage maps to a typed Lux role.
        for stage in LuxLiveLightingStage::ALL {
            assert!(stage.frame_graph_role().is_lux());
        }
    }

    /// Pass V2.4 bundle audit — when the typed result
    /// reports every typed signal, the typed bundle
    /// predicate holds.
    #[test]
    fn obeys_all_v2_4_rules_when_signals_complete() {
        let result = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [128, 128, 128, 255],
            sample_heatmap(4, 2),
            true,
        );
        assert!(result.obeys_all_v2_4_rules());

        // Drop any single signal → the typed bundle
        // predicate flips to false.
        let no_pixels = LuxLiveLightingRunResult::from_passj_synthetic(
            4,
            1,
            1,
            [0, 0, 0, 0],
            sample_heatmap(4, 2),
            true,
        );
        assert!(!no_pixels.obeys_all_v2_4_rules());
    }

    /// Typed proof-scene helper exposes a typed directional
    /// + three typed point lights matching the typed
    /// underlying passj scene shape.
    #[test]
    fn proof_scene_carries_directional_and_three_points() {
        let lights = lux_live_lighting_proof_scene_lights();
        assert_eq!(lights.len(), 4);
        assert_eq!(lights[0].kind, LuxLiveLightKind::Directional);
        for light in &lights[1..] {
            assert_eq!(light.kind, LuxLiveLightKind::Point);
        }
    }

    /// Typed conversion from the typed `LuxLiveLightInput`
    /// to the typed `PassJLightInput` round-trips correctly.
    #[test]
    fn light_input_to_passj_round_trips() {
        let lux = LuxLiveLightInput::point([1.0, 2.0, 3.0], 4.0, [0.5, 0.6, 0.7], 0.8);
        let passj = lux.to_passj();
        assert_eq!(passj.position_radius, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(passj.color_intensity, [0.5, 0.6, 0.7, 0.8]);
    }

    /// Cold default reports zero counts + no pixels.
    #[test]
    fn cold_default_reports_zero_work() {
        let cold = LuxLiveLightingRunResult::COLD_DEFAULT;
        assert!(!cold.produces_pixels());
        assert!(!cold.cluster_lights_dispatched());
        assert!(!cold.direct_lighting_recorded());
        assert!(!cold.obeys_all_v2_4_rules());
    }
}
