use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use crate::render_handler::{
    CefDirtyRect, CefOwnedPaintFrame, CefPaintElement, CefPaintFrame, CefPaintSink,
    CefPaintValidationError, CefUiFrameGeneration, CefUiFrameMetadata, CefUiScaleFactor,
    validate_paint_frame,
};

pub type UiSurfaceGeneration = CefUiFrameGeneration;

pub const DIRTY_RECT_COALESCE_THRESHOLD: usize = 32;
pub const DIRTY_RECT_EXPLOSION_THRESHOLD: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiFullUploadReason {
    FirstFrame,
    Resize,
    ScaleFactorChanged,
    DirtyRectExplosion,
    EmptyDirtyRects,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum CefUiUploadPlan {
    None,
    DirtyRects {
        generation: CefUiFrameGeneration,
        rects: Vec<CefDirtyRect>,
    },
    FullFrame {
        generation: CefUiFrameGeneration,
        reason: CefUiFullUploadReason,
    },
}

impl CefUiUploadPlan {
    #[must_use]
    pub const fn generation(&self) -> Option<CefUiFrameGeneration> {
        match self {
            Self::None => None,
            Self::DirtyRects { generation, .. } | Self::FullFrame { generation, .. } => {
                Some(*generation)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CefUiCompositorFrame {
    pub element: CefPaintElement,
    pub metadata: CefUiFrameMetadata,
    pub upload_plan: CefUiUploadPlan,
    pixels: Vec<u8>,
}

impl CefUiCompositorFrame {
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CefUiBufferSlot {
    frame: Option<CefUiCompositorFrame>,
}

impl CefUiBufferSlot {
    fn replace(&mut self, frame: CefUiCompositorFrame) {
        self.frame = Some(frame);
    }

    fn clear(&mut self) {
        self.frame = None;
    }

    fn generation(&self) -> Option<CefUiFrameGeneration> {
        self.frame.as_ref().map(|frame| frame.metadata.generation)
    }

    fn frame(&self) -> Option<&CefUiCompositorFrame> {
        self.frame.as_ref()
    }
}

#[derive(Debug)]
pub struct CefUiCompositor {
    published_generation: AtomicU64,
    write_slot: CefUiBufferSlot,
    ready_slot: CefUiBufferSlot,
    read_slot: CefUiBufferSlot,
    last_surface: Option<CefUiSurfaceKey>,
    accepting_paint: bool,
}

impl CefUiCompositor {
    #[must_use]
    pub fn new() -> Self {
        Self {
            published_generation: AtomicU64::new(0),
            write_slot: CefUiBufferSlot::default(),
            ready_slot: CefUiBufferSlot::default(),
            read_slot: CefUiBufferSlot::default(),
            last_surface: None,
            accepting_paint: true,
        }
    }

    pub fn stop_accepting_paint(&mut self) {
        self.accepting_paint = false;
    }

    pub fn release_buffers(&mut self) {
        self.write_slot.clear();
        self.ready_slot.clear();
        self.read_slot.clear();
        self.last_surface = None;
    }

    pub fn ingest_paint(
        &mut self,
        frame: CefPaintFrame<'_>,
    ) -> Result<CefUiFrameGeneration, UiCompositorError> {
        if !self.accepting_paint {
            return Err(UiCompositorError::ShuttingDown);
        }
        validate_paint_frame(frame).map_err(UiCompositorError::InvalidPaint)?;

        let generation = CefUiFrameGeneration(
            self.published_generation
                .load(Ordering::Acquire)
                .saturating_add(1),
        );
        let surface = CefUiSurfaceKey::from_frame(frame);
        let upload_plan = self.upload_plan_for(frame, generation, surface);
        let dirty_rects = dirty_rects_for_metadata(frame, &upload_plan);
        let metadata = CefUiFrameMetadata::new(
            frame.width,
            frame.height,
            frame.scale_factor,
            generation,
            dirty_rects,
            frame.alpha_mode,
            frame.timestamp_ns,
        );

        self.write_slot.replace(CefUiCompositorFrame {
            element: frame.element,
            metadata,
            upload_plan,
            pixels: frame.bytes.to_vec(),
        });
        std::mem::swap(&mut self.write_slot, &mut self.ready_slot);
        self.last_surface = Some(surface);
        self.published_generation
            .store(generation.0, Ordering::Release);

        Ok(generation)
    }

    #[must_use]
    pub fn consume_ready(&mut self) -> Option<&CefUiCompositorFrame> {
        let ready_generation = self.ready_slot.generation()?;
        let read_generation = self
            .read_slot
            .generation()
            .unwrap_or(CefUiFrameGeneration(0));
        if ready_generation <= read_generation {
            return self.read_slot.frame();
        }
        std::mem::swap(&mut self.ready_slot, &mut self.read_slot);
        self.read_slot.frame()
    }

    #[must_use]
    pub fn current_frame(&self) -> Option<&CefUiCompositorFrame> {
        self.read_slot.frame()
    }

    #[must_use]
    pub fn current_upload_plan(&self) -> CefUiUploadPlan {
        self.read_slot
            .frame()
            .map_or(CefUiUploadPlan::None, |frame| frame.upload_plan.clone())
    }

    #[must_use]
    pub fn latest_generation(&self) -> CefUiFrameGeneration {
        CefUiFrameGeneration(self.published_generation.load(Ordering::Acquire))
    }

    #[must_use]
    pub fn dirty_rects(&self) -> &[CefDirtyRect] {
        self.read_slot
            .frame()
            .map_or(&[], |frame| frame.metadata.dirty_rects.as_slice())
    }

    fn upload_plan_for(
        &self,
        frame: CefPaintFrame<'_>,
        generation: CefUiFrameGeneration,
        surface: CefUiSurfaceKey,
    ) -> CefUiUploadPlan {
        let Some(previous_surface) = self.last_surface else {
            return CefUiUploadPlan::FullFrame {
                generation,
                reason: CefUiFullUploadReason::FirstFrame,
            };
        };
        if previous_surface.width != surface.width || previous_surface.height != surface.height {
            return CefUiUploadPlan::FullFrame {
                generation,
                reason: CefUiFullUploadReason::Resize,
            };
        }
        if previous_surface.scale_factor != surface.scale_factor {
            return CefUiUploadPlan::FullFrame {
                generation,
                reason: CefUiFullUploadReason::ScaleFactorChanged,
            };
        }
        if frame.dirty_rects.is_empty() {
            return CefUiUploadPlan::FullFrame {
                generation,
                reason: CefUiFullUploadReason::EmptyDirtyRects,
            };
        }
        if frame.dirty_rects.len() > DIRTY_RECT_EXPLOSION_THRESHOLD {
            return CefUiUploadPlan::FullFrame {
                generation,
                reason: CefUiFullUploadReason::DirtyRectExplosion,
            };
        }

        let rects = coalesce_dirty_rects(frame.dirty_rects);
        CefUiUploadPlan::DirtyRects { generation, rects }
    }
}

impl Default for CefUiCompositor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SharedCefUiCompositor {
    inner: Arc<Mutex<CefUiCompositor>>,
}

impl SharedCefUiCompositor {
    #[must_use]
    pub fn new(compositor: CefUiCompositor) -> Self {
        Self {
            inner: Arc::new(Mutex::new(compositor)),
        }
    }

    #[must_use]
    pub fn inner(&self) -> Arc<Mutex<CefUiCompositor>> {
        Arc::clone(&self.inner)
    }

    pub fn with_compositor<R>(&self, f: impl FnOnce(&mut CefUiCompositor) -> R) -> Option<R> {
        self.inner
            .lock()
            .ok()
            .map(|mut compositor| f(&mut compositor))
    }
}

impl Default for SharedCefUiCompositor {
    fn default() -> Self {
        Self::new(CefUiCompositor::new())
    }
}

impl CefPaintSink for SharedCefUiCompositor {
    fn ingest_cef_paint(&self, frame: CefOwnedPaintFrame) {
        if let Some(result) =
            self.with_compositor(|compositor| compositor.ingest_paint(frame.borrowed()))
            && let Err(error) = result
        {
            tracing::debug!(?error, "dropping CEF paint frame");
        }
    }
}

pub type UiCompositorState = CefUiCompositor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CefUiSurfaceKey {
    width: i32,
    height: i32,
    scale_factor: CefUiScaleFactor,
}

impl CefUiSurfaceKey {
    fn from_frame(frame: CefPaintFrame<'_>) -> Self {
        Self {
            width: frame.width,
            height: frame.height,
            scale_factor: frame.scale_factor,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCompositorError {
    ShuttingDown,
    InvalidPaint(CefPaintValidationError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct NativeWindowHandle(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct CefUiPhysicalRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl CefUiPhysicalRect {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiOverlayMode {
    PassiveHudClickThrough,
    Interactive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct CefUiOverlaySurfaceConfig {
    pub game_window: NativeWindowHandle,
    pub content_rect: CefUiPhysicalRect,
    pub dpi_scale: CefUiScaleFactor,
    pub mode: CefUiOverlayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct CefUiOverlayBehavior {
    pub transparent: bool,
    pub borderless: bool,
    pub taskbar_entry: bool,
    pub topmost_while_game_focused: bool,
    pub click_through: bool,
}

impl CefUiOverlayBehavior {
    #[must_use]
    pub const fn for_mode(mode: CefUiOverlayMode) -> Self {
        Self {
            transparent: true,
            borderless: true,
            taskbar_entry: false,
            topmost_while_game_focused: true,
            click_through: matches!(mode, CefUiOverlayMode::PassiveHudClickThrough),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CefUiOverlaySurface {
    config: CefUiOverlaySurfaceConfig,
    visible: bool,
}

impl CefUiOverlaySurface {
    #[must_use]
    pub const fn new(config: CefUiOverlaySurfaceConfig) -> Self {
        Self {
            config,
            visible: false,
        }
    }

    #[must_use]
    pub const fn config(&self) -> CefUiOverlaySurfaceConfig {
        self.config
    }

    #[must_use]
    pub const fn behavior(&self) -> CefUiOverlayBehavior {
        CefUiOverlayBehavior::for_mode(self.config.mode)
    }

    pub fn update_tracking(
        &mut self,
        content_rect: CefUiPhysicalRect,
        dpi_scale: CefUiScaleFactor,
    ) {
        self.config.content_rect = content_rect;
        self.config.dpi_scale = dpi_scale;
    }

    pub fn set_mode(&mut self, mode: CefUiOverlayMode) {
        self.config.mode = mode;
    }

    pub fn set_visible_with_game(&mut self, visible: bool) {
        self.visible = visible;
    }

    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }
}

#[cfg(windows)]
pub mod windows_overlay {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT,
        WS_POPUP, WS_VISIBLE,
    };

    use super::{CefUiOverlayBehavior, CefUiOverlayMode};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct WindowsOverlayStyleBits {
        pub style: u32,
        pub ex_style: u32,
    }

    #[must_use]
    pub const fn style_bits_for_mode(mode: CefUiOverlayMode) -> WindowsOverlayStyleBits {
        let behavior = CefUiOverlayBehavior::for_mode(mode);
        let mut ex_style = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST;
        if behavior.click_through {
            ex_style |= WS_EX_TRANSPARENT;
        }
        WindowsOverlayStyleBits {
            style: WS_POPUP | WS_VISIBLE,
            ex_style,
        }
    }
}

fn coalesce_dirty_rects(dirty_rects: &[CefDirtyRect]) -> Vec<CefDirtyRect> {
    if dirty_rects.len() <= DIRTY_RECT_COALESCE_THRESHOLD {
        return dirty_rects.to_vec();
    }
    dirty_rects
        .iter()
        .copied()
        .reduce(CefDirtyRect::union)
        .into_iter()
        .collect()
}

fn dirty_rects_for_metadata(
    frame: CefPaintFrame<'_>,
    upload_plan: &CefUiUploadPlan,
) -> Vec<CefDirtyRect> {
    match upload_plan {
        CefUiUploadPlan::None => Vec::new(),
        CefUiUploadPlan::DirtyRects { rects, .. } => rects.clone(),
        CefUiUploadPlan::FullFrame { .. } => {
            vec![CefDirtyRect::full_frame(frame.width, frame.height)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paint_frame<'a>(
        width: i32,
        height: i32,
        rects: &'a [CefDirtyRect],
        bytes: &'a [u8],
    ) -> CefPaintFrame<'a> {
        CefPaintFrame::new(CefPaintElement::View, width, height, rects, bytes)
    }

    #[test]
    fn compositor_tracks_generation_and_dirty_rects() {
        let rects = [CefDirtyRect::new(0, 0, 2, 2)];
        let bytes = [255_u8; 16];
        let mut compositor = CefUiCompositor::new();
        let generation = compositor
            .ingest_paint(paint_frame(2, 2, &rects, &bytes))
            .expect("valid paint frame");
        let frame = compositor.consume_ready().expect("ready frame");

        assert_eq!(generation, CefUiFrameGeneration(1));
        assert_eq!(
            frame.metadata.dirty_rects,
            vec![CefDirtyRect::full_frame(2, 2)]
        );
        assert_eq!(
            frame.upload_plan,
            CefUiUploadPlan::FullFrame {
                generation,
                reason: CefUiFullUploadReason::FirstFrame
            }
        );
    }

    #[test]
    fn compositor_uses_dirty_rect_upload_after_first_frame() {
        let rects = [CefDirtyRect::new(0, 0, 1, 1)];
        let bytes = [255_u8; 16];
        let mut compositor = CefUiCompositor::new();
        compositor
            .ingest_paint(paint_frame(2, 2, &rects, &bytes))
            .expect("first frame");
        let _ = compositor.consume_ready();
        let generation = compositor
            .ingest_paint(paint_frame(2, 2, &rects, &bytes))
            .expect("second frame");
        let frame = compositor.consume_ready().expect("second ready frame");

        assert_eq!(
            frame.upload_plan,
            CefUiUploadPlan::DirtyRects {
                generation,
                rects: rects.to_vec()
            }
        );
    }

    #[test]
    fn compositor_coalesces_many_dirty_rects() {
        let rects = (0..=DIRTY_RECT_COALESCE_THRESHOLD)
            .map(|x| CefDirtyRect::new(i32::try_from(x).expect("small x"), 0, 1, 1))
            .collect::<Vec<_>>();
        let bytes = vec![255_u8; 64 * 10 * 4];
        let mut compositor = CefUiCompositor::new();
        compositor
            .ingest_paint(paint_frame(
                64,
                10,
                &[CefDirtyRect::new(0, 0, 1, 1)],
                &bytes,
            ))
            .expect("first frame");
        let _ = compositor.consume_ready();
        compositor
            .ingest_paint(paint_frame(64, 10, &rects, &bytes))
            .expect("coalesced frame");
        let frame = compositor.consume_ready().expect("ready frame");

        assert_eq!(
            frame.metadata.dirty_rects,
            vec![CefDirtyRect::new(0, 0, 33, 1)]
        );
    }

    #[test]
    fn compositor_full_uploads_on_resize() {
        let rects = [CefDirtyRect::new(0, 0, 1, 1)];
        let bytes_2x2 = [255_u8; 16];
        let bytes_3x3 = [255_u8; 36];
        let mut compositor = CefUiCompositor::new();
        compositor
            .ingest_paint(paint_frame(2, 2, &rects, &bytes_2x2))
            .expect("first frame");
        let _ = compositor.consume_ready();
        let generation = compositor
            .ingest_paint(paint_frame(3, 3, &rects, &bytes_3x3))
            .expect("resized frame");
        let frame = compositor.consume_ready().expect("ready frame");

        assert_eq!(
            frame.upload_plan,
            CefUiUploadPlan::FullFrame {
                generation,
                reason: CefUiFullUploadReason::Resize
            }
        );
    }

    #[test]
    fn compositor_rejects_paint_after_shutdown_begins() {
        let mut compositor = CefUiCompositor::new();
        compositor.stop_accepting_paint();

        assert_eq!(
            compositor.ingest_paint(CefPaintFrame::new(
                CefPaintElement::View,
                1,
                1,
                &[],
                &[0, 0, 0, 0]
            )),
            Err(UiCompositorError::ShuttingDown)
        );
    }

    #[test]
    fn overlay_behavior_click_through_only_for_passive_hud() {
        assert!(
            CefUiOverlayBehavior::for_mode(CefUiOverlayMode::PassiveHudClickThrough).click_through
        );
        assert!(!CefUiOverlayBehavior::for_mode(CefUiOverlayMode::Interactive).click_through);
    }
}
