const MAX_TEXT_INPUT_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum BrowserUiInputOwner {
    Game,
    BrowserUi,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum BrowserUiInputEvent {
    MouseMove { x: i32, y: i32 },
    MouseButton { button: MouseButton, pressed: bool },
    MouseWheel { delta_x: i32, delta_y: i32 },
    Key { code: u32, pressed: bool },
    Text { utf8: String },
    Focus { focused: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserUiInputValidationError {
    OutsideViewport,
    OversizeText,
}

pub fn validate_input_event(
    event: &BrowserUiInputEvent,
    viewport_width: i32,
    viewport_height: i32,
) -> Result<(), BrowserUiInputValidationError> {
    match event {
        BrowserUiInputEvent::MouseMove { x, y } => {
            if *x < 0 || *y < 0 || *x >= viewport_width || *y >= viewport_height {
                return Err(BrowserUiInputValidationError::OutsideViewport);
            }
            Ok(())
        }
        BrowserUiInputEvent::Text { utf8 } if utf8.len() > MAX_TEXT_INPUT_BYTES => {
            Err(BrowserUiInputValidationError::OversizeText)
        }
        BrowserUiInputEvent::MouseButton { .. }
        | BrowserUiInputEvent::MouseWheel { .. }
        | BrowserUiInputEvent::Key { .. }
        | BrowserUiInputEvent::Text { .. }
        | BrowserUiInputEvent::Focus { .. } => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_pointer_events_outside_viewport() {
        assert_eq!(
            validate_input_event(
                &BrowserUiInputEvent::MouseMove { x: 2000, y: 20 },
                1280,
                720
            ),
            Err(BrowserUiInputValidationError::OutsideViewport)
        );
    }

    #[test]
    fn rejects_oversize_text_events() {
        assert_eq!(
            validate_input_event(
                &BrowserUiInputEvent::Text {
                    utf8: "a".repeat(MAX_TEXT_INPUT_BYTES + 1),
                },
                1280,
                720
            ),
            Err(BrowserUiInputValidationError::OversizeText)
        );
    }
}
