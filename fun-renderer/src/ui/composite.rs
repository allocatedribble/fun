use crate::component_api::{
    RenderColor, UiColorSpace, UiCompositeOrder, UiDebugBorder, UiLayer, UiOpacity, UiTargetRect,
};
use crate::extraction::ExtractedUiSurface;
use crate::frame_graph::FrameGraphResourceType;

use super::native_ui::{RendererNativeUiAlphaMode, RendererNativeUiLayer};

pub const UI_COMPOSITE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiCompositeColorPolicy {
    SdrLinearScene,
    HdrScene,
    Hdr10Output,
}

impl UiCompositeColorPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SdrLinearScene => "sdr_linear_scene",
            Self::HdrScene => "hdr_scene",
            Self::Hdr10Output => "hdr10_output",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiCompositeColorConversion {
    None,
    SrgbToLinear,
    LinearToHdr,
    SdrToHdr10Pq,
}

impl UiCompositeColorConversion {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SrgbToLinear => "srgb_to_linear",
            Self::LinearToHdr => "linear_to_hdr",
            Self::SdrToHdr10Pq => "sdr_to_hdr10_pq",
        }
    }
}

#[must_use]
pub const fn select_color_conversion(
    surface_color_space: UiColorSpace,
    output_policy: UiCompositeColorPolicy,
) -> UiCompositeColorConversion {
    match (surface_color_space, output_policy) {
        (UiColorSpace::Linear, UiCompositeColorPolicy::SdrLinearScene)
        | (UiColorSpace::Hdr10, UiCompositeColorPolicy::Hdr10Output) => {
            UiCompositeColorConversion::None
        }
        (UiColorSpace::Srgb, UiCompositeColorPolicy::SdrLinearScene) => {
            UiCompositeColorConversion::SrgbToLinear
        }
        (UiColorSpace::Linear, UiCompositeColorPolicy::HdrScene) => {
            UiCompositeColorConversion::LinearToHdr
        }
        (UiColorSpace::Srgb, UiCompositeColorPolicy::HdrScene) => {
            UiCompositeColorConversion::LinearToHdr
        }
        (UiColorSpace::Srgb | UiColorSpace::Linear, UiCompositeColorPolicy::Hdr10Output) => {
            UiCompositeColorConversion::SdrToHdr10Pq
        }
        (UiColorSpace::Hdr10, UiCompositeColorPolicy::SdrLinearScene)
        | (UiColorSpace::Hdr10, UiCompositeColorPolicy::HdrScene) => {
            UiCompositeColorConversion::None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiCompositeLayerEntry {
    pub stable_id: crate::component_api::RenderStableId,
    pub layer: UiLayer,
    pub composite_order: UiCompositeOrder,
    pub opacity: UiOpacity,
    pub color_space: UiColorSpace,
    pub target_rect: UiTargetRect,
    pub debug_border: UiDebugBorder,
    pub alpha_mode: RendererNativeUiAlphaMode,
    pub source_resource_type: FrameGraphResourceType,
    pub color_conversion: UiCompositeColorConversion,
}

impl UiCompositeLayerEntry {
    #[must_use]
    pub fn from_extracted(
        extracted: ExtractedUiSurface,
        native_ui_layer: Option<RendererNativeUiLayer>,
        output_policy: UiCompositeColorPolicy,
    ) -> Self {
        let alpha_mode = native_ui_layer
            .map(|layer| layer.alpha_mode)
            .unwrap_or(RendererNativeUiAlphaMode::Premultiplied);
        let source_resource_type = native_ui_layer
            .map(|layer| layer.resource_type)
            .unwrap_or(FrameGraphResourceType::UiColorAlpha);
        Self {
            stable_id: extracted.stable_id,
            layer: extracted.layer,
            composite_order: extracted.composite_order,
            opacity: extracted.opacity,
            color_space: extracted.color_space,
            target_rect: extracted.target_rect,
            debug_border: extracted.debug_border,
            alpha_mode,
            source_resource_type,
            color_conversion: select_color_conversion(extracted.color_space, output_policy),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiCompositeContract {
    pub schema_version: u16,
    pub scene_input_resource: FrameGraphResourceType,
    pub ui_input_resource: FrameGraphResourceType,
    pub output_resource: FrameGraphResourceType,
}

impl UiCompositeContract {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: UI_COMPOSITE_SCHEMA_VERSION,
        scene_input_resource: FrameGraphResourceType::DisplayResolutionSceneColor,
        ui_input_resource: FrameGraphResourceType::UiColorAlpha,
        output_resource: FrameGraphResourceType::FinalComposedOutput,
    };
}

impl Default for UiCompositeContract {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiCompositeError {
    OpacityOutOfRange,
    InvalidTargetRect,
    DebugBorderUnsupportedThickness,
    UiLayerLimitExceeded,
}

impl UiCompositeError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpacityOutOfRange => "opacity_out_of_range",
            Self::InvalidTargetRect => "invalid_target_rect",
            Self::DebugBorderUnsupportedThickness => "debug_border_unsupported_thickness",
            Self::UiLayerLimitExceeded => "ui_layer_limit_exceeded",
        }
    }
}

pub const MAX_UI_COMPOSITE_LAYERS: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct UiCompositePlan {
    schema_version: u16,
    contract: UiCompositeContract,
    output_policy: UiCompositeColorPolicy,
    layers: Vec<UiCompositeLayerEntry>,
    debug_border_layer_count: u32,
    color_conversion_count: u32,
}

impl UiCompositePlan {
    #[must_use]
    pub fn empty(contract: UiCompositeContract, output_policy: UiCompositeColorPolicy) -> Self {
        Self {
            schema_version: UI_COMPOSITE_SCHEMA_VERSION,
            contract,
            output_policy,
            layers: Vec::new(),
            debug_border_layer_count: 0,
            color_conversion_count: 0,
        }
    }

    pub fn build(
        extracted_surfaces: &[ExtractedUiSurface],
        active_native_ui_layer: Option<RendererNativeUiLayer>,
        contract: UiCompositeContract,
        output_policy: UiCompositeColorPolicy,
    ) -> Result<Self, UiCompositeError> {
        if extracted_surfaces.len() > MAX_UI_COMPOSITE_LAYERS {
            return Err(UiCompositeError::UiLayerLimitExceeded);
        }
        let mut plan = Self::empty(contract, output_policy);
        for surface in extracted_surfaces {
            if !surface.target_rect.is_valid() {
                return Err(UiCompositeError::InvalidTargetRect);
            }
            if !(0.0..=1.0).contains(&surface.opacity.alpha) {
                return Err(UiCompositeError::OpacityOutOfRange);
            }
            if surface.debug_border.is_enabled() && !surface.debug_border.thickness_px.is_finite() {
                return Err(UiCompositeError::DebugBorderUnsupportedThickness);
            }
            let entry = UiCompositeLayerEntry::from_extracted(
                *surface,
                active_native_ui_layer,
                output_policy,
            );
            if entry.debug_border.is_enabled() {
                plan.debug_border_layer_count = plan.debug_border_layer_count.saturating_add(1);
            }
            if !matches!(entry.color_conversion, UiCompositeColorConversion::None) {
                plan.color_conversion_count = plan.color_conversion_count.saturating_add(1);
            }
            plan.layers.push(entry);
        }
        plan.layers.sort_by(|left, right| {
            (left.layer.layer, left.composite_order.order)
                .cmp(&(right.layer.layer, right.composite_order.order))
        });
        Ok(plan)
    }

    #[must_use]
    pub fn layers(&self) -> &[UiCompositeLayerEntry] {
        &self.layers
    }

    #[must_use]
    pub const fn contract(&self) -> UiCompositeContract {
        self.contract
    }

    #[must_use]
    pub const fn output_policy(&self) -> UiCompositeColorPolicy {
        self.output_policy
    }

    #[must_use]
    pub const fn debug_border_layer_count(&self) -> u32 {
        self.debug_border_layer_count
    }

    #[must_use]
    pub const fn color_conversion_count(&self) -> u32 {
        self.color_conversion_count
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn writes_final_target(&self) -> bool {
        matches!(
            self.contract.output_resource,
            FrameGraphResourceType::FinalComposedOutput
        )
    }

    #[must_use]
    pub fn debug_border_color_for(&self, entry: &UiCompositeLayerEntry) -> RenderColor {
        if entry.debug_border.is_enabled() {
            entry.debug_border.color
        } else {
            RenderColor::WHITE
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiCompositePassDiagnostics {
    pub schema_version: u16,
    pub layers_composited: u32,
    pub debug_border_layers: u32,
    pub color_conversion_count: u32,
    pub uses_premultiplied_alpha: bool,
    pub output_policy: Option<UiCompositeColorPolicy>,
}

impl UiCompositePassDiagnostics {
    #[must_use]
    pub fn from_plan(plan: &UiCompositePlan) -> Self {
        let uses_premultiplied_alpha = plan
            .layers
            .iter()
            .any(|entry| matches!(entry.alpha_mode, RendererNativeUiAlphaMode::Premultiplied));
        Self {
            schema_version: UI_COMPOSITE_SCHEMA_VERSION,
            layers_composited: u32::try_from(plan.layers.len()).unwrap_or(u32::MAX),
            debug_border_layers: plan.debug_border_layer_count,
            color_conversion_count: plan.color_conversion_count,
            uses_premultiplied_alpha,
            output_policy: Some(plan.output_policy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_api::{RenderExtent2d, RenderStableId, UiDebugBorderStyle, UiSurface};

    fn ui_surface(
        stable: u64,
        layer: u16,
        order: i16,
        color_space: UiColorSpace,
    ) -> ExtractedUiSurface {
        ExtractedUiSurface {
            stable_id: RenderStableId::new(stable),
            surface: UiSurface {
                surface_id: RenderStableId::new(stable),
                extent: RenderExtent2d::new(1920, 1080),
            },
            layer: UiLayer { layer },
            composite_order: UiCompositeOrder { order },
            opacity: UiOpacity { alpha: 1.0 },
            color_space,
            target_rect: UiTargetRect::FULL_WINDOW,
            debug_border: UiDebugBorder::OFF,
        }
    }

    #[test]
    fn composite_contract_reads_scene_color_and_ui_then_writes_final() {
        let contract = UiCompositeContract::default();

        assert_eq!(
            contract.scene_input_resource,
            FrameGraphResourceType::DisplayResolutionSceneColor
        );
        assert_eq!(
            contract.ui_input_resource,
            FrameGraphResourceType::UiColorAlpha
        );
        assert_eq!(
            contract.output_resource,
            FrameGraphResourceType::FinalComposedOutput
        );
    }

    #[test]
    fn composite_plan_orders_layers_by_layer_then_order_and_keeps_premultiplied_alpha() {
        let surfaces = [
            ui_surface(2, 4, 1, UiColorSpace::Linear),
            ui_surface(1, 2, 0, UiColorSpace::Srgb),
            ui_surface(3, 4, 0, UiColorSpace::Linear),
        ];
        let plan = UiCompositePlan::build(
            &surfaces,
            None,
            UiCompositeContract::default(),
            UiCompositeColorPolicy::SdrLinearScene,
        )
        .expect("plan");

        let stable_order: Vec<u64> = plan
            .layers()
            .iter()
            .map(|entry| entry.stable_id.0)
            .collect();
        assert_eq!(stable_order, [1, 3, 2]);
        assert!(plan.writes_final_target());
        let diagnostics = UiCompositePassDiagnostics::from_plan(&plan);
        assert!(diagnostics.uses_premultiplied_alpha);
        assert_eq!(diagnostics.layers_composited, 3);
    }

    #[test]
    fn composite_plan_records_color_conversion_for_srgb_into_linear_scene() {
        let surfaces = [ui_surface(7, 1, 0, UiColorSpace::Srgb)];
        let plan = UiCompositePlan::build(
            &surfaces,
            None,
            UiCompositeContract::default(),
            UiCompositeColorPolicy::SdrLinearScene,
        )
        .expect("plan");

        assert_eq!(plan.color_conversion_count(), 1);
        assert_eq!(
            plan.layers()[0].color_conversion,
            UiCompositeColorConversion::SrgbToLinear
        );
    }

    #[test]
    fn composite_plan_routes_sdr_into_hdr10_output_with_pq_conversion() {
        let surfaces = [
            ui_surface(8, 1, 0, UiColorSpace::Srgb),
            ui_surface(9, 1, 1, UiColorSpace::Linear),
            ui_surface(10, 1, 2, UiColorSpace::Hdr10),
        ];
        let plan = UiCompositePlan::build(
            &surfaces,
            None,
            UiCompositeContract::default(),
            UiCompositeColorPolicy::Hdr10Output,
        )
        .expect("plan");

        assert_eq!(plan.color_conversion_count(), 2);
        assert_eq!(
            plan.layers()[0].color_conversion,
            UiCompositeColorConversion::SdrToHdr10Pq
        );
        assert_eq!(
            plan.layers()[1].color_conversion,
            UiCompositeColorConversion::SdrToHdr10Pq
        );
        assert_eq!(
            plan.layers()[2].color_conversion,
            UiCompositeColorConversion::None
        );
    }

    #[test]
    fn composite_plan_rejects_invalid_target_rect_and_out_of_range_opacity() {
        let mut surface = ui_surface(11, 1, 0, UiColorSpace::Linear);
        surface.target_rect = UiTargetRect::new(0.0, 0.0, 0.0, 1.0);

        let invalid_rect = UiCompositePlan::build(
            &[surface],
            None,
            UiCompositeContract::default(),
            UiCompositeColorPolicy::SdrLinearScene,
        );
        assert_eq!(invalid_rect, Err(UiCompositeError::InvalidTargetRect));

        let mut bad_alpha = ui_surface(12, 1, 0, UiColorSpace::Linear);
        bad_alpha.opacity = UiOpacity { alpha: 1.5 };
        let result = UiCompositePlan::build(
            &[bad_alpha],
            None,
            UiCompositeContract::default(),
            UiCompositeColorPolicy::SdrLinearScene,
        );
        assert_eq!(result, Err(UiCompositeError::OpacityOutOfRange));
    }

    #[test]
    fn debug_border_propagates_into_layer_diagnostics() {
        let mut surface = ui_surface(13, 1, 0, UiColorSpace::Linear);
        surface.debug_border =
            UiDebugBorder::hairline(RenderColor::linear_rgba(1.0, 0.5, 0.0, 1.0));
        let plan = UiCompositePlan::build(
            &[surface],
            None,
            UiCompositeContract::default(),
            UiCompositeColorPolicy::SdrLinearScene,
        )
        .expect("plan");

        let diagnostics = UiCompositePassDiagnostics::from_plan(&plan);
        assert_eq!(diagnostics.debug_border_layers, 1);
        assert!(plan.layers()[0].debug_border.is_enabled());
        assert_eq!(
            plan.layers()[0].debug_border.style,
            UiDebugBorderStyle::Hairline
        );
        assert_eq!(
            plan.debug_border_color_for(&plan.layers()[0]).linear_rgba[0],
            1.0
        );
    }

    #[test]
    fn composite_plan_rejects_too_many_layers() {
        let mut surfaces = Vec::new();
        for i in 0..(MAX_UI_COMPOSITE_LAYERS as u64 + 1) {
            surfaces.push(ui_surface(i + 100, 1, i as i16, UiColorSpace::Linear));
        }
        let result = UiCompositePlan::build(
            &surfaces,
            None,
            UiCompositeContract::default(),
            UiCompositeColorPolicy::SdrLinearScene,
        );
        assert_eq!(result, Err(UiCompositeError::UiLayerLimitExceeded));
    }
}
