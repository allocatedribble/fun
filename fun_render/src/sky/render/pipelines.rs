use bevy::{
    asset::{AssetServer, embedded_asset, load_embedded_asset},
    prelude::*,
    render::render_resource::{
        BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedComputePipelineId,
        ComputePipelineDescriptor, PipelineCache, ShaderStages, StorageTextureAccess,
        TextureFormat, TextureSampleType,
        binding_types::{
            texture_2d, texture_3d, texture_storage_2d, texture_storage_3d, uniform_buffer,
        },
    },
    shader::load_shader_library,
};

use super::prepare::GpuCloudParams;

pub const CLOUD_TEXTURE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub fn load_cloud_shader_assets(app: &mut App) {
    load_shader_library!(app, "sky_common.wgsl");
    embedded_asset!(app, "cloud_density.wgsl");
    embedded_asset!(app, "cloud_raymarch.wgsl");
    embedded_asset!(app, "cloud_temporal.wgsl");
    embedded_asset!(app, "cloud_composite.wgsl");
    embedded_asset!(app, "cloud_debug.wgsl");
}

#[derive(Resource)]
pub struct FunCloudPipelineLayouts {
    pub generation: BindGroupLayoutDescriptor,
    pub raymarch: BindGroupLayoutDescriptor,
    pub temporal: BindGroupLayoutDescriptor,
    pub composite: BindGroupLayoutDescriptor,
}

#[derive(Resource)]
pub struct FunCloudPipelines {
    pub weather_map: CachedComputePipelineId,
    pub shape_noise: CachedComputePipelineId,
    pub raymarch: CachedComputePipelineId,
    pub temporal: CachedComputePipelineId,
    pub composite: CachedComputePipelineId,
    pub debug: CachedComputePipelineId,
}

pub fn init_cloud_pipelines(
    mut commands: Commands,
    pipeline_cache: Res<PipelineCache>,
    asset_server: Res<AssetServer>,
) {
    let generation = BindGroupLayoutDescriptor::new(
        "fun_cloud_generation_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<GpuCloudParams>(false),
                texture_storage_2d(CLOUD_TEXTURE_FORMAT, StorageTextureAccess::WriteOnly),
                texture_storage_3d(CLOUD_TEXTURE_FORMAT, StorageTextureAccess::WriteOnly),
            ),
        ),
    );

    let float_texture = TextureSampleType::Float { filterable: false };
    let raymarch = BindGroupLayoutDescriptor::new(
        "fun_cloud_raymarch_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<GpuCloudParams>(false),
                texture_2d(float_texture),
                texture_3d(float_texture),
                texture_storage_2d(CLOUD_TEXTURE_FORMAT, StorageTextureAccess::WriteOnly),
                texture_storage_2d(CLOUD_TEXTURE_FORMAT, StorageTextureAccess::WriteOnly),
            ),
        ),
    );

    let temporal = BindGroupLayoutDescriptor::new(
        "fun_cloud_temporal_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<GpuCloudParams>(false),
                texture_2d(float_texture),
                texture_2d(float_texture),
                texture_storage_2d(CLOUD_TEXTURE_FORMAT, StorageTextureAccess::WriteOnly),
                texture_storage_2d(CLOUD_TEXTURE_FORMAT, StorageTextureAccess::WriteOnly),
            ),
        ),
    );

    let composite = BindGroupLayoutDescriptor::new(
        "fun_cloud_composite_bind_group_layout",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::COMPUTE,
            (
                uniform_buffer::<GpuCloudParams>(false),
                texture_2d(float_texture),
                texture_2d(float_texture),
                texture_storage_2d(CLOUD_TEXTURE_FORMAT, StorageTextureAccess::WriteOnly),
            ),
        ),
    );

    let density_shader = load_embedded_asset!(asset_server.as_ref(), "cloud_density.wgsl");
    let raymarch_shader = load_embedded_asset!(asset_server.as_ref(), "cloud_raymarch.wgsl");
    let temporal_shader = load_embedded_asset!(asset_server.as_ref(), "cloud_temporal.wgsl");
    let composite_shader = load_embedded_asset!(asset_server.as_ref(), "cloud_composite.wgsl");
    let debug_shader = load_embedded_asset!(asset_server.as_ref(), "cloud_debug.wgsl");

    let pipelines = FunCloudPipelines {
        weather_map: pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("fun_cloud_weather_map_pipeline".into()),
            layout: vec![generation.clone()],
            shader: density_shader.clone(),
            entry_point: Some("generate_weather_map".into()),
            zero_initialize_workgroup_memory: true,
            ..default()
        }),
        shape_noise: pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("fun_cloud_shape_noise_pipeline".into()),
            layout: vec![generation.clone()],
            shader: density_shader,
            entry_point: Some("generate_shape_noise".into()),
            zero_initialize_workgroup_memory: true,
            ..default()
        }),
        raymarch: pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("fun_cloud_raymarch_pipeline".into()),
            layout: vec![raymarch.clone()],
            shader: raymarch_shader,
            entry_point: Some("raymarch_cloud_layer".into()),
            zero_initialize_workgroup_memory: true,
            ..default()
        }),
        temporal: pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("fun_cloud_temporal_pipeline".into()),
            layout: vec![temporal.clone()],
            shader: temporal_shader,
            entry_point: Some("resolve_cloud_history".into()),
            zero_initialize_workgroup_memory: true,
            ..default()
        }),
        composite: pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("fun_cloud_composite_pipeline".into()),
            layout: vec![composite.clone()],
            shader: composite_shader,
            entry_point: Some("composite_clouds".into()),
            zero_initialize_workgroup_memory: true,
            ..default()
        }),
        debug: pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("fun_cloud_debug_pipeline".into()),
            layout: vec![composite.clone()],
            shader: debug_shader,
            entry_point: Some("write_cloud_debug_overlay".into()),
            zero_initialize_workgroup_memory: true,
            ..default()
        }),
    };

    commands.insert_resource(FunCloudPipelineLayouts {
        generation,
        raymarch,
        temporal,
        composite,
    });
    commands.insert_resource(pipelines);
}
