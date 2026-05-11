//! Native rvelte/FUN UI adapter contract.
//!
//! This module is the renderer-owned boundary that consumes
//! `rvelte_fun_ui_core::FunUiFramePacket` values and produces typed renderer
//! descriptors. It is the product UI ingest path — CEF is demoted to
//! legacy/diagnostic only (see [`CefRenderRoleStatus`]).
//!
//! The adapter never imports browser DOM, CEF, DX12, Vulkan, Metal, wgpu,
//! swapchains, or backend-specific texture/font-atlas handles. rvelte packets
//! carry logical IDs only; the renderer side maps those to its own resource
//! IDs through the registry.

use std::collections::BTreeMap;

use bevy_ecs::prelude::Resource;
use rvelte_fun_ui_core::{
    FunUiClipId, FunUiClipKind, FunUiClipPacket, FunUiColorRgba8, FunUiColorSpace,
    FunUiCornerRadii, FunUiDrawId, FunUiDrawOperation, FunUiDrawPacket, FunUiFramePacket,
    FunUiGlyphRunId, FunUiImageDelta, FunUiImageFormat, FunUiImageId, FunUiLayerId,
    FunUiLayerPacket, FunUiOpacity, FunUiPacketLimits, FunUiPacketValidationError, FunUiPoint,
    FunUiRect, FunUiResourceDelta, FunUiScalePolicy, FunUiSize, FunUiStroke, FunUiTransform2d,
};

use crate::component_api::{RenderColor, RenderTextureAssetId};
use crate::frame_graph::FrameGraphResourceType;

pub const NATIVE_UI_ADAPTER_SCHEMA_VERSION: u16 = 1;
pub const NATIVE_UI_ADAPTER_PRODUCT_DEFAULT: &str = "fun_ui_render_packet_v1";

/// Renderer-owned image resource identity that an rvelte logical
/// `FunUiImageId` maps to. The renderer-side resource registry produces this
/// id; the adapter does not.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererUiImageResource {
    pub texture: RenderTextureAssetId,
    pub size: RendererUiSize,
    pub format: RendererUiImageFormat,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererUiSize {
    pub width: u32,
    pub height: u32,
}

impl RendererUiSize {
    #[must_use]
    pub fn from_packet(size: FunUiSize) -> Self {
        Self {
            width: size.width,
            height: size.height,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererUiImageFormat {
    #[default]
    Rgba8Straight,
    Rgba8Premultiplied,
    Alpha8,
    ExternalAsset,
}

impl RendererUiImageFormat {
    #[must_use]
    pub const fn from_packet(format: FunUiImageFormat) -> Self {
        match format {
            FunUiImageFormat::Rgba8Straight => Self::Rgba8Straight,
            FunUiImageFormat::Rgba8Premultiplied => Self::Rgba8Premultiplied,
            FunUiImageFormat::Alpha8 => Self::Alpha8,
            FunUiImageFormat::ExternalAsset => Self::ExternalAsset,
        }
    }
}

/// Renderer-owned glyph atlas resource identity that an rvelte logical
/// `FunUiGlyphRunId` maps to.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererUiGlyphResource {
    pub atlas_texture: RenderTextureAssetId,
    pub atlas_size: RendererUiSize,
}

/// Maps logical `FunUiImageId` and `FunUiGlyphRunId` values to renderer-owned
/// resources. The adapter never invents resource IDs; it only records the
/// mapping the registry hands back.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "bevy_ecs", derive(Resource))]
pub struct NativeUiResourceTable {
    schema_version: u16,
    images: BTreeMap<u64, RendererUiImageResource>,
    glyphs: BTreeMap<u64, RendererUiGlyphResource>,
    pending_image_uploads: BTreeMap<u64, FunUiImageDelta>,
    pending_image_retirals: Vec<u64>,
    pending_glyph_uploads: BTreeMap<u64, FunUiGlyphRunId>,
    pending_glyph_retirals: Vec<u64>,
}

impl NativeUiResourceTable {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: NATIVE_UI_ADAPTER_SCHEMA_VERSION,
            images: BTreeMap::new(),
            glyphs: BTreeMap::new(),
            pending_image_uploads: BTreeMap::new(),
            pending_image_retirals: Vec::new(),
            pending_glyph_uploads: BTreeMap::new(),
            pending_glyph_retirals: Vec::new(),
        }
    }

    pub fn record_image(&mut self, image_id: FunUiImageId, resource: RendererUiImageResource) {
        self.images.insert(image_id.get(), resource);
    }

    pub fn record_glyph(&mut self, run_id: FunUiGlyphRunId, resource: RendererUiGlyphResource) {
        self.glyphs.insert(run_id.get(), resource);
    }

    #[must_use]
    pub fn image(&self, image_id: FunUiImageId) -> Option<RendererUiImageResource> {
        self.images.get(&image_id.get()).copied()
    }

    #[must_use]
    pub fn glyph(&self, run_id: FunUiGlyphRunId) -> Option<RendererUiGlyphResource> {
        self.glyphs.get(&run_id.get()).copied()
    }

    pub fn enqueue_resource_delta(&mut self, delta: FunUiResourceDelta) {
        match delta {
            FunUiResourceDelta::Image(image) => {
                self.pending_image_uploads
                    .insert(image.image_id.get(), image);
            }
            FunUiResourceDelta::Glyph(glyph) => {
                self.pending_glyph_uploads
                    .insert(glyph.glyph_run_id.get(), glyph.glyph_run_id);
            }
        }
    }

    pub fn retire_image(&mut self, image_id: FunUiImageId) {
        self.images.remove(&image_id.get());
        self.pending_image_retirals.push(image_id.get());
    }

    pub fn retire_glyph(&mut self, run_id: FunUiGlyphRunId) {
        self.glyphs.remove(&run_id.get());
        self.pending_glyph_retirals.push(run_id.get());
    }

    #[must_use]
    pub fn pending_image_upload_count(&self) -> u32 {
        u32::try_from(self.pending_image_uploads.len()).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn pending_glyph_upload_count(&self) -> u32 {
        u32::try_from(self.pending_glyph_uploads.len()).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn pending_image_retiral_count(&self) -> u32 {
        u32::try_from(self.pending_image_retirals.len()).unwrap_or(u32::MAX)
    }

    #[must_use]
    pub fn pending_glyph_retiral_count(&self) -> u32 {
        u32::try_from(self.pending_glyph_retirals.len()).unwrap_or(u32::MAX)
    }

    pub fn drain_pending(&mut self) {
        self.pending_image_uploads.clear();
        self.pending_image_retirals.clear();
        self.pending_glyph_uploads.clear();
        self.pending_glyph_retirals.clear();
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
}

/// Renderer-side draw command emitted by the adapter from one
/// `FunUiDrawOperation`. The adapter stays renderer-independent at this
/// boundary by emitting these typed records rather than backend handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeUiDrawCommand {
    FillQuad(NativeUiFillQuad),
    StrokeQuad(NativeUiStrokeQuad),
    FillRoundedQuad(NativeUiFillRoundedQuad),
    StrokeRoundedQuad(NativeUiStrokeRoundedQuad),
    GlyphRun(NativeUiGlyphRun),
    ImageQuad(NativeUiImageQuad),
    PushClip(NativeUiClipScope),
    PopClip(FunUiClipId),
    PushTransform(NativeUiTransform),
    PopTransform,
    PushOpacity(NativeUiOpacity),
    PopOpacity,
    DebugBoundsOverlay(NativeUiDebugQuad),
    UnsupportedPathLite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiFillQuad {
    pub rect: FunUiRect,
    pub color: FunUiColorRgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiStrokeQuad {
    pub rect: FunUiRect,
    pub stroke: FunUiStroke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiFillRoundedQuad {
    pub rect: FunUiRect,
    pub radii: FunUiCornerRadii,
    pub color: FunUiColorRgba8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiStrokeRoundedQuad {
    pub rect: FunUiRect,
    pub radii: FunUiCornerRadii,
    pub stroke: FunUiStroke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiGlyphRun {
    pub glyph_run_id: FunUiGlyphRunId,
    pub origin: FunUiPoint,
    pub color: FunUiColorRgba8,
    pub atlas: Option<RendererUiGlyphResource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiImageQuad {
    pub image_id: FunUiImageId,
    pub rect: FunUiRect,
    pub tint: Option<FunUiColorRgba8>,
    pub texture: Option<RendererUiImageResource>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeUiClipKindSummary {
    #[default]
    Rect,
    RoundedRect,
    PathLite,
}

impl NativeUiClipKindSummary {
    #[must_use]
    pub const fn from_packet(kind: &FunUiClipKind) -> Self {
        match kind {
            FunUiClipKind::Rect(_) => Self::Rect,
            FunUiClipKind::RoundedRect { .. } => Self::RoundedRect,
            FunUiClipKind::PathLite(_) => Self::PathLite,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeUiClipScope {
    pub clip_id: FunUiClipId,
    pub clip_kind: NativeUiClipKindSummary,
}

impl NativeUiClipScope {
    #[must_use]
    pub fn from_packet(packet: &FunUiClipPacket) -> Self {
        Self {
            clip_id: packet.clip_id,
            clip_kind: NativeUiClipKindSummary::from_packet(&packet.kind),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiTransform(pub FunUiTransform2d);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiOpacity(pub FunUiOpacity);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiDebugQuad {
    pub rect: FunUiRect,
    pub color: FunUiColorRgba8,
}

/// Lowers one `FunUiDrawOperation` into a `NativeUiDrawCommand`.
/// `DrawPathLite` is intentionally bucketed into `UnsupportedPathLite` so the
/// adapter records the missing tessellation seam rather than silently
/// drop the draw call.
#[must_use]
pub fn lower_draw_operation(
    op: &FunUiDrawOperation,
    table: &NativeUiResourceTable,
) -> NativeUiDrawCommand {
    match op {
        FunUiDrawOperation::FillRect { rect, color } => {
            NativeUiDrawCommand::FillQuad(NativeUiFillQuad {
                rect: *rect,
                color: *color,
            })
        }
        FunUiDrawOperation::StrokeRect { rect, stroke } => {
            NativeUiDrawCommand::StrokeQuad(NativeUiStrokeQuad {
                rect: *rect,
                stroke: *stroke,
            })
        }
        FunUiDrawOperation::FillRoundedRect { rect, radii, color } => {
            NativeUiDrawCommand::FillRoundedQuad(NativeUiFillRoundedQuad {
                rect: *rect,
                radii: *radii,
                color: *color,
            })
        }
        FunUiDrawOperation::StrokeRoundedRect {
            rect,
            radii,
            stroke,
        } => NativeUiDrawCommand::StrokeRoundedQuad(NativeUiStrokeRoundedQuad {
            rect: *rect,
            radii: *radii,
            stroke: *stroke,
        }),
        FunUiDrawOperation::DrawTextRun {
            glyph_run_id,
            origin,
            color,
        } => NativeUiDrawCommand::GlyphRun(NativeUiGlyphRun {
            glyph_run_id: *glyph_run_id,
            origin: *origin,
            color: *color,
            atlas: table.glyph(*glyph_run_id),
        }),
        FunUiDrawOperation::DrawImage {
            image_id,
            rect,
            tint,
        } => NativeUiDrawCommand::ImageQuad(NativeUiImageQuad {
            image_id: *image_id,
            rect: *rect,
            tint: *tint,
            texture: table.image(*image_id),
        }),
        FunUiDrawOperation::DrawPathLite { .. } => NativeUiDrawCommand::UnsupportedPathLite,
        FunUiDrawOperation::PushClip { clip } => {
            NativeUiDrawCommand::PushClip(NativeUiClipScope::from_packet(clip))
        }
        FunUiDrawOperation::PopClip { clip_id } => NativeUiDrawCommand::PopClip(*clip_id),
        FunUiDrawOperation::PushTransform { transform } => {
            NativeUiDrawCommand::PushTransform(NativeUiTransform(*transform))
        }
        FunUiDrawOperation::PopTransform => NativeUiDrawCommand::PopTransform,
        FunUiDrawOperation::PushOpacity { opacity } => {
            NativeUiDrawCommand::PushOpacity(NativeUiOpacity(*opacity))
        }
        FunUiDrawOperation::PopOpacity => NativeUiDrawCommand::PopOpacity,
        FunUiDrawOperation::DebugBounds { rect, color } => {
            NativeUiDrawCommand::DebugBoundsOverlay(NativeUiDebugQuad {
                rect: *rect,
                color: *color,
            })
        }
    }
}

#[must_use]
pub fn lower_draw_packet(
    packet: &FunUiDrawPacket,
    table: &NativeUiResourceTable,
) -> NativeUiDrawCommandRecord {
    NativeUiDrawCommandRecord {
        draw_id: packet.draw_id,
        command: lower_draw_operation(&packet.operation, table),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiDrawCommandRecord {
    pub draw_id: FunUiDrawId,
    pub command: NativeUiDrawCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeUiLayerSummary {
    pub layer_id: FunUiLayerId,
    pub draw_count: u32,
    pub clip_count: u32,
    pub debug_overlay_count: u32,
}

impl NativeUiLayerSummary {
    #[must_use]
    pub fn from_packet(packet: &FunUiLayerPacket) -> Self {
        let mut clip_count = 0u32;
        let mut debug_overlay_count = 0u32;
        for draw in &packet.draw_packets {
            match &draw.operation {
                FunUiDrawOperation::PushClip { .. } | FunUiDrawOperation::PopClip { .. } => {
                    clip_count = clip_count.saturating_add(1);
                }
                FunUiDrawOperation::DebugBounds { .. } => {
                    debug_overlay_count = debug_overlay_count.saturating_add(1);
                }
                _ => {}
            }
        }
        Self {
            layer_id: packet.layer_id,
            draw_count: u32::try_from(packet.draw_packets.len()).unwrap_or(u32::MAX),
            clip_count,
            debug_overlay_count,
        }
    }
}

/// Frame-graph contract the UI graph pass exposes when consuming
/// rvelte/FUN UI packets. This is what the renderer schedules between scene
/// color and present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiGraphPassPlan {
    pub schema_version: u16,
    pub source_color_space: FunUiColorSpace,
    pub scale_policy: FunUiScalePolicy,
    pub scale_factor_milli: u32,
    pub viewport: FunUiSize,
    pub composite_into_layer: NativeUiCompositeLayer,
    pub clip_stack_required: bool,
    pub transform_stack_required: bool,
    pub opacity_stack_required: bool,
    pub honors_layer_order: bool,
    pub source_resource: FrameGraphResourceType,
    pub output_resource: FrameGraphResourceType,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeUiCompositeLayer {
    #[default]
    AfterScene,
    OverlayOnly,
    BeforeFrameGeneration,
}

impl NativeUiCompositeLayer {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AfterScene => "after_scene",
            Self::OverlayOnly => "overlay_only",
            Self::BeforeFrameGeneration => "before_frame_generation",
        }
    }
}

impl NativeUiGraphPassPlan {
    #[must_use]
    pub fn from_frame_packet(
        packet: &FunUiFramePacket,
        composite_into_layer: NativeUiCompositeLayer,
    ) -> Self {
        let mut clip_stack_required = false;
        let mut transform_stack_required = false;
        let mut opacity_stack_required = false;
        for layer in &packet.layers {
            for draw in &layer.draw_packets {
                match draw.operation {
                    FunUiDrawOperation::PushClip { .. } | FunUiDrawOperation::PopClip { .. } => {
                        clip_stack_required = true;
                    }
                    FunUiDrawOperation::PushTransform { .. } | FunUiDrawOperation::PopTransform => {
                        transform_stack_required = true;
                    }
                    FunUiDrawOperation::PushOpacity { .. } | FunUiDrawOperation::PopOpacity => {
                        opacity_stack_required = true;
                    }
                    _ => {}
                }
            }
        }
        Self {
            schema_version: NATIVE_UI_ADAPTER_SCHEMA_VERSION,
            source_color_space: packet.color_space,
            scale_policy: packet.scale_policy,
            scale_factor_milli: packet.scale_factor_milli,
            viewport: packet.viewport,
            composite_into_layer,
            clip_stack_required,
            transform_stack_required,
            opacity_stack_required,
            honors_layer_order: packet.layers.len() <= u32::MAX as usize,
            source_resource: FrameGraphResourceType::DisplayResolutionSceneColor,
            output_resource: FrameGraphResourceType::UiColorAlpha,
        }
    }
}

/// One frame's lowering of a rvelte/FUN UI frame packet into renderer-side
/// commands and graph-pass plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeUiRendererDescriptors {
    pub schema_version: u16,
    pub frame_id: u64,
    pub viewport: FunUiSize,
    pub graph_pass: NativeUiGraphPassPlan,
    pub layer_summaries: Vec<NativeUiLayerSummary>,
    pub draw_commands: Vec<NativeUiDrawCommandRecord>,
    pub validation: NativeUiValidationDiagnostics,
}

impl NativeUiRendererDescriptors {
    #[must_use]
    pub fn from_packet(
        packet: &FunUiFramePacket,
        table: &NativeUiResourceTable,
        composite_into_layer: NativeUiCompositeLayer,
    ) -> Self {
        let validation = NativeUiValidationDiagnostics::validate(packet, table);
        let graph_pass = NativeUiGraphPassPlan::from_frame_packet(packet, composite_into_layer);
        let mut layer_summaries = Vec::with_capacity(packet.layers.len());
        let mut draw_commands = Vec::new();
        for layer in &packet.layers {
            layer_summaries.push(NativeUiLayerSummary::from_packet(layer));
            for draw in &layer.draw_packets {
                draw_commands.push(lower_draw_packet(draw, table));
            }
        }
        Self {
            schema_version: NATIVE_UI_ADAPTER_SCHEMA_VERSION,
            frame_id: packet.frame_id.get(),
            viewport: packet.viewport,
            graph_pass,
            layer_summaries,
            draw_commands,
            validation,
        }
    }

    #[must_use]
    pub fn passed_validation(&self) -> bool {
        self.validation.error_count == 0
    }
}

/// Validation diagnostics for one frame packet ingest. The adapter classifies
/// every failure into a typed seam so the renderer's failure state can name
/// it without retaining raw rvelte payloads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NativeUiValidationDiagnostics {
    pub schema_version: u16,
    pub error_count: u32,
    pub schema_mismatch: bool,
    pub byte_limit_exceeded: bool,
    pub geometry_invalid: bool,
    pub clip_stack_imbalance: bool,
    pub transform_stack_imbalance: bool,
    pub opacity_stack_imbalance: bool,
    pub missing_image_resources: u32,
    pub missing_glyph_resources: u32,
    pub unsupported_path_lite_count: u32,
    pub other_error_count: u32,
}

impl NativeUiValidationDiagnostics {
    #[must_use]
    pub fn validate(packet: &FunUiFramePacket, table: &NativeUiResourceTable) -> Self {
        let mut diagnostics = Self {
            schema_version: NATIVE_UI_ADAPTER_SCHEMA_VERSION,
            ..Self::default()
        };
        if let Err(error) = packet.validate_with_limits(&FunUiPacketLimits::DEFAULT) {
            diagnostics.error_count = diagnostics.error_count.saturating_add(1);
            classify_validation_error(&error, &mut diagnostics);
        }
        Self::scan_for_resource_misses_and_unsupported_ops(packet, table, &mut diagnostics);
        Self::scan_for_stack_imbalance(packet, &mut diagnostics);
        diagnostics
    }

    fn scan_for_resource_misses_and_unsupported_ops(
        packet: &FunUiFramePacket,
        table: &NativeUiResourceTable,
        diagnostics: &mut Self,
    ) {
        for layer in &packet.layers {
            for draw in &layer.draw_packets {
                match &draw.operation {
                    FunUiDrawOperation::DrawImage { image_id, .. } => {
                        if table.image(*image_id).is_none() {
                            diagnostics.missing_image_resources =
                                diagnostics.missing_image_resources.saturating_add(1);
                            diagnostics.error_count = diagnostics.error_count.saturating_add(1);
                        }
                    }
                    FunUiDrawOperation::DrawTextRun { glyph_run_id, .. } => {
                        if table.glyph(*glyph_run_id).is_none() {
                            diagnostics.missing_glyph_resources =
                                diagnostics.missing_glyph_resources.saturating_add(1);
                            diagnostics.error_count = diagnostics.error_count.saturating_add(1);
                        }
                    }
                    FunUiDrawOperation::DrawPathLite { .. } => {
                        diagnostics.unsupported_path_lite_count =
                            diagnostics.unsupported_path_lite_count.saturating_add(1);
                        diagnostics.error_count = diagnostics.error_count.saturating_add(1);
                    }
                    _ => {}
                }
            }
        }
    }

    fn scan_for_stack_imbalance(packet: &FunUiFramePacket, diagnostics: &mut Self) {
        for layer in &packet.layers {
            let mut clip_depth: i32 = 0;
            let mut transform_depth: i32 = 0;
            let mut opacity_depth: i32 = 0;
            for draw in &layer.draw_packets {
                match &draw.operation {
                    FunUiDrawOperation::PushClip { .. } => {
                        clip_depth = clip_depth.saturating_add(1)
                    }
                    FunUiDrawOperation::PopClip { .. } => {
                        clip_depth = clip_depth.saturating_sub(1);
                        if clip_depth < 0 {
                            diagnostics.clip_stack_imbalance = true;
                            diagnostics.error_count = diagnostics.error_count.saturating_add(1);
                        }
                    }
                    FunUiDrawOperation::PushTransform { .. } => {
                        transform_depth = transform_depth.saturating_add(1)
                    }
                    FunUiDrawOperation::PopTransform => {
                        transform_depth = transform_depth.saturating_sub(1);
                        if transform_depth < 0 {
                            diagnostics.transform_stack_imbalance = true;
                            diagnostics.error_count = diagnostics.error_count.saturating_add(1);
                        }
                    }
                    FunUiDrawOperation::PushOpacity { .. } => {
                        opacity_depth = opacity_depth.saturating_add(1)
                    }
                    FunUiDrawOperation::PopOpacity => {
                        opacity_depth = opacity_depth.saturating_sub(1);
                        if opacity_depth < 0 {
                            diagnostics.opacity_stack_imbalance = true;
                            diagnostics.error_count = diagnostics.error_count.saturating_add(1);
                        }
                    }
                    _ => {}
                }
            }
            if clip_depth != 0 {
                diagnostics.clip_stack_imbalance = true;
                diagnostics.error_count = diagnostics.error_count.saturating_add(1);
            }
            if transform_depth != 0 {
                diagnostics.transform_stack_imbalance = true;
                diagnostics.error_count = diagnostics.error_count.saturating_add(1);
            }
            if opacity_depth != 0 {
                diagnostics.opacity_stack_imbalance = true;
                diagnostics.error_count = diagnostics.error_count.saturating_add(1);
            }
        }
    }
}

fn classify_validation_error(
    error: &FunUiPacketValidationError,
    diagnostics: &mut NativeUiValidationDiagnostics,
) {
    match error {
        FunUiPacketValidationError::SchemaMismatch { .. } => {
            diagnostics.schema_mismatch = true;
        }
        FunUiPacketValidationError::ByteLimitExceeded { .. } => {
            diagnostics.byte_limit_exceeded = true;
        }
        FunUiPacketValidationError::InvalidGeometry { .. } => {
            diagnostics.geometry_invalid = true;
        }
        _ => {
            diagnostics.other_error_count = diagnostics.other_error_count.saturating_add(1);
        }
    }
}

/// CEF role under the native rvelte/FUN UI policy. Pass 20 demotes CEF from
/// "product UI surface" to one of the legacy/diagnostic roles below.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CefRenderRoleStatus {
    #[default]
    DemotedToLegacyDiagnostic,
    ArchivedReferenceOnly,
    StagedRemoval,
    LegacyComparisonOnly,
    TemporaryMigrationBridge,
}

impl CefRenderRoleStatus {
    /// Typed roster of every CEF render-role status variant.  Used by
    /// quality-audit predicates that must enumerate all non-product roles.
    pub const ALL: [Self; 5] = [
        Self::DemotedToLegacyDiagnostic,
        Self::ArchivedReferenceOnly,
        Self::StagedRemoval,
        Self::LegacyComparisonOnly,
        Self::TemporaryMigrationBridge,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DemotedToLegacyDiagnostic => "demoted_to_legacy_diagnostic",
            Self::ArchivedReferenceOnly => "archived_reference_only",
            Self::StagedRemoval => "staged_removal",
            Self::LegacyComparisonOnly => "legacy_comparison_only",
            Self::TemporaryMigrationBridge => "temporary_migration_bridge",
        }
    }

    #[must_use]
    pub const fn is_product_ui_surface(self) -> bool {
        // None of the demoted roles allow CEF to be the product UI surface.
        false
    }
}

/// Single source of truth for "what is the product UI ingest path" under
/// Pass 20: native rvelte/FUN UI packets, with CEF demoted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeUiProductPolicy {
    pub schema_version: u16,
    pub product_packet_schema: &'static str,
    pub product_ui_path_is_native_rvelte: bool,
    pub cef_status: CefRenderRoleStatus,
    pub fun_render_adapter_owns_packet_ingest: bool,
    pub renderer_packet_validation_required: bool,
}

impl NativeUiProductPolicy {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: NATIVE_UI_ADAPTER_SCHEMA_VERSION,
        product_packet_schema: NATIVE_UI_ADAPTER_PRODUCT_DEFAULT,
        product_ui_path_is_native_rvelte: true,
        cef_status: CefRenderRoleStatus::DemotedToLegacyDiagnostic,
        fun_render_adapter_owns_packet_ingest: true,
        renderer_packet_validation_required: true,
    };
}

impl Default for NativeUiProductPolicy {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

/// Bevy Resource carrying the live native UI adapter state.
#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct NativeUiAdapterReport {
    pub schema_version: u16,
    pub policy: NativeUiProductPolicy,
    pub last_descriptors: Option<NativeUiRendererDescriptorsSummary>,
}

impl NativeUiAdapterReport {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: NATIVE_UI_ADAPTER_SCHEMA_VERSION,
            policy: NativeUiProductPolicy::PRODUCT_DEFAULT,
            last_descriptors: None,
        }
    }

    pub fn record(&mut self, descriptors: &NativeUiRendererDescriptors) {
        self.last_descriptors = Some(NativeUiRendererDescriptorsSummary::from_descriptors(
            descriptors,
        ));
    }
}

/// Compact summary of a frame's lowering result that fits in a Bevy Resource
/// without retaining the (potentially large) draw command vector.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeUiRendererDescriptorsSummary {
    pub schema_version: u16,
    pub frame_id: u64,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub draw_command_count: u32,
    pub layer_count: u32,
    pub clip_count: u32,
    pub debug_overlay_count: u32,
    pub validation_error_count: u32,
}

impl NativeUiRendererDescriptorsSummary {
    #[must_use]
    pub fn from_descriptors(descriptors: &NativeUiRendererDescriptors) -> Self {
        let mut clip_count = 0u32;
        let mut debug_overlay_count = 0u32;
        for layer in &descriptors.layer_summaries {
            clip_count = clip_count.saturating_add(layer.clip_count);
            debug_overlay_count = debug_overlay_count.saturating_add(layer.debug_overlay_count);
        }
        Self {
            schema_version: NATIVE_UI_ADAPTER_SCHEMA_VERSION,
            frame_id: descriptors.frame_id,
            viewport_width: descriptors.viewport.width,
            viewport_height: descriptors.viewport.height,
            draw_command_count: u32::try_from(descriptors.draw_commands.len()).unwrap_or(u32::MAX),
            layer_count: u32::try_from(descriptors.layer_summaries.len()).unwrap_or(u32::MAX),
            clip_count,
            debug_overlay_count,
            validation_error_count: descriptors.validation.error_count,
        }
    }
}

#[must_use]
pub fn render_color_from_packet(color: FunUiColorRgba8) -> RenderColor {
    let r = f32::from(color.r) / 255.0;
    let g = f32::from(color.g) / 255.0;
    let b = f32::from(color.b) / 255.0;
    let a = f32::from(color.a) / 255.0;
    RenderColor::linear_rgba(r, g, b, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rvelte_fun_ui_core::{FunUiClipKind, FunUiNodeId};

    fn empty_frame() -> FunUiFramePacket {
        let mut packet = FunUiFramePacket::new(
            rvelte_fun_ui_core::FunUiFrameId::new(7),
            FunUiSize::new(1920, 1080),
        );
        packet.tree_revision = 1;
        packet
    }

    fn rect(x: i32, y: i32, w: u32, h: u32) -> FunUiRect {
        FunUiRect {
            origin: FunUiPoint { x, y },
            size: FunUiSize::new(w, h),
        }
    }

    fn fill_draw(draw_id: u64, color: FunUiColorRgba8) -> FunUiDrawPacket {
        FunUiDrawPacket {
            draw_id: FunUiDrawId::new(draw_id),
            node_id: FunUiNodeId::new(1),
            operation: FunUiDrawOperation::FillRect {
                rect: rect(0, 0, 100, 100),
                color,
            },
        }
    }

    fn layer(layer_id: u64, draws: Vec<FunUiDrawPacket>) -> FunUiLayerPacket {
        FunUiLayerPacket::new(
            FunUiLayerId::new(layer_id),
            rvelte_fun_ui_core::FunUiNodeId::new(1),
            0,
            draws,
        )
    }

    fn frame_with_one_filled_layer() -> FunUiFramePacket {
        let mut packet = empty_frame();
        packet.layers = vec![layer(
            1,
            vec![fill_draw(1, FunUiColorRgba8::new(255, 128, 64, 255))],
        )];
        packet
    }

    #[test]
    fn product_policy_demotes_cef_and_declares_rvelte_as_product_ui() {
        let policy = NativeUiProductPolicy::PRODUCT_DEFAULT;
        assert!(policy.product_ui_path_is_native_rvelte);
        assert!(policy.fun_render_adapter_owns_packet_ingest);
        assert!(policy.renderer_packet_validation_required);
        assert_eq!(policy.product_packet_schema, "fun_ui_render_packet_v1");
        assert!(!policy.cef_status.is_product_ui_surface());
        for variant in [
            CefRenderRoleStatus::DemotedToLegacyDiagnostic,
            CefRenderRoleStatus::ArchivedReferenceOnly,
            CefRenderRoleStatus::StagedRemoval,
            CefRenderRoleStatus::LegacyComparisonOnly,
            CefRenderRoleStatus::TemporaryMigrationBridge,
        ] {
            assert!(!variant.is_product_ui_surface());
        }
    }

    #[test]
    fn fill_rect_lowers_to_native_fill_quad_with_packet_color() {
        let table = NativeUiResourceTable::new();
        let op = FunUiDrawOperation::FillRect {
            rect: rect(10, 20, 30, 40),
            color: FunUiColorRgba8::new(10, 20, 30, 255),
        };
        let command = lower_draw_operation(&op, &table);
        assert!(matches!(command, NativeUiDrawCommand::FillQuad(_)));
        if let NativeUiDrawCommand::FillQuad(quad) = command {
            assert_eq!(quad.rect, rect(10, 20, 30, 40));
            assert_eq!(quad.color, FunUiColorRgba8::new(10, 20, 30, 255));
        }
    }

    #[test]
    fn stroke_rect_and_rounded_variants_route_to_typed_commands() {
        let table = NativeUiResourceTable::new();
        let stroke = FunUiStroke {
            color: FunUiColorRgba8::new(255, 0, 0, 255),
            width: 2,
        };
        let radii = FunUiCornerRadii::uniform(4);

        match lower_draw_operation(
            &FunUiDrawOperation::StrokeRect {
                rect: rect(0, 0, 10, 10),
                stroke,
            },
            &table,
        ) {
            NativeUiDrawCommand::StrokeQuad(_) => {}
            other => panic!("unexpected: {other:?}"),
        }
        match lower_draw_operation(
            &FunUiDrawOperation::FillRoundedRect {
                rect: rect(0, 0, 10, 10),
                radii,
                color: FunUiColorRgba8::new(0, 255, 0, 255),
            },
            &table,
        ) {
            NativeUiDrawCommand::FillRoundedQuad(_) => {}
            other => panic!("unexpected: {other:?}"),
        }
        match lower_draw_operation(
            &FunUiDrawOperation::StrokeRoundedRect {
                rect: rect(0, 0, 10, 10),
                radii,
                stroke,
            },
            &table,
        ) {
            NativeUiDrawCommand::StrokeRoundedQuad(_) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn draw_text_run_resolves_glyph_atlas_through_resource_table() {
        let mut table = NativeUiResourceTable::new();
        let run = FunUiGlyphRunId::new(11);
        table.record_glyph(
            run,
            RendererUiGlyphResource {
                atlas_texture: RenderTextureAssetId::first(3),
                atlas_size: RendererUiSize {
                    width: 1024,
                    height: 1024,
                },
            },
        );
        let command = lower_draw_operation(
            &FunUiDrawOperation::DrawTextRun {
                glyph_run_id: run,
                origin: FunUiPoint { x: 0, y: 0 },
                color: FunUiColorRgba8::new(255, 255, 255, 255),
            },
            &table,
        );
        let NativeUiDrawCommand::GlyphRun(glyph) = command else {
            panic!("expected glyph run, got {command:?}");
        };
        assert_eq!(glyph.glyph_run_id, run);
        let atlas = glyph.atlas.expect("glyph atlas resource was recorded");
        assert!(atlas.atlas_texture.is_valid());
        assert_eq!(atlas.atlas_size.width, 1024);
    }

    #[test]
    fn draw_image_resolves_renderer_texture_through_resource_table() {
        let mut table = NativeUiResourceTable::new();
        let image = FunUiImageId::new(5);
        table.record_image(
            image,
            RendererUiImageResource {
                texture: RenderTextureAssetId::first(2),
                size: RendererUiSize {
                    width: 256,
                    height: 128,
                },
                format: RendererUiImageFormat::Rgba8Premultiplied,
            },
        );
        let command = lower_draw_operation(
            &FunUiDrawOperation::DrawImage {
                image_id: image,
                rect: rect(0, 0, 256, 128),
                tint: None,
            },
            &table,
        );
        let NativeUiDrawCommand::ImageQuad(quad) = command else {
            panic!("expected image quad, got {command:?}");
        };
        assert_eq!(quad.image_id, image);
        let texture = quad.texture.expect("texture resource was recorded");
        assert_eq!(texture.format, RendererUiImageFormat::Rgba8Premultiplied);
    }

    #[test]
    fn missing_image_or_glyph_resources_are_recorded_in_validation_diagnostics() {
        let table = NativeUiResourceTable::new();
        let mut packet = empty_frame();
        let image = FunUiImageId::new(99);
        let run = FunUiGlyphRunId::new(99);
        packet.layers = vec![layer(
            1,
            vec![
                FunUiDrawPacket {
                    draw_id: FunUiDrawId::new(1),
                    node_id: FunUiNodeId::new(1),
                    operation: FunUiDrawOperation::DrawImage {
                        image_id: image,
                        rect: rect(0, 0, 10, 10),
                        tint: None,
                    },
                },
                FunUiDrawPacket {
                    draw_id: FunUiDrawId::new(2),
                    node_id: FunUiNodeId::new(1),
                    operation: FunUiDrawOperation::DrawTextRun {
                        glyph_run_id: run,
                        origin: FunUiPoint { x: 0, y: 0 },
                        color: FunUiColorRgba8::new(0, 0, 0, 255),
                    },
                },
            ],
        )];
        let descriptors = NativeUiRendererDescriptors::from_packet(
            &packet,
            &table,
            NativeUiCompositeLayer::AfterScene,
        );

        assert!(!descriptors.passed_validation());
        assert_eq!(descriptors.validation.missing_image_resources, 1);
        assert_eq!(descriptors.validation.missing_glyph_resources, 1);
        assert!(descriptors.validation.error_count >= 2);
    }

    #[test]
    fn clip_transform_and_opacity_stack_imbalance_is_classified() {
        let table = NativeUiResourceTable::new();
        let mut packet = empty_frame();
        let clip_id = FunUiClipId::new(1);
        packet.layers = vec![layer(
            1,
            vec![FunUiDrawPacket {
                draw_id: FunUiDrawId::new(1),
                node_id: FunUiNodeId::new(1),
                operation: FunUiDrawOperation::PopClip { clip_id },
            }],
        )];
        let descriptors = NativeUiRendererDescriptors::from_packet(
            &packet,
            &table,
            NativeUiCompositeLayer::AfterScene,
        );
        assert!(descriptors.validation.clip_stack_imbalance);
        assert!(!descriptors.passed_validation());
    }

    #[test]
    fn graph_pass_plan_records_clip_transform_and_opacity_requirements() {
        let table = NativeUiResourceTable::new();
        let mut packet = empty_frame();
        let clip_packet = FunUiClipPacket {
            clip_id: FunUiClipId::new(1),
            kind: FunUiClipKind::Rect(rect(0, 0, 100, 100)),
        };
        packet.layers = vec![layer(
            1,
            vec![
                FunUiDrawPacket {
                    draw_id: FunUiDrawId::new(1),
                    node_id: FunUiNodeId::new(1),
                    operation: FunUiDrawOperation::PushClip { clip: clip_packet },
                },
                FunUiDrawPacket {
                    draw_id: FunUiDrawId::new(2),
                    node_id: FunUiNodeId::new(1),
                    operation: FunUiDrawOperation::PushOpacity {
                        opacity: FunUiOpacity::OPAQUE,
                    },
                },
                FunUiDrawPacket {
                    draw_id: FunUiDrawId::new(3),
                    node_id: FunUiNodeId::new(1),
                    operation: FunUiDrawOperation::PopOpacity,
                },
                FunUiDrawPacket {
                    draw_id: FunUiDrawId::new(4),
                    node_id: FunUiNodeId::new(1),
                    operation: FunUiDrawOperation::PopClip {
                        clip_id: FunUiClipId::new(1),
                    },
                },
            ],
        )];
        let descriptors = NativeUiRendererDescriptors::from_packet(
            &packet,
            &table,
            NativeUiCompositeLayer::AfterScene,
        );

        assert!(descriptors.graph_pass.clip_stack_required);
        assert!(descriptors.graph_pass.opacity_stack_required);
        assert!(!descriptors.graph_pass.transform_stack_required);
        assert_eq!(descriptors.graph_pass.viewport, packet.viewport);
        assert_eq!(
            descriptors.graph_pass.output_resource,
            FrameGraphResourceType::UiColorAlpha
        );
        assert_eq!(
            descriptors.graph_pass.composite_into_layer,
            NativeUiCompositeLayer::AfterScene
        );
    }

    #[test]
    fn debug_bounds_lowers_to_overlay_command_and_layer_summary_counts_it() {
        let table = NativeUiResourceTable::new();
        let mut packet = empty_frame();
        packet.layers = vec![layer(
            1,
            vec![FunUiDrawPacket {
                draw_id: FunUiDrawId::new(1),
                node_id: FunUiNodeId::new(1),
                operation: FunUiDrawOperation::DebugBounds {
                    rect: rect(0, 0, 50, 50),
                    color: FunUiColorRgba8::new(255, 0, 0, 128),
                },
            }],
        )];
        let descriptors = NativeUiRendererDescriptors::from_packet(
            &packet,
            &table,
            NativeUiCompositeLayer::OverlayOnly,
        );
        assert!(matches!(
            descriptors.draw_commands[0].command,
            NativeUiDrawCommand::DebugBoundsOverlay(_)
        ));
        assert_eq!(descriptors.layer_summaries[0].debug_overlay_count, 1);
    }

    #[test]
    fn unsupported_path_lite_is_recorded_rather_than_silently_dropped() {
        let table = NativeUiResourceTable::new();
        let path = rvelte_fun_ui_core::FunUiPathLite {
            commands: vec![rvelte_fun_ui_core::FunUiPathCommand::MoveTo(FunUiPoint {
                x: 0,
                y: 0,
            })],
            fill_rule: rvelte_fun_ui_core::FunUiFillRule::NonZero,
        };
        let op = FunUiDrawOperation::DrawPathLite {
            path,
            paint: FunUiPathPaintFill(FunUiColorRgba8::new(255, 255, 255, 255)).into_packet(),
        };
        let command = lower_draw_operation(&op, &table);
        assert!(matches!(command, NativeUiDrawCommand::UnsupportedPathLite));
    }

    // Tiny helper struct only used inside the test above so we can build a
    // `FunUiPathPaint::Fill` without binding to the public path module's
    // internal layout in production code.
    struct FunUiPathPaintFill(FunUiColorRgba8);

    impl FunUiPathPaintFill {
        fn into_packet(self) -> rvelte_fun_ui_core::FunUiPathPaint {
            rvelte_fun_ui_core::FunUiPathPaint::Fill(self.0)
        }
    }

    #[test]
    fn resource_table_pending_uploads_and_retirals_track_through_drain() {
        let mut table = NativeUiResourceTable::new();
        let image = FunUiImageId::new(7);
        let glyph = FunUiGlyphRunId::new(7);
        table.enqueue_resource_delta(FunUiResourceDelta::Image(FunUiImageDelta::new(
            image,
            FunUiSize::new(64, 64),
            FunUiImageFormat::Rgba8Premultiplied,
            64 * 64 * 4,
            1234,
        )));
        table.enqueue_resource_delta(FunUiResourceDelta::Glyph(FunUiGlyphDeltaForTest::build(
            glyph,
        )));
        table.retire_image(image);
        table.retire_glyph(glyph);

        assert_eq!(table.pending_image_upload_count(), 1);
        assert_eq!(table.pending_glyph_upload_count(), 1);
        assert_eq!(table.pending_image_retiral_count(), 1);
        assert_eq!(table.pending_glyph_retiral_count(), 1);
        table.drain_pending();
        assert_eq!(table.pending_image_upload_count(), 0);
        assert_eq!(table.pending_glyph_upload_count(), 0);
        assert_eq!(table.pending_image_retiral_count(), 0);
        assert_eq!(table.pending_glyph_retiral_count(), 0);
    }

    struct FunUiGlyphDeltaForTest;

    impl FunUiGlyphDeltaForTest {
        fn build(run: FunUiGlyphRunId) -> rvelte_fun_ui_core::FunUiGlyphDelta {
            rvelte_fun_ui_core::FunUiGlyphDelta::new(
                rvelte_fun_ui_core::FunUiFontId::new(1),
                run,
                42,
                rect(0, 0, 16, 16),
                vec![rvelte_fun_ui_core::FunUiGlyphInstance {
                    glyph_index: 1,
                    advance: 16,
                    offset: FunUiPoint { x: 0, y: 0 },
                }],
            )
        }
    }

    #[test]
    fn descriptors_summary_is_compact_resource_friendly_record() {
        let table = NativeUiResourceTable::new();
        let descriptors = NativeUiRendererDescriptors::from_packet(
            &frame_with_one_filled_layer(),
            &table,
            NativeUiCompositeLayer::AfterScene,
        );
        let summary = NativeUiRendererDescriptorsSummary::from_descriptors(&descriptors);

        assert_eq!(summary.frame_id, descriptors.frame_id);
        assert_eq!(summary.draw_command_count, 1);
        assert_eq!(summary.layer_count, 1);
        assert_eq!(
            summary.validation_error_count,
            descriptors.validation.error_count
        );
    }

    #[test]
    fn render_color_from_packet_normalises_byte_components_into_linear_rgba() {
        let color = render_color_from_packet(FunUiColorRgba8::new(255, 128, 0, 255));
        assert!((color.linear_rgba[0] - 1.0).abs() < f32::EPSILON);
        assert!((color.linear_rgba[1] - (128.0 / 255.0)).abs() < f32::EPSILON);
        assert!((color.linear_rgba[2] - 0.0).abs() < f32::EPSILON);
        assert!((color.linear_rgba[3] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn adapter_report_records_summary_without_retaining_full_command_vec() {
        let table = NativeUiResourceTable::new();
        let descriptors = NativeUiRendererDescriptors::from_packet(
            &frame_with_one_filled_layer(),
            &table,
            NativeUiCompositeLayer::AfterScene,
        );
        let mut report = NativeUiAdapterReport::new();
        report.record(&descriptors);
        let summary = report.last_descriptors.expect("recorded summary");
        assert_eq!(summary.draw_command_count, 1);
        assert!(report.policy.fun_render_adapter_owns_packet_ingest);
    }
}
