//! Whole-packet rvelte/FUN UI consumer for fun-renderer.
//!
//! This is the renderer-owned ingestion path for `FunUiFramePacket`. The
//! compatibility command-sink walker remains in `rvelte-fun-render-adapter` for
//! fake renderers, golden traces, and minimal backends; this module keeps the
//! preferred path packet-shaped all the way into fun-renderer descriptors.

use std::collections::BTreeMap;

use rvelte_core::{SourceId, SourceSpan};
use rvelte_fun_render_adapter::{
    FUN_RENDER_UI_ADAPTER_SCHEMA_VERSION, FunRenderUiAdapterError, FunRenderUiCapabilityReport,
    FunRenderUiColorSpaceClass, FunRenderUiImageDimensionClass, FunRenderUiMemoryPressureClass,
    FunRenderUiSubmitResult, FunUiFramePacketConsumer,
};
use rvelte_fun_ui_core::{
    FUN_UI_PACKET_SCHEMA_VERSION, FunUiColorSpace, FunUiDrawOperation, FunUiFramePacket,
    FunUiGlyphRunId, FunUiImageDelta, FunUiImageId, FunUiLayerPacket, FunUiPacketLimits,
    FunUiPacketValidationError, FunUiResourceDelta, FunUiSize,
};

use crate::backend::{Renderer, RendererBackend};
use crate::component_api::RenderTextureAssetId;
use crate::ui::native_adapter::{
    NativeUiCompositeLayer, NativeUiRendererDescriptors, NativeUiRendererDescriptorsSummary,
    NativeUiResourceTable, RendererUiGlyphResource, RendererUiImageFormat, RendererUiImageResource,
    RendererUiSize,
};

/// The concrete renderer type accepted by the packet consumer.
pub type FunRenderer<B> = Renderer<B>;

/// Renderer capability report used by the rvelte packet boundary.
pub type RendererCaps = FunRenderUiCapabilityReport;

/// Successful whole-packet submission report.
pub type SubmitReport = FunRendererUiSubmitReport;

/// Typed whole-packet submission error.
pub type SubmitError = FunRendererUiSubmitError;

/// Stable resource identifier inside a source module.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceId(pub u64);

impl ResourceId {
    /// Creates a resource ID.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric ID.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable renderer cache key for UI resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceCacheKey {
    /// Source module that declared the resource.
    pub source_id: SourceId,
    /// Logical resource inside the source module.
    pub resource_id: ResourceId,
    /// Redacted content hash or semantic content digest.
    pub content_hash: u64,
    /// Hash of renderer capabilities that affect materialization.
    pub renderer_caps_hash: u64,
}

/// Generic handle for renderer-owned non-texture UI cache entries.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UiRendererCacheHandle {
    /// Dense renderer cache slot.
    pub slot: u32,
    /// Slot generation.
    pub generation: u32,
}

impl UiRendererCacheHandle {
    #[must_use]
    const fn first(slot: u32) -> Self {
        Self {
            slot,
            generation: 1,
        }
    }
}

/// Renderer-owned vector/path handle.
pub type UiRendererVectorPathHandle = UiRendererCacheHandle;
/// Renderer-owned gradient handle.
pub type UiRendererGradientHandle = UiRendererCacheHandle;
/// Renderer-owned nine-patch or border handle.
pub type UiRendererNinePatchBorderHandle = UiRendererCacheHandle;
/// Renderer-owned retained layer/render-target handle.
pub type UiRendererLayerHandle = UiRendererCacheHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UiRendererLayerCacheEntry {
    handle: UiRendererLayerHandle,
    content_hash: u64,
}

/// Stable renderer-side cache for packet resources and retained layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiRendererResourceCache {
    source_id: SourceId,
    table: NativeUiResourceTable,
    image_atlas_handles: BTreeMap<ResourceCacheKey, RendererUiImageResource>,
    glyph_atlas_handles: BTreeMap<ResourceCacheKey, RendererUiGlyphResource>,
    vector_path_handles: BTreeMap<ResourceCacheKey, UiRendererVectorPathHandle>,
    gradient_handles: BTreeMap<ResourceCacheKey, UiRendererGradientHandle>,
    nine_patch_border_handles: BTreeMap<ResourceCacheKey, UiRendererNinePatchBorderHandle>,
    layer_handles: BTreeMap<ResourceCacheKey, UiRendererLayerHandle>,
    logical_images: BTreeMap<u64, ResourceCacheKey>,
    logical_glyphs: BTreeMap<u64, ResourceCacheKey>,
    layers_by_id: BTreeMap<u64, UiRendererLayerCacheEntry>,
    last_valid_frame: Option<NativeUiRendererDescriptorsSummary>,
    last_descriptors: Option<NativeUiRendererDescriptors>,
}

impl UiRendererResourceCache {
    /// Creates an empty renderer UI resource cache.
    #[must_use]
    pub fn new(source_id: SourceId) -> Self {
        Self {
            source_id,
            table: NativeUiResourceTable::new(),
            image_atlas_handles: BTreeMap::new(),
            glyph_atlas_handles: BTreeMap::new(),
            vector_path_handles: BTreeMap::new(),
            gradient_handles: BTreeMap::new(),
            nine_patch_border_handles: BTreeMap::new(),
            layer_handles: BTreeMap::new(),
            logical_images: BTreeMap::new(),
            logical_glyphs: BTreeMap::new(),
            layers_by_id: BTreeMap::new(),
            last_valid_frame: None,
            last_descriptors: None,
        }
    }

    /// Returns the source module used by cache keys.
    #[must_use]
    pub const fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// Updates the cache source module for subsequent resource deltas.
    pub fn set_source_id(&mut self, source_id: SourceId) {
        self.source_id = source_id;
    }

    /// Returns the renderer resource table consumed by native descriptors.
    #[must_use]
    pub const fn resource_table(&self) -> &NativeUiResourceTable {
        &self.table
    }

    /// Returns the mutable renderer resource table.
    pub const fn resource_table_mut(&mut self) -> &mut NativeUiResourceTable {
        &mut self.table
    }

    /// Returns the most recent valid descriptor summary.
    #[must_use]
    pub const fn last_valid_frame(&self) -> Option<NativeUiRendererDescriptorsSummary> {
        self.last_valid_frame
    }

    /// Returns the most recent full descriptor set.
    #[must_use]
    pub fn last_descriptors(&self) -> Option<&NativeUiRendererDescriptors> {
        self.last_descriptors.as_ref()
    }

    /// Number of cached image resources.
    #[must_use]
    pub fn image_resource_count(&self) -> usize {
        self.image_atlas_handles.len()
    }

    /// Number of cached glyph resources.
    #[must_use]
    pub fn glyph_resource_count(&self) -> usize {
        self.glyph_atlas_handles.len()
    }

    /// Number of cached retained layers.
    #[must_use]
    pub fn layer_resource_count(&self) -> usize {
        self.layers_by_id.len()
    }

    fn record_last_valid(&mut self, descriptors: NativeUiRendererDescriptors) {
        self.last_valid_frame = Some(NativeUiRendererDescriptorsSummary::from_descriptors(
            &descriptors,
        ));
        self.last_descriptors = Some(descriptors);
    }

    fn apply_resource_deltas(
        &mut self,
        packet: &FunUiFramePacket,
        caps_hash: u64,
    ) -> ResourceDeltaApplyReport {
        let mut report = ResourceDeltaApplyReport::default();
        for delta in &packet.resource_deltas {
            self.table.enqueue_resource_delta(delta.clone());
            match delta {
                FunUiResourceDelta::Image(image) => {
                    let key = ResourceCacheKey {
                        source_id: self.source_id,
                        resource_id: ResourceId::new(image.image_id.get()),
                        content_hash: image.content_digest,
                        renderer_caps_hash: caps_hash,
                    };
                    let resource = *self
                        .image_atlas_handles
                        .entry(key)
                        .or_insert_with(|| image_resource_for_key(*image, key));
                    self.logical_images.insert(image.image_id.get(), key);
                    self.table.record_image(image.image_id, resource);
                    report.image_deltas_applied = report.image_deltas_applied.saturating_add(1);
                }
                FunUiResourceDelta::Glyph(glyph) => {
                    let key = ResourceCacheKey {
                        source_id: self.source_id,
                        resource_id: ResourceId::new(glyph.glyph_run_id.get()),
                        content_hash: glyph_content_hash(glyph.source_text_digest, glyph),
                        renderer_caps_hash: caps_hash,
                    };
                    let resource = *self
                        .glyph_atlas_handles
                        .entry(key)
                        .or_insert_with(|| glyph_resource_for_key(glyph.bounds.size, key));
                    self.logical_glyphs.insert(glyph.glyph_run_id.get(), key);
                    self.table.record_glyph(glyph.glyph_run_id, resource);
                    report.glyph_deltas_applied = report.glyph_deltas_applied.saturating_add(1);
                }
            }
        }
        report
    }

    fn ensure_draw_resources(
        &mut self,
        packet: &FunUiFramePacket,
        caps_hash: u64,
        policy: UiPacketSubmitPolicy,
        caps: &RendererCaps,
    ) -> Result<(), SubmitError> {
        for layer in &packet.layers {
            for draw in &layer.draw_packets {
                match &draw.operation {
                    FunUiDrawOperation::DrawImage { image_id, .. } => {
                        if self.table.image(*image_id).is_none() {
                            if policy.allow_missing_resource_placeholder {
                                self.record_placeholder_image(*image_id, caps_hash);
                            } else {
                                return Err(SubmitError::ResourceDeclined {
                                    kind: "image",
                                    resource_id: image_id.get(),
                                });
                            }
                        }
                    }
                    FunUiDrawOperation::DrawTextRun { glyph_run_id, .. } => {
                        if self.table.glyph(*glyph_run_id).is_none() {
                            if policy.allow_missing_resource_placeholder {
                                self.record_placeholder_glyph(*glyph_run_id, caps_hash);
                            } else {
                                return Err(SubmitError::ResourceDeclined {
                                    kind: "glyph",
                                    resource_id: glyph_run_id.get(),
                                });
                            }
                        }
                    }
                    FunUiDrawOperation::DrawPathLite { .. } => {
                        if !caps.supports_path_lite || !policy.allow_unsupported_op_degradation {
                            return Err(SubmitError::UnsupportedOp {
                                operation: "draw_path_lite",
                            });
                        }
                        let key = ResourceCacheKey {
                            source_id: self.source_id,
                            resource_id: ResourceId::new(draw.draw_id.get()),
                            content_hash: hash_draw_operation(&draw.operation),
                            renderer_caps_hash: caps_hash,
                        };
                        self.vector_path_handles.entry(key).or_insert_with(|| {
                            UiRendererVectorPathHandle::first(slot_from_hash(key.content_hash))
                        });
                    }
                    FunUiDrawOperation::StrokeRect { .. }
                    | FunUiDrawOperation::StrokeRoundedRect { .. } => {
                        let key = ResourceCacheKey {
                            source_id: self.source_id,
                            resource_id: ResourceId::new(draw.draw_id.get()),
                            content_hash: hash_draw_operation(&draw.operation),
                            renderer_caps_hash: caps_hash,
                        };
                        self.nine_patch_border_handles
                            .entry(key)
                            .or_insert_with(|| {
                                UiRendererNinePatchBorderHandle::first(slot_from_hash(
                                    key.content_hash,
                                ))
                            });
                    }
                    FunUiDrawOperation::FillRect { .. }
                    | FunUiDrawOperation::FillRoundedRect { .. }
                    | FunUiDrawOperation::PushClip { .. }
                    | FunUiDrawOperation::PopClip { .. }
                    | FunUiDrawOperation::PushTransform { .. }
                    | FunUiDrawOperation::PopTransform
                    | FunUiDrawOperation::PushOpacity { .. }
                    | FunUiDrawOperation::PopOpacity
                    | FunUiDrawOperation::DebugBounds { .. } => {}
                }
            }
        }
        Ok(())
    }

    fn record_placeholder_image(&mut self, image_id: FunUiImageId, caps_hash: u64) {
        let key = ResourceCacheKey {
            source_id: self.source_id,
            resource_id: ResourceId::new(image_id.get()),
            content_hash: stable_hash_pair(0xf00d_fa11_bacc_0001, image_id.get()),
            renderer_caps_hash: caps_hash,
        };
        let resource =
            *self
                .image_atlas_handles
                .entry(key)
                .or_insert_with(|| RendererUiImageResource {
                    texture: RenderTextureAssetId::first(slot_from_hash(key.content_hash)),
                    size: RendererUiSize {
                        width: 16,
                        height: 16,
                    },
                    format: RendererUiImageFormat::Rgba8Premultiplied,
                });
        self.logical_images.insert(image_id.get(), key);
        self.table.record_image(image_id, resource);
    }

    fn record_placeholder_glyph(&mut self, glyph_run_id: FunUiGlyphRunId, caps_hash: u64) {
        let key = ResourceCacheKey {
            source_id: self.source_id,
            resource_id: ResourceId::new(glyph_run_id.get()),
            content_hash: stable_hash_pair(0xf00d_fa11_bacc_0002, glyph_run_id.get()),
            renderer_caps_hash: caps_hash,
        };
        let resource =
            *self
                .glyph_atlas_handles
                .entry(key)
                .or_insert_with(|| RendererUiGlyphResource {
                    atlas_texture: RenderTextureAssetId::first(slot_from_hash(key.content_hash)),
                    atlas_size: RendererUiSize {
                        width: 16,
                        height: 16,
                    },
                });
        self.logical_glyphs.insert(glyph_run_id.get(), key);
        self.table.record_glyph(glyph_run_id, resource);
    }

    fn update_layer_cache(
        &mut self,
        packet: &FunUiFramePacket,
        caps_hash: u64,
    ) -> LayerReuseReport {
        let mut report = LayerReuseReport::default();
        let mut seen_layers = BTreeMap::new();
        for layer in &packet.layers {
            let content_hash = hash_layer(layer);
            let key = ResourceCacheKey {
                source_id: self.source_id,
                resource_id: ResourceId::new(layer.layer_id.get()),
                content_hash,
                renderer_caps_hash: caps_hash,
            };
            let previous = self.layers_by_id.get(&layer.layer_id.get()).copied();
            match previous {
                Some(entry) if entry.content_hash == content_hash => {
                    report.unchanged_layers = report.unchanged_layers.saturating_add(1);
                    seen_layers.insert(layer.layer_id.get(), entry);
                }
                _ => {
                    let handle = *self.layer_handles.entry(key).or_insert_with(|| {
                        UiRendererLayerHandle::first(slot_from_hash(stable_hash_pair(
                            layer.layer_id.get(),
                            caps_hash,
                        )))
                    });
                    report.changed_layers = report.changed_layers.saturating_add(1);
                    seen_layers.insert(
                        layer.layer_id.get(),
                        UiRendererLayerCacheEntry {
                            handle,
                            content_hash,
                        },
                    );
                }
            }
        }
        self.layers_by_id = seen_layers;
        report
    }
}

impl Default for UiRendererResourceCache {
    fn default() -> Self {
        Self::new(SourceId::new(0))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ResourceDeltaApplyReport {
    image_deltas_applied: u32,
    glyph_deltas_applied: u32,
}

impl ResourceDeltaApplyReport {
    const fn total(self) -> u32 {
        self.image_deltas_applied
            .saturating_add(self.glyph_deltas_applied)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LayerReuseReport {
    unchanged_layers: u32,
    changed_layers: u32,
}

/// Policy for loss and fallback during packet submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiPacketSubmitPolicy {
    /// Allow placeholder resources for missing images or glyphs.
    pub allow_missing_resource_placeholder: bool,
    /// Allow approved degradations for unsupported operations.
    pub allow_unsupported_op_degradation: bool,
    /// Preserve the previous valid frame if a GPU upload fails.
    pub keep_previous_valid_frame_on_gpu_upload_failure: bool,
}

impl Default for UiPacketSubmitPolicy {
    fn default() -> Self {
        Self {
            allow_missing_resource_placeholder: false,
            allow_unsupported_op_degradation: false,
            keep_previous_valid_frame_on_gpu_upload_failure: true,
        }
    }
}

/// Optional packet debug overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiPacketDebugMode {
    PaintBounds,
    LayoutBounds,
    HitRegions,
    AccessibilityNamesRoles,
    LayerBoxes,
    DirtyRegions,
    SelectorMatchCostHeatmap,
    TextShapingCacheMisses,
}

impl UiPacketDebugMode {
    /// Stable label for diagnostics and UI toggles.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PaintBounds => "paint_bounds",
            Self::LayoutBounds => "layout_bounds",
            Self::HitRegions => "hit_regions",
            Self::AccessibilityNamesRoles => "accessibility_names_roles",
            Self::LayerBoxes => "layer_boxes",
            Self::DirtyRegions => "dirty_regions",
            Self::SelectorMatchCostHeatmap => "selector_match_cost_heatmap",
            Self::TextShapingCacheMisses => "text_shaping_cache_misses",
        }
    }
}

/// Debug overlay toggles for packet submission.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UiPacketDebugConfig {
    pub paint_bounds: bool,
    pub layout_bounds: bool,
    pub hit_regions: bool,
    pub accessibility_names_roles: bool,
    pub layer_boxes: bool,
    pub dirty_regions: bool,
    pub selector_match_cost_heatmap: bool,
    pub text_shaping_cache_misses: bool,
}

impl UiPacketDebugConfig {
    /// Returns the enabled modes in stable order.
    #[must_use]
    pub fn enabled_modes(self) -> Vec<UiPacketDebugMode> {
        let mut modes = Vec::new();
        if self.paint_bounds {
            modes.push(UiPacketDebugMode::PaintBounds);
        }
        if self.layout_bounds {
            modes.push(UiPacketDebugMode::LayoutBounds);
        }
        if self.hit_regions {
            modes.push(UiPacketDebugMode::HitRegions);
        }
        if self.accessibility_names_roles {
            modes.push(UiPacketDebugMode::AccessibilityNamesRoles);
        }
        if self.layer_boxes {
            modes.push(UiPacketDebugMode::LayerBoxes);
        }
        if self.dirty_regions {
            modes.push(UiPacketDebugMode::DirtyRegions);
        }
        if self.selector_match_cost_heatmap {
            modes.push(UiPacketDebugMode::SelectorMatchCostHeatmap);
        }
        if self.text_shaping_cache_misses {
            modes.push(UiPacketDebugMode::TextShapingCacheMisses);
        }
        modes
    }
}

/// Renderer-owned whole-packet consumer.
pub struct FunRendererUiPacketConsumer<'a, B: RendererBackend> {
    renderer: &'a mut FunRenderer<B>,
    resources: &'a mut UiRendererResourceCache,
    caps: RendererCaps,
    composite_layer: NativeUiCompositeLayer,
    policy: UiPacketSubmitPolicy,
    debug: UiPacketDebugConfig,
}

impl<'a, B: RendererBackend> FunRendererUiPacketConsumer<'a, B> {
    /// Creates a consumer with explicit renderer capabilities.
    pub fn new(
        renderer: &'a mut FunRenderer<B>,
        resources: &'a mut UiRendererResourceCache,
        caps: RendererCaps,
    ) -> Self {
        Self {
            renderer,
            resources,
            caps,
            composite_layer: NativeUiCompositeLayer::AfterScene,
            policy: UiPacketSubmitPolicy::default(),
            debug: UiPacketDebugConfig::default(),
        }
    }

    /// Creates a consumer with fun-renderer's conservative default UI caps.
    pub fn for_renderer(
        renderer: &'a mut FunRenderer<B>,
        resources: &'a mut UiRendererResourceCache,
    ) -> Self {
        Self::new(renderer, resources, fun_renderer_default_ui_caps(B::NAME))
    }

    /// Sets the compositor layer targeted by the UI graph pass.
    #[must_use]
    pub const fn with_composite_layer(mut self, layer: NativeUiCompositeLayer) -> Self {
        self.composite_layer = layer;
        self
    }

    /// Sets loss/fallback policy.
    #[must_use]
    pub const fn with_policy(mut self, policy: UiPacketSubmitPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Sets debug overlay modes.
    #[must_use]
    pub const fn with_debug(mut self, debug: UiPacketDebugConfig) -> Self {
        self.debug = debug;
        self
    }

    /// Returns the capability report advertised by this consumer.
    #[must_use]
    pub fn caps(&self) -> &RendererCaps {
        &self.caps
    }

    /// Returns the renderer resource cache.
    #[must_use]
    pub fn resources(&self) -> &UiRendererResourceCache {
        self.resources
    }

    /// Consumes a whole frame packet through fun-renderer's preferred path.
    pub fn consume_frame_packet(
        &mut self,
        packet: &FunUiFramePacket,
    ) -> Result<SubmitReport, SubmitError> {
        let _renderer_binding = &mut *self.renderer;
        validate_packet_against_caps(packet, &self.caps)?;

        let caps_hash = renderer_caps_hash(&self.caps);
        let resource_report = self.resources.apply_resource_deltas(packet, caps_hash);
        self.resources
            .ensure_draw_resources(packet, caps_hash, self.policy, &self.caps)?;
        let layer_report = self.resources.update_layer_cache(packet, caps_hash);

        let descriptors = NativeUiRendererDescriptors::from_packet(
            packet,
            self.resources.resource_table(),
            self.composite_layer,
        );
        if !descriptors.passed_validation() {
            return Err(error_from_native_validation(
                packet,
                &descriptors,
                self.resources,
            ));
        }

        let adapter_submit = submit_result_from_descriptors(packet, &descriptors);
        let hit_regions_published = u32::try_from(packet.hit_regions.len()).unwrap_or(u32::MAX);
        let accessibility_packets_published =
            u32::try_from(packet.accessibility.len()).unwrap_or(u32::MAX);
        let frame_graph_nodes_submitted = if packet.layers.is_empty() { 0u32 } else { 1u32 }
            .saturating_add(layer_report.changed_layers);
        let debug_overlay_modes = self.debug.enabled_modes();

        self.resources.record_last_valid(descriptors);

        Ok(SubmitReport {
            backend_family: B::NAME,
            adapter_submit,
            resource_deltas_applied: resource_report.total(),
            unchanged_layers_reused: layer_report.unchanged_layers,
            changed_layers_recorded: layer_report.changed_layers,
            frame_graph_nodes_submitted,
            hit_regions_published,
            accessibility_packets_published,
            previous_valid_frame_kept: false,
            debug_overlay_modes,
        })
    }
}

impl<B: RendererBackend> FunUiFramePacketConsumer for FunRendererUiPacketConsumer<'_, B> {
    fn capability_report(&self) -> FunRenderUiCapabilityReport {
        self.caps.clone()
    }

    fn submit_frame(
        &mut self,
        frame: &FunUiFramePacket,
    ) -> Result<FunRenderUiSubmitResult, FunRenderUiAdapterError> {
        self.consume_frame_packet(frame)
            .map(|report| report.adapter_submit)
            .map_err(SubmitError::into_adapter_error)
    }
}

/// Successful fun-renderer packet submission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunRendererUiSubmitReport {
    pub backend_family: &'static str,
    pub adapter_submit: FunRenderUiSubmitResult,
    pub resource_deltas_applied: u32,
    pub unchanged_layers_reused: u32,
    pub changed_layers_recorded: u32,
    pub frame_graph_nodes_submitted: u32,
    pub hit_regions_published: u32,
    pub accessibility_packets_published: u32,
    pub previous_valid_frame_kept: bool,
    pub debug_overlay_modes: Vec<UiPacketDebugMode>,
}

/// Typed fun-renderer packet submission failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunRendererUiSubmitError {
    Adapter(FunRenderUiAdapterError),
    ResourceDeclined {
        kind: &'static str,
        resource_id: u64,
    },
    UnsupportedOp {
        operation: &'static str,
    },
    CapabilityMismatch {
        capability: &'static str,
        source_span: Option<SourceSpan>,
    },
    GpuUploadFailed {
        kind: &'static str,
        resource_id: u64,
        previous_frame_available: bool,
    },
}

impl FunRendererUiSubmitError {
    /// Stable diagnostic code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Adapter(error) => error.code(),
            Self::ResourceDeclined { .. } => "fun.renderer.ui.resource_declined",
            Self::UnsupportedOp { .. } => "fun.renderer.ui.unsupported_op",
            Self::CapabilityMismatch { .. } => "fun.renderer.ui.capability_mismatch",
            Self::GpuUploadFailed { .. } => "fun.renderer.ui.gpu_upload_failed",
        }
    }

    fn into_adapter_error(self) -> FunRenderUiAdapterError {
        match self {
            Self::Adapter(error) => error,
            Self::ResourceDeclined { kind, resource_id } => {
                FunRenderUiAdapterError::MissingResource {
                    kind,
                    id: resource_id,
                }
            }
            Self::UnsupportedOp { operation } => {
                FunRenderUiAdapterError::UnsupportedPrimitive { operation }
            }
            Self::CapabilityMismatch { capability, .. } => {
                FunRenderUiAdapterError::BackendRejected { code: capability }
            }
            Self::GpuUploadFailed { .. } => FunRenderUiAdapterError::BackendRejected {
                code: "fun.renderer.ui.gpu_upload_failed",
            },
        }
    }
}

/// Conservative default UI caps for the native fun-renderer packet path.
#[must_use]
pub fn fun_renderer_default_ui_caps(backend_family: &'static str) -> RendererCaps {
    let mut caps = FunRenderUiCapabilityReport::full();
    caps.backend_family = backend_family;
    caps.max_logical_layers = 512;
    caps.max_draw_operations = 16_384;
    caps.max_clips = 64;
    caps.max_transforms = 64;
    caps.max_resource_deltas = 8_192;
    caps.max_glyph_runs = 8_192;
    caps.max_image_dimension_class = FunRenderUiImageDimensionClass::Up4096;
    caps.supports_path_lite = false;
    caps.supports_gradients_lite = false;
    caps.supports_shadows_lite = false;
    caps.color_space_classes = vec![FunRenderUiColorSpaceClass::Srgb];
    caps.memory_pressure_class = FunRenderUiMemoryPressureClass::Desktop;
    caps
}

fn validate_packet_against_caps(
    packet: &FunUiFramePacket,
    caps: &RendererCaps,
) -> Result<(), SubmitError> {
    if packet.schema_version != FUN_UI_PACKET_SCHEMA_VERSION
        || caps.packet_schema_version != FUN_UI_PACKET_SCHEMA_VERSION
        || caps.adapter_schema_version != FUN_RENDER_UI_ADAPTER_SCHEMA_VERSION
    {
        return Err(SubmitError::Adapter(
            FunRenderUiAdapterError::SchemaMismatch {
                expected: FUN_UI_PACKET_SCHEMA_VERSION,
                observed: packet.schema_version,
            },
        ));
    }

    let limits = FunUiPacketLimits {
        max_layers: caps.max_logical_layers as usize,
        max_draw_packets: caps.max_draw_operations as usize,
        max_resource_deltas: caps.max_resource_deltas as usize,
        max_clip_depth: caps.max_clips as usize,
        max_transform_depth: caps.max_transforms as usize,
        ..FunUiPacketLimits::DEFAULT
    };
    packet
        .validate_with_limits(&limits)
        .map_err(adapter_error_from_validation)?;
    check_capability_limits(packet, caps)?;
    check_primitive_capabilities(packet, caps)?;
    Ok(())
}

fn adapter_error_from_validation(error: FunUiPacketValidationError) -> SubmitError {
    SubmitError::Adapter(FunRenderUiAdapterError::InvalidPacket { diagnostic: error })
}

fn check_capability_limits(
    packet: &FunUiFramePacket,
    caps: &RendererCaps,
) -> Result<(), SubmitError> {
    let color_space = match packet.color_space {
        FunUiColorSpace::Srgb => FunRenderUiColorSpaceClass::Srgb,
        FunUiColorSpace::DisplayP3 => FunRenderUiColorSpaceClass::DisplayP3,
        FunUiColorSpace::Hdr10 => FunRenderUiColorSpaceClass::Hdr10,
    };
    if !caps.supports_color_space(color_space) {
        return Err(SubmitError::CapabilityMismatch {
            capability: "color_space",
            source_span: None,
        });
    }

    let layer_count = u32::try_from(packet.layers.len()).unwrap_or(u32::MAX);
    if layer_count > caps.max_logical_layers {
        return Err(SubmitError::Adapter(
            FunRenderUiAdapterError::CapabilityLimitExceeded {
                kind: "logical_layers",
                observed: layer_count,
                limit: caps.max_logical_layers,
            },
        ));
    }

    let resource_count = u32::try_from(packet.resource_deltas.len()).unwrap_or(u32::MAX);
    if resource_count > caps.max_resource_deltas {
        return Err(SubmitError::Adapter(
            FunRenderUiAdapterError::CapabilityLimitExceeded {
                kind: "resource_deltas",
                observed: resource_count,
                limit: caps.max_resource_deltas,
            },
        ));
    }

    let mut total_draws = 0u64;
    let mut glyph_run_count = 0u32;
    for layer in &packet.layers {
        let clip_count = u32::try_from(layer.clip_stack.clips.len()).unwrap_or(u32::MAX);
        if clip_count > caps.max_clips {
            return Err(SubmitError::Adapter(
                FunRenderUiAdapterError::CapabilityLimitExceeded {
                    kind: "clips",
                    observed: clip_count,
                    limit: caps.max_clips,
                },
            ));
        }
        let transform_count =
            u32::try_from(layer.transform_stack.transforms.len()).unwrap_or(u32::MAX);
        if transform_count > caps.max_transforms {
            return Err(SubmitError::Adapter(
                FunRenderUiAdapterError::CapabilityLimitExceeded {
                    kind: "transforms",
                    observed: transform_count,
                    limit: caps.max_transforms,
                },
            ));
        }
        total_draws = total_draws.saturating_add(layer.draw_packets.len() as u64);
        for draw in &layer.draw_packets {
            if matches!(draw.operation, FunUiDrawOperation::DrawTextRun { .. }) {
                glyph_run_count = glyph_run_count.saturating_add(1);
            }
        }
    }

    let total_draws = u32::try_from(total_draws).unwrap_or(u32::MAX);
    if total_draws > caps.max_draw_operations {
        return Err(SubmitError::Adapter(
            FunRenderUiAdapterError::CapabilityLimitExceeded {
                kind: "draw_operations",
                observed: total_draws,
                limit: caps.max_draw_operations,
            },
        ));
    }

    if glyph_run_count > caps.max_glyph_runs {
        return Err(SubmitError::Adapter(
            FunRenderUiAdapterError::CapabilityLimitExceeded {
                kind: "glyph_runs",
                observed: glyph_run_count,
                limit: caps.max_glyph_runs,
            },
        ));
    }

    let max_side = caps.max_image_dimension_class.max_side();
    for delta in &packet.resource_deltas {
        if let FunUiResourceDelta::Image(image) = delta {
            let observed = image.size.width.max(image.size.height);
            if observed > max_side {
                return Err(SubmitError::Adapter(
                    FunRenderUiAdapterError::CapabilityLimitExceeded {
                        kind: "image_dimension",
                        observed,
                        limit: max_side,
                    },
                ));
            }
        }
    }
    Ok(())
}

fn check_primitive_capabilities(
    packet: &FunUiFramePacket,
    caps: &RendererCaps,
) -> Result<(), SubmitError> {
    for layer in &packet.layers {
        for draw in &layer.draw_packets {
            let (enabled, label) = match draw.operation {
                FunUiDrawOperation::FillRect { .. } => (caps.supports_solid_rect, "fill_rect"),
                FunUiDrawOperation::StrokeRect { .. } => (caps.supports_border, "stroke_rect"),
                FunUiDrawOperation::FillRoundedRect { .. } => {
                    (caps.supports_rounded_rect, "fill_rounded_rect")
                }
                FunUiDrawOperation::StrokeRoundedRect { .. } => {
                    (caps.supports_border, "stroke_rounded_rect")
                }
                FunUiDrawOperation::DrawTextRun { .. } => {
                    (caps.supports_text_run_reference, "draw_text_run")
                }
                FunUiDrawOperation::DrawImage { .. } => {
                    (caps.supports_image_reference, "draw_image")
                }
                FunUiDrawOperation::DrawPathLite { .. } => {
                    (caps.supports_path_lite, "draw_path_lite")
                }
                FunUiDrawOperation::PushClip { .. } => (caps.supports_clip_stack, "push_clip"),
                FunUiDrawOperation::PopClip { .. } => (caps.supports_clip_stack, "pop_clip"),
                FunUiDrawOperation::PushTransform { .. } => {
                    (caps.supports_transform_stack, "push_transform")
                }
                FunUiDrawOperation::PopTransform => {
                    (caps.supports_transform_stack, "pop_transform")
                }
                FunUiDrawOperation::PushOpacity { .. } => {
                    (caps.supports_opacity_stack, "push_opacity")
                }
                FunUiDrawOperation::PopOpacity => (caps.supports_opacity_stack, "pop_opacity"),
                FunUiDrawOperation::DebugBounds { .. } => {
                    (caps.supports_debug_bounds, "debug_bounds")
                }
            };
            if !enabled {
                return Err(SubmitError::UnsupportedOp { operation: label });
            }
        }
    }
    Ok(())
}

fn error_from_native_validation(
    packet: &FunUiFramePacket,
    descriptors: &NativeUiRendererDescriptors,
    resources: &UiRendererResourceCache,
) -> SubmitError {
    let validation = &descriptors.validation;
    if validation.schema_mismatch {
        return SubmitError::Adapter(FunRenderUiAdapterError::SchemaMismatch {
            expected: FUN_UI_PACKET_SCHEMA_VERSION,
            observed: packet.schema_version,
        });
    }
    if validation.missing_glyph_resources > 0 {
        return SubmitError::ResourceDeclined {
            kind: "glyph",
            resource_id: first_missing_glyph(packet, resources.resource_table()),
        };
    }
    if validation.missing_image_resources > 0 {
        return SubmitError::ResourceDeclined {
            kind: "image",
            resource_id: first_missing_image(packet, resources.resource_table()),
        };
    }
    if validation.unsupported_path_lite_count > 0 {
        return SubmitError::UnsupportedOp {
            operation: "draw_path_lite",
        };
    }
    if validation.clip_stack_imbalance {
        return SubmitError::Adapter(FunRenderUiAdapterError::InvalidClipStack {
            detail: "fun_renderer_clip_stack_imbalance",
            clip_id: None,
        });
    }
    if validation.transform_stack_imbalance {
        return SubmitError::Adapter(FunRenderUiAdapterError::InvalidTransformStack {
            detail: "fun_renderer_transform_stack_imbalance",
        });
    }
    if validation.opacity_stack_imbalance {
        return SubmitError::Adapter(FunRenderUiAdapterError::InvalidOpacityStack {
            detail: "fun_renderer_opacity_stack_imbalance",
        });
    }
    SubmitError::Adapter(FunRenderUiAdapterError::BackendRejected {
        code: "fun.renderer.ui.native_validation_failed",
    })
}

fn submit_result_from_descriptors(
    packet: &FunUiFramePacket,
    descriptors: &NativeUiRendererDescriptors,
) -> FunRenderUiSubmitResult {
    let (clip_pushes, transform_pushes, opacity_pushes) = count_stack_pushes(packet);
    FunRenderUiSubmitResult {
        layers_submitted: u32::try_from(descriptors.layer_summaries.len()).unwrap_or(u32::MAX),
        draws_submitted: u32::try_from(descriptors.draw_commands.len()).unwrap_or(u32::MAX),
        glyph_intents_uploaded: u32::try_from(
            packet
                .resource_deltas
                .iter()
                .filter(|delta| matches!(delta, FunUiResourceDelta::Glyph(_)))
                .count(),
        )
        .unwrap_or(u32::MAX),
        image_intents_uploaded: u32::try_from(
            packet
                .resource_deltas
                .iter()
                .filter(|delta| matches!(delta, FunUiResourceDelta::Image(_)))
                .count(),
        )
        .unwrap_or(u32::MAX),
        clip_pushes,
        transform_pushes,
        opacity_pushes,
    }
}

fn count_stack_pushes(packet: &FunUiFramePacket) -> (u32, u32, u32) {
    let mut clip_pushes = 0u32;
    let mut transform_pushes = 0u32;
    let mut opacity_pushes = 0u32;
    for layer in &packet.layers {
        for draw in &layer.draw_packets {
            match draw.operation {
                FunUiDrawOperation::PushClip { .. } => clip_pushes = clip_pushes.saturating_add(1),
                FunUiDrawOperation::PushTransform { .. } => {
                    transform_pushes = transform_pushes.saturating_add(1);
                }
                FunUiDrawOperation::PushOpacity { .. } => {
                    opacity_pushes = opacity_pushes.saturating_add(1);
                }
                _ => {}
            }
        }
    }
    (clip_pushes, transform_pushes, opacity_pushes)
}

fn first_missing_image(packet: &FunUiFramePacket, table: &NativeUiResourceTable) -> u64 {
    for layer in &packet.layers {
        for draw in &layer.draw_packets {
            if let FunUiDrawOperation::DrawImage { image_id, .. } = draw.operation
                && table.image(image_id).is_none()
            {
                return image_id.get();
            }
        }
    }
    0
}

fn first_missing_glyph(packet: &FunUiFramePacket, table: &NativeUiResourceTable) -> u64 {
    for layer in &packet.layers {
        for draw in &layer.draw_packets {
            if let FunUiDrawOperation::DrawTextRun { glyph_run_id, .. } = draw.operation
                && table.glyph(glyph_run_id).is_none()
            {
                return glyph_run_id.get();
            }
        }
    }
    0
}

fn image_resource_for_key(
    image: FunUiImageDelta,
    key: ResourceCacheKey,
) -> RendererUiImageResource {
    RendererUiImageResource {
        texture: RenderTextureAssetId::first(slot_from_hash(key.content_hash)),
        size: RendererUiSize::from_packet(image.size),
        format: RendererUiImageFormat::from_packet(image.format),
    }
}

fn glyph_resource_for_key(size: FunUiSize, key: ResourceCacheKey) -> RendererUiGlyphResource {
    RendererUiGlyphResource {
        atlas_texture: RenderTextureAssetId::first(slot_from_hash(key.content_hash)),
        atlas_size: RendererUiSize {
            width: size.width.max(16),
            height: size.height.max(16),
        },
    }
}

fn glyph_content_hash(source_text_digest: u64, glyph: &rvelte_fun_ui_core::FunUiGlyphDelta) -> u64 {
    let mut hash = stable_hash_pair(source_text_digest, glyph.glyph_run_id.get());
    hash = stable_hash_pair(hash, glyph.font_id.get());
    hash = stable_hash_pair(hash, glyph.bounds.size.width.into());
    hash = stable_hash_pair(hash, glyph.bounds.size.height.into());
    stable_hash_pair(hash, glyph.glyphs.len() as u64)
}

fn renderer_caps_hash(caps: &RendererCaps) -> u64 {
    let mut hash = stable_hash_bytes(0xcbf2_9ce4_8422_2325, caps.backend_family.as_bytes());
    hash = stable_hash_pair(hash, caps.max_logical_layers.into());
    hash = stable_hash_pair(hash, caps.max_draw_operations.into());
    hash = stable_hash_pair(hash, caps.max_clips.into());
    hash = stable_hash_pair(hash, caps.max_transforms.into());
    hash = stable_hash_pair(hash, caps.max_resource_deltas.into());
    hash = stable_hash_pair(hash, caps.max_glyph_runs.into());
    hash = stable_hash_pair(hash, caps.max_image_dimension_class.max_side().into());
    for enabled in [
        caps.supports_solid_rect,
        caps.supports_rounded_rect,
        caps.supports_border,
        caps.supports_text_run_reference,
        caps.supports_image_reference,
        caps.supports_clip_stack,
        caps.supports_transform_stack,
        caps.supports_opacity_stack,
        caps.supports_path_lite,
        caps.supports_debug_bounds,
    ] {
        hash = stable_hash_pair(hash, u64::from(enabled));
    }
    hash
}

fn hash_layer(layer: &FunUiLayerPacket) -> u64 {
    let mut hash = stable_hash_pair(layer.layer_id.get(), layer.node_id.get());
    hash = stable_hash_pair(hash, layer.z_order as u32 as u64);
    hash = stable_hash_pair(hash, layer.opacity.milli.into());
    hash = stable_hash_pair(hash, layer.clip_stack.clips.len() as u64);
    hash = stable_hash_pair(hash, layer.transform_stack.transforms.len() as u64);
    for draw in &layer.draw_packets {
        hash = stable_hash_pair(hash, draw.draw_id.get());
        hash = stable_hash_pair(hash, draw.node_id.get());
        hash = stable_hash_pair(hash, hash_draw_operation(&draw.operation));
    }
    hash
}

fn hash_draw_operation(operation: &FunUiDrawOperation) -> u64 {
    let mut hash = stable_hash_bytes(0xcbf2_9ce4_8422_2325, operation.as_str().as_bytes());
    match operation {
        FunUiDrawOperation::FillRect { rect, color }
        | FunUiDrawOperation::FillRoundedRect { rect, color, .. }
        | FunUiDrawOperation::DebugBounds { rect, color } => {
            hash = hash_rect(hash, rect);
            hash = hash_color(hash, *color);
        }
        FunUiDrawOperation::StrokeRect { rect, stroke }
        | FunUiDrawOperation::StrokeRoundedRect { rect, stroke, .. } => {
            hash = hash_rect(hash, rect);
            hash = hash_color(hash, stroke.color);
            hash = stable_hash_pair(hash, stroke.width.into());
        }
        FunUiDrawOperation::DrawTextRun {
            glyph_run_id,
            origin,
            color,
        } => {
            hash = stable_hash_pair(hash, glyph_run_id.get());
            hash = stable_hash_pair(hash, origin.x as u32 as u64);
            hash = stable_hash_pair(hash, origin.y as u32 as u64);
            hash = hash_color(hash, *color);
        }
        FunUiDrawOperation::DrawImage {
            image_id,
            rect,
            tint,
        } => {
            hash = stable_hash_pair(hash, image_id.get());
            hash = hash_rect(hash, rect);
            if let Some(tint) = tint {
                hash = hash_color(hash, *tint);
            }
        }
        FunUiDrawOperation::DrawPathLite { path, paint } => {
            hash = stable_hash_pair(hash, path.commands.len() as u64);
            for command in &path.commands {
                hash = hash_path_command(hash, command);
            }
            hash = hash_path_paint(hash, *paint);
        }
        FunUiDrawOperation::PushClip { clip } => {
            hash = stable_hash_pair(hash, clip.clip_id.get());
        }
        FunUiDrawOperation::PopClip { clip_id } => {
            hash = stable_hash_pair(hash, clip_id.get());
        }
        FunUiDrawOperation::PushTransform { transform } => {
            hash = stable_hash_pair(hash, transform.m11_milli as u32 as u64);
            hash = stable_hash_pair(hash, transform.m22_milli as u32 as u64);
            hash = stable_hash_pair(hash, transform.tx as u32 as u64);
            hash = stable_hash_pair(hash, transform.ty as u32 as u64);
        }
        FunUiDrawOperation::PushOpacity { opacity } => {
            hash = stable_hash_pair(hash, opacity.milli.into());
        }
        FunUiDrawOperation::PopTransform | FunUiDrawOperation::PopOpacity => {}
    }
    hash
}

fn hash_rect(hash: u64, rect: &rvelte_fun_ui_core::FunUiRect) -> u64 {
    let hash = stable_hash_pair(hash, rect.origin.x as u32 as u64);
    let hash = stable_hash_pair(hash, rect.origin.y as u32 as u64);
    let hash = stable_hash_pair(hash, rect.size.width.into());
    stable_hash_pair(hash, rect.size.height.into())
}

fn hash_color(hash: u64, color: rvelte_fun_ui_core::FunUiColorRgba8) -> u64 {
    let packed = u64::from(color.r)
        | (u64::from(color.g) << 8)
        | (u64::from(color.b) << 16)
        | (u64::from(color.a) << 24);
    stable_hash_pair(hash, packed)
}

fn hash_path_command(hash: u64, command: &rvelte_fun_ui_core::FunUiPathCommand) -> u64 {
    match command {
        rvelte_fun_ui_core::FunUiPathCommand::MoveTo(point) => {
            hash_point(stable_hash_pair(hash, 1), point)
        }
        rvelte_fun_ui_core::FunUiPathCommand::LineTo(point) => {
            hash_point(stable_hash_pair(hash, 2), point)
        }
        rvelte_fun_ui_core::FunUiPathCommand::QuadTo { control, to } => {
            let hash = hash_point(stable_hash_pair(hash, 3), control);
            hash_point(hash, to)
        }
        rvelte_fun_ui_core::FunUiPathCommand::CubicTo {
            control_a,
            control_b,
            to,
        } => {
            let hash = hash_point(stable_hash_pair(hash, 4), control_a);
            let hash = hash_point(hash, control_b);
            hash_point(hash, to)
        }
        rvelte_fun_ui_core::FunUiPathCommand::Close => stable_hash_pair(hash, 5),
    }
}

fn hash_path_paint(hash: u64, paint: rvelte_fun_ui_core::FunUiPathPaint) -> u64 {
    match paint {
        rvelte_fun_ui_core::FunUiPathPaint::Fill(color) => {
            hash_color(stable_hash_pair(hash, 1), color)
        }
        rvelte_fun_ui_core::FunUiPathPaint::Stroke(stroke) => {
            let hash = hash_color(stable_hash_pair(hash, 2), stroke.color);
            stable_hash_pair(hash, stroke.width.into())
        }
    }
}

fn hash_point(hash: u64, point: &rvelte_fun_ui_core::FunUiPoint) -> u64 {
    let hash = stable_hash_pair(hash, point.x as u32 as u64);
    stable_hash_pair(hash, point.y as u32 as u64)
}

fn stable_hash_pair(seed: u64, value: u64) -> u64 {
    stable_hash_bytes(seed, &value.to_le_bytes())
}

fn stable_hash_bytes(mut seed: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        seed ^= u64::from(*byte);
        seed = seed.wrapping_mul(0x0000_0100_0000_01b3);
    }
    seed
}

fn slot_from_hash(hash: u64) -> u32 {
    let slot = (hash as u32) & 0x7fff_fffe;
    slot.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use rvelte_fun_ui_core::{
        FunUiColorRgba8, FunUiDrawId, FunUiFrameId, FunUiGlyphDelta, FunUiGlyphInstance,
        FunUiImageFormat, FunUiImageId, FunUiLayerId, FunUiLayerPacket, FunUiNodeId,
        FunUiPathCommand, FunUiPathLite, FunUiPathPaint, FunUiPoint, FunUiRect, FunUiStroke,
    };

    fn empty_frame() -> FunUiFramePacket {
        let mut packet = FunUiFramePacket::new(FunUiFrameId::new(7), FunUiSize::new(640, 360));
        packet.tree_revision = 1;
        packet
    }

    fn fill_frame() -> FunUiFramePacket {
        let mut packet = empty_frame();
        packet.layers = vec![FunUiLayerPacket::new(
            FunUiLayerId::new(1),
            FunUiNodeId::new(1),
            0,
            vec![rvelte_fun_ui_core::FunUiDrawPacket::new(
                FunUiDrawId::new(1),
                FunUiNodeId::new(1),
                FunUiDrawOperation::FillRect {
                    rect: FunUiRect::new(0, 0, 120, 40),
                    color: FunUiColorRgba8::new(20, 40, 80, 255),
                },
            )],
        )];
        packet
    }

    fn image_frame(with_delta: bool) -> FunUiFramePacket {
        let mut packet = empty_frame();
        if with_delta {
            packet
                .resource_deltas
                .push(FunUiResourceDelta::Image(FunUiImageDelta::new(
                    FunUiImageId::new(12),
                    FunUiSize::new(32, 32),
                    FunUiImageFormat::Rgba8Premultiplied,
                    32 * 32 * 4,
                    0x1234_5678,
                )));
        }
        packet.layers = vec![FunUiLayerPacket::new(
            FunUiLayerId::new(1),
            FunUiNodeId::new(1),
            0,
            vec![rvelte_fun_ui_core::FunUiDrawPacket::new(
                FunUiDrawId::new(1),
                FunUiNodeId::new(1),
                FunUiDrawOperation::DrawImage {
                    image_id: FunUiImageId::new(12),
                    rect: FunUiRect::new(0, 0, 32, 32),
                    tint: None,
                },
            )],
        )];
        packet
    }

    fn path_frame() -> FunUiFramePacket {
        let mut packet = empty_frame();
        let path = FunUiPathLite::new(
            rvelte_fun_ui_core::FunUiFillRule::NonZero,
            vec![
                FunUiPathCommand::MoveTo(FunUiPoint::new(0, 0)),
                FunUiPathCommand::LineTo(FunUiPoint::new(16, 16)),
            ],
        );
        packet.layers = vec![FunUiLayerPacket::new(
            FunUiLayerId::new(1),
            FunUiNodeId::new(1),
            0,
            vec![rvelte_fun_ui_core::FunUiDrawPacket::new(
                FunUiDrawId::new(1),
                FunUiNodeId::new(1),
                FunUiDrawOperation::DrawPathLite {
                    path,
                    paint: FunUiPathPaint::Stroke(FunUiStroke::new(
                        FunUiColorRgba8::new(255, 255, 255, 255),
                        1,
                    )),
                },
            )],
        )];
        packet
    }

    fn glyph_frame() -> FunUiFramePacket {
        let mut packet = empty_frame();
        packet
            .resource_deltas
            .push(FunUiResourceDelta::Glyph(FunUiGlyphDelta::new(
                rvelte_fun_ui_core::FunUiFontId::new(1),
                FunUiGlyphRunId::new(2),
                0xfeed_face,
                FunUiRect::new(0, 0, 24, 16),
                vec![FunUiGlyphInstance {
                    glyph_index: 3,
                    advance: 12,
                    offset: FunUiPoint::new(0, 0),
                }],
            )));
        packet.layers = vec![FunUiLayerPacket::new(
            FunUiLayerId::new(1),
            FunUiNodeId::new(1),
            0,
            vec![rvelte_fun_ui_core::FunUiDrawPacket::new(
                FunUiDrawId::new(1),
                FunUiNodeId::new(1),
                FunUiDrawOperation::DrawTextRun {
                    glyph_run_id: FunUiGlyphRunId::new(2),
                    origin: FunUiPoint::new(0, 12),
                    color: FunUiColorRgba8::new(255, 255, 255, 255),
                },
            )],
        )];
        packet
    }

    #[test]
    fn consumer_applies_resource_deltas_and_submits_descriptors() {
        let mut renderer = Renderer::<NullBackend>::new();
        let mut resources = UiRendererResourceCache::new(SourceId::new(1));
        let mut consumer = FunRendererUiPacketConsumer::for_renderer(&mut renderer, &mut resources);

        let report = consumer
            .consume_frame_packet(&image_frame(true))
            .expect("image delta should materialize and submit");

        assert_eq!(report.backend_family, "null");
        assert_eq!(report.adapter_submit.image_intents_uploaded, 1);
        assert_eq!(report.resource_deltas_applied, 1);
        assert_eq!(report.changed_layers_recorded, 1);
        assert_eq!(consumer.resources().image_resource_count(), 1);
        assert!(consumer.resources().last_descriptors().is_some());
    }

    #[test]
    fn missing_image_is_declined_unless_placeholder_policy_allows() {
        let mut renderer = Renderer::<NullBackend>::new();
        let mut resources = UiRendererResourceCache::new(SourceId::new(1));
        let mut consumer = FunRendererUiPacketConsumer::for_renderer(&mut renderer, &mut resources);

        let error = consumer
            .consume_frame_packet(&image_frame(false))
            .expect_err("missing image should fail by default");
        assert!(matches!(
            error,
            SubmitError::ResourceDeclined {
                kind: "image",
                resource_id: 12
            }
        ));

        let policy = UiPacketSubmitPolicy {
            allow_missing_resource_placeholder: true,
            ..UiPacketSubmitPolicy::default()
        };
        let mut renderer = Renderer::<NullBackend>::new();
        let mut resources = UiRendererResourceCache::new(SourceId::new(1));
        let mut consumer = FunRendererUiPacketConsumer::for_renderer(&mut renderer, &mut resources)
            .with_policy(policy);
        let report = consumer
            .consume_frame_packet(&image_frame(false))
            .expect("placeholder policy should materialize fallback image");
        assert_eq!(report.adapter_submit.draws_submitted, 1);
        assert_eq!(consumer.resources().image_resource_count(), 1);
    }

    #[test]
    fn glyph_resource_deltas_feed_native_table() {
        let mut renderer = Renderer::<NullBackend>::new();
        let mut resources = UiRendererResourceCache::new(SourceId::new(1));
        let mut consumer = FunRendererUiPacketConsumer::for_renderer(&mut renderer, &mut resources);

        let report = consumer
            .consume_frame_packet(&glyph_frame())
            .expect("glyph delta should satisfy text draw");

        assert_eq!(report.adapter_submit.glyph_intents_uploaded, 1);
        assert_eq!(consumer.resources().glyph_resource_count(), 1);
    }

    #[test]
    fn unchanged_layer_is_reused_on_second_submit() {
        let mut renderer = Renderer::<NullBackend>::new();
        let mut resources = UiRendererResourceCache::new(SourceId::new(1));
        let mut consumer = FunRendererUiPacketConsumer::for_renderer(&mut renderer, &mut resources);
        let frame = fill_frame();

        let first = consumer
            .consume_frame_packet(&frame)
            .expect("first frame should submit");
        let second = consumer
            .consume_frame_packet(&frame)
            .expect("second frame should reuse unchanged layer");

        assert_eq!(first.changed_layers_recorded, 1);
        assert_eq!(second.unchanged_layers_reused, 1);
        assert_eq!(second.changed_layers_recorded, 0);
    }

    #[test]
    fn unsupported_path_lite_rejects_before_partial_submission() {
        let mut renderer = Renderer::<NullBackend>::new();
        let mut resources = UiRendererResourceCache::new(SourceId::new(1));
        let mut consumer = FunRendererUiPacketConsumer::for_renderer(&mut renderer, &mut resources);

        let error = consumer
            .consume_frame_packet(&path_frame())
            .expect_err("native packet path does not claim path-lite yet");

        assert_eq!(error.code(), "fun.renderer.ui.unsupported_op");
        assert!(resources.last_valid_frame().is_none());
    }

    #[test]
    fn debug_mode_labels_are_stable() {
        let config = UiPacketDebugConfig {
            paint_bounds: true,
            hit_regions: true,
            accessibility_names_roles: true,
            text_shaping_cache_misses: true,
            ..UiPacketDebugConfig::default()
        };
        let labels = config
            .enabled_modes()
            .into_iter()
            .map(UiPacketDebugMode::as_str)
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec![
                "paint_bounds",
                "hit_regions",
                "accessibility_names_roles",
                "text_shaping_cache_misses",
            ]
        );
    }

    #[test]
    fn implements_rvelte_packet_consumer_trait() {
        let mut renderer = Renderer::<NullBackend>::new();
        let mut resources = UiRendererResourceCache::new(SourceId::new(1));
        let mut consumer = FunRendererUiPacketConsumer::for_renderer(&mut renderer, &mut resources);

        let report = FunUiFramePacketConsumer::submit_frame(&mut consumer, &fill_frame())
            .expect("trait submit should route to whole-packet consumer");

        assert_eq!(report.layers_submitted, 1);
        assert_eq!(report.draws_submitted, 1);
    }
}
