#[cfg(all(feature = "dx12_dlss_native", not(target_os = "windows")))]
compile_error!("fun_render/dx12_dlss_native is a Windows-only experimental feature");
#[cfg(all(feature = "dx12_native_interop", not(target_os = "windows")))]
compile_error!("fun_render/dx12_native_interop is a Windows-only experimental feature");

pub mod bridge;
pub mod capabilities;
mod catalog;
mod compiled_world;
mod composition;
pub mod compute_culling;
mod config;
pub mod core;
mod dlss_correctness;
mod dx12_dlss_rr;
mod dx12_dlss_sr;
#[cfg(all(target_os = "windows", feature = "dx12_native_interop"))]
pub mod dx12_native;
pub mod entity_render_strategy;
pub mod extraction;
pub mod gpu_visibility;
pub mod hiz_occlusion;
pub mod indirect_draw;
pub mod instance_tables;
pub mod lighting;
pub mod lux_extraction;
pub mod material_pipeline;
#[cfg(feature = "offscreen")]
pub mod offscreen;
mod pipeline_warmup;
pub mod residency;
pub mod settings_bridge;
mod signature;
pub mod sky;
pub mod solari;
pub mod static_batches;
pub mod upload_arena;
pub mod upload_budget;
pub mod upload_labels;
pub mod upload_ranges;
pub mod upload_report;
pub mod virtual_geometry;
#[cfg(feature = "winit_presentation")]
pub mod winit;
pub mod world_stream;

pub use bridge::{
    BRIDGE_HOOKS, BridgeFeatureToggles, BridgeHookDescriptor, BridgeHookKind,
    FUN_RENDER_BRIDGE_API_SCHEMA_VERSION, RendererBridgeFrameGraphReport, RendererBridgeHooks,
    RendererBridgeRuntimeState, RendererBridgeSettings, install_renderer_bridge_api,
    renderer_bridge_benchmark_noop, renderer_bridge_debug_overlay_noop,
    renderer_bridge_extract_noop, renderer_bridge_frame_description_from_settings,
    renderer_bridge_initialize_runtime,
};
pub use capabilities::{
    BevyBackendCapabilityReport, BridgeFeatureFlagReport, RENDERER_CAPABILITY_REPORT_PATH_ENV,
    RENDERER_CAPABILITY_REPORT_SCHEMA, RendererCapabilityReport, emit_renderer_capability_report,
};
#[cfg(all(feature = "diagnostics", debug_assertions))]
pub use catalog::catalog_ref_summary;
pub use catalog::{
    CompiledRenderAsset, MaterialAlphaModeKey, MaterialHandleCache, MaterialKey,
    WorldRenderCatalog, prewarm_world_render_catalog, warn_missing_catalog_ref,
};
pub use compiled_world::{CompiledStaticAsset, CompiledWorldPackage, CompiledWorldPackageId};
pub use composition::{
    FUN_RENDER_CEF_UI_STAGE, FUN_RENDER_CEF_UI_Z_INDEX, FUN_RENDER_COMPOSITION_ORDER,
    FUN_RENDER_DEBUG_OVERLAY_STAGE, FUN_RENDER_DEBUG_OVERLAY_Z_INDEX, FUN_RENDER_HUD_UI_STAGE,
    FUN_RENDER_HUD_UI_Z_INDEX, FunRenderCompositionStage,
};
pub use compute_culling::{
    FUN_COMPUTE_CULLING_BUFFER_PLAN, FUN_COMPUTE_CULLING_SCHEMA_VERSION,
    FUN_COMPUTE_CULLING_SHADER, FUN_COMPUTE_CULLING_WORKGROUP_SIZE, FUN_GPU_CULLING_ENV,
    FunComputeCullingBenchmarkSample, FunComputeCullingBufferDescriptor,
    FunComputeCullingBufferKind, FunComputeCullingBufferLifetime, FunComputeCullingConfig,
    FunComputeCullingDecisionInput, FunComputeCullingFrameReport, FunComputeCullingPipelineLayout,
    FunComputeCullingPipelines, FunComputeCullingPolicy, FunComputeCullingRuntimeMode,
    FunComputeCullingStage, FunCullingBucketKey, FunCullingBucketRecord, FunCullingCpuCandidate,
    FunCullingCpuPlane, FunCullingCpuReferenceOutput, FunCullingExecutionPath,
    FunGpuCullingInstanceMetadata, FunGpuCullingViewConstants, FunGpuDispatchIndirectArgs,
    FunGpuDrawIndirectArgs, FunGpuMeshletClusterMetadata, FunGpuVisibilityCounters,
    init_compute_culling_pipelines, install_fun_compute_culling,
    load_compute_culling_shader_assets, simulate_compute_culling_cpu_reference,
    stable_sort_culling_buckets,
};
pub use config::{
    ClientOpaqueRenderer, ClientRenderConfig, ClientRenderProfile, ClientWindowConfig,
    FunRenderAppOptions, FunRenderPresentation, FunRenderRtFeatures, NativeDlssConfig,
    NativeDlssMode, RenderBackendSelectionFacts, RenderGeometryClass, RenderGeometryPolicy,
    RtHairMode, RtMegaGeometryMode, RtOpacityMaskMode, RtVendorEmulation,
    log_native_dlss_startup_diagnostics, selected_max_frame_latency, selected_present_mode,
    selected_render_backend,
};
pub use core::{
    FunDrawCallCounters, FunDrawCallRecord, FunDrawSubmissionKind, FunRenderCorePlugin,
    FunRenderPhaseKind, enable_solari_lighting_for_ready_world, install_fun_render_core,
    request_solari_lighting_history_reset,
};
pub use dlss_correctness::{
    DlssCameraValidation, DlssDebugVisualization, DlssDepthConvention, DlssDepthDiagnostics,
    DlssHistoryReset, DlssMipBiasState, DlssMotionVectorConvention, DlssMotionVectorDirection,
    DlssMotionVectorUnits, DlssResetReason, Dx12DlssPreviousViewProjection, Dx12NativeDlssCamera,
    Dx12NativeDlssCameraRuntimeState, is_dlss_supported_color_format, native_dlss_input_resolution,
    native_dlss_internal_scale_factor, native_dlss_mip_bias, native_dlss_mip_bias_state,
    validate_camera_for_dx12_dlss,
};
pub use dx12_dlss_rr::{
    Dx12NativeDlssRrGateRejection, Dx12NativeDlssRrGuideSurfaceSpec,
    Dx12NativeDlssRrGuideSurfaceStatus, Dx12NativeDlssRrStatus, Dx12NativeDlssRrSupport,
    install_dx12_native_dlss_rr, solari_rr_guide_surface_audit,
};
pub use dx12_dlss_sr::{
    Dx12NativeDlssSrFailure, Dx12NativeDlssSrNode, Dx12NativeDlssSrOutputDescriptor,
    Dx12NativeDlssSrRuntimeMode, Dx12NativeDlssSrState, Dx12NativeDlssSrStatus,
    Dx12NativeDlssSrSupport, Dx12NativeDlssSrTransition, Dx12NativeDlssSrView,
    install_dx12_native_dlss_sr, log_dx12_native_dlss_sr_support_once,
};
#[cfg(all(target_os = "windows", feature = "dx12_native_interop"))]
pub use dx12_native::{
    DX12_CEF_SHARED_TEXTURE_RING_DEPTH_DEFAULT, DX12_CEF_TRANSPORT_SCHEMA_VERSION,
    Dx12CefTransportPath, Dx12CefTransportPolicy, Dx12CommandListHandle, Dx12DeviceQueueHandles,
    Dx12DlssResourceStatePlan, Dx12DlssResourceStateRules, Dx12NativeHandles,
    Dx12NativeInteropError, Dx12NativeInteropFailure, Dx12NativeObjectKind,
    Dx12NativeResourceState, Dx12ObjectLabel, Dx12ObjectNameOutcome, Dx12TextureHandle,
    DxgiFormatLike, active_backend_is_dx12, dx12_cef_transport_policy, dx12_object_naming_enabled,
    extract_dx12_native_handles, extract_dx12_texture_handle,
    log_dx12_dlss_resource_state_plan_once, set_dx12_object_name, validate_dx12_backend,
    validate_dx12_device_queue, validate_render_device_dx12_backend, with_dx12_command_list,
    with_dx12_command_list_checked, with_dx12_device_queue, with_dx12_device_queue_checked,
    with_dx12_texture, with_dx12_texture_checked,
};
#[cfg(all(
    target_os = "windows",
    feature = "dx12_native_interop",
    feature = "dx12_mesh_shader_experiment"
))]
pub use dx12_native::{
    DX12_MESH_SHADER_EXPERIMENT_FEATURE, DX12_MESH_SHADER_EXPERIMENT_NATIVE_BOUNDARY,
    DX12_MESH_SHADER_EXPERIMENT_SCHEMA_VERSION, Dx12MeshShaderAdmissionDecision,
    Dx12MeshShaderAdmissionInput, Dx12MeshShaderBenchmarkOutcome, Dx12MeshShaderBenchmarkPolicy,
    Dx12MeshShaderBenchmarkReport, Dx12MeshShaderBenchmarkSample, Dx12MeshShaderBenchmarkScene,
    Dx12MeshShaderClusterInputSummary, Dx12MeshShaderClusterSource, Dx12MeshShaderComparisonPath,
    Dx12MeshShaderDispatchPlan, decide_dx12_mesh_shader_experiment,
    dx12_mesh_shader_native_boundary_valid, evaluate_dx12_mesh_shader_benchmark,
    plan_dx12_mesh_shader_experiment, summarize_funvg_lite_clusters,
};
pub use entity_render_strategy::{
    FUN_ENTITY_RENDER_STRATEGIES, FUN_ENTITY_RENDER_STRATEGY_SCHEMA_VERSION,
    FunDrawCallScalingTarget, FunEntityRenderClass, FunEntityRenderFeatureFlags,
    FunEntityRenderPathTarget, FunEntityRenderStrategy, FunEntityRenderStrategyBudgetInput,
    FunEntityRenderStrategyBudgetReport, FunEntityRenderStrategyRegistry,
    evaluate_entity_render_strategy_budget, fun_entity_render_strategies,
    strategy_for_entity_render_class,
};
pub use extraction::{
    ExtractedToRendererScene, FUN_RENDER_SCENE_EXTRACTION_SCHEMA_VERSION,
    FunRenderSceneExtractionBridge, FunRenderSceneExtractionReport,
    extract_fun_scene_lights_to_renderer_database,
    extract_fun_scene_renderables_to_renderer_database,
};
pub use fun_renderer;
pub use fun_renderer::benchmark::*;
pub use fun_renderer::ecs::*;
pub use fun_renderer::frame_graph::*;
pub use fun_renderer::fun_lux;
pub use fun_renderer::fun_scene;
pub use fun_renderer::heuristics::*;
pub use fun_renderer::parity::*;
pub use fun_renderer::resource::*;
pub use fun_renderer::scene::*;
pub use fun_renderer::settings::*;
pub use fun_renderer::ui::*;
pub use fun_renderer::{
    BackendCapabilities, ClearColorFrame, DeviceBackend, FUN_RENDER_BRIDGE_PACKAGE_NAME,
    FUN_RENDERER_AI_INTERFACE_DESCRIPTORS, FUN_RENDERER_AI_OWNER_PACKAGE_NAME,
    FUN_RENDERER_BACKEND_DESCRIPTORS, FUN_RENDERER_BACKEND_FUTURE_DEFAULT_FLIP_LOCATION,
    FUN_RENDERER_CEF_RUNTIME_POLICY, FUN_RENDERER_CRATE_NAME, FUN_RENDERER_CURRENT_AUTO_RESOLUTION,
    FUN_RENDERER_DYNAMIC_SCENE_TARGET, FUN_RENDERER_FRAME_GENERATION_CONTRACT,
    FUN_RENDERER_LIGHTING_SCALE_POLICY, FUN_RENDERER_PACKAGE_NAME,
    FUN_RENDERER_PRESENTATION_FEATURE_DESCRIPTORS, FUN_RENDERER_PRODUCT_TOPOLOGY,
    FUN_RENDERER_REQUIRES_BEVY_ECS, FUN_RENDERER_RUNTIME_BACKEND_ENV,
    FUN_RENDERER_SCENE_OWNER_PACKAGE_NAME, FUN_RENDERER_SCHEMA_VERSION,
    FUN_RENDERER_SUBSYSTEM_DESCRIPTORS, FUN_RENDERER_UI_RUNTIME_POLICY, FrameGraphInterface,
    FrameGraphSubmission, FunRendererAiInterfaceDescriptor, FunRendererAiInterfaceKind,
    FunRendererAiOwnedSurface, FunRendererBackend, FunRendererBackendDescriptor,
    FunRendererBackendSelection, FunRendererBackendSelectionReason, FunRendererBevyRole,
    FunRendererCefRuntimePolicy, FunRendererDynamicSceneTarget, FunRendererFrameGenerationContract,
    FunRendererFrameGraphStage, FunRendererLightingScalePolicy, FunRendererOwner,
    FunRendererPresentationFeature, FunRendererPresentationFeatureDescriptor,
    FunRendererProductTopology, FunRendererRuntimeBackend, FunRendererSubsystem,
    FunRendererSubsystemDescriptor, FunRendererUiRuntimePolicy, NoopRendererCore, PassDescriptor,
    PassHandle, PassKind, PassRegistry, PresentResult, Presentation,
    RENDERER_CEF_COMPOSITOR_INTERFACE, RENDERER_CORE_API_SCHEMA_VERSION,
    RENDERER_CORE_INTERFACE_MAP, RENDERER_UPLOAD_ARENA_SEAM,
    RENDERER_UPSCALE_FRAME_GENERATION_INTERFACE, RendererCoreBootReport, RendererCoreDiagnostics,
    RendererCoreInterfaceMap, RendererCoreLifecycle, RendererCoreSettings,
    RendererCoreShutdownReport, RendererFeatureToggles, ResourceAllocator, ResourceHandle,
    ResourceKind, ResourceRequest, SceneDatabase, SceneInstanceId, SceneInstanceRecord,
    owner_for_subsystem, pass_kind_for_frame_graph_role,
};
pub use gpu_visibility::{
    FUN_GPU_VISIBILITY_SCHEMA_VERSION, GPU_VIS_OBJECT_CEF_UI, GPU_VIS_OBJECT_DEBUG,
    GPU_VIS_OBJECT_FOLIAGE, GPU_VIS_OBJECT_MESHLET, GPU_VIS_OBJECT_OCCLUDER,
    GPU_VIS_OBJECT_PARTICLE, GPU_VIS_OBJECT_RASTER, GPU_VIS_OBJECT_SKINNED,
    GPU_VIS_OBJECT_STATIC_OPAQUE, GPU_VIS_OBJECT_TRANSPARENT, GPU_VIS_OBJECT_VIEWMODEL,
    GPU_VISIBILITY_BUFFER_PLAN, GPU_VISIBILITY_STATIC_OPAQUE_STAGES, GPU_VISIBILITY_WORKGROUP_SIZE,
    GpuBoundsRecord, GpuCompactionReport, GpuDrawBucket, GpuFrustumCullOutput, GpuFrustumPlane,
    GpuIndirectBuildOutput, GpuInstanceRecord, GpuLodPolicy, GpuLodSelection, GpuMaterialBucket,
    GpuMeshClusterRange, GpuObjectRecord, GpuVisibilityBatchId, GpuVisibilityBufferDescriptor,
    GpuVisibilityBufferKind, GpuVisibilityBufferLifetime, GpuVisibilityCellId,
    GpuVisibilityObjectId, GpuVisibilityStage, GpuVisibilityViewConstants,
    StaticOpaqueGpuVisibilityConfig, StaticOpaqueGpuVisibilityFrame,
    StaticOpaqueGpuVisibilityPolicy, StaticOpaqueVisibilityDecision,
    StaticOpaqueVisibilityDecisionInput, StaticOpaqueVisibilityFallbackReason,
    StaticOpaqueVisibilityPath, build_indirect_buckets_for_static_opaque,
    build_static_opaque_gpu_visibility_frame, compact_visible_instance_ids,
    compact_visible_object_ids, compact_visible_static_opaque, conservative_sphere_visible,
    decide_static_opaque_visibility_path, draw_bucket_key_for_object,
    draw_packet_report_for_static_visibility_frame, frustum_cull_static_objects,
    fun_render_path_from_code, gpu_render_path_code, gpu_visibility_buffer_descriptor,
    select_lod_with_hysteresis, select_lods_for_visible_objects,
};
pub use hiz_occlusion::{
    FUN_HIZ_OCCLUSION_PIPELINE, FUN_HIZ_OCCLUSION_SCHEMA_VERSION, FunHiZOcclusionAdaptiveState,
    FunHiZOcclusionBenchmarkSample, FunHiZOcclusionCullReport, FunHiZOcclusionDecision,
    FunHiZOcclusionFrameInput, FunHiZOcclusionFrameReport, FunHiZOcclusionPayoffSample,
    FunHiZOcclusionPipelineStage, FunHiZOcclusionPolicy, FunHiZOcclusionPrerequisite,
    FunHiZOcclusionPrerequisites, FunHiZOcclusionTestCandidate, FunHiZOcclusionTestLevel,
    FunOccluderCandidate, FunOccluderClass, FunOccluderSelectionDecision,
    FunOccluderSelectionPolicy, FunOccluderSelectionReport, FunTemporalOcclusionDecision,
    FunTemporalOcclusionInput, evaluate_occluder, occlusion_cull_after_visibility_culling,
    select_occluders, temporal_occlusion_decision,
};
pub use indirect_draw::{
    FUN_DRAW_CALL_BUDGETS, FUN_INDIRECT_DRAW_SCHEMA_VERSION, FunDepthPrepassMode, FunDrawBucket,
    FunDrawBucketKey, FunDrawBucketOffender, FunDrawBudgetLane, FunDrawCallBudget,
    FunDrawFallbackReason, FunDrawPacketBuildOptions, FunDrawPacketReport, FunDrawPacketSource,
    FunDrawSubmissionCapabilities, FunDrawSubmissionPath, FunDrawSubmissionSink,
    FunDrawSubmissionSummary, FunIndirectArgsBuffer, FunIndirectArgsRange, FunIndirectDrawPacket,
    FunMaterialSignature, FunMeshletFormat, FunMultiDrawGroupKey, FunShaderPipelineSignature,
    FunSkeletalMode, FunTextureTableCompatibility, FunTransparencyMode,
    build_draw_packets_from_compacted_ids, build_draw_packets_from_culling_output,
    draw_call_budget_for_lane, submit_draw_packet_report,
};
pub use instance_tables::{
    DYNAMIC_INSTANCE_RING_FRAMES, DynamicInstanceClass, DynamicInstanceTable,
    FUN_INSTANCE_TABLE_SCHEMA_VERSION, INSTANCE_FLAG_DYNAMIC, INSTANCE_FLAG_HAS_TINT,
    INSTANCE_FLAG_MESHLET, INSTANCE_FLAG_RASTER, INSTANCE_FLAG_RAY_PROXY, INSTANCE_FLAG_SHADOW,
    INSTANCE_FLAG_STATIC, INSTANCE_INDEX_NONE, INSTANCE_SMALL_WRITE_MAX_BYTES, InstanceDirtyRange,
    InstanceDirtyReason, InstanceGpuRecord, InstanceId, InstanceRange,
    InstanceTableArenaWriteRequest, InstanceTableKind, InstanceUploadPath, InstanceUploadPlan,
    StaticInstanceAllocation, StaticInstanceAllocationRecord, StaticInstanceTable,
    write_instance_range_with_existing_encoder,
};
pub use lux_extraction::{
    FUN_RENDER_LUX_EXTRACTION_SCHEMA_VERSION, FunRenderLuxExtractionBridge,
    FunRenderLuxExtractionReport, fun_render_lux_extraction_system,
    install_renderer_bridge_lux_extraction,
};
pub use material_pipeline::{
    DATA_FLAG_CLEARCOAT, DATA_FLAG_EMISSIVE_BOOST, DATA_FLAG_PARALLAX_MAPPING,
    DATA_FLAG_WIND_VERTEX_ANIMATION, DENSE_MESHLET_CLUSTER_PIPELINE, DYNAMIC_OPAQUE_PIPELINE,
    FUN_MATERIAL_PIPELINE_SCHEMA_VERSION, FUN_RENDER_PIPELINE_SIGNATURES,
    MaterialConsolidationReason, MaterialInstancePolicy, MaterialInstanceTint,
    PIPELINE_FEATURE_ALPHA_TEST, PIPELINE_FEATURE_CLEARCOAT, PIPELINE_FEATURE_EMISSIVE_BOOST,
    PIPELINE_FEATURE_PARALLAX_MAPPING, PIPELINE_FEATURE_WIND_VERTEX_ANIMATION, RenderBatchKey,
    RenderBatchShadowMode, RenderPipelineFeaturePolicy, RenderPipelineFeatureRequest,
    RenderPipelineSignatureCatalog, RenderPipelineSignatureDescriptor, RenderPipelineSignatureKind,
    SIMPLE_OPAQUE_STATIC_PIPELINE, TRANSPARENT_PIPELINE, TextureTableId, VIEWMODEL_PIPELINE,
    clamp_pipeline_features_for_visual_importance, count_unique_pipeline_signatures,
    material_policy_for_catalog_entry, material_policy_for_stream_color,
    material_signature_for_key, mesh_format_for_render_class, pipeline_signature_for_render_class,
    render_batch_key_for_catalog_entry, signature_bucket_u32, sort_render_batch_keys,
};
#[cfg(feature = "offscreen")]
pub use offscreen::{EditorOffscreenRenderTarget, FunRenderOffscreenPresentationPlugin};
pub use pipeline_warmup::{FunPipelineWarmupConfig, FunPipelineWarmupMode};
pub use residency::{
    FUN_RESIDENCY_SCHEMA_VERSION, FunResidencyBudget, FunResidencyContentKind,
    FunResidencyEvictionDecision, FunResidencyFrameContext, FunResidencyFrameReport,
    FunResidencyImportanceTag, FunResidencyPageState, FunResidencyPatchOutcome,
    FunResidencyPriorityInput, FunResidencyUploadDecision, FunResidencyUploadUrgency,
    GeometryPageId, GeometryPageUploadRequest, GeometryResidencyManager, GeometryResidencyPage,
    MaterialEntryId, MaterialResidencyManager, MaterialResidencyUpdate, TexturePageId,
    TexturePageUploadRequest, TextureResidencyManager, TextureResidencyPage, plan_residency_frame,
    residency_budget_for_lane, residency_upload_urgency,
};
pub use settings_bridge::{
    RENDERER_SETTINGS_UI_BRIDGE_SCHEMA, RendererSettingsBenchmarkReproLabel,
    RendererSettingsUiDisabledReason, RendererSettingsUiModel, RendererSettingsUiOption,
    RendererSettingsUiOptionReason, RendererSettingsUiSelection,
    renderer_settings_ui_model_from_bridge, virtual_geometry_budget_to_ui_label,
};
pub use signature::{
    FunGeometryClass, FunMaterialClass, FunRenderDistanceBand, FunRenderPath, FunRenderPathArbiter,
    FunRenderPathInput, RenderPathSignature, render_path_signature_for_options,
};
pub use sky::{
    FunCloudDebugOverlay, FunCloudHistoryResetEvent, FunCloudHistoryResetReason,
    FunCloudHistoryState, FunCloudInternalScale, FunCloudQuality, FunCloudSettings,
    FunCloudTypeMix, FunSkyPlugin, FunWeatherPattern, FunWeatherPatternPhase, FunWeatherProfile,
    FunWeatherProfileId, FunWeatherState, FunWeatherTransition, FunWeatherTransitionCurve,
    FunWeatherValidationError, request_cloud_history_reset,
};
pub use solari::benchmark_parse_solari_denoise_mode;
pub use solari::{
    parse_solari_denoise_mode, solari_runtime_params_from_env, solari_settings_from_env,
};
pub use static_batches::{
    StaticRenderBatch, StaticRenderBatchBounds, StaticRenderBatchBuilder,
    StaticRenderBatchInstanceRange, StaticRenderBatchKey, StaticRenderBatchSpawnRecord,
    StaticRenderCell, StaticRenderCellId, StaticRenderGpuInstanceBufferHandle,
    static_catalog_spec_is_batchable, static_catalog_spec_may_batch,
    static_catalog_spec_needs_identity_proxy,
};
pub use upload_arena::{
    FUN_UPLOAD_ARENA_OWNER_MODULE, FUN_UPLOAD_ARENA_RESOURCE_SCHEMA_VERSION,
    FUN_UPLOAD_ARENA_RESOURCE_SHIM, FunUploadArena, FunUploadArenaLabelStats,
    FunUploadArenaResourceShim, FunUploadArenaStats, FunUploadArenaWriteRequest, UploadArenaError,
    UploadWriteLabel,
};
pub use upload_budget::{
    FunUploadBudget, FunUploadBudgetClass, FunUploadBudgetDecision, FunUploadBudgetTracker,
    FunUploadBudgetUsage, FunUploadSubsystem, FunUploadWriteIntent,
};
pub use upload_labels::{
    FUN_UPLOAD_LABELS, FunUploadExpectedFrequency, FunUploadResourceKind,
    UPLOAD_CEF_CPU_DIRTY_RECT, UPLOAD_CEF_CPU_FULL_FRAME, UPLOAD_CEF_GPU_COPY_METADATA,
    UPLOAD_CEF_METADATA, UPLOAD_CLOUD_PARAMS, UPLOAD_DLSS_CONSTANTS, UPLOAD_DLSS_PARAMS,
    UPLOAD_FRAME_CONSTANTS, UPLOAD_INSTANCE_DIRTY_RANGE, UPLOAD_MESHLET_INSTANCE_RANGE,
    UPLOAD_MESHLET_MATERIAL_RANGE, UPLOAD_SOLARI_PARAMS, UPLOAD_TEXTURE_DIRTY_RECT,
    UPLOAD_VIEW_CONSTANTS, UPLOAD_VIEW_VISIBILITY, UPLOAD_WORLD_STREAM_STATIC_MESH,
    UploadLabelDescriptor, upload_label_descriptor, upload_label_descriptor_or_default,
};
pub use upload_ranges::{
    DEFAULT_DYNAMIC_UPLOAD_SLAB_BYTES, DEFAULT_STATIC_SLAB_RECORDS, DEFAULT_TEXTURE_DIRTY_RECT_CAP,
    DynamicRingUploadSlabs, DynamicUploadSlabAllocation, FUN_UPLOAD_RANGE_SCHEMA_VERSION,
    InstanceSoADirtyRange, InstanceSoAField, PersistentBufferCapacityPlan,
    PersistentBufferGrowthPolicy, PersistentBufferRangeUploadPlan, PersistentBufferUploadPath,
    TextureDirtyRect, TextureUploadPath, TextureUploadPlan, TextureUploadPolicy,
    TextureUploadRequest, instance_dirty_range_intent, merge_instance_dirty_ranges,
    persistent_buffer_capacity_plan, plan_cef_cpu_dirty_rect_upload, plan_dynamic_texture_upload,
    plan_instance_soa_uploads, plan_persistent_buffer_range_upload,
};
pub use upload_report::{
    FunUploadBudgetDecisionCounts, FunUploadFrameReport, FunUploadFrameReportBuilder,
    FunUploadLabelReport,
};
pub use virtual_geometry::{
    FUN_VG_PROTOTYPE_NAME, FUN_VG_RUNTIME_PIPELINE, FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
    VirtualGeometryAcceptanceReport, VirtualGeometryAsset, VirtualGeometryAssetDecision,
    VirtualGeometryAssetId, VirtualGeometryAssetKind, VirtualGeometryBenchmarkSample,
    VirtualGeometryBounds, VirtualGeometryChildRange, VirtualGeometryCluster,
    VirtualGeometryClusterCullHint, VirtualGeometryClusterId, VirtualGeometryDrawPacket,
    VirtualGeometryDrawPath, VirtualGeometryHierarchyNode, VirtualGeometryHierarchyNodeId,
    VirtualGeometryLitePolicy, VirtualGeometryNormalCone, VirtualGeometryPage,
    VirtualGeometryPageBudget, VirtualGeometryPageBudgetUsage, VirtualGeometryPageId,
    VirtualGeometryPageRequest, VirtualGeometryPageRequestDecision,
    VirtualGeometryPageRequestReason, VirtualGeometryRange, VirtualGeometryResidency,
    VirtualGeometryRuntimeStage, VirtualGeometrySelectedCluster, VirtualGeometrySelectionPolicy,
    VirtualGeometrySelectionReason, VirtualGeometryStaticCell, VirtualGeometryViewContext,
    VirtualGeometryViewSelection, VirtualGeometryVisibilityBufferExtensionInput,
    VirtualGeometryVisibilityBufferExtensionPlan, VirtualGeometryVisibilityBufferExtensionStatus,
    build_virtual_geometry_draw_packets, evaluate_virtual_geometry_asset,
    select_virtual_geometry_for_static_cell, select_virtual_geometry_view,
    virtual_geometry_draw_bucket_key, virtual_geometry_screen_space_error,
    virtual_geometry_visibility_buffer_extension_plan,
};
#[cfg(feature = "winit_presentation")]
pub use winit::FunRenderWinitPresentationPlugin;
pub use world_stream::{
    FUN_RENDER_WORLD_PREP_SCHEMA_VERSION, PrimitiveGeometryPolicyKey, PrimitiveRenderCache,
    PrimitiveRenderCacheKey, PrimitiveRenderCacheLookup, PrimitiveRenderCacheLookupKind,
    PrimitiveRenderCachePrimitive, PrimitiveRenderHandles, PrimitiveSizeBucket,
    RENDER_WORLD_PREP_PHASES, RenderWorldApplyOptions, RenderWorldChunkOutcome, RenderWorldContext,
    RenderWorldFallbackProxy, RenderWorldFallbackProxyReason, RenderWorldPrepPhase,
    RenderWorldStatus, apply_render_world_chunk, despawn_render_context,
    prewarm_primitive_render_cache, spawn_render_entity_from_spec,
    update_render_context_visibility,
};

pub const WINIT_PRESENTATION_ENABLED: bool = cfg!(feature = "winit_presentation");
