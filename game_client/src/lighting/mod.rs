mod render;

use bevy::{
    camera::CameraMainTextureUsages,
    core_pipeline::prepass::DeferredPrepass,
    pbr::DefaultOpaqueRendererMethod,
    prelude::*,
    reflect::Reflect,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::{ShaderType, TextureUsages},
        view::Msaa,
    },
};

pub struct LightingPipelinePlugin;

impl Plugin for LightingPipelinePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DefaultOpaqueRendererMethod::deferred())
            .register_type::<LightingBackend>()
            .register_type::<LightingPipelineSettings>()
            .register_type::<LightingPipelineView>()
            .init_resource::<LightingPipelineSettings>()
            .add_plugins((
                ExtractResourcePlugin::<LightingPipelineSettings>::default(),
                ExtractComponentPlugin::<LightingPipelineView>::default(),
                UniformComponentPlugin::<LightingPipelineView>::default(),
                render::LightingPipelineRenderPlugin,
            ))
            .add_systems(PostUpdate, apply_lighting_camera_requirements);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Default)]
#[reflect(Default)]
#[repr(u32)]
pub enum LightingBackend {
    #[default]
    Shiny = 0,
    Reference = 1,
}

impl LightingBackend {
    const fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::Reference,
            _ => Self::Shiny,
        }
    }

    const fn as_raw(self) -> u32 {
        self as u32
    }

    const fn requires_deferred_prepass(self) -> bool {
        matches!(self, Self::Shiny | Self::Reference)
    }
}

#[derive(Resource, Debug, Clone, Reflect, ExtractResource)]
#[reflect(Resource, Default)]
pub struct LightingPipelineSettings {
    pub default_backend: LightingBackend,
    pub enable_debug_overlay: bool,
}

impl Default for LightingPipelineSettings {
    fn default() -> Self {
        Self {
            default_backend: LightingBackend::Shiny,
            enable_debug_overlay: false,
        }
    }
}

#[derive(Component, Debug, Clone, Copy, Reflect, ExtractComponent, ShaderType)]
#[reflect(Component, Default)]
pub struct LightingPipelineView {
    backend_override: u32,
    flags: u32,
    exposure_bias: f32,
    reserved: u32,
}

impl LightingPipelineView {
    const INHERIT_BACKEND: u32 = u32::MAX;
    const FORCE_STORAGE_BINDING: u32 = 1 << 0;

    pub const fn inherit() -> Self {
        Self {
            backend_override: Self::INHERIT_BACKEND,
            flags: 0,
            exposure_bias: 0.0,
            reserved: 0,
        }
    }

    pub const fn shiny() -> Self {
        Self::with_backend(LightingBackend::Shiny)
    }

    pub const fn reference() -> Self {
        Self::with_backend(LightingBackend::Reference)
    }

    pub const fn with_backend(backend: LightingBackend) -> Self {
        Self {
            backend_override: backend.as_raw(),
            ..Self::inherit()
        }
    }

    pub const fn with_exposure_bias(mut self, exposure_bias: f32) -> Self {
        self.exposure_bias = exposure_bias;
        self
    }

    pub const fn enable_storage_binding(mut self) -> Self {
        self.flags |= Self::FORCE_STORAGE_BINDING;
        self
    }

    pub const fn effective_backend(self, settings: &LightingPipelineSettings) -> LightingBackend {
        if self.backend_override == Self::INHERIT_BACKEND {
            settings.default_backend
        } else {
            LightingBackend::from_raw(self.backend_override)
        }
    }

    pub const fn flags(self) -> u32 {
        self.flags
    }
}

impl Default for LightingPipelineView {
    fn default() -> Self {
        Self::inherit()
    }
}

fn apply_lighting_camera_requirements(
    settings: Res<LightingPipelineSettings>,
    mut commands: Commands,
    cameras: Query<(
        Entity,
        &LightingPipelineView,
        Option<&Msaa>,
        Option<&DeferredPrepass>,
        Option<&CameraMainTextureUsages>,
    )>,
) {
    for (entity, view, msaa, deferred_prepass, usages) in &cameras {
        let backend = view.effective_backend(&settings);
        let needs_deferred_prepass = backend.requires_deferred_prepass();
        let needs_storage_binding =
            (view.flags() & LightingPipelineView::FORCE_STORAGE_BINDING) != 0;

        if needs_deferred_prepass && deferred_prepass.is_none() {
            commands.entity(entity).insert(DeferredPrepass);
        }

        if !needs_deferred_prepass && deferred_prepass.is_some() {
            commands.entity(entity).remove::<DeferredPrepass>();
        }

        if needs_deferred_prepass && msaa != Some(&Msaa::Off) {
            commands.entity(entity).insert(Msaa::Off);
        }

        if needs_storage_binding
            && !usages
                .map(|usages| usages.0.contains(TextureUsages::STORAGE_BINDING))
                .unwrap_or(false)
        {
            commands
                .entity(entity)
                .insert(CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING));
        }
    }
}
