use fun_window::backend::winit::{WinitSurfaceFrame, run_surface_present_utility};
use fun_window::{DpiScale, SurfaceId, WindowId, WindowSize};

use crate::surface::{
    RendererFrameOutcome, RendererPresentStatus, RendererSurfaceConfig, RendererSurfaceError,
    RendererSurfaceFrameReport, RendererSurfaceLifecycleState, RendererSurfaceService,
};

/// Test/example utility for proving a real WGPU surface clear and present.
///
/// This helper is intentionally outside product runtime modules. Engine and
/// client boot code must use renderer modules and scheduler-visible lifecycle
/// services instead of calling this smoke utility.
#[cfg(any(test, feature = "renderer-surface-smoke"))]
pub fn run_single_surface_present_smoke(
    spec: fun_window::WindowSpec,
    config: RendererSurfaceConfig,
) -> Result<RendererFrameOutcome, RendererSurfaceError> {
    let mut surface_result = Err(RendererSurfaceError::FrameNotRendered {
        frame_label: config.labels().frame,
    });
    run_surface_present_utility(spec, |frame| {
        surface_result = pollster::block_on(render_wgpu_surface_clear(frame, config.clone()));
        Ok(())
    })?;
    surface_result
}

async fn render_wgpu_surface_clear(
    frame: WinitSurfaceFrame<'_>,
    config: RendererSurfaceConfig,
) -> Result<RendererFrameOutcome, RendererSurfaceError> {
    let mut service = RendererSurfaceService::new();
    let handle = service
        .create_surface_from_window_handles(
            frame.window(),
            frame.handles(),
            frame.physical_size(),
            frame.dpi_scale(),
            config,
        )
        .await?;
    service.configure_surface(handle)?;
    let surface_frame = service.acquire_frame(handle)?;
    let recorded_frame = service.record_clear_frame(handle, &surface_frame)?;
    service.submit_frame(recorded_frame)?;
    let status = service.present_frame(surface_frame)?;
    let report = RendererSurfaceFrameReport::from_status(status);
    let outcome = RendererFrameOutcome::presented(report);
    let released = service.release_surface(handle)?;
    debug_assert_eq!(
        released.lifecycle_state(),
        RendererSurfaceLifecycleState::Released
    );
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_config_can_customize_clear_color() {
        let config =
            RendererSurfaceConfig::product_default().with_clear_color([0.05, 0.1, 0.2, 1.0]);

        assert_eq!(config.clear_color(), [0.05, 0.1, 0.2, 1.0]);
    }

    #[test]
    fn smoke_outcome_exposes_surface_identity_without_backend_objects() {
        let report = RendererSurfaceFrameReport::new(
            WindowId::PRIMARY,
            SurfaceId::PRIMARY,
            WindowSize::new(640, 360).expect("valid size"),
            DpiScale::ONE,
            RendererPresentStatus::Presented,
        );
        let outcome = RendererFrameOutcome::presented(report);

        assert_eq!(outcome.surface_frame(), Some(report));
        assert_eq!(outcome.present_status(), RendererPresentStatus::Presented);
        assert!(outcome.present_error().is_none());
        assert_eq!(report.window().get(), 1);
        assert_eq!(report.surface_id().get(), 1);
        assert_eq!(report.width(), 640);
    }
}
