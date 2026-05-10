//! Pass 73 product renderer sink boundary.
//!
//! Pass 71 (`rvelte/docs/project-fun-native-adapter-design.md`)
//! pinned the product-owned renderer surface as a "product
//! `Renderer2DCommandSink` that talks to fun-renderer". This module
//! lands the typed marker for that surface.
//!
//! [`ProductRenderer2DCommandSink`] is a renderer-independent typed
//! marker around the rvelte-side
//! [`rvelte_fun_render_adapter::Renderer2DCommandSink`] composite
//! trait. Product code names its concrete sink by implementing the
//! marker plus the four underlying sub-traits (`Renderer2DDevice`,
//! `Renderer2DResourceArena`, `Renderer2DFrame`, `Renderer2DLayer`).
//!
//! [`ProductRendererBackend`] is the typed catalog of concrete
//! backends a product process can opt into. Today the only
//! supported backend is the fun-renderer native UI adapter behind
//! the `fun_renderer_backend` feature. Future passes may add more
//! backends (e.g. a CPU rasterizer for headless tooling).
//!
//! The marker carries no methods of its own — it exists so the
//! design's typed boundary is named in code, and so the adapter can
//! refuse to accept a sink that has not opted into the product
//! contract.

use rvelte_fun_render_adapter::{
    FunRenderUiCapabilityReport, Renderer2DCommandSink, Renderer2DDevice,
};

/// Stable schema label for the product renderer-sink contract.
pub const PRODUCT_RENDERER_SINK_SCHEMA: &str = "fun.rvelte.product_renderer_sink.v1";

/// Numeric schema version for the product renderer-sink contract.
pub const PRODUCT_RENDERER_SINK_SCHEMA_VERSION: u16 = 1;

/// Typed catalog of product renderer backends. Pass 73 names one
/// concrete backend; later passes may add more (CPU rasterizer for
/// headless tooling, software-only fallback, replay sink for
/// deterministic benchmarks).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ProductRendererBackend {
    /// In-memory rvelte-side fake renderer (test lane).
    Fake = 0,
    /// fun-renderer's native UI adapter behind the
    /// `fun_renderer_backend` feature.
    FunRendererNativeUiAdapter = 1,
}

impl ProductRendererBackend {
    /// Stable label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fake => "fake",
            Self::FunRendererNativeUiAdapter => "fun_renderer_native_ui_adapter",
        }
    }

    /// Every backend in stable order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[Self::Fake, Self::FunRendererNativeUiAdapter]
    }
}

/// Typed marker that the product adapter expects every renderer
/// sink to implement. Pass 73 keeps the marker empty so any type
/// that implements rvelte's four `Renderer2D*` sub-traits is
/// automatically a `ProductRenderer2DCommandSink` through the
/// blanket impl below.
///
/// The marker carries one required method
/// ([`Self::product_backend`]) so a sink can name which concrete
/// backend it represents; the adapter uses this label in typed
/// diagnostics and in test scaffolding.
///
/// The marker is renderer-independent by construction: every
/// method is over rvelte typed payloads only, never over a backend
/// handle.
pub trait ProductRenderer2DCommandSink: Renderer2DCommandSink {
    /// Returns the typed backend this sink represents.
    fn product_backend(&self) -> ProductRendererBackend;
}

/// Helper that confirms a sink reports a non-empty capability
/// report. Pass 73 keeps the check intentionally tiny — the
/// product adapter consults this on mount to surface a typed
/// diagnostic if a sink mis-reports zero capabilities.
#[must_use]
pub fn sink_capability_report<S: ProductRenderer2DCommandSink>(
    sink: &S,
) -> FunRenderUiCapabilityReport {
    Renderer2DDevice::capability_report(sink)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "test-only assertions")]
mod tests {
    use super::{
        PRODUCT_RENDERER_SINK_SCHEMA, PRODUCT_RENDERER_SINK_SCHEMA_VERSION,
        ProductRenderer2DCommandSink, ProductRendererBackend, sink_capability_report,
    };
    use rvelte_fun_render_adapter::fake::FakeRenderer2DCommandSink;

    impl ProductRenderer2DCommandSink for FakeRenderer2DCommandSink {
        fn product_backend(&self) -> ProductRendererBackend {
            ProductRendererBackend::Fake
        }
    }

    #[test]
    fn schema_label_is_stable() {
        assert_eq!(
            PRODUCT_RENDERER_SINK_SCHEMA,
            "fun.rvelte.product_renderer_sink.v1"
        );
        assert_eq!(PRODUCT_RENDERER_SINK_SCHEMA_VERSION, 1);
    }

    #[test]
    fn two_backends_named() {
        assert_eq!(ProductRendererBackend::all().len(), 2);
        for b in ProductRendererBackend::all() {
            assert!(!b.as_str().is_empty());
        }
    }

    #[test]
    fn fake_renderer_implements_product_sink_marker() {
        let renderer = FakeRenderer2DCommandSink::new();
        assert_eq!(renderer.product_backend(), ProductRendererBackend::Fake);
    }

    #[test]
    fn capability_report_helper_round_trips_through_marker() {
        let renderer = FakeRenderer2DCommandSink::new();
        let report = sink_capability_report(&renderer);
        assert!(report.supports_solid_rect);
    }

    #[test]
    fn backend_labels_are_unique() {
        let mut seen = Vec::new();
        for b in ProductRendererBackend::all() {
            assert!(
                !seen.contains(&b.as_str()),
                "duplicate label: {}",
                b.as_str()
            );
            seen.push(b.as_str());
        }
    }
}
