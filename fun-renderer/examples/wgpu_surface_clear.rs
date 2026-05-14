use fun_renderer::surface::{
    RendererSurfaceConfig, RendererSurfaceError, RendererSurfaceLifecycleState,
    RendererSurfaceService,
};
use fun_window::backend::winit::{WinitSurfaceFrame, run_surface_present_utility};
use fun_window::{WindowSize, WindowSpec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec = WindowSpec::default()
        .with_title("FUN Renderer Surface Clear")
        .with_logical_size(WindowSize::DEFAULT)
        .with_visible(true);
    let config = RendererSurfaceConfig::product_default()
        .with_frame_label("fun_renderer.example.wgpu_surface_clear.frame")
        .with_clear_label("fun_renderer.example.wgpu_surface_clear.clear")
        .with_clear_color([0.08, 0.12, 0.18, 1.0]);
    let mut result = Err(RendererSurfaceError::FrameNotRendered {
        frame_label: config.labels().frame,
    });
    run_surface_present_utility(spec, |frame| {
        result = pollster::block_on(clear_surface(frame, config.clone()));
        Ok(())
    })?;
    result?;
    Ok(())
}

async fn clear_surface(
    frame: WinitSurfaceFrame<'_>,
    config: RendererSurfaceConfig,
) -> Result<(), RendererSurfaceError> {
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
    service.present_frame(surface_frame)?;
    let released = service.release_surface(handle)?;
    debug_assert_eq!(
        released.lifecycle_state(),
        RendererSurfaceLifecycleState::Released
    );
    Ok(())
}
