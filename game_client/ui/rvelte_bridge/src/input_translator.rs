//! Pass-72 product input adapter.
//!
//! Translates product-side native input events into the typed
//! [`rvelte_fun_input::FunUiNativeInputEvent`] that the rvelte
//! input router consumes. The translator is the only call site that
//! crosses from product input identity into rvelte's typed input
//! contract; every other call site routes through it.

use rvelte_fun_input::{
    FunUiInputModifiers, FunUiKeyCode, FunUiKeyInput, FunUiNativeInputEvent, FunUiPointerButton,
    FunUiPointerId, FunUiPointerInput, FunUiTextInput,
};
use rvelte_fun_ui_core::FunUiPoint;
use serde::{Deserialize, Serialize};

/// Product-side input event before translation. Carries only typed
/// payloads — no JS event objects, no DOM `MouseEvent`. Pass 72
/// covers the input categories the existing app shell scenarios
/// already exercise; later passes can add new typed variants
/// without breaking the translator's exhaustiveness check.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ProductInputEvent {
    /// Pointer down at logical coordinates.
    PointerDown {
        /// Stable pointer ID.
        pointer_id: u64,
        /// Logical x coordinate in UI units.
        x: i32,
        /// Logical y coordinate in UI units.
        y: i32,
        /// Pointer button.
        button: ProductPointerButton,
    },
    /// Pointer up at logical coordinates.
    PointerUp {
        /// Stable pointer ID.
        pointer_id: u64,
        /// Logical x coordinate in UI units.
        x: i32,
        /// Logical y coordinate in UI units.
        y: i32,
        /// Pointer button.
        button: ProductPointerButton,
    },
    /// Pointer move at logical coordinates.
    PointerMove {
        /// Stable pointer ID.
        pointer_id: u64,
        /// Logical x coordinate in UI units.
        x: i32,
        /// Logical y coordinate in UI units.
        y: i32,
    },
    /// Key down with a typed key code.
    KeyDown {
        /// Stable key code.
        key: ProductKey,
        /// Whether shift is held.
        shift: bool,
    },
    /// Text input fragment.
    TextInput {
        /// Bounded text fragment (max 64 ASCII characters).
        text: String,
    },
    /// Move focus to the next focusable target.
    FocusNext,
    /// Move focus to the previous focusable target.
    FocusPrevious,
    /// Activate the focused target.
    Activate,
    /// Cancel the active modal or focused cancel target.
    Cancel,
}

/// Stable pointer-button identifier exposed at the product input
/// boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ProductPointerButton {
    /// Primary button (typically left mouse).
    Primary,
    /// Secondary button (typically right mouse).
    Secondary,
    /// Middle button.
    Middle,
}

impl ProductPointerButton {
    fn into_typed(self) -> FunUiPointerButton {
        match self {
            Self::Primary => FunUiPointerButton::Primary,
            Self::Secondary => FunUiPointerButton::Secondary,
            Self::Middle => FunUiPointerButton::Middle,
        }
    }
}

/// Stable key identifier exposed at the product input boundary.
/// Pass 72 covers the keys the existing scenarios exercise; later
/// passes can extend this enum without breaking the translator's
/// exhaustive match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum ProductKey {
    /// Enter / Return.
    Enter,
    /// Space.
    Space,
    /// Escape.
    Escape,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Arrow up.
    ArrowUp,
    /// Arrow down.
    ArrowDown,
    /// Arrow left.
    ArrowLeft,
    /// Arrow right.
    ArrowRight,
}

impl ProductKey {
    fn into_typed(self) -> FunUiKeyCode {
        match self {
            Self::Enter => FunUiKeyCode::Enter,
            Self::Space => FunUiKeyCode::Space,
            Self::Escape => FunUiKeyCode::Escape,
            Self::Tab => FunUiKeyCode::Tab,
            Self::Backspace => FunUiKeyCode::Backspace,
            Self::ArrowUp => FunUiKeyCode::ArrowUp,
            Self::ArrowDown => FunUiKeyCode::ArrowDown,
            Self::ArrowLeft => FunUiKeyCode::ArrowLeft,
            Self::ArrowRight => FunUiKeyCode::ArrowRight,
        }
    }
}

/// Translates a product input event into the typed rvelte input
/// event. Returns a stable rejection reason on failure; the
/// adapter surfaces these as
/// [`crate::ProductRvelteDiagnostic::InputTranslation`].
pub fn translate_product_input(
    event: ProductInputEvent,
) -> Result<FunUiNativeInputEvent, &'static str> {
    match event {
        ProductInputEvent::PointerDown {
            pointer_id,
            x,
            y,
            button,
        } => Ok(FunUiNativeInputEvent::PointerDown {
            pointer: FunUiPointerInput::new(FunUiPointerId::new(pointer_id), FunUiPoint::new(x, y))
                .with_button(button.into_typed()),
        }),
        ProductInputEvent::PointerUp {
            pointer_id,
            x,
            y,
            button,
        } => Ok(FunUiNativeInputEvent::PointerUp {
            pointer: FunUiPointerInput::new(FunUiPointerId::new(pointer_id), FunUiPoint::new(x, y))
                .with_button(button.into_typed()),
        }),
        ProductInputEvent::PointerMove { pointer_id, x, y } => {
            Ok(FunUiNativeInputEvent::PointerMove {
                pointer: FunUiPointerInput::new(
                    FunUiPointerId::new(pointer_id),
                    FunUiPoint::new(x, y),
                ),
            })
        }
        ProductInputEvent::KeyDown { key, shift } => {
            let mut modifiers = FunUiInputModifiers::NONE;
            modifiers.shift = shift;
            Ok(FunUiNativeInputEvent::KeyDown {
                key: FunUiKeyInput {
                    key: key.into_typed(),
                    modifiers,
                    repeat: false,
                },
            })
        }
        ProductInputEvent::TextInput { text } => {
            if text.is_empty() {
                return Err("text_input_empty");
            }
            if text.len() > 64 {
                return Err("text_input_too_long");
            }
            for ch in text.chars() {
                if ch.is_control() && ch != ' ' {
                    return Err("text_input_control_character");
                }
                if !ch.is_ascii() {
                    return Err("text_input_non_ascii");
                }
            }
            Ok(FunUiNativeInputEvent::TextInput {
                text: FunUiTextInput { text },
            })
        }
        ProductInputEvent::FocusNext => Ok(FunUiNativeInputEvent::FocusNext),
        ProductInputEvent::FocusPrevious => Ok(FunUiNativeInputEvent::FocusPrevious),
        ProductInputEvent::Activate => Ok(FunUiNativeInputEvent::Activate),
        ProductInputEvent::Cancel => Ok(FunUiNativeInputEvent::Cancel),
    }
}
