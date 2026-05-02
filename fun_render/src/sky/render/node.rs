use bevy::{
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_resource::{ComputePass, ComputePassDescriptor, PipelineCache},
        renderer::{RenderContext, ViewQuery},
    },
};

use crate::sky::{
    config::{FunCloudDebugOverlay, FunCloudQuality, FunCloudSettings},
    render::{pipelines::FunCloudPipelines, prepare::FunCloudGpuTextures},
};

pub fn run_cloud_compute_passes(
    view: ViewQuery<&ExtractedCamera>,
    settings: Option<Res<FunCloudSettings>>,
    pipelines: Option<Res<FunCloudPipelines>>,
    pipeline_cache: Res<PipelineCache>,
    mut textures: ResMut<FunCloudGpuTextures>,
    mut ctx: RenderContext,
) {
    let (Some(settings), Some(pipelines)) = (settings.as_deref(), pipelines.as_deref()) else {
        return;
    };
    if !settings.enabled || settings.quality == FunCloudQuality::Off {
        return;
    }

    let camera = view.into_inner();
    let Some(viewport) = camera.physical_viewport_size else {
        return;
    };
    if viewport.x == 0 || viewport.y == 0 {
        return;
    }

    let Some(allocation) = textures.allocation.as_mut() else {
        return;
    };

    let (
        Some(weather_map_pipeline),
        Some(raymarch_pipeline),
        Some(temporal_pipeline),
        Some(composite_pipeline),
    ) = (
        pipeline_cache.get_compute_pipeline(pipelines.weather_map),
        pipeline_cache.get_compute_pipeline(pipelines.raymarch),
        pipeline_cache.get_compute_pipeline(pipelines.temporal),
        pipeline_cache.get_compute_pipeline(
            if settings.debug_overlay == FunCloudDebugOverlay::None {
                pipelines.composite
            } else {
                pipelines.debug
            },
        ),
    )
    else {
        return;
    };

    let shape_noise_pipeline = if allocation.shape_noise_generated {
        None
    } else {
        let Some(shape_noise_pipeline) = pipeline_cache.get_compute_pipeline(pipelines.shape_noise)
        else {
            return;
        };
        Some(shape_noise_pipeline)
    };

    let history_even = allocation.params.get().quality.w % 2 == 0;
    let temporal_bind_group = if history_even {
        &allocation.temporal_a_to_b_bind_group
    } else {
        &allocation.temporal_b_to_a_bind_group
    };
    let composite_bind_group = if history_even {
        &allocation.composite_b_bind_group
    } else {
        &allocation.composite_a_bind_group
    };

    let command_encoder = ctx.command_encoder();
    let mut pass = command_encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some("fun_cloud_compute"),
        timestamp_writes: None,
    });

    pass.set_pipeline(weather_map_pipeline);
    pass.set_bind_group(0, &allocation.generation_bind_group, &[]);
    dispatch_2d(
        &mut pass,
        allocation.key.weather_map_size,
        allocation.key.weather_map_size,
        8,
    );

    if let Some(shape_noise_pipeline) = shape_noise_pipeline {
        pass.set_pipeline(shape_noise_pipeline);
        pass.set_bind_group(0, &allocation.generation_bind_group, &[]);
        dispatch_3d(
            &mut pass,
            allocation.key.shape_noise_size,
            allocation.key.shape_noise_size,
            allocation.key.shape_noise_size,
            4,
        );
        allocation.shape_noise_generated = true;
    }

    pass.set_pipeline(raymarch_pipeline);
    pass.set_bind_group(0, &allocation.raymarch_bind_group, &[]);
    dispatch_2d(
        &mut pass,
        allocation.key.internal_size.x,
        allocation.key.internal_size.y,
        8,
    );

    pass.set_pipeline(temporal_pipeline);
    pass.set_bind_group(0, temporal_bind_group, &[]);
    dispatch_2d(
        &mut pass,
        allocation.key.internal_size.x,
        allocation.key.internal_size.y,
        8,
    );

    pass.set_pipeline(composite_pipeline);
    pass.set_bind_group(0, composite_bind_group, &[]);
    dispatch_2d(
        &mut pass,
        allocation.key.internal_size.x,
        allocation.key.internal_size.y,
        8,
    );
}

fn dispatch_2d(pass: &mut ComputePass<'_>, width: u32, height: u32, workgroup_size: u32) {
    pass.dispatch_workgroups(
        width.div_ceil(workgroup_size),
        height.div_ceil(workgroup_size),
        1,
    );
}

fn dispatch_3d(
    pass: &mut ComputePass<'_>,
    width: u32,
    height: u32,
    depth: u32,
    workgroup_size: u32,
) {
    pass.dispatch_workgroups(
        width.div_ceil(workgroup_size),
        height.div_ceil(workgroup_size),
        depth.div_ceil(workgroup_size),
    );
}
