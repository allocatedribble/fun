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
pub struct NativeUiRoute(pub &'static str);

impl NativeUiRoute {
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
pub enum NativeUiSurfaceValidationError {
    ProductRequiresGpuOnly,
}

#[derive(Debug, Clone, FunFromTemplate, Component)]
pub struct NativeUiSurface {
    pub route: NativeUiRoute,
    pub layer: UiLayer,
    pub composition: UiCompositionPolicy,
    pub gpu_only: bool,
}

impl NativeUiSurface {
    pub const PRODUCT_DEFAULT: Self = Self {
        route: NativeUiRoute::ROOT,
        layer: UiLayer::Hud,
        composition: UiCompositionPolicy::FullWindow,
        gpu_only: true,
    };

    #[must_use]
    pub const fn product(route: NativeUiRoute, layer: UiLayer) -> Self {
        Self {
            route,
            layer,
            composition: UiCompositionPolicy::FullWindow,
            gpu_only: true,
        }
    }

    pub const fn validate_product(&self) -> Result<(), NativeUiSurfaceValidationError> {
        if self.gpu_only {
            Ok(())
        } else {
            Err(NativeUiSurfaceValidationError::ProductRequiresGpuOnly)
        }
    }
}

impl Default for NativeUiSurface {
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
pub fn product_scene_accepts_native_ui_surface(surface: &NativeUiSurface) -> bool {
    surface.validate_product().is_ok()
}
