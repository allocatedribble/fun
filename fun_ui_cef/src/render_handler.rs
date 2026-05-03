#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefPaintElement {
    View,
    Popup,
}

impl CefPaintElement {
    #[must_use]
    pub fn from_cef(value: cef::PaintElementType) -> Self {
        if value == cef::PaintElementType::POPUP {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum CefPaintBufferFormat {
    Bgra8Premultiplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefPaintFrame<'a> {
    pub element: CefPaintElement,
    pub width: i32,
    pub height: i32,
    pub format: CefPaintBufferFormat,
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
            format: CefPaintBufferFormat::Bgra8Premultiplied,
            dirty_rects,
            bytes,
        }
    }

    #[must_use]
    pub fn expected_byte_len(self) -> Option<usize> {
        let width = usize::try_from(self.width).ok()?;
        let height = usize::try_from(self.height).ok()?;
        width.checked_mul(height)?.checked_mul(4)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefPaintValidationError {
    InvalidSize,
    ByteLengthMismatch,
    DirtyRectOutOfBounds,
}

pub fn validate_paint_frame(frame: CefPaintFrame<'_>) -> Result<(), CefPaintValidationError> {
    let Some(expected_byte_len) = frame.expected_byte_len() else {
        return Err(CefPaintValidationError::InvalidSize);
    };
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
}
