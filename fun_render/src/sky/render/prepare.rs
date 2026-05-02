use bevy::{
    ecs::system::SystemParam,
    math::{Mat4, UVec2, UVec4, Vec2, Vec3, Vec4},
    prelude::*,
    render::{
        camera::ExtractedCamera,
        render_resource::{
            BindGroup, BindGroupEntries, Extent3d, PipelineCache, ShaderType, Texture,
            TextureDescriptor, TextureDimension, TextureUsages, TextureView, TextureViewDescriptor,
        },
        renderer::{RenderDevice, RenderQueue},
        view::ExtractedView,
    },
};
use tracing::debug;

use crate::sky::{
    config::{FunCloudDebugOverlay, FunCloudQuality, FunCloudSettings},
    plugin::{FunCloudHistoryState, FunCloudSceneLightState},
    weather::FunWeatherState,
};

use super::pipelines::{CLOUD_TEXTURE_FORMAT, FunCloudPipelineLayouts};

pub const DEFAULT_CLOUD_VIEW_SIZE: UVec2 = UVec2::new(1280, 720);
pub const CHEAP_WEATHER_MAP_SIZE: u32 = 256;
pub const BALANCED_WEATHER_MAP_SIZE: u32 = 512;
pub const CINEMATIC_WEATHER_MAP_SIZE: u32 = 1024;
pub const CHEAP_SHAPE_NOISE_SIZE: u32 = 32;
pub const DEFAULT_SHAPE_NOISE_SIZE: u32 = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct GpuCloudParams {
    pub view_size: UVec4,
    pub quality: UVec4,
    pub profile0: Vec4,
    pub profile1: Vec4,
    pub sky_zenith: Vec4,
    pub sky_horizon: Vec4,
    pub ambient: Vec4,
    pub wind: Vec4,
    pub sun: Vec4,
    pub history: UVec4,
    pub current_camera: Vec4,
    pub previous_camera: Vec4,
    pub wind_history: Vec4,
    pub current_world_from_clip: Mat4,
    pub previous_clip_from_world: Mat4,
}

impl GpuCloudParams {
    #[allow(
        clippy::too_many_arguments,
        reason = "GPU uniform construction mirrors the shader contract explicitly"
    )]
    pub fn new(
        settings: FunCloudSettings,
        state: FunWeatherState,
        history_state: FunCloudHistoryState,
        scene_light: FunCloudSceneLightState,
        internal_size: UVec2,
        weather_map_size: u32,
        shape_noise_size: u32,
        frame_index: u32,
        current_world_from_clip: Mat4,
        previous_clip_from_world: Mat4,
        current_camera_position: Vec3,
        previous_camera_position: Vec3,
        current_wind_offset: Vec2,
        previous_wind_offset: Vec2,
        history_age_frames: u32,
        history_reset_this_frame: bool,
    ) -> Self {
        let profile = state.profile;
        let seed_low = (state.weather_seed & 0xffff_ffff) as u32;
        let seed_high = (state.weather_seed >> 32) as u32;
        let sun_direction = profile
            .sun_direction_override
            .unwrap_or(scene_light.direction_to_light)
            .normalize_or_zero();
        let runtime_flags = u32::from(settings.temporal_enabled)
            | (debug_overlay_code(settings.debug_overlay) << 8)
            | (u32::from(settings.shadows_enabled) << 16);

        Self {
            view_size: UVec4::new(
                internal_size.x.max(1),
                internal_size.y.max(1),
                weather_map_size.max(1),
                shape_noise_size.max(1),
            ),
            quality: UVec4::new(
                settings.quality.primary_step_count(),
                settings.quality.light_step_count(),
                runtime_flags,
                frame_index,
            ),
            profile0: Vec4::new(
                profile.cloud_coverage,
                profile.cloud_density,
                profile.cloud_base_meters,
                profile.cloud_top_meters,
            ),
            profile1: Vec4::new(
                profile.weather_map_scale_meters,
                profile.storm_intensity,
                profile.turbulence_strength,
                profile.detail_strength,
            ),
            sky_zenith: Vec4::new(
                profile.sky_zenith_rgb[0],
                profile.sky_zenith_rgb[1],
                profile.sky_zenith_rgb[2],
                profile.sun_illuminance_lux,
            ),
            sky_horizon: Vec4::new(
                profile.sky_horizon_rgb[0],
                profile.sky_horizon_rgb[1],
                profile.sky_horizon_rgb[2],
                profile.anvil_strength,
            ),
            ambient: Vec4::new(
                profile.ambient_rgb[0],
                profile.ambient_rgb[1],
                profile.ambient_rgb[2],
                profile.edge_softness,
            ),
            wind: Vec4::new(
                profile.wind_direction.x,
                profile.wind_direction.z,
                profile.wind_speed_mps + profile.wind_shear_mps * 0.25,
                (seed_low ^ seed_high) as f32 / u32::MAX as f32,
            ),
            sun: Vec4::new(
                sun_direction.x,
                sun_direction.y,
                sun_direction.z,
                scene_light.illuminance_lux,
            ),
            history: UVec4::new(
                history_state.generation(),
                history_state.reset_count().min(u64::from(u32::MAX)) as u32,
                u32::from(history_reset_this_frame),
                history_age_frames,
            ),
            current_camera: Vec4::new(
                current_camera_position.x,
                current_camera_position.y,
                current_camera_position.z,
                1.0,
            ),
            previous_camera: Vec4::new(
                previous_camera_position.x,
                previous_camera_position.y,
                previous_camera_position.z,
                1.0,
            ),
            wind_history: Vec4::new(
                current_wind_offset.x,
                current_wind_offset.y,
                previous_wind_offset.x,
                previous_wind_offset.y,
            ),
            current_world_from_clip,
            previous_clip_from_world,
        }
    }
}

const fn debug_overlay_code(overlay: FunCloudDebugOverlay) -> u32 {
    match overlay {
        FunCloudDebugOverlay::None => 0,
        FunCloudDebugOverlay::Coverage => 1,
        FunCloudDebugOverlay::Density => 2,
        FunCloudDebugOverlay::Steps => 3,
        FunCloudDebugOverlay::History => 4,
        FunCloudDebugOverlay::Weather => 5,
    }
}

#[derive(Resource, Default)]
pub struct FunCloudGpuTextures {
    pub allocation: Option<FunCloudTextureAllocation>,
    frame_index: u32,
}

impl FunCloudGpuTextures {
    pub fn allocation(&self) -> Option<&FunCloudTextureAllocation> {
        self.allocation.as_ref()
    }
}

pub struct FunCloudTextureAllocation {
    pub key: FunCloudTextureKey,
    _weather_map: Texture,
    _shape_noise: Texture,
    _cloud_color: Texture,
    _cloud_transmittance: Texture,
    _history_color_a: Texture,
    _history_color_b: Texture,
    _debug: Texture,
    pub weather_map_view: TextureView,
    pub shape_noise_view: TextureView,
    pub cloud_color_view: TextureView,
    pub cloud_transmittance_view: TextureView,
    pub history_color_a_view: TextureView,
    pub history_color_b_view: TextureView,
    pub debug_view: TextureView,
    pub params: UniformCloudParams,
    pub generation_bind_group: BindGroup,
    pub raymarch_bind_group: BindGroup,
    pub temporal_a_to_b_bind_group: BindGroup,
    pub temporal_b_to_a_bind_group: BindGroup,
    pub composite_a_bind_group: BindGroup,
    pub composite_b_bind_group: BindGroup,
    pub shape_noise_generated: bool,
    history_generation: u32,
    history_age_frames: u32,
    previous_clip_from_world: Mat4,
    previous_camera_position: Vec3,
    previous_wind_offset: Vec2,
}

pub type UniformCloudParams = bevy::render::render_resource::UniformBuffer<GpuCloudParams>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunCloudTextureKey {
    pub internal_size: UVec2,
    pub weather_map_size: u32,
    pub shape_noise_size: u32,
    pub debug_overlay: FunCloudDebugOverlay,
}

impl FunCloudTextureKey {
    pub fn vram_bytes(self) -> u64 {
        let lowres_texels = u64::from(self.internal_size.x) * u64::from(self.internal_size.y);
        let weather_texels = u64::from(self.weather_map_size) * u64::from(self.weather_map_size);
        let noise_texels = u64::from(self.shape_noise_size)
            * u64::from(self.shape_noise_size)
            * u64::from(self.shape_noise_size);
        let rgba16_bytes = 8;

        (lowres_texels * 5 + weather_texels + noise_texels) * rgba16_bytes
    }
}

#[derive(SystemParam)]
pub struct PrepareCloudTexturesParams<'w, 's> {
    cameras: Query<'w, 's, (&'static ExtractedCamera, &'static ExtractedView)>,
    settings: Option<Res<'w, FunCloudSettings>>,
    weather_state: Option<Res<'w, FunWeatherState>>,
    history_state: Option<Res<'w, FunCloudHistoryState>>,
    scene_light: Option<Res<'w, FunCloudSceneLightState>>,
    layouts: Option<Res<'w, FunCloudPipelineLayouts>>,
    pipeline_cache: Res<'w, PipelineCache>,
    render_device: Res<'w, RenderDevice>,
    render_queue: Res<'w, RenderQueue>,
}

pub fn prepare_cloud_textures(
    params: PrepareCloudTexturesParams,
    mut textures: ResMut<FunCloudGpuTextures>,
) {
    let (Some(settings), Some(weather_state), Some(layouts)) = (
        params.settings.as_deref(),
        params.weather_state.as_deref(),
        params.layouts.as_deref(),
    ) else {
        textures.allocation = None;
        return;
    };

    if !settings.enabled || settings.quality == FunCloudQuality::Off {
        textures.allocation = None;
        return;
    }

    let view_sample = params
        .cameras
        .iter()
        .find_map(|(camera, view)| camera.physical_viewport_size.map(|_| view));
    let view_size = params
        .cameras
        .iter()
        .find_map(|(camera, _)| camera.physical_viewport_size)
        .unwrap_or(DEFAULT_CLOUD_VIEW_SIZE);
    let (current_world_from_clip, current_clip_from_world, current_camera_position) = view_sample
        .map(cloud_camera_matrices)
        .unwrap_or((Mat4::IDENTITY, Mat4::IDENTITY, Vec3::ZERO));
    let internal_size = settings.internal_scale.scale_size(view_size);
    let key = FunCloudTextureKey {
        internal_size,
        weather_map_size: weather_map_size(settings.quality),
        shape_noise_size: shape_noise_size(settings.quality),
        debug_overlay: settings.debug_overlay,
    };

    let needs_reallocation = textures
        .allocation
        .as_ref()
        .map(|allocation| allocation.key != key)
        .unwrap_or(true);

    if needs_reallocation {
        textures.allocation = Some(create_cloud_texture_allocation(
            key,
            layouts,
            &params.pipeline_cache,
            &params.render_device,
            &params.render_queue,
        ));
        debug!(
            target: "fun::render::clouds::vram",
            cloud_internal_width = key.internal_size.x,
            cloud_internal_height = key.internal_size.y,
            cloud_weather_map_size = key.weather_map_size,
            cloud_shape_noise_size = key.shape_noise_size,
            cloud_vram_bytes = key.vram_bytes(),
            "allocated cloud render textures"
        );
    }

    textures.frame_index = textures.frame_index.wrapping_add(1);
    let frame_index = textures.frame_index;
    if let Some(allocation) = textures.allocation.as_mut() {
        let history_state = params.history_state.as_deref().copied().unwrap_or_default();
        let scene_light = params.scene_light.as_deref().copied().unwrap_or_default();
        let current_wind_offset = cloud_wind_offset(*weather_state, frame_index);
        let history_reset_this_frame = allocation.history_generation != history_state.generation();
        if history_reset_this_frame {
            allocation.history_generation = history_state.generation();
            allocation.history_age_frames = 0;
            allocation.previous_clip_from_world = current_clip_from_world;
            allocation.previous_camera_position = current_camera_position;
            allocation.previous_wind_offset = current_wind_offset;
        }
        let previous_clip_from_world = allocation.previous_clip_from_world;
        let previous_camera_position = allocation.previous_camera_position;
        let previous_wind_offset = allocation.previous_wind_offset;
        let history_age_frames = allocation.history_age_frames;
        allocation.params.set(GpuCloudParams::new(
            *settings,
            *weather_state,
            history_state,
            scene_light,
            key.internal_size,
            key.weather_map_size,
            key.shape_noise_size,
            frame_index,
            current_world_from_clip,
            previous_clip_from_world,
            current_camera_position,
            previous_camera_position,
            current_wind_offset,
            previous_wind_offset,
            history_age_frames,
            history_reset_this_frame,
        ));
        allocation
            .params
            .write_buffer(&params.render_device, &params.render_queue);
        allocation.previous_clip_from_world = current_clip_from_world;
        allocation.previous_camera_position = current_camera_position;
        allocation.previous_wind_offset = current_wind_offset;
        allocation.history_age_frames = if history_reset_this_frame {
            0
        } else {
            allocation.history_age_frames.saturating_add(1)
        };
    }
}

fn cloud_camera_matrices(view: &ExtractedView) -> (Mat4, Mat4, Vec3) {
    let world_from_view = view.world_from_view.to_matrix();
    let view_from_world = world_from_view.inverse();
    let clip_from_world = view
        .clip_from_world
        .unwrap_or_else(|| view.clip_from_view * view_from_world);
    (
        clip_from_world.inverse(),
        clip_from_world,
        view.world_from_view.translation(),
    )
}

fn cloud_wind_offset(state: FunWeatherState, frame_index: u32) -> Vec2 {
    let profile = state.profile;
    Vec2::new(profile.wind_direction.x, profile.wind_direction.z)
        * (profile.wind_speed_mps + profile.wind_shear_mps * 0.25)
        * frame_index as f32
        * 0.000_015
}

fn create_cloud_texture_allocation(
    key: FunCloudTextureKey,
    layouts: &FunCloudPipelineLayouts,
    pipeline_cache: &PipelineCache,
    render_device: &RenderDevice,
    render_queue: &RenderQueue,
) -> FunCloudTextureAllocation {
    let weather_map = create_texture_2d(
        render_device,
        "fun_cloud_weather_map",
        key.weather_map_size,
        key.weather_map_size,
    );
    let shape_noise = create_texture_3d(
        render_device,
        "fun_cloud_shape_noise",
        key.shape_noise_size,
        key.shape_noise_size,
        key.shape_noise_size,
    );
    let cloud_color = create_texture_2d(
        render_device,
        "fun_cloud_color_lowres",
        key.internal_size.x,
        key.internal_size.y,
    );
    let cloud_transmittance = create_texture_2d(
        render_device,
        "fun_cloud_transmittance_lowres",
        key.internal_size.x,
        key.internal_size.y,
    );
    let history_color_a = create_texture_2d(
        render_device,
        "fun_cloud_history_color_a",
        key.internal_size.x,
        key.internal_size.y,
    );
    let history_color_b = create_texture_2d(
        render_device,
        "fun_cloud_history_color_b",
        key.internal_size.x,
        key.internal_size.y,
    );
    let debug = create_texture_2d(
        render_device,
        "fun_cloud_debug",
        key.internal_size.x,
        key.internal_size.y,
    );

    let weather_map_view = weather_map.create_view(&TextureViewDescriptor::default());
    let shape_noise_view = shape_noise.create_view(&TextureViewDescriptor::default());
    let cloud_color_view = cloud_color.create_view(&TextureViewDescriptor::default());
    let cloud_transmittance_view =
        cloud_transmittance.create_view(&TextureViewDescriptor::default());
    let history_color_a_view = history_color_a.create_view(&TextureViewDescriptor::default());
    let history_color_b_view = history_color_b.create_view(&TextureViewDescriptor::default());
    let debug_view = debug.create_view(&TextureViewDescriptor::default());

    let mut params = UniformCloudParams::default();
    params.set_label(Some("fun_cloud_runtime_params"));
    params.write_buffer(render_device, render_queue);

    let generation_bind_group = render_device.create_bind_group(
        "fun_cloud_generation_bind_group",
        &pipeline_cache.get_bind_group_layout(&layouts.generation),
        &BindGroupEntries::sequential((
            params.binding().expect("cloud params buffer should exist"),
            &weather_map_view,
            &shape_noise_view,
        )),
    );
    let raymarch_bind_group = render_device.create_bind_group(
        "fun_cloud_raymarch_bind_group",
        &pipeline_cache.get_bind_group_layout(&layouts.raymarch),
        &BindGroupEntries::sequential((
            params.binding().expect("cloud params buffer should exist"),
            &weather_map_view,
            &shape_noise_view,
            &cloud_color_view,
            &cloud_transmittance_view,
        )),
    );
    let temporal_a_to_b_bind_group = render_device.create_bind_group(
        "fun_cloud_temporal_a_to_b_bind_group",
        &pipeline_cache.get_bind_group_layout(&layouts.temporal),
        &BindGroupEntries::sequential((
            params.binding().expect("cloud params buffer should exist"),
            &cloud_color_view,
            &history_color_a_view,
            &history_color_b_view,
            &debug_view,
        )),
    );
    let temporal_b_to_a_bind_group = render_device.create_bind_group(
        "fun_cloud_temporal_b_to_a_bind_group",
        &pipeline_cache.get_bind_group_layout(&layouts.temporal),
        &BindGroupEntries::sequential((
            params.binding().expect("cloud params buffer should exist"),
            &cloud_color_view,
            &history_color_b_view,
            &history_color_a_view,
            &debug_view,
        )),
    );
    let composite_a_bind_group = render_device.create_bind_group(
        "fun_cloud_composite_a_bind_group",
        &pipeline_cache.get_bind_group_layout(&layouts.composite),
        &BindGroupEntries::sequential((
            params.binding().expect("cloud params buffer should exist"),
            &history_color_a_view,
            &cloud_transmittance_view,
            &debug_view,
        )),
    );
    let composite_b_bind_group = render_device.create_bind_group(
        "fun_cloud_composite_b_bind_group",
        &pipeline_cache.get_bind_group_layout(&layouts.composite),
        &BindGroupEntries::sequential((
            params.binding().expect("cloud params buffer should exist"),
            &history_color_b_view,
            &cloud_transmittance_view,
            &debug_view,
        )),
    );

    FunCloudTextureAllocation {
        key,
        _weather_map: weather_map,
        _shape_noise: shape_noise,
        _cloud_color: cloud_color,
        _cloud_transmittance: cloud_transmittance,
        _history_color_a: history_color_a,
        _history_color_b: history_color_b,
        _debug: debug,
        weather_map_view,
        shape_noise_view,
        cloud_color_view,
        cloud_transmittance_view,
        history_color_a_view,
        history_color_b_view,
        debug_view,
        params,
        generation_bind_group,
        raymarch_bind_group,
        temporal_a_to_b_bind_group,
        temporal_b_to_a_bind_group,
        composite_a_bind_group,
        composite_b_bind_group,
        shape_noise_generated: false,
        history_generation: 0,
        history_age_frames: 0,
        previous_clip_from_world: Mat4::IDENTITY,
        previous_camera_position: Vec3::ZERO,
        previous_wind_offset: Vec2::ZERO,
    }
}

fn create_texture_2d(
    render_device: &RenderDevice,
    label: &'static str,
    width: u32,
    height: u32,
) -> Texture {
    render_device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: CLOUD_TEXTURE_FORMAT,
        usage: cloud_texture_usage(),
        view_formats: &[],
    })
}

fn create_texture_3d(
    render_device: &RenderDevice,
    label: &'static str,
    width: u32,
    height: u32,
    depth: u32,
) -> Texture {
    render_device.create_texture(&TextureDescriptor {
        label: Some(label),
        size: Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: depth.max(1),
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D3,
        format: CLOUD_TEXTURE_FORMAT,
        usage: cloud_texture_usage(),
        view_formats: &[],
    })
}

fn cloud_texture_usage() -> TextureUsages {
    TextureUsages::TEXTURE_BINDING.union(TextureUsages::STORAGE_BINDING)
}

const fn weather_map_size(quality: FunCloudQuality) -> u32 {
    match quality {
        FunCloudQuality::Off => 1,
        FunCloudQuality::Cheap => CHEAP_WEATHER_MAP_SIZE,
        FunCloudQuality::Balanced => BALANCED_WEATHER_MAP_SIZE,
        FunCloudQuality::Cinematic => CINEMATIC_WEATHER_MAP_SIZE,
    }
}

const fn shape_noise_size(quality: FunCloudQuality) -> u32 {
    match quality {
        FunCloudQuality::Off => 1,
        FunCloudQuality::Cheap => CHEAP_SHAPE_NOISE_SIZE,
        FunCloudQuality::Balanced | FunCloudQuality::Cinematic => DEFAULT_SHAPE_NOISE_SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sky::weather::FunWeatherProfileId;

    #[test]
    fn cloud_texture_key_vram_counts_all_persistent_textures() {
        let key = FunCloudTextureKey {
            internal_size: UVec2::new(640, 360),
            weather_map_size: 512,
            shape_noise_size: 64,
            debug_overlay: FunCloudDebugOverlay::None,
        };

        assert_eq!(key.vram_bytes(), 13_410_304);
    }

    #[test]
    fn cloud_params_encode_quality_weather_and_seed_without_strings() {
        let settings = FunCloudSettings {
            enabled: true,
            quality: FunCloudQuality::Balanced,
            internal_scale: crate::sky::config::FunCloudInternalScale::Half,
            temporal_enabled: true,
            shadows_enabled: false,
            profile_id: FunWeatherProfileId::StormFront,
            debug_overlay: FunCloudDebugOverlay::Coverage,
        };
        let state = FunWeatherState::from_profile_id(FunWeatherProfileId::StormFront).unwrap();

        let params = GpuCloudParams::new(
            settings,
            state,
            FunCloudHistoryState::default(),
            FunCloudSceneLightState::default(),
            UVec2::new(960, 540),
            512,
            64,
            17,
            Mat4::IDENTITY,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec2::ZERO,
            Vec2::ZERO,
            3,
            true,
        );

        assert_eq!(params.view_size, UVec4::new(960, 540, 512, 64));
        assert_eq!(params.quality.x, 20);
        assert_eq!(params.quality.y, 4);
        assert_eq!(params.quality.z & 1, 1);
        assert_eq!((params.quality.z >> 8) & 0xff, 1);
        assert_eq!(params.quality.w, 17);
        assert_eq!(params.history.x, 1);
        assert_eq!(params.history.z, 1);
        assert_eq!(params.history.w, 3);
        assert!(params.profile0.x > 0.0);
        assert!(params.profile1.y > 0.0);
    }
}
