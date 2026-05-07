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
    CefGpuImport,
    UiImportPlaceholder,
    UpscaleBoundary,
    FrameGenerationBoundary,
    Compose,
    DiagnosticsReadback,
    Present,
}

impl FrameGraphPassRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::StaticScenePlaceholder => "static_scene_placeholder",
            Self::VirtualResourceFeedback => "virtual_resource_feedback",
            Self::CefGpuImport => "cef_gpu_import",
            Self::UiImportPlaceholder => "ui_import_placeholder",
            Self::UpscaleBoundary => "upscale_boundary",
            Self::FrameGenerationBoundary => "frame_generation_boundary",
            Self::Compose => "compose",
            Self::DiagnosticsReadback => "diagnostics_readback",
            Self::Present => "present",
        }
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
}

impl FrameGraphResourceType {
    pub const ALL: [Self; 18] = [
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
        }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererFrameDescription {
    pub frame_index: u64,
    pub include_static_scene_placeholder: bool,
    pub include_ui_placeholder: bool,
    pub include_virtual_resource_slot: bool,
    pub include_upscaling_slot: bool,
    pub include_frame_generation_slot: bool,
    pub include_diagnostics_readback: bool,
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
#[cfg_attr(feature = "bevy_ecs", derive(bevy_ecs::prelude::Resource))]
pub struct RendererFrameGraph {
    frame_index: u64,
    passes: Vec<FrameGraphPass>,
    resources: Vec<FrameGraphResource>,
}

impl RendererFrameGraph {
    #[must_use]
    pub fn from_frame_description(description: RendererFrameDescription) -> Self {
        let mut graph = Self {
            frame_index: description.frame_index,
            passes: Vec::new(),
            resources: Vec::new(),
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
                "fun_renderer.pass.cef_gpu_import",
                FrameGraphPassType::CopyImport,
                FrameGraphPassRole::CefGpuImport,
                "cef_gpu_import",
                FrameGraphDiagnosticCategory::Ui,
                Some(FrameGraphBenchmarkCategory::UiImport),
                "fun_renderer::frame_graph::cef_gpu_import",
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
        failures
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
        for resource_type in FrameGraphResourceType::ALL {
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
                FrameGraphPassRole::CefGpuImport
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
        assert_eq!(diagnostics.pass_count, 5);
        assert_eq!(
            diagnostics.resource_count,
            FrameGraphResourceType::ALL.len() as u16
        );

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
                FrameGraphPassRole::CefGpuImport,
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
