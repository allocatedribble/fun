use std::sync::Arc;

use bevy::prelude::*;
use fun_ecs::frame_graph::{ECS_CROSS_DOMAIN_FRAME_FLOW, EcsCrossDomainFrameStage};
use fun_renderer::component_api as renderer_api;

#[cfg(feature = "fun_renderer_core")]
use fun_renderer::{
    frame_graph::FrameGraphPassRole,
    render_work_graph::{RendererGraphInputs, run_renderer_work_graph_deterministic},
    schedule_contract::LuxNodeKind,
};
#[cfg(feature = "fun_renderer_core")]
use fun_scheduler_core::MonotonicClock;
#[cfg(feature = "fun_renderer_core")]
use fun_scheduler_types::work_graph::WorkGraphId;

pub const FUN_RENDER_LIGHTING_ENGINE_SCHEMA_VERSION: u16 = 1;
const FUN_RENDER_LIGHTING_WORK_GRAPH_ID: u64 = 0x4655_4e4c_4954_0001;

#[derive(Debug, Clone, Resource)]
pub struct FunRendererLightingEngineState {
    pub schema_version: u16,
    pub typed_light_count: u8,
    pub scheduler_replay_digest: Option<u64>,
    pub scheduler_visit_count: u16,
    pub ecs_lux_policy_before_renderer_frame_graph: bool,
    pub ecs_renderer_handoff_before_upload: bool,
}

pub fn setup_lighting(mut commands: Commands) {
    let schedule = run_lighting_scheduler_graph();
    let state = FunRendererLightingEngineState {
        schema_version: FUN_RENDER_LIGHTING_ENGINE_SCHEMA_VERSION,
        typed_light_count: 2,
        scheduler_replay_digest: schedule.map(|schedule| schedule.replay_digest),
        scheduler_visit_count: schedule.map_or(0, |schedule| schedule.visit_count),
        ecs_lux_policy_before_renderer_frame_graph: stage_precedes(
            EcsCrossDomainFrameStage::LuxChooseLightingPolicy,
            EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
        ),
        ecs_renderer_handoff_before_upload: stage_precedes(
            EcsCrossDomainFrameStage::RendererConsumeHandoffs,
            EcsCrossDomainFrameStage::RendererUploadArtifacts,
        ),
    };

    game_shared::fun_diag_info!(
        target: "fun::render::lighting",
        schema_version = state.schema_version,
        typed_light_count = state.typed_light_count,
        scheduler_replay_digest = state
            .scheduler_replay_digest
            .map(|digest| format!("{digest:016x}"))
            .unwrap_or_else(|| "unavailable".to_owned()),
        scheduler_visit_count = state.scheduler_visit_count,
        ecs_lux_policy_before_renderer_frame_graph =
            state.ecs_lux_policy_before_renderer_frame_graph,
        ecs_renderer_handoff_before_upload = state.ecs_renderer_handoff_before_upload,
        "fun-renderer lighting engine initialized"
    );
    commands.insert_resource(state);

    commands.spawn((
        Name::new("FunRendererSun"),
        renderer_api::DirectionalLight {
            color: renderer_api::RenderColor::linear_rgba(1.0, 0.95, 0.88, 1.0),
            illuminance_lux: 65_000.0,
            angular_radius_radians: 0.011,
        },
        renderer_api::ShadowCaster {
            mode: renderer_api::ShadowMode::VirtualPages,
        },
        renderer_api::LightLayer::default(),
        renderer_api::LightBounds::default(),
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        Name::new("FunRendererNearKey"),
        renderer_api::PointLight {
            color: renderer_api::RenderColor::linear_rgba(0.62, 0.72, 1.0, 1.0),
            intensity_lumens: 55_000.0,
            range: 32.0,
        },
        renderer_api::ShadowCaster {
            mode: renderer_api::ShadowMode::VirtualPages,
        },
        renderer_api::LightLayer::default(),
        renderer_api::LightBounds::default(),
        Transform::from_xyz(0.0, 6.0, 4.0),
    ));
}

#[derive(Debug, Clone, Copy)]
struct LightingSchedulerSummary {
    replay_digest: u64,
    visit_count: u16,
}

#[cfg(feature = "fun_renderer_core")]
fn run_lighting_scheduler_graph() -> Option<LightingSchedulerSummary> {
    let inputs = RendererGraphInputs {
        frame_index: 0,
        generation: 1,
        lux_passes: vec![
            LuxNodeKind::LuxUploadLightBuffers,
            LuxNodeKind::LuxClusterLights,
            LuxNodeKind::LuxShadowRequests,
            LuxNodeKind::LuxVirtualShadowPages,
            LuxNodeKind::LuxDirectLighting,
            LuxNodeKind::Tonemap,
            LuxNodeKind::FinalOutput,
        ],
        record_passes: vec![
            FrameGraphPassRole::LuxVirtualShadowFilter,
            FrameGraphPassRole::StaticScenePlaceholder,
        ],
        emit_diagnostics: true,
    };
    match run_renderer_work_graph_deterministic(
        WorkGraphId(FUN_RENDER_LIGHTING_WORK_GRAPH_ID),
        &inputs,
        Arc::new(MonotonicClock::new()),
    ) {
        Ok(report) => Some(LightingSchedulerSummary {
            replay_digest: report.replay_digest,
            visit_count: report.descriptor_visit_order.len().min(u16::MAX as usize) as u16,
        }),
        Err(error) => {
            let error_summary = format!("{error:?}");
            let _ = &error_summary;
            game_shared::fun_diag_warn!(
                target: "fun::render::lighting",
                error = %error_summary,
                "fun-renderer lighting scheduler graph rejected"
            );
            None
        }
    }
}

#[cfg(not(feature = "fun_renderer_core"))]
fn run_lighting_scheduler_graph() -> Option<LightingSchedulerSummary> {
    None
}

fn stage_precedes(before: EcsCrossDomainFrameStage, after: EcsCrossDomainFrameStage) -> bool {
    let before_index = ECS_CROSS_DOMAIN_FRAME_FLOW
        .iter()
        .position(|candidate| *candidate == before);
    let after_index = ECS_CROSS_DOMAIN_FRAME_FLOW
        .iter()
        .position(|candidate| *candidate == after);
    matches!((before_index, after_index), (Some(before), Some(after)) if before < after)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ecs_cross_domain_lighting_policy_precedes_renderer_frame_graph() {
        assert!(stage_precedes(
            EcsCrossDomainFrameStage::LuxChooseLightingPolicy,
            EcsCrossDomainFrameStage::RendererExecuteFrameGraph,
        ));
        assert!(stage_precedes(
            EcsCrossDomainFrameStage::RendererConsumeHandoffs,
            EcsCrossDomainFrameStage::RendererUploadArtifacts,
        ));
    }

    #[cfg(feature = "fun_renderer_core")]
    #[test]
    fn lighting_scheduler_graph_runs_fun_renderer_lux_path() {
        let schedule = run_lighting_scheduler_graph().expect("lighting graph should run");
        assert!(schedule.replay_digest != 0);
        assert!(schedule.visit_count >= 10);
    }

    #[test]
    fn fun_renderer_lights_default_to_virtual_shadow_pages() {
        let mode = renderer_api::ShadowCaster::default().mode;
        assert_eq!(mode, renderer_api::ShadowMode::VirtualPages);
    }
}
