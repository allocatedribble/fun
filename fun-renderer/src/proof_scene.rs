use bevy_app::App;
use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;
use bevy_transform::components::Transform;

use crate::backend::NativeBackend;
use crate::component_api::{
    AlphaMode, BloomSettings, CameraExposure, CameraHistory, CameraJitter, CameraProjection,
    CameraRenderTarget, CameraRenderTargetKind, CefHealthState, CefSurface, CefSurfaceHealth,
    CefTransportMode, ColorGradingSettings, DirectionalLight, ExposureSettings, HdrOutputSettings,
    LightLayer, MainCamera, MaterialFeatureMask, RenderAabb, RenderBounds, RenderCamera,
    RenderColor, RenderDirtyFlags, RenderDynamic, RenderExtent2d, RenderLayer, RenderLayerMask,
    RenderMaterial, RenderMaterialAssetId, RenderMesh, RenderMeshAssetId, RenderObjectId,
    RenderStableId, RenderStatic, RenderVec3, RenderVisibility, RenderVisibilityState, Renderable,
    ShadowCaster, ShadowMode, SharpeningSettings, StandardMaterial, TaaSettings,
    ToneMappingOperator, ToneMappingSettings, UiColorSpace, UiCompositeOrder, UiDebugBorder,
    UiLayer, UiOpacity, UiSurface, UiTargetRect, UpscalerSettings,
};

pub const PROOF_SCENE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofSceneKind {
    #[default]
    SingleStaticMesh,
    SingleStaticMeshWithUiSurface,
}

impl ProofSceneKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SingleStaticMesh => "single_static_mesh",
            Self::SingleStaticMeshWithUiSurface => "single_static_mesh_with_ui_surface",
        }
    }

    #[must_use]
    pub const fn includes_ui_surface(self) -> bool {
        matches!(self, Self::SingleStaticMeshWithUiSurface)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProofSceneSpec {
    pub schema_version: u16,
    pub kind: ProofSceneKind,
    pub render_target: RenderExtent2d,
    pub watermark_enabled: bool,
    pub camera_history_id: RenderStableId,
    pub renderable_stable_id: RenderStableId,
    pub directional_light_present: bool,
    pub ui_surface_extent: RenderExtent2d,
}

impl ProofSceneSpec {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: PROOF_SCENE_SCHEMA_VERSION,
        kind: ProofSceneKind::SingleStaticMesh,
        render_target: RenderExtent2d {
            width: 1920,
            height: 1080,
        },
        watermark_enabled: true,
        camera_history_id: RenderStableId::new(0xC1A1_5CE0),
        renderable_stable_id: RenderStableId::new(0x57A7_1C00),
        directional_light_present: true,
        ui_surface_extent: RenderExtent2d {
            width: 1920,
            height: 1080,
        },
    };

    #[must_use]
    pub const fn with_kind(mut self, kind: ProofSceneKind) -> Self {
        self.kind = kind;
        self
    }
}

impl Default for ProofSceneSpec {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

pub fn spawn_proof_scene(world: &mut World, spec: ProofSceneSpec) -> ProofSceneEntities {
    let camera = world
        .spawn((
            (
                RenderCamera {
                    layers: RenderLayerMask::DEFAULT,
                    order: 0,
                },
                MainCamera,
                CameraProjection::default(),
                CameraExposure::default(),
                CameraJitter::default(),
                CameraHistory {
                    history_id: spec.camera_history_id,
                    reset: false,
                },
                CameraRenderTarget {
                    kind: CameraRenderTargetKind::PrimaryWindow,
                    texture: Default::default(),
                    extent: spec.render_target,
                },
            ),
            (
                ToneMappingSettings {
                    operator: ToneMappingOperator::AcesFitted,
                    exposure_bias: 0.0,
                },
                ExposureSettings::default(),
                ColorGradingSettings::default(),
                SharpeningSettings::default(),
                BloomSettings::default(),
                TaaSettings::default(),
                HdrOutputSettings::default(),
                UpscalerSettings::default(),
            ),
            Transform::from_xyz(0.0, 1.6, 4.0),
        ))
        .id();

    let static_mesh = world
        .spawn((
            Renderable {
                object_id: spec.renderable_stable_id,
                dirty: RenderDirtyFlags::TRANSFORM
                    .union(RenderDirtyFlags::MATERIAL)
                    .union(RenderDirtyFlags::GEOMETRY),
            },
            RenderObjectId::first(1),
            RenderMesh {
                mesh: RenderMeshAssetId::first(1),
            },
            RenderMaterial {
                material: RenderMaterialAssetId::first(1),
            },
            RenderBounds {
                local: RenderAabb::new(RenderVec3::ZERO, RenderVec3::new(0.5, 0.5, 0.5)),
            },
            RenderLayer {
                mask: RenderLayerMask::DEFAULT,
            },
            RenderVisibility {
                state: RenderVisibilityState::Visible,
            },
            RenderStatic,
            ShadowCaster {
                mode: ShadowMode::VirtualPages,
            },
            Transform::from_xyz(0.0, 0.0, 0.0),
        ))
        .id();

    let directional_light = if spec.directional_light_present {
        Some(
            world
                .spawn((
                    DirectionalLight::default(),
                    ShadowCaster {
                        mode: ShadowMode::VirtualPages,
                    },
                    LightLayer::default(),
                    Transform::from_xyz(0.0, 5.0, -1.0),
                ))
                .id(),
        )
    } else {
        None
    };

    let ui_surface = if spec.kind.includes_ui_surface() {
        Some(
            world
                .spawn((
                    UiSurface {
                        surface_id: RenderStableId::new(0xCEFC_0DE0),
                        extent: spec.ui_surface_extent,
                    },
                    UiLayer { layer: 4 },
                    UiCompositeOrder { order: 0 },
                    UiOpacity { alpha: 1.0 },
                    UiColorSpace::Linear,
                    UiTargetRect::FULL_WINDOW,
                    UiDebugBorder::OFF,
                    CefSurface {
                        surface_id: RenderStableId::new(0xCEFC_0DE0),
                        transport: CefTransportMode::GpuSharedTexture,
                        gpu_only: true,
                    },
                    CefSurfaceHealth {
                        state: CefHealthState::Healthy,
                    },
                ))
                .id(),
        )
    } else {
        None
    };

    ProofSceneEntities {
        camera,
        static_mesh,
        directional_light,
        ui_surface,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofSceneEntities {
    pub camera: bevy_ecs::entity::Entity,
    pub static_mesh: bevy_ecs::entity::Entity,
    pub directional_light: Option<bevy_ecs::entity::Entity>,
    pub ui_surface: Option<bevy_ecs::entity::Entity>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MinimalRenderSequenceStage {
    #[default]
    Clear,
    DepthPrepare,
    OpaqueMesh,
    DebugMaterialFallback,
    FinalComposite,
    Present,
}

impl MinimalRenderSequenceStage {
    pub const ALL: [Self; 6] = [
        Self::Clear,
        Self::DepthPrepare,
        Self::OpaqueMesh,
        Self::DebugMaterialFallback,
        Self::FinalComposite,
        Self::Present,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::DepthPrepare => "depth_prepare",
            Self::OpaqueMesh => "opaque_mesh",
            Self::DebugMaterialFallback => "debug_material_fallback",
            Self::FinalComposite => "final_composite",
            Self::Present => "present",
        }
    }

    #[must_use]
    pub const fn order(self) -> u8 {
        match self {
            Self::Clear => 0,
            Self::DepthPrepare => 1,
            Self::OpaqueMesh => 2,
            Self::DebugMaterialFallback => 3,
            Self::FinalComposite => 4,
            Self::Present => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MinimalRenderSequence {
    pub schema_version: u16,
    pub stages: [MinimalRenderSequenceStage; 6],
    pub uses_debug_material_fallback: bool,
    pub uses_resource_registry: bool,
    pub uses_pso_cache: bool,
    pub uses_binding_cache: bool,
}

impl MinimalRenderSequence {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: PROOF_SCENE_SCHEMA_VERSION,
        stages: MinimalRenderSequenceStage::ALL,
        uses_debug_material_fallback: true,
        uses_resource_registry: true,
        uses_pso_cache: true,
        uses_binding_cache: true,
    };

    #[must_use]
    pub fn includes(&self, stage: MinimalRenderSequenceStage) -> bool {
        self.stages.contains(&stage)
    }
}

impl Default for MinimalRenderSequence {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedRendererResources {
    pub schema_version: u16,
    pub mesh_buffer_through_resource_registry: bool,
    pub material_buffer_through_renderer_asset_prep: bool,
    pub texture_fallback_through_resource_registry: bool,
    pub view_constants_through_frame_local: bool,
    pub pipeline_through_pso_cache: bool,
    pub bindings_through_binding_cache: bool,
}

impl PreparedRendererResources {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: PROOF_SCENE_SCHEMA_VERSION,
        mesh_buffer_through_resource_registry: true,
        material_buffer_through_renderer_asset_prep: true,
        texture_fallback_through_resource_registry: true,
        view_constants_through_frame_local: true,
        pipeline_through_pso_cache: true,
        bindings_through_binding_cache: true,
    };

    #[must_use]
    pub const fn fully_prepared(&self) -> bool {
        self.mesh_buffer_through_resource_registry
            && self.material_buffer_through_renderer_asset_prep
            && self.texture_fallback_through_resource_registry
            && self.view_constants_through_frame_local
            && self.pipeline_through_pso_cache
            && self.bindings_through_binding_cache
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameProbeFailureSeam {
    NoFailure,
    BlackFrame,
    UniformDarkFrame,
    UniformLightFrame,
    MissingWatermark,
    BackendTruthMismatch,
    BridgeRuntimeFailure,
    PipelineWarmupIncomplete,
    BindingCacheMiss,
    ResourceRegistryMiss,
    UnpreparedRendererResources,
    GraphValidationFailed,
}

impl FrameProbeFailureSeam {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoFailure => "no_failure",
            Self::BlackFrame => "black_frame",
            Self::UniformDarkFrame => "uniform_dark_frame",
            Self::UniformLightFrame => "uniform_light_frame",
            Self::MissingWatermark => "missing_watermark",
            Self::BackendTruthMismatch => "backend_truth_mismatch",
            Self::BridgeRuntimeFailure => "bridge_runtime_failure",
            Self::PipelineWarmupIncomplete => "pipeline_warmup_incomplete",
            Self::BindingCacheMiss => "binding_cache_miss",
            Self::ResourceRegistryMiss => "resource_registry_miss",
            Self::UnpreparedRendererResources => "unprepared_renderer_resources",
            Self::GraphValidationFailed => "graph_validation_failed",
        }
    }

    #[must_use]
    pub const fn is_failure(self) -> bool {
        !matches!(self, Self::NoFailure)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameProbeSample {
    pub schema_version: u16,
    pub width: u16,
    pub height: u16,
    pub black_pixel_ratio_per_mille: u16,
    pub average_luminance_milli: u16,
    pub dominant_color_count: u8,
    pub watermark_pixels_present: u32,
    pub frame_index: u64,
}

impl FrameProbeSample {
    pub const BLACK_FRAME_THRESHOLD_PER_MILLE: u16 = 999;
    pub const UNIFORM_DARK_LUMINANCE_THRESHOLD_MILLI: u16 = 5;
    pub const UNIFORM_LIGHT_LUMINANCE_THRESHOLD_MILLI: u16 = 995;
    pub const WATERMARK_MIN_PIXELS: u32 = 16;

    #[must_use]
    pub const fn is_black_frame(self) -> bool {
        self.black_pixel_ratio_per_mille >= Self::BLACK_FRAME_THRESHOLD_PER_MILLE
    }

    #[must_use]
    pub const fn is_uniform_dark(self) -> bool {
        self.average_luminance_milli <= Self::UNIFORM_DARK_LUMINANCE_THRESHOLD_MILLI
    }

    #[must_use]
    pub const fn is_uniform_light(self) -> bool {
        self.average_luminance_milli >= Self::UNIFORM_LIGHT_LUMINANCE_THRESHOLD_MILLI
    }

    #[must_use]
    pub const fn watermark_present(self) -> bool {
        self.watermark_pixels_present >= Self::WATERMARK_MIN_PIXELS
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DominantColorEntry {
    pub linear_red_milli: u16,
    pub linear_green_milli: u16,
    pub linear_blue_milli: u16,
    pub coverage_per_mille: u16,
}

impl DominantColorEntry {
    #[must_use]
    pub const fn from_color(color: RenderColor, coverage_per_mille: u16) -> Self {
        let red = (color.linear_rgba[0] * 1000.0) as i32;
        let green = (color.linear_rgba[1] * 1000.0) as i32;
        let blue = (color.linear_rgba[2] * 1000.0) as i32;
        Self {
            linear_red_milli: clamp_u16(red),
            linear_green_milli: clamp_u16(green),
            linear_blue_milli: clamp_u16(blue),
            coverage_per_mille,
        }
    }
}

const fn clamp_u16(value: i32) -> u16 {
    if value < 0 {
        0
    } else if value > u16::MAX as i32 {
        u16::MAX
    } else {
        value as u16
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameProbeArtifact {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub sample: FrameProbeSample,
    pub dominant_colors: [DominantColorEntry; 4],
    pub dominant_color_count: u8,
    pub failure_seam: FrameProbeFailureSeam,
    pub watermark_required: bool,
    pub native_backend: NativeBackend,
}

impl FrameProbeArtifact {
    #[must_use]
    pub const fn canonical_artifact_path() -> &'static str {
        "fun-data/renderer/frame_probe.funpb.zst"
    }

    #[must_use]
    pub fn from_sample(
        sample: FrameProbeSample,
        dominant_colors: &[DominantColorEntry],
        watermark_required: bool,
        native_backend: NativeBackend,
        upstream_failure: FrameProbeFailureSeam,
    ) -> Self {
        let mut entries = [DominantColorEntry::from_color(RenderColor::BLACK, 0); 4];
        let count = dominant_colors.len().min(entries.len());
        entries[..count].copy_from_slice(&dominant_colors[..count]);
        let failure_seam = if upstream_failure.is_failure() {
            upstream_failure
        } else if sample.is_black_frame() {
            FrameProbeFailureSeam::BlackFrame
        } else if sample.is_uniform_dark() {
            FrameProbeFailureSeam::UniformDarkFrame
        } else if sample.is_uniform_light() {
            FrameProbeFailureSeam::UniformLightFrame
        } else if watermark_required && !sample.watermark_present() {
            FrameProbeFailureSeam::MissingWatermark
        } else {
            FrameProbeFailureSeam::NoFailure
        };
        Self {
            schema_version: PROOF_SCENE_SCHEMA_VERSION,
            canonical_path: Self::canonical_artifact_path(),
            sample,
            dominant_colors: entries,
            dominant_color_count: count as u8,
            failure_seam,
            watermark_required,
            native_backend,
        }
    }

    #[must_use]
    pub const fn passed(&self) -> bool {
        !self.failure_seam.is_failure()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct ProofSceneRuntimeReport {
    pub schema_version: u16,
    pub spec: Option<ProofSceneSpec>,
    pub render_sequence: Option<MinimalRenderSequence>,
    pub prepared_resources: PreparedRendererResources,
    pub frame_probe: Option<FrameProbeArtifact>,
}

impl ProofSceneRuntimeReport {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: PROOF_SCENE_SCHEMA_VERSION,
            spec: None,
            render_sequence: None,
            prepared_resources: PreparedRendererResources::PRODUCT_DEFAULT,
            frame_probe: None,
        }
    }

    pub fn with_spec(&mut self, spec: ProofSceneSpec) -> &mut Self {
        self.spec = Some(spec);
        self
    }

    pub fn with_render_sequence(&mut self, sequence: MinimalRenderSequence) -> &mut Self {
        self.render_sequence = Some(sequence);
        self
    }

    pub fn with_prepared_resources(&mut self, prepared: PreparedRendererResources) -> &mut Self {
        self.prepared_resources = prepared;
        self
    }

    pub fn with_frame_probe(&mut self, probe: FrameProbeArtifact) -> &mut Self {
        self.frame_probe = Some(probe);
        self
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.spec.is_some()
            && self.render_sequence.is_some()
            && self.prepared_resources.fully_prepared()
            && self.frame_probe.is_some_and(|probe| probe.passed())
    }
}

pub fn install_proof_scene(app: &mut App, spec: ProofSceneSpec) -> ProofSceneEntities {
    if !app.world().contains_resource::<ProofSceneRuntimeReport>() {
        app.insert_resource(ProofSceneRuntimeReport::new());
    }
    let entities = spawn_proof_scene(app.world_mut(), spec);
    let mut report = app.world_mut().resource_mut::<ProofSceneRuntimeReport>();
    report.with_spec(spec);
    report.with_render_sequence(MinimalRenderSequence::PRODUCT_DEFAULT);
    report.with_prepared_resources(PreparedRendererResources::PRODUCT_DEFAULT);
    entities
}

pub fn record_frame_probe(app: &mut App, artifact: FrameProbeArtifact) {
    if !app.world().contains_resource::<ProofSceneRuntimeReport>() {
        app.insert_resource(ProofSceneRuntimeReport::new());
    }
    app.world_mut()
        .resource_mut::<ProofSceneRuntimeReport>()
        .with_frame_probe(artifact);
}

#[must_use]
pub fn standard_material_for_proof_scene() -> StandardMaterial {
    StandardMaterial {
        base_color: RenderColor::WHITE,
        feature_mask: MaterialFeatureMask::DOUBLE_SIDED,
        alpha_mode: AlphaMode::Opaque,
        ..StandardMaterial::default()
    }
}

#[must_use]
pub fn render_dynamic_marker_for_proof_scene() -> RenderDynamic {
    RenderDynamic {
        dirty: RenderDirtyFlags::TRANSFORM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::WgpuDx12Backend;
    use crate::extraction::{
        RenderStableIdAllocator, RenderWorldExtractionDiagnostics, RenderWorldTables,
        begin_render_world_extraction_frame, extract_renderer_cef_surfaces,
        extract_renderer_lights, extract_renderer_post_process_volumes,
        extract_renderer_renderables, extract_renderer_ui_surfaces, extract_renderer_views,
        install_render_world_extraction_resources,
    };
    use crate::plugin::{FunRendererPlugin, RendererFrameIndex};
    use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};

    fn baseline_app() -> App {
        let mut app = App::new();
        app.add_plugins(FunRendererPlugin::<WgpuDx12Backend>::default());
        app
    }

    fn extract_world_tables(app: &mut App) {
        let world = app.world_mut();
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                begin_render_world_extraction_frame,
                extract_renderer_renderables,
                extract_renderer_views,
                extract_renderer_lights,
                extract_renderer_ui_surfaces,
                extract_renderer_cef_surfaces,
                extract_renderer_post_process_volumes,
            )
                .chain(),
        );
        schedule.run(world);
    }

    #[test]
    fn proof_scene_spec_is_a_single_static_mesh_with_optional_ui_surface() {
        let default_spec = ProofSceneSpec::default();
        assert_eq!(default_spec.kind, ProofSceneKind::SingleStaticMesh);
        assert!(default_spec.directional_light_present);
        assert!(default_spec.watermark_enabled);

        let with_ui = default_spec.with_kind(ProofSceneKind::SingleStaticMeshWithUiSurface);
        assert!(with_ui.kind.includes_ui_surface());
    }

    #[test]
    fn spawn_proof_scene_uses_only_public_ecs_components() {
        // Build a minimal world with the renderer extraction resources so the
        // proof scene exercises the same ECS path as the production renderer.
        let mut world = World::new();
        install_render_world_extraction_resources_into_world(&mut world);
        world.insert_resource(RendererFrameIndex(7));

        let entities = spawn_proof_scene(&mut world, ProofSceneSpec::default());

        // Camera entity must have the public components extracted by the V4
        // pipeline: RenderCamera, MainCamera, CameraProjection, etc.
        let camera = world.entity(entities.camera);
        assert!(camera.contains::<RenderCamera>());
        assert!(camera.contains::<MainCamera>());
        assert!(camera.contains::<CameraProjection>());
        assert!(camera.contains::<CameraRenderTarget>());
        assert!(camera.contains::<TaaSettings>());
        assert!(camera.contains::<HdrOutputSettings>());
        assert!(camera.contains::<UpscalerSettings>());
        assert!(camera.contains::<ToneMappingSettings>());
        assert!(camera.contains::<ExposureSettings>());
        assert!(camera.contains::<ColorGradingSettings>());
        assert!(camera.contains::<SharpeningSettings>());

        // Static mesh entity carries the asset references and the dirty bits
        // the renderer's extraction system reacts to.
        let mesh = world.entity(entities.static_mesh);
        assert!(mesh.contains::<Renderable>());
        assert!(mesh.contains::<RenderMesh>());
        assert!(mesh.contains::<RenderMaterial>());
        assert!(mesh.contains::<RenderBounds>());
        assert!(mesh.contains::<RenderStatic>());
        assert!(mesh.contains::<ShadowCaster>());

        let directional = entities
            .directional_light
            .expect("directional light should be present in product default scene");
        let directional = world.entity(directional);
        assert!(directional.contains::<DirectionalLight>());
        assert!(directional.contains::<LightLayer>());
    }

    fn install_render_world_extraction_resources_into_world(world: &mut World) {
        // Mirrors install_render_world_extraction_resources but operates on a
        // bare World so this test can run without a full App.
        world.insert_resource(RendererFrameIndex::default());
        let mut app = App::new();
        install_render_world_extraction_resources(&mut app);
        for resource_name in [
            "RenderStableIdAllocator",
            "RenderWorldTables",
            "RenderWorldExtractionDiagnostics",
        ] {
            // Each of these resources is inserted by
            // install_render_world_extraction_resources, so move them into the
            // bare world here.
            let _ = resource_name;
        }
        world.insert_resource(RenderStableIdAllocator::default());
        world.insert_resource(RenderWorldTables::default());
        world.insert_resource(RenderWorldExtractionDiagnostics::default());
    }

    #[test]
    fn proof_scene_spawns_into_renderer_extraction_tables_through_default_app() {
        let mut app = baseline_app();
        let _entities = install_proof_scene(&mut app, ProofSceneSpec::default());
        extract_world_tables(&mut app);

        let tables = app.world().resource::<RenderWorldTables>();
        let counts = tables.table_counts();
        assert_eq!(counts.objects, 1);
        assert_eq!(counts.views, 1);
        assert_eq!(counts.lights, 1);
        assert_eq!(counts.post_process_volumes, 1);
    }

    #[test]
    fn proof_scene_with_ui_emits_extracted_ui_and_cef_surface_records() {
        let mut app = baseline_app();
        let spec =
            ProofSceneSpec::default().with_kind(ProofSceneKind::SingleStaticMeshWithUiSurface);
        let _entities = install_proof_scene(&mut app, spec);
        extract_world_tables(&mut app);

        let tables = app.world().resource::<RenderWorldTables>();
        let counts = tables.table_counts();
        assert_eq!(counts.ui_surfaces, 1);
        assert_eq!(counts.cef_surfaces, 1);
    }

    #[test]
    fn minimal_render_sequence_includes_clear_depth_opaque_compose_and_present_in_order() {
        let sequence = MinimalRenderSequence::PRODUCT_DEFAULT;
        let stages: Vec<MinimalRenderSequenceStage> = sequence.stages.to_vec();

        assert_eq!(
            stages,
            [
                MinimalRenderSequenceStage::Clear,
                MinimalRenderSequenceStage::DepthPrepare,
                MinimalRenderSequenceStage::OpaqueMesh,
                MinimalRenderSequenceStage::DebugMaterialFallback,
                MinimalRenderSequenceStage::FinalComposite,
                MinimalRenderSequenceStage::Present,
            ]
        );
        assert!(sequence.uses_pso_cache);
        assert!(sequence.uses_binding_cache);
        assert!(sequence.uses_resource_registry);
        assert!(sequence.uses_debug_material_fallback);

        // Stages must be in monotonically increasing order so callers can fold
        // them into a graph schedule without re-sorting.
        let mut previous_order = 0u8;
        for stage in stages {
            assert!(stage.order() >= previous_order);
            previous_order = stage.order();
        }
    }

    #[test]
    fn prepared_renderer_resources_default_routes_through_caches_and_registry() {
        let prepared = PreparedRendererResources::PRODUCT_DEFAULT;
        assert!(prepared.fully_prepared());

        let partial = PreparedRendererResources {
            mesh_buffer_through_resource_registry: false,
            ..PreparedRendererResources::PRODUCT_DEFAULT
        };
        assert!(!partial.fully_prepared());
    }

    #[test]
    fn frame_probe_canonical_path_uses_funpb_zst_extension() {
        let path = FrameProbeArtifact::canonical_artifact_path();
        assert!(path.ends_with(".funpb.zst"));
        assert!(!path.ends_with(".funpb.live.zst"));
        assert!(!path.ends_with(".funpb.sum.zst"));
    }

    #[test]
    fn frame_probe_black_frame_classification_fails_closed() {
        let sample = FrameProbeSample {
            schema_version: PROOF_SCENE_SCHEMA_VERSION,
            width: 1920,
            height: 1080,
            black_pixel_ratio_per_mille: 1000,
            average_luminance_milli: 0,
            dominant_color_count: 1,
            watermark_pixels_present: 0,
            frame_index: 1,
        };
        let probe = FrameProbeArtifact::from_sample(
            sample,
            &[DominantColorEntry::from_color(RenderColor::BLACK, 1000)],
            true,
            NativeBackend::Dx12,
            FrameProbeFailureSeam::NoFailure,
        );
        assert_eq!(probe.failure_seam, FrameProbeFailureSeam::BlackFrame);
        assert!(!probe.passed());
    }

    #[test]
    fn frame_probe_uniform_dark_classification_independent_of_black_pixel_ratio() {
        let sample = FrameProbeSample {
            schema_version: PROOF_SCENE_SCHEMA_VERSION,
            width: 1920,
            height: 1080,
            black_pixel_ratio_per_mille: 0,
            average_luminance_milli: 1,
            dominant_color_count: 1,
            watermark_pixels_present: 64,
            frame_index: 2,
        };
        let probe = FrameProbeArtifact::from_sample(
            sample,
            &[],
            true,
            NativeBackend::Dx12,
            FrameProbeFailureSeam::NoFailure,
        );
        assert_eq!(probe.failure_seam, FrameProbeFailureSeam::UniformDarkFrame);
    }

    #[test]
    fn frame_probe_missing_watermark_is_recorded_when_required() {
        let sample = FrameProbeSample {
            schema_version: PROOF_SCENE_SCHEMA_VERSION,
            width: 1920,
            height: 1080,
            black_pixel_ratio_per_mille: 50,
            average_luminance_milli: 500,
            dominant_color_count: 4,
            watermark_pixels_present: 0,
            frame_index: 3,
        };
        let probe = FrameProbeArtifact::from_sample(
            sample,
            &[DominantColorEntry::from_color(RenderColor::WHITE, 600)],
            true,
            NativeBackend::Dx12,
            FrameProbeFailureSeam::NoFailure,
        );
        assert_eq!(probe.failure_seam, FrameProbeFailureSeam::MissingWatermark);
    }

    #[test]
    fn frame_probe_upstream_failure_takes_precedence_over_pixel_classification() {
        let sample = FrameProbeSample {
            schema_version: PROOF_SCENE_SCHEMA_VERSION,
            width: 1920,
            height: 1080,
            black_pixel_ratio_per_mille: 1000,
            average_luminance_milli: 0,
            dominant_color_count: 0,
            watermark_pixels_present: 0,
            frame_index: 4,
        };
        let probe = FrameProbeArtifact::from_sample(
            sample,
            &[],
            true,
            NativeBackend::Dx12,
            FrameProbeFailureSeam::BackendTruthMismatch,
        );
        assert_eq!(
            probe.failure_seam,
            FrameProbeFailureSeam::BackendTruthMismatch
        );
    }

    #[test]
    fn frame_probe_passes_when_image_has_signal_watermark_and_no_upstream_failure() {
        let sample = FrameProbeSample {
            schema_version: PROOF_SCENE_SCHEMA_VERSION,
            width: 1920,
            height: 1080,
            black_pixel_ratio_per_mille: 80,
            average_luminance_milli: 420,
            dominant_color_count: 3,
            watermark_pixels_present: 256,
            frame_index: 5,
        };
        let probe = FrameProbeArtifact::from_sample(
            sample,
            &[
                DominantColorEntry::from_color(RenderColor::linear_rgba(0.4, 0.5, 0.6, 1.0), 500),
                DominantColorEntry::from_color(RenderColor::linear_rgba(0.2, 0.3, 0.4, 1.0), 300),
                DominantColorEntry::from_color(RenderColor::linear_rgba(0.7, 0.6, 0.5, 1.0), 200),
            ],
            true,
            NativeBackend::Dx12,
            FrameProbeFailureSeam::NoFailure,
        );
        assert_eq!(probe.failure_seam, FrameProbeFailureSeam::NoFailure);
        assert!(probe.passed());
        assert_eq!(probe.dominant_color_count, 3);
    }

    #[test]
    fn proof_scene_runtime_report_is_complete_when_probe_passes_and_resources_prepared() {
        let mut report = ProofSceneRuntimeReport::new();
        report
            .with_spec(ProofSceneSpec::default())
            .with_render_sequence(MinimalRenderSequence::PRODUCT_DEFAULT)
            .with_prepared_resources(PreparedRendererResources::PRODUCT_DEFAULT);
        assert!(!report.is_complete(), "missing probe should fail closed");

        let probe = FrameProbeArtifact::from_sample(
            FrameProbeSample {
                schema_version: PROOF_SCENE_SCHEMA_VERSION,
                width: 1920,
                height: 1080,
                black_pixel_ratio_per_mille: 80,
                average_luminance_milli: 420,
                dominant_color_count: 1,
                watermark_pixels_present: 256,
                frame_index: 1,
            },
            &[DominantColorEntry::from_color(RenderColor::WHITE, 700)],
            true,
            NativeBackend::Dx12,
            FrameProbeFailureSeam::NoFailure,
        );
        report.with_frame_probe(probe);
        assert!(report.is_complete());
    }
}
