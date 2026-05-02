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
use tracing::{info, warn};

use crate::{
    FunCloudDebugOverlay, FunCloudInternalScale, FunCloudQuality, FunCloudSettings,
    FunWeatherProfileId,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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
    pub geometry_policy: RenderGeometryPolicy,
    pub meshlet_min_triangles: usize,
    pub rt_features: FunRenderRtFeatures,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    pub fps_overlay_enabled: bool,
}

impl ClientRenderConfig {
    pub fn from_env() -> Self {
        let cloud_settings = FunCloudSettings::from_env();
        Self {
            solari_enabled: std::env::var_os("FUN_DISABLE_SOLARI").is_none(),
            dlss_rr_enabled: std::env::var_os("FUN_DISABLE_DLSS_RR").is_none(),
            meshlets_enabled: std::env::var_os("FUN_DISABLE_MESHLETS").is_none(),
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
            geometry_policy: RenderGeometryPolicy::from_env(),
            meshlet_min_triangles: env_usize("FUN_MESHLET_MIN_TRIANGLES", 512),
            rt_features: FunRenderRtFeatures::from_env(),
            #[cfg(all(feature = "render_diagnostics", debug_assertions))]
            fps_overlay_enabled: std::env::var_os("FUN_DISABLE_FPS_OVERLAY").is_none(),
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
    AllMeshlet,
    AllRaster,
}

impl RenderGeometryPolicy {
    pub fn from_env() -> Self {
        match std::env::var("FUN_RENDER_GEOMETRY_POLICY")
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Ok("all_meshlet") | Ok("all-meshlet") | Ok("meshlet") => Self::AllMeshlet,
            Ok("all_raster") | Ok("all-raster") | Ok("raster") => Self::AllRaster,
            Ok("hybrid") | Ok("") | Err(_) => Self::Hybrid,
            Ok(other) => {
                warn!(
                    target: "fun::render",
                    policy = other,
                    "unknown FUN_RENDER_GEOMETRY_POLICY; using hybrid"
                );
                Self::Hybrid
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub enum RenderGeometryClass {
    SimpleRaster,
    MeshletStaticDense,
    MeshletDynamicDense,
    RayProxyOnly,
    Viewmodel,
}

impl RenderGeometryClass {
    pub const fn uses_meshlet(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::MeshletStaticDense | RenderGeometryClass::MeshletDynamicDense
        )
    }

    pub const fn uses_raster_mesh(self) -> bool {
        matches!(
            self,
            RenderGeometryClass::SimpleRaster | RenderGeometryClass::Viewmodel
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

pub fn selected_render_backend() -> Backends {
    match std::env::var("FUN_RENDER_BACKEND")
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Ok("dx12") | Ok("d3d12") | Ok("directx12") => Backends::DX12,
        Ok("auto") => Backends::VULKAN | Backends::DX12,
        Ok("vulkan") | Ok("vk") | Ok("") | Err(_) => Backends::VULKAN,
        Ok(other) => {
            info!("[fun render] unknown FUN_RENDER_BACKEND={other}; using Vulkan");
            Backends::VULKAN
        }
    }
}

pub fn selected_present_mode() -> PresentMode {
    match std::env::var("FUN_PRESENT_MODE")
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Ok("auto_no_vsync") | Ok("autonovsync") | Ok("auto-no-vsync") => PresentMode::AutoNoVsync,
        Ok("auto_vsync") | Ok("autovsync") | Ok("auto-vsync") => PresentMode::AutoVsync,
        Ok("fifo") | Ok("vsync") => PresentMode::Fifo,
        Ok("fifo_relaxed") | Ok("fifo-relaxed") => PresentMode::FifoRelaxed,
        Ok("mailbox") => PresentMode::Mailbox,
        Ok("immediate") | Ok("") | Err(_) => PresentMode::Immediate,
        Ok(other) => {
            info!("[fun render] unknown FUN_PRESENT_MODE={other}; using Immediate");
            PresentMode::Immediate
        }
    }
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
        FunRenderRtFeatures, RtHairMode, RtMegaGeometryMode, RtOpacityMaskMode, RtVendorEmulation,
    };
    use bevy::solari::prelude::{SolariGeometryMode, SolariHairMode, SolariOpacityMode};

    fn features_from_pairs(pairs: &[(&'static str, &'static str)]) -> FunRenderRtFeatures {
        FunRenderRtFeatures::from_env_reader(|name| {
            pairs
                .iter()
                .find_map(|(key, value)| (*key == name).then_some((*value).to_owned()))
        })
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
}
