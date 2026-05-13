use crate::frame_graph::FrameGraphResourceType;

pub const NATIVE_UI_COMPOSITOR_SCHEMA_VERSION: u16 = 1;
pub const NATIVE_UI_COMPOSITOR_PRODUCT_TRANSPORT: RendererNativeUiTransportMode =
    RendererNativeUiTransportMode::D3d11On12SharedTexture;
pub const NATIVE_UI_COMPOSITOR_FRAME_GRAPH_RESOURCE: FrameGraphResourceType =
    FrameGraphResourceType::UiColorAlpha;
pub const NATIVE_UI_COMPOSITOR_IMPORT_ALLOCATION_SITE: &str =
    "fun_renderer.native_ui_compositor.imported_ui";
pub const NATIVE_UI_COMPOSITOR_TRANSIENT_ALLOCATION_SITE: &str =
    "fun_renderer.native_ui_compositor.ui_color_alpha";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RendererNativeUiFrameId(pub u64);

impl RendererNativeUiFrameId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RendererNativeUiTextureId {
    pub index: u32,
    pub generation: u32,
}

impl RendererNativeUiTextureId {
    pub const INVALID: Self = Self {
        index: u32::MAX,
        generation: 0,
    };

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.index != u32::MAX && self.generation != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererNativeUiExtent {
    pub width: u32,
    pub height: u32,
}

impl RendererNativeUiExtent {
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.width > 0 && self.height > 0
    }

    #[must_use]
    pub const fn pixel_count(self) -> u64 {
        (self.width as u64).saturating_mul(self.height as u64)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererNativeUiDirtyRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl RendererNativeUiDirtyRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererNativeUiAlphaMode {
    #[default]
    Premultiplied,
    Straight,
    Unknown,
}

impl RendererNativeUiAlphaMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Premultiplied => "premultiplied",
            Self::Straight => "straight",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererNativeUiTransportMode {
    #[default]
    Disabled,
    CpuOnPaint,
    D3d11On12SharedTexture,
    VulkanExternalMemory,
}

impl RendererNativeUiTransportMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::CpuOnPaint => "cpu_on_paint",
            Self::D3d11On12SharedTexture => "d3d11on12_shared_texture",
            Self::VulkanExternalMemory => "vulkan_external_memory",
        }
    }

    #[must_use]
    pub const fn is_gpu_transport(self) -> bool {
        matches!(
            self,
            Self::D3d11On12SharedTexture | Self::VulkanExternalMemory
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererNativeUiImportSyncStatus {
    #[default]
    NotImported,
    CopiedIntoRendererTexture,
    ReusedPreviousFrame,
    DroppedStaleFrame,
    FailedClosed,
}

impl RendererNativeUiImportSyncStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotImported => "not_imported",
            Self::CopiedIntoRendererTexture => "copied_into_renderer_texture",
            Self::ReusedPreviousFrame => "reused_previous_frame",
            Self::DroppedStaleFrame => "dropped_stale_frame",
            Self::FailedClosed => "failed_closed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererNativeUiFailClosedReason {
    CpuOnPaintRuntimeFallback,
    InvalidSharedTextureHandle,
    InvalidFrameExtent,
    SharedTextureUnavailable,
    ImportFailed,
    SurfaceNotGpuOnly,
}

impl RendererNativeUiFailClosedReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CpuOnPaintRuntimeFallback => "cpu_on_paint_runtime_fallback",
            Self::InvalidSharedTextureHandle => "invalid_shared_texture_handle",
            Self::InvalidFrameExtent => "invalid_frame_extent",
            Self::SharedTextureUnavailable => "shared_texture_unavailable",
            Self::ImportFailed => "import_failed",
            Self::SurfaceNotGpuOnly => "surface_not_gpu_only",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererNativeUiCompositorError {
    ProductSurfaceRequiresGpuOnly,
    ProductRequiresGpuTransport,
    InvalidFrameId,
    InvalidExtent,
    CpuFallbackForbidden(RendererNativeUiFailClosedReason),
}

impl RendererNativeUiCompositorError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProductSurfaceRequiresGpuOnly => "product_surface_requires_gpu_only",
            Self::ProductRequiresGpuTransport => "product_requires_gpu_transport",
            Self::InvalidFrameId => "invalid_frame_id",
            Self::InvalidExtent => "invalid_extent",
            Self::CpuFallbackForbidden(reason) => reason.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererNativeUiSurfaceDescriptor {
    pub route: fun_scene::NativeUiRoute,
    pub layer: fun_scene::UiLayer,
    pub composition: fun_scene::UiCompositionPolicy,
    pub gpu_only: bool,
}

impl RendererNativeUiSurfaceDescriptor {
    pub const PRODUCT_DEFAULT: Self = Self {
        route: fun_scene::NativeUiRoute::ROOT,
        layer: fun_scene::UiLayer::Hud,
        composition: fun_scene::UiCompositionPolicy::FullWindow,
        gpu_only: true,
    };

    #[must_use]
    pub const fn from_fun_scene(surface: &fun_scene::NativeUiSurface) -> Self {
        Self {
            route: surface.route,
            layer: surface.layer,
            composition: surface.composition,
            gpu_only: surface.gpu_only,
        }
    }

    pub const fn validate_product(self) -> Result<(), RendererNativeUiCompositorError> {
        if self.gpu_only {
            Ok(())
        } else {
            Err(RendererNativeUiCompositorError::ProductSurfaceRequiresGpuOnly)
        }
    }
}

impl Default for RendererNativeUiSurfaceDescriptor {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererNativeUiImportedFrame {
    pub frame_id: RendererNativeUiFrameId,
    pub extent: RendererNativeUiExtent,
    pub transport: RendererNativeUiTransportMode,
    pub alpha_mode: RendererNativeUiAlphaMode,
    pub dirty_rect_count: u32,
    pub dirty_rect_union: Option<RendererNativeUiDirtyRect>,
    pub callback_timestamp_ns: u64,
    pub import_begin_timestamp_ns: u64,
    pub import_complete_timestamp_ns: u64,
    pub copied_bytes: u64,
}

impl RendererNativeUiImportedFrame {
    #[must_use]
    pub const fn import_copy_duration_ns(self) -> u64 {
        self.import_complete_timestamp_ns
            .saturating_sub(self.import_begin_timestamp_ns)
    }

    #[must_use]
    pub const fn callback_to_import_latency_ns(self) -> u64 {
        self.import_begin_timestamp_ns
            .saturating_sub(self.callback_timestamp_ns)
    }

    pub const fn validate_product(self) -> Result<(), RendererNativeUiCompositorError> {
        if !self.frame_id.is_valid() {
            return Err(RendererNativeUiCompositorError::InvalidFrameId);
        }
        if !self.extent.is_valid() {
            return Err(RendererNativeUiCompositorError::InvalidExtent);
        }
        if !self.transport.is_gpu_transport() {
            return Err(RendererNativeUiCompositorError::ProductRequiresGpuTransport);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererNativeUiOwnedTexture {
    pub texture_id: RendererNativeUiTextureId,
    pub frame_id: RendererNativeUiFrameId,
    pub extent: RendererNativeUiExtent,
    pub alpha_mode: RendererNativeUiAlphaMode,
    pub transport: RendererNativeUiTransportMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererNativeUiLayer {
    pub texture_id: RendererNativeUiTextureId,
    pub frame_id: RendererNativeUiFrameId,
    pub layer: fun_scene::UiLayer,
    pub composition: fun_scene::UiCompositionPolicy,
    pub resource_type: FrameGraphResourceType,
    pub alpha_mode: RendererNativeUiAlphaMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererNativeUiCompositorDiagnostics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub latest_frame_id: RendererNativeUiFrameId,
    pub active_texture: RendererNativeUiTextureId,
    pub imported_frame_count: u64,
    pub dropped_ui_frames: u64,
    pub stale_ui_frame_age_ns: u64,
    pub callback_to_import_latency_ns: u64,
    pub import_copy_duration_ns: u64,
    pub ui_composite_pass_duration_ns: u64,
    pub invalid_handle_count: u64,
    pub cpu_fallback_attempts: u64,
    pub fail_closed_count: u64,
    pub copied_bytes: u64,
    pub dirty_rect_count: u64,
    pub last_sync_status: RendererNativeUiImportSyncStatus,
    pub last_fail_closed_reason: Option<RendererNativeUiFailClosedReason>,
}

impl Default for RendererNativeUiCompositorDiagnostics {
    fn default() -> Self {
        Self {
            schema_version: NATIVE_UI_COMPOSITOR_SCHEMA_VERSION,
            frame_index: 0,
            latest_frame_id: RendererNativeUiFrameId::INVALID,
            active_texture: RendererNativeUiTextureId::INVALID,
            imported_frame_count: 0,
            dropped_ui_frames: 0,
            stale_ui_frame_age_ns: 0,
            callback_to_import_latency_ns: 0,
            import_copy_duration_ns: 0,
            ui_composite_pass_duration_ns: 0,
            invalid_handle_count: 0,
            cpu_fallback_attempts: 0,
            fail_closed_count: 0,
            copied_bytes: 0,
            dirty_rect_count: 0,
            last_sync_status: RendererNativeUiImportSyncStatus::NotImported,
            last_fail_closed_reason: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "fun_ecs", derive(fun_ecs::Resource))]
pub struct RendererNativeUiCompositor {
    next_texture_generation: u32,
    active_texture: Option<RendererNativeUiOwnedTexture>,
    active_layer: Option<RendererNativeUiLayer>,
    diagnostics: RendererNativeUiCompositorDiagnostics,
}

impl Default for RendererNativeUiCompositor {
    fn default() -> Self {
        Self {
            next_texture_generation: 1,
            active_texture: None,
            active_layer: None,
            diagnostics: RendererNativeUiCompositorDiagnostics::default(),
        }
    }
}

impl RendererNativeUiCompositor {
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.diagnostics.frame_index = frame_index;
    }

    #[must_use]
    pub const fn active_texture(&self) -> Option<RendererNativeUiOwnedTexture> {
        self.active_texture
    }

    #[must_use]
    pub const fn active_layer(&self) -> Option<RendererNativeUiLayer> {
        self.active_layer
    }

    #[must_use]
    pub const fn diagnostics(&self) -> RendererNativeUiCompositorDiagnostics {
        self.diagnostics
    }

    pub fn import_accelerated_frame(
        &mut self,
        surface: RendererNativeUiSurfaceDescriptor,
        frame: RendererNativeUiImportedFrame,
    ) -> Result<RendererNativeUiLayer, RendererNativeUiCompositorError> {
        surface.validate_product()?;
        if let Err(error) = frame.validate_product() {
            self.record_import_rejection(error);
            return Err(error);
        }

        if self
            .active_texture
            .is_some_and(|texture| frame.frame_id <= texture.frame_id)
        {
            self.diagnostics.dropped_ui_frames =
                self.diagnostics.dropped_ui_frames.saturating_add(1);
            self.diagnostics.last_sync_status = RendererNativeUiImportSyncStatus::DroppedStaleFrame;
            self.diagnostics.stale_ui_frame_age_ns = self
                .diagnostics
                .frame_index
                .saturating_sub(frame.frame_id.0)
                .saturating_mul(16_666_667);
            return Ok(self.active_layer.expect("active layer exists with texture"));
        }

        let texture = self.renderer_owned_texture_for(frame);
        let layer = RendererNativeUiLayer {
            texture_id: texture.texture_id,
            frame_id: frame.frame_id,
            layer: surface.layer,
            composition: surface.composition,
            resource_type: NATIVE_UI_COMPOSITOR_FRAME_GRAPH_RESOURCE,
            alpha_mode: frame.alpha_mode,
        };

        self.active_texture = Some(texture);
        self.active_layer = Some(layer);
        self.diagnostics.latest_frame_id = frame.frame_id;
        self.diagnostics.active_texture = texture.texture_id;
        self.diagnostics.imported_frame_count =
            self.diagnostics.imported_frame_count.saturating_add(1);
        self.diagnostics.callback_to_import_latency_ns = frame.callback_to_import_latency_ns();
        self.diagnostics.import_copy_duration_ns = frame.import_copy_duration_ns();
        self.diagnostics.copied_bytes = self
            .diagnostics
            .copied_bytes
            .saturating_add(frame.copied_bytes);
        self.diagnostics.dirty_rect_count = self
            .diagnostics
            .dirty_rect_count
            .saturating_add(u64::from(frame.dirty_rect_count));
        self.diagnostics.last_sync_status =
            RendererNativeUiImportSyncStatus::CopiedIntoRendererTexture;
        self.diagnostics.last_fail_closed_reason = None;
        Ok(layer)
    }

    pub fn record_cpu_fallback_attempt(
        &mut self,
        reason: RendererNativeUiFailClosedReason,
    ) -> RendererNativeUiCompositorError {
        self.diagnostics.cpu_fallback_attempts =
            self.diagnostics.cpu_fallback_attempts.saturating_add(1);
        self.record_fail_closed(reason);
        RendererNativeUiCompositorError::CpuFallbackForbidden(reason)
    }

    pub fn record_fail_closed(&mut self, reason: RendererNativeUiFailClosedReason) {
        self.diagnostics.fail_closed_count = self.diagnostics.fail_closed_count.saturating_add(1);
        self.diagnostics.last_sync_status = RendererNativeUiImportSyncStatus::FailedClosed;
        self.diagnostics.last_fail_closed_reason = Some(reason);
    }

    pub fn record_ui_composite_duration(&mut self, duration_ns: u64) {
        self.diagnostics.ui_composite_pass_duration_ns = duration_ns;
    }

    fn renderer_owned_texture_for(
        &mut self,
        frame: RendererNativeUiImportedFrame,
    ) -> RendererNativeUiOwnedTexture {
        let reuse = self
            .active_texture
            .filter(|texture| texture.extent == frame.extent)
            .map(|texture| texture.texture_id);
        let texture_id = match reuse {
            Some(texture_id) => texture_id,
            None => {
                let generation = self.next_texture_generation;
                self.next_texture_generation = self.next_texture_generation.saturating_add(1);
                RendererNativeUiTextureId {
                    index: 0,
                    generation,
                }
            }
        };
        RendererNativeUiOwnedTexture {
            texture_id,
            frame_id: frame.frame_id,
            extent: frame.extent,
            alpha_mode: frame.alpha_mode,
            transport: frame.transport,
        }
    }

    fn record_import_rejection(&mut self, error: RendererNativeUiCompositorError) {
        match error {
            RendererNativeUiCompositorError::InvalidFrameId
            | RendererNativeUiCompositorError::InvalidExtent => {
                self.diagnostics.invalid_handle_count =
                    self.diagnostics.invalid_handle_count.saturating_add(1);
                self.record_fail_closed(match error {
                    RendererNativeUiCompositorError::InvalidExtent => {
                        RendererNativeUiFailClosedReason::InvalidFrameExtent
                    }
                    _ => RendererNativeUiFailClosedReason::InvalidSharedTextureHandle,
                });
            }
            RendererNativeUiCompositorError::ProductRequiresGpuTransport => {
                let _ = self.record_cpu_fallback_attempt(
                    RendererNativeUiFailClosedReason::CpuOnPaintRuntimeFallback,
                );
            }
            RendererNativeUiCompositorError::ProductSurfaceRequiresGpuOnly
            | RendererNativeUiCompositorError::CpuFallbackForbidden(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu_frame(id: u64) -> RendererNativeUiImportedFrame {
        RendererNativeUiImportedFrame {
            frame_id: RendererNativeUiFrameId(id),
            extent: RendererNativeUiExtent::new(1280, 720),
            transport: RendererNativeUiTransportMode::D3d11On12SharedTexture,
            alpha_mode: RendererNativeUiAlphaMode::Premultiplied,
            dirty_rect_count: 2,
            dirty_rect_union: Some(RendererNativeUiDirtyRect::new(0, 0, 1280, 720)),
            callback_timestamp_ns: 10,
            import_begin_timestamp_ns: 30,
            import_complete_timestamp_ns: 80,
            copied_bytes: 1280 * 720 * 4,
        }
    }

    #[test]
    fn native_ui_compositor_imports_gpu_frames_into_renderer_owned_texture() {
        let mut compositor = RendererNativeUiCompositor::default();
        compositor.begin_frame(12);

        let layer = compositor
            .import_accelerated_frame(RendererNativeUiSurfaceDescriptor::default(), gpu_frame(1))
            .expect("gpu frame should import");

        assert!(layer.texture_id.is_valid());
        assert_eq!(layer.frame_id, RendererNativeUiFrameId(1));
        assert_eq!(layer.resource_type, FrameGraphResourceType::UiColorAlpha);

        let diagnostics = compositor.diagnostics();
        assert_eq!(diagnostics.imported_frame_count, 1);
        assert_eq!(diagnostics.callback_to_import_latency_ns, 20);
        assert_eq!(diagnostics.import_copy_duration_ns, 50);
        assert_eq!(
            diagnostics.last_sync_status,
            RendererNativeUiImportSyncStatus::CopiedIntoRendererTexture
        );
        assert_eq!(diagnostics.fail_closed_count, 0);
    }

    #[test]
    fn native_ui_compositor_rejects_cpu_on_paint_as_product_transport() {
        let mut compositor = RendererNativeUiCompositor::default();
        let mut frame = gpu_frame(2);
        frame.transport = RendererNativeUiTransportMode::CpuOnPaint;

        assert_eq!(
            compositor
                .import_accelerated_frame(RendererNativeUiSurfaceDescriptor::default(), frame),
            Err(RendererNativeUiCompositorError::ProductRequiresGpuTransport)
        );

        let diagnostics = compositor.diagnostics();
        assert_eq!(diagnostics.cpu_fallback_attempts, 1);
        assert_eq!(diagnostics.fail_closed_count, 1);
        assert_eq!(
            diagnostics.last_fail_closed_reason,
            Some(RendererNativeUiFailClosedReason::CpuOnPaintRuntimeFallback)
        );
    }

    #[test]
    fn native_ui_compositor_rejects_non_gpu_scene_surface() {
        let mut compositor = RendererNativeUiCompositor::default();
        let surface = RendererNativeUiSurfaceDescriptor {
            gpu_only: false,
            ..RendererNativeUiSurfaceDescriptor::default()
        };

        assert_eq!(
            compositor.import_accelerated_frame(surface, gpu_frame(3)),
            Err(RendererNativeUiCompositorError::ProductSurfaceRequiresGpuOnly)
        );
    }

    #[test]
    fn native_ui_compositor_drops_stale_frames_without_reusing_callback_handles() {
        let mut compositor = RendererNativeUiCompositor::default();
        let first = compositor
            .import_accelerated_frame(RendererNativeUiSurfaceDescriptor::default(), gpu_frame(9))
            .expect("first import");
        let stale = compositor
            .import_accelerated_frame(RendererNativeUiSurfaceDescriptor::default(), gpu_frame(8))
            .expect("stale frame reuses active layer");

        assert_eq!(first, stale);
        assert_eq!(compositor.diagnostics().dropped_ui_frames, 1);
        assert_eq!(
            compositor.diagnostics().last_sync_status,
            RendererNativeUiImportSyncStatus::DroppedStaleFrame
        );
    }

    #[test]
    fn native_ui_compositor_records_ui_composite_benchmark_payload() {
        let mut compositor = RendererNativeUiCompositor::default();
        compositor
            .import_accelerated_frame(RendererNativeUiSurfaceDescriptor::default(), gpu_frame(4))
            .expect("gpu frame");
        compositor.record_ui_composite_duration(440_000);

        let diagnostics = compositor.diagnostics();
        assert_eq!(diagnostics.ui_composite_pass_duration_ns, 440_000);

        if let Some(path) = std::env::var_os("FUN_RENDERER_NATIVE_UI_COMPOSITOR_BENCHMARK_ARTIFACT")
        {
            let path = std::path::PathBuf::from(path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("artifact parent");
            }
            let payload = format!(
                concat!(
                    "{{\n",
                    "  \"schema\": \"fun.renderer.native_ui_compositor.benchmark.v1\",\n",
                    "  \"ui_composite_pass_duration_ns\": {},\n",
                    "  \"callback_to_import_latency_ns\": {},\n",
                    "  \"import_copy_duration_ns\": {},\n",
                    "  \"cpu_fallback_attempts\": {},\n",
                    "  \"fail_closed_count\": {},\n",
                    "  \"last_sync_status\": \"{}\"\n",
                    "}}\n"
                ),
                diagnostics.ui_composite_pass_duration_ns,
                diagnostics.callback_to_import_latency_ns,
                diagnostics.import_copy_duration_ns,
                diagnostics.cpu_fallback_attempts,
                diagnostics.fail_closed_count,
                diagnostics.last_sync_status.as_str(),
            );
            std::fs::write(path, payload).expect("artifact write");
        }
    }
}
