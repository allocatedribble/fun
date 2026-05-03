use crate::render_handler::{
    CefDirtyRect, CefPaintElement, CefPaintFrame, CefPaintValidationError, validate_paint_frame,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct UiSurfaceGeneration(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCompositorBuffer {
    pub generation: UiSurfaceGeneration,
    pub element: CefPaintElement,
    pub width: i32,
    pub height: i32,
    pixels: Vec<u8>,
}

impl UiCompositorBuffer {
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiCompositorState {
    latest_generation: UiSurfaceGeneration,
    view_buffer: Option<UiCompositorBuffer>,
    popup_buffer: Option<UiCompositorBuffer>,
    dirty_rects: Vec<CefDirtyRect>,
    accepting_paint: bool,
}

impl UiCompositorState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            latest_generation: UiSurfaceGeneration(0),
            view_buffer: None,
            popup_buffer: None,
            dirty_rects: Vec::new(),
            accepting_paint: true,
        }
    }

    pub fn stop_accepting_paint(&mut self) {
        self.accepting_paint = false;
    }

    pub fn release_buffers(&mut self) {
        self.view_buffer = None;
        self.popup_buffer = None;
        self.dirty_rects.clear();
    }

    pub fn ingest_paint(
        &mut self,
        frame: CefPaintFrame<'_>,
    ) -> Result<UiSurfaceGeneration, UiCompositorError> {
        if !self.accepting_paint {
            return Err(UiCompositorError::ShuttingDown);
        }
        validate_paint_frame(frame).map_err(UiCompositorError::InvalidPaint)?;

        self.latest_generation = UiSurfaceGeneration(self.latest_generation.0.saturating_add(1));
        self.dirty_rects.clear();
        self.dirty_rects.extend_from_slice(frame.dirty_rects);

        let buffer = UiCompositorBuffer {
            generation: self.latest_generation,
            element: frame.element,
            width: frame.width,
            height: frame.height,
            pixels: frame.bytes.to_vec(),
        };
        match frame.element {
            CefPaintElement::View => self.view_buffer = Some(buffer),
            CefPaintElement::Popup => self.popup_buffer = Some(buffer),
        }

        Ok(self.latest_generation)
    }

    #[must_use]
    pub const fn latest_generation(&self) -> UiSurfaceGeneration {
        self.latest_generation
    }

    #[must_use]
    pub fn dirty_rects(&self) -> &[CefDirtyRect] {
        &self.dirty_rects
    }
}

impl Default for UiCompositorState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCompositorError {
    ShuttingDown,
    InvalidPaint(CefPaintValidationError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositor_tracks_generation_and_dirty_rects() {
        let rects = [CefDirtyRect::new(0, 0, 2, 2)];
        let bytes = [255_u8; 16];
        let mut compositor = UiCompositorState::new();
        let generation = compositor
            .ingest_paint(CefPaintFrame::new(
                CefPaintElement::View,
                2,
                2,
                &rects,
                &bytes,
            ))
            .expect("valid paint frame");

        assert_eq!(generation, UiSurfaceGeneration(1));
        assert_eq!(compositor.dirty_rects(), &rects);
    }

    #[test]
    fn compositor_rejects_paint_after_shutdown_begins() {
        let mut compositor = UiCompositorState::new();
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
}
