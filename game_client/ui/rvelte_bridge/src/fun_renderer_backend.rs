//! Pass-73 fun-renderer-backed packet consumer.
//!
//! Implements the rvelte-side
//! [`rvelte_fun_render_adapter::FunUiFramePacketConsumer`] trait
//! against fun-renderer's experimental
//! [`fun_renderer::ui::native_adapter::NativeUiAdapter`] (the
//! `native_ui_adapter` feature). Unlike the rvelte-side
//! [`fake::FakeRenderer2DCommandSink`], this consumer does **not**
//! walk the packet through the 14-method
//! [`Renderer2DCommandSink`] surface. The whole-packet contract
//! is exactly what fun-renderer wants:
//! [`NativeUiRendererDescriptors::from_packet`] consumes a
//! [`FunUiFramePacket`] and produces typed renderer descriptors in
//! one pass.
//!
//! Feature gate: `fun_renderer_backend`. With the gate off, the
//! bridge crate stays free of retired_engine / wgpu / dx12 / vulkan / metal /
//! swapchain dependencies. With the gate on, this module is
//! the *only* place fun-renderer enters the bridge.

use fun_renderer::component_api::RenderTextureAssetId;
use fun_renderer::ui::native_adapter::{
    NativeUiCompositeLayer, NativeUiRendererDescriptors, NativeUiResourceTable,
    RendererUiGlyphResource, RendererUiImageFormat, RendererUiImageResource, RendererUiSize,
};
use rvelte_fun_render_adapter::{
    FunRenderUiAdapterError, FunRenderUiCapabilityReport, FunRenderUiSubmitResult,
    FunUiFramePacketConsumer,
};
use rvelte_fun_ui_core::FunUiFramePacket;

/// Stable schema label for the fun-renderer-backed consumer
/// contract.
pub const FUN_RENDERER_PACKET_CONSUMER_SCHEMA: &str =
    "fun.product.rvelte_bridge.fun_renderer_backend.v1";

/// Numeric schema version for the fun-renderer-backed consumer
/// contract.
pub const FUN_RENDERER_PACKET_CONSUMER_SCHEMA_VERSION: u16 = 1;

/// Owns the fun-renderer-side resource table, the composite-layer
/// policy, the most recently produced descriptors, and the
/// capability report this consumer advertises.
///
/// One instance per game-client process. The bridge's
/// [`crate::ProductRvelteAdapter`] is generic over any
/// [`FunUiFramePacketConsumer`], so swapping the rvelte fake
/// renderer for this consumer is a one-line change at construction
/// time.
pub struct FunRendererPacketConsumer {
    capability: FunRenderUiCapabilityReport,
    resource_table: NativeUiResourceTable,
    composite_into_layer: NativeUiCompositeLayer,
    last_descriptors: Option<NativeUiRendererDescriptors>,
    submit_count: u32,
    auto_materialize_resources: bool,
}

impl FunRendererPacketConsumer {
    /// Constructs a consumer with the rvelte adapter's `full()`
    /// capability and the default `AfterScene` composite policy.
    /// Production callers can override either via
    /// [`Self::with_capability`] or [`Self::with_composite_layer`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            capability: FunRenderUiCapabilityReport::full(),
            resource_table: NativeUiResourceTable::new(),
            composite_into_layer: NativeUiCompositeLayer::AfterScene,
            last_descriptors: None,
            submit_count: 0,
            auto_materialize_resources: false,
        }
    }

    /// Sets the capability report this consumer advertises. The
    /// rvelte adapter consults this report before walking the
    /// packet, so trimming flags here causes the adapter to reject
    /// unsupported draws before they reach fun-renderer.
    #[must_use]
    pub fn with_capability(mut self, capability: FunRenderUiCapabilityReport) -> Self {
        self.capability = capability;
        self
    }

    /// Selects the composite layer fun-renderer should fold the UI
    /// into.
    #[must_use]
    pub const fn with_composite_layer(mut self, layer: NativeUiCompositeLayer) -> Self {
        self.composite_into_layer = layer;
        self
    }

    /// Enables deterministic placeholder materialization for resource
    /// deltas before validation. Product runtime callers use this so
    /// route packets can flow into the native UI descriptor path before
    /// the final atlas allocator lands; targeted tests keep it disabled
    /// to prove missing-resource rejection.
    #[must_use]
    pub const fn with_auto_materialize_resources(mut self, enabled: bool) -> Self {
        self.auto_materialize_resources = enabled;
        self
    }

    /// Returns a shared reference to the resource table fun-renderer
    /// uses to map logical IDs to renderer-owned resources.
    #[must_use]
    pub const fn resource_table(&self) -> &NativeUiResourceTable {
        &self.resource_table
    }

    /// Returns a mutable reference so the consumer's owner can record
    /// renderer-owned resources (e.g. when the resource registry
    /// hands back a freshly-allocated texture).
    pub const fn resource_table_mut(&mut self) -> &mut NativeUiResourceTable {
        &mut self.resource_table
    }

    /// Returns the most recently produced descriptors, when any.
    /// Useful for tooling that wants to inspect the lowered output.
    #[must_use]
    pub fn last_descriptors(&self) -> Option<&NativeUiRendererDescriptors> {
        self.last_descriptors.as_ref()
    }

    /// Returns the number of frames this consumer has translated.
    #[must_use]
    pub const fn submit_count(&self) -> u32 {
        self.submit_count
    }

    /// Returns the active composite-layer policy.
    #[must_use]
    pub const fn composite_into_layer(&self) -> NativeUiCompositeLayer {
        self.composite_into_layer
    }
}

impl Default for FunRendererPacketConsumer {
    fn default() -> Self {
        Self::new()
    }
}

impl FunUiFramePacketConsumer for FunRendererPacketConsumer {
    fn capability_report(&self) -> FunRenderUiCapabilityReport {
        self.capability.clone()
    }

    fn submit_frame(
        &mut self,
        frame: &FunUiFramePacket,
    ) -> Result<FunRenderUiSubmitResult, FunRenderUiAdapterError> {
        if self.auto_materialize_resources {
            self.materialize_resource_deltas(frame);
        }

        // The rvelte adapter has already run schema / packet
        // validate / capability-limit gates. Hand the packet to
        // fun-renderer's whole-packet lowering directly.
        let descriptors = NativeUiRendererDescriptors::from_packet(
            frame,
            &self.resource_table,
            self.composite_into_layer,
        );

        // fun-renderer surfaces validation diagnostics
        // (`missing_image_resources`, `missing_glyph_resources`,
        // `unsupported_path_lite_count`, etc.) inside the
        // descriptors. Lift them into the rvelte typed-error
        // surface so callers see a single rejection vocabulary.
        if !descriptors.passed_validation() {
            let validation = &descriptors.validation;
            if validation.schema_mismatch {
                return Err(FunRenderUiAdapterError::SchemaMismatch {
                    expected: rvelte_fun_ui_core::FUN_UI_PACKET_SCHEMA_VERSION,
                    observed: frame.schema_version,
                });
            }
            if validation.missing_glyph_resources > 0 {
                let missing_id = first_missing_glyph(frame, &self.resource_table);
                return Err(FunRenderUiAdapterError::MissingResource {
                    kind: "glyph",
                    id: missing_id,
                });
            }
            if validation.missing_image_resources > 0 {
                let missing_id = first_missing_image(frame, &self.resource_table);
                return Err(FunRenderUiAdapterError::MissingResource {
                    kind: "image",
                    id: missing_id,
                });
            }
            if validation.unsupported_path_lite_count > 0 {
                return Err(FunRenderUiAdapterError::UnsupportedPrimitive {
                    operation: "draw_path_lite",
                });
            }
            if validation.clip_stack_imbalance {
                return Err(FunRenderUiAdapterError::InvalidClipStack {
                    detail: "fun_renderer_clip_stack_imbalance",
                    clip_id: None,
                });
            }
            if validation.transform_stack_imbalance {
                return Err(FunRenderUiAdapterError::InvalidTransformStack {
                    detail: "fun_renderer_transform_stack_imbalance",
                });
            }
            if validation.opacity_stack_imbalance {
                return Err(FunRenderUiAdapterError::InvalidOpacityStack {
                    detail: "fun_renderer_opacity_stack_imbalance",
                });
            }
            return Err(FunRenderUiAdapterError::BackendRejected {
                code: "rvt.fun_render.adapter.fun_renderer_validation_failed",
            });
        }

        let layers_submitted = u32::try_from(descriptors.layer_summaries.len()).unwrap_or(u32::MAX);
        let draws_submitted = u32::try_from(descriptors.draw_commands.len()).unwrap_or(u32::MAX);
        let glyph_intents_uploaded = u32::try_from(
            frame
                .resource_deltas
                .iter()
                .filter(|delta| matches!(delta, rvelte_fun_ui_core::FunUiResourceDelta::Glyph(_)))
                .count(),
        )
        .unwrap_or(u32::MAX);
        let image_intents_uploaded = u32::try_from(
            frame
                .resource_deltas
                .iter()
                .filter(|delta| matches!(delta, rvelte_fun_ui_core::FunUiResourceDelta::Image(_)))
                .count(),
        )
        .unwrap_or(u32::MAX);
        let (clip_pushes, transform_pushes, opacity_pushes) = count_stack_pushes(frame);

        self.last_descriptors = Some(descriptors);
        self.submit_count = self.submit_count.saturating_add(1);

        Ok(FunRenderUiSubmitResult {
            layers_submitted,
            draws_submitted,
            glyph_intents_uploaded,
            image_intents_uploaded,
            clip_pushes,
            transform_pushes,
            opacity_pushes,
        })
    }
}

impl FunRendererPacketConsumer {
    fn materialize_resource_deltas(&mut self, frame: &FunUiFramePacket) {
        for delta in &frame.resource_deltas {
            match delta {
                rvelte_fun_ui_core::FunUiResourceDelta::Glyph(glyph) => {
                    if self.resource_table.glyph(glyph.glyph_run_id).is_none() {
                        self.resource_table.record_glyph(
                            glyph.glyph_run_id,
                            RendererUiGlyphResource {
                                atlas_texture: RenderTextureAssetId::default(),
                                atlas_size: RendererUiSize {
                                    width: 256,
                                    height: 256,
                                },
                            },
                        );
                    }
                }
                rvelte_fun_ui_core::FunUiResourceDelta::Image(image) => {
                    if self.resource_table.image(image.image_id).is_none() {
                        self.resource_table.record_image(
                            image.image_id,
                            RendererUiImageResource {
                                texture: RenderTextureAssetId::default(),
                                size: RendererUiSize::from_packet(image.size),
                                format: RendererUiImageFormat::from_packet(image.format),
                            },
                        );
                    }
                }
            }
        }
    }
}

fn first_missing_glyph(frame: &FunUiFramePacket, table: &NativeUiResourceTable) -> u64 {
    for layer in &frame.layers {
        for draw in &layer.draw_packets {
            if let rvelte_fun_ui_core::FunUiDrawOperation::DrawTextRun { glyph_run_id, .. } =
                &draw.operation
                && table.glyph(*glyph_run_id).is_none()
            {
                return glyph_run_id.get();
            }
        }
    }
    0
}

fn first_missing_image(frame: &FunUiFramePacket, table: &NativeUiResourceTable) -> u64 {
    for layer in &frame.layers {
        for draw in &layer.draw_packets {
            if let rvelte_fun_ui_core::FunUiDrawOperation::DrawImage { image_id, .. } =
                &draw.operation
                && table.image(*image_id).is_none()
            {
                return image_id.get();
            }
        }
    }
    0
}

fn count_stack_pushes(frame: &FunUiFramePacket) -> (u32, u32, u32) {
    let mut clip_pushes = 0u32;
    let mut transform_pushes = 0u32;
    let mut opacity_pushes = 0u32;
    for layer in &frame.layers {
        for draw in &layer.draw_packets {
            match draw.operation {
                rvelte_fun_ui_core::FunUiDrawOperation::PushClip { .. } => {
                    clip_pushes = clip_pushes.saturating_add(1);
                }
                rvelte_fun_ui_core::FunUiDrawOperation::PushTransform { .. } => {
                    transform_pushes = transform_pushes.saturating_add(1);
                }
                rvelte_fun_ui_core::FunUiDrawOperation::PushOpacity { .. } => {
                    opacity_pushes = opacity_pushes.saturating_add(1);
                }
                _ => {}
            }
        }
    }
    (clip_pushes, transform_pushes, opacity_pushes)
}
