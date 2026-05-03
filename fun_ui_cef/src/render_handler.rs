use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cef::rc::Rc;
use cef::{
    AcceleratedPaintInfo, Browser, ImplRenderHandler, PaintElementType, Rect, RenderHandler,
    ScreenInfo, WrapRenderHandler, wrap_render_handler,
};

use crate::diagnostics::{FUN_UI_DIAGNOSTICS_TARGET, SharedCefUiTransportCounters};

pub const CEF_UI_BYTES_PER_PIXEL: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefPaintElement {
    View,
    Popup,
}

impl CefPaintElement {
    #[must_use]
    pub fn from_cef(value: PaintElementType) -> Self {
        if value == PaintElementType::POPUP {
            Self::Popup
        } else {
            Self::View
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct CefDirtyRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl CefDirtyRect {
    #[must_use]
    pub const fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn full_frame(width: i32, height: i32) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    #[must_use]
    pub const fn is_inside(self, width: i32, height: i32) -> bool {
        if self.is_empty() || width <= 0 || height <= 0 {
            return false;
        }
        self.x >= 0
            && self.y >= 0
            && self.x.saturating_add(self.width) <= width
            && self.y.saturating_add(self.height) <= height
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let left = self.x.min(other.x);
        let top = self.y.min(other.y);
        let right = self
            .x
            .saturating_add(self.width)
            .max(other.x.saturating_add(other.width));
        let bottom = self
            .y
            .saturating_add(self.height)
            .max(other.y.saturating_add(other.height));
        Self {
            x: left,
            y: top,
            width: right.saturating_sub(left),
            height: bottom.saturating_sub(top),
        }
    }
}

impl From<&Rect> for CefDirtyRect {
    fn from(value: &Rect) -> Self {
        Self {
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefPaintBufferFormat {
    Bgra8Premultiplied,
}

impl CefPaintBufferFormat {
    #[must_use]
    pub const fn requires_normalization_for_overlay(self) -> bool {
        match self {
            Self::Bgra8Premultiplied => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefUiAlphaMode {
    Premultiplied,
    Unknown,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct CefUiFrameGeneration(pub u64);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct CefUiFrameTimestampNs(pub u64);

impl CefUiFrameTimestampNs {
    #[must_use]
    pub fn now() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                duration.as_nanos().min(u128::from(u64::MAX)) as u64
            });
        Self(nanos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct CefUiScaleFactor {
    pub millipoints: u32,
}

impl CefUiScaleFactor {
    pub const ONE: Self = Self { millipoints: 1000 };

    #[must_use]
    pub const fn from_millipoints(millipoints: u32) -> Self {
        Self { millipoints }
    }

    #[must_use]
    pub fn from_f32(scale_factor: f32) -> Option<Self> {
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return None;
        }
        let millipoints = (scale_factor * 1000.0).round();
        if !(1.0..=u32::MAX as f32).contains(&millipoints) {
            return None;
        }
        Some(Self {
            millipoints: millipoints as u32,
        })
    }

    #[must_use]
    pub const fn as_f32(self) -> f32 {
        self.millipoints as f32 / 1000.0
    }
}

impl Default for CefUiScaleFactor {
    fn default() -> Self {
        Self::ONE
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct CefUiFrameMetadata {
    pub width: i32,
    pub height: i32,
    pub scale_factor: CefUiScaleFactor,
    pub generation: CefUiFrameGeneration,
    pub dirty_rects: Vec<CefDirtyRect>,
    pub alpha_mode: CefUiAlphaMode,
    pub timestamp_ns: CefUiFrameTimestampNs,
}

impl CefUiFrameMetadata {
    #[must_use]
    pub fn new(
        width: i32,
        height: i32,
        scale_factor: CefUiScaleFactor,
        generation: CefUiFrameGeneration,
        dirty_rects: Vec<CefDirtyRect>,
        alpha_mode: CefUiAlphaMode,
        timestamp_ns: CefUiFrameTimestampNs,
    ) -> Self {
        Self {
            width,
            height,
            scale_factor,
            generation,
            dirty_rects,
            alpha_mode,
            timestamp_ns,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefPaintFrame<'a> {
    pub element: CefPaintElement,
    pub width: i32,
    pub height: i32,
    pub scale_factor: CefUiScaleFactor,
    pub format: CefPaintBufferFormat,
    pub alpha_mode: CefUiAlphaMode,
    pub timestamp_ns: CefUiFrameTimestampNs,
    pub dirty_rects: &'a [CefDirtyRect],
    pub bytes: &'a [u8],
}

impl<'a> CefPaintFrame<'a> {
    #[must_use]
    pub const fn new(
        element: CefPaintElement,
        width: i32,
        height: i32,
        dirty_rects: &'a [CefDirtyRect],
        bytes: &'a [u8],
    ) -> Self {
        Self {
            element,
            width,
            height,
            scale_factor: CefUiScaleFactor::ONE,
            format: CefPaintBufferFormat::Bgra8Premultiplied,
            alpha_mode: CefUiAlphaMode::Premultiplied,
            timestamp_ns: CefUiFrameTimestampNs(0),
            dirty_rects,
            bytes,
        }
    }

    #[must_use]
    pub const fn with_metadata(
        mut self,
        scale_factor: CefUiScaleFactor,
        alpha_mode: CefUiAlphaMode,
        timestamp_ns: CefUiFrameTimestampNs,
    ) -> Self {
        self.scale_factor = scale_factor;
        self.alpha_mode = alpha_mode;
        self.timestamp_ns = timestamp_ns;
        self
    }

    #[must_use]
    pub fn expected_byte_len(self) -> Option<usize> {
        expected_paint_byte_len(self.width, self.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CefOwnedPaintFrame {
    pub element: CefPaintElement,
    pub width: i32,
    pub height: i32,
    pub scale_factor: CefUiScaleFactor,
    pub format: CefPaintBufferFormat,
    pub alpha_mode: CefUiAlphaMode,
    pub timestamp_ns: CefUiFrameTimestampNs,
    pub dirty_rects: Vec<CefDirtyRect>,
    pub bytes: Vec<u8>,
}

impl CefOwnedPaintFrame {
    #[must_use]
    pub fn borrowed(&self) -> CefPaintFrame<'_> {
        CefPaintFrame {
            element: self.element,
            width: self.width,
            height: self.height,
            scale_factor: self.scale_factor,
            format: self.format,
            alpha_mode: self.alpha_mode,
            timestamp_ns: self.timestamp_ns,
            dirty_rects: &self.dirty_rects,
            bytes: &self.bytes,
        }
    }
}

pub trait CefPaintSink: Send + Sync + 'static {
    fn ingest_cef_paint(&self, frame: CefOwnedPaintFrame);
}

#[derive(Clone)]
struct FunCefRenderHandlerLogs {
    view_rect_logged: Arc<AtomicBool>,
    paint_logged: Arc<AtomicBool>,
    accelerated_paint_logged: Arc<AtomicBool>,
}

impl Default for FunCefRenderHandlerLogs {
    fn default() -> Self {
        Self {
            view_rect_logged: Arc::new(AtomicBool::new(false)),
            paint_logged: Arc::new(AtomicBool::new(false)),
            accelerated_paint_logged: Arc::new(AtomicBool::new(false)),
        }
    }
}

wrap_render_handler! {
    pub struct FunCefRenderHandler {
        paint_sink: Arc<dyn CefPaintSink>,
        scale_factor: CefUiScaleFactor,
        viewport_width: i32,
        viewport_height: i32,
        transport_counters: SharedCefUiTransportCounters,
        logs: FunCefRenderHandlerLogs,
    }

    impl RenderHandler {
        fn root_screen_rect(
            &self,
            _browser: Option<&mut Browser>,
            rect: Option<&mut Rect>,
        ) -> std::os::raw::c_int {
            if let Some(rect) = rect {
                *rect = viewport_rect(self.viewport_width, self.viewport_height);
            }
            1
        }

        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if !self.logs.view_rect_logged.swap(true, Ordering::AcqRel) {
                tracing::info!(
                    target: FUN_UI_DIAGNOSTICS_TARGET,
                    width = self.viewport_width,
                    height = self.viewport_height,
                    "CEF UI render handler view rect requested"
                );
            }
            if let Some(rect) = rect {
                *rect = Rect {
                    x: 0,
                    y: 0,
                    width: self.viewport_width,
                    height: self.viewport_height,
                };
            }
        }

        fn screen_point(
            &self,
            _browser: Option<&mut Browser>,
            view_x: std::os::raw::c_int,
            view_y: std::os::raw::c_int,
            screen_x: Option<&mut std::os::raw::c_int>,
            screen_y: Option<&mut std::os::raw::c_int>,
        ) -> std::os::raw::c_int {
            if let Some(screen_x) = screen_x {
                *screen_x = view_x;
            }
            if let Some(screen_y) = screen_y {
                *screen_y = view_y;
            }
            1
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> std::os::raw::c_int {
            if let Some(screen_info) = screen_info {
                *screen_info = viewport_screen_info(
                    self.viewport_width,
                    self.viewport_height,
                    self.scale_factor,
                );
            }
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: i32,
            height: i32,
        ) {
            let Some(expected_byte_len) = expected_paint_byte_len(width, height) else {
                return;
            };

            let dirty_rects: Vec<CefDirtyRect> = dirty_rects
                .unwrap_or_default()
                .iter()
                .map(CefDirtyRect::from)
                .collect();

            let Some(bytes) = copy_cef_paint_buffer(buffer, expected_byte_len) else {
                return;
            };
            self.transport_counters.record_on_paint(bytes.len());
            if !self.logs.paint_logged.swap(true, Ordering::AcqRel) {
                tracing::info!(
                    target: FUN_UI_DIAGNOSTICS_TARGET,
                    width,
                    height,
                    dirty_rect_count = dirty_rects.len(),
                    "CEF UI paint frame received"
                );
            }
            self.paint_sink.ingest_cef_paint(CefOwnedPaintFrame {
                element: CefPaintElement::from_cef(type_),
                width,
                height,
                scale_factor: self.scale_factor,
                format: CefPaintBufferFormat::Bgra8Premultiplied,
                alpha_mode: CefUiAlphaMode::Premultiplied,
                timestamp_ns: CefUiFrameTimestampNs::now(),
                dirty_rects,
                bytes,
            });
        }

        fn on_accelerated_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            dirty_rects: Option<&[Rect]>,
            info: Option<&AcceleratedPaintInfo>,
        ) {
            self.transport_counters.record_on_accelerated_paint();
            self.transport_counters.record_gpu_copy_failure();
            if !self.logs.accelerated_paint_logged.swap(true, Ordering::AcqRel) {
                tracing::warn!(
                    target: FUN_UI_DIAGNOSTICS_TARGET,
                    element = ?CefPaintElement::from_cef(type_),
                    dirty_rect_count = dirty_rects.unwrap_or_default().len(),
                    shared_texture_handle_present = accelerated_paint_shared_texture_present(info),
                    "CEF UI accelerated paint received before GPU transport bridge is ready"
                );
            }
        }

    }
}

#[must_use]
pub fn new_fun_cef_render_handler(
    paint_sink: Arc<dyn CefPaintSink>,
    scale_factor: CefUiScaleFactor,
) -> RenderHandler {
    new_fun_cef_render_handler_for_viewport(paint_sink, scale_factor, 1280, 720)
}

#[must_use]
pub fn new_fun_cef_render_handler_for_viewport(
    paint_sink: Arc<dyn CefPaintSink>,
    scale_factor: CefUiScaleFactor,
    viewport_width: u32,
    viewport_height: u32,
) -> RenderHandler {
    new_fun_cef_render_handler_for_viewport_with_counters(
        paint_sink,
        scale_factor,
        viewport_width,
        viewport_height,
        SharedCefUiTransportCounters::default(),
    )
}

#[must_use]
pub fn new_fun_cef_render_handler_for_viewport_with_counters(
    paint_sink: Arc<dyn CefPaintSink>,
    scale_factor: CefUiScaleFactor,
    viewport_width: u32,
    viewport_height: u32,
    transport_counters: SharedCefUiTransportCounters,
) -> RenderHandler {
    FunCefRenderHandler::new(
        paint_sink,
        scale_factor,
        viewport_width.min(i32::MAX as u32) as i32,
        viewport_height.min(i32::MAX as u32) as i32,
        transport_counters,
        FunCefRenderHandlerLogs::default(),
    )
}

#[must_use]
fn viewport_rect(width: i32, height: i32) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width,
        height,
    }
}

#[must_use]
fn viewport_screen_info(width: i32, height: i32, scale_factor: CefUiScaleFactor) -> ScreenInfo {
    let rect = viewport_rect(width, height);
    ScreenInfo {
        device_scale_factor: scale_factor.as_f32(),
        depth: 32,
        depth_per_component: 8,
        is_monochrome: 0,
        rect: rect.clone(),
        available_rect: rect,
        ..ScreenInfo::default()
    }
}

fn copy_cef_paint_buffer(buffer: *const u8, expected_byte_len: usize) -> Option<Vec<u8>> {
    if buffer.is_null() {
        return None;
    }
    // CEF owns `buffer` only for the paint callback; copy it before returning.
    Some(unsafe { std::slice::from_raw_parts(buffer, expected_byte_len) }.to_vec())
}

#[cfg(target_os = "windows")]
fn accelerated_paint_shared_texture_present(info: Option<&AcceleratedPaintInfo>) -> bool {
    info.is_some_and(|info| !info.shared_texture_handle.is_null())
}

#[cfg(not(target_os = "windows"))]
fn accelerated_paint_shared_texture_present(_info: Option<&AcceleratedPaintInfo>) -> bool {
    false
}

#[must_use]
pub fn expected_paint_byte_len(width: i32, height: i32) -> Option<usize> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    width
        .checked_mul(height)?
        .checked_mul(CEF_UI_BYTES_PER_PIXEL)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefPaintValidationError {
    InvalidSize,
    ByteLengthMismatch,
    DirtyRectOutOfBounds,
    UnsupportedFormat,
}

pub fn validate_paint_frame(frame: CefPaintFrame<'_>) -> Result<(), CefPaintValidationError> {
    let Some(expected_byte_len) = frame.expected_byte_len() else {
        return Err(CefPaintValidationError::InvalidSize);
    };
    if frame.format.requires_normalization_for_overlay() {
        return Err(CefPaintValidationError::UnsupportedFormat);
    }
    if expected_byte_len != frame.bytes.len() {
        return Err(CefPaintValidationError::ByteLengthMismatch);
    }
    for rect in frame.dirty_rects {
        if !rect.is_inside(frame.width, frame.height) {
            return Err(CefPaintValidationError::DirtyRectOutOfBounds);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dirty_rects_outside_paint_buffer() {
        let rects = [CefDirtyRect::new(10, 10, 20, 20)];
        let bytes = [0_u8; 16];
        let frame = CefPaintFrame::new(CefPaintElement::View, 2, 2, &rects, &bytes);

        assert_eq!(
            validate_paint_frame(frame),
            Err(CefPaintValidationError::DirtyRectOutOfBounds)
        );
    }

    #[test]
    fn rejects_zero_sized_paint_buffer() {
        let frame = CefPaintFrame::new(CefPaintElement::View, 0, 1, &[], &[]);

        assert_eq!(
            validate_paint_frame(frame),
            Err(CefPaintValidationError::InvalidSize)
        );
    }

    #[test]
    fn unions_dirty_rectangles() {
        let a = CefDirtyRect::new(2, 4, 8, 10);
        let b = CefDirtyRect::new(8, 1, 4, 4);

        assert_eq!(a.union(b), CefDirtyRect::new(2, 1, 10, 13));
    }

    #[test]
    fn viewport_screen_info_matches_offscreen_viewport() {
        let screen_info = viewport_screen_info(1280, 720, CefUiScaleFactor::from_millipoints(1500));

        assert_eq!(screen_info.device_scale_factor, 1.5);
        assert_eq!(screen_info.depth, 32);
        assert_eq!(screen_info.depth_per_component, 8);
        assert_eq!(screen_info.rect.width, 1280);
        assert_eq!(screen_info.available_rect.height, 720);
    }
}
