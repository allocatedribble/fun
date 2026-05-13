use bevy::{
    pbr::DefaultOpaqueRendererMethod,
    prelude::*,
    render::{
        RenderPlugin,
        backend_capabilities::{RenderBackendCapabilities, RenderCapabilitySupport},
        extract_resource::ExtractResource,
        settings::{Backends, InstanceFlags, RenderCreation, WgpuSettings},
    },
    solari::prelude::{SolariFeaturePolicy, SolariGeometryMode, SolariHairMode, SolariOpacityMode},
    window::{PresentMode, WindowResolution},
};
use std::num::NonZeroU32;
use tracing::{info, warn};

use crate::{
    FunCloudDebugOverlay, FunCloudInternalScale, FunCloudQuality, FunCloudSettings,
    FunWeatherProfileId,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
pub const DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientRenderProfile {
    Default,
    Diagnostics,
}

impl ClientRenderProfile {
    pub fn from_env() -> Self {
        let Ok(value) = std::env::var("FUN_CLIENT_RENDER_PROFILE") else {
            return Self::Default;
        };
        Self::parse(&value).unwrap_or_else(|| {
            warn!(
                target: "fun::render",
                render_profile = value,
                "unknown FUN_CLIENT_RENDER_PROFILE; using default"
            );
            Self::Default
        })
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("default") || value.eq_ignore_ascii_case("debug") {
            return Some(Self::Default);
        }
        if value.eq_ignore_ascii_case("diagnostics")
            || value.eq_ignore_ascii_case("debug_diagnostics")
            || value.eq_ignore_ascii_case("debug+diagnostics")
        {
            return Some(Self::Diagnostics);
        }
        None
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Diagnostics => "diagnostics",
        }
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct ClientRenderConfig {
    pub solari_enabled: bool,
    pub dlss_rr_enabled: bool,
    pub native_dlss: NativeDlssConfig,
    pub meshlets_enabled: bool,
    pub clouds_enabled: bool,
    pub cloud_quality: FunCloudQuality,
    pub cloud_internal_scale: FunCloudInternalScale,
    pub cloud_temporal_enabled: bool,
    pub cloud_shadows_enabled: bool,
    pub cloud_profile_id: FunWeatherProfileId,
    pub cloud_debug_overlay: FunCloudDebugOverlay,
    pub dlss_rr_disabled_by_denoise_mode: bool,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    pub render_profile_verbose: bool,
    pub static_batch_renderer_enabled: bool,
    pub geometry_policy: RenderGeometryPolicy,
    pub meshlet_min_triangles: usize,
    pub rt_features: FunRenderRtFeatures,
    #[cfg(debug_assertions)]
    pub fps_overlay_enabled: bool,
}

impl ClientRenderConfig {
    pub fn from_env() -> Self {
        let cloud_settings = FunCloudSettings::from_env();
        let native_dlss = NativeDlssConfig::from_env();
        Self {
            solari_enabled: std::env::var_os("FUN_ENABLE_SOLARI").is_some()
                && std::env::var_os("FUN_DISABLE_SOLARI").is_none(),
            dlss_rr_enabled: native_dlss.allow_ray_reconstruction
                && std::env::var_os("FUN_DISABLE_DLSS_RR").is_none(),
            native_dlss,
            meshlets_enabled: std::env::var_os("FUN_ENABLE_MESHLETS").is_some()
                && std::env::var_os("FUN_DISABLE_MESHLETS").is_none(),
            clouds_enabled: cloud_settings.enabled,
            cloud_quality: cloud_settings.quality,
            cloud_internal_scale: cloud_settings.internal_scale,
            cloud_temporal_enabled: cloud_settings.temporal_enabled,
            cloud_shadows_enabled: cloud_settings.shadows_enabled,
            cloud_profile_id: cloud_settings.profile_id,
            cloud_debug_overlay: cloud_settings.debug_overlay,
            dlss_rr_disabled_by_denoise_mode: false,
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            render_profile_verbose: std::env::var_os("FUN_RENDER_PROFILE_VERBOSE").is_some(),
            static_batch_renderer_enabled: std::env::var_os("FUN_ENABLE_STATIC_BATCH_RENDERER")
                .is_some()
                && std::env::var_os("FUN_DISABLE_STATIC_BATCH_RENDERER").is_none(),
            geometry_policy: RenderGeometryPolicy::from_env(),
            meshlet_min_triangles: env_usize("FUN_MESHLET_MIN_TRIANGLES", 512),
            rt_features: FunRenderRtFeatures::from_env(),
            #[cfg(debug_assertions)]
            fps_overlay_enabled: std::env::var_os("FUN_ENABLE_FPS_OVERLAY").is_some()
                && std::env::var_os("FUN_DISABLE_FPS_OVERLAY").is_none(),
        }
    }

    pub const fn cloud_settings(self) -> FunCloudSettings {
        FunCloudSettings {
            enabled: self.clouds_enabled,
            quality: self.cloud_quality,
            internal_scale: self.cloud_internal_scale,
            temporal_enabled: self.cloud_temporal_enabled,
            shadows_enabled: self.cloud_shadows_enabled,
            profile_id: self.cloud_profile_id,
            debug_overlay: self.cloud_debug_overlay,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum NativeDlssMode {
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

impl NativeDlssMode {
    pub const DEFAULT: Self = Self::Quality;

    #[must_use]
    pub fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            Some("quality") | Some("q") | None => Self::Quality,
            Some("balanced") | Some("balance") | Some("b") => Self::Balanced,
            Some("performance") | Some("perf") | Some("p") => Self::Performance,
            Some("ultra_performance")
            | Some("ultra-performance")
            | Some("ultraperformance")
            | Some("ultra")
            | Some("up") => Self::UltraPerformance,
            Some(other) => {
                warn!(
                    target: "fun::render",
                    mode = other,
                    "unknown FUN_RENDER_DX12_DLSS_MODE; using quality"
                );
                Self::Quality
            }
        }
    }

    #[must_use]
    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Quality => "quality",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
            Self::UltraPerformance => "ultra_performance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Resource, ExtractResource)]
pub struct NativeDlssConfig {
    pub enabled: bool,
    pub mode: NativeDlssMode,
    pub sharpness: f32,
    pub allow_ray_reconstruction: bool,
    pub force_reset_next_frame: bool,
    pub debug_overlay: bool,
}

impl NativeDlssConfig {
    pub const DEFAULT_SHARPNESS: f32 = 0.0;

    #[must_use]
    pub fn from_env() -> Self {
        Self::from_env_reader(|name| std::env::var(name).ok())
    }

    #[must_use]
    pub fn from_env_reader(mut read: impl FnMut(&'static str) -> Option<String>) -> Self {
        let dlss_enabled = read("FUN_RENDER_DX12_DLSS").or_else(|| read("FUN_DX12_DLSS"));
        let dlss_mode = read("FUN_RENDER_DX12_DLSS_MODE").or_else(|| read("FUN_DX12_DLSS_MODE"));
        let dlss_sharpness =
            read("FUN_RENDER_DX12_DLSS_SHARPNESS").or_else(|| read("FUN_DX12_DLSS_SHARPNESS"));
        let dlss_rr = read("FUN_RENDER_DX12_DLSS_RR").or_else(|| read("FUN_DX12_DLSS_RR"));
        let dlss_reset = read("FUN_RENDER_DX12_DLSS_RESET").or_else(|| read("FUN_DX12_DLSS_RESET"));
        let dlss_debug = read("FUN_RENDER_DX12_DLSS_DEBUG").or_else(|| read("FUN_DX12_DLSS_DEBUG"));

        Self {
            enabled: native_dlss_compiled_for_this_target()
                && native_dlss_enable_value(dlss_enabled.as_deref()),
            mode: NativeDlssMode::from_env_value(dlss_mode.as_deref()),
            sharpness: env_f32_value(
                "FUN_RENDER_DX12_DLSS_SHARPNESS",
                dlss_sharpness.as_deref(),
                Self::DEFAULT_SHARPNESS,
                -1.0,
                1.0,
            ),
            allow_ray_reconstruction: env_bool_value(
                "FUN_RENDER_DX12_DLSS_RR",
                dlss_rr.as_deref(),
                false,
            ),
            force_reset_next_frame: env_bool_value(
                "FUN_RENDER_DX12_DLSS_RESET",
                dlss_reset.as_deref(),
                false,
            ),
            debug_overlay: env_bool_value(
                "FUN_RENDER_DX12_DLSS_DEBUG",
                dlss_debug.as_deref(),
                false,
            ),
        }
    }

    #[must_use]
    pub const fn compiled_for_this_target() -> bool {
        native_dlss_compiled_for_this_target()
    }
}

impl Default for NativeDlssConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: NativeDlssMode::DEFAULT,
            sharpness: Self::DEFAULT_SHARPNESS,
            allow_ray_reconstruction: false,
            force_reset_next_frame: false,
            debug_overlay: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, ExtractResource)]
pub struct FunRenderRtFeatures {
    pub sample_direct: bool,
    pub sample_indirect: bool,
    pub sample_reflections: bool,
    pub surface_cache: bool,
    pub megageom: RtMegaGeometryMode,
    pub opacity_mask: RtOpacityMaskMode,
    pub hair: RtHairMode,
    pub async_readback: bool,
    pub validation: bool,
    pub vendor_emulation: RtVendorEmulation,
}

impl FunRenderRtFeatures {
    pub fn from_env() -> Self {
        Self::from_env_reader(|name| std::env::var(name).ok())
    }

    pub fn from_env_reader(mut read: impl FnMut(&'static str) -> Option<String>) -> Self {
        let vendor_emulation = if env_bool_value(
            "FUN_RENDER_UNKNOWN_VENDOR",
            read("FUN_RENDER_UNKNOWN_VENDOR").as_deref(),
            false,
        ) {
            RtVendorEmulation::Unknown
        } else {
            RtVendorEmulation::from_env_value(read("FUN_RENDER_VENDOR_EMULATION").as_deref())
        };

        Self {
            sample_direct: env_bool_value(
                "FUN_RT_SAMPLE_DIRECT",
                read("FUN_RT_SAMPLE_DIRECT").as_deref(),
                true,
            ),
            sample_indirect: env_bool_value(
                "FUN_RT_SAMPLE_INDIRECT",
                read("FUN_RT_SAMPLE_INDIRECT").as_deref(),
                true,
            ),
            sample_reflections: env_bool_value(
                "FUN_RT_SAMPLE_REFLECTIONS",
                read("FUN_RT_SAMPLE_REFLECTIONS").as_deref(),
                true,
            ),
            surface_cache: env_bool_value(
                "FUN_RT_SURFACE_CACHE",
                read("FUN_RT_SURFACE_CACHE").as_deref(),
                true,
            ),
            megageom: RtMegaGeometryMode::from_env_value(read("FUN_RT_MEGAGEOM").as_deref()),
            opacity_mask: RtOpacityMaskMode::from_env_value(read("FUN_RT_OPACITY_MASK").as_deref()),
            hair: RtHairMode::from_env_value(read("FUN_RT_HAIR").as_deref()),
            async_readback: env_bool_value(
                "FUN_RT_ASYNC_READBACK",
                read("FUN_RT_ASYNC_READBACK").as_deref(),
                false,
            ),
            validation: env_bool_value(
                "FUN_RT_VALIDATION",
                read("FUN_RT_VALIDATION").as_deref(),
                false,
            ),
            vendor_emulation,
        }
    }

    pub fn rt_feature_hash(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash_bool(&mut hash, self.sample_direct);
        hash_bool(&mut hash, self.sample_indirect);
        hash_bool(&mut hash, self.sample_reflections);
        hash_bool(&mut hash, self.surface_cache);
        hash_str(&mut hash, self.megageom.as_env_value());
        hash_str(&mut hash, self.opacity_mask.as_env_value());
        hash_str(&mut hash, self.hair.as_env_value());
        hash_bool(&mut hash, self.async_readback);
        hash_bool(&mut hash, self.validation);
        hash_str(&mut hash, self.vendor_emulation.as_env_value());
        hash
    }

    pub fn capability_hash(self) -> u64 {
        self.rt_feature_hash()
    }

    pub const fn solari_feature_policy(self) -> SolariFeaturePolicy {
        SolariFeaturePolicy {
            direct_lighting: self.sample_direct,
            indirect_lighting: self.sample_indirect,
            reflections: self.sample_reflections,
            surface_cache: self.surface_cache,
            async_readback: self.async_readback,
            validation: self.validation,
            opacity_mode: match self.opacity_mask {
                RtOpacityMaskMode::Off => SolariOpacityMode::Off,
                RtOpacityMaskMode::Baked => SolariOpacityMode::Baked,
                RtOpacityMaskMode::Native => SolariOpacityMode::Native,
            },
            hair_mode: match self.hair {
                RtHairMode::Off => SolariHairMode::Off,
                RtHairMode::Cards => SolariHairMode::Cards,
                RtHairMode::Strands => SolariHairMode::Strands,
                RtHairMode::NativeLss => SolariHairMode::NativeLinearSweptSphere,
            },
            geometry_mode: match self.megageom {
                RtMegaGeometryMode::Off => SolariGeometryMode::Mesh,
                RtMegaGeometryMode::Software => SolariGeometryMode::SoftwareCluster,
                RtMegaGeometryMode::Native => SolariGeometryMode::NativeCluster,
            },
        }
    }

    pub fn log_config(self) {
        info!(
            "[fun render] RT gates: hash={:016x} direct={} indirect={} reflections={} surface_cache={} megageom={} opacity_mask={} hair={} async_readback={} validation={} vendor_emulation={}",
            self.rt_feature_hash(),
            self.sample_direct,
            self.sample_indirect,
            self.sample_reflections,
            self.surface_cache,
            self.megageom.as_env_value(),
            self.opacity_mask.as_env_value(),
            self.hair.as_env_value(),
            self.async_readback,
            self.validation,
            self.vendor_emulation.as_env_value(),
        );
        info!(
            target: "fun::render",
            rt_feature_hash = %format_args!("{:016x}", self.rt_feature_hash()),
            rt_sample_direct = self.sample_direct,
            rt_sample_indirect = self.sample_indirect,
            rt_sample_reflections = self.sample_reflections,
            rt_surface_cache = self.surface_cache,
            rt_megageom = self.megageom.as_env_value(),
            rt_opacity_mask = self.opacity_mask.as_env_value(),
            rt_hair = self.hair.as_env_value(),
            rt_async_readback = self.async_readback,
            rt_validation = self.validation,
            rt_vendor_emulation = self.vendor_emulation.as_env_value(),
            "Fun render RT feature gates"
        );
    }

    pub fn log_backend_fallbacks(self, capabilities: &RenderBackendCapabilities) {
        if self.megageom == RtMegaGeometryMode::Native {
            warn!(
                target: "fun::render",
                requested = self.megageom.as_env_value(),
                backend = capabilities.backend,
                vendor = capabilities.vendor.as_str(),
                "native MegaGeometry backend hook is not implemented; portable RT geometry remains the fallback"
            );
        }
        if self.opacity_mask == RtOpacityMaskMode::Native
            && capabilities.hardware_opacity_micromap != RenderCapabilitySupport::Supported
        {
            warn!(
                target: "fun::render",
                requested = self.opacity_mask.as_env_value(),
                hardware_opacity_micromap = capabilities.hardware_opacity_micromap.as_str(),
                "native opacity micromaps are not enabled by the backend; baked/opaque fallback remains active"
            );
        }
        if self.hair == RtHairMode::NativeLss
            && capabilities.linear_swept_sphere_rt != RenderCapabilitySupport::Supported
        {
            warn!(
                target: "fun::render",
                requested = self.hair.as_env_value(),
                linear_swept_sphere_rt = capabilities.linear_swept_sphere_rt.as_str(),
                "native LSS hair RT is not enabled by the backend; cards/strands fallback remains active"
            );
        }
        if self.async_readback
            && capabilities.async_copy_queue != RenderCapabilitySupport::Supported
        {
            warn!(
                target: "fun::render",
                async_copy_queue = capabilities.async_copy_queue.as_str(),
                "async RT readback requested without a proven async copy queue; graphics-queue readback fallback remains active"
            );
        }
        if self.validation
            && capabilities.native_rt_validation != RenderCapabilitySupport::Supported
        {
            warn!(
                target: "fun::render",
                native_rt_validation = capabilities.native_rt_validation.as_str(),
                "native RT validation requested without a backend hook; portable validation checks remain the fallback"
            );
        }
    }
}

impl Default for FunRenderRtFeatures {
    fn default() -> Self {
        Self {
            sample_direct: true,
            sample_indirect: true,
            sample_reflections: true,
            surface_cache: true,
            megageom: RtMegaGeometryMode::Off,
            opacity_mask: RtOpacityMaskMode::Off,
            hair: RtHairMode::Off,
            async_readback: false,
            validation: false,
            vendor_emulation: RtVendorEmulation::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtMegaGeometryMode {
    Off,
    Software,
    Native,
}

impl RtMegaGeometryMode {
    fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            None | Some("") | Some("off") => Self::Off,
            Some("software") | Some("portable") => Self::Software,
            Some("native") => Self::Native,
            Some(unknown) => {
                warn!(target: "fun::render", value = unknown, "unknown FUN_RT_MEGAGEOM; using off");
                Self::Off
            }
        }
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Software => "software",
            Self::Native => "native",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtOpacityMaskMode {
    Off,
    Baked,
    Native,
}

impl RtOpacityMaskMode {
    fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            None | Some("") | Some("off") => Self::Off,
            Some("baked") | Some("software") | Some("portable") => Self::Baked,
            Some("native") => Self::Native,
            Some(unknown) => {
                warn!(
                    target: "fun::render",
                    value = unknown,
                    "unknown FUN_RT_OPACITY_MASK; using off"
                );
                Self::Off
            }
        }
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Baked => "baked",
            Self::Native => "native",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtHairMode {
    Off,
    Cards,
    Strands,
    NativeLss,
}

impl RtHairMode {
    fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            None | Some("") | Some("off") => Self::Off,
            Some("cards") => Self::Cards,
            Some("strands") => Self::Strands,
            Some("native_lss") | Some("native-lss") | Some("lss") => Self::NativeLss,
            Some(unknown) => {
                warn!(target: "fun::render", value = unknown, "unknown FUN_RT_HAIR; using off");
                Self::Off
            }
        }
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Cards => "cards",
            Self::Strands => "strands",
            Self::NativeLss => "native_lss",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtVendorEmulation {
    Auto,
    Unknown,
    Nvidia,
    Amd,
    Intel,
}

impl RtVendorEmulation {
    fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            None | Some("") | Some("auto") | Some("actual") | Some("native") => Self::Auto,
            Some("unknown") => Self::Unknown,
            Some("nvidia") | Some("nv") => Self::Nvidia,
            Some("amd") | Some("radeon") => Self::Amd,
            Some("intel") => Self::Intel,
            Some(unknown) => {
                warn!(
                    target: "fun::render",
                    value = unknown,
                    "unknown FUN_RENDER_VENDOR_EMULATION; using actual backend vendor"
                );
                Self::Auto
            }
        }
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Unknown => "unknown",
            Self::Nvidia => "nvidia",
            Self::Amd => "amd",
            Self::Intel => "intel",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderGeometryPolicy {
    Hybrid,
    RasterOnly,
    GpuCulledRaster,
    MeshletWhereSupported,
    VirtualStaticExperimental,
}

impl RenderGeometryPolicy {
    pub fn from_env() -> Self {
        Self::from_env_value(std::env::var("FUN_RENDER_GEOMETRY_POLICY").ok().as_deref())
    }

    fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::to_ascii_lowercase).as_deref() {
            None | Some("") | Some("hybrid") => Self::Hybrid,
            Some("raster") | Some("raster_only") | Some("raster-only") | Some("all_raster")
            | Some("all-raster") => Self::RasterOnly,
            Some("gpu")
            | Some("gpu_culled")
            | Some("gpu-culled")
            | Some("gpu_culled_raster")
            | Some("gpu-culled-raster")
            | Some("gpu_culling")
            | Some("gpu-culling") => Self::GpuCulledRaster,
            Some("meshlet")
            | Some("meshlets")
            | Some("meshlet_where_supported")
            | Some("meshlet-where-supported")
            | Some("all_meshlet")
            | Some("all-meshlet") => Self::MeshletWhereSupported,
            Some("virtual_static")
            | Some("virtual-static")
            | Some("virtual_static_experimental")
            | Some("virtual-static-experimental")
            | Some("funvg")
            | Some("fun_vg")
            | Some("fun-vg") => Self::VirtualStaticExperimental,
            Some(other) => {
                warn!(
                    target: "fun::render",
                    policy = other,
                    "unknown FUN_RENDER_GEOMETRY_POLICY; using hybrid"
                );
                Self::Hybrid
            }
        }
    }

    pub const fn as_env_value(self) -> &'static str {
        match self {
            Self::Hybrid => "hybrid",
            Self::RasterOnly => "raster_only",
            Self::GpuCulledRaster => "gpu_culled_raster",
            Self::MeshletWhereSupported => "meshlet_where_supported",
            Self::VirtualStaticExperimental => "virtual_static_experimental",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Component)]
pub enum RenderGeometryClass {
    SimpleRaster,
    InstancedStaticRaster,
    GpuCulledStaticRaster,
    GpuCulledDynamicRaster,
    MeshletStaticDense,
    MeshletDynamicDense,
    VirtualStaticCluster,
    FoliageAggregate,
    TransparentRaster,
    RayProxyOnly,
    Viewmodel,
}

impl RenderGeometryClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SimpleRaster => "simple_raster",
            Self::InstancedStaticRaster => "instanced_static_raster",
            Self::GpuCulledStaticRaster => "gpu_culled_static_raster",
            Self::GpuCulledDynamicRaster => "gpu_culled_dynamic_raster",
            Self::MeshletStaticDense => "meshlet_static_dense",
            Self::MeshletDynamicDense => "meshlet_dynamic_dense",
            Self::VirtualStaticCluster => "virtual_static_cluster",
            Self::FoliageAggregate => "foliage_aggregate",
            Self::TransparentRaster => "transparent_raster",
            Self::RayProxyOnly => "ray_proxy_only",
            Self::Viewmodel => "viewmodel",
        }
    }

    pub const fn uses_meshlet(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::MeshletStaticDense | RenderGeometryClass::MeshletDynamicDense
        )
    }

    pub const fn uses_raster_mesh(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::SimpleRaster
                | RenderGeometryClass::InstancedStaticRaster
                | RenderGeometryClass::GpuCulledStaticRaster
                | RenderGeometryClass::GpuCulledDynamicRaster
                | RenderGeometryClass::FoliageAggregate
                | RenderGeometryClass::TransparentRaster
                | RenderGeometryClass::Viewmodel
        )
    }

    pub const fn uses_gpu_culling(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::GpuCulledStaticRaster
                | RenderGeometryClass::GpuCulledDynamicRaster
                | RenderGeometryClass::VirtualStaticCluster
        )
    }

    pub const fn is_static_world(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::InstancedStaticRaster
                | RenderGeometryClass::GpuCulledStaticRaster
                | RenderGeometryClass::MeshletStaticDense
                | RenderGeometryClass::VirtualStaticCluster
                | RenderGeometryClass::FoliageAggregate
        )
    }
}

#[derive(Debug, Clone, Copy, Resource)]
pub struct ClientWindowConfig {
    pub maximized: bool,
    width: Option<u32>,
    height: Option<u32>,
}

impl ClientWindowConfig {
    pub fn from_env() -> Self {
        Self {
            maximized: std::env::var_os("FUN_WINDOW_MAXIMIZED").is_some(),
            width: env_u32_opt("FUN_WINDOW_WIDTH"),
            height: env_u32_opt("FUN_WINDOW_HEIGHT"),
        }
    }

    pub fn resolution(&self) -> WindowResolution {
        match (self.width, self.height) {
            (Some(width), Some(height)) => WindowResolution::new(width, height),
            _ => WindowResolution::default(),
        }
    }

    pub const fn requested_width(&self) -> Option<u32> {
        self.width
    }

    pub const fn requested_height(&self) -> Option<u32> {
        self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClientOpaqueRenderer {
    Deferred,
    Forward,
}

impl ClientOpaqueRenderer {
    pub(crate) fn method(self) -> DefaultOpaqueRendererMethod {
        match self {
            Self::Deferred => DefaultOpaqueRendererMethod::deferred(),
            Self::Forward => DefaultOpaqueRendererMethod::forward(),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deferred => "deferred",
            Self::Forward => "forward",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunRenderPresentation {
    WinitWindow,
    EditorOffscreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunRenderAppOptions {
    pub runtime_mode: &'static str,
    pub render_profile: ClientRenderProfile,
    pub hosted_by_editor: bool,
    pub presentation: FunRenderPresentation,
}

impl FunRenderAppOptions {
    pub const fn winit_client(
        runtime_mode: &'static str,
        render_profile: ClientRenderProfile,
        hosted_by_editor: bool,
    ) -> Self {
        Self {
            runtime_mode,
            render_profile,
            hosted_by_editor,
            presentation: FunRenderPresentation::WinitWindow,
        }
    }

    pub const fn editor_offscreen(render_profile: ClientRenderProfile) -> Self {
        Self {
            runtime_mode: "editor_static_preview",
            render_profile,
            hosted_by_editor: true,
            presentation: FunRenderPresentation::EditorOffscreen,
        }
    }

    pub fn is_editor_preview(self) -> bool {
        matches!(self.presentation, FunRenderPresentation::EditorOffscreen)
            || self.runtime_mode == "editor_preview"
    }
}

#[cfg_attr(
    not(any(feature = "offscreen", feature = "winit_presentation")),
    allow(dead_code)
)]
pub(crate) fn client_render_creation(render_backend: Backends) -> RenderCreation {
    RenderCreation::Automatic(Box::new(WgpuSettings {
        backends: Some(render_backend),
        instance_flags: InstanceFlags::empty().with_env(),
        ..default()
    }))
}

#[cfg_attr(
    not(any(feature = "offscreen", feature = "winit_presentation")),
    allow(dead_code)
)]
pub(crate) fn render_plugin(render_backend: Backends) -> RenderPlugin {
    RenderPlugin {
        render_creation: client_render_creation(render_backend),
        ..default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderBackendSelectionFacts {
    pub requested_label: &'static str,
    pub selected_backends: Backends,
    pub selection_reason: &'static str,
}

impl RenderBackendSelectionFacts {
    #[must_use]
    pub fn from_env() -> Self {
        let value = std::env::var("FUN_RENDER_BACKEND").ok();
        Self::from_env_value(value.as_deref())
    }

    #[must_use]
    pub fn from_env_value(value: Option<&str>) -> Self {
        let Some(value) = value else {
            return Self {
                requested_label: default_render_backend_label(),
                selected_backends: default_render_backend(),
                selection_reason: "env_missing_default",
            };
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Self {
                requested_label: default_render_backend_label(),
                selected_backends: default_render_backend(),
                selection_reason: "empty_env_default",
            };
        }
        if trimmed.eq_ignore_ascii_case("dx12")
            || trimmed.eq_ignore_ascii_case("d3d12")
            || trimmed.eq_ignore_ascii_case("directx12")
        {
            return Self {
                requested_label: "dx12",
                selected_backends: Backends::DX12,
                selection_reason: "explicit_dx12",
            };
        }
        if trimmed.eq_ignore_ascii_case("vulkan") || trimmed.eq_ignore_ascii_case("vk") {
            return Self {
                requested_label: "vulkan",
                selected_backends: Backends::VULKAN,
                selection_reason: "explicit_vulkan",
            };
        }
        if trimmed.eq_ignore_ascii_case("auto") {
            return Self {
                requested_label: "auto",
                selected_backends: Backends::VULKAN | Backends::DX12,
                selection_reason: "explicit_auto",
            };
        }
        Self {
            requested_label: "invalid",
            selected_backends: default_render_backend(),
            selection_reason: "invalid_env_defaulted",
        }
    }
}

pub fn selected_render_backend() -> Backends {
    match std::env::var("FUN_RENDER_BACKEND") {
        Ok(value) => {
            let facts = RenderBackendSelectionFacts::from_env_value(Some(&value));
            if facts.selection_reason == "invalid_env_defaulted" {
                info!(
                    target: "fun::render",
                    render_backend = value,
                    fallback = default_render_backend_label(),
                    "unknown FUN_RENDER_BACKEND; using platform default"
                );
            }
            facts.selected_backends
        }
        Err(_) => RenderBackendSelectionFacts::from_env_value(None).selected_backends,
    }
}

#[cfg(test)]
fn render_backend_from_env_value(value: Option<&str>) -> Option<Backends> {
    let Some(value) = value else {
        return Some(default_render_backend());
    };
    if value.eq_ignore_ascii_case("dx12")
        || value.eq_ignore_ascii_case("d3d12")
        || value.eq_ignore_ascii_case("directx12")
    {
        Some(Backends::DX12)
    } else if value.eq_ignore_ascii_case("auto") {
        Some(Backends::VULKAN | Backends::DX12)
    } else if value.eq_ignore_ascii_case("vulkan") || value.eq_ignore_ascii_case("vk") {
        Some(Backends::VULKAN)
    } else if value.is_empty() {
        Some(default_render_backend())
    } else {
        None
    }
}

fn default_render_backend() -> Backends {
    if cfg!(target_os = "windows") {
        Backends::DX12
    } else {
        Backends::VULKAN
    }
}

const fn default_render_backend_label() -> &'static str {
    if cfg!(target_os = "windows") {
        "dx12"
    } else {
        "vulkan"
    }
}

pub fn log_native_dlss_startup_diagnostics(render_backend: Backends, config: NativeDlssConfig) {
    info!(
        target: "fun::render",
        compiled = NativeDlssConfig::compiled_for_this_target(),
        backend = ?render_backend,
        runtime_enabled = config.enabled,
        mode = config.mode.as_env_value(),
        sharpness = config.sharpness,
        allow_ray_reconstruction = config.allow_ray_reconstruction,
        debug_overlay = config.debug_overlay,
        "FUN DX12 DLSS startup configuration"
    );
    if !NativeDlssConfig::compiled_for_this_target() {
        info!(
            target: "fun::render",
            compiled = false,
            sdk_runtime_found = false,
            native_handle_extraction_available = false,
            sr_supported = false,
            rr_supported = false,
            "FUN DX12 DLSS native bridge is not compiled"
        );
        return;
    }
    if render_backend != Backends::DX12 {
        info!(
            target: "fun::render",
            backend = ?render_backend,
            required_backend = "dx12",
            "FUN DX12 DLSS disabled because the selected backend is not DX12"
        );
        return;
    }
    #[cfg(all(target_os = "windows", feature = "dx12_dlss_native"))]
    {
        let support = fun_dx12_dlss::query_support_from_env();
        info!(
            target: "fun::render",
            sdk_runtime_found = support.runtime_found,
            native_handle_extraction_available = true,
            sr_supported = support.sr_supported,
            rr_supported = support.rr_supported,
            driver_needs_update = support.needs_updated_driver,
            fallback_reason = support.reason,
            "FUN DX12 DLSS native bridge support query"
        );
    }
    #[cfg(not(all(target_os = "windows", feature = "dx12_dlss_native")))]
    {
        info!(
            target: "fun::render",
            sdk_runtime_found = false,
            native_handle_extraction_available = false,
            sr_supported = false,
            rr_supported = false,
            "FUN DX12 DLSS native bridge pending SDK integration"
        );
    }
}

const fn native_dlss_compiled_for_this_target() -> bool {
    cfg!(all(target_os = "windows", feature = "dx12_dlss_native"))
}

pub fn selected_present_mode() -> PresentMode {
    selected_present_mode_from_env_reader(|name| std::env::var(name).ok())
}

fn selected_present_mode_from_env_reader(
    mut read: impl FnMut(&'static str) -> Option<String>,
) -> PresentMode {
    let Some((name, value)) = read("FUN_RENDER_PRESENT_MODE")
        .map(|value| ("FUN_RENDER_PRESENT_MODE", value))
        .or_else(|| read("FUN_PRESENT_MODE").map(|value| ("FUN_PRESENT_MODE", value)))
    else {
        return PresentMode::Immediate;
    };

    match present_mode_from_env_value(Some(&value)) {
        Some(mode) => mode,
        None => {
            info!("[fun render] unknown {name}={value}; using Immediate");
            PresentMode::Immediate
        }
    }
}

fn present_mode_from_env_value(value: Option<&str>) -> Option<PresentMode> {
    let Some(value) = value else {
        return Some(PresentMode::Immediate);
    };
    match value.to_ascii_lowercase().as_str() {
        "auto_no_vsync" | "autonovsync" | "auto-no-vsync" => Some(PresentMode::AutoNoVsync),
        "auto_vsync" | "autovsync" | "auto-vsync" => Some(PresentMode::AutoVsync),
        "fifo" | "vsync" => Some(PresentMode::Fifo),
        "fifo_relaxed" | "fifo-relaxed" => Some(PresentMode::FifoRelaxed),
        "mailbox" => Some(PresentMode::Mailbox),
        "immediate" | "" => Some(PresentMode::Immediate),
        _ => None,
    }
}

#[cfg_attr(not(feature = "winit_presentation"), allow(dead_code))]
pub fn selected_max_frame_latency() -> NonZeroU32 {
    selected_max_frame_latency_from_env_reader(|name| std::env::var(name).ok())
}

fn selected_max_frame_latency_from_env_reader(
    mut read: impl FnMut(&'static str) -> Option<String>,
) -> NonZeroU32 {
    let Some((name, value)) = read("FUN_RENDER_MAX_FRAME_LATENCY")
        .map(|value| ("FUN_RENDER_MAX_FRAME_LATENCY", value))
        .or_else(|| {
            read("FUN_PRESENT_MAX_FRAME_LATENCY")
                .map(|value| ("FUN_PRESENT_MAX_FRAME_LATENCY", value))
        })
    else {
        return default_max_frame_latency();
    };

    match value.parse::<u32>() {
        Ok(parsed) if parsed > 0 => {
            NonZeroU32::new(parsed).unwrap_or_else(default_max_frame_latency)
        }
        _ => {
            warn!(
                target: "fun::render",
                setting = name,
                value,
                default_value = DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY,
                "ignored invalid maximum frame latency setting"
            );
            default_max_frame_latency()
        }
    }
}

fn default_max_frame_latency() -> NonZeroU32 {
    NonZeroU32::new(DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY)
        .expect("default maximum frame latency must be non-zero")
}

fn env_u32_opt(name: &str) -> Option<u32> {
    let value = std::env::var(name).ok()?;
    match value.parse::<u32>() {
        Ok(parsed) if parsed > 0 => Some(parsed),
        _ => {
            warn!(
                target: "fun::render",
                setting = name,
                value,
                "ignored invalid positive integer setting"
            );
            None
        }
    }
}

fn env_bool_value(name: &'static str, value: Option<&str>, default_value: bool) -> bool {
    let Some(value) = value else {
        return default_value;
    };
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => {
            warn!(
                target: "fun::render",
                setting = name,
                value = value,
                default_value,
                "ignored invalid boolean render setting"
            );
            default_value
        }
    }
}

fn native_dlss_enable_value(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return false;
    };
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" | "auto" => true,
        "0" | "false" | "no" | "off" | "disabled" => false,
        other => {
            warn!(
                target: "fun::render",
                value = other,
                "invalid FUN_RENDER_DX12_DLSS value; disabling native DLSS"
            );
            false
        }
    }
}

fn env_f32_value(
    name: &'static str,
    value: Option<&str>,
    default_value: f32,
    min_value: f32,
    max_value: f32,
) -> f32 {
    let Some(value) = value else {
        return default_value;
    };
    match value.parse::<f32>() {
        Ok(parsed) if parsed.is_finite() && parsed >= min_value && parsed <= max_value => parsed,
        _ => {
            warn!(
                target: "fun::render",
                setting = name,
                value,
                min_value,
                max_value,
                "ignored invalid floating-point setting"
            );
            default_value
        }
    }
}

fn env_usize(name: &str, default_value: usize) -> usize {
    let Ok(value) = std::env::var(name) else {
        return default_value;
    };
    match value.parse::<usize>() {
        Ok(parsed) if parsed > 0 => parsed,
        _ => {
            warn!(
                target: "fun::render",
                setting = name,
                value,
                default_value,
                "ignored invalid positive integer setting"
            );
            default_value
        }
    }
}

fn hash_str(hash: &mut u64, value: &str) {
    for byte in value.as_bytes() {
        hash_byte(hash, *byte);
    }
    hash_byte(hash, 0xff);
}

fn hash_bool(hash: &mut u64, value: bool) {
    hash_byte(hash, u8::from(value));
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(FNV_PRIME);
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY, FunRenderRtFeatures, NativeDlssConfig,
        NativeDlssMode, RenderBackendSelectionFacts, RenderGeometryClass, RenderGeometryPolicy,
        RtHairMode, RtMegaGeometryMode, RtOpacityMaskMode, RtVendorEmulation,
        default_render_backend, present_mode_from_env_value, render_backend_from_env_value,
        selected_max_frame_latency_from_env_reader, selected_present_mode_from_env_reader,
    };
    use bevy::render::settings::Backends;
    use bevy::solari::prelude::{SolariGeometryMode, SolariHairMode, SolariOpacityMode};
    use bevy::window::PresentMode;

    fn features_from_pairs(pairs: &[(&'static str, &'static str)]) -> FunRenderRtFeatures {
        FunRenderRtFeatures::from_env_reader(|name| {
            pairs
                .iter()
                .find_map(|(key, value)| (*key == name).then_some((*value).to_owned()))
        })
    }

    #[test]
    fn geometry_policy_keeps_legacy_aliases() {
        assert_eq!(
            RenderGeometryPolicy::from_env_value(Some("all_raster")),
            RenderGeometryPolicy::RasterOnly
        );
        assert_eq!(
            RenderGeometryPolicy::from_env_value(Some("all-meshlet")),
            RenderGeometryPolicy::MeshletWhereSupported
        );
        assert_eq!(
            RenderGeometryPolicy::from_env_value(Some("gpu_culled_raster")),
            RenderGeometryPolicy::GpuCulledRaster
        );
        assert_eq!(
            RenderGeometryPolicy::from_env_value(Some("virtual_static_experimental"))
                .as_env_value(),
            "virtual_static_experimental"
        );
    }

    #[test]
    fn render_geometry_class_usage_flags_are_stable() {
        assert!(RenderGeometryClass::MeshletStaticDense.uses_meshlet());
        assert!(RenderGeometryClass::MeshletDynamicDense.uses_meshlet());
        assert!(RenderGeometryClass::GpuCulledStaticRaster.uses_raster_mesh());
        assert!(RenderGeometryClass::TransparentRaster.uses_raster_mesh());
        assert!(RenderGeometryClass::VirtualStaticCluster.uses_gpu_culling());
        assert!(!RenderGeometryClass::VirtualStaticCluster.uses_raster_mesh());
        assert!(RenderGeometryClass::VirtualStaticCluster.is_static_world());
        assert_eq!(
            RenderGeometryClass::FoliageAggregate.as_str(),
            "foliage_aggregate"
        );
    }

    #[test]
    fn rt_env_switches_map_to_engine_solari_policy() {
        let features = features_from_pairs(&[
            ("FUN_RT_SAMPLE_DIRECT", "0"),
            ("FUN_RT_SAMPLE_INDIRECT", "1"),
            ("FUN_RT_SAMPLE_REFLECTIONS", "0"),
            ("FUN_RT_SURFACE_CACHE", "0"),
            ("FUN_RT_MEGAGEOM", "software"),
            ("FUN_RT_OPACITY_MASK", "baked"),
            ("FUN_RT_HAIR", "strands"),
            ("FUN_RT_ASYNC_READBACK", "1"),
            ("FUN_RT_VALIDATION", "1"),
            ("FUN_RENDER_VENDOR_EMULATION", "nvidia"),
        ]);

        assert!(!features.sample_direct);
        assert!(features.sample_indirect);
        assert!(!features.sample_reflections);
        assert!(!features.surface_cache);
        assert_eq!(features.megageom, RtMegaGeometryMode::Software);
        assert_eq!(features.opacity_mask, RtOpacityMaskMode::Baked);
        assert_eq!(features.hair, RtHairMode::Strands);
        assert!(features.async_readback);
        assert!(features.validation);
        assert_eq!(features.vendor_emulation, RtVendorEmulation::Nvidia);

        let policy = features.solari_feature_policy();
        assert!(!policy.direct_lighting);
        assert!(policy.indirect_lighting);
        assert!(!policy.reflections);
        assert!(!policy.surface_cache);
        assert!(policy.async_readback);
        assert!(policy.validation);
        assert_eq!(policy.geometry_mode, SolariGeometryMode::SoftwareCluster);
        assert_eq!(policy.opacity_mode, SolariOpacityMode::Baked);
        assert_eq!(policy.hair_mode, SolariHairMode::Strands);
    }

    #[test]
    fn native_rt_env_modes_map_to_native_engine_policy() {
        let policy = features_from_pairs(&[
            ("FUN_RT_MEGAGEOM", "native"),
            ("FUN_RT_OPACITY_MASK", "native"),
            ("FUN_RT_HAIR", "native_lss"),
            ("FUN_RT_ASYNC_READBACK", "1"),
        ])
        .solari_feature_policy();

        assert_eq!(policy.geometry_mode, SolariGeometryMode::NativeCluster);
        assert_eq!(policy.opacity_mode, SolariOpacityMode::Native);
        assert_eq!(policy.hair_mode, SolariHairMode::NativeLinearSweptSphere);
        assert!(policy.async_readback);
    }

    #[test]
    fn unknown_vendor_flag_overrides_vendor_emulation_value() {
        let features = features_from_pairs(&[
            ("FUN_RENDER_UNKNOWN_VENDOR", "1"),
            ("FUN_RENDER_VENDOR_EMULATION", "amd"),
        ]);

        assert_eq!(features.vendor_emulation, RtVendorEmulation::Unknown);
    }

    #[test]
    fn invalid_boolean_rt_env_switch_uses_default() {
        let features = features_from_pairs(&[
            ("FUN_RT_SAMPLE_DIRECT", "definitely"),
            ("FUN_RT_ASYNC_READBACK", "definitely"),
        ]);

        assert!(features.sample_direct);
        assert!(!features.async_readback);
    }

    #[test]
    fn backend_selection_uses_platform_default_and_preserves_explicit_backends() {
        assert_eq!(
            render_backend_from_env_value(None),
            Some(default_render_backend())
        );
        assert_eq!(
            render_backend_from_env_value(Some("")),
            Some(default_render_backend())
        );
        assert_eq!(
            render_backend_from_env_value(Some("vulkan")),
            Some(Backends::VULKAN)
        );
        assert_eq!(
            render_backend_from_env_value(Some("dx12")),
            Some(Backends::DX12)
        );
        assert_eq!(
            render_backend_from_env_value(Some("d3d12")),
            Some(Backends::DX12)
        );
        assert_eq!(
            render_backend_from_env_value(Some("directx12")),
            Some(Backends::DX12)
        );
        assert_eq!(
            render_backend_from_env_value(Some("auto")),
            Some(Backends::VULKAN | Backends::DX12)
        );
        assert_eq!(render_backend_from_env_value(Some("bad")), None);
    }

    #[test]
    fn backend_selection_facts_keep_requested_and_selected_separate() {
        let invalid = RenderBackendSelectionFacts::from_env_value(Some("definitely"));
        assert_eq!(invalid.requested_label, "invalid");
        assert_eq!(invalid.selected_backends, default_render_backend());
        assert_eq!(invalid.selection_reason, "invalid_env_defaulted");

        let automatic = RenderBackendSelectionFacts::from_env_value(Some("auto"));
        assert_eq!(automatic.requested_label, "auto");
        assert_eq!(
            automatic.selected_backends,
            Backends::VULKAN | Backends::DX12
        );
        assert_eq!(automatic.selection_reason, "explicit_auto");
    }

    #[test]
    fn backend_selection_prefers_dx12_by_default_on_windows() {
        #[cfg(target_os = "windows")]
        assert_eq!(default_render_backend(), Backends::DX12);

        #[cfg(not(target_os = "windows"))]
        assert_eq!(default_render_backend(), Backends::VULKAN);
    }

    #[test]
    fn present_mode_selection_preserves_legacy_alias_and_prefers_render_env() {
        assert_eq!(
            present_mode_from_env_value(None),
            Some(PresentMode::Immediate)
        );
        assert_eq!(
            present_mode_from_env_value(Some("auto_no_vsync")),
            Some(PresentMode::AutoNoVsync)
        );
        assert_eq!(
            present_mode_from_env_value(Some("auto-vsync")),
            Some(PresentMode::AutoVsync)
        );
        assert_eq!(
            present_mode_from_env_value(Some("fifo")),
            Some(PresentMode::Fifo)
        );
        assert_eq!(
            present_mode_from_env_value(Some("fifo_relaxed")),
            Some(PresentMode::FifoRelaxed)
        );
        assert_eq!(
            present_mode_from_env_value(Some("mailbox")),
            Some(PresentMode::Mailbox)
        );
        assert_eq!(present_mode_from_env_value(Some("bad")), None);

        let selected = selected_present_mode_from_env_reader(|name| {
            match name {
                "FUN_RENDER_PRESENT_MODE" => Some("auto_no_vsync"),
                "FUN_PRESENT_MODE" => Some("fifo"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!(selected, PresentMode::AutoNoVsync);

        let legacy = selected_present_mode_from_env_reader(|name| {
            match name {
                "FUN_PRESENT_MODE" => Some("fifo"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!(legacy, PresentMode::Fifo);
    }

    #[test]
    fn maximum_frame_latency_uses_render_env_then_legacy_env() {
        let default = selected_max_frame_latency_from_env_reader(|_| None);
        assert_eq!(default.get(), DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY);

        let selected = selected_max_frame_latency_from_env_reader(|name| {
            match name {
                "FUN_RENDER_MAX_FRAME_LATENCY" => Some("2"),
                "FUN_PRESENT_MAX_FRAME_LATENCY" => Some("4"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!(selected.get(), 2);

        let legacy = selected_max_frame_latency_from_env_reader(|name| {
            match name {
                "FUN_PRESENT_MAX_FRAME_LATENCY" => Some("4"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!(legacy.get(), 4);

        let invalid = selected_max_frame_latency_from_env_reader(|name| {
            match name {
                "FUN_RENDER_MAX_FRAME_LATENCY" => Some("0"),
                _ => None,
            }
            .map(str::to_owned)
        });
        assert_eq!(invalid.get(), DEFAULT_DESIRED_MAXIMUM_FRAME_LATENCY);
    }

    #[test]
    fn native_dlss_config_is_disabled_without_runtime_enable() {
        let config = NativeDlssConfig::from_env_reader(|_| None);

        assert!(!config.enabled);
        assert_eq!(config.mode, NativeDlssMode::Quality);
        assert_eq!(config.sharpness, NativeDlssConfig::DEFAULT_SHARPNESS);
        assert!(!config.allow_ray_reconstruction);
        assert!(!config.force_reset_next_frame);
        assert!(!config.debug_overlay);
    }

    #[test]
    fn native_dlss_config_parses_runtime_controls() {
        let config = NativeDlssConfig::from_env_reader(|name| {
            match name {
                "FUN_RENDER_DX12_DLSS" => Some("auto"),
                "FUN_RENDER_DX12_DLSS_MODE" => Some("balanced"),
                "FUN_RENDER_DX12_DLSS_SHARPNESS" => Some("0.25"),
                "FUN_RENDER_DX12_DLSS_RR" => Some("1"),
                "FUN_RENDER_DX12_DLSS_RESET" => Some("true"),
                "FUN_RENDER_DX12_DLSS_DEBUG" => Some("on"),
                _ => None,
            }
            .map(str::to_owned)
        });

        assert_eq!(
            config.enabled,
            cfg!(all(target_os = "windows", feature = "dx12_dlss_native"))
        );
        assert_eq!(config.mode, NativeDlssMode::Balanced);
        assert_eq!(config.sharpness, 0.25);
        assert!(config.allow_ray_reconstruction);
        assert!(config.force_reset_next_frame);
        assert!(config.debug_overlay);
    }

    #[test]
    fn native_dlss_config_accepts_short_fun_dx12_aliases() {
        let config = NativeDlssConfig::from_env_reader(|name| {
            match name {
                "FUN_DX12_DLSS" => Some("1"),
                "FUN_DX12_DLSS_MODE" => Some("performance"),
                "FUN_DX12_DLSS_SHARPNESS" => Some("0.125"),
                "FUN_DX12_DLSS_RR" => Some("0"),
                "FUN_DX12_DLSS_RESET" => Some("1"),
                "FUN_DX12_DLSS_DEBUG" => Some("true"),
                _ => None,
            }
            .map(str::to_owned)
        });

        assert_eq!(
            config.enabled,
            cfg!(all(target_os = "windows", feature = "dx12_dlss_native"))
        );
        assert_eq!(config.mode, NativeDlssMode::Performance);
        assert_eq!(config.sharpness, 0.125);
        assert!(!config.allow_ray_reconstruction);
        assert!(config.force_reset_next_frame);
        assert!(config.debug_overlay);
    }

    #[test]
    fn native_dlss_invalid_values_fall_back_safely() {
        let config = NativeDlssConfig::from_env_reader(|name| {
            match name {
                "FUN_RENDER_DX12_DLSS" => Some("maybe"),
                "FUN_RENDER_DX12_DLSS_MODE" => Some("cinematic"),
                "FUN_RENDER_DX12_DLSS_SHARPNESS" => Some("5.0"),
                _ => None,
            }
            .map(str::to_owned)
        });

        assert!(!config.enabled);
        assert_eq!(config.mode, NativeDlssMode::Quality);
        assert_eq!(config.sharpness, NativeDlssConfig::DEFAULT_SHARPNESS);
    }
}
