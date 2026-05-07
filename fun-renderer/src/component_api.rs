use bevy_ecs::prelude::Component;

pub const RENDERER_COMPONENT_API_SCHEMA_VERSION: u16 = 1;
pub const PUBLIC_RENDER_COMPONENT_COUNT: usize = 43;
pub const PUBLIC_RENDER_ASSET_COUNT: usize = 7;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderStableId(pub u64);

impl RenderStableId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderAssetId {
    pub slot: u32,
    pub generation: u32,
}

impl RenderAssetId {
    pub const INVALID: Self = Self {
        slot: u32::MAX,
        generation: 0,
    };

    #[must_use]
    pub const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    #[must_use]
    pub const fn first(slot: u32) -> Self {
        Self {
            slot,
            generation: 1,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.slot != u32::MAX && self.generation != 0
    }
}

pub type RenderMeshAssetId = RenderAssetId;
pub type RenderMaterialAssetId = RenderAssetId;
pub type RenderTextureAssetId = RenderAssetId;
pub type RenderSamplerAssetId = RenderAssetId;
pub type RenderShaderAssetId = RenderAssetId;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RenderVec2 {
    pub x: f32,
    pub y: f32,
}

impl RenderVec2 {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RenderVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl RenderVec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderColor {
    pub linear_rgba: [f32; 4],
}

impl RenderColor {
    pub const WHITE: Self = Self {
        linear_rgba: [1.0, 1.0, 1.0, 1.0],
    };
    pub const BLACK: Self = Self {
        linear_rgba: [0.0, 0.0, 0.0, 1.0],
    };

    #[must_use]
    pub const fn linear_rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            linear_rgba: [r, g, b, a],
        }
    }
}

impl Default for RenderColor {
    fn default() -> Self {
        Self::WHITE
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderExtent2d {
    pub width: u32,
    pub height: u32,
}

impl RenderExtent2d {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.width > 0 && self.height > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderAabb {
    pub center: RenderVec3,
    pub half_extents: RenderVec3,
}

impl RenderAabb {
    #[must_use]
    pub const fn new(center: RenderVec3, half_extents: RenderVec3) -> Self {
        Self {
            center,
            half_extents,
        }
    }
}

impl Default for RenderAabb {
    fn default() -> Self {
        Self::new(RenderVec3::ZERO, RenderVec3::new(0.5, 0.5, 0.5))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderLayerMask(pub u64);

impl RenderLayerMask {
    pub const DEFAULT: Self = Self(1);
    pub const ALL: Self = Self(u64::MAX);

    #[must_use]
    pub const fn contains(self, layer: RenderLayerMask) -> bool {
        self.0 & layer.0 == layer.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderDirtyFlags(pub u16);

impl RenderDirtyFlags {
    pub const NONE: Self = Self(0);
    pub const TRANSFORM: Self = Self(1 << 0);
    pub const MATERIAL: Self = Self(1 << 1);
    pub const GEOMETRY: Self = Self(1 << 2);
    pub const VISIBILITY: Self = Self(1 << 3);
    pub const LIGHT: Self = Self(1 << 4);
    pub const UI: Self = Self(1 << 5);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialFeatureMask(pub u32);

impl MaterialFeatureMask {
    pub const NONE: Self = Self(0);
    pub const BASE_COLOR_TEXTURE: Self = Self(1 << 0);
    pub const NORMAL_TEXTURE: Self = Self(1 << 1);
    pub const METALLIC_ROUGHNESS_TEXTURE: Self = Self(1 << 2);
    pub const EMISSIVE: Self = Self(1 << 3);
    pub const ALPHA_BLEND: Self = Self(1 << 4);
    pub const DOUBLE_SIDED: Self = Self(1 << 5);
    pub const CUSTOM_SHADER: Self = Self(1 << 6);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComponentTemperature {
    Hot,
    Cold,
    Marker,
    AssetReference,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeDetectionPolicy {
    BevyChanged,
    MarkerSpecialization,
    DirtyBitsOrEvents,
    AssetVersion,
    StableUntilDespawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererComponentApiDescriptor {
    pub name: &'static str,
    pub temperature: ComponentTemperature,
    pub change_detection: ChangeDetectionPolicy,
    pub backend_handle_free: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererAssetApiDescriptor {
    pub name: &'static str,
    pub change_detection: ChangeDetectionPolicy,
    pub backend_handle_free: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct Renderable {
    pub object_id: RenderStableId,
    pub dirty: RenderDirtyFlags,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderMesh {
    pub mesh: RenderMeshAssetId,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderMaterial {
    pub material: RenderMaterialAssetId,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Component)]
pub struct RenderBounds {
    pub local: RenderAabb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderLayer {
    pub mask: RenderLayerMask,
}

impl Default for RenderLayer {
    fn default() -> Self {
        Self {
            mask: RenderLayerMask::DEFAULT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderVisibilityState {
    Visible,
    Hidden,
    Inherited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderVisibility {
    pub state: RenderVisibilityState,
}

impl Default for RenderVisibility {
    fn default() -> Self {
        Self {
            state: RenderVisibilityState::Inherited,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderObjectId(pub RenderStableId);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderStatic;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderDynamic {
    pub dirty: RenderDirtyFlags,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderDebugName {
    pub name: &'static str,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct RenderCamera {
    pub layers: RenderLayerMask,
    pub order: i16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct MainCamera;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraProjectionMode {
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct CameraProjection {
    pub mode: CameraProjectionMode,
    pub vertical_fov_radians: f32,
    pub orthographic_height: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraProjection {
    fn default() -> Self {
        Self {
            mode: CameraProjectionMode::Perspective,
            vertical_fov_radians: 1.047_197_6,
            orthographic_height: 10.0,
            near: 0.05,
            far: 50_000.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct CameraExposure {
    pub exposure_value: f32,
    pub auto_exposure: bool,
}

impl Default for CameraExposure {
    fn default() -> Self {
        Self {
            exposure_value: 0.0,
            auto_exposure: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Component)]
pub struct CameraJitter {
    pub frame_index: u64,
    pub offset: RenderVec2,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct CameraHistory {
    pub history_id: RenderStableId,
    pub reset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraRenderTargetKind {
    PrimaryWindow,
    Texture,
    Headless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct CameraRenderTarget {
    pub kind: CameraRenderTargetKind,
    pub texture: RenderTextureAssetId,
    pub extent: RenderExtent2d,
}

impl Default for CameraRenderTarget {
    fn default() -> Self {
        Self {
            kind: CameraRenderTargetKind::PrimaryWindow,
            texture: RenderTextureAssetId::INVALID,
            extent: RenderExtent2d::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum CameraDebugView {
    #[default]
    None,
    Depth,
    MotionVectors,
    Normals,
    BaseColor,
    RoughnessMetallic,
    LightHeatmap,
    VirtualPages,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct DirectionalLight {
    pub color: RenderColor,
    pub illuminance_lux: f32,
    pub angular_radius_radians: f32,
}

impl Default for DirectionalLight {
    fn default() -> Self {
        Self {
            color: RenderColor::WHITE,
            illuminance_lux: 80_000.0,
            angular_radius_radians: 0.009_35,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct PointLight {
    pub color: RenderColor,
    pub intensity_lumens: f32,
    pub range: f32,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            color: RenderColor::WHITE,
            intensity_lumens: 800.0,
            range: 16.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct SpotLight {
    pub color: RenderColor,
    pub intensity_lumens: f32,
    pub range: f32,
    pub inner_angle_radians: f32,
    pub outer_angle_radians: f32,
}

impl Default for SpotLight {
    fn default() -> Self {
        Self {
            color: RenderColor::WHITE,
            intensity_lumens: 1_200.0,
            range: 24.0,
            inner_angle_radians: 0.35,
            outer_angle_radians: 0.65,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowMode {
    None,
    VirtualPages,
    RayTraced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct ShadowCaster {
    pub mode: ShadowMode,
}

impl Default for ShadowCaster {
    fn default() -> Self {
        Self {
            mode: ShadowMode::VirtualPages,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct ShadowReceiver {
    pub priority: u8,
}

impl Default for ShadowReceiver {
    fn default() -> Self {
        Self { priority: 128 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LightLayer {
    pub mask: RenderLayerMask,
}

impl Default for LightLayer {
    fn default() -> Self {
        Self {
            mask: RenderLayerMask::DEFAULT,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Component)]
pub struct LightBounds {
    pub bounds: RenderAabb,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct UiSurface {
    pub surface_id: RenderStableId,
    pub extent: RenderExtent2d,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct UiLayer {
    pub layer: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct UiCompositeOrder {
    pub order: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct UiOpacity {
    pub alpha: f32,
}

impl Default for UiOpacity {
    fn default() -> Self {
        Self { alpha: 1.0 }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum UiColorSpace {
    Srgb,
    #[default]
    Linear,
    Hdr10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Component)]
pub enum CefTransportMode {
    GpuSharedTexture,
    CpuDiagnosticOnly,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct CefSurface {
    pub surface_id: RenderStableId,
    pub transport: CefTransportMode,
    pub gpu_only: bool,
}

impl Default for CefSurface {
    fn default() -> Self {
        Self {
            surface_id: RenderStableId::INVALID,
            transport: CefTransportMode::GpuSharedTexture,
            gpu_only: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct CefFrameProducer {
    pub producer_id: RenderStableId,
    pub max_frame_rate_hz: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CefHealthState {
    Healthy,
    WaitingForFrame,
    TransportDegraded,
    FailedClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct CefSurfaceHealth {
    pub state: CefHealthState,
}

impl Default for CefSurfaceHealth {
    fn default() -> Self {
        Self {
            state: CefHealthState::WaitingForFrame,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToneMappingOperator {
    AcesFitted,
    Reinhard,
    Linear,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct ToneMappingSettings {
    pub operator: ToneMappingOperator,
    pub exposure_bias: f32,
}

impl Default for ToneMappingSettings {
    fn default() -> Self {
        Self {
            operator: ToneMappingOperator::AcesFitted,
            exposure_bias: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct BloomSettings {
    pub intensity: f32,
    pub threshold: f32,
    pub radius: f32,
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self {
            intensity: 0.08,
            threshold: 1.2,
            radius: 0.6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct TaaSettings {
    pub enabled: bool,
    pub history_weight: f32,
    pub clamp_strength: f32,
}

impl Default for TaaSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            history_weight: 0.9,
            clamp_strength: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct HdrOutputSettings {
    pub enabled: bool,
    pub max_nits: f32,
    pub paper_white_nits: f32,
}

impl Default for HdrOutputSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_nits: 1_000.0,
            paper_white_nits: 200.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerMode {
    #[default]
    Native,
    Dlss,
    Fsr,
    DebugNearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct UpscalerSettings {
    pub mode: UpscalerMode,
    pub render_scale: f32,
    pub sharpen: f32,
}

impl Default for UpscalerSettings {
    fn default() -> Self {
        Self {
            mode: UpscalerMode::Native,
            render_scale: 1.0,
            sharpen: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UpscalerState {
    Disabled,
    Ready,
    MissingCapability,
    InvalidInputs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct UpscalerStatus {
    pub state: UpscalerState,
}

impl Default for UpscalerStatus {
    fn default() -> Self {
        Self {
            state: UpscalerState::Disabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlphaMode {
    Opaque,
    Masked,
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandardMaterial {
    pub base_color: RenderColor,
    pub base_color_texture: RenderTextureAssetId,
    pub normal_texture: RenderTextureAssetId,
    pub metallic_roughness_texture: RenderTextureAssetId,
    pub emissive_color: RenderColor,
    pub metallic: f32,
    pub roughness: f32,
    pub alpha_mode: AlphaMode,
    pub feature_mask: MaterialFeatureMask,
}

impl Default for StandardMaterial {
    fn default() -> Self {
        Self {
            base_color: RenderColor::WHITE,
            base_color_texture: RenderTextureAssetId::INVALID,
            normal_texture: RenderTextureAssetId::INVALID,
            metallic_roughness_texture: RenderTextureAssetId::INVALID,
            emissive_color: RenderColor::BLACK,
            metallic: 0.0,
            roughness: 0.5,
            alpha_mode: AlphaMode::Opaque,
            feature_mask: MaterialFeatureMask::NONE,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialBindingLayout {
    #[default]
    StandardPbr,
    Unlit,
    Ui,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialBindingSchema {
    pub layout: MaterialBindingLayout,
    pub uniform_slots: u8,
    pub texture_slots: u8,
    pub sampler_slots: u8,
}

impl MaterialBindingSchema {
    pub const STANDARD_PBR: Self = Self {
        layout: MaterialBindingLayout::StandardPbr,
        uniform_slots: 1,
        texture_slots: 4,
        sampler_slots: 1,
    };
}

impl Default for MaterialBindingSchema {
    fn default() -> Self {
        Self::STANDARD_PBR
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderMaterial {
    pub shader: RenderShaderAssetId,
    pub binding_schema: MaterialBindingSchema,
    pub feature_mask: MaterialFeatureMask,
}

impl Default for ShaderMaterial {
    fn default() -> Self {
        Self {
            shader: RenderShaderAssetId::INVALID,
            binding_schema: MaterialBindingSchema::STANDARD_PBR,
            feature_mask: MaterialFeatureMask::CUSTOM_SHADER,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderTextureFormat {
    #[default]
    Rgba8Srgb,
    Rgba16Float,
    Rg16Float,
    Depth32Float,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderTextureUsage {
    #[default]
    Sampled,
    RenderTarget,
    DepthStencil,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderTexture {
    pub texture: RenderTextureAssetId,
    pub extent: RenderExtent2d,
    pub format: RenderTextureFormat,
    pub usage: RenderTextureUsage,
    pub mip_count: u8,
}

impl Default for RenderTexture {
    fn default() -> Self {
        Self {
            texture: RenderTextureAssetId::INVALID,
            extent: RenderExtent2d::default(),
            format: RenderTextureFormat::Rgba8Srgb,
            usage: RenderTextureUsage::Sampled,
            mip_count: 1,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderFilterMode {
    Nearest,
    #[default]
    Linear,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderAddressMode {
    #[default]
    ClampToEdge,
    Repeat,
    MirrorRepeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSampler {
    pub min_filter: RenderFilterMode,
    pub mag_filter: RenderFilterMode,
    pub mip_filter: RenderFilterMode,
    pub address_u: RenderAddressMode,
    pub address_v: RenderAddressMode,
    pub address_w: RenderAddressMode,
}

impl Default for RenderSampler {
    fn default() -> Self {
        Self {
            min_filter: RenderFilterMode::Linear,
            mag_filter: RenderFilterMode::Linear,
            mip_filter: RenderFilterMode::Linear,
            address_u: RenderAddressMode::ClampToEdge,
            address_v: RenderAddressMode::ClampToEdge,
            address_w: RenderAddressMode::ClampToEdge,
        }
    }
}

pub const RENDERER_COMPONENT_HOT_COLD_SPLIT_GUIDE: [RendererComponentApiDescriptor;
    PUBLIC_RENDER_COMPONENT_COUNT] = [
    component(
        "Renderable",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "RenderMesh",
        ComponentTemperature::AssetReference,
        ChangeDetectionPolicy::AssetVersion,
    ),
    component(
        "RenderMaterial",
        ComponentTemperature::AssetReference,
        ChangeDetectionPolicy::AssetVersion,
    ),
    component(
        "RenderBounds",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "RenderLayer",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::StableUntilDespawn,
    ),
    component(
        "RenderVisibility",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "RenderObjectId",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::StableUntilDespawn,
    ),
    component(
        "RenderStatic",
        ComponentTemperature::Marker,
        ChangeDetectionPolicy::MarkerSpecialization,
    ),
    component(
        "RenderDynamic",
        ComponentTemperature::Marker,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "RenderDebugName",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::StableUntilDespawn,
    ),
    component(
        "RenderCamera",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "MainCamera",
        ComponentTemperature::Marker,
        ChangeDetectionPolicy::MarkerSpecialization,
    ),
    component(
        "CameraProjection",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "CameraExposure",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "CameraJitter",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "CameraHistory",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "CameraRenderTarget",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "CameraDebugView",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "DirectionalLight",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "PointLight",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "SpotLight",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "ShadowCaster",
        ComponentTemperature::Marker,
        ChangeDetectionPolicy::MarkerSpecialization,
    ),
    component(
        "ShadowReceiver",
        ComponentTemperature::Marker,
        ChangeDetectionPolicy::MarkerSpecialization,
    ),
    component(
        "LightLayer",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::StableUntilDespawn,
    ),
    component(
        "LightBounds",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "UiSurface",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "UiLayer",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::StableUntilDespawn,
    ),
    component(
        "UiCompositeOrder",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "UiOpacity",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "UiColorSpace",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "CefSurface",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "CefFrameProducer",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "CefTransportMode",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "CefSurfaceHealth",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "ToneMappingSettings",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "BloomSettings",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "TaaSettings",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "HdrOutputSettings",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "UpscalerSettings",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::BevyChanged,
    ),
    component(
        "UpscalerStatus",
        ComponentTemperature::Cold,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "RenderDirtyFlags",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::DirtyBitsOrEvents,
    ),
    component(
        "RenderLayerMask",
        ComponentTemperature::Hot,
        ChangeDetectionPolicy::StableUntilDespawn,
    ),
    component(
        "MaterialFeatureMask",
        ComponentTemperature::AssetReference,
        ChangeDetectionPolicy::AssetVersion,
    ),
];

pub const RENDERER_ASSET_API: [RendererAssetApiDescriptor; PUBLIC_RENDER_ASSET_COUNT] = [
    asset("StandardMaterial", ChangeDetectionPolicy::AssetVersion),
    asset("ShaderMaterial", ChangeDetectionPolicy::AssetVersion),
    asset("RenderTexture", ChangeDetectionPolicy::AssetVersion),
    asset("RenderSampler", ChangeDetectionPolicy::AssetVersion),
    asset("MaterialBindingSchema", ChangeDetectionPolicy::AssetVersion),
    asset("MaterialFeatureMask", ChangeDetectionPolicy::AssetVersion),
    asset("RenderAssetId", ChangeDetectionPolicy::AssetVersion),
];

pub const PUBLIC_RENDERER_API_FORBIDDEN_TERMS: [&str; 10] = [
    "wgpu",
    "wgpu_core",
    "wgpu_hal",
    "ID3D12",
    "VkDevice",
    "MTLDevice",
    "CommandEncoder",
    "DescriptorHeap",
    "QueueHandle",
    "DeviceHandle",
];

const fn component(
    name: &'static str,
    temperature: ComponentTemperature,
    change_detection: ChangeDetectionPolicy,
) -> RendererComponentApiDescriptor {
    RendererComponentApiDescriptor {
        name,
        temperature,
        change_detection,
        backend_handle_free: true,
    }
}

const fn asset(
    name: &'static str,
    change_detection: ChangeDetectionPolicy,
) -> RendererAssetApiDescriptor {
    RendererAssetApiDescriptor {
        name,
        change_detection,
        backend_handle_free: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::Component;

    fn assert_component<T: Component>() {}
    fn assert_copy<T: Copy>() {}

    #[test]
    fn public_renderer_components_are_bevy_components() {
        assert_component::<Renderable>();
        assert_component::<RenderMesh>();
        assert_component::<RenderMaterial>();
        assert_component::<RenderBounds>();
        assert_component::<RenderLayer>();
        assert_component::<RenderVisibility>();
        assert_component::<RenderObjectId>();
        assert_component::<RenderStatic>();
        assert_component::<RenderDynamic>();
        assert_component::<RenderDebugName>();
        assert_component::<RenderCamera>();
        assert_component::<MainCamera>();
        assert_component::<CameraProjection>();
        assert_component::<CameraExposure>();
        assert_component::<CameraJitter>();
        assert_component::<CameraHistory>();
        assert_component::<CameraRenderTarget>();
        assert_component::<CameraDebugView>();
        assert_component::<DirectionalLight>();
        assert_component::<PointLight>();
        assert_component::<SpotLight>();
        assert_component::<ShadowCaster>();
        assert_component::<ShadowReceiver>();
        assert_component::<LightLayer>();
        assert_component::<LightBounds>();
        assert_component::<UiSurface>();
        assert_component::<UiLayer>();
        assert_component::<UiCompositeOrder>();
        assert_component::<UiOpacity>();
        assert_component::<UiColorSpace>();
        assert_component::<CefTransportMode>();
        assert_component::<CefSurface>();
        assert_component::<CefFrameProducer>();
        assert_component::<CefSurfaceHealth>();
        assert_component::<ToneMappingSettings>();
        assert_component::<BloomSettings>();
        assert_component::<TaaSettings>();
        assert_component::<HdrOutputSettings>();
        assert_component::<UpscalerSettings>();
        assert_component::<UpscalerStatus>();
    }

    #[test]
    fn hot_components_are_compact_and_copyable() {
        assert_copy::<Renderable>();
        assert_copy::<RenderMesh>();
        assert_copy::<RenderMaterial>();
        assert_copy::<RenderBounds>();
        assert_copy::<RenderStatic>();
        assert_copy::<RenderDynamic>();
        assert!(core::mem::size_of::<Renderable>() <= 16);
        assert!(core::mem::size_of::<RenderMesh>() <= 8);
        assert!(core::mem::size_of::<RenderMaterial>() <= 8);
        assert!(core::mem::size_of::<RenderVisibility>() <= 1);
        assert!(core::mem::size_of::<RenderStatic>() == 0);
    }

    #[test]
    fn component_and_asset_api_names_cover_pass_three_contract() {
        for required in [
            "Renderable",
            "RenderMesh",
            "RenderMaterial",
            "RenderBounds",
            "RenderLayer",
            "RenderVisibility",
            "RenderObjectId",
            "RenderStatic",
            "RenderDynamic",
            "RenderDebugName",
            "RenderCamera",
            "MainCamera",
            "CameraProjection",
            "CameraExposure",
            "CameraJitter",
            "CameraHistory",
            "CameraRenderTarget",
            "CameraDebugView",
            "DirectionalLight",
            "PointLight",
            "SpotLight",
            "ShadowCaster",
            "ShadowReceiver",
            "LightLayer",
            "LightBounds",
            "UiSurface",
            "UiLayer",
            "UiCompositeOrder",
            "UiOpacity",
            "UiColorSpace",
            "CefSurface",
            "CefFrameProducer",
            "CefTransportMode",
            "CefSurfaceHealth",
            "ToneMappingSettings",
            "BloomSettings",
            "TaaSettings",
            "HdrOutputSettings",
            "UpscalerSettings",
            "UpscalerStatus",
        ] {
            assert!(
                RENDERER_COMPONENT_HOT_COLD_SPLIT_GUIDE
                    .iter()
                    .any(|descriptor| descriptor.name == required),
                "missing component API descriptor: {required}"
            );
        }

        for required in [
            "StandardMaterial",
            "ShaderMaterial",
            "RenderTexture",
            "RenderSampler",
            "MaterialBindingSchema",
            "MaterialFeatureMask",
        ] {
            assert!(
                RENDERER_ASSET_API
                    .iter()
                    .any(|descriptor| descriptor.name == required),
                "missing asset API descriptor: {required}"
            );
        }
    }

    #[test]
    fn public_api_descriptors_ban_backend_handle_leakage() {
        for descriptor in RENDERER_COMPONENT_HOT_COLD_SPLIT_GUIDE {
            assert!(descriptor.backend_handle_free, "{}", descriptor.name);
            for forbidden in PUBLIC_RENDERER_API_FORBIDDEN_TERMS {
                assert!(
                    !descriptor.name.contains(forbidden),
                    "{} leaked forbidden backend term {forbidden}",
                    descriptor.name
                );
            }
        }

        for descriptor in RENDERER_ASSET_API {
            assert!(descriptor.backend_handle_free, "{}", descriptor.name);
            for forbidden in PUBLIC_RENDERER_API_FORBIDDEN_TERMS {
                assert!(
                    !descriptor.name.contains(forbidden),
                    "{} leaked forbidden backend term {forbidden}",
                    descriptor.name
                );
            }
        }
    }

    #[test]
    fn material_and_texture_assets_are_backend_agnostic_records() {
        let material = StandardMaterial {
            base_color_texture: RenderTextureAssetId::first(4),
            feature_mask: MaterialFeatureMask::BASE_COLOR_TEXTURE
                .union(MaterialFeatureMask::DOUBLE_SIDED),
            ..StandardMaterial::default()
        };
        let texture = RenderTexture {
            texture: material.base_color_texture,
            extent: RenderExtent2d::new(1024, 1024),
            format: RenderTextureFormat::Rgba8Srgb,
            usage: RenderTextureUsage::Sampled,
            mip_count: 8,
        };

        assert!(material.base_color_texture.is_valid());
        assert!(
            material
                .feature_mask
                .contains(MaterialFeatureMask::BASE_COLOR_TEXTURE)
        );
        assert!(texture.extent.is_valid());
        assert_eq!(MaterialBindingSchema::STANDARD_PBR.texture_slots, 4);
    }
}
