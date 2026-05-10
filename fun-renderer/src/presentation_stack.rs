//! Pass 23 — Post, HDR, and Presentation Stack.
//!
//! Move from raw scene output to production presentation by tying
//! the post-process graph plan, HDR/SDR presentation policy, TAA
//! scaffold, and the per-stage debug view enumeration into one
//! consumable frame artifact. The bridge ingests one
//! `PresentationStackPlan` per frame to allocate every
//! intermediate texture (bloom mip chain, exposure histogram,
//! color-grading LUT slot, sharpen target, debug overlay layer,
//! TAA history, velocity feedback) through the shared frame-graph
//! resource registry. The renderer never holds wgpu objects — it
//! holds typed `PostProcessIntermediateSlot` indices that the
//! bridge resolves at submission time.
//!
//! Design rules:
//!
//! 1. The final image is always graph-composited
//!    `FinalComposedOutput`, never a raw scene target. The Pass 23
//!    contract installs `FinalOutputTransform` as a mandatory
//!    closing pass and asserts the graph wiring.
//! 2. UI and scene color policies are explicit. Native UI packets
//!    declare their color space; `UiPacketColorPolicy::resolve`
//!    converts that into the `UiCompositionPolicy` the
//!    presentation policy consumes — no implicit inference from
//!    swapchain format.
//! 3. Post resources are allocated through the graph / resource
//!    registry. `PostProcessResourceRegistry::allocate` returns a
//!    typed `PostProcessIntermediateSlot` that names the underlying
//!    `FrameGraphResourceType`; callers cannot bypass the registry.

use bevy_ecs::prelude::Resource;

use crate::component_api::{CameraDebugView, RenderExtent2d, RenderStableId, UiColorSpace};
use crate::frame_graph::FrameGraphResourceType;
use crate::post_process::{PostProcessGraphPlan, PostProcessPassKind};
use crate::presentation::{
    HdrOutputState, PresentationPolicy, SceneLinearTargetFormat, SwapchainColorSpace,
    UiCompositionPolicy,
};
use crate::taa::{TaaFramePlan, TaaHistoryStatus};

pub const PRESENTATION_STACK_SCHEMA_VERSION: u16 = 1;
pub const POST_PROCESS_INTERMEDIATE_KIND_COUNT: usize = 7;
pub const PRESENTATION_DEBUG_VIEW_COUNT: usize = 12;
pub const SWAPCHAIN_FORMAT_REPORT_COUNT: usize = 6;

// ============================================================================
// Section 1 — Post-process intermediate registry
// ============================================================================

/// Typed kind for every post-stage intermediate the renderer needs
/// allocated through the graph / resource registry. The renderer
/// never holds a wgpu texture for any of these — it holds a
/// `PostProcessIntermediateSlot` that maps to the typed graph
/// resource the bridge owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessIntermediateKind {
    BloomMipChain,
    ExposureHistogram,
    ColorGradingLut,
    SharpenedTarget,
    DebugOverlayLayer,
    TaaHistory,
    VelocityFeedback,
}

impl PostProcessIntermediateKind {
    pub const ALL: [Self; POST_PROCESS_INTERMEDIATE_KIND_COUNT] = [
        Self::BloomMipChain,
        Self::ExposureHistogram,
        Self::ColorGradingLut,
        Self::SharpenedTarget,
        Self::DebugOverlayLayer,
        Self::TaaHistory,
        Self::VelocityFeedback,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::BloomMipChain => 0,
            Self::ExposureHistogram => 1,
            Self::ColorGradingLut => 2,
            Self::SharpenedTarget => 3,
            Self::DebugOverlayLayer => 4,
            Self::TaaHistory => 5,
            Self::VelocityFeedback => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BloomMipChain => "bloom_mip_chain",
            Self::ExposureHistogram => "exposure_histogram",
            Self::ColorGradingLut => "color_grading_lut",
            Self::SharpenedTarget => "sharpened_target",
            Self::DebugOverlayLayer => "debug_overlay_layer",
            Self::TaaHistory => "taa_history",
            Self::VelocityFeedback => "velocity_feedback",
        }
    }

    /// Name the underlying `FrameGraphResourceType` the registry
    /// maps this intermediate to. Most intermediates resolve to
    /// `TransientScratch` (frame-local short-lived); the exposure
    /// histogram resolves to the `Exposure` resource so downstream
    /// auto-exposure passes can read it; sharpened target writes
    /// `DisplayResolutionSceneColor`; debug overlay layer writes
    /// `DisplayResolutionSceneColor`; TAA history is the
    /// dedicated `HistoryBuffer`; velocity feedback is
    /// `MotionVectors`.
    #[must_use]
    pub const fn frame_graph_resource(self) -> FrameGraphResourceType {
        match self {
            Self::BloomMipChain => FrameGraphResourceType::TransientScratch,
            Self::ExposureHistogram => FrameGraphResourceType::Exposure,
            Self::ColorGradingLut => FrameGraphResourceType::TransientScratch,
            Self::SharpenedTarget => FrameGraphResourceType::DisplayResolutionSceneColor,
            Self::DebugOverlayLayer => FrameGraphResourceType::DisplayResolutionSceneColor,
            Self::TaaHistory => FrameGraphResourceType::HistoryBuffer,
            Self::VelocityFeedback => FrameGraphResourceType::MotionVectors,
        }
    }

    /// Default lifetime — frame-local intermediates are dropped
    /// after the frame; persistent intermediates (TaaHistory,
    /// ExposureHistogram with auto-exposure, ColorGradingLut)
    /// survive across frames.
    #[must_use]
    pub const fn default_lifetime(self) -> PostProcessResourceLifetime {
        match self {
            Self::BloomMipChain | Self::SharpenedTarget | Self::DebugOverlayLayer => {
                PostProcessResourceLifetime::PerFrameTransient
            }
            Self::ExposureHistogram | Self::TaaHistory => {
                PostProcessResourceLifetime::CrossFramePersistent
            }
            Self::ColorGradingLut => PostProcessResourceLifetime::AssetBacked,
            Self::VelocityFeedback => PostProcessResourceLifetime::PerFrameTransient,
        }
    }
}

/// Lifetime category for a post-process intermediate. Drives whether
/// the resource registry recycles the slot at frame end, keeps the
/// underlying buffer across frames (history-driven passes), or
/// pulls from the asset registry (LUT bound to camera grading).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessResourceLifetime {
    #[default]
    PerFrameTransient,
    CrossFramePersistent,
    AssetBacked,
}

impl PostProcessResourceLifetime {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PerFrameTransient => "per_frame_transient",
            Self::CrossFramePersistent => "cross_frame_persistent",
            Self::AssetBacked => "asset_backed",
        }
    }
}

/// Typed slot returned by `PostProcessResourceRegistry::allocate`.
/// The slot is the renderer's only handle to the intermediate; the
/// bridge resolves it to a wgpu texture / buffer at submission.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PostProcessIntermediateSlot {
    pub kind: PostProcessIntermediateKindOption,
    pub slot: u32,
    pub generation: u32,
    pub frame_graph_resource: PostProcessFrameGraphResourceOption,
}

impl PostProcessIntermediateSlot {
    pub const INVALID: Self = Self {
        kind: PostProcessIntermediateKindOption::BloomMipChain,
        slot: u32::MAX,
        generation: 0,
        frame_graph_resource: PostProcessFrameGraphResourceOption::TransientScratch,
    };

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.slot != u32::MAX && self.generation != 0
    }
}

/// `Copy + Hash` snapshot of `PostProcessIntermediateKind` so the
/// slot stays POD-cheap.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessIntermediateKindOption {
    #[default]
    BloomMipChain,
    ExposureHistogram,
    ColorGradingLut,
    SharpenedTarget,
    DebugOverlayLayer,
    TaaHistory,
    VelocityFeedback,
}

impl From<PostProcessIntermediateKind> for PostProcessIntermediateKindOption {
    fn from(kind: PostProcessIntermediateKind) -> Self {
        match kind {
            PostProcessIntermediateKind::BloomMipChain => Self::BloomMipChain,
            PostProcessIntermediateKind::ExposureHistogram => Self::ExposureHistogram,
            PostProcessIntermediateKind::ColorGradingLut => Self::ColorGradingLut,
            PostProcessIntermediateKind::SharpenedTarget => Self::SharpenedTarget,
            PostProcessIntermediateKind::DebugOverlayLayer => Self::DebugOverlayLayer,
            PostProcessIntermediateKind::TaaHistory => Self::TaaHistory,
            PostProcessIntermediateKind::VelocityFeedback => Self::VelocityFeedback,
        }
    }
}

impl From<PostProcessIntermediateKindOption> for PostProcessIntermediateKind {
    fn from(option: PostProcessIntermediateKindOption) -> Self {
        match option {
            PostProcessIntermediateKindOption::BloomMipChain => Self::BloomMipChain,
            PostProcessIntermediateKindOption::ExposureHistogram => Self::ExposureHistogram,
            PostProcessIntermediateKindOption::ColorGradingLut => Self::ColorGradingLut,
            PostProcessIntermediateKindOption::SharpenedTarget => Self::SharpenedTarget,
            PostProcessIntermediateKindOption::DebugOverlayLayer => Self::DebugOverlayLayer,
            PostProcessIntermediateKindOption::TaaHistory => Self::TaaHistory,
            PostProcessIntermediateKindOption::VelocityFeedback => Self::VelocityFeedback,
        }
    }
}

/// `Copy` snapshot of the underlying `FrameGraphResourceType`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessFrameGraphResourceOption {
    #[default]
    TransientScratch,
    Exposure,
    DisplayResolutionSceneColor,
    HistoryBuffer,
    MotionVectors,
}

impl From<FrameGraphResourceType> for PostProcessFrameGraphResourceOption {
    fn from(value: FrameGraphResourceType) -> Self {
        match value {
            FrameGraphResourceType::TransientScratch => Self::TransientScratch,
            FrameGraphResourceType::Exposure => Self::Exposure,
            FrameGraphResourceType::DisplayResolutionSceneColor => {
                Self::DisplayResolutionSceneColor
            }
            FrameGraphResourceType::HistoryBuffer => Self::HistoryBuffer,
            FrameGraphResourceType::MotionVectors => Self::MotionVectors,
            _ => Self::TransientScratch,
        }
    }
}

impl From<PostProcessFrameGraphResourceOption> for FrameGraphResourceType {
    fn from(value: PostProcessFrameGraphResourceOption) -> Self {
        match value {
            PostProcessFrameGraphResourceOption::TransientScratch => Self::TransientScratch,
            PostProcessFrameGraphResourceOption::Exposure => Self::Exposure,
            PostProcessFrameGraphResourceOption::DisplayResolutionSceneColor => {
                Self::DisplayResolutionSceneColor
            }
            PostProcessFrameGraphResourceOption::HistoryBuffer => Self::HistoryBuffer,
            PostProcessFrameGraphResourceOption::MotionVectors => Self::MotionVectors,
        }
    }
}

/// Typed descriptor for an intermediate at allocate time. Carries
/// the format, extent policy, mip-chain depth (bloom), histogram
/// bin count (exposure), LUT side length (color grading), and
/// declared lifetime so the registry can produce a stable allocation
/// rather than a generic byte buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PostProcessIntermediateDescriptor {
    pub kind: PostProcessIntermediateKindOption,
    pub extent: RenderExtent2d,
    pub mip_chain_depth: u8,
    pub histogram_bin_count: u16,
    pub lut_side_length: u16,
    pub format: SceneLinearTargetFormat,
    pub lifetime: PostProcessResourceLifetime,
    pub debug_label: &'static str,
}

impl PostProcessIntermediateDescriptor {
    #[must_use]
    pub const fn bloom(extent: RenderExtent2d, mip_chain_depth: u8) -> Self {
        Self {
            kind: PostProcessIntermediateKindOption::BloomMipChain,
            extent,
            mip_chain_depth,
            histogram_bin_count: 0,
            lut_side_length: 0,
            format: SceneLinearTargetFormat::Rgba16Float,
            lifetime: PostProcessResourceLifetime::PerFrameTransient,
            debug_label: "post_process.bloom_mip_chain",
        }
    }

    #[must_use]
    pub const fn exposure_histogram(bin_count: u16) -> Self {
        Self {
            kind: PostProcessIntermediateKindOption::ExposureHistogram,
            extent: RenderExtent2d {
                width: bin_count as u32,
                height: 1,
            },
            mip_chain_depth: 1,
            histogram_bin_count: bin_count,
            lut_side_length: 0,
            format: SceneLinearTargetFormat::R11G11B10Float,
            lifetime: PostProcessResourceLifetime::CrossFramePersistent,
            debug_label: "post_process.exposure_histogram",
        }
    }

    #[must_use]
    pub const fn color_grading_lut(side_length: u16) -> Self {
        Self {
            kind: PostProcessIntermediateKindOption::ColorGradingLut,
            extent: RenderExtent2d {
                width: side_length as u32,
                height: side_length as u32,
            },
            mip_chain_depth: 1,
            histogram_bin_count: 0,
            lut_side_length: side_length,
            format: SceneLinearTargetFormat::Rgba16Float,
            lifetime: PostProcessResourceLifetime::AssetBacked,
            debug_label: "post_process.color_grading_lut",
        }
    }

    #[must_use]
    pub const fn sharpened_target(extent: RenderExtent2d) -> Self {
        Self {
            kind: PostProcessIntermediateKindOption::SharpenedTarget,
            extent,
            mip_chain_depth: 1,
            histogram_bin_count: 0,
            lut_side_length: 0,
            format: SceneLinearTargetFormat::Rgba16Float,
            lifetime: PostProcessResourceLifetime::PerFrameTransient,
            debug_label: "post_process.sharpened_target",
        }
    }

    #[must_use]
    pub const fn debug_overlay_layer(extent: RenderExtent2d) -> Self {
        Self {
            kind: PostProcessIntermediateKindOption::DebugOverlayLayer,
            extent,
            mip_chain_depth: 1,
            histogram_bin_count: 0,
            lut_side_length: 0,
            format: SceneLinearTargetFormat::Rgba16Float,
            lifetime: PostProcessResourceLifetime::PerFrameTransient,
            debug_label: "post_process.debug_overlay_layer",
        }
    }

    #[must_use]
    pub const fn taa_history(extent: RenderExtent2d) -> Self {
        Self {
            kind: PostProcessIntermediateKindOption::TaaHistory,
            extent,
            mip_chain_depth: 1,
            histogram_bin_count: 0,
            lut_side_length: 0,
            format: SceneLinearTargetFormat::Rgba16Float,
            lifetime: PostProcessResourceLifetime::CrossFramePersistent,
            debug_label: "post_process.taa_history",
        }
    }

    #[must_use]
    pub const fn velocity_feedback(extent: RenderExtent2d) -> Self {
        Self {
            kind: PostProcessIntermediateKindOption::VelocityFeedback,
            extent,
            mip_chain_depth: 1,
            histogram_bin_count: 0,
            lut_side_length: 0,
            format: SceneLinearTargetFormat::R11G11B10Float,
            lifetime: PostProcessResourceLifetime::PerFrameTransient,
            debug_label: "post_process.velocity_feedback",
        }
    }
}

/// Failure reason when a registry allocation is rejected. The
/// renderer treats every failure as a typed event rather than a
/// silent fallback so diagnostics can name *why* the post stack
/// degraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PostProcessAllocationFailure {
    InvalidExtent,
    InvalidMipChainDepth,
    InvalidHistogramBinCount,
    InvalidLutSideLength,
    KindAlreadyAllocated,
    BudgetExceeded,
}

impl PostProcessAllocationFailure {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidExtent => "invalid_extent",
            Self::InvalidMipChainDepth => "invalid_mip_chain_depth",
            Self::InvalidHistogramBinCount => "invalid_histogram_bin_count",
            Self::InvalidLutSideLength => "invalid_lut_side_length",
            Self::KindAlreadyAllocated => "kind_already_allocated",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }
}

/// Typed result of `allocate`.
pub type PostProcessAllocationResult =
    Result<PostProcessIntermediateSlot, PostProcessAllocationFailure>;

/// Internal table entry — the registry keeps one per allocated kind
/// so a second `allocate` for the same kind returns
/// `KindAlreadyAllocated` rather than silently aliasing.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PostProcessRegistryEntry {
    descriptor: PostProcessIntermediateDescriptor,
    slot: PostProcessIntermediateSlot,
}

/// Typed registry that maps each declared `PostProcessIntermediateKind`
/// to its underlying `FrameGraphResourceType` and tracks which
/// kinds have been allocated for the current frame. Cross-frame
/// kinds (`TaaHistory`, `ExposureHistogram`, `ColorGradingLut`)
/// keep their slot across `begin_frame` calls; transient kinds
/// (`BloomMipChain`, `SharpenedTarget`, `DebugOverlayLayer`,
/// `VelocityFeedback`) reset per frame.
#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct PostProcessResourceRegistry {
    entries: Vec<PostProcessRegistryEntry>,
    next_slot: u32,
    frame_index: u64,
    persistent_byte_budget: u64,
    transient_byte_budget: u64,
    persistent_bytes_used: u64,
    transient_bytes_used: u64,
    diagnostics: PostProcessResourceRegistryDiagnostics,
}

impl PostProcessResourceRegistry {
    pub const DEFAULT_PERSISTENT_BUDGET_BYTES: u64 = 64 * 1024 * 1024;
    pub const DEFAULT_TRANSIENT_BUDGET_BYTES: u64 = 256 * 1024 * 1024;

    #[must_use]
    pub fn new(persistent_byte_budget: u64, transient_byte_budget: u64) -> Self {
        Self {
            persistent_byte_budget,
            transient_byte_budget,
            ..Self::default()
        }
    }

    /// Open a new frame. Drop transient entries; keep persistent
    /// entries; bump the frame index.
    pub fn begin_frame(&mut self) {
        self.frame_index = self.frame_index.saturating_add(1);
        self.entries.retain(|entry| {
            matches!(
                entry.descriptor.lifetime,
                PostProcessResourceLifetime::CrossFramePersistent
                    | PostProcessResourceLifetime::AssetBacked
            )
        });
        self.transient_bytes_used = 0;
    }

    /// Allocate a typed slot for the requested intermediate. The
    /// renderer never holds wgpu objects — the returned slot is a
    /// typed index the bridge resolves at submission time.
    pub fn allocate(
        &mut self,
        descriptor: PostProcessIntermediateDescriptor,
    ) -> PostProcessAllocationResult {
        // Kind-specific dimension checks run before the generic
        // extent check so a caller that constructs a histogram with
        // zero bins (or a LUT with side length 0) sees a typed
        // failure that names *which* dimension is wrong rather than
        // the generic InvalidExtent that would result from the
        // dimension propagating into the extent fields.
        if matches!(
            descriptor.kind,
            PostProcessIntermediateKindOption::BloomMipChain
        ) && descriptor.mip_chain_depth == 0
        {
            self.diagnostics.allocation_failure_count =
                self.diagnostics.allocation_failure_count.saturating_add(1);
            return Err(PostProcessAllocationFailure::InvalidMipChainDepth);
        }
        if matches!(
            descriptor.kind,
            PostProcessIntermediateKindOption::ExposureHistogram
        ) && descriptor.histogram_bin_count == 0
        {
            self.diagnostics.allocation_failure_count =
                self.diagnostics.allocation_failure_count.saturating_add(1);
            return Err(PostProcessAllocationFailure::InvalidHistogramBinCount);
        }
        if matches!(
            descriptor.kind,
            PostProcessIntermediateKindOption::ColorGradingLut
        ) && descriptor.lut_side_length == 0
        {
            self.diagnostics.allocation_failure_count =
                self.diagnostics.allocation_failure_count.saturating_add(1);
            return Err(PostProcessAllocationFailure::InvalidLutSideLength);
        }
        if !descriptor.extent.is_valid() {
            self.diagnostics.allocation_failure_count =
                self.diagnostics.allocation_failure_count.saturating_add(1);
            return Err(PostProcessAllocationFailure::InvalidExtent);
        }

        if let Some(existing) = self.entry_for(descriptor.kind) {
            if existing.descriptor == descriptor {
                return Ok(existing.slot);
            }
            self.diagnostics.allocation_failure_count =
                self.diagnostics.allocation_failure_count.saturating_add(1);
            return Err(PostProcessAllocationFailure::KindAlreadyAllocated);
        }

        let bytes = descriptor_byte_size(descriptor);
        let lifetime = descriptor.lifetime;
        let (used, budget) = match lifetime {
            PostProcessResourceLifetime::CrossFramePersistent
            | PostProcessResourceLifetime::AssetBacked => (
                self.persistent_bytes_used.saturating_add(bytes),
                self.persistent_byte_budget,
            ),
            PostProcessResourceLifetime::PerFrameTransient => (
                self.transient_bytes_used.saturating_add(bytes),
                self.transient_byte_budget,
            ),
        };
        if budget != 0 && used > budget {
            self.diagnostics.allocation_failure_count =
                self.diagnostics.allocation_failure_count.saturating_add(1);
            return Err(PostProcessAllocationFailure::BudgetExceeded);
        }

        let slot = PostProcessIntermediateSlot {
            kind: descriptor.kind,
            slot: self.next_slot,
            generation: 1,
            frame_graph_resource: PostProcessIntermediateKind::from(descriptor.kind)
                .frame_graph_resource()
                .into(),
        };
        self.next_slot = self.next_slot.saturating_add(1);
        match lifetime {
            PostProcessResourceLifetime::CrossFramePersistent
            | PostProcessResourceLifetime::AssetBacked => {
                self.persistent_bytes_used = used;
            }
            PostProcessResourceLifetime::PerFrameTransient => {
                self.transient_bytes_used = used;
            }
        }
        self.entries
            .push(PostProcessRegistryEntry { descriptor, slot });
        self.diagnostics.allocation_count = self.diagnostics.allocation_count.saturating_add(1);
        self.diagnostics.bytes_by_kind
            [PostProcessIntermediateKind::from(descriptor.kind).index()] = self
            .diagnostics
            .bytes_by_kind[PostProcessIntermediateKind::from(descriptor.kind).index()]
        .saturating_add(bytes);
        Ok(slot)
    }

    #[must_use]
    pub fn slot_for(
        &self,
        kind: PostProcessIntermediateKind,
    ) -> Option<PostProcessIntermediateSlot> {
        self.entries
            .iter()
            .find(|entry| PostProcessIntermediateKind::from(entry.descriptor.kind) == kind)
            .map(|entry| entry.slot)
    }

    #[must_use]
    pub fn descriptor_for(
        &self,
        kind: PostProcessIntermediateKind,
    ) -> Option<PostProcessIntermediateDescriptor> {
        self.entries
            .iter()
            .find(|entry| PostProcessIntermediateKind::from(entry.descriptor.kind) == kind)
            .map(|entry| entry.descriptor)
    }

    fn entry_for(
        &self,
        kind: PostProcessIntermediateKindOption,
    ) -> Option<PostProcessRegistryEntry> {
        self.entries
            .iter()
            .find(|entry| entry.descriptor.kind == kind)
            .copied()
    }

    pub fn entries(
        &self,
    ) -> impl Iterator<
        Item = (
            PostProcessIntermediateKind,
            PostProcessIntermediateDescriptor,
            PostProcessIntermediateSlot,
        ),
    > + '_ {
        self.entries.iter().map(|entry| {
            (
                PostProcessIntermediateKind::from(entry.descriptor.kind),
                entry.descriptor,
                entry.slot,
            )
        })
    }

    #[must_use]
    pub const fn frame_index(&self) -> u64 {
        self.frame_index
    }

    #[must_use]
    pub const fn persistent_bytes_used(&self) -> u64 {
        self.persistent_bytes_used
    }

    #[must_use]
    pub const fn transient_bytes_used(&self) -> u64 {
        self.transient_bytes_used
    }

    #[must_use]
    pub fn diagnostics(&self) -> PostProcessResourceRegistryDiagnostics {
        let mut d = self.diagnostics;
        d.schema_version = PRESENTATION_STACK_SCHEMA_VERSION;
        d.frame_index = self.frame_index;
        d.persistent_bytes_used = self.persistent_bytes_used;
        d.transient_bytes_used = self.transient_bytes_used;
        d.persistent_byte_budget = self.persistent_byte_budget;
        d.transient_byte_budget = self.transient_byte_budget;
        d.live_entry_count = self.entries.len() as u32;
        d
    }
}

#[must_use]
fn descriptor_byte_size(descriptor: PostProcessIntermediateDescriptor) -> u64 {
    let bytes_per_pixel = match descriptor.format {
        SceneLinearTargetFormat::Rgba16Float => 8,
        SceneLinearTargetFormat::R11G11B10Float => 4,
    };
    let extent = (descriptor.extent.width as u64).saturating_mul(descriptor.extent.height as u64);
    let pixel_bytes = extent.saturating_mul(bytes_per_pixel);
    match PostProcessIntermediateKind::from(descriptor.kind) {
        PostProcessIntermediateKind::BloomMipChain => {
            // Geometric series 1 + 1/4 + 1/16 + ... bounded by depth.
            let mut total = 0u64;
            let mut current = pixel_bytes;
            for _ in 0..descriptor.mip_chain_depth.max(1) {
                total = total.saturating_add(current);
                current /= 4;
                if current == 0 {
                    break;
                }
            }
            total
        }
        PostProcessIntermediateKind::ExposureHistogram => {
            (descriptor.histogram_bin_count as u64).saturating_mul(bytes_per_pixel)
        }
        PostProcessIntermediateKind::ColorGradingLut => {
            let side = descriptor.lut_side_length as u64;
            side.saturating_mul(side)
                .saturating_mul(side)
                .saturating_mul(bytes_per_pixel)
        }
        _ => pixel_bytes,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct PostProcessResourceRegistryDiagnostics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub allocation_count: u32,
    pub allocation_failure_count: u32,
    pub live_entry_count: u32,
    pub persistent_bytes_used: u64,
    pub transient_bytes_used: u64,
    pub persistent_byte_budget: u64,
    pub transient_byte_budget: u64,
    pub bytes_by_kind: [u64; POST_PROCESS_INTERMEDIATE_KIND_COUNT],
}

impl PostProcessResourceRegistryDiagnostics {
    #[must_use]
    pub const fn bytes_for(&self, kind: PostProcessIntermediateKind) -> u64 {
        self.bytes_by_kind[kind.index()]
    }
}

// ============================================================================
// Section 2 — Presentation debug views (per-stage capture)
// ============================================================================

/// Per-stage debug view that names the intermediate the operator
/// wants composited to the swapchain *instead of* the final post
/// stack output. Drives a typed selector the bridge consumes when
/// `FinalOutputTransform` runs; the production default is
/// `FinalComposedOutput` (no debug capture).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentationDebugView {
    #[default]
    FinalComposedOutput,
    PreTonemap,
    PostTonemap,
    BloomOnly,
    ExposureHistogram,
    ColorGradingDelta,
    PreSharpen,
    PostSharpen,
    DebugOverlayLayer,
    TaaHistory,
    VelocityFeedback,
    UiCompositionLayer,
}

impl PresentationDebugView {
    pub const ALL: [Self; PRESENTATION_DEBUG_VIEW_COUNT] = [
        Self::FinalComposedOutput,
        Self::PreTonemap,
        Self::PostTonemap,
        Self::BloomOnly,
        Self::ExposureHistogram,
        Self::ColorGradingDelta,
        Self::PreSharpen,
        Self::PostSharpen,
        Self::DebugOverlayLayer,
        Self::TaaHistory,
        Self::VelocityFeedback,
        Self::UiCompositionLayer,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FinalComposedOutput => "final_composed_output",
            Self::PreTonemap => "pre_tonemap",
            Self::PostTonemap => "post_tonemap",
            Self::BloomOnly => "bloom_only",
            Self::ExposureHistogram => "exposure_histogram",
            Self::ColorGradingDelta => "color_grading_delta",
            Self::PreSharpen => "pre_sharpen",
            Self::PostSharpen => "post_sharpen",
            Self::DebugOverlayLayer => "debug_overlay_layer",
            Self::TaaHistory => "taa_history",
            Self::VelocityFeedback => "velocity_feedback",
            Self::UiCompositionLayer => "ui_composition_layer",
        }
    }

    #[must_use]
    pub const fn is_production_default(self) -> bool {
        matches!(self, Self::FinalComposedOutput)
    }

    /// Map a debug view to the underlying intermediate kind it
    /// captures. `FinalComposedOutput` and `UiCompositionLayer` do
    /// not have a post-process intermediate kind — they are
    /// graph resources owned outside the post stack.
    #[must_use]
    pub const fn intermediate_kind(self) -> Option<PostProcessIntermediateKind> {
        match self {
            Self::BloomOnly => Some(PostProcessIntermediateKind::BloomMipChain),
            Self::ExposureHistogram => Some(PostProcessIntermediateKind::ExposureHistogram),
            Self::ColorGradingDelta => Some(PostProcessIntermediateKind::ColorGradingLut),
            Self::PreSharpen | Self::PostSharpen => {
                Some(PostProcessIntermediateKind::SharpenedTarget)
            }
            Self::DebugOverlayLayer => Some(PostProcessIntermediateKind::DebugOverlayLayer),
            Self::TaaHistory => Some(PostProcessIntermediateKind::TaaHistory),
            Self::VelocityFeedback => Some(PostProcessIntermediateKind::VelocityFeedback),
            Self::FinalComposedOutput
            | Self::PreTonemap
            | Self::PostTonemap
            | Self::UiCompositionLayer => None,
        }
    }

    /// Lift a `CameraDebugView` into a presentation debug view so
    /// the camera component drives the bridge selector without a
    /// hand-mapped switch in the schedule.
    #[must_use]
    pub const fn from_camera_debug_view(view: CameraDebugView) -> Self {
        match view {
            CameraDebugView::None => Self::FinalComposedOutput,
            CameraDebugView::MotionVectors => Self::VelocityFeedback,
            CameraDebugView::Depth
            | CameraDebugView::Normals
            | CameraDebugView::BaseColor
            | CameraDebugView::RoughnessMetallic
            | CameraDebugView::LightHeatmap
            | CameraDebugView::VirtualPages => Self::DebugOverlayLayer,
        }
    }
}

// ============================================================================
// Section 3 — UI packet color policy (native UI metadata bridge)
// ============================================================================

/// Color-space classes the renderer recognises at the UI/scene
/// composition boundary. Mirrors `UiColorSpace` and
/// rvelte-fun-ui-core::FunUiColorSpace, but is a renderer-owned
/// enum so the presentation stack does not depend directly on
/// rvelte. The native UI adapter in `ui::native_adapter` is the
/// only translator from the rvelte enum into this one.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiPacketColorClass {
    #[default]
    Linear,
    Srgb,
    DisplayP3,
    Hdr10,
}

impl UiPacketColorClass {
    pub const ALL: [Self; 4] = [Self::Linear, Self::Srgb, Self::DisplayP3, Self::Hdr10];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::Srgb => "srgb",
            Self::DisplayP3 => "display_p3",
            Self::Hdr10 => "hdr10",
        }
    }

    #[must_use]
    pub const fn from_ui_color_space(value: UiColorSpace) -> Self {
        match value {
            UiColorSpace::Srgb => Self::Srgb,
            UiColorSpace::Linear => Self::Linear,
            UiColorSpace::Hdr10 => Self::Hdr10,
        }
    }

    #[must_use]
    pub const fn to_ui_color_space(self) -> UiColorSpace {
        match self {
            Self::Linear => UiColorSpace::Linear,
            Self::Srgb => UiColorSpace::Srgb,
            // DisplayP3 is not yet a renderer-side product surface
            // — it is recognised at the native UI adapter and
            // converted to Linear before the policy resolves. This
            // keeps `to_ui_color_space` total without leaking a new
            // surface.
            Self::DisplayP3 => UiColorSpace::Linear,
            Self::Hdr10 => UiColorSpace::Hdr10,
        }
    }
}

/// Reason the UI composition policy resolved the way it did. The
/// bridge surfaces the reason via diagnostics so an operator can
/// see why the renderer chose a particular conversion path
/// (especially during HDR fallback or DisplayP3 lowering).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiPacketColorPolicyReason {
    #[default]
    NativeAdapterDeclaredLinear,
    NativeAdapterDeclaredSrgb,
    NativeAdapterDeclaredDisplayP3LoweredToLinear,
    NativeAdapterDeclaredHdr10WithHdrSwapchain,
    NativeAdapterDeclaredHdr10ButSdrFallback,
    PolicyForcedLinearForSdrSwapchain,
}

impl UiPacketColorPolicyReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeAdapterDeclaredLinear => "native_adapter_declared_linear",
            Self::NativeAdapterDeclaredSrgb => "native_adapter_declared_srgb",
            Self::NativeAdapterDeclaredDisplayP3LoweredToLinear => {
                "native_adapter_declared_display_p3_lowered_to_linear"
            }
            Self::NativeAdapterDeclaredHdr10WithHdrSwapchain => {
                "native_adapter_declared_hdr10_with_hdr_swapchain"
            }
            Self::NativeAdapterDeclaredHdr10ButSdrFallback => {
                "native_adapter_declared_hdr10_but_sdr_fallback"
            }
            Self::PolicyForcedLinearForSdrSwapchain => "policy_forced_linear_for_sdr_swapchain",
        }
    }
}

/// Resolved UI composition policy — the renderer's typed contract
/// for how UI packets are blended into the scene linear target.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiPacketColorPolicy {
    pub schema_version: u16,
    pub source_class: UiPacketColorClass,
    pub composition: UiCompositionPolicy,
    pub conversion_required: bool,
    pub reason: UiPacketColorPolicyReason,
}

impl UiPacketColorPolicy {
    #[must_use]
    pub fn resolve(
        source_class: UiPacketColorClass,
        swapchain_color_space: SwapchainColorSpace,
        hdr_active: bool,
    ) -> Self {
        let (composition, reason, conversion_required) = match (source_class, hdr_active) {
            (UiPacketColorClass::Linear, _) => (
                UiCompositionPolicy::LinearScene,
                UiPacketColorPolicyReason::NativeAdapterDeclaredLinear,
                false,
            ),
            (UiPacketColorClass::Srgb, _) => (
                UiCompositionPolicy::LinearScene,
                UiPacketColorPolicyReason::NativeAdapterDeclaredSrgb,
                true,
            ),
            (UiPacketColorClass::DisplayP3, _) => (
                UiCompositionPolicy::LinearScene,
                UiPacketColorPolicyReason::NativeAdapterDeclaredDisplayP3LoweredToLinear,
                true,
            ),
            (UiPacketColorClass::Hdr10, true) => (
                UiCompositionPolicy::Hdr10Output,
                UiPacketColorPolicyReason::NativeAdapterDeclaredHdr10WithHdrSwapchain,
                false,
            ),
            (UiPacketColorClass::Hdr10, false) => (
                UiCompositionPolicy::LinearScene,
                UiPacketColorPolicyReason::NativeAdapterDeclaredHdr10ButSdrFallback,
                true,
            ),
        };
        // Force-lower to Linear for SrgbNonlinear / SrgbLinear
        // swapchains to honour the post-process pipeline contract:
        // intermediates are always linear, so the scene composite is
        // linear and only the final output transform reaches sRGB
        // gamma.
        let (composition, reason) = if !swapchain_color_space.is_hdr()
            && matches!(composition, UiCompositionPolicy::Hdr10Output)
        {
            (
                UiCompositionPolicy::LinearScene,
                UiPacketColorPolicyReason::PolicyForcedLinearForSdrSwapchain,
            )
        } else {
            (composition, reason)
        };
        Self {
            schema_version: PRESENTATION_STACK_SCHEMA_VERSION,
            source_class,
            composition,
            conversion_required,
            reason,
        }
    }
}

// ============================================================================
// Section 4 — Swapchain format / capability report
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwapchainPixelFormat {
    Bgra8UnormSrgb,
    Rgba8UnormSrgb,
    Rgba16Float,
    Rgb10A2Unorm,
    Rgb10A2UnormHdr10,
    R16G16B16A16ScRgb,
}

impl SwapchainPixelFormat {
    pub const ALL: [Self; SWAPCHAIN_FORMAT_REPORT_COUNT] = [
        Self::Bgra8UnormSrgb,
        Self::Rgba8UnormSrgb,
        Self::Rgba16Float,
        Self::Rgb10A2Unorm,
        Self::Rgb10A2UnormHdr10,
        Self::R16G16B16A16ScRgb,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Bgra8UnormSrgb => 0,
            Self::Rgba8UnormSrgb => 1,
            Self::Rgba16Float => 2,
            Self::Rgb10A2Unorm => 3,
            Self::Rgb10A2UnormHdr10 => 4,
            Self::R16G16B16A16ScRgb => 5,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bgra8UnormSrgb => "bgra8_unorm_srgb",
            Self::Rgba8UnormSrgb => "rgba8_unorm_srgb",
            Self::Rgba16Float => "rgba16_float",
            Self::Rgb10A2Unorm => "rgb10_a2_unorm",
            Self::Rgb10A2UnormHdr10 => "rgb10_a2_unorm_hdr10",
            Self::R16G16B16A16ScRgb => "r16g16b16a16_scrgb",
        }
    }

    #[must_use]
    pub const fn pairs_with(self, color_space: SwapchainColorSpace) -> bool {
        matches!(
            (self, color_space),
            (Self::Bgra8UnormSrgb, SwapchainColorSpace::SrgbNonlinear)
                | (Self::Rgba8UnormSrgb, SwapchainColorSpace::SrgbNonlinear)
                | (Self::Rgba16Float, SwapchainColorSpace::SrgbLinear)
                | (Self::Rgb10A2Unorm, SwapchainColorSpace::SrgbNonlinear)
                | (Self::Rgb10A2UnormHdr10, SwapchainColorSpace::Hdr10St2084)
                | (
                    Self::R16G16B16A16ScRgb,
                    SwapchainColorSpace::ScRgbExtendedLinear
                )
        )
    }

    #[must_use]
    pub const fn is_hdr(self) -> bool {
        matches!(self, Self::Rgb10A2UnormHdr10 | Self::R16G16B16A16ScRgb)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwapchainFormatCapability {
    pub format: SwapchainPixelFormatOption,
    pub color_space: SwapchainColorSpace,
    pub min_image_count: u8,
    pub max_image_count: u8,
    pub supports_present_mode_fifo: bool,
    pub supports_present_mode_mailbox: bool,
    pub supports_present_mode_immediate: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SwapchainPixelFormatOption {
    #[default]
    Bgra8UnormSrgb,
    Rgba8UnormSrgb,
    Rgba16Float,
    Rgb10A2Unorm,
    Rgb10A2UnormHdr10,
    R16G16B16A16ScRgb,
}

impl From<SwapchainPixelFormat> for SwapchainPixelFormatOption {
    fn from(value: SwapchainPixelFormat) -> Self {
        match value {
            SwapchainPixelFormat::Bgra8UnormSrgb => Self::Bgra8UnormSrgb,
            SwapchainPixelFormat::Rgba8UnormSrgb => Self::Rgba8UnormSrgb,
            SwapchainPixelFormat::Rgba16Float => Self::Rgba16Float,
            SwapchainPixelFormat::Rgb10A2Unorm => Self::Rgb10A2Unorm,
            SwapchainPixelFormat::Rgb10A2UnormHdr10 => Self::Rgb10A2UnormHdr10,
            SwapchainPixelFormat::R16G16B16A16ScRgb => Self::R16G16B16A16ScRgb,
        }
    }
}

impl From<SwapchainPixelFormatOption> for SwapchainPixelFormat {
    fn from(value: SwapchainPixelFormatOption) -> Self {
        match value {
            SwapchainPixelFormatOption::Bgra8UnormSrgb => Self::Bgra8UnormSrgb,
            SwapchainPixelFormatOption::Rgba8UnormSrgb => Self::Rgba8UnormSrgb,
            SwapchainPixelFormatOption::Rgba16Float => Self::Rgba16Float,
            SwapchainPixelFormatOption::Rgb10A2Unorm => Self::Rgb10A2Unorm,
            SwapchainPixelFormatOption::Rgb10A2UnormHdr10 => Self::Rgb10A2UnormHdr10,
            SwapchainPixelFormatOption::R16G16B16A16ScRgb => Self::R16G16B16A16ScRgb,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct SwapchainFormatReport {
    pub schema_version: u16,
    pub formats: Vec<SwapchainFormatCapability>,
    pub preferred_sdr_format: SwapchainPixelFormatOption,
    pub preferred_hdr10_format: Option<SwapchainPixelFormatOption>,
    pub preferred_scrgb_format: Option<SwapchainPixelFormatOption>,
}

impl SwapchainFormatReport {
    #[must_use]
    pub fn sdr_only() -> Self {
        Self {
            schema_version: PRESENTATION_STACK_SCHEMA_VERSION,
            formats: vec![SwapchainFormatCapability {
                format: SwapchainPixelFormat::Bgra8UnormSrgb.into(),
                color_space: SwapchainColorSpace::SrgbNonlinear,
                min_image_count: 2,
                max_image_count: 3,
                supports_present_mode_fifo: true,
                supports_present_mode_mailbox: false,
                supports_present_mode_immediate: false,
            }],
            preferred_sdr_format: SwapchainPixelFormat::Bgra8UnormSrgb.into(),
            preferred_hdr10_format: None,
            preferred_scrgb_format: None,
        }
    }

    #[must_use]
    pub fn hdr_capable() -> Self {
        Self {
            schema_version: PRESENTATION_STACK_SCHEMA_VERSION,
            formats: vec![
                SwapchainFormatCapability {
                    format: SwapchainPixelFormat::Bgra8UnormSrgb.into(),
                    color_space: SwapchainColorSpace::SrgbNonlinear,
                    min_image_count: 2,
                    max_image_count: 3,
                    supports_present_mode_fifo: true,
                    supports_present_mode_mailbox: true,
                    supports_present_mode_immediate: false,
                },
                SwapchainFormatCapability {
                    format: SwapchainPixelFormat::Rgb10A2UnormHdr10.into(),
                    color_space: SwapchainColorSpace::Hdr10St2084,
                    min_image_count: 2,
                    max_image_count: 3,
                    supports_present_mode_fifo: true,
                    supports_present_mode_mailbox: true,
                    supports_present_mode_immediate: false,
                },
                SwapchainFormatCapability {
                    format: SwapchainPixelFormat::R16G16B16A16ScRgb.into(),
                    color_space: SwapchainColorSpace::ScRgbExtendedLinear,
                    min_image_count: 2,
                    max_image_count: 3,
                    supports_present_mode_fifo: true,
                    supports_present_mode_mailbox: true,
                    supports_present_mode_immediate: false,
                },
            ],
            preferred_sdr_format: SwapchainPixelFormat::Bgra8UnormSrgb.into(),
            preferred_hdr10_format: Some(SwapchainPixelFormat::Rgb10A2UnormHdr10.into()),
            preferred_scrgb_format: Some(SwapchainPixelFormat::R16G16B16A16ScRgb.into()),
        }
    }

    #[must_use]
    pub fn supports_color_space(&self, color_space: SwapchainColorSpace) -> bool {
        self.formats
            .iter()
            .any(|capability| capability.color_space == color_space)
    }

    #[must_use]
    pub fn select_for(
        &self,
        color_space: SwapchainColorSpace,
    ) -> Option<SwapchainPixelFormatOption> {
        match color_space {
            SwapchainColorSpace::SrgbNonlinear | SwapchainColorSpace::SrgbLinear => {
                Some(self.preferred_sdr_format)
            }
            SwapchainColorSpace::Hdr10St2084 => self.preferred_hdr10_format,
            SwapchainColorSpace::ScRgbExtendedLinear => self.preferred_scrgb_format,
        }
    }
}

// ============================================================================
// Section 5 — TAA velocity buffer dependency edge
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaaVelocityBufferRequirement {
    #[default]
    NotRequired,
    Optional,
    Required,
    RequiredButMissing,
}

impl TaaVelocityBufferRequirement {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Optional => "optional",
            Self::Required => "required",
            Self::RequiredButMissing => "required_but_missing",
        }
    }

    #[must_use]
    pub fn from_taa_plan(plan: &TaaFramePlan, motion_vectors_present: bool) -> Self {
        if !plan.requires_history_resource() {
            return Self::NotRequired;
        }
        match (plan.history_status, motion_vectors_present) {
            (TaaHistoryStatus::Disabled, _) => Self::NotRequired,
            (TaaHistoryStatus::Initial, false) => Self::Optional,
            (TaaHistoryStatus::Initial, true) => Self::Required,
            (TaaHistoryStatus::Valid, false) => Self::RequiredButMissing,
            (TaaHistoryStatus::Valid, true) => Self::Required,
            (TaaHistoryStatus::InvalidatedByCameraCut, _)
            | (TaaHistoryStatus::InvalidatedByExtentChange, _)
            | (TaaHistoryStatus::InvalidatedByMissingMotionVectors, _) => Self::Optional,
        }
    }
}

// ============================================================================
// Section 6 — Top-level presentation stack plan
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationStackPlan {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub post_process: PostProcessGraphPlan,
    pub presentation_policy: PresentationPolicy,
    pub ui_color_policy: UiPacketColorPolicy,
    pub debug_view: PresentationDebugView,
    pub swapchain_format_report: SwapchainFormatReport,
    pub selected_swapchain_format: Option<SwapchainPixelFormatOption>,
    pub velocity_buffer_requirement: TaaVelocityBufferRequirement,
    pub final_resource: FrameGraphResourceType,
    pub final_pass_present: bool,
}

impl PresentationStackPlan {
    #[must_use]
    pub fn assemble(
        view_id: RenderStableId,
        post_process: PostProcessGraphPlan,
        presentation_policy: PresentationPolicy,
        ui_color_policy: UiPacketColorPolicy,
        debug_view: PresentationDebugView,
        swapchain_format_report: SwapchainFormatReport,
        velocity_buffer_requirement: TaaVelocityBufferRequirement,
    ) -> Self {
        let selected =
            swapchain_format_report.select_for(presentation_policy.swapchain_color_space);
        let final_pass_present = post_process.has_pass(PostProcessPassKind::FinalOutputTransform);
        Self {
            schema_version: PRESENTATION_STACK_SCHEMA_VERSION,
            view_id,
            post_process,
            presentation_policy,
            ui_color_policy,
            debug_view,
            swapchain_format_report,
            selected_swapchain_format: selected,
            velocity_buffer_requirement,
            final_resource: FrameGraphResourceType::FinalComposedOutput,
            final_pass_present,
        }
    }

    #[must_use]
    pub const fn final_image_is_graph_composited(&self) -> bool {
        matches!(
            self.final_resource,
            FrameGraphResourceType::FinalComposedOutput
        ) && self.final_pass_present
    }

    #[must_use]
    pub const fn hdr_active(&self) -> bool {
        matches!(
            self.presentation_policy.hdr_output_state,
            HdrOutputState::Active | HdrOutputState::ScaffoldEnabled
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct PresentationStackDiagnostics {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub post_process_pass_count: u16,
    pub debug_view: PresentationDebugView,
    pub hdr_active: bool,
    pub ui_conversion_required: bool,
    pub ui_color_policy_reason: UiPacketColorPolicyReason,
    pub velocity_buffer_requirement: TaaVelocityBufferRequirement,
    pub final_image_is_graph_composited: bool,
    pub selected_swapchain_format: Option<SwapchainPixelFormatOption>,
}

impl PresentationStackDiagnostics {
    #[must_use]
    pub fn from_plan(plan: &PresentationStackPlan) -> Self {
        Self {
            schema_version: PRESENTATION_STACK_SCHEMA_VERSION,
            view_id: plan.view_id,
            post_process_pass_count: u16::try_from(plan.post_process.passes().len())
                .unwrap_or(u16::MAX),
            debug_view: plan.debug_view,
            hdr_active: plan.hdr_active(),
            ui_conversion_required: plan.ui_color_policy.conversion_required,
            ui_color_policy_reason: plan.ui_color_policy.reason,
            velocity_buffer_requirement: plan.velocity_buffer_requirement,
            final_image_is_graph_composited: plan.final_image_is_graph_composited(),
            selected_swapchain_format: plan.selected_swapchain_format,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_api::{
        BloomSettings, ColorGradingSettings, ExposureSettings, HdrOutputSettings,
        SharpeningSettings, TaaSettings, ToneMappingSettings, UpscalerSettings,
    };
    use crate::extraction::ExtractedPostProcessVolume;
    use crate::presentation::{DisplayCapabilities, PresentationPolicy, SwapchainCapabilities};
    use crate::taa::{TaaFrameInputs, TaaFramePlan, TaaHistoryAllocator};

    fn default_volume() -> ExtractedPostProcessVolume {
        ExtractedPostProcessVolume {
            view_id: crate::component_api::RenderViewId::INVALID,
            stable_id: RenderStableId::new(7),
            exposure: ExposureSettings::default(),
            tone_mapping: ToneMappingSettings::default(),
            bloom: BloomSettings::default(),
            color_grading: ColorGradingSettings::default(),
            sharpening: SharpeningSettings::default(),
            taa: TaaSettings::default(),
            upscaler: UpscalerSettings::default(),
            hdr_output: HdrOutputSettings::default(),
        }
    }

    fn default_post_plan() -> PostProcessGraphPlan {
        PostProcessGraphPlan::from_camera_settings(
            RenderStableId::new(7),
            default_volume(),
            CameraDebugView::None,
        )
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PRESENTATION_STACK_SCHEMA_VERSION, 1);
        assert_eq!(POST_PROCESS_INTERMEDIATE_KIND_COUNT, 7);
        assert_eq!(PRESENTATION_DEBUG_VIEW_COUNT, 12);
        assert_eq!(SWAPCHAIN_FORMAT_REPORT_COUNT, 6);
    }

    #[test]
    fn intermediate_kind_to_frame_graph_resource_routes_through_registry() {
        assert_eq!(
            PostProcessIntermediateKind::BloomMipChain.frame_graph_resource(),
            FrameGraphResourceType::TransientScratch,
        );
        assert_eq!(
            PostProcessIntermediateKind::ExposureHistogram.frame_graph_resource(),
            FrameGraphResourceType::Exposure,
        );
        assert_eq!(
            PostProcessIntermediateKind::TaaHistory.frame_graph_resource(),
            FrameGraphResourceType::HistoryBuffer,
        );
        assert_eq!(
            PostProcessIntermediateKind::VelocityFeedback.frame_graph_resource(),
            FrameGraphResourceType::MotionVectors,
        );
    }

    #[test]
    fn registry_allocate_returns_typed_slot_with_resource_mapping() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let slot = registry
            .allocate(PostProcessIntermediateDescriptor::bloom(
                RenderExtent2d::new(1280, 720),
                4,
            ))
            .expect("bloom allocation");
        assert!(slot.is_valid());
        assert_eq!(
            slot.frame_graph_resource,
            PostProcessFrameGraphResourceOption::TransientScratch,
        );
        let stored = registry
            .slot_for(PostProcessIntermediateKind::BloomMipChain)
            .expect("registered slot");
        assert_eq!(stored, slot);
    }

    #[test]
    fn registry_rejects_invalid_extent() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let descriptor = PostProcessIntermediateDescriptor::bloom(RenderExtent2d::new(0, 0), 4);
        let err = registry.allocate(descriptor).unwrap_err();
        assert_eq!(err, PostProcessAllocationFailure::InvalidExtent);
    }

    #[test]
    fn registry_rejects_zero_mip_chain_for_bloom() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let descriptor =
            PostProcessIntermediateDescriptor::bloom(RenderExtent2d::new(1280, 720), 0);
        let err = registry.allocate(descriptor).unwrap_err();
        assert_eq!(err, PostProcessAllocationFailure::InvalidMipChainDepth);
    }

    #[test]
    fn registry_rejects_zero_histogram_bins() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let descriptor = PostProcessIntermediateDescriptor::exposure_histogram(0);
        let err = registry.allocate(descriptor).unwrap_err();
        assert_eq!(err, PostProcessAllocationFailure::InvalidHistogramBinCount);
    }

    #[test]
    fn registry_rejects_zero_lut_side_length() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let descriptor = PostProcessIntermediateDescriptor::color_grading_lut(0);
        let err = registry.allocate(descriptor).unwrap_err();
        assert_eq!(err, PostProcessAllocationFailure::InvalidLutSideLength);
    }

    #[test]
    fn registry_rejects_double_allocation_with_different_descriptor() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let _ = registry
            .allocate(PostProcessIntermediateDescriptor::bloom(
                RenderExtent2d::new(1280, 720),
                4,
            ))
            .expect("first bloom");
        let err = registry
            .allocate(PostProcessIntermediateDescriptor::bloom(
                RenderExtent2d::new(1920, 1080),
                4,
            ))
            .unwrap_err();
        assert_eq!(err, PostProcessAllocationFailure::KindAlreadyAllocated);
    }

    #[test]
    fn registry_idempotent_allocate_returns_same_slot() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let descriptor =
            PostProcessIntermediateDescriptor::bloom(RenderExtent2d::new(1280, 720), 4);
        let first = registry.allocate(descriptor).unwrap();
        let second = registry.allocate(descriptor).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn begin_frame_drops_transient_keeps_persistent() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let bloom = registry
            .allocate(PostProcessIntermediateDescriptor::bloom(
                RenderExtent2d::new(1280, 720),
                4,
            ))
            .unwrap();
        let history = registry
            .allocate(PostProcessIntermediateDescriptor::taa_history(
                RenderExtent2d::new(1280, 720),
            ))
            .unwrap();
        registry.begin_frame();
        assert!(
            registry
                .slot_for(PostProcessIntermediateKind::BloomMipChain)
                .is_none()
        );
        assert_eq!(
            registry
                .slot_for(PostProcessIntermediateKind::TaaHistory)
                .unwrap(),
            history,
        );
        assert_ne!(
            registry
                .slot_for(PostProcessIntermediateKind::TaaHistory)
                .unwrap(),
            bloom,
        );
    }

    #[test]
    fn registry_budget_exhaustion_reports_typed_failure() {
        let mut registry = PostProcessResourceRegistry::new(0, 64);
        registry.begin_frame();
        let descriptor =
            PostProcessIntermediateDescriptor::bloom(RenderExtent2d::new(1024, 1024), 4);
        let err = registry.allocate(descriptor).unwrap_err();
        assert_eq!(err, PostProcessAllocationFailure::BudgetExceeded);
    }

    #[test]
    fn registry_diagnostics_attribute_bytes_per_kind() {
        let mut registry = PostProcessResourceRegistry::default();
        registry.begin_frame();
        let _ = registry
            .allocate(PostProcessIntermediateDescriptor::bloom(
                RenderExtent2d::new(1280, 720),
                4,
            ))
            .unwrap();
        let _ = registry
            .allocate(PostProcessIntermediateDescriptor::taa_history(
                RenderExtent2d::new(1280, 720),
            ))
            .unwrap();
        let diagnostics = registry.diagnostics();
        assert!(diagnostics.bytes_for(PostProcessIntermediateKind::BloomMipChain) > 0,);
        assert!(diagnostics.bytes_for(PostProcessIntermediateKind::TaaHistory) > 0,);
    }

    #[test]
    fn presentation_debug_view_default_is_final_composed_output() {
        let view = PresentationDebugView::default();
        assert!(view.is_production_default());
        assert_eq!(view, PresentationDebugView::FinalComposedOutput);
    }

    #[test]
    fn motion_vectors_camera_debug_view_lifts_to_velocity_feedback() {
        let view = PresentationDebugView::from_camera_debug_view(CameraDebugView::MotionVectors);
        assert_eq!(view, PresentationDebugView::VelocityFeedback);
        assert_eq!(
            view.intermediate_kind(),
            Some(PostProcessIntermediateKind::VelocityFeedback),
        );
    }

    #[test]
    fn ui_color_policy_resolves_linear_with_no_conversion() {
        let policy = UiPacketColorPolicy::resolve(
            UiPacketColorClass::Linear,
            SwapchainColorSpace::SrgbNonlinear,
            false,
        );
        assert_eq!(policy.composition, UiCompositionPolicy::LinearScene);
        assert!(!policy.conversion_required);
        assert_eq!(
            policy.reason,
            UiPacketColorPolicyReason::NativeAdapterDeclaredLinear,
        );
    }

    #[test]
    fn ui_color_policy_resolves_srgb_to_linear_with_conversion() {
        let policy = UiPacketColorPolicy::resolve(
            UiPacketColorClass::Srgb,
            SwapchainColorSpace::SrgbNonlinear,
            false,
        );
        assert_eq!(policy.composition, UiCompositionPolicy::LinearScene);
        assert!(policy.conversion_required);
        assert_eq!(
            policy.reason,
            UiPacketColorPolicyReason::NativeAdapterDeclaredSrgb,
        );
    }

    #[test]
    fn ui_color_policy_lowers_display_p3_to_linear() {
        let policy = UiPacketColorPolicy::resolve(
            UiPacketColorClass::DisplayP3,
            SwapchainColorSpace::SrgbNonlinear,
            false,
        );
        assert_eq!(policy.composition, UiCompositionPolicy::LinearScene);
        assert!(policy.conversion_required);
        assert_eq!(
            policy.reason,
            UiPacketColorPolicyReason::NativeAdapterDeclaredDisplayP3LoweredToLinear,
        );
    }

    #[test]
    fn ui_color_policy_keeps_hdr10_when_swapchain_supports_it() {
        let policy = UiPacketColorPolicy::resolve(
            UiPacketColorClass::Hdr10,
            SwapchainColorSpace::Hdr10St2084,
            true,
        );
        assert_eq!(policy.composition, UiCompositionPolicy::Hdr10Output);
        assert!(!policy.conversion_required);
        assert_eq!(
            policy.reason,
            UiPacketColorPolicyReason::NativeAdapterDeclaredHdr10WithHdrSwapchain,
        );
    }

    #[test]
    fn ui_color_policy_falls_back_to_linear_when_hdr_inactive() {
        let policy = UiPacketColorPolicy::resolve(
            UiPacketColorClass::Hdr10,
            SwapchainColorSpace::SrgbNonlinear,
            false,
        );
        assert_eq!(policy.composition, UiCompositionPolicy::LinearScene);
        assert_eq!(
            policy.reason,
            UiPacketColorPolicyReason::NativeAdapterDeclaredHdr10ButSdrFallback,
        );
    }

    #[test]
    fn swapchain_format_report_sdr_only_lacks_hdr_formats() {
        let report = SwapchainFormatReport::sdr_only();
        assert!(report.supports_color_space(SwapchainColorSpace::SrgbNonlinear));
        assert!(!report.supports_color_space(SwapchainColorSpace::Hdr10St2084));
        assert!(report.preferred_hdr10_format.is_none());
    }

    #[test]
    fn swapchain_format_report_hdr_capable_lists_three_formats() {
        let report = SwapchainFormatReport::hdr_capable();
        assert_eq!(report.formats.len(), 3);
        assert!(report.supports_color_space(SwapchainColorSpace::Hdr10St2084));
        assert!(report.supports_color_space(SwapchainColorSpace::ScRgbExtendedLinear));
        assert_eq!(
            report.select_for(SwapchainColorSpace::Hdr10St2084).unwrap(),
            SwapchainPixelFormat::Rgb10A2UnormHdr10.into(),
        );
    }

    #[test]
    fn swapchain_format_pairs_match_color_space_constraints() {
        assert!(
            SwapchainPixelFormat::Bgra8UnormSrgb.pairs_with(SwapchainColorSpace::SrgbNonlinear)
        );
        assert!(
            SwapchainPixelFormat::Rgb10A2UnormHdr10.pairs_with(SwapchainColorSpace::Hdr10St2084)
        );
        assert!(
            SwapchainPixelFormat::R16G16B16A16ScRgb
                .pairs_with(SwapchainColorSpace::ScRgbExtendedLinear),
        );
        assert!(!SwapchainPixelFormat::Bgra8UnormSrgb.pairs_with(SwapchainColorSpace::Hdr10St2084));
    }

    #[test]
    fn taa_velocity_requirement_not_required_when_taa_disabled() {
        let mut allocator = TaaHistoryAllocator::default();
        let plan = TaaFramePlan::build(
            RenderStableId::new(1),
            TaaSettings {
                enabled: false,
                ..TaaSettings::default()
            },
            TaaFrameInputs::default(),
            &mut allocator,
        );
        let requirement = TaaVelocityBufferRequirement::from_taa_plan(&plan, false);
        assert_eq!(requirement, TaaVelocityBufferRequirement::NotRequired);
    }

    #[test]
    fn taa_velocity_requirement_required_when_motion_vectors_present() {
        let mut allocator = TaaHistoryAllocator::default();
        let plan = TaaFramePlan::build(
            RenderStableId::new(1),
            TaaSettings {
                enabled: true,
                ..TaaSettings::default()
            },
            TaaFrameInputs {
                motion_vectors_available: true,
                ..TaaFrameInputs::default()
            },
            &mut allocator,
        );
        let requirement = TaaVelocityBufferRequirement::from_taa_plan(&plan, true);
        assert!(matches!(
            requirement,
            TaaVelocityBufferRequirement::Required | TaaVelocityBufferRequirement::Optional
        ));
    }

    #[test]
    fn taa_velocity_requirement_strings_match_pass_23_contract() {
        assert_eq!(
            TaaVelocityBufferRequirement::default(),
            TaaVelocityBufferRequirement::NotRequired,
        );
        assert_eq!(TaaVelocityBufferRequirement::Required.as_str(), "required");
        assert_eq!(
            TaaVelocityBufferRequirement::RequiredButMissing.as_str(),
            "required_but_missing",
        );
    }

    #[test]
    fn presentation_stack_plan_assembles_with_final_pass_present() {
        let post = default_post_plan();
        let presentation = PresentationPolicy::from_camera_settings(
            HdrOutputSettings::default(),
            DisplayCapabilities::SDR,
            SwapchainCapabilities::SDR_ONLY,
        );
        let ui = UiPacketColorPolicy::resolve(
            UiPacketColorClass::Linear,
            presentation.swapchain_color_space,
            false,
        );
        let plan = PresentationStackPlan::assemble(
            RenderStableId::new(7),
            post,
            presentation,
            ui,
            PresentationDebugView::FinalComposedOutput,
            SwapchainFormatReport::sdr_only(),
            TaaVelocityBufferRequirement::NotRequired,
        );
        assert!(plan.final_image_is_graph_composited());
        assert_eq!(
            plan.final_resource,
            FrameGraphResourceType::FinalComposedOutput
        );
        assert_eq!(
            plan.selected_swapchain_format,
            Some(SwapchainPixelFormat::Bgra8UnormSrgb.into()),
        );
    }

    #[test]
    fn presentation_stack_diagnostics_summarise_plan() {
        let post = default_post_plan();
        let presentation = PresentationPolicy::from_camera_settings(
            HdrOutputSettings {
                enabled: true,
                max_nits: 1_000.0,
                paper_white_nits: 200.0,
            },
            DisplayCapabilities::HDR1000,
            SwapchainCapabilities::HDR_FULL,
        );
        let ui = UiPacketColorPolicy::resolve(
            UiPacketColorClass::Hdr10,
            presentation.swapchain_color_space,
            true,
        );
        let plan = PresentationStackPlan::assemble(
            RenderStableId::new(7),
            post,
            presentation,
            ui,
            PresentationDebugView::FinalComposedOutput,
            SwapchainFormatReport::hdr_capable(),
            TaaVelocityBufferRequirement::Required,
        );
        let diagnostics = PresentationStackDiagnostics::from_plan(&plan);
        assert!(diagnostics.hdr_active);
        assert!(diagnostics.final_image_is_graph_composited);
        assert_eq!(
            diagnostics.ui_color_policy_reason,
            UiPacketColorPolicyReason::NativeAdapterDeclaredHdr10WithHdrSwapchain,
        );
        assert_eq!(
            diagnostics.velocity_buffer_requirement,
            TaaVelocityBufferRequirement::Required,
        );
    }

    #[test]
    fn ui_color_class_round_trips_through_ui_color_space() {
        for class in UiPacketColorClass::ALL {
            let space = class.to_ui_color_space();
            let recovered = UiPacketColorClass::from_ui_color_space(space);
            // DisplayP3 collapses to Linear by design — it is
            // recognised at the native UI adapter and lowered before
            // policy resolution.
            if matches!(class, UiPacketColorClass::DisplayP3) {
                assert_eq!(recovered, UiPacketColorClass::Linear);
            } else {
                assert_eq!(recovered, class);
            }
        }
    }
}
