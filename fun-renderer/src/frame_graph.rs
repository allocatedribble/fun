pub const FRAME_GRAPH_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGraphPassHandle(pub u16);

impl FrameGraphPassHandle {
    pub const INVALID: Self = Self(u16::MAX);

    #[must_use]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameGraphResourceHandle(pub u16);

impl FrameGraphResourceHandle {
    pub const INVALID: Self = Self(u16::MAX);

    #[must_use]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphPassType {
    Render,
    Compute,
    CopyImport,
    Readback,
    Presentation,
    VendorSdk,
}

impl FrameGraphPassType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Compute => "compute",
            Self::CopyImport => "copy_import",
            Self::Readback => "readback",
            Self::Presentation => "presentation",
            Self::VendorSdk => "vendor_sdk",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphPassRole {
    Clear,
    StaticScenePlaceholder,
    VirtualResourceFeedback,
    NativeUiGpuImport,
    UiImportPlaceholder,
    UpscaleBoundary,
    FrameGenerationBoundary,
    PostProcessExposure,
    PostProcessBloom,
    // Pass V2.5 — granular typed HDR-post roles that
    // refine the coarse `PostProcessExposure` /
    // `PostProcessBloom` umbrellas above.  The typed
    // `FunLuxHdrPipelineStage::order_key` ordering is
    // mirrored by the typed `hdr_pipeline` module so the
    // typed graph compiler can sort these roles
    // consistently with the typed fun-lux policy.
    PostProcessExposureHistogram,
    PostProcessExposureAdapt,
    PostProcessBloomPrefilter,
    PostProcessBloomDownsample,
    PostProcessBloomUpsample,
    PostProcessBloomComposite,
    PostProcessToneMapping,
    PostProcessColorGradingLut,
    PostProcessSharpening,
    PostProcessDebugOverlay,
    PostProcessFinalOutputTransform,
    Compose,
    DiagnosticsReadback,
    Present,
    // Pass 3 — typed Lux roles. The renderer's
    // `LuxGraphCompiler` (in `lux_graph.rs`) translates
    // every `fun_lux::LuxPassRequest` variant into one of
    // these typed roles; the renderer-side scheduler reads
    // the typed role to pick the live pipeline.
    LuxUploadLightBuffers,
    LuxClusterLights,
    LuxReservoirTemporalReuse,
    LuxReservoirSpatialReuse,
    LuxShadowRequests,
    LuxVirtualShadowPages,
    LuxVirtualShadowFilter,
    LuxVoxelShadowDemandMark,
    LuxVoxelShadowPageBuild,
    LuxVoxelSdfDistantShadowResolve,
    LuxVoxelRadianceClipmapUpdate,
    LuxVoxelCanopyTransmittanceInject,
    LuxVoxelTerrainAoResolve,
    LuxStormExtinctionInject,
    LuxDirectLighting,
    LuxGiTrace,
    LuxGiCacheUpdate,
    LuxReflectionTrace,
    LuxDenoise,
    LuxVolumetricFogInject,
    LuxVolumetricLightInject,
    LuxVolumetricTemporalReproject,
    LuxVolumetricIntegrate,
    LuxVolumetricComposite,
    // Pass C7.2 — typed cloud shadow roles.  Grouped under
    // Lux because Lux owns shadow consumption (the typed
    // direct-lighting pass samples the typed cloud world
    // shadow alongside the typed Lux virtual shadow pages).
    //
    // - LuxCloudShadowProject: typed cloud transmittance →
    //   world-shadow projection pass.  Reads the typed cloud
    //   transmittance target produced by the typed
    //   `CloudPassRole::Raymarch` and writes the typed
    //   `CloudWorldShadowTransmittance` target.
    // - LuxCloudShadowFilter: typed gaussian filter +
    //   distance-fade resolve.  Reads
    //   `CloudWorldShadowTransmittance`, writes
    //   `CloudWorldShadowFiltered`.
    // - LuxCloudShadowRegisterLayer: typed bridge that
    //   registers the typed filtered cloud shadow as a
    //   typed Lux shadow layer the typed direct-lighting
    //   pass can sample alongside the typed virtual
    //   shadow pages.
    LuxCloudShadowProject,
    LuxCloudShadowFilter,
    LuxCloudShadowRegisterLayer,
    LuxDebugOverlay,
}

impl FrameGraphPassRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::StaticScenePlaceholder => "static_scene_placeholder",
            Self::VirtualResourceFeedback => "virtual_resource_feedback",
            Self::NativeUiGpuImport => "native_ui_gpu_import",
            Self::UiImportPlaceholder => "ui_import_placeholder",
            Self::UpscaleBoundary => "upscale_boundary",
            Self::FrameGenerationBoundary => "frame_generation_boundary",
            Self::PostProcessExposure => "post_process_exposure",
            Self::PostProcessBloom => "post_process_bloom",
            Self::PostProcessExposureHistogram => "post_process_exposure_histogram",
            Self::PostProcessExposureAdapt => "post_process_exposure_adapt",
            Self::PostProcessBloomPrefilter => "post_process_bloom_prefilter",
            Self::PostProcessBloomDownsample => "post_process_bloom_downsample",
            Self::PostProcessBloomUpsample => "post_process_bloom_upsample",
            Self::PostProcessBloomComposite => "post_process_bloom_composite",
            Self::PostProcessToneMapping => "post_process_tone_mapping",
            Self::PostProcessColorGradingLut => "post_process_color_grading_lut",
            Self::PostProcessSharpening => "post_process_sharpening",
            Self::PostProcessDebugOverlay => "post_process_debug_overlay",
            Self::PostProcessFinalOutputTransform => "post_process_final_output_transform",
            Self::Compose => "compose",
            Self::DiagnosticsReadback => "diagnostics_readback",
            Self::Present => "present",
            Self::LuxUploadLightBuffers => "lux_upload_light_buffers",
            Self::LuxClusterLights => "lux_cluster_lights",
            Self::LuxReservoirTemporalReuse => "lux_reservoir_temporal_reuse",
            Self::LuxReservoirSpatialReuse => "lux_reservoir_spatial_reuse",
            Self::LuxShadowRequests => "lux_shadow_requests",
            Self::LuxVirtualShadowPages => "lux_virtual_shadow_pages",
            Self::LuxVirtualShadowFilter => "lux_virtual_shadow_filter",
            Self::LuxVoxelShadowDemandMark => "lux_voxel_shadow_demand_mark",
            Self::LuxVoxelShadowPageBuild => "lux_voxel_shadow_page_build",
            Self::LuxVoxelSdfDistantShadowResolve => "lux_voxel_sdf_distant_shadow_resolve",
            Self::LuxVoxelRadianceClipmapUpdate => "lux_voxel_radiance_clipmap_update",
            Self::LuxVoxelCanopyTransmittanceInject => "lux_voxel_canopy_transmittance_inject",
            Self::LuxVoxelTerrainAoResolve => "lux_voxel_terrain_ao_resolve",
            Self::LuxStormExtinctionInject => "lux_storm_extinction_inject",
            Self::LuxDirectLighting => "lux_direct_lighting",
            Self::LuxGiTrace => "lux_gi_trace",
            Self::LuxGiCacheUpdate => "lux_gi_cache_update",
            Self::LuxReflectionTrace => "lux_reflection_trace",
            Self::LuxDenoise => "lux_denoise",
            Self::LuxVolumetricFogInject => "lux_volumetric_fog_inject",
            Self::LuxVolumetricLightInject => "lux_volumetric_light_inject",
            Self::LuxVolumetricTemporalReproject => "lux_volumetric_temporal_reproject",
            Self::LuxVolumetricIntegrate => "lux_volumetric_integrate",
            Self::LuxVolumetricComposite => "lux_volumetric_composite",
            Self::LuxCloudShadowProject => "lux_cloud_shadow_project",
            Self::LuxCloudShadowFilter => "lux_cloud_shadow_filter",
            Self::LuxCloudShadowRegisterLayer => "lux_cloud_shadow_register_layer",
            Self::LuxDebugOverlay => "lux_debug_overlay",
        }
    }

    /// Typed predicate: is this role a Pass 3 Lux role?
    /// `LuxGraphCompiler` uses this to filter passes the
    /// renderer schedules through the lux scheduling lane.
    #[must_use]
    pub const fn is_lux(self) -> bool {
        matches!(
            self,
            Self::LuxUploadLightBuffers
                | Self::LuxClusterLights
                | Self::LuxReservoirTemporalReuse
                | Self::LuxReservoirSpatialReuse
                | Self::LuxShadowRequests
                | Self::LuxVirtualShadowPages
                | Self::LuxVirtualShadowFilter
                | Self::LuxVoxelShadowDemandMark
                | Self::LuxVoxelShadowPageBuild
                | Self::LuxVoxelSdfDistantShadowResolve
                | Self::LuxVoxelRadianceClipmapUpdate
                | Self::LuxVoxelCanopyTransmittanceInject
                | Self::LuxVoxelTerrainAoResolve
                | Self::LuxStormExtinctionInject
                | Self::LuxDirectLighting
                | Self::LuxGiTrace
                | Self::LuxGiCacheUpdate
                | Self::LuxReflectionTrace
                | Self::LuxDenoise
                | Self::LuxVolumetricFogInject
                | Self::LuxVolumetricLightInject
                | Self::LuxVolumetricTemporalReproject
                | Self::LuxVolumetricIntegrate
                | Self::LuxVolumetricComposite
                | Self::LuxCloudShadowProject
                | Self::LuxCloudShadowFilter
                | Self::LuxCloudShadowRegisterLayer
                | Self::LuxDebugOverlay
        )
    }

    /// Pass 3 typed ordering key for Lux passes. Roles outside
    /// the Lux lane return `None`; the typed key positions Lux
    /// passes between scene/depth (≤ 50) and post-processing
    /// (≥ 500).
    #[must_use]
    pub const fn lux_order_key(self) -> Option<u16> {
        Some(match self {
            Self::LuxUploadLightBuffers => 100,
            Self::LuxClusterLights => 110,
            Self::LuxReservoirTemporalReuse => 115,
            Self::LuxReservoirSpatialReuse => 116,
            Self::LuxShadowRequests => 120,
            Self::LuxVirtualShadowPages => 130,
            Self::LuxVirtualShadowFilter => 135,
            Self::LuxVoxelShadowDemandMark => 136,
            Self::LuxVoxelShadowPageBuild => 138,
            // Pass C7.2 — cloud shadow lane sits between
            // the typed virtual shadow filter (135) and
            // the typed direct lighting (200) so the typed
            // direct-lighting pass sees the typed
            // registered cloud shadow layer alongside the
            // typed virtual shadow pages.  Same-frame mode
            // uses these keys directly; one-frame-delayed
            // mode runs the typed project + filter passes
            // here but reads the typed PREVIOUS frame's
            // filtered target in `LuxDirectLighting`.
            Self::LuxCloudShadowProject => 140,
            Self::LuxCloudShadowFilter => 145,
            Self::LuxCloudShadowRegisterLayer => 150,
            Self::LuxDirectLighting => 200,
            Self::LuxVoxelSdfDistantShadowResolve => 205,
            Self::LuxGiTrace => 210,
            Self::LuxGiCacheUpdate => 215,
            Self::LuxVoxelRadianceClipmapUpdate => 216,
            Self::LuxReflectionTrace => 220,
            Self::LuxVoxelTerrainAoResolve => 225,
            Self::LuxDenoise => 230,
            Self::LuxVolumetricFogInject => 300,
            Self::LuxStormExtinctionInject => 302,
            Self::LuxVoxelCanopyTransmittanceInject => 306,
            Self::LuxVolumetricLightInject => 310,
            Self::LuxVolumetricTemporalReproject => 320,
            Self::LuxVolumetricIntegrate => 330,
            Self::LuxVolumetricComposite => 340,
            Self::LuxDebugOverlay => 900,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphResourceType {
    RenderResolutionSceneColor,
    DisplayResolutionSceneColor,
    Depth,
    MotionVectors,
    Exposure,
    ReactiveMask,
    TransparencyMask,
    HdrMetadata,
    FrameTiming,
    PresentResources,
    FrameGenerationResetFlags,
    PresentableFrames,
    PacingDiagnostics,
    NormalsMaterialIds,
    UiColorAlpha,
    FinalComposedOutput,
    TransientScratch,
    HistoryBuffer,
    // Pass 3 — typed Lux resource types. The renderer's
    // `LuxGraphCompiler` maps every `fun_lux::LuxResourceIntent`
    // variant onto one of these typed resource types; the
    // renderer-side allocator reads the typed type + the
    // typed `lux_resources::LuxResourceLifetime` classifier
    // to pick the right `wgpu` allocation.
    LuxLightBuffer,
    LuxLightIndexBuffer,
    LuxClusterGrid,
    LuxReservoirBuffer,
    LuxShadowRequestBuffer,
    LuxShadowAtlas,
    LuxVirtualShadowPages,
    LuxVoxelShadowPageTable,
    LuxVoxelTerrainSdfPool,
    LuxSurfaceCache,
    LuxRadianceCache,
    LuxVoxelTerrainRadianceClipmap,
    LuxVoxelCanopyOpacityClipmap,
    LuxStormExtinctionClipmap,
    LuxProbeCache,
    LuxReflectionBuffer,
    LuxDenoiseHistory,
    LuxVolumetricFroxelDensity,
    LuxVolumetricFroxelScattering,
    LuxVolumetricIntegratedFog,
    LuxVolumetricHistory,
    // Pass C7.2 — typed cloud shadow resources.  The
    // typed `LuxCloudShadowProject` pass writes the
    // typed `CloudWorldShadowTransmittance` target;
    // the typed `LuxCloudShadowFilter` pass reads it,
    // writes `CloudWorldShadowFiltered`.  The typed
    // `CloudShadowProjectionConstants` uniform carries
    // the typed sun direction + bounds the typed
    // direct-lighting pass needs to sample the typed
    // filtered shadow layer.
    CloudWorldShadowTransmittance,
    CloudWorldShadowFiltered,
    CloudShadowProjectionConstants,
    // Pass C7.4.5 — typed cloud shadow input + aux-layer
    // resources.  The typed `LuxCloudShadowProject` pass
    // reads the typed `CloudWeatherMap` + `CloudShapeNoise`
    // (sourced from the typed cloud raymarch's persistent
    // texture set) and writes the typed projected
    // transmittance.  The typed `LuxCloudShadowRegisterLayer`
    // pass writes the typed `CloudShadowAuxLayer` metadata
    // resource the typed `LuxDirectLighting` pass will read
    // in a typed later sub-pass to sample the typed filtered
    // cloud shadow alongside the typed Lux virtual shadow
    // pages.
    CloudWeatherMap,
    CloudShapeNoise,
    CloudShadowAuxLayer,
}

impl FrameGraphResourceType {
    pub const ALL: [Self; 45] = [
        Self::RenderResolutionSceneColor,
        Self::DisplayResolutionSceneColor,
        Self::Depth,
        Self::MotionVectors,
        Self::Exposure,
        Self::ReactiveMask,
        Self::TransparencyMask,
        Self::HdrMetadata,
        Self::FrameTiming,
        Self::PresentResources,
        Self::FrameGenerationResetFlags,
        Self::PresentableFrames,
        Self::PacingDiagnostics,
        Self::NormalsMaterialIds,
        Self::UiColorAlpha,
        Self::FinalComposedOutput,
        Self::TransientScratch,
        Self::HistoryBuffer,
        Self::LuxLightBuffer,
        Self::LuxLightIndexBuffer,
        Self::LuxClusterGrid,
        Self::LuxReservoirBuffer,
        Self::LuxShadowRequestBuffer,
        Self::LuxShadowAtlas,
        Self::LuxVirtualShadowPages,
        Self::LuxVoxelShadowPageTable,
        Self::LuxVoxelTerrainSdfPool,
        Self::LuxSurfaceCache,
        Self::LuxRadianceCache,
        Self::LuxVoxelTerrainRadianceClipmap,
        Self::LuxVoxelCanopyOpacityClipmap,
        Self::LuxStormExtinctionClipmap,
        Self::LuxProbeCache,
        Self::LuxReflectionBuffer,
        Self::LuxDenoiseHistory,
        Self::LuxVolumetricFroxelDensity,
        Self::LuxVolumetricFroxelScattering,
        Self::LuxVolumetricIntegratedFog,
        Self::LuxVolumetricHistory,
        // Pass C7.2 — typed cloud shadow resources.
        Self::CloudWorldShadowTransmittance,
        Self::CloudWorldShadowFiltered,
        Self::CloudShadowProjectionConstants,
        // Pass C7.4.5 — typed cloud shadow input + aux-layer
        // resources (project pass reads + register-layer pass
        // writes).
        Self::CloudWeatherMap,
        Self::CloudShapeNoise,
        Self::CloudShadowAuxLayer,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RenderResolutionSceneColor => "render_resolution_scene_color",
            Self::DisplayResolutionSceneColor => "display_resolution_scene_color",
            Self::Depth => "depth",
            Self::MotionVectors => "motion_vectors",
            Self::Exposure => "exposure",
            Self::ReactiveMask => "reactive_mask",
            Self::TransparencyMask => "transparency_mask",
            Self::HdrMetadata => "hdr_metadata",
            Self::FrameTiming => "frame_timing",
            Self::PresentResources => "present_resources",
            Self::FrameGenerationResetFlags => "frame_generation_reset_flags",
            Self::PresentableFrames => "presentable_frames",
            Self::PacingDiagnostics => "pacing_diagnostics",
            Self::NormalsMaterialIds => "normals_material_ids",
            Self::UiColorAlpha => "ui_color_alpha",
            Self::FinalComposedOutput => "final_composed_output",
            Self::TransientScratch => "transient_scratch",
            Self::HistoryBuffer => "history_buffer",
            Self::LuxLightBuffer => "lux_light_buffer",
            Self::LuxLightIndexBuffer => "lux_light_index_buffer",
            Self::LuxClusterGrid => "lux_cluster_grid",
            Self::LuxReservoirBuffer => "lux_reservoir_buffer",
            Self::LuxShadowRequestBuffer => "lux_shadow_request_buffer",
            Self::LuxShadowAtlas => "lux_shadow_atlas",
            Self::LuxVirtualShadowPages => "lux_virtual_shadow_pages",
            Self::LuxVoxelShadowPageTable => "lux_voxel_shadow_page_table",
            Self::LuxVoxelTerrainSdfPool => "lux_voxel_terrain_sdf_pool",
            Self::LuxSurfaceCache => "lux_surface_cache",
            Self::LuxRadianceCache => "lux_radiance_cache",
            Self::LuxVoxelTerrainRadianceClipmap => "lux_voxel_terrain_radiance_clipmap",
            Self::LuxVoxelCanopyOpacityClipmap => "lux_voxel_canopy_opacity_clipmap",
            Self::LuxStormExtinctionClipmap => "lux_storm_extinction_clipmap",
            Self::LuxProbeCache => "lux_probe_cache",
            Self::LuxReflectionBuffer => "lux_reflection_buffer",
            Self::LuxDenoiseHistory => "lux_denoise_history",
            Self::LuxVolumetricFroxelDensity => "lux_volumetric_froxel_density",
            Self::LuxVolumetricFroxelScattering => "lux_volumetric_froxel_scattering",
            Self::LuxVolumetricIntegratedFog => "lux_volumetric_integrated_fog",
            Self::LuxVolumetricHistory => "lux_volumetric_history",
            Self::CloudWorldShadowTransmittance => "cloud_world_shadow_transmittance",
            Self::CloudWorldShadowFiltered => "cloud_world_shadow_filtered",
            Self::CloudShadowProjectionConstants => "cloud_shadow_projection_constants",
            Self::CloudWeatherMap => "cloud_weather_map",
            Self::CloudShapeNoise => "cloud_shape_noise",
            Self::CloudShadowAuxLayer => "cloud_shadow_aux_layer",
        }
    }

    /// Typed predicate: is this resource type a Pass 3 Lux
    /// resource type?
    #[must_use]
    pub const fn is_lux(self) -> bool {
        matches!(
            self,
            Self::LuxLightBuffer
                | Self::LuxLightIndexBuffer
                | Self::LuxClusterGrid
                | Self::LuxReservoirBuffer
                | Self::LuxShadowRequestBuffer
                | Self::LuxShadowAtlas
                | Self::LuxVirtualShadowPages
                | Self::LuxVoxelShadowPageTable
                | Self::LuxVoxelTerrainSdfPool
                | Self::LuxSurfaceCache
                | Self::LuxRadianceCache
                | Self::LuxVoxelTerrainRadianceClipmap
                | Self::LuxVoxelCanopyOpacityClipmap
                | Self::LuxStormExtinctionClipmap
                | Self::LuxProbeCache
                | Self::LuxReflectionBuffer
                | Self::LuxDenoiseHistory
                | Self::LuxVolumetricFroxelDensity
                | Self::LuxVolumetricFroxelScattering
                | Self::LuxVolumetricIntegratedFog
                | Self::LuxVolumetricHistory
        )
    }

    /// Typed predicate: is this resource type a typed
    /// cloud-owned resource (Pass C7.2+ taxonomy)?  Mirrors
    /// the typed `is_lux` predicate at the typed cloud
    /// boundary.
    #[must_use]
    pub const fn is_cloud_owned(self) -> bool {
        matches!(
            self,
            Self::CloudWorldShadowTransmittance
                | Self::CloudWorldShadowFiltered
                | Self::CloudShadowProjectionConstants
                | Self::CloudWeatherMap
                | Self::CloudShapeNoise
                | Self::CloudShadowAuxLayer
        )
    }

    /// Pass C9.0 typed predicate — is this typed resource
    /// type a typed "core required" resource that every
    /// typed renderer build must allocate (typed scene
    /// color / depth / present / UI / final output /
    /// scratch / history / etc.)?  Returns `false` for
    /// typed conditional resources (typed `is_lux` +
    /// typed `is_cloud_owned`) which only allocate when
    /// the typed Lux / cloud sub-paths register them.
    ///
    /// Drives the typed `validate_required_resources`
    /// gate so the typed default frame description does
    /// NOT fail validation when typed Lux / cloud paths
    /// are not yet registered.
    #[must_use]
    pub const fn is_core_required(self) -> bool {
        !self.is_lux() && !self.is_cloud_owned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphDiagnosticCategory {
    Scene,
    Ui,
    Upscale,
    FrameGeneration,
    VirtualResources,
    Diagnostics,
    Presentation,
    PostProcess,
}

impl FrameGraphDiagnosticCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Ui => "ui",
            Self::Upscale => "upscale",
            Self::FrameGeneration => "frame_generation",
            Self::VirtualResources => "virtual_resources",
            Self::Diagnostics => "diagnostics",
            Self::Presentation => "presentation",
            Self::PostProcess => "post_process",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameGraphBenchmarkCategory {
    Clear,
    ScenePlaceholder,
    UiImport,
    Compose,
    Present,
    Upscale,
    FrameGeneration,
    VirtualResources,
    Diagnostics,
    PostProcessExposure,
    PostProcessBloom,
    PostProcessToneMapping,
    PostProcessColorGradingLut,
    PostProcessSharpening,
    PostProcessDebugOverlay,
    PostProcessFinalOutputTransform,
}

impl FrameGraphBenchmarkCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::ScenePlaceholder => "scene_placeholder",
            Self::UiImport => "ui_import",
            Self::Compose => "compose",
            Self::Present => "present",
            Self::Upscale => "upscale",
            Self::FrameGeneration => "frame_generation",
            Self::VirtualResources => "virtual_resources",
            Self::Diagnostics => "diagnostics",
            Self::PostProcessExposure => "post_process_exposure",
            Self::PostProcessBloom => "post_process_bloom",
            Self::PostProcessToneMapping => "post_process_tone_mapping",
            Self::PostProcessColorGradingLut => "post_process_color_grading_lut",
            Self::PostProcessSharpening => "post_process_sharpening",
            Self::PostProcessDebugOverlay => "post_process_debug_overlay",
            Self::PostProcessFinalOutputTransform => "post_process_final_output_transform",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGraphResourceDescriptor {
    pub stable_id: &'static str,
    pub resource_type: FrameGraphResourceType,
    pub debug_label: &'static str,
}

impl FrameGraphResourceDescriptor {
    #[must_use]
    pub const fn new(
        stable_id: &'static str,
        resource_type: FrameGraphResourceType,
        debug_label: &'static str,
    ) -> Self {
        Self {
            stable_id,
            resource_type,
            debug_label,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGraphPassDescriptor {
    pub stable_id: &'static str,
    pub pass_type: FrameGraphPassType,
    pub role: FrameGraphPassRole,
    pub stable_label: &'static str,
    pub diagnostic_category: FrameGraphDiagnosticCategory,
    pub benchmark_category: Option<FrameGraphBenchmarkCategory>,
    pub marker: &'static str,
    pub enabled: bool,
}

impl FrameGraphPassDescriptor {
    #[must_use]
    pub const fn new(
        stable_id: &'static str,
        pass_type: FrameGraphPassType,
        role: FrameGraphPassRole,
        stable_label: &'static str,
        diagnostic_category: FrameGraphDiagnosticCategory,
        benchmark_category: Option<FrameGraphBenchmarkCategory>,
        marker: &'static str,
    ) -> Self {
        Self {
            stable_id,
            pass_type,
            role,
            stable_label,
            diagnostic_category,
            benchmark_category,
            marker,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostProcessPassRequest {
    pub include_exposure: bool,
    pub include_bloom: bool,
    pub include_tone_mapping: bool,
    pub include_color_grading_lut: bool,
    pub include_sharpening: bool,
    pub include_debug_overlay: bool,
    pub include_final_output_transform: bool,
    pub require_history_buffer: bool,
}

impl PostProcessPassRequest {
    pub const NONE: Self = Self {
        include_exposure: false,
        include_bloom: false,
        include_tone_mapping: false,
        include_color_grading_lut: false,
        include_sharpening: false,
        include_debug_overlay: false,
        include_final_output_transform: false,
        require_history_buffer: false,
    };

    pub const TONE_MAPPING_ONLY: Self = Self {
        include_exposure: false,
        include_bloom: false,
        include_tone_mapping: true,
        include_color_grading_lut: false,
        include_sharpening: false,
        include_debug_overlay: false,
        include_final_output_transform: true,
        require_history_buffer: false,
    };
}

impl Default for PostProcessPassRequest {
    fn default() -> Self {
        Self::TONE_MAPPING_ONLY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererFrameDescription {
    pub frame_index: u64,
    pub include_static_scene_placeholder: bool,
    pub include_ui_placeholder: bool,
    pub include_virtual_resource_slot: bool,
    pub include_upscaling_slot: bool,
    pub include_frame_generation_slot: bool,
    pub include_diagnostics_readback: bool,
    pub post_process: PostProcessPassRequest,
}

impl RendererFrameDescription {
    #[must_use]
    pub const fn static_scene_with_ui(frame_index: u64) -> Self {
        Self {
            frame_index,
            include_static_scene_placeholder: true,
            include_ui_placeholder: true,
            include_virtual_resource_slot: false,
            include_upscaling_slot: false,
            include_frame_generation_slot: false,
            include_diagnostics_readback: false,
            post_process: PostProcessPassRequest::TONE_MAPPING_ONLY,
        }
    }

    #[must_use]
    pub const fn with_virtual_resources(mut self, enabled: bool) -> Self {
        self.include_virtual_resource_slot = enabled;
        self
    }

    #[must_use]
    pub const fn with_upscaling(mut self, enabled: bool) -> Self {
        self.include_upscaling_slot = enabled;
        self
    }

    #[must_use]
    pub const fn with_frame_generation(mut self, enabled: bool) -> Self {
        self.include_frame_generation_slot = enabled;
        self
    }

    #[must_use]
    pub const fn with_diagnostics_readback(mut self, enabled: bool) -> Self {
        self.include_diagnostics_readback = enabled;
        self
    }

    #[must_use]
    pub const fn with_post_process(mut self, request: PostProcessPassRequest) -> Self {
        self.post_process = request;
        self
    }
}

impl Default for RendererFrameDescription {
    fn default() -> Self {
        Self::static_scene_with_ui(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameGraphResource {
    pub handle: FrameGraphResourceHandle,
    pub descriptor: FrameGraphResourceDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameGraphPass {
    pub handle: FrameGraphPassHandle,
    pub descriptor: FrameGraphPassDescriptor,
    pub reads: Vec<FrameGraphResourceHandle>,
    pub writes: Vec<FrameGraphResourceHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameGraphValidationFailureCode {
    MissingRequiredResource,
    PresentPassNotLast,
    UiSceneSeparationBroken,
    ComposeContractBroken,
    PresentContractBroken,
    UpscaleContractBroken,
    FrameGenerationContractBroken,
    InvalidResourceHandle,
    /// Pass 3 — a Lux pass reads a resource that no
    /// upstream pass writes (the typed contract demands
    /// every read be backed by a typed write).
    LuxPassReadsUnwrittenResource,
    /// Pass 3 — a Lux pass appears out of order against
    /// its typed `lux_order_key`.
    LuxPassOrderViolation,
    /// Pass 3 — a Lux pass declared neither reads nor
    /// writes (every typed Lux pass MUST declare at least
    /// one resource interaction).
    LuxPassDeclaresNoReadsOrWrites,
}

impl FrameGraphValidationFailureCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MissingRequiredResource => "missing_required_resource",
            Self::PresentPassNotLast => "present_pass_not_last",
            Self::UiSceneSeparationBroken => "ui_scene_separation_broken",
            Self::ComposeContractBroken => "compose_contract_broken",
            Self::PresentContractBroken => "present_contract_broken",
            Self::UpscaleContractBroken => "upscale_contract_broken",
            Self::FrameGenerationContractBroken => "frame_generation_contract_broken",
            Self::InvalidResourceHandle => "invalid_resource_handle",
            Self::LuxPassReadsUnwrittenResource => "lux_pass_reads_unwritten_resource",
            Self::LuxPassOrderViolation => "lux_pass_order_violation",
            Self::LuxPassDeclaresNoReadsOrWrites => "lux_pass_declares_no_reads_or_writes",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGraphValidationFailure {
    pub code: FrameGraphValidationFailureCode,
    pub pass: Option<FrameGraphPassHandle>,
    pub resource: Option<FrameGraphResourceHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGraphPassOrderRecord {
    pub pass: FrameGraphPassHandle,
    pub order: u16,
    pub stable_id: &'static str,
    pub pass_type: FrameGraphPassType,
    pub role: FrameGraphPassRole,
    pub diagnostic_category: FrameGraphDiagnosticCategory,
    pub marker: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGraphResourceLifetime {
    pub resource: FrameGraphResourceHandle,
    pub stable_id: &'static str,
    pub resource_type: FrameGraphResourceType,
    pub first_pass: FrameGraphPassHandle,
    pub last_pass: FrameGraphPassHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameGraphPassTiming {
    pub pass: FrameGraphPassHandle,
    pub stable_id: &'static str,
    pub elapsed_ns: u64,
    pub executed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFrameGraphDiagnostics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub pass_count: u16,
    pub resource_count: u16,
    pub validation_failures: Vec<FrameGraphValidationFailure>,
    pub pass_order: Vec<FrameGraphPassOrderRecord>,
    pub resource_lifetimes: Vec<FrameGraphResourceLifetime>,
    pub pass_timings: Vec<FrameGraphPassTiming>,
}

impl RendererFrameGraphDiagnostics {
    #[must_use]
    pub fn graph_valid(&self) -> bool {
        self.validation_failures.is_empty()
    }

    #[must_use]
    pub fn validation_failure_count(&self) -> u16 {
        u16::try_from(self.validation_failures.len()).unwrap_or(u16::MAX)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererFrameGraphDebugArtifact {
    pub schema_version: u16,
    pub frame_index: u64,
    pub content: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "fun_ecs", derive(fun_ecs::Resource))]
pub struct RendererFrameGraph {
    frame_index: u64,
    passes: Vec<FrameGraphPass>,
    resources: Vec<FrameGraphResource>,
    /// Pass V2.2 — typed validation failures pushed by
    /// external compilers (notably
    /// [`crate::lux_graph::LuxGraphCompiler`]).  These get
    /// merged into the typed validation_failures list at
    /// `execute()` so the typed graph diagnostics surface
    /// them alongside the graph's intrinsic validations.
    /// Without this buffer, Lux compile failures would
    /// remain invisible to the typed renderer diagnostics
    /// and the typed bridge could silently present a
    /// successful state.
    external_validation_failures: Vec<FrameGraphValidationFailure>,
}

impl RendererFrameGraph {
    #[must_use]
    pub fn from_frame_description(description: RendererFrameDescription) -> Self {
        let mut graph = Self {
            frame_index: description.frame_index,
            passes: Vec::new(),
            resources: Vec::new(),
            external_validation_failures: Vec::new(),
        };

        let render_scene = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.scene_color.render_resolution",
            FrameGraphResourceType::RenderResolutionSceneColor,
            "scene_color_render_resolution",
        ));
        let display_scene = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.scene_color.display_resolution",
            FrameGraphResourceType::DisplayResolutionSceneColor,
            "scene_color_display_resolution",
        ));
        let depth = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.depth",
            FrameGraphResourceType::Depth,
            "depth",
        ));
        let motion = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.motion_vectors",
            FrameGraphResourceType::MotionVectors,
            "motion_vectors",
        ));
        let exposure = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.exposure",
            FrameGraphResourceType::Exposure,
            "exposure",
        ));
        let reactive_mask = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.reactive_mask",
            FrameGraphResourceType::ReactiveMask,
            "reactive_mask",
        ));
        let transparency_mask = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.transparency_mask",
            FrameGraphResourceType::TransparencyMask,
            "transparency_mask",
        ));
        let hdr_metadata = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.hdr_metadata",
            FrameGraphResourceType::HdrMetadata,
            "hdr_metadata",
        ));
        let frame_timing = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.frame_timing",
            FrameGraphResourceType::FrameTiming,
            "frame_timing",
        ));
        let present_resources = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.present_resources",
            FrameGraphResourceType::PresentResources,
            "present_resources",
        ));
        let frame_generation_reset_flags =
            graph.declare_resource(FrameGraphResourceDescriptor::new(
                "fun_renderer.resource.frame_generation_reset_flags",
                FrameGraphResourceType::FrameGenerationResetFlags,
                "frame_generation_reset_flags",
            ));
        let presentable_frames = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.presentable_frames",
            FrameGraphResourceType::PresentableFrames,
            "presentable_frames",
        ));
        let pacing_diagnostics = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.pacing_diagnostics",
            FrameGraphResourceType::PacingDiagnostics,
            "pacing_diagnostics",
        ));
        let normals_material = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.normals_material_ids",
            FrameGraphResourceType::NormalsMaterialIds,
            "normals_material_ids",
        ));
        let ui = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.ui_color_alpha",
            FrameGraphResourceType::UiColorAlpha,
            "ui_color_alpha",
        ));
        let final_output = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.final_composed_output",
            FrameGraphResourceType::FinalComposedOutput,
            "final_composed_output",
        ));
        let scratch = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.transient_scratch",
            FrameGraphResourceType::TransientScratch,
            "transient_scratch",
        ));
        let history = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "fun_renderer.resource.history_buffer",
            FrameGraphResourceType::HistoryBuffer,
            "history_buffer",
        ));

        let clear = graph.register_pass(FrameGraphPassDescriptor::new(
            "fun_renderer.pass.clear",
            FrameGraphPassType::Render,
            FrameGraphPassRole::Clear,
            "clear",
            FrameGraphDiagnosticCategory::Scene,
            Some(FrameGraphBenchmarkCategory::Clear),
            "fun_renderer::frame_graph::clear",
        ));
        graph.add_pass_write(clear, render_scene);
        graph.add_pass_write(clear, frame_timing);
        graph.add_pass_write(clear, present_resources);
        graph.add_pass_write(clear, frame_generation_reset_flags);
        if !description.include_static_scene_placeholder && !description.include_upscaling_slot {
            graph.add_pass_write(clear, display_scene);
        }

        if description.include_static_scene_placeholder {
            let scene = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.static_scene_placeholder",
                FrameGraphPassType::Render,
                FrameGraphPassRole::StaticScenePlaceholder,
                "static_scene_placeholder",
                FrameGraphDiagnosticCategory::Scene,
                Some(FrameGraphBenchmarkCategory::ScenePlaceholder),
                "fun_renderer::frame_graph::static_scene_placeholder",
            ));
            graph.add_pass_read(scene, render_scene);
            graph.add_pass_write(scene, render_scene);
            if !description.include_upscaling_slot {
                graph.add_pass_write(scene, display_scene);
            }
            graph.add_pass_write(scene, depth);
            graph.add_pass_write(scene, motion);
            graph.add_pass_write(scene, exposure);
            graph.add_pass_write(scene, reactive_mask);
            graph.add_pass_write(scene, transparency_mask);
            graph.add_pass_write(scene, hdr_metadata);
            graph.add_pass_write(scene, normals_material);
        }

        if description.include_virtual_resource_slot {
            let virtual_resources = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.virtual_resource_feedback",
                FrameGraphPassType::Compute,
                FrameGraphPassRole::VirtualResourceFeedback,
                "virtual_resource_feedback",
                FrameGraphDiagnosticCategory::VirtualResources,
                Some(FrameGraphBenchmarkCategory::VirtualResources),
                "fun_renderer::frame_graph::virtual_resource_feedback",
            ));
            graph.add_pass_read(virtual_resources, depth);
            graph.add_pass_read(virtual_resources, motion);
            graph.add_pass_write(virtual_resources, scratch);
        }

        if description.include_ui_placeholder {
            let ui_import = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.native_ui_gpu_import",
                FrameGraphPassType::CopyImport,
                FrameGraphPassRole::NativeUiGpuImport,
                "native_ui_gpu_import",
                FrameGraphDiagnosticCategory::Ui,
                Some(FrameGraphBenchmarkCategory::UiImport),
                "fun_renderer::frame_graph::native_ui_gpu_import",
            ));
            graph.add_pass_write(ui_import, ui);
        }

        if description.include_upscaling_slot {
            let upscale = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.upscale_boundary",
                FrameGraphPassType::VendorSdk,
                FrameGraphPassRole::UpscaleBoundary,
                "upscale_boundary",
                FrameGraphDiagnosticCategory::Upscale,
                Some(FrameGraphBenchmarkCategory::Upscale),
                "fun_renderer::frame_graph::upscale_boundary",
            ));
            graph.add_pass_read(upscale, render_scene);
            graph.add_pass_read(upscale, depth);
            graph.add_pass_read(upscale, motion);
            graph.add_pass_read(upscale, exposure);
            graph.add_pass_read(upscale, reactive_mask);
            graph.add_pass_read(upscale, transparency_mask);
            graph.add_pass_read(upscale, hdr_metadata);
            graph.add_pass_write(upscale, display_scene);
        }

        if description.include_frame_generation_slot {
            let frame_generation = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.frame_generation_boundary",
                FrameGraphPassType::VendorSdk,
                FrameGraphPassRole::FrameGenerationBoundary,
                "frame_generation_boundary",
                FrameGraphDiagnosticCategory::FrameGeneration,
                Some(FrameGraphBenchmarkCategory::FrameGeneration),
                "fun_renderer::frame_graph::frame_generation_boundary",
            ));
            graph.add_pass_read(frame_generation, display_scene);
            graph.add_pass_read(frame_generation, ui);
            graph.add_pass_read(frame_generation, depth);
            graph.add_pass_read(frame_generation, motion);
            graph.add_pass_read(frame_generation, frame_timing);
            graph.add_pass_read(frame_generation, present_resources);
            graph.add_pass_read(frame_generation, frame_generation_reset_flags);
            graph.add_pass_write(frame_generation, history);
            graph.add_pass_write(frame_generation, presentable_frames);
            graph.add_pass_write(frame_generation, pacing_diagnostics);
        }

        // Post-process passes run between upscaling/frame generation and compose.
        // They are gated on the description.post_process flags so history buffers
        // are only allocated when TAA/upscaling actually needs them, and the
        // final-output-transform pass is always present so the swapchain only
        // sees the renderer-owned post stack output.
        let post_process = description.post_process;
        if post_process.include_exposure {
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.post_process_exposure",
                FrameGraphPassType::Compute,
                FrameGraphPassRole::PostProcessExposure,
                "post_process_exposure",
                FrameGraphDiagnosticCategory::PostProcess,
                Some(FrameGraphBenchmarkCategory::PostProcessExposure),
                "fun_renderer::frame_graph::post_process_exposure",
            ));
            graph.add_pass_read(pass, render_scene);
            graph.add_pass_write(pass, exposure);
        }
        if post_process.include_bloom {
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.post_process_bloom",
                FrameGraphPassType::Render,
                FrameGraphPassRole::PostProcessBloom,
                "post_process_bloom",
                FrameGraphDiagnosticCategory::PostProcess,
                Some(FrameGraphBenchmarkCategory::PostProcessBloom),
                "fun_renderer::frame_graph::post_process_bloom",
            ));
            graph.add_pass_read(pass, display_scene);
            graph.add_pass_read(pass, exposure);
            graph.add_pass_write(pass, display_scene);
        }
        if post_process.include_tone_mapping {
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.post_process_tone_mapping",
                FrameGraphPassType::Render,
                FrameGraphPassRole::PostProcessToneMapping,
                "post_process_tone_mapping",
                FrameGraphDiagnosticCategory::PostProcess,
                Some(FrameGraphBenchmarkCategory::PostProcessToneMapping),
                "fun_renderer::frame_graph::post_process_tone_mapping",
            ));
            graph.add_pass_read(pass, display_scene);
            graph.add_pass_read(pass, exposure);
            graph.add_pass_read(pass, hdr_metadata);
            if post_process.require_history_buffer {
                graph.add_pass_read(pass, history);
            }
            graph.add_pass_write(pass, display_scene);
        }
        if post_process.include_color_grading_lut {
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.post_process_color_grading_lut",
                FrameGraphPassType::Render,
                FrameGraphPassRole::PostProcessColorGradingLut,
                "post_process_color_grading_lut",
                FrameGraphDiagnosticCategory::PostProcess,
                Some(FrameGraphBenchmarkCategory::PostProcessColorGradingLut),
                "fun_renderer::frame_graph::post_process_color_grading_lut",
            ));
            graph.add_pass_read(pass, display_scene);
            graph.add_pass_write(pass, display_scene);
        }
        if post_process.include_sharpening {
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.post_process_sharpening",
                FrameGraphPassType::Render,
                FrameGraphPassRole::PostProcessSharpening,
                "post_process_sharpening",
                FrameGraphDiagnosticCategory::PostProcess,
                Some(FrameGraphBenchmarkCategory::PostProcessSharpening),
                "fun_renderer::frame_graph::post_process_sharpening",
            ));
            graph.add_pass_read(pass, display_scene);
            graph.add_pass_write(pass, display_scene);
        }
        if post_process.include_debug_overlay {
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.post_process_debug_overlay",
                FrameGraphPassType::Render,
                FrameGraphPassRole::PostProcessDebugOverlay,
                "post_process_debug_overlay",
                FrameGraphDiagnosticCategory::PostProcess,
                Some(FrameGraphBenchmarkCategory::PostProcessDebugOverlay),
                "fun_renderer::frame_graph::post_process_debug_overlay",
            ));
            graph.add_pass_read(pass, display_scene);
            graph.add_pass_read(pass, depth);
            graph.add_pass_read(pass, motion);
            graph.add_pass_write(pass, display_scene);
        }
        if post_process.include_final_output_transform {
            let pass = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.post_process_final_output_transform",
                FrameGraphPassType::Render,
                FrameGraphPassRole::PostProcessFinalOutputTransform,
                "post_process_final_output_transform",
                FrameGraphDiagnosticCategory::PostProcess,
                Some(FrameGraphBenchmarkCategory::PostProcessFinalOutputTransform),
                "fun_renderer::frame_graph::post_process_final_output_transform",
            ));
            graph.add_pass_read(pass, display_scene);
            graph.add_pass_read(pass, hdr_metadata);
            graph.add_pass_write(pass, display_scene);
        }

        let compose = graph.register_pass(FrameGraphPassDescriptor::new(
            "fun_renderer.pass.compose",
            FrameGraphPassType::Render,
            FrameGraphPassRole::Compose,
            "compose",
            FrameGraphDiagnosticCategory::Ui,
            Some(FrameGraphBenchmarkCategory::Compose),
            "fun_renderer::frame_graph::compose",
        ));
        graph.add_pass_read(compose, display_scene);
        graph.add_pass_read(compose, ui);
        if description.include_frame_generation_slot {
            graph.add_pass_read(compose, presentable_frames);
        }
        graph.add_pass_write(compose, final_output);

        if description.include_diagnostics_readback {
            let readback = graph.register_pass(FrameGraphPassDescriptor::new(
                "fun_renderer.pass.diagnostics_readback",
                FrameGraphPassType::Readback,
                FrameGraphPassRole::DiagnosticsReadback,
                "diagnostics_readback",
                FrameGraphDiagnosticCategory::Diagnostics,
                Some(FrameGraphBenchmarkCategory::Diagnostics),
                "fun_renderer::frame_graph::diagnostics_readback",
            ));
            graph.add_pass_read(readback, final_output);
        }

        let present = graph.register_pass(FrameGraphPassDescriptor::new(
            "fun_renderer.pass.present",
            FrameGraphPassType::Presentation,
            FrameGraphPassRole::Present,
            "present",
            FrameGraphDiagnosticCategory::Presentation,
            Some(FrameGraphBenchmarkCategory::Present),
            "fun_renderer::frame_graph::present",
        ));
        graph.add_pass_read(present, final_output);
        graph.add_pass_read(present, present_resources);

        graph
    }

    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }

    #[must_use]
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.resources.len()
    }

    #[must_use]
    pub fn passes(&self) -> &[FrameGraphPass] {
        &self.passes
    }

    #[must_use]
    pub fn resources(&self) -> &[FrameGraphResource] {
        &self.resources
    }

    pub fn declare_resource(
        &mut self,
        descriptor: FrameGraphResourceDescriptor,
    ) -> FrameGraphResourceHandle {
        let Ok(index) = u16::try_from(self.resources.len()) else {
            return FrameGraphResourceHandle::INVALID;
        };
        let handle = FrameGraphResourceHandle::new(index);
        self.resources
            .push(FrameGraphResource { handle, descriptor });
        handle
    }

    pub fn register_pass(&mut self, descriptor: FrameGraphPassDescriptor) -> FrameGraphPassHandle {
        let Ok(index) = u16::try_from(self.passes.len()) else {
            return FrameGraphPassHandle::INVALID;
        };
        let handle = FrameGraphPassHandle::new(index);
        self.passes.push(FrameGraphPass {
            handle,
            descriptor,
            reads: Vec::new(),
            writes: Vec::new(),
        });
        handle
    }

    pub fn add_pass_read(
        &mut self,
        pass: FrameGraphPassHandle,
        resource: FrameGraphResourceHandle,
    ) -> bool {
        if !self.resource_exists(resource) {
            return false;
        }
        let Some(pass) = self.pass_mut(pass) else {
            return false;
        };
        if !pass.reads.contains(&resource) {
            pass.reads.push(resource);
        }
        true
    }

    pub fn add_pass_write(
        &mut self,
        pass: FrameGraphPassHandle,
        resource: FrameGraphResourceHandle,
    ) -> bool {
        if !self.resource_exists(resource) {
            return false;
        }
        let Some(pass) = self.pass_mut(pass) else {
            return false;
        };
        if !pass.writes.contains(&resource) {
            pass.writes.push(resource);
        }
        true
    }

    #[must_use]
    pub fn pass(&self, handle: FrameGraphPassHandle) -> Option<&FrameGraphPass> {
        if !handle.is_valid() {
            return None;
        }
        self.passes.get(usize::from(handle.0))
    }

    #[must_use]
    pub fn resource(&self, handle: FrameGraphResourceHandle) -> Option<&FrameGraphResource> {
        if !handle.is_valid() {
            return None;
        }
        self.resources.get(usize::from(handle.0))
    }

    #[must_use]
    pub fn resource_handle_for_type(
        &self,
        resource_type: FrameGraphResourceType,
    ) -> Option<FrameGraphResourceHandle> {
        self.resources
            .iter()
            .find(|resource| resource.descriptor.resource_type == resource_type)
            .map(|resource| resource.handle)
    }

    #[must_use]
    pub fn pass_handle_for_role(&self, role: FrameGraphPassRole) -> Option<FrameGraphPassHandle> {
        self.passes
            .iter()
            .find(|pass| pass.descriptor.role == role && pass.descriptor.enabled)
            .map(|pass| pass.handle)
    }

    #[must_use]
    pub fn pass_reads_resource_type(
        &self,
        pass: FrameGraphPassHandle,
        resource_type: FrameGraphResourceType,
    ) -> bool {
        self.pass(pass).is_some_and(|pass| {
            pass.reads
                .iter()
                .any(|handle| self.resource_type(*handle) == Some(resource_type))
        })
    }

    #[must_use]
    pub fn pass_writes_resource_type(
        &self,
        pass: FrameGraphPassHandle,
        resource_type: FrameGraphResourceType,
    ) -> bool {
        self.pass(pass).is_some_and(|pass| {
            pass.writes
                .iter()
                .any(|handle| self.resource_type(*handle) == Some(resource_type))
        })
    }

    #[must_use]
    pub fn validate(&self) -> Vec<FrameGraphValidationFailure> {
        let mut failures = Vec::new();
        self.validate_required_resources(&mut failures);
        self.validate_present_last(&mut failures);
        self.validate_scene_ui_separation(&mut failures);
        self.validate_compose_contract(&mut failures);
        self.validate_present_contract(&mut failures);
        self.validate_upscale_contract(&mut failures);
        self.validate_frame_generation_contract(&mut failures);
        // Pass V2.2 — merge typed externally-pushed
        // validation failures (Lux compile failures, etc.)
        // so the typed diagnostics surface them too.
        failures.extend(self.external_validation_failures.iter().copied());
        failures
    }

    /// Pass V2.2 — push a typed externally-detected
    /// validation failure (typically from
    /// [`crate::lux_graph::LuxGraphCompiler`]) into the
    /// typed graph state.  The failure is merged into the
    /// graph's intrinsic validations at
    /// [`Self::execute`].
    pub fn push_external_validation_failure(&mut self, failure: FrameGraphValidationFailure) {
        self.external_validation_failures.push(failure);
    }

    /// Pass V2.2 — typed predicate: does the graph carry
    /// any externally-pushed validation failures?
    #[must_use]
    pub fn has_external_validation_failures(&self) -> bool {
        !self.external_validation_failures.is_empty()
    }

    #[must_use]
    pub fn execute(&self) -> RendererFrameGraphDiagnostics {
        let validation_failures = self.validate();
        let graph_valid = validation_failures.is_empty();
        let pass_order = self
            .passes
            .iter()
            .filter(|pass| pass.descriptor.enabled)
            .enumerate()
            .map(|(order, pass)| FrameGraphPassOrderRecord {
                pass: pass.handle,
                order: u16::try_from(order).unwrap_or(u16::MAX),
                stable_id: pass.descriptor.stable_id,
                pass_type: pass.descriptor.pass_type,
                role: pass.descriptor.role,
                diagnostic_category: pass.descriptor.diagnostic_category,
                marker: pass.descriptor.marker,
            })
            .collect();
        let resource_lifetimes = self.resource_lifetimes();
        let pass_timings = self
            .passes
            .iter()
            .filter(|pass| pass.descriptor.enabled)
            .map(|pass| FrameGraphPassTiming {
                pass: pass.handle,
                stable_id: pass.descriptor.stable_id,
                elapsed_ns: 0,
                executed: graph_valid,
            })
            .collect();
        RendererFrameGraphDiagnostics {
            schema_version: FRAME_GRAPH_SCHEMA_VERSION,
            frame_index: self.frame_index,
            pass_count: u16::try_from(self.passes.len()).unwrap_or(u16::MAX),
            resource_count: u16::try_from(self.resources.len()).unwrap_or(u16::MAX),
            validation_failures,
            pass_order,
            resource_lifetimes,
            pass_timings,
        }
    }

    #[must_use]
    pub fn debug_artifact(
        &self,
        diagnostics: &RendererFrameGraphDiagnostics,
    ) -> RendererFrameGraphDebugArtifact {
        use core::fmt::Write as _;

        let mut content = String::new();
        let _ = writeln!(
            content,
            "schema_version={} frame_index={} graph_valid={} pass_count={} resource_count={} validation_failure_count={}",
            FRAME_GRAPH_SCHEMA_VERSION,
            diagnostics.frame_index,
            diagnostics.graph_valid(),
            diagnostics.pass_count,
            diagnostics.resource_count,
            diagnostics.validation_failure_count(),
        );
        let _ = writeln!(content, "passes:");
        for record in &diagnostics.pass_order {
            let _ = writeln!(
                content,
                "{} {} type={} role={} category={} marker={}",
                record.order,
                record.stable_id,
                record.pass_type.as_str(),
                record.role.as_str(),
                record.diagnostic_category.as_str(),
                record.marker,
            );
        }
        let _ = writeln!(content, "resources:");
        for lifetime in &diagnostics.resource_lifetimes {
            let _ = writeln!(
                content,
                "{} {} type={} first_pass={} last_pass={}",
                lifetime.resource.0,
                lifetime.stable_id,
                lifetime.resource_type.as_str(),
                lifetime.first_pass.0,
                lifetime.last_pass.0,
            );
        }
        let _ = writeln!(content, "validation_failures:");
        for failure in &diagnostics.validation_failures {
            let pass = failure.pass.map_or(u16::MAX, |handle| handle.0);
            let resource = failure.resource.map_or(u16::MAX, |handle| handle.0);
            let _ = writeln!(
                content,
                "{} pass={} resource={}",
                failure.code.as_str(),
                pass,
                resource,
            );
        }
        RendererFrameGraphDebugArtifact {
            schema_version: FRAME_GRAPH_SCHEMA_VERSION,
            frame_index: diagnostics.frame_index,
            content,
        }
    }

    fn pass_mut(&mut self, handle: FrameGraphPassHandle) -> Option<&mut FrameGraphPass> {
        if !handle.is_valid() {
            return None;
        }
        self.passes.get_mut(usize::from(handle.0))
    }

    fn resource_exists(&self, handle: FrameGraphResourceHandle) -> bool {
        handle.is_valid() && self.resources.get(usize::from(handle.0)).is_some()
    }

    fn resource_type(&self, handle: FrameGraphResourceHandle) -> Option<FrameGraphResourceType> {
        self.resource(handle)
            .map(|resource| resource.descriptor.resource_type)
    }

    fn validate_required_resources(&self, failures: &mut Vec<FrameGraphValidationFailure>) {
        // Pass C9.0 — typed conditional resources (typed
        // Lux + typed cloud-owned) are only required when
        // the typed sub-path registers them.  Iterate the
        // typed `is_core_required` set so the typed default
        // frame description passes validation without
        // forcing typed Lux / cloud allocations the typed
        // product surface may not enable.
        for resource_type in FrameGraphResourceType::ALL {
            if !resource_type.is_core_required() {
                continue;
            }
            if self.resource_handle_for_type(resource_type).is_none() {
                failures.push(FrameGraphValidationFailure {
                    code: FrameGraphValidationFailureCode::MissingRequiredResource,
                    pass: None,
                    resource: None,
                });
            }
        }
    }

    fn validate_present_last(&self, failures: &mut Vec<FrameGraphValidationFailure>) {
        let Some(last_pass) = self.passes.iter().rfind(|pass| pass.descriptor.enabled) else {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::PresentPassNotLast,
                pass: None,
                resource: None,
            });
            return;
        };
        if last_pass.descriptor.role != FrameGraphPassRole::Present {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::PresentPassNotLast,
                pass: Some(last_pass.handle),
                resource: None,
            });
        }
    }

    fn validate_scene_ui_separation(&self, failures: &mut Vec<FrameGraphValidationFailure>) {
        for pass in &self.passes {
            let role = pass.descriptor.role;
            let allowed = matches!(
                role,
                FrameGraphPassRole::NativeUiGpuImport
                    | FrameGraphPassRole::UiImportPlaceholder
                    | FrameGraphPassRole::FrameGenerationBoundary
                    | FrameGraphPassRole::Compose
            );
            if allowed {
                continue;
            }
            if pass.reads.iter().chain(pass.writes.iter()).any(|resource| {
                self.resource_type(*resource) == Some(FrameGraphResourceType::UiColorAlpha)
            }) {
                failures.push(FrameGraphValidationFailure {
                    code: FrameGraphValidationFailureCode::UiSceneSeparationBroken,
                    pass: Some(pass.handle),
                    resource: self.resource_handle_for_type(FrameGraphResourceType::UiColorAlpha),
                });
            }
        }
    }

    fn validate_compose_contract(&self, failures: &mut Vec<FrameGraphValidationFailure>) {
        let Some(compose) = self.pass_handle_for_role(FrameGraphPassRole::Compose) else {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::ComposeContractBroken,
                pass: None,
                resource: None,
            });
            return;
        };
        let reads_scene = self
            .pass_reads_resource_type(compose, FrameGraphResourceType::DisplayResolutionSceneColor);
        let reads_ui = self.pass_reads_resource_type(compose, FrameGraphResourceType::UiColorAlpha);
        let writes_final =
            self.pass_writes_resource_type(compose, FrameGraphResourceType::FinalComposedOutput);
        if !(reads_scene && reads_ui && writes_final) {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::ComposeContractBroken,
                pass: Some(compose),
                resource: None,
            });
        }
    }

    fn validate_present_contract(&self, failures: &mut Vec<FrameGraphValidationFailure>) {
        let Some(present) = self.pass_handle_for_role(FrameGraphPassRole::Present) else {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::PresentContractBroken,
                pass: None,
                resource: None,
            });
            return;
        };
        if !self.pass_reads_resource_type(present, FrameGraphResourceType::FinalComposedOutput) {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::PresentContractBroken,
                pass: Some(present),
                resource: self
                    .resource_handle_for_type(FrameGraphResourceType::FinalComposedOutput),
            });
        }
    }

    fn validate_upscale_contract(&self, failures: &mut Vec<FrameGraphValidationFailure>) {
        let Some(upscale) = self.pass_handle_for_role(FrameGraphPassRole::UpscaleBoundary) else {
            return;
        };
        let required_reads = [
            FrameGraphResourceType::RenderResolutionSceneColor,
            FrameGraphResourceType::Depth,
            FrameGraphResourceType::MotionVectors,
            FrameGraphResourceType::Exposure,
            FrameGraphResourceType::ReactiveMask,
            FrameGraphResourceType::TransparencyMask,
            FrameGraphResourceType::HdrMetadata,
        ];
        for resource_type in required_reads {
            if !self.pass_reads_resource_type(upscale, resource_type) {
                failures.push(FrameGraphValidationFailure {
                    code: FrameGraphValidationFailureCode::UpscaleContractBroken,
                    pass: Some(upscale),
                    resource: self.resource_handle_for_type(resource_type),
                });
            }
        }
        if !self
            .pass_writes_resource_type(upscale, FrameGraphResourceType::DisplayResolutionSceneColor)
        {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::UpscaleContractBroken,
                pass: Some(upscale),
                resource: self
                    .resource_handle_for_type(FrameGraphResourceType::DisplayResolutionSceneColor),
            });
        }
        if self.pass_reads_resource_type(upscale, FrameGraphResourceType::UiColorAlpha) {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::UiSceneSeparationBroken,
                pass: Some(upscale),
                resource: self.resource_handle_for_type(FrameGraphResourceType::UiColorAlpha),
            });
        }
    }

    fn validate_frame_generation_contract(&self, failures: &mut Vec<FrameGraphValidationFailure>) {
        let Some(frame_generation) =
            self.pass_handle_for_role(FrameGraphPassRole::FrameGenerationBoundary)
        else {
            return;
        };
        let Some(upscale) = self.pass_handle_for_role(FrameGraphPassRole::UpscaleBoundary) else {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::FrameGenerationContractBroken,
                pass: Some(frame_generation),
                resource: None,
            });
            return;
        };
        if upscale.0 >= frame_generation.0 {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::FrameGenerationContractBroken,
                pass: Some(frame_generation),
                resource: None,
            });
        }
        let required = [
            FrameGraphResourceType::DisplayResolutionSceneColor,
            FrameGraphResourceType::UiColorAlpha,
            FrameGraphResourceType::Depth,
            FrameGraphResourceType::MotionVectors,
            FrameGraphResourceType::FrameTiming,
            FrameGraphResourceType::PresentResources,
            FrameGraphResourceType::FrameGenerationResetFlags,
        ];
        for resource_type in required {
            if !self.pass_reads_resource_type(frame_generation, resource_type) {
                failures.push(FrameGraphValidationFailure {
                    code: FrameGraphValidationFailureCode::FrameGenerationContractBroken,
                    pass: Some(frame_generation),
                    resource: self.resource_handle_for_type(resource_type),
                });
            }
        }
        for resource_type in [
            FrameGraphResourceType::HistoryBuffer,
            FrameGraphResourceType::PresentableFrames,
            FrameGraphResourceType::PacingDiagnostics,
        ] {
            if !self.pass_writes_resource_type(frame_generation, resource_type) {
                failures.push(FrameGraphValidationFailure {
                    code: FrameGraphValidationFailureCode::FrameGenerationContractBroken,
                    pass: Some(frame_generation),
                    resource: self.resource_handle_for_type(resource_type),
                });
            }
        }
        if self.pass_reads_resource_type(
            frame_generation,
            FrameGraphResourceType::FinalComposedOutput,
        ) {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::FrameGenerationContractBroken,
                pass: Some(frame_generation),
                resource: self
                    .resource_handle_for_type(FrameGraphResourceType::FinalComposedOutput),
            });
        }
        let Some(compose) = self.pass_handle_for_role(FrameGraphPassRole::Compose) else {
            return;
        };
        if !self.pass_reads_resource_type(compose, FrameGraphResourceType::PresentableFrames) {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::FrameGenerationContractBroken,
                pass: Some(compose),
                resource: self.resource_handle_for_type(FrameGraphResourceType::PresentableFrames),
            });
        }
        let Some(present) = self.pass_handle_for_role(FrameGraphPassRole::Present) else {
            return;
        };
        if !self.pass_reads_resource_type(present, FrameGraphResourceType::PresentResources) {
            failures.push(FrameGraphValidationFailure {
                code: FrameGraphValidationFailureCode::FrameGenerationContractBroken,
                pass: Some(present),
                resource: self.resource_handle_for_type(FrameGraphResourceType::PresentResources),
            });
        }
    }

    fn resource_lifetimes(&self) -> Vec<FrameGraphResourceLifetime> {
        let mut lifetimes = Vec::new();
        for resource in &self.resources {
            let mut first_pass = None;
            let mut last_pass = None;
            for pass in &self.passes {
                if !pass.descriptor.enabled {
                    continue;
                }
                let used =
                    pass.reads.contains(&resource.handle) || pass.writes.contains(&resource.handle);
                if used {
                    first_pass.get_or_insert(pass.handle);
                    last_pass = Some(pass.handle);
                }
            }
            if let (Some(first_pass), Some(last_pass)) = (first_pass, last_pass) {
                lifetimes.push(FrameGraphResourceLifetime {
                    resource: resource.handle,
                    stable_id: resource.descriptor.stable_id,
                    resource_type: resource.descriptor.resource_type,
                    first_pass,
                    last_pass,
                });
            }
        }
        lifetimes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_graph_initial_passes_keep_scene_ui_and_present_separate() {
        let graph = RendererFrameGraph::from_frame_description(RendererFrameDescription::default());
        let diagnostics = graph.execute();

        assert!(diagnostics.graph_valid());
        // The default description now ships a renderer-owned post stack: tone
        // mapping + final-output transform so the swapchain only ever sees
        // post-process output, not raw scene color.
        assert_eq!(diagnostics.pass_count, 7);
        // Pass C9.0 — the typed default frame graph allocates only the
        // typed core-required resources (typed scene/ui/present/etc.).
        // The typed Lux + typed cloud-owned resources are typed
        // conditional — registered only by the typed Lux + typed cloud
        // sub-paths when they opt in.
        let core_required_count = FrameGraphResourceType::ALL
            .iter()
            .filter(|kind| kind.is_core_required())
            .count();
        assert_eq!(diagnostics.resource_count, core_required_count as u16,);

        let roles: Vec<FrameGraphPassRole> = diagnostics
            .pass_order
            .iter()
            .map(|record| record.role)
            .collect();
        assert_eq!(
            roles,
            [
                FrameGraphPassRole::Clear,
                FrameGraphPassRole::StaticScenePlaceholder,
                FrameGraphPassRole::NativeUiGpuImport,
                FrameGraphPassRole::PostProcessToneMapping,
                FrameGraphPassRole::PostProcessFinalOutputTransform,
                FrameGraphPassRole::Compose,
                FrameGraphPassRole::Present,
            ]
        );

        let compose = graph
            .pass_handle_for_role(FrameGraphPassRole::Compose)
            .expect("compose pass should exist");
        assert!(graph.pass_reads_resource_type(
            compose,
            FrameGraphResourceType::DisplayResolutionSceneColor
        ));
        assert!(graph.pass_reads_resource_type(compose, FrameGraphResourceType::UiColorAlpha));
        assert!(
            graph.pass_writes_resource_type(compose, FrameGraphResourceType::FinalComposedOutput)
        );

        let present = graph
            .pass_handle_for_role(FrameGraphPassRole::Present)
            .expect("present pass should exist");
        assert!(
            graph.pass_reads_resource_type(present, FrameGraphResourceType::FinalComposedOutput)
        );
        assert_eq!(present.0, diagnostics.pass_count - 1);
    }

    #[test]
    fn frame_graph_vendor_virtual_readback_slots_receive_required_inputs() {
        let description = RendererFrameDescription::static_scene_with_ui(7)
            .with_virtual_resources(true)
            .with_upscaling(true)
            .with_frame_generation(true)
            .with_diagnostics_readback(true);
        let graph = RendererFrameGraph::from_frame_description(description);
        let diagnostics = graph.execute();

        assert!(diagnostics.graph_valid());
        assert!(
            diagnostics
                .pass_order
                .iter()
                .any(|record| record.pass_type == FrameGraphPassType::Compute
                    && record.role == FrameGraphPassRole::VirtualResourceFeedback)
        );
        assert!(
            diagnostics
                .pass_order
                .iter()
                .any(|record| record.pass_type == FrameGraphPassType::VendorSdk
                    && record.role == FrameGraphPassRole::UpscaleBoundary)
        );
        assert!(
            diagnostics
                .pass_order
                .iter()
                .any(|record| record.pass_type == FrameGraphPassType::VendorSdk
                    && record.role == FrameGraphPassRole::FrameGenerationBoundary)
        );
        assert!(
            diagnostics
                .pass_order
                .iter()
                .any(|record| record.pass_type == FrameGraphPassType::Readback)
        );

        let frame_generation = graph
            .pass_handle_for_role(FrameGraphPassRole::FrameGenerationBoundary)
            .expect("frame-generation boundary should exist");
        for resource_type in [
            FrameGraphResourceType::DisplayResolutionSceneColor,
            FrameGraphResourceType::UiColorAlpha,
            FrameGraphResourceType::Depth,
            FrameGraphResourceType::MotionVectors,
            FrameGraphResourceType::FrameTiming,
            FrameGraphResourceType::PresentResources,
            FrameGraphResourceType::FrameGenerationResetFlags,
        ] {
            assert!(
                graph.pass_reads_resource_type(frame_generation, resource_type),
                "{}",
                resource_type.as_str()
            );
        }
        for resource_type in [
            FrameGraphResourceType::HistoryBuffer,
            FrameGraphResourceType::PresentableFrames,
            FrameGraphResourceType::PacingDiagnostics,
        ] {
            assert!(
                graph.pass_writes_resource_type(frame_generation, resource_type),
                "{}",
                resource_type.as_str()
            );
        }

        let present = diagnostics.pass_order.last().expect("present pass");
        assert_eq!(present.role, FrameGraphPassRole::Present);
    }

    #[test]
    fn frame_graph_debug_artifact_names_passes_resources_and_markers() {
        let description = RendererFrameDescription::static_scene_with_ui(11)
            .with_upscaling(true)
            .with_frame_generation(true);
        let graph = RendererFrameGraph::from_frame_description(description);
        let diagnostics = graph.execute();
        let artifact = graph.debug_artifact(&diagnostics);

        assert!(artifact.content.contains("fun_renderer.pass.clear"));
        assert!(artifact.content.contains("fun_renderer.pass.compose"));
        assert!(
            artifact
                .content
                .contains("fun_renderer.resource.ui_color_alpha")
        );
        assert!(
            artifact
                .content
                .contains("fun_renderer::frame_graph::frame_generation_boundary")
        );
        assert!(artifact.content.contains("graph_valid=true"));

        if let Some(path) = std::env::var_os("FUN_RENDERER_FRAME_GRAPH_DEBUG_ARTIFACT") {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent)
                    .expect("debug artifact parent should be creatable when requested");
            }
            std::fs::write(path, artifact.content.as_bytes())
                .expect("debug artifact should be writable when requested");
        }
    }

    #[test]
    fn frame_graph_registration_api_reports_ui_scene_contract_failure() {
        let mut graph = RendererFrameGraph::default();
        let ui = graph.declare_resource(FrameGraphResourceDescriptor::new(
            "test.resource.ui",
            FrameGraphResourceType::UiColorAlpha,
            "ui",
        ));
        for resource_type in FrameGraphResourceType::ALL {
            if resource_type != FrameGraphResourceType::UiColorAlpha {
                graph.declare_resource(FrameGraphResourceDescriptor::new(
                    resource_type.as_str(),
                    resource_type,
                    resource_type.as_str(),
                ));
            }
        }
        let scene = graph.register_pass(FrameGraphPassDescriptor::new(
            "test.pass.scene",
            FrameGraphPassType::Render,
            FrameGraphPassRole::StaticScenePlaceholder,
            "scene",
            FrameGraphDiagnosticCategory::Scene,
            None,
            "test::scene",
        ));
        graph.add_pass_write(scene, ui);
        graph.register_pass(FrameGraphPassDescriptor::new(
            "test.pass.present",
            FrameGraphPassType::Presentation,
            FrameGraphPassRole::Present,
            "present",
            FrameGraphDiagnosticCategory::Presentation,
            None,
            "test::present",
        ));

        let failures = graph.validate();
        assert!(failures.iter().any(|failure| {
            failure.code == FrameGraphValidationFailureCode::UiSceneSeparationBroken
        }));
    }
}
