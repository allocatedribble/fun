use crate::{
    FunRendererBackend,
    backend::PipelineId,
    ir::{
        CullMode, DescriptorFingerprint, DrawPacket, MAX_RENDER_TARGETS_PER_PASS,
        PrimitiveTopology, TextureFormat,
    },
};

pub use crate::ir::{
    ComputePipelineDesc, PipelineFamily, PipelineLayoutDesc, PipelineVariantKey, RenderPipelineDesc,
};

pub const PIPELINE_REGISTRY_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineKind {
    Render,
    Compute,
}

impl PipelineKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Compute => "compute",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineFeatureMask {
    pub bits: u32,
}

impl PipelineFeatureMask {
    pub const NONE: Self = Self { bits: 0 };
    pub const CEF_GPU_ONLY: Self = Self { bits: 1 << 0 };
    pub const UPSCALING: Self = Self { bits: 1 << 1 };
    pub const DLSS: Self = Self { bits: 1 << 2 };
    pub const FSR: Self = Self { bits: 1 << 3 };
    pub const FRAME_GENERATION: Self = Self { bits: 1 << 4 };
    pub const MANY_LIGHT: Self = Self { bits: 1 << 5 };
    pub const VIRTUAL_GEOMETRY: Self = Self { bits: 1 << 6 };
    pub const VIRTUAL_SHADOWS: Self = Self { bits: 1 << 7 };
    pub const HYBRID_GI: Self = Self { bits: 1 << 8 };
    pub const COMPUTE_CULLING: Self = Self { bits: 1 << 9 };
    pub const CLOUDS: Self = Self { bits: 1 << 10 };
    pub const SOLARI: Self = Self { bits: 1 << 11 };
    pub const MESHLETS: Self = Self { bits: 1 << 12 };
    pub const UI_COMPOSITE: Self = Self { bits: 1 << 13 };
    pub const DEBUG_OVERLAY: Self = Self { bits: 1 << 14 };
    pub const ALL: Self = Self { bits: u32::MAX };

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    #[must_use]
    pub const fn contains_all(self, required: Self) -> bool {
        self.bits & required.bits == required.bits
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineBackendMask {
    pub bits: u8,
}

impl PipelineBackendMask {
    pub const NONE: Self = Self { bits: 0 };
    pub const DX12: Self = Self { bits: 1 << 0 };
    pub const VULKAN: Self = Self { bits: 1 << 1 };
    pub const METAL: Self = Self { bits: 1 << 2 };
    pub const ALL: Self = Self {
        bits: Self::DX12.bits | Self::VULKAN.bits | Self::METAL.bits,
    };

    #[must_use]
    pub const fn contains_backend(self, backend: FunRendererBackend) -> bool {
        match backend {
            FunRendererBackend::Dx12 => self.bits & Self::DX12.bits != 0,
            FunRendererBackend::Vulkan => self.bits & Self::VULKAN.bits != 0,
            FunRendererBackend::Metal => self.bits & Self::METAL.bits != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QualityTier {
    Competitive,
    Balanced,
    Cinematic,
}

impl QualityTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Competitive => "competitive",
            Self::Balanced => "balanced",
            Self::Cinematic => "cinematic",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineQualityTierMask {
    pub bits: u8,
}

impl PipelineQualityTierMask {
    pub const NONE: Self = Self { bits: 0 };
    pub const COMPETITIVE: Self = Self { bits: 1 << 0 };
    pub const BALANCED: Self = Self { bits: 1 << 1 };
    pub const CINEMATIC: Self = Self { bits: 1 << 2 };
    pub const ALL: Self = Self {
        bits: Self::COMPETITIVE.bits | Self::BALANCED.bits | Self::CINEMATIC.bits,
    };

    #[must_use]
    pub const fn contains_tier(self, tier: QualityTier) -> bool {
        match tier {
            QualityTier::Competitive => self.bits & Self::COMPETITIVE.bits != 0,
            QualityTier::Balanced => self.bits & Self::BALANCED.bits != 0,
            QualityTier::Cinematic => self.bits & Self::CINEMATIC.bits != 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderVariantAxis {
    Backend,
    HdrLdr,
    Msaa,
    Skinning,
    AlphaMode,
    LightingTier,
    ShadowTier,
    UpscalerMode,
    VirtualGeometry,
}

impl ShaderVariantAxis {
    pub const ALL_ALLOWED: [Self; 9] = [
        Self::Backend,
        Self::HdrLdr,
        Self::Msaa,
        Self::Skinning,
        Self::AlphaMode,
        Self::LightingTier,
        Self::ShadowTier,
        Self::UpscalerMode,
        Self::VirtualGeometry,
    ];

    #[must_use]
    pub const fn bit(self) -> u16 {
        match self {
            Self::Backend => 1 << 0,
            Self::HdrLdr => 1 << 1,
            Self::Msaa => 1 << 2,
            Self::Skinning => 1 << 3,
            Self::AlphaMode => 1 << 4,
            Self::LightingTier => 1 << 5,
            Self::ShadowTier => 1 << 6,
            Self::UpscalerMode => 1 << 7,
            Self::VirtualGeometry => 1 << 8,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Backend => "backend",
            Self::HdrLdr => "hdr_ldr",
            Self::Msaa => "msaa",
            Self::Skinning => "skinning",
            Self::AlphaMode => "alpha_mode",
            Self::LightingTier => "lighting_tier",
            Self::ShadowTier => "shadow_tier",
            Self::UpscalerMode => "upscaler_mode",
            Self::VirtualGeometry => "virtual_geometry",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderVariantAxes {
    pub bits: u16,
}

impl ShaderVariantAxes {
    pub const NONE: Self = Self { bits: 0 };
    pub const BACKEND: Self = Self {
        bits: ShaderVariantAxis::Backend.bit(),
    };
    pub const HDR_LDR: Self = Self {
        bits: ShaderVariantAxis::HdrLdr.bit(),
    };
    pub const MSAA: Self = Self {
        bits: ShaderVariantAxis::Msaa.bit(),
    };
    pub const SKINNING: Self = Self {
        bits: ShaderVariantAxis::Skinning.bit(),
    };
    pub const ALPHA_MODE: Self = Self {
        bits: ShaderVariantAxis::AlphaMode.bit(),
    };
    pub const LIGHTING_TIER: Self = Self {
        bits: ShaderVariantAxis::LightingTier.bit(),
    };
    pub const SHADOW_TIER: Self = Self {
        bits: ShaderVariantAxis::ShadowTier.bit(),
    };
    pub const UPSCALER_MODE: Self = Self {
        bits: ShaderVariantAxis::UpscalerMode.bit(),
    };
    pub const VIRTUAL_GEOMETRY: Self = Self {
        bits: ShaderVariantAxis::VirtualGeometry.bit(),
    };
    pub const ALL_ALLOWED_BITS: u16 = Self::BACKEND.bits
        | Self::HDR_LDR.bits
        | Self::MSAA.bits
        | Self::SKINNING.bits
        | Self::ALPHA_MODE.bits
        | Self::LIGHTING_TIER.bits
        | Self::SHADOW_TIER.bits
        | Self::UPSCALER_MODE.bits
        | Self::VIRTUAL_GEOMETRY.bits;

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    #[must_use]
    pub const fn contains(self, axis: ShaderVariantAxis) -> bool {
        self.bits & axis.bit() != 0
    }

    #[must_use]
    pub const fn has_only_allowed_axes(self) -> bool {
        self.bits & !Self::ALL_ALLOWED_BITS == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineWarmupBoundary {
    RendererInitialization,
    SceneLoad,
    QualityTierChange,
    BackendChange,
    MaterialShaderCatalogChange,
}

impl PipelineWarmupBoundary {
    pub const ALL: [Self; 5] = [
        Self::RendererInitialization,
        Self::SceneLoad,
        Self::QualityTierChange,
        Self::BackendChange,
        Self::MaterialShaderCatalogChange,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RendererInitialization => "renderer_initialization",
            Self::SceneLoad => "scene_load",
            Self::QualityTierChange => "quality_tier_change",
            Self::BackendChange => "backend_change",
            Self::MaterialShaderCatalogChange => "material_shader_catalog_change",
        }
    }
}

const RENDERER_INIT_WARMUP: &[PipelineWarmupBoundary] =
    &[PipelineWarmupBoundary::RendererInitialization];
const SCENE_LOAD_WARMUP: &[PipelineWarmupBoundary] = &[PipelineWarmupBoundary::SceneLoad];
const MATERIAL_CATALOG_WARMUP: &[PipelineWarmupBoundary] =
    &[PipelineWarmupBoundary::MaterialShaderCatalogChange];
const QUALITY_AND_BACKEND_WARMUP: &[PipelineWarmupBoundary] = &[
    PipelineWarmupBoundary::QualityTierChange,
    PipelineWarmupBoundary::BackendChange,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineWarmupPolicy {
    pub boundaries: &'static [PipelineWarmupBoundary],
    pub required_before_hot_loop: bool,
}

impl PipelineWarmupPolicy {
    #[must_use]
    pub const fn contains_boundary(self, boundary: PipelineWarmupBoundary) -> bool {
        let mut index = 0;
        while index < self.boundaries.len() {
            if matches_boundary(self.boundaries[index], boundary) {
                return true;
            }
            index += 1;
        }
        false
    }
}

const fn matches_boundary(left: PipelineWarmupBoundary, right: PipelineWarmupBoundary) -> bool {
    matches!(
        (left, right),
        (
            PipelineWarmupBoundary::RendererInitialization,
            PipelineWarmupBoundary::RendererInitialization
        ) | (
            PipelineWarmupBoundary::SceneLoad,
            PipelineWarmupBoundary::SceneLoad
        ) | (
            PipelineWarmupBoundary::QualityTierChange,
            PipelineWarmupBoundary::QualityTierChange
        ) | (
            PipelineWarmupBoundary::BackendChange,
            PipelineWarmupBoundary::BackendChange
        ) | (
            PipelineWarmupBoundary::MaterialShaderCatalogChange,
            PipelineWarmupBoundary::MaterialShaderCatalogChange
        )
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderModuleDescriptor {
    pub stable_id: &'static str,
    pub static_label: &'static str,
    pub shader_path: &'static str,
    pub feature_mask: PipelineFeatureMask,
    pub backend_mask: PipelineBackendMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShaderVariantDescriptor {
    pub stable_id: &'static str,
    pub shader_module: &'static str,
    pub axes: ShaderVariantAxes,
    pub feature_mask: PipelineFeatureMask,
    pub backend_mask: PipelineBackendMask,
    pub quality_tier_mask: PipelineQualityTierMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineDescriptor {
    pub stable_id: &'static str,
    pub kind: PipelineKind,
    pub static_label: &'static str,
    pub shader_path: &'static str,
    pub entry_point: &'static str,
    pub shader_variant: &'static str,
    pub feature_mask: PipelineFeatureMask,
    pub backend_mask: PipelineBackendMask,
    pub quality_tier_mask: PipelineQualityTierMask,
    pub warmup_policy: PipelineWarmupPolicy,
    pub pass_dependencies: &'static [&'static str],
    pub runtime_creation_allowed_during_benchmark: bool,
}

impl PipelineDescriptor {
    #[must_use]
    pub const fn is_eligible_for(self, request: PipelineWarmupRequest) -> bool {
        self.backend_mask.contains_backend(request.backend)
            && self.quality_tier_mask.contains_tier(request.quality_tier)
            && request.features.contains_all(self.feature_mask)
            && self.warmup_policy.contains_boundary(request.boundary)
    }
}

const COMPUTE_CULLING_FEATURES: PipelineFeatureMask =
    PipelineFeatureMask::COMPUTE_CULLING.union(PipelineFeatureMask::MESHLETS);
const CLOUD_FEATURES: PipelineFeatureMask = PipelineFeatureMask::CLOUDS;
const CEF_UI_FEATURES: PipelineFeatureMask = PipelineFeatureMask::CEF_GPU_ONLY
    .union(PipelineFeatureMask::UI_COMPOSITE)
    .union(PipelineFeatureMask::FRAME_GENERATION);
const VIRTUAL_GEOMETRY_FEATURES: PipelineFeatureMask =
    PipelineFeatureMask::VIRTUAL_GEOMETRY.union(PipelineFeatureMask::MESHLETS);
const VIRTUAL_SHADOW_FEATURES: PipelineFeatureMask = PipelineFeatureMask::VIRTUAL_SHADOWS;
const LUX_FEATURES: PipelineFeatureMask = PipelineFeatureMask::MANY_LIGHT
    .union(PipelineFeatureMask::HYBRID_GI)
    .union(PipelineFeatureMask::SOLARI);
const UPSCALE_FEATURES: PipelineFeatureMask = PipelineFeatureMask::UPSCALING
    .union(PipelineFeatureMask::DLSS)
    .union(PipelineFeatureMask::FSR)
    .union(PipelineFeatureMask::FRAME_GENERATION);

const COMPUTE_CULLING_DEPS: &[&str] = &["gpu_scene_database", "virtual_geometry"];
const CLOUD_DEPS: &[&str] = &["gpu_scene_database", "lighting"];
const UI_COMPOSITE_DEPS: &[&str] = &["cef_gpu_import", "present"];
const VIRTUAL_GEOMETRY_DEPS: &[&str] = &["gpu_scene_database", "visibility"];
const VIRTUAL_SHADOW_DEPS: &[&str] = &["virtual_geometry", "lighting"];
const LUX_DEPS: &[&str] = &["gpu_scene_database", "virtual_shadows"];
const UPSCALE_DEPS: &[&str] = &["lighting", "ui_color", "motion_vectors", "depth"];

pub const SHADER_MODULES: [ShaderModuleDescriptor; 10] = [
    ShaderModuleDescriptor {
        stable_id: "shader.compute_culling",
        static_label: "fun_compute_culling_shader",
        shader_path: "fun_render/src/compute_culling.wgsl",
        feature_mask: COMPUTE_CULLING_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.cloud_density",
        static_label: "fun_cloud_density_shader",
        shader_path: "fun_render/src/sky/render/cloud_density.wgsl",
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.cloud_raymarch",
        static_label: "fun_cloud_raymarch_shader",
        shader_path: "fun_render/src/sky/render/cloud_raymarch.wgsl",
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.cloud_temporal",
        static_label: "fun_cloud_temporal_shader",
        shader_path: "fun_render/src/sky/render/cloud_temporal.wgsl",
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.cloud_composite",
        static_label: "fun_cloud_composite_shader",
        shader_path: "fun_render/src/sky/render/cloud_composite.wgsl",
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.cloud_debug",
        static_label: "fun_cloud_debug_shader",
        shader_path: "fun_render/src/sky/render/cloud_debug.wgsl",
        feature_mask: CLOUD_FEATURES.union(PipelineFeatureMask::DEBUG_OVERLAY),
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.cef_compositor",
        static_label: "renderer_cef_gpu_compositor_shader",
        shader_path: "fun-renderer/shaders/cef_compositor.wgsl",
        feature_mask: CEF_UI_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.virtual_geometry",
        static_label: "renderer_virtual_geometry_shader",
        shader_path: "fun-renderer/shaders/virtual_geometry.wgsl",
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.virtual_shadow",
        static_label: "renderer_virtual_shadow_shader",
        shader_path: "fun-renderer/shaders/virtual_shadow.wgsl",
        feature_mask: VIRTUAL_SHADOW_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
    ShaderModuleDescriptor {
        stable_id: "shader.lux_direct_lighting",
        static_label: "lux_direct_lighting_shader",
        shader_path: "fun-lux/shaders/direct_lighting.wgsl",
        feature_mask: LUX_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
    },
];

pub const SHADER_VARIANTS: [ShaderVariantDescriptor; 12] = [
    ShaderVariantDescriptor {
        stable_id: "variant.compute_culling.default",
        shader_module: "shader.compute_culling",
        axes: ShaderVariantAxes::BACKEND.union(ShaderVariantAxes::VIRTUAL_GEOMETRY),
        feature_mask: COMPUTE_CULLING_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.cloud.generation",
        shader_module: "shader.cloud_density",
        axes: ShaderVariantAxes::BACKEND.union(ShaderVariantAxes::LIGHTING_TIER),
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.cloud.raymarch",
        shader_module: "shader.cloud_raymarch",
        axes: ShaderVariantAxes::BACKEND.union(ShaderVariantAxes::LIGHTING_TIER),
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.cloud.temporal",
        shader_module: "shader.cloud_temporal",
        axes: ShaderVariantAxes::BACKEND.union(ShaderVariantAxes::HDR_LDR),
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.cloud.composite",
        shader_module: "shader.cloud_composite",
        axes: ShaderVariantAxes::BACKEND
            .union(ShaderVariantAxes::HDR_LDR)
            .union(ShaderVariantAxes::MSAA),
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.cloud.debug",
        shader_module: "shader.cloud_debug",
        axes: ShaderVariantAxes::BACKEND.union(ShaderVariantAxes::HDR_LDR),
        feature_mask: CLOUD_FEATURES.union(PipelineFeatureMask::DEBUG_OVERLAY),
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.cef_compositor.present",
        shader_module: "shader.cef_compositor",
        axes: ShaderVariantAxes::BACKEND
            .union(ShaderVariantAxes::HDR_LDR)
            .union(ShaderVariantAxes::UPSCALER_MODE),
        feature_mask: CEF_UI_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.virtual_geometry.static",
        shader_module: "shader.virtual_geometry",
        axes: ShaderVariantAxes::BACKEND
            .union(ShaderVariantAxes::ALPHA_MODE)
            .union(ShaderVariantAxes::VIRTUAL_GEOMETRY),
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.virtual_geometry.skinned",
        shader_module: "shader.virtual_geometry",
        axes: ShaderVariantAxes::BACKEND
            .union(ShaderVariantAxes::SKINNING)
            .union(ShaderVariantAxes::ALPHA_MODE)
            .union(ShaderVariantAxes::VIRTUAL_GEOMETRY),
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.virtual_shadow.directional",
        shader_module: "shader.virtual_shadow",
        axes: ShaderVariantAxes::BACKEND.union(ShaderVariantAxes::SHADOW_TIER),
        feature_mask: VIRTUAL_SHADOW_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.lux.direct_many_light",
        shader_module: "shader.lux_direct_lighting",
        axes: ShaderVariantAxes::BACKEND
            .union(ShaderVariantAxes::LIGHTING_TIER)
            .union(ShaderVariantAxes::SHADOW_TIER),
        feature_mask: LUX_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
    ShaderVariantDescriptor {
        stable_id: "variant.upscale.present_boundary",
        shader_module: "shader.cef_compositor",
        axes: ShaderVariantAxes::BACKEND
            .union(ShaderVariantAxes::HDR_LDR)
            .union(ShaderVariantAxes::UPSCALER_MODE),
        feature_mask: UPSCALE_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
    },
];

pub const PIPELINES: [PipelineDescriptor; 23] = [
    compute_culling_pipeline(
        "pipeline.compute_culling.reset",
        "fun_compute_culling_reset_pipeline",
        "reset_culling_state",
    ),
    compute_culling_pipeline(
        "pipeline.compute_culling.instance_frustum",
        "fun_compute_culling_instance_frustum_pipeline",
        "instance_frustum_cull",
    ),
    compute_culling_pipeline(
        "pipeline.compute_culling.lod_select",
        "fun_compute_culling_lod_select_pipeline",
        "lod_select",
    ),
    compute_culling_pipeline(
        "pipeline.compute_culling.meshlet_cluster",
        "fun_compute_culling_meshlet_cluster_pipeline",
        "meshlet_cluster_cull",
    ),
    compute_culling_pipeline(
        "pipeline.compute_culling.hiz_occlusion",
        "fun_compute_culling_hiz_occlusion_pipeline",
        "hiz_occlusion_cull",
    ),
    compute_culling_pipeline(
        "pipeline.compute_culling.compaction",
        "fun_compute_culling_compaction_pipeline",
        "compact_visible_instances",
    ),
    compute_culling_pipeline(
        "pipeline.compute_culling.indirect_args",
        "fun_compute_culling_indirect_args_pipeline",
        "generate_indirect_args",
    ),
    cloud_compute_pipeline(
        "pipeline.cloud.weather_map",
        "fun_cloud_weather_map_pipeline",
        "fun_render/src/sky/render/cloud_density.wgsl",
        "generate_weather_map",
        "variant.cloud.generation",
    ),
    cloud_compute_pipeline(
        "pipeline.cloud.shape_noise",
        "fun_cloud_shape_noise_pipeline",
        "fun_render/src/sky/render/cloud_density.wgsl",
        "generate_shape_noise",
        "variant.cloud.generation",
    ),
    cloud_compute_pipeline(
        "pipeline.cloud.raymarch",
        "fun_cloud_raymarch_pipeline",
        "fun_render/src/sky/render/cloud_raymarch.wgsl",
        "raymarch_cloud_layer",
        "variant.cloud.raymarch",
    ),
    cloud_compute_pipeline(
        "pipeline.cloud.temporal",
        "fun_cloud_temporal_pipeline",
        "fun_render/src/sky/render/cloud_temporal.wgsl",
        "resolve_cloud_history",
        "variant.cloud.temporal",
    ),
    cloud_compute_pipeline(
        "pipeline.cloud.composite",
        "fun_cloud_composite_pipeline",
        "fun_render/src/sky/render/cloud_composite.wgsl",
        "composite_clouds",
        "variant.cloud.composite",
    ),
    PipelineDescriptor {
        stable_id: "pipeline.cloud.debug",
        kind: PipelineKind::Compute,
        static_label: "fun_cloud_debug_pipeline",
        shader_path: "fun_render/src/sky/render/cloud_debug.wgsl",
        entry_point: "write_cloud_debug_overlay",
        shader_variant: "variant.cloud.debug",
        feature_mask: CLOUD_FEATURES.union(PipelineFeatureMask::DEBUG_OVERLAY),
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: SCENE_LOAD_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: CLOUD_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.cloud.view_composite",
        kind: PipelineKind::Render,
        static_label: "fun_cloud_view_composite_pipeline",
        shader_path: "fun_render/src/sky/render/cloud_composite.wgsl",
        entry_point: "fragment_clouds_to_view",
        shader_variant: "variant.cloud.composite",
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: QUALITY_AND_BACKEND_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: CLOUD_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.cef.gpu_composite",
        kind: PipelineKind::Render,
        static_label: "renderer_cef_gpu_composite_pipeline",
        shader_path: "fun-renderer/shaders/cef_compositor.wgsl",
        entry_point: "fragment_cef_to_ui_color",
        shader_variant: "variant.cef_compositor.present",
        feature_mask: CEF_UI_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: RENDERER_INIT_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: UI_COMPOSITE_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.virtual_geometry.static_cluster_pages",
        kind: PipelineKind::Render,
        static_label: "renderer_virtual_geometry_static_cluster_pipeline",
        shader_path: "fun-renderer/shaders/virtual_geometry.wgsl",
        entry_point: "fragment_virtual_geometry",
        shader_variant: "variant.virtual_geometry.static",
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: MATERIAL_CATALOG_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: VIRTUAL_GEOMETRY_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.virtual_geometry.skinned_cluster_pages",
        kind: PipelineKind::Render,
        static_label: "renderer_virtual_geometry_skinned_cluster_pipeline",
        shader_path: "fun-renderer/shaders/virtual_geometry.wgsl",
        entry_point: "fragment_virtual_geometry",
        shader_variant: "variant.virtual_geometry.skinned",
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: MATERIAL_CATALOG_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: VIRTUAL_GEOMETRY_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.virtual_shadow.directional_pages",
        kind: PipelineKind::Compute,
        static_label: "renderer_virtual_shadow_directional_pages_pipeline",
        shader_path: "fun-renderer/shaders/virtual_shadow.wgsl",
        entry_point: "update_directional_shadow_pages",
        shader_variant: "variant.virtual_shadow.directional",
        feature_mask: VIRTUAL_SHADOW_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: SCENE_LOAD_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: VIRTUAL_SHADOW_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.lux.direct_many_light",
        kind: PipelineKind::Compute,
        static_label: "lux_direct_many_light_pipeline",
        shader_path: "fun-lux/shaders/direct_lighting.wgsl",
        entry_point: "shade_many_light_direct",
        shader_variant: "variant.lux.direct_many_light",
        feature_mask: LUX_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: SCENE_LOAD_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: LUX_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.upscale.dlss_sr",
        kind: PipelineKind::Compute,
        static_label: "renderer_dlss_sr_present_boundary_pipeline",
        shader_path: "fun-renderer/shaders/present_boundary.wgsl",
        entry_point: "dispatch_dlss_sr",
        shader_variant: "variant.upscale.present_boundary",
        feature_mask: UPSCALE_FEATURES.union(PipelineFeatureMask::DLSS),
        backend_mask: PipelineBackendMask::DX12,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: RENDERER_INIT_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: UPSCALE_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.upscale.fsr_sr",
        kind: PipelineKind::Compute,
        static_label: "renderer_fsr_sr_present_boundary_pipeline",
        shader_path: "fun-renderer/shaders/present_boundary.wgsl",
        entry_point: "dispatch_fsr_sr",
        shader_variant: "variant.upscale.present_boundary",
        feature_mask: UPSCALE_FEATURES.union(PipelineFeatureMask::FSR),
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: RENDERER_INIT_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: UPSCALE_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.frame_generation.present_boundary",
        kind: PipelineKind::Compute,
        static_label: "renderer_frame_generation_present_boundary_pipeline",
        shader_path: "fun-renderer/shaders/present_boundary.wgsl",
        entry_point: "dispatch_frame_generation",
        shader_variant: "variant.upscale.present_boundary",
        feature_mask: UPSCALE_FEATURES,
        backend_mask: PipelineBackendMask::DX12,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: RENDERER_INIT_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: UPSCALE_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
    PipelineDescriptor {
        stable_id: "pipeline.material.catalog_rebuild",
        kind: PipelineKind::Render,
        static_label: "renderer_material_catalog_pipeline",
        shader_path: "fun-renderer/shaders/material_catalog.wgsl",
        entry_point: "fragment_material_catalog",
        shader_variant: "variant.virtual_geometry.static",
        feature_mask: PipelineFeatureMask::NONE,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: MATERIAL_CATALOG_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: VIRTUAL_GEOMETRY_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    },
];

const fn compute_culling_pipeline(
    stable_id: &'static str,
    static_label: &'static str,
    entry_point: &'static str,
) -> PipelineDescriptor {
    PipelineDescriptor {
        stable_id,
        kind: PipelineKind::Compute,
        static_label,
        shader_path: "fun_render/src/compute_culling.wgsl",
        entry_point,
        shader_variant: "variant.compute_culling.default",
        feature_mask: COMPUTE_CULLING_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: RENDERER_INIT_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: COMPUTE_CULLING_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    }
}

const fn cloud_compute_pipeline(
    stable_id: &'static str,
    static_label: &'static str,
    shader_path: &'static str,
    entry_point: &'static str,
    shader_variant: &'static str,
) -> PipelineDescriptor {
    PipelineDescriptor {
        stable_id,
        kind: PipelineKind::Compute,
        static_label,
        shader_path,
        entry_point,
        shader_variant,
        feature_mask: CLOUD_FEATURES,
        backend_mask: PipelineBackendMask::ALL,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        warmup_policy: PipelineWarmupPolicy {
            boundaries: SCENE_LOAD_WARMUP,
            required_before_hot_loop: true,
        },
        pass_dependencies: CLOUD_DEPS,
        runtime_creation_allowed_during_benchmark: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineRegistry {
    pub shader_modules: &'static [ShaderModuleDescriptor],
    pub shader_variants: &'static [ShaderVariantDescriptor],
    pub pipelines: &'static [PipelineDescriptor],
}

impl PipelineRegistry {
    #[must_use]
    pub const fn new(
        shader_modules: &'static [ShaderModuleDescriptor],
        shader_variants: &'static [ShaderVariantDescriptor],
        pipelines: &'static [PipelineDescriptor],
    ) -> Self {
        Self {
            shader_modules,
            shader_variants,
            pipelines,
        }
    }

    #[must_use]
    pub const fn default_static() -> Self {
        Self::new(&SHADER_MODULES, &SHADER_VARIANTS, &PIPELINES)
    }

    #[must_use]
    pub const fn pipeline_count(self) -> usize {
        self.pipelines.len()
    }

    #[must_use]
    pub const fn shader_module_count(self) -> usize {
        self.shader_modules.len()
    }

    #[must_use]
    pub const fn shader_variant_count(self) -> usize {
        self.shader_variants.len()
    }

    #[must_use]
    pub fn find_pipeline_by_label(&self, label: &str) -> Option<&PipelineDescriptor> {
        self.pipelines
            .iter()
            .find(|descriptor| descriptor.static_label == label)
    }

    #[must_use]
    pub fn warmup_plan(&self, request: PipelineWarmupRequest) -> PipelineWarmupPlan {
        let mut eligible_pipeline_count = 0;
        let mut skipped_backend_count = 0;
        let mut skipped_feature_count = 0;
        let mut skipped_quality_count = 0;
        let mut skipped_boundary_count = 0;
        let mut first_pipeline_label = None;

        for pipeline in self.pipelines {
            if !pipeline.backend_mask.contains_backend(request.backend) {
                skipped_backend_count += 1;
                continue;
            }
            if !request.features.contains_all(pipeline.feature_mask) {
                skipped_feature_count += 1;
                continue;
            }
            if !pipeline
                .quality_tier_mask
                .contains_tier(request.quality_tier)
            {
                skipped_quality_count += 1;
                continue;
            }
            if !pipeline.warmup_policy.contains_boundary(request.boundary) {
                skipped_boundary_count += 1;
                continue;
            }
            eligible_pipeline_count += 1;
            if first_pipeline_label.is_none() {
                first_pipeline_label = Some(pipeline.static_label);
            }
        }

        PipelineWarmupPlan {
            boundary: request.boundary,
            backend: request.backend,
            quality_tier: request.quality_tier,
            eligible_pipeline_count,
            total_pipeline_count: self.pipelines.len(),
            skipped_backend_count,
            skipped_feature_count,
            skipped_quality_count,
            skipped_boundary_count,
            first_pipeline_label,
        }
    }

    #[must_use]
    pub fn audit_runtime_creation(
        &self,
        samples: &[PipelineRuntimeCreationSample],
    ) -> PipelineRuntimeCreationAudit {
        let mut unexpected_creation_count = 0;
        let mut whitelisted_creation_count = 0;
        let mut non_benchmark_creation_count = 0;
        let mut unknown_label_count = 0;
        let mut first_unexpected_label = None;

        for sample in samples {
            if sample.count == 0 {
                continue;
            }
            if !sample.benchmark_window {
                non_benchmark_creation_count += sample.count;
                continue;
            }

            let allow_during_benchmark = self
                .find_pipeline_by_label(sample.static_label)
                .map(|descriptor| descriptor.runtime_creation_allowed_during_benchmark)
                .unwrap_or_else(|| {
                    unknown_label_count += 1;
                    false
                });

            if allow_during_benchmark {
                whitelisted_creation_count += sample.count;
            } else {
                unexpected_creation_count += sample.count;
                if first_unexpected_label.is_none() {
                    first_unexpected_label = Some(sample.static_label);
                }
            }
        }

        PipelineRuntimeCreationAudit {
            unexpected_creation_count,
            whitelisted_creation_count,
            non_benchmark_creation_count,
            unknown_label_count,
            first_unexpected_label,
        }
    }

    #[must_use]
    pub fn variant_axes_are_constrained(&self) -> bool {
        self.shader_variants
            .iter()
            .all(|variant| variant.axes.has_only_allowed_axes())
    }
}

impl Default for PipelineRegistry {
    fn default() -> Self {
        Self::default_static()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineWarmupRequest {
    pub boundary: PipelineWarmupBoundary,
    pub backend: FunRendererBackend,
    pub quality_tier: QualityTier,
    pub features: PipelineFeatureMask,
}

impl PipelineWarmupRequest {
    #[must_use]
    pub const fn renderer_initialization(backend: FunRendererBackend) -> Self {
        Self {
            boundary: PipelineWarmupBoundary::RendererInitialization,
            backend,
            quality_tier: QualityTier::Balanced,
            features: PipelineFeatureMask::ALL,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineWarmupPlan {
    pub boundary: PipelineWarmupBoundary,
    pub backend: FunRendererBackend,
    pub quality_tier: QualityTier,
    pub eligible_pipeline_count: usize,
    pub total_pipeline_count: usize,
    pub skipped_backend_count: usize,
    pub skipped_feature_count: usize,
    pub skipped_quality_count: usize,
    pub skipped_boundary_count: usize,
    pub first_pipeline_label: Option<&'static str>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PipelineRuntimeCounters {
    pub render_pipelines_created_this_frame: u32,
    pub compute_pipelines_created_this_frame: u32,
    pub shader_modules_created_this_frame: u32,
    pub pipeline_cache_hits: u32,
    pub pipeline_cache_misses: u32,
    pub warmup_failures: u32,
    pub fallback_pipeline_usage: u32,
}

impl PipelineRuntimeCounters {
    #[must_use]
    pub const fn unexpected_runtime_creation_count(self) -> u32 {
        self.render_pipelines_created_this_frame
            .saturating_add(self.compute_pipelines_created_this_frame)
            .saturating_add(self.shader_modules_created_this_frame)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeCreationKind {
    RenderPipeline,
    ComputePipeline,
    ShaderModule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineRuntimeCreationSample {
    pub kind: RuntimeCreationKind,
    pub static_label: &'static str,
    pub count: u32,
    pub benchmark_window: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PipelineRuntimeCreationAudit {
    pub unexpected_creation_count: u32,
    pub whitelisted_creation_count: u32,
    pub non_benchmark_creation_count: u32,
    pub unknown_label_count: u32,
    pub first_unexpected_label: Option<&'static str>,
}

impl PipelineRuntimeCreationAudit {
    #[must_use]
    pub const fn passes_perf_gate(self) -> bool {
        self.unexpected_creation_count == 0
    }
}

pub struct PreparedPipelinePhase;

pub type PreparedPipelineId = PipelineId<PreparedPipelinePhase>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderEntry {
    pub shader_hash: u64,
    pub module_label: &'static str,
    pub entry_point: &'static str,
}

impl ShaderEntry {
    pub const EMPTY: Self = Self {
        shader_hash: 0,
        module_label: "",
        entry_point: "",
    };

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            self.module_label,
            &[
                self.shader_hash,
                DescriptorFingerprint::from_label_and_words(self.entry_point, &[]).0,
            ],
        )
    }
}

impl Default for ShaderEntry {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderEntrySet {
    pub vertex: ShaderEntry,
    pub fragment: ShaderEntry,
    pub compute: ShaderEntry,
}

impl ShaderEntrySet {
    pub const EMPTY: Self = Self {
        vertex: ShaderEntry::EMPTY,
        fragment: ShaderEntry::EMPTY,
        compute: ShaderEntry::EMPTY,
    };

    #[must_use]
    pub const fn render(vertex: ShaderEntry, fragment: ShaderEntry) -> Self {
        Self {
            vertex,
            fragment,
            compute: ShaderEntry::EMPTY,
        }
    }

    #[must_use]
    pub const fn compute(compute: ShaderEntry) -> Self {
        Self {
            vertex: ShaderEntry::EMPTY,
            fragment: ShaderEntry::EMPTY,
            compute,
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            "shader_entry_set",
            &[
                self.vertex.stable_hash().0,
                self.fragment.stable_hash().0,
                self.compute.stable_hash().0,
            ],
        )
    }
}

impl Default for ShaderEntrySet {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderTargetState {
    pub color_formats: [TextureFormat; MAX_RENDER_TARGETS_PER_PASS],
    pub color_target_count: u8,
    pub depth_format: TextureFormat,
    pub msaa_count: u8,
}

impl RenderTargetState {
    pub const EMPTY: Self = Self {
        color_formats: [TextureFormat::Undefined; MAX_RENDER_TARGETS_PER_PASS],
        color_target_count: 0,
        depth_format: TextureFormat::Undefined,
        msaa_count: 1,
    };

    #[must_use]
    pub const fn from_render_desc(desc: RenderPipelineDesc) -> Self {
        Self {
            color_formats: desc.color_formats,
            color_target_count: desc.color_target_count,
            depth_format: desc.depth_format,
            msaa_count: desc.sample_count,
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            "render_target_state",
            &[
                u64::from(self.color_target_count),
                self.color_formats[0] as u64,
                self.color_formats[1] as u64,
                self.color_formats[2] as u64,
                self.color_formats[3] as u64,
                self.depth_format as u64,
                u64::from(self.msaa_count),
            ],
        )
    }
}

impl Default for RenderTargetState {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthCompareFunction {
    Never,
    Less,
    Equal,
    #[default]
    LessEqual,
    Greater,
    GreaterEqual,
    Always,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DepthState {
    pub format: TextureFormat,
    pub write_enabled: bool,
    pub compare: DepthCompareFunction,
}

impl DepthState {
    pub const DISABLED: Self = Self {
        format: TextureFormat::Undefined,
        write_enabled: false,
        compare: DepthCompareFunction::Always,
    };

    #[must_use]
    pub const fn from_render_desc(desc: RenderPipelineDesc) -> Self {
        if desc.depth_format.is_depth() {
            Self {
                format: desc.depth_format,
                write_enabled: true,
                compare: DepthCompareFunction::LessEqual,
            }
        } else {
            Self::DISABLED
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            "depth_state",
            &[
                self.format as u64,
                self.write_enabled as u64,
                self.compare as u64,
            ],
        )
    }
}

impl Default for DepthState {
    fn default() -> Self {
        Self::DISABLED
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    #[default]
    Disabled,
    AlphaBlend,
    PremultipliedAlpha,
    Additive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlendState {
    pub mode: BlendMode,
    pub color_write_mask: u8,
}

impl BlendState {
    pub const OPAQUE: Self = Self {
        mode: BlendMode::Disabled,
        color_write_mask: 0x0f,
    };

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            "blend_state",
            &[self.mode as u64, u64::from(self.color_write_mask)],
        )
    }
}

impl Default for BlendState {
    fn default() -> Self {
        Self::OPAQUE
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineFrontFace {
    #[default]
    CounterClockwise,
    Clockwise,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineFillMode {
    #[default]
    Fill,
    Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterState {
    pub topology: PrimitiveTopology,
    pub cull_mode: CullMode,
    pub front_face: PipelineFrontFace,
    pub fill_mode: PipelineFillMode,
}

impl RasterState {
    #[must_use]
    pub const fn from_render_desc(desc: RenderPipelineDesc) -> Self {
        Self {
            topology: desc.topology,
            cull_mode: desc.cull_mode,
            front_face: PipelineFrontFace::CounterClockwise,
            fill_mode: PipelineFillMode::Fill,
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            "raster_state",
            &[
                self.topology as u64,
                self.cull_mode as u64,
                self.front_face as u64,
                self.fill_mode as u64,
            ],
        )
    }
}

impl Default for RasterState {
    fn default() -> Self {
        Self {
            topology: PrimitiveTopology::TriangleList,
            cull_mode: CullMode::Back,
            front_face: PipelineFrontFace::CounterClockwise,
            fill_mode: PipelineFillMode::Fill,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VertexStepMode {
    #[default]
    Vertex,
    Instance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VertexLayout {
    pub stride_bytes: u16,
    pub attribute_count: u8,
    pub attribute_signature: DescriptorFingerprint,
    pub step_mode: VertexStepMode,
}

impl VertexLayout {
    pub const EMPTY: Self = Self {
        stride_bytes: 0,
        attribute_count: 0,
        attribute_signature: DescriptorFingerprint::from_u64(0),
        step_mode: VertexStepMode::Vertex,
    };

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            "vertex_layout",
            &[
                u64::from(self.stride_bytes),
                u64::from(self.attribute_count),
                self.attribute_signature.0,
                self.step_mode as u64,
            ],
        )
    }
}

impl Default for VertexLayout {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPipelineKeyDesc {
    pub pipeline: RenderPipelineDesc,
    pub shader_entries: ShaderEntrySet,
    pub reflection_signature: DescriptorFingerprint,
    pub bind_layout_hash: DescriptorFingerprint,
    pub blend_state: BlendState,
    pub vertex_layout: VertexLayout,
    pub quality_tier: QualityTier,
    pub feature_mask: PipelineFeatureMask,
    pub backend: FunRendererBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineCacheKey {
    pub stable_name: &'static str,
    pub kind: PipelineKind,
    pub shader_entries: ShaderEntrySet,
    pub reflection_signature: DescriptorFingerprint,
    pub bind_layout_hash: DescriptorFingerprint,
    pub render_target_state: RenderTargetState,
    pub depth_state: DepthState,
    pub blend_state: BlendState,
    pub raster_state: RasterState,
    pub vertex_layout: VertexLayout,
    pub quality_tier: QualityTier,
    pub feature_mask: PipelineFeatureMask,
    pub backend: FunRendererBackend,
}

impl PipelineCacheKey {
    #[must_use]
    pub fn from_render_desc(desc: RenderPipelineKeyDesc) -> Self {
        Self {
            stable_name: desc.pipeline.stable_name,
            kind: PipelineKind::Render,
            shader_entries: desc.shader_entries,
            reflection_signature: desc.reflection_signature,
            bind_layout_hash: desc.bind_layout_hash,
            render_target_state: RenderTargetState::from_render_desc(desc.pipeline),
            depth_state: DepthState::from_render_desc(desc.pipeline),
            blend_state: desc.blend_state,
            raster_state: RasterState::from_render_desc(desc.pipeline),
            vertex_layout: desc.vertex_layout,
            quality_tier: desc.quality_tier,
            feature_mask: desc.feature_mask,
            backend: desc.backend,
        }
    }

    #[must_use]
    pub fn from_compute_desc(
        desc: ComputePipelineDesc,
        shader_entries: ShaderEntrySet,
        reflection_signature: DescriptorFingerprint,
        bind_layout_hash: DescriptorFingerprint,
        quality_tier: QualityTier,
        feature_mask: PipelineFeatureMask,
        backend: FunRendererBackend,
    ) -> Self {
        Self {
            stable_name: desc.stable_name,
            kind: PipelineKind::Compute,
            shader_entries,
            reflection_signature,
            bind_layout_hash,
            render_target_state: RenderTargetState::EMPTY,
            depth_state: DepthState::DISABLED,
            blend_state: BlendState::OPAQUE,
            raster_state: RasterState::default(),
            vertex_layout: VertexLayout::EMPTY,
            quality_tier,
            feature_mask,
            backend,
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            self.stable_name,
            &[
                self.kind as u64,
                self.shader_entries.stable_hash().0,
                self.reflection_signature.0,
                self.bind_layout_hash.0,
                self.render_target_state.stable_hash().0,
                self.depth_state.stable_hash().0,
                self.blend_state.stable_hash().0,
                self.raster_state.stable_hash().0,
                self.vertex_layout.stable_hash().0,
                self.raster_state.topology as u64,
                self.quality_tier as u64,
                u64::from(self.feature_mask.bits),
                self.backend as u64,
            ],
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedPipelineDrawPacket {
    pub pipeline: PreparedPipelineId,
    pub draw: DrawPacket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineCreationPhase {
    Warmup,
    AssetLoad,
    RuntimeMeasured,
}

impl PipelineCreationPhase {
    #[must_use]
    pub const fn allows_creation(self) -> bool {
        !matches!(self, Self::RuntimeMeasured)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineCreationKind {
    ShaderModule,
    PipelineLayout,
    RenderPipeline,
    ComputePipeline,
}

impl PipelineCreationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShaderModule => "shader_module",
            Self::PipelineLayout => "pipeline_layout",
            Self::RenderPipeline => "render_pipeline",
            Self::ComputePipeline => "compute_pipeline",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCacheTelemetry {
    pub shader_modules_created: u32,
    pub pipeline_layouts_created: u32,
    pub render_pipelines_created: u32,
    pub compute_pipelines_created: u32,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub runtime_creation_failures: u32,
    pub missing_variant_fallbacks: u32,
    pub debug_material_draws: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPipelineVariantReason {
    NotPreparedDuringWarmup,
    FeatureVariantUnavailable,
}

impl MissingPipelineVariantReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotPreparedDuringWarmup => "not_prepared_during_warmup",
            Self::FeatureVariantUnavailable => "feature_variant_unavailable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingPipelineVariantFailure {
    pub requested_pipeline: &'static str,
    pub fallback_pipeline: PreparedPipelineId,
    pub reason: MissingPipelineVariantReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineVariantResolution {
    pub pipeline: PreparedPipelineId,
    pub used_debug_material: bool,
    pub failure: Option<MissingPipelineVariantFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineCacheError {
    RuntimeCreationAfterWarmup {
        kind: PipelineCreationKind,
        stable_name: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineCacheDecision {
    pub kind: PipelineCreationKind,
    pub stable_name: &'static str,
    pub fingerprint: DescriptorFingerprint,
    pub prepared_pipeline: PreparedPipelineId,
    pub created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PipelineCacheEntry {
    kind: PipelineCreationKind,
    stable_name: &'static str,
    fingerprint: DescriptorFingerprint,
    prepared_pipeline: PreparedPipelineId,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PipelineCache {
    entries: Vec<PipelineCacheEntry>,
    pub telemetry: PipelineCacheTelemetry,
}

impl PipelineCache {
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn find_prepared_pipeline(&self, key: PipelineCacheKey) -> Option<PreparedPipelineId> {
        let fingerprint = key.stable_hash();
        self.entries
            .iter()
            .find(|entry| {
                matches!(
                    (key.kind, entry.kind),
                    (PipelineKind::Render, PipelineCreationKind::RenderPipeline)
                        | (PipelineKind::Compute, PipelineCreationKind::ComputePipeline)
                ) && entry.fingerprint == fingerprint
            })
            .map(|entry| entry.prepared_pipeline)
    }

    pub fn ensure_shader_module(
        &mut self,
        stable_name: &'static str,
        fingerprint: DescriptorFingerprint,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.ensure_artifact(
            PipelineCreationKind::ShaderModule,
            stable_name,
            fingerprint,
            phase,
        )
    }

    pub fn ensure_pipeline_layout(
        &mut self,
        stable_name: &'static str,
        fingerprint: DescriptorFingerprint,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.ensure_artifact(
            PipelineCreationKind::PipelineLayout,
            stable_name,
            fingerprint,
            phase,
        )
    }

    pub fn ensure_render_pipeline(
        &mut self,
        key: PipelineCacheKey,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.ensure_artifact(
            PipelineCreationKind::RenderPipeline,
            key.stable_name,
            key.stable_hash(),
            phase,
        )
    }

    pub fn ensure_compute_pipeline(
        &mut self,
        key: PipelineCacheKey,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.ensure_artifact(
            PipelineCreationKind::ComputePipeline,
            key.stable_name,
            key.stable_hash(),
            phase,
        )
    }

    #[must_use]
    pub fn resolve_pipeline_or_debug_material(
        &mut self,
        key: PipelineCacheKey,
        debug_material_pipeline: PreparedPipelineId,
        reason: MissingPipelineVariantReason,
    ) -> PipelineVariantResolution {
        if let Some(pipeline) = self.find_prepared_pipeline(key) {
            self.telemetry.cache_hits = self.telemetry.cache_hits.saturating_add(1);
            return PipelineVariantResolution {
                pipeline,
                used_debug_material: false,
                failure: None,
            };
        }

        self.telemetry.cache_misses = self.telemetry.cache_misses.saturating_add(1);
        self.telemetry.missing_variant_fallbacks =
            self.telemetry.missing_variant_fallbacks.saturating_add(1);
        self.telemetry.debug_material_draws = self.telemetry.debug_material_draws.saturating_add(1);
        PipelineVariantResolution {
            pipeline: debug_material_pipeline,
            used_debug_material: true,
            failure: Some(MissingPipelineVariantFailure {
                requested_pipeline: key.stable_name,
                fallback_pipeline: debug_material_pipeline,
                reason,
            }),
        }
    }

    fn ensure_artifact(
        &mut self,
        kind: PipelineCreationKind,
        stable_name: &'static str,
        fingerprint: DescriptorFingerprint,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.kind == kind && entry.fingerprint == fingerprint)
        {
            self.telemetry.cache_hits = self.telemetry.cache_hits.saturating_add(1);
            return Ok(PipelineCacheDecision {
                kind,
                stable_name,
                fingerprint,
                prepared_pipeline: entry.prepared_pipeline,
                created: false,
            });
        }

        self.telemetry.cache_misses = self.telemetry.cache_misses.saturating_add(1);
        if !phase.allows_creation() {
            self.telemetry.runtime_creation_failures =
                self.telemetry.runtime_creation_failures.saturating_add(1);
            return Err(PipelineCacheError::RuntimeCreationAfterWarmup { kind, stable_name });
        }

        let prepared_pipeline = PreparedPipelineId::first(self.entries.len() as u32);
        self.entries.push(PipelineCacheEntry {
            kind,
            stable_name,
            fingerprint,
            prepared_pipeline,
        });
        self.entries
            .sort_by_key(|entry| (entry.kind as u8, entry.fingerprint.0));
        match kind {
            PipelineCreationKind::ShaderModule => {
                self.telemetry.shader_modules_created =
                    self.telemetry.shader_modules_created.saturating_add(1);
            }
            PipelineCreationKind::PipelineLayout => {
                self.telemetry.pipeline_layouts_created =
                    self.telemetry.pipeline_layouts_created.saturating_add(1);
            }
            PipelineCreationKind::RenderPipeline => {
                self.telemetry.render_pipelines_created =
                    self.telemetry.render_pipelines_created.saturating_add(1);
            }
            PipelineCreationKind::ComputePipeline => {
                self.telemetry.compute_pipelines_created =
                    self.telemetry.compute_pipelines_created.saturating_add(1);
            }
        }
        Ok(PipelineCacheDecision {
            kind,
            stable_name,
            fingerprint,
            prepared_pipeline,
            created: true,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PipelineWarmupPassKind {
    Depth,
    Opaque,
    AlphaTest,
    Transparent,
    Shadow,
    Sky,
    Ui,
    Post,
    DebugViews,
    Compute,
}

impl PipelineWarmupPassKind {
    pub const ALL: [Self; 10] = [
        Self::Depth,
        Self::Opaque,
        Self::AlphaTest,
        Self::Transparent,
        Self::Shadow,
        Self::Sky,
        Self::Ui,
        Self::Post,
        Self::DebugViews,
        Self::Compute,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Depth => "depth",
            Self::Opaque => "opaque",
            Self::AlphaTest => "alpha_test",
            Self::Transparent => "transparent",
            Self::Shadow => "shadow",
            Self::Sky => "sky",
            Self::Ui => "ui",
            Self::Post => "post",
            Self::DebugViews => "debug_views",
            Self::Compute => "compute",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineWarmupManifestEntry {
    pub pass_kind: PipelineWarmupPassKind,
    pub pipeline_static_label: &'static str,
    pub required_before_measured_frames: bool,
    pub feature_mask: PipelineFeatureMask,
    pub quality_tier_mask: PipelineQualityTierMask,
    pub backend_mask: PipelineBackendMask,
}

pub const PASS12_WARMUP_MANIFEST_ENTRIES: [PipelineWarmupManifestEntry; 10] = [
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Depth,
        pipeline_static_label: "renderer_virtual_geometry_static_cluster_pipeline",
        required_before_measured_frames: true,
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Opaque,
        pipeline_static_label: "renderer_virtual_geometry_static_cluster_pipeline",
        required_before_measured_frames: true,
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::AlphaTest,
        pipeline_static_label: "renderer_virtual_geometry_skinned_cluster_pipeline",
        required_before_measured_frames: true,
        feature_mask: VIRTUAL_GEOMETRY_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Transparent,
        pipeline_static_label: "fun_cloud_view_composite_pipeline",
        required_before_measured_frames: true,
        feature_mask: CLOUD_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Shadow,
        pipeline_static_label: "renderer_virtual_shadow_directional_pages_pipeline",
        required_before_measured_frames: true,
        feature_mask: VIRTUAL_SHADOW_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Sky,
        pipeline_static_label: "fun_cloud_view_composite_pipeline",
        required_before_measured_frames: true,
        feature_mask: CLOUD_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Ui,
        pipeline_static_label: "renderer_cef_gpu_composite_pipeline",
        required_before_measured_frames: true,
        feature_mask: CEF_UI_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Post,
        pipeline_static_label: "renderer_fsr_sr_present_boundary_pipeline",
        required_before_measured_frames: true,
        feature_mask: UPSCALE_FEATURES.union(PipelineFeatureMask::FSR),
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::DebugViews,
        pipeline_static_label: "fun_cloud_debug_pipeline",
        required_before_measured_frames: true,
        feature_mask: CLOUD_FEATURES.union(PipelineFeatureMask::DEBUG_OVERLAY),
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
    PipelineWarmupManifestEntry {
        pass_kind: PipelineWarmupPassKind::Compute,
        pipeline_static_label: "fun_compute_culling_instance_frustum_pipeline",
        required_before_measured_frames: true,
        feature_mask: COMPUTE_CULLING_FEATURES,
        quality_tier_mask: PipelineQualityTierMask::ALL,
        backend_mask: PipelineBackendMask::ALL,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineWarmupManifest {
    pub entries: &'static [PipelineWarmupManifestEntry],
}

impl PipelineWarmupManifest {
    pub const PASS12: Self = Self {
        entries: &PASS12_WARMUP_MANIFEST_ENTRIES,
    };

    #[must_use]
    pub const fn pass12_static() -> Self {
        Self::PASS12
    }

    #[must_use]
    pub fn validate_against_registry(
        self,
        registry: &PipelineRegistry,
    ) -> PipelineWarmupManifestReport {
        let mut missing_registry_labels = 0;
        let mut first_missing_label = None;
        for entry in self.entries {
            if registry
                .find_pipeline_by_label(entry.pipeline_static_label)
                .is_none()
            {
                missing_registry_labels += 1;
                if first_missing_label.is_none() {
                    first_missing_label = Some(entry.pipeline_static_label);
                }
            }
        }

        let mut missing_required_pass_kinds = 0;
        let mut first_missing_pass_kind = None;
        for pass_kind in PipelineWarmupPassKind::ALL {
            let covered = self
                .entries
                .iter()
                .any(|entry| entry.pass_kind == pass_kind && entry.required_before_measured_frames);
            if !covered {
                missing_required_pass_kinds += 1;
                if first_missing_pass_kind.is_none() {
                    first_missing_pass_kind = Some(pass_kind);
                }
            }
        }

        PipelineWarmupManifestReport {
            entry_count: self.entries.len(),
            missing_registry_labels,
            missing_required_pass_kinds,
            first_missing_label,
            first_missing_pass_kind,
        }
    }
}

impl Default for PipelineWarmupManifest {
    fn default() -> Self {
        Self::PASS12
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineWarmupManifestReport {
    pub entry_count: usize,
    pub missing_registry_labels: usize,
    pub missing_required_pass_kinds: usize,
    pub first_missing_label: Option<&'static str>,
    pub first_missing_pass_kind: Option<PipelineWarmupPassKind>,
}

impl PipelineWarmupManifestReport {
    #[must_use]
    pub const fn passes(self) -> bool {
        self.missing_registry_labels == 0 && self.missing_required_pass_kinds == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_registry_contains_compute_culling_warmup_descriptors() {
        let registry = PipelineRegistry::default();

        for label in [
            "fun_compute_culling_reset_pipeline",
            "fun_compute_culling_instance_frustum_pipeline",
            "fun_compute_culling_lod_select_pipeline",
            "fun_compute_culling_meshlet_cluster_pipeline",
            "fun_compute_culling_hiz_occlusion_pipeline",
            "fun_compute_culling_compaction_pipeline",
            "fun_compute_culling_indirect_args_pipeline",
        ] {
            let descriptor = registry
                .find_pipeline_by_label(label)
                .unwrap_or_else(|| panic!("missing registered pipeline descriptor: {label}"));
            assert_eq!(descriptor.kind, PipelineKind::Compute);
            assert_eq!(
                descriptor.shader_path,
                "fun_render/src/compute_culling.wgsl"
            );
            assert!(
                descriptor
                    .warmup_policy
                    .contains_boundary(PipelineWarmupBoundary::RendererInitialization)
            );
            assert!(!descriptor.runtime_creation_allowed_during_benchmark);
        }
    }

    #[test]
    fn pipeline_registry_covers_render_compute_shader_and_dependencies() {
        let registry = PipelineRegistry::default();

        assert!(registry.pipeline_count() >= 20);
        assert!(registry.shader_module_count() >= 8);
        assert!(registry.shader_variant_count() >= 10);
        assert!(
            registry
                .pipelines
                .iter()
                .any(|descriptor| descriptor.kind == PipelineKind::Render)
        );
        assert!(
            registry
                .pipelines
                .iter()
                .any(|descriptor| descriptor.kind == PipelineKind::Compute)
        );
        assert!(registry.variant_axes_are_constrained());
        assert!(registry.pipelines.iter().all(|descriptor| {
            descriptor.backend_mask != PipelineBackendMask::NONE
                && descriptor.quality_tier_mask != PipelineQualityTierMask::NONE
                && !descriptor.static_label.is_empty()
                && !descriptor.shader_path.is_empty()
                && !descriptor.pass_dependencies.is_empty()
        }));
    }

    #[test]
    fn pipeline_warmup_plan_filters_by_backend_boundary_and_feature_mask() {
        let registry = PipelineRegistry::default();
        let init_dx12 = registry.warmup_plan(PipelineWarmupRequest::renderer_initialization(
            FunRendererBackend::Dx12,
        ));
        let init_vulkan = registry.warmup_plan(PipelineWarmupRequest::renderer_initialization(
            FunRendererBackend::Vulkan,
        ));
        let init_metal = registry.warmup_plan(PipelineWarmupRequest::renderer_initialization(
            FunRendererBackend::Metal,
        ));
        let scene_load = registry.warmup_plan(PipelineWarmupRequest {
            boundary: PipelineWarmupBoundary::SceneLoad,
            backend: FunRendererBackend::Dx12,
            quality_tier: QualityTier::Balanced,
            features: PipelineFeatureMask::ALL,
        });
        let minimal_scene_load = registry.warmup_plan(PipelineWarmupRequest {
            features: PipelineFeatureMask::CLOUDS,
            ..PipelineWarmupRequest {
                boundary: PipelineWarmupBoundary::SceneLoad,
                backend: FunRendererBackend::Dx12,
                quality_tier: QualityTier::Balanced,
                features: PipelineFeatureMask::ALL,
            }
        });

        assert!(init_dx12.eligible_pipeline_count >= 10);
        assert!(init_dx12.eligible_pipeline_count > init_vulkan.eligible_pipeline_count);
        assert_eq!(
            init_vulkan.eligible_pipeline_count,
            init_metal.eligible_pipeline_count
        );
        assert_eq!(
            init_dx12.first_pipeline_label,
            Some("fun_compute_culling_reset_pipeline")
        );
        assert!(scene_load.eligible_pipeline_count >= 7);
        assert!(minimal_scene_load.eligible_pipeline_count < scene_load.eligible_pipeline_count);
        assert_eq!(
            PipelineWarmupBoundary::ALL.len(),
            [
                PipelineWarmupBoundary::RendererInitialization,
                PipelineWarmupBoundary::SceneLoad,
                PipelineWarmupBoundary::QualityTierChange,
                PipelineWarmupBoundary::BackendChange,
                PipelineWarmupBoundary::MaterialShaderCatalogChange,
            ]
            .len()
        );
    }

    #[test]
    fn shader_variant_axes_reject_unregistered_bits() {
        let allowed = ShaderVariantAxes::BACKEND.union(ShaderVariantAxes::MSAA);
        let stringly_axis = ShaderVariantAxes {
            bits: ShaderVariantAxes::ALL_ALLOWED_BITS | (1 << 15),
        };

        assert!(allowed.has_only_allowed_axes());
        assert!(!stringly_axis.has_only_allowed_axes());
        assert!(
            ShaderVariantAxis::ALL_ALLOWED
                .iter()
                .all(|axis| ShaderVariantAxes::ALL_ALLOWED_BITS & axis.bit() != 0)
        );
    }

    #[test]
    fn runtime_creation_audit_names_exact_pipeline_label() {
        let registry = PipelineRegistry::default();
        let audit = registry.audit_runtime_creation(&[
            PipelineRuntimeCreationSample {
                kind: RuntimeCreationKind::ComputePipeline,
                static_label: "fun_compute_culling_lod_select_pipeline",
                count: 1,
                benchmark_window: true,
            },
            PipelineRuntimeCreationSample {
                kind: RuntimeCreationKind::ShaderModule,
                static_label: "unknown_runtime_shader",
                count: 1,
                benchmark_window: true,
            },
        ]);

        assert!(!audit.passes_perf_gate());
        assert_eq!(audit.unexpected_creation_count, 2);
        assert_eq!(audit.unknown_label_count, 1);
        assert_eq!(
            audit.first_unexpected_label,
            Some("fun_compute_culling_lod_select_pipeline")
        );
    }

    #[test]
    fn pipeline_cache_key_tracks_pass12_pso_dimensions() {
        let base = render_pipeline_desc("test.render.pipeline", 1, TextureFormat::Depth32Float);
        let entries = test_render_entries();
        let vertex_layout = VertexLayout {
            stride_bytes: 32,
            attribute_count: 3,
            attribute_signature: DescriptorFingerprint::from_u64(30),
            step_mode: VertexStepMode::Vertex,
        };
        let key = PipelineCacheKey::from_render_desc(RenderPipelineKeyDesc {
            pipeline: base,
            shader_entries: entries,
            reflection_signature: DescriptorFingerprint::from_u64(10),
            bind_layout_hash: DescriptorFingerprint::from_u64(20),
            blend_state: BlendState::OPAQUE,
            vertex_layout,
            quality_tier: QualityTier::Balanced,
            feature_mask: PipelineFeatureMask::VIRTUAL_GEOMETRY,
            backend: FunRendererBackend::Dx12,
        });
        let mut msaa_desc = base;
        msaa_desc.sample_count = 4;
        let msaa_key = PipelineCacheKey::from_render_desc(RenderPipelineKeyDesc {
            pipeline: msaa_desc,
            shader_entries: entries,
            reflection_signature: DescriptorFingerprint::from_u64(10),
            bind_layout_hash: DescriptorFingerprint::from_u64(20),
            blend_state: BlendState::OPAQUE,
            vertex_layout,
            quality_tier: QualityTier::Balanced,
            feature_mask: PipelineFeatureMask::VIRTUAL_GEOMETRY,
            backend: FunRendererBackend::Dx12,
        });
        let vulkan_key = PipelineCacheKey {
            backend: FunRendererBackend::Vulkan,
            ..key
        };
        let feature_key = PipelineCacheKey {
            feature_mask: PipelineFeatureMask::VIRTUAL_GEOMETRY
                .union(PipelineFeatureMask::DEBUG_OVERLAY),
            ..key
        };

        assert_ne!(key.stable_hash(), msaa_key.stable_hash());
        assert_ne!(key.stable_hash(), vulkan_key.stable_hash());
        assert_ne!(key.stable_hash(), feature_key.stable_hash());
    }

    #[test]
    fn pipeline_cache_reuses_prepared_handles_after_warmup() {
        let mut cache = PipelineCache::default();
        let key = PipelineCacheKey::from_render_desc(RenderPipelineKeyDesc {
            pipeline: render_pipeline_desc("test.reused.render", 1, TextureFormat::Depth32Float),
            shader_entries: test_render_entries(),
            reflection_signature: DescriptorFingerprint::from_u64(40),
            bind_layout_hash: DescriptorFingerprint::from_u64(41),
            blend_state: BlendState::OPAQUE,
            vertex_layout: VertexLayout::EMPTY,
            quality_tier: QualityTier::Balanced,
            feature_mask: PipelineFeatureMask::NONE,
            backend: FunRendererBackend::Dx12,
        });
        let warmup = cache
            .ensure_render_pipeline(key, PipelineCreationPhase::Warmup)
            .expect("warmup creates the pipeline");
        let runtime = cache
            .ensure_render_pipeline(key, PipelineCreationPhase::RuntimeMeasured)
            .expect("runtime reuses prepared pipeline");

        assert!(warmup.created);
        assert!(!runtime.created);
        assert_eq!(warmup.prepared_pipeline, runtime.prepared_pipeline);
        assert_eq!(cache.telemetry.render_pipelines_created, 1);
        assert_eq!(cache.telemetry.cache_hits, 1);
    }

    #[test]
    fn pipeline_cache_blocks_measured_runtime_creation_for_all_artifacts() {
        let mut cache = PipelineCache::default();
        let render_key = PipelineCacheKey::from_render_desc(RenderPipelineKeyDesc {
            pipeline: render_pipeline_desc("test.runtime.render", 1, TextureFormat::Depth32Float),
            shader_entries: test_render_entries(),
            reflection_signature: DescriptorFingerprint::from_u64(50),
            bind_layout_hash: DescriptorFingerprint::from_u64(51),
            blend_state: BlendState::OPAQUE,
            vertex_layout: VertexLayout::EMPTY,
            quality_tier: QualityTier::Balanced,
            feature_mask: PipelineFeatureMask::NONE,
            backend: FunRendererBackend::Dx12,
        });
        let compute_key = PipelineCacheKey::from_compute_desc(
            compute_pipeline_desc("test.runtime.compute"),
            ShaderEntrySet::compute(ShaderEntry {
                shader_hash: 52,
                module_label: "test.compute.module",
                entry_point: "main",
            }),
            DescriptorFingerprint::from_u64(53),
            DescriptorFingerprint::from_u64(54),
            QualityTier::Balanced,
            PipelineFeatureMask::COMPUTE_CULLING,
            FunRendererBackend::Dx12,
        );

        assert_eq!(
            cache.ensure_shader_module(
                "test.runtime.shader",
                DescriptorFingerprint::from_u64(55),
                PipelineCreationPhase::RuntimeMeasured,
            ),
            Err(PipelineCacheError::RuntimeCreationAfterWarmup {
                kind: PipelineCreationKind::ShaderModule,
                stable_name: "test.runtime.shader",
            })
        );
        assert_eq!(
            cache.ensure_pipeline_layout(
                "test.runtime.layout",
                DescriptorFingerprint::from_u64(56),
                PipelineCreationPhase::RuntimeMeasured,
            ),
            Err(PipelineCacheError::RuntimeCreationAfterWarmup {
                kind: PipelineCreationKind::PipelineLayout,
                stable_name: "test.runtime.layout",
            })
        );
        assert_eq!(
            cache.ensure_render_pipeline(render_key, PipelineCreationPhase::RuntimeMeasured),
            Err(PipelineCacheError::RuntimeCreationAfterWarmup {
                kind: PipelineCreationKind::RenderPipeline,
                stable_name: "test.runtime.render",
            })
        );
        assert_eq!(
            cache.ensure_compute_pipeline(compute_key, PipelineCreationPhase::RuntimeMeasured),
            Err(PipelineCacheError::RuntimeCreationAfterWarmup {
                kind: PipelineCreationKind::ComputePipeline,
                stable_name: "test.runtime.compute",
            })
        );
        assert_eq!(cache.telemetry.runtime_creation_failures, 4);
    }

    #[test]
    fn missing_pipeline_variant_resolves_to_debug_material_with_named_failure() {
        let mut cache = PipelineCache::default();
        let fallback = PreparedPipelineId::first(99);
        let key = PipelineCacheKey::from_render_desc(RenderPipelineKeyDesc {
            pipeline: render_pipeline_desc("test.missing.variant", 1, TextureFormat::Depth32Float),
            shader_entries: test_render_entries(),
            reflection_signature: DescriptorFingerprint::from_u64(60),
            bind_layout_hash: DescriptorFingerprint::from_u64(61),
            blend_state: BlendState::OPAQUE,
            vertex_layout: VertexLayout::EMPTY,
            quality_tier: QualityTier::Cinematic,
            feature_mask: PipelineFeatureMask::DEBUG_OVERLAY,
            backend: FunRendererBackend::Dx12,
        });
        let resolution = cache.resolve_pipeline_or_debug_material(
            key,
            fallback,
            MissingPipelineVariantReason::NotPreparedDuringWarmup,
        );

        assert_eq!(resolution.pipeline, fallback);
        assert!(resolution.used_debug_material);
        assert_eq!(
            resolution
                .failure
                .expect("debug fallback should report the missing variant")
                .requested_pipeline,
            "test.missing.variant"
        );
        assert_eq!(cache.telemetry.missing_variant_fallbacks, 1);
        assert_eq!(cache.telemetry.debug_material_draws, 1);
    }

    #[test]
    fn pass12_warmup_manifest_covers_required_render_compute_passes() {
        let registry = PipelineRegistry::default();
        let manifest = PipelineWarmupManifest::pass12_static();
        let report = manifest.validate_against_registry(&registry);

        assert!(report.passes());
        assert_eq!(report.entry_count, PipelineWarmupPassKind::ALL.len());
        for pass_kind in PipelineWarmupPassKind::ALL {
            assert!(
                manifest.entries.iter().any(|entry| {
                    entry.pass_kind == pass_kind && entry.required_before_measured_frames
                }),
                "missing pass 12 warmup category {}",
                pass_kind.as_str()
            );
        }
        assert!(manifest.entries.iter().any(|entry| {
            registry
                .find_pipeline_by_label(entry.pipeline_static_label)
                .is_some_and(|descriptor| descriptor.kind == PipelineKind::Render)
        }));
        assert!(manifest.entries.iter().any(|entry| {
            registry
                .find_pipeline_by_label(entry.pipeline_static_label)
                .is_some_and(|descriptor| descriptor.kind == PipelineKind::Compute)
        }));
    }

    fn test_render_entries() -> ShaderEntrySet {
        ShaderEntrySet::render(
            ShaderEntry {
                shader_hash: 1,
                module_label: "test.vertex.module",
                entry_point: "vs_main",
            },
            ShaderEntry {
                shader_hash: 2,
                module_label: "test.fragment.module",
                entry_point: "fs_main",
            },
        )
    }

    fn render_pipeline_desc(
        stable_name: &'static str,
        sample_count: u8,
        depth_format: TextureFormat,
    ) -> RenderPipelineDesc {
        RenderPipelineDesc {
            stable_name,
            color_target_count: 1,
            color_formats: [
                TextureFormat::Rgba16Float,
                TextureFormat::Undefined,
                TextureFormat::Undefined,
                TextureFormat::Undefined,
            ],
            depth_format,
            sample_count,
            ..RenderPipelineDesc::default()
        }
    }

    fn compute_pipeline_desc(stable_name: &'static str) -> ComputePipelineDesc {
        ComputePipelineDesc {
            id: crate::ir::IrComputePipelineId::new(1),
            schema_version: crate::ir::PIPELINE_IR_SCHEMA_VERSION,
            stable_name,
            layout: crate::ir::IrPipelineLayoutId::new(1),
            compute_shader: crate::ir::IrShaderModuleId::new(1),
            requires_work_graphs: false,
        }
    }
}
