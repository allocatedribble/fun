//! Pass-72 typed diagnostics surfaced to the product diagnostic
//! channel.
//!
//! The adapter never reflects raw host or backend error strings
//! into product copy. Every rejection lifts to a typed
//! [`ProductRvelteDiagnostic`] variant whose [`code`] returns a
//! stable `fun.product.rvelte_bridge.*` identifier.

use fun_native_app::FunNativeAppError;
use rvelte_fun_native_codegen::host_bridge::FunNativeHostError;
use rvelte_fun_render_adapter::FunRenderUiAdapterError;

/// Stable schema label for the product-side adapter diagnostic
/// contract.
pub const PRODUCT_RVELTE_ADAPTER_SCHEMA: &str = "fun.product.rvelte_bridge.adapter.v1";

/// Numeric schema version for the product-side adapter diagnostic
/// contract.
pub const PRODUCT_RVELTE_ADAPTER_SCHEMA_VERSION: u16 = 1;

/// Typed diagnostic surfaced to the product diagnostic channel.
#[derive(Clone, Debug)]
pub enum ProductRvelteDiagnostic {
    /// Host-bridge layer rejected an inbound payload.
    HostBridge(FunNativeHostError),
    /// Renderer-adapter layer rejected the frame.
    Adapter(FunRenderUiAdapterError),
    /// App-shell layer rejected a runtime-loop step.
    AppShell(FunNativeAppError),
    /// Product input event could not be translated to typed rvelte
    /// input. The reason is a stable label.
    InputTranslation {
        /// Stable reason label.
        reason: &'static str,
    },
    /// The adapter was asked to act on a route the registry does
    /// not know.
    UnknownRoute {
        /// Stable u32 product-route code that was unknown.
        route_id: u32,
    },
    /// `tick` was called with no active route mounted. Pass-72
    /// safe blank/error UI state: callers must mount a route or
    /// handle the diagnostic.
    NoActiveRoute,
    /// `tick` discovered the renderer sink had been taken twice.
    /// This indicates a programmer error in the adapter; pass-72
    /// returns the typed diagnostic rather than panicking so the
    /// product can fail closed.
    RendererSinkUnavailable,
}

impl ProductRvelteDiagnostic {
    /// Stable machine code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::HostBridge(_) => "fun.product.rvelte_bridge.host_bridge",
            Self::Adapter(_) => "fun.product.rvelte_bridge.adapter",
            Self::AppShell(_) => "fun.product.rvelte_bridge.app_shell",
            Self::InputTranslation { .. } => "fun.product.rvelte_bridge.input_translation",
            Self::UnknownRoute { .. } => "fun.product.rvelte_bridge.unknown_route",
            Self::NoActiveRoute => "fun.product.rvelte_bridge.no_active_route",
            Self::RendererSinkUnavailable => "fun.product.rvelte_bridge.renderer_sink_unavailable",
        }
    }
}

impl From<FunNativeHostError> for ProductRvelteDiagnostic {
    fn from(error: FunNativeHostError) -> Self {
        Self::HostBridge(error)
    }
}

impl From<FunRenderUiAdapterError> for ProductRvelteDiagnostic {
    fn from(error: FunRenderUiAdapterError) -> Self {
        Self::Adapter(error)
    }
}

impl From<FunNativeAppError> for ProductRvelteDiagnostic {
    fn from(error: FunNativeAppError) -> Self {
        Self::AppShell(error)
    }
}
