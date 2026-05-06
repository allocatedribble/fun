use bevy_ecs::prelude::Component;

use crate::FunFromTemplate;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewportId(pub u32);

impl ViewportId {
    pub const PRIMARY: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CefRoute(pub &'static str);

impl CefRoute {
    pub const ROOT: Self = Self("/");
    pub const LAUNCHER: Self = Self("/launcher");
    pub const EDITOR: Self = Self("/editor");
    pub const HUD: Self = Self("/hud");
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiLayer {
    Launcher,
    Editor,
    #[default]
    Hud,
    Diagnostics,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiCompositionPolicy {
    #[default]
    FullWindow,
    ViewportOverlay,
    DockedPanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefSurfaceValidationError {
    ProductRequiresGpuOnly,
}

#[derive(Debug, Clone, FunFromTemplate, Component)]
pub struct CefSurface {
    pub route: CefRoute,
    pub layer: UiLayer,
    pub composition: UiCompositionPolicy,
    pub gpu_only: bool,
}

impl CefSurface {
    pub const PRODUCT_DEFAULT: Self = Self {
        route: CefRoute::ROOT,
        layer: UiLayer::Hud,
        composition: UiCompositionPolicy::FullWindow,
        gpu_only: true,
    };

    #[must_use]
    pub const fn product(route: CefRoute, layer: UiLayer) -> Self {
        Self {
            route,
            layer,
            composition: UiCompositionPolicy::FullWindow,
            gpu_only: true,
        }
    }

    pub const fn validate_product(&self) -> Result<(), CefSurfaceValidationError> {
        if self.gpu_only {
            Ok(())
        } else {
            Err(CefSurfaceValidationError::ProductRequiresGpuOnly)
        }
    }
}

impl Default for CefSurface {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiScalePolicy {
    NativePixels,
    #[default]
    DpiAware,
    ViewportRelative,
}

#[derive(Debug, Default, Clone, FunFromTemplate, Component)]
pub struct ViewportUiTarget {
    pub viewport: ViewportId,
    pub scale_policy: UiScalePolicy,
}

#[must_use]
pub fn product_scene_accepts_cef_surface(surface: &CefSurface) -> bool {
    surface.validate_product().is_ok()
}
