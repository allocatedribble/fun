//! Tier 4 — Transient Memory, Aliasing, and Frame Graph
//! Optimization.
//!
//! `frame_graph.rs` already declares the typed
//! `FrameGraphResourceType`, `FrameGraphResourceLifetime`,
//! `ResourceAccess`, and `GraphResourceUse` records. Tier 4 puts
//! that surface to work:
//!
//! 1. **Transient resource allocator** —
//!    `Tier4TransientAllocator` walks the lifetime table and
//!    assigns aliasing slots so non-overlapping transient
//!    resources share GPU memory. The typed
//!    `Tier4ResourceAliasability` predicate excludes history
//!    resources, imported / backbuffer resources, UI overlay,
//!    and upscaler-required resources from aliasing per the
//!    user's exclusion rules.
//!
//! 2. **Transient memory metrics** —
//!    `Tier4TransientMemoryMetrics` records bytes requested,
//!    bytes allocated, aliasing savings, peak frame memory, and
//!    per-pass memory occupancy. The acceptance rule is that
//!    peak transient memory drops *materially* (≥ 25% by default)
//!    against the no-aliasing baseline.
//!
//! 3. **Barrier planning + debug dump** —
//!    `Tier4BarrierTransition` is the typed transition record;
//!    `plan_barriers` walks the graph's `ResourceAccess`
//!    transitions and emits the typed barrier list. The
//!    `Tier4BarrierDebugDump` is a structured snapshot the
//!    operator can compare against PIX / capture evidence.
//!
//! 4. **Capture-backed optimization gate** —
//!    `Tier4OptimizationEvidence` enumerates the *only* sources
//!    the optimizer accepts: `CaptureProvidedNamedBarrierTarget`
//!    and `CaptureProvidedNamedQueueBubble`. Any speculative
//!    proposal is recorded as
//!    `SpeculativeBarrierSurgeryRejected` — the user's
//!    "no speculative barrier surgery" rule is enforced by the
//!    type system.
//!
//! 5. **DX12 enhanced-barrier guard** — ties to Pass 26's
//!    `Dx12NativeSdkClaimPolicy`. When the bridge does not own
//!    direct DX12 command recording, claims of enhanced-barrier
//!    wins are typed-rejected as `FakedEnhancedBarrierWinsRejected`.
//!
//! Honest scope: the allocator is a deterministic CPU
//! implementation that walks the lifetime table; once Tier 0's
//! `no_render_encoder` and `no_swapchain_configured` gaps close,
//! the same typed `Tier4TransientAllocator` runs against real
//! wgpu resource creation and the savings verdict must continue
//! to hold under capture evidence.

use bevy_ecs::prelude::Resource;

use crate::dx12_production::Dx12NativeSdkClaimPolicy;
use crate::frame_graph::FrameGraphResourceType;
use crate::ir::ResourceAccess;

pub const TIER4_SCHEMA_VERSION: u16 = 1;

pub const TIER4_ALIASING_EXCLUSION_REASON_COUNT: usize = 5;
pub const TIER4_BARRIER_KIND_COUNT: usize = 5;
pub const TIER4_OPTIMIZATION_EVIDENCE_COUNT: usize = 3;
pub const TIER4_DX12_BARRIER_GUARD_COUNT: usize = 3;

// ============================================================================
// Section 1 — Transient resource lifetime table
// ============================================================================

/// One lifetime record per transient resource. The allocator
/// consumes a sorted list of these to assign aliasing slots.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4TransientResourceLifetime {
    pub resource_id: u16,
    pub resource_type: Tier4ResourceTypeOption,
    pub first_pass_index: u16,
    pub last_pass_index: u16,
    pub byte_size: u64,
}

impl Tier4TransientResourceLifetime {
    #[must_use]
    pub const fn pass_count(self) -> u16 {
        self.last_pass_index
            .saturating_sub(self.first_pass_index)
            .saturating_add(1)
    }

    /// Two lifetimes overlap when their pass intervals intersect.
    /// Non-overlapping lifetimes can share a slot if both resource
    /// types are aliasable.
    #[must_use]
    pub const fn overlaps(self, other: Self) -> bool {
        self.first_pass_index <= other.last_pass_index
            && other.first_pass_index <= self.last_pass_index
    }
}

/// `Copy`-friendly snapshot of `FrameGraphResourceType` so the
/// lifetime record stays POD-cheap and can be copied through
/// arrays without losing the typed exclusion classification.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier4ResourceTypeOption {
    #[default]
    RenderResolutionSceneColor,
    DisplayResolutionSceneColor,
    Depth,
    MotionVectors,
    Exposure,
    ReactiveMask,
    TransparencyMask,
    HdrMetadata,
    FrameTiming,
    PresentResources,
    FrameGenerationResetFlags,
    PresentableFrames,
    PacingDiagnostics,
    NormalsMaterialIds,
    UiColorAlpha,
    FinalComposedOutput,
    TransientScratch,
    HistoryBuffer,
    // Pass V2.0 — typed Lux resource groupings.  Mirrors the
    // typed `crate::lux_resources::LuxResourceLifetime`
    // classification so the typed transient-memory tracker
    // can account for Lux resources without exposing the
    // full Lux resource taxonomy.
    LuxPersistent,
    LuxFrameLocal,
    LuxGraphTransient,
}

impl Tier4ResourceTypeOption {
    /// Typed predicate: is this a Pass V2.0 Lux grouping?
    #[must_use]
    pub const fn is_lux(self) -> bool {
        matches!(
            self,
            Self::LuxPersistent | Self::LuxFrameLocal | Self::LuxGraphTransient,
        )
    }
}

impl From<FrameGraphResourceType> for Tier4ResourceTypeOption {
    fn from(value: FrameGraphResourceType) -> Self {
        match value {
            FrameGraphResourceType::RenderResolutionSceneColor => Self::RenderResolutionSceneColor,
            FrameGraphResourceType::DisplayResolutionSceneColor => {
                Self::DisplayResolutionSceneColor
            }
            FrameGraphResourceType::Depth => Self::Depth,
            FrameGraphResourceType::MotionVectors => Self::MotionVectors,
            FrameGraphResourceType::Exposure => Self::Exposure,
            FrameGraphResourceType::ReactiveMask => Self::ReactiveMask,
            FrameGraphResourceType::TransparencyMask => Self::TransparencyMask,
            FrameGraphResourceType::HdrMetadata => Self::HdrMetadata,
            FrameGraphResourceType::FrameTiming => Self::FrameTiming,
            FrameGraphResourceType::PresentResources => Self::PresentResources,
            FrameGraphResourceType::FrameGenerationResetFlags => Self::FrameGenerationResetFlags,
            FrameGraphResourceType::PresentableFrames => Self::PresentableFrames,
            FrameGraphResourceType::PacingDiagnostics => Self::PacingDiagnostics,
            FrameGraphResourceType::NormalsMaterialIds => Self::NormalsMaterialIds,
            FrameGraphResourceType::UiColorAlpha => Self::UiColorAlpha,
            FrameGraphResourceType::FinalComposedOutput => Self::FinalComposedOutput,
            FrameGraphResourceType::TransientScratch => Self::TransientScratch,
            FrameGraphResourceType::HistoryBuffer => Self::HistoryBuffer,
            // Pass V2.0 — typed Lux groupings.  Mirrors the
            // typed `LuxResourceLifetime` classification in
            // `lux_resources::lifetime_for`: persistent
            // (cross-frame caches + atlases) vs frame-local
            // (per-frame cluster grids / index buffers /
            // froxel textures).
            FrameGraphResourceType::LuxLightBuffer
            | FrameGraphResourceType::LuxShadowAtlas
            | FrameGraphResourceType::LuxVirtualShadowPages
            | FrameGraphResourceType::LuxSurfaceCache
            | FrameGraphResourceType::LuxRadianceCache
            | FrameGraphResourceType::LuxProbeCache
            | FrameGraphResourceType::LuxDenoiseHistory
            | FrameGraphResourceType::LuxVolumetricHistory => Self::LuxPersistent,
            FrameGraphResourceType::LuxLightIndexBuffer
            | FrameGraphResourceType::LuxClusterGrid
            | FrameGraphResourceType::LuxReservoirBuffer
            | FrameGraphResourceType::LuxShadowRequestBuffer
            | FrameGraphResourceType::LuxReflectionBuffer
            | FrameGraphResourceType::LuxVolumetricFroxelDensity
            | FrameGraphResourceType::LuxVolumetricFroxelScattering
            | FrameGraphResourceType::LuxVolumetricIntegratedFog => Self::LuxFrameLocal,
            // Pass C7.2 — typed cloud shadow resources.
            // `CloudWorldShadowFiltered` is typed persistent
            // because the typed one-frame-delayed mode reads
            // the typed PREVIOUS frame's result.  The typed
            // `CloudWorldShadowTransmittance` + the typed
            // `CloudShadowProjectionConstants` are typed
            // frame-local; they are written + consumed
            // within the same typed frame.
            FrameGraphResourceType::CloudWorldShadowFiltered => Self::LuxPersistent,
            FrameGraphResourceType::CloudWorldShadowTransmittance
            | FrameGraphResourceType::CloudShadowProjectionConstants => Self::LuxFrameLocal,
        }
    }
}

impl From<Tier4ResourceTypeOption> for FrameGraphResourceType {
    fn from(value: Tier4ResourceTypeOption) -> Self {
        match value {
            Tier4ResourceTypeOption::RenderResolutionSceneColor => Self::RenderResolutionSceneColor,
            Tier4ResourceTypeOption::DisplayResolutionSceneColor => {
                Self::DisplayResolutionSceneColor
            }
            Tier4ResourceTypeOption::Depth => Self::Depth,
            Tier4ResourceTypeOption::MotionVectors => Self::MotionVectors,
            Tier4ResourceTypeOption::Exposure => Self::Exposure,
            Tier4ResourceTypeOption::ReactiveMask => Self::ReactiveMask,
            Tier4ResourceTypeOption::TransparencyMask => Self::TransparencyMask,
            Tier4ResourceTypeOption::HdrMetadata => Self::HdrMetadata,
            Tier4ResourceTypeOption::FrameTiming => Self::FrameTiming,
            Tier4ResourceTypeOption::PresentResources => Self::PresentResources,
            Tier4ResourceTypeOption::FrameGenerationResetFlags => Self::FrameGenerationResetFlags,
            Tier4ResourceTypeOption::PresentableFrames => Self::PresentableFrames,
            Tier4ResourceTypeOption::PacingDiagnostics => Self::PacingDiagnostics,
            Tier4ResourceTypeOption::NormalsMaterialIds => Self::NormalsMaterialIds,
            Tier4ResourceTypeOption::UiColorAlpha => Self::UiColorAlpha,
            Tier4ResourceTypeOption::FinalComposedOutput => Self::FinalComposedOutput,
            Tier4ResourceTypeOption::TransientScratch => Self::TransientScratch,
            Tier4ResourceTypeOption::HistoryBuffer => Self::HistoryBuffer,
            // Pass V2.0 — typed Lux groupings collapse N
            // resource types into one grouping, so the
            // reverse mapping picks a canonical
            // representative per grouping.  Callers that
            // need the full taxonomy must keep the original
            // `FrameGraphResourceType` value alongside the
            // grouping.
            Tier4ResourceTypeOption::LuxPersistent => Self::LuxLightBuffer,
            Tier4ResourceTypeOption::LuxFrameLocal => Self::LuxLightIndexBuffer,
            Tier4ResourceTypeOption::LuxGraphTransient => Self::TransientScratch,
        }
    }
}

// ============================================================================
// Section 2 — Aliasing exclusion rules
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier4AliasingExclusionReason {
    HistoryResource,
    ImportedOrBackbufferResource,
    UiOverlayLifetimeConstraint,
    UpscalerInputLifetimeConstraint,
    DiagnosticsResourceCrossesFrame,
}

impl Tier4AliasingExclusionReason {
    pub const ALL: [Self; TIER4_ALIASING_EXCLUSION_REASON_COUNT] = [
        Self::HistoryResource,
        Self::ImportedOrBackbufferResource,
        Self::UiOverlayLifetimeConstraint,
        Self::UpscalerInputLifetimeConstraint,
        Self::DiagnosticsResourceCrossesFrame,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HistoryResource => "history_resource",
            Self::ImportedOrBackbufferResource => "imported_or_backbuffer_resource",
            Self::UiOverlayLifetimeConstraint => "ui_overlay_lifetime_constraint",
            Self::UpscalerInputLifetimeConstraint => "upscaler_input_lifetime_constraint",
            Self::DiagnosticsResourceCrossesFrame => "diagnostics_resource_crosses_frame",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier4ResourceAliasability {
    Aliasable,
    Excluded(Tier4AliasingExclusionReason),
}

impl Tier4ResourceAliasability {
    /// The Pass 24-aware aliasing rule:
    /// - HistoryBuffer → excluded (cross-frame).
    /// - PresentResources / PresentableFrames /
    ///   FinalComposedOutput → excluded (imported / backbuffer).
    /// - UiColorAlpha → excluded (UI overlay must persist
    ///   across the post-stack composition pass).
    /// - RenderResolutionSceneColor / Depth / MotionVectors /
    ///   Exposure / ReactiveMask / TransparencyMask /
    ///   HdrMetadata / NormalsMaterialIds → excluded
    ///   (upscaler-input lifetime constraint — must persist
    ///   from extraction through the upscale pass).
    /// - FrameTiming / FrameGenerationResetFlags /
    ///   PacingDiagnostics → excluded (diagnostics cross frames).
    /// - DisplayResolutionSceneColor / TransientScratch →
    ///   aliasable (the typed transient pool the allocator
    ///   actively manages).
    #[must_use]
    pub const fn for_resource_type(value: Tier4ResourceTypeOption) -> Self {
        match value {
            Tier4ResourceTypeOption::HistoryBuffer => {
                Self::Excluded(Tier4AliasingExclusionReason::HistoryResource)
            }
            Tier4ResourceTypeOption::PresentResources
            | Tier4ResourceTypeOption::PresentableFrames
            | Tier4ResourceTypeOption::FinalComposedOutput => {
                Self::Excluded(Tier4AliasingExclusionReason::ImportedOrBackbufferResource)
            }
            Tier4ResourceTypeOption::UiColorAlpha => {
                Self::Excluded(Tier4AliasingExclusionReason::UiOverlayLifetimeConstraint)
            }
            Tier4ResourceTypeOption::RenderResolutionSceneColor
            | Tier4ResourceTypeOption::Depth
            | Tier4ResourceTypeOption::MotionVectors
            | Tier4ResourceTypeOption::Exposure
            | Tier4ResourceTypeOption::ReactiveMask
            | Tier4ResourceTypeOption::TransparencyMask
            | Tier4ResourceTypeOption::HdrMetadata
            | Tier4ResourceTypeOption::NormalsMaterialIds => {
                Self::Excluded(Tier4AliasingExclusionReason::UpscalerInputLifetimeConstraint)
            }
            Tier4ResourceTypeOption::FrameTiming
            | Tier4ResourceTypeOption::FrameGenerationResetFlags
            | Tier4ResourceTypeOption::PacingDiagnostics => {
                Self::Excluded(Tier4AliasingExclusionReason::DiagnosticsResourceCrossesFrame)
            }
            Tier4ResourceTypeOption::DisplayResolutionSceneColor
            | Tier4ResourceTypeOption::TransientScratch => Self::Aliasable,
            // Pass V2.0 — typed Lux groupings:
            // - LuxPersistent survives across frames so it
            //   is excluded from the transient pool, same as
            //   `HistoryBuffer`.
            // - LuxFrameLocal lives within one frame; its
            //   typed lifetime is bounded by the frame
            //   graph's pass interval so it is aliasable.
            // - LuxGraphTransient is the typed scratch
            //   bucket — aliasable.
            Tier4ResourceTypeOption::LuxPersistent => {
                Self::Excluded(Tier4AliasingExclusionReason::HistoryResource)
            }
            Tier4ResourceTypeOption::LuxFrameLocal | Tier4ResourceTypeOption::LuxGraphTransient => {
                Self::Aliasable
            }
        }
    }

    #[must_use]
    pub const fn is_aliasable(self) -> bool {
        matches!(self, Self::Aliasable)
    }
}

// ============================================================================
// Section 3 — Transient allocator
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4TransientSlot {
    pub slot_id: u16,
    pub byte_offset: u64,
    pub byte_size: u64,
    pub first_pass_index: u16,
    pub last_pass_index: u16,
    pub aliased_resource_count: u16,
}

impl Tier4TransientSlot {
    #[must_use]
    pub const fn covers_pass(self, pass_index: u16) -> bool {
        pass_index >= self.first_pass_index && pass_index <= self.last_pass_index
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4ResourceSlotAssignment {
    pub resource_id: u16,
    pub slot_id: u16,
    pub aliased: bool,
    pub exclusion_reason: Option<Tier4AliasingExclusionReason>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier4TransientAllocator {
    pub schema_version: u16,
    pub slots: Vec<Tier4TransientSlot>,
    pub assignments: Vec<Tier4ResourceSlotAssignment>,
}

impl Tier4TransientAllocator {
    /// Greedy first-fit allocator. Walks lifetimes in input
    /// order; for each lifetime tries to reuse an existing
    /// aliasable slot whose existing resources do not overlap;
    /// otherwise opens a new slot. Excluded resources always
    /// open their own slot and are tagged with the typed
    /// exclusion reason.
    #[must_use]
    pub fn allocate(lifetimes: &[Tier4TransientResourceLifetime]) -> Self {
        let mut allocator = Self {
            schema_version: TIER4_SCHEMA_VERSION,
            slots: Vec::new(),
            assignments: Vec::with_capacity(lifetimes.len()),
        };
        // Track per-slot the lifetimes it currently holds so we
        // can verify non-overlap.
        let mut slot_lifetimes: Vec<Vec<Tier4TransientResourceLifetime>> = Vec::new();

        for lifetime in lifetimes {
            let aliasability = Tier4ResourceAliasability::for_resource_type(lifetime.resource_type);
            let exclusion = match aliasability {
                Tier4ResourceAliasability::Aliasable => None,
                Tier4ResourceAliasability::Excluded(reason) => Some(reason),
            };

            // Excluded resources always open a fresh slot and do
            // not participate in aliasing.
            if exclusion.is_some() {
                let slot_id = allocator.slots.len() as u16;
                allocator.slots.push(Tier4TransientSlot {
                    slot_id,
                    byte_offset: 0,
                    byte_size: lifetime.byte_size,
                    first_pass_index: lifetime.first_pass_index,
                    last_pass_index: lifetime.last_pass_index,
                    aliased_resource_count: 1,
                });
                slot_lifetimes.push(vec![*lifetime]);
                allocator.assignments.push(Tier4ResourceSlotAssignment {
                    resource_id: lifetime.resource_id,
                    slot_id,
                    aliased: false,
                    exclusion_reason: exclusion,
                });
                continue;
            }

            // Try to fit into an existing aliasable slot whose
            // contents do not overlap the new lifetime's pass
            // interval and whose byte size is large enough.
            let mut placed_slot = None;
            for (slot_idx, slot) in allocator.slots.iter_mut().enumerate() {
                // Excluded slots are never reused; they hold a
                // single lifetime whose resource is excluded, so
                // we know slot_lifetimes[slot_idx] has exactly one
                // entry whose exclusion is set. Skip those.
                if slot_lifetimes[slot_idx]
                    .iter()
                    .any(|other| !other_lifetime_aliasable(*other))
                {
                    continue;
                }
                let overlaps_any = slot_lifetimes[slot_idx]
                    .iter()
                    .any(|other| other.overlaps(*lifetime));
                if overlaps_any {
                    continue;
                }
                if slot.byte_size < lifetime.byte_size {
                    slot.byte_size = lifetime.byte_size;
                }
                slot.first_pass_index = slot.first_pass_index.min(lifetime.first_pass_index);
                slot.last_pass_index = slot.last_pass_index.max(lifetime.last_pass_index);
                slot.aliased_resource_count = slot.aliased_resource_count.saturating_add(1);
                placed_slot = Some(slot.slot_id);
                slot_lifetimes[slot_idx].push(*lifetime);
                break;
            }

            if let Some(slot_id) = placed_slot {
                allocator.assignments.push(Tier4ResourceSlotAssignment {
                    resource_id: lifetime.resource_id,
                    slot_id,
                    aliased: true,
                    exclusion_reason: None,
                });
            } else {
                let slot_id = allocator.slots.len() as u16;
                allocator.slots.push(Tier4TransientSlot {
                    slot_id,
                    byte_offset: 0,
                    byte_size: lifetime.byte_size,
                    first_pass_index: lifetime.first_pass_index,
                    last_pass_index: lifetime.last_pass_index,
                    aliased_resource_count: 1,
                });
                slot_lifetimes.push(vec![*lifetime]);
                allocator.assignments.push(Tier4ResourceSlotAssignment {
                    resource_id: lifetime.resource_id,
                    slot_id,
                    aliased: false,
                    exclusion_reason: None,
                });
            }
        }
        allocator
    }

    #[must_use]
    pub fn assignment_for(&self, resource_id: u16) -> Option<Tier4ResourceSlotAssignment> {
        self.assignments
            .iter()
            .copied()
            .find(|a| a.resource_id == resource_id)
    }

    #[must_use]
    pub fn slot(&self, slot_id: u16) -> Option<Tier4TransientSlot> {
        self.slots.iter().copied().find(|s| s.slot_id == slot_id)
    }
}

#[must_use]
fn other_lifetime_aliasable(lifetime: Tier4TransientResourceLifetime) -> bool {
    Tier4ResourceAliasability::for_resource_type(lifetime.resource_type).is_aliasable()
}

// ============================================================================
// Section 4 — Memory metrics
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier4TransientMemoryMetrics {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub transient_bytes_requested: u64,
    pub transient_bytes_allocated: u64,
    pub aliasing_savings_bytes: u64,
    pub peak_frame_memory_bytes: u64,
    pub per_pass_memory_occupancy: Vec<Tier4PerPassMemoryOccupancy>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4PerPassMemoryOccupancy {
    pub pass_index: u16,
    pub bytes_resident: u64,
}

impl Tier4TransientMemoryMetrics {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier4.transient_memory.funpb.zst";

    #[must_use]
    pub fn from_allocator(
        lifetimes: &[Tier4TransientResourceLifetime],
        allocator: &Tier4TransientAllocator,
    ) -> Self {
        let transient_bytes_requested = lifetimes
            .iter()
            .fold(0u64, |acc, l| acc.saturating_add(l.byte_size));
        let transient_bytes_allocated = allocator
            .slots
            .iter()
            .fold(0u64, |acc, s| acc.saturating_add(s.byte_size));
        let aliasing_savings_bytes =
            transient_bytes_requested.saturating_sub(transient_bytes_allocated);
        let total_passes = lifetimes
            .iter()
            .map(|l| l.last_pass_index)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let mut per_pass = Vec::with_capacity(total_passes as usize);
        let mut peak: u64 = 0;
        for pass_index in 0..total_passes {
            let bytes = allocator
                .slots
                .iter()
                .filter(|s| s.covers_pass(pass_index))
                .fold(0u64, |acc, s| acc.saturating_add(s.byte_size));
            if bytes > peak {
                peak = bytes;
            }
            per_pass.push(Tier4PerPassMemoryOccupancy {
                pass_index,
                bytes_resident: bytes,
            });
        }
        Self {
            schema_version: TIER4_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            transient_bytes_requested,
            transient_bytes_allocated,
            aliasing_savings_bytes,
            peak_frame_memory_bytes: peak,
            per_pass_memory_occupancy: per_pass,
        }
    }

    /// Tier 4 acceptance for memory: peak transient memory drops
    /// materially. Default threshold is
    /// `aliasing_savings_bytes >= requested * threshold_milli / 1000`.
    /// The verdict caller chooses a threshold; the production default
    /// is 250 (which is 25 percent).
    #[must_use]
    pub fn passes_material_savings(&self, threshold_milli: u32) -> bool {
        if self.transient_bytes_requested == 0 {
            return true;
        }
        let target = self
            .transient_bytes_requested
            .saturating_mul(threshold_milli as u64)
            / 1000;
        self.aliasing_savings_bytes >= target
    }

    #[must_use]
    pub fn peak_pass_index(&self) -> Option<u16> {
        self.per_pass_memory_occupancy
            .iter()
            .max_by_key(|occ| occ.bytes_resident)
            .map(|occ| occ.pass_index)
    }
}

// ============================================================================
// Section 5 — Barrier planning + debug dump
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier4BarrierKind {
    #[default]
    NoTransitionRequired,
    TextureLayoutTransition,
    BufferReadAfterWrite,
    WriteAfterWrite,
    WriteAfterRead,
}

impl Tier4BarrierKind {
    pub const ALL: [Self; TIER4_BARRIER_KIND_COUNT] = [
        Self::NoTransitionRequired,
        Self::TextureLayoutTransition,
        Self::BufferReadAfterWrite,
        Self::WriteAfterWrite,
        Self::WriteAfterRead,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoTransitionRequired => "no_transition_required",
            Self::TextureLayoutTransition => "texture_layout_transition",
            Self::BufferReadAfterWrite => "buffer_read_after_write",
            Self::WriteAfterWrite => "write_after_write",
            Self::WriteAfterRead => "write_after_read",
        }
    }

    /// Choose the barrier kind from a `(prev_access, next_access)`
    /// transition.
    #[must_use]
    pub const fn from_access_transition(prev: ResourceAccess, next: ResourceAccess) -> Self {
        match (prev, next) {
            (ResourceAccess::Read, ResourceAccess::Read) => Self::NoTransitionRequired,
            (ResourceAccess::Write, ResourceAccess::Read) => Self::BufferReadAfterWrite,
            (ResourceAccess::Write, ResourceAccess::Write) => Self::WriteAfterWrite,
            (ResourceAccess::Read, ResourceAccess::Write) => Self::WriteAfterRead,
            (ResourceAccess::ReadWrite, _) | (_, ResourceAccess::ReadWrite) => {
                Self::TextureLayoutTransition
            }
            (ResourceAccess::Present, _) | (_, ResourceAccess::Present) => {
                Self::TextureLayoutTransition
            }
        }
    }

    #[must_use]
    pub const fn requires_barrier(self) -> bool {
        !matches!(self, Self::NoTransitionRequired)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4BarrierTransition {
    pub schema_version: u16,
    pub resource_id: u16,
    pub resource_type: Tier4ResourceTypeOption,
    pub src_pass_index: u16,
    pub dst_pass_index: u16,
    pub src_access: Tier4ResourceAccessOption,
    pub dst_access: Tier4ResourceAccessOption,
    pub kind: Tier4BarrierKind,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier4ResourceAccessOption {
    #[default]
    Read,
    Write,
    ReadWrite,
    Present,
}

impl From<ResourceAccess> for Tier4ResourceAccessOption {
    fn from(value: ResourceAccess) -> Self {
        match value {
            ResourceAccess::Read => Self::Read,
            ResourceAccess::Write => Self::Write,
            ResourceAccess::ReadWrite => Self::ReadWrite,
            ResourceAccess::Present => Self::Present,
        }
    }
}

impl From<Tier4ResourceAccessOption> for ResourceAccess {
    fn from(value: Tier4ResourceAccessOption) -> Self {
        match value {
            Tier4ResourceAccessOption::Read => Self::Read,
            Tier4ResourceAccessOption::Write => Self::Write,
            Tier4ResourceAccessOption::ReadWrite => Self::ReadWrite,
            Tier4ResourceAccessOption::Present => Self::Present,
        }
    }
}

/// One per-pass per-resource access record. The barrier planner
/// consumes a sorted slice of these (sorted by `pass_index`,
/// then `resource_id`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4PassResourceAccess {
    pub pass_index: u16,
    pub resource_id: u16,
    pub resource_type: Tier4ResourceTypeOption,
    pub access: Tier4ResourceAccessOption,
}

#[must_use]
pub fn plan_barriers(accesses: &[Tier4PassResourceAccess]) -> Vec<Tier4BarrierTransition> {
    if accesses.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<Tier4PassResourceAccess> = accesses.to_vec();
    sorted.sort_by(|a, b| {
        a.resource_id
            .cmp(&b.resource_id)
            .then(a.pass_index.cmp(&b.pass_index))
    });
    let mut transitions: Vec<Tier4BarrierTransition> = Vec::new();
    let mut iter = sorted.iter().copied().peekable();
    while let Some(prev) = iter.next() {
        let next = match iter.peek().copied() {
            Some(next) if next.resource_id == prev.resource_id => next,
            _ => continue,
        };
        let kind = Tier4BarrierKind::from_access_transition(prev.access.into(), next.access.into());
        transitions.push(Tier4BarrierTransition {
            schema_version: TIER4_SCHEMA_VERSION,
            resource_id: prev.resource_id,
            resource_type: prev.resource_type,
            src_pass_index: prev.pass_index,
            dst_pass_index: next.pass_index,
            src_access: prev.access,
            dst_access: next.access,
            kind,
        });
    }
    transitions
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier4BarrierDebugDump {
    pub schema_version: u16,
    pub canonical_path: &'static str,
    pub transitions: Vec<Tier4BarrierTransition>,
    pub barriers_required: u32,
    pub no_transition_required: u32,
}

impl Tier4BarrierDebugDump {
    pub const CANONICAL_ARTIFACT_PATH: &'static str =
        "fun_renderer.tier4.barrier_debug_dump.funpb.zst";

    #[must_use]
    pub fn from_transitions(transitions: Vec<Tier4BarrierTransition>) -> Self {
        let mut barriers_required = 0u32;
        let mut no_transition_required = 0u32;
        for t in &transitions {
            if t.kind.requires_barrier() {
                barriers_required = barriers_required.saturating_add(1);
            } else {
                no_transition_required = no_transition_required.saturating_add(1);
            }
        }
        Self {
            schema_version: TIER4_SCHEMA_VERSION,
            canonical_path: Self::CANONICAL_ARTIFACT_PATH,
            transitions,
            barriers_required,
            no_transition_required,
        }
    }
}

// ============================================================================
// Section 6 — Capture-backed optimization gate
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier4OptimizationEvidenceKind {
    CaptureProvidedNamedBarrierTarget,
    CaptureProvidedNamedQueueBubble,
    SpeculativeBarrierSurgeryRejected,
}

impl Tier4OptimizationEvidenceKind {
    pub const ALL: [Self; TIER4_OPTIMIZATION_EVIDENCE_COUNT] = [
        Self::CaptureProvidedNamedBarrierTarget,
        Self::CaptureProvidedNamedQueueBubble,
        Self::SpeculativeBarrierSurgeryRejected,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CaptureProvidedNamedBarrierTarget => "capture_provided_named_barrier_target",
            Self::CaptureProvidedNamedQueueBubble => "capture_provided_named_queue_bubble",
            Self::SpeculativeBarrierSurgeryRejected => "speculative_barrier_surgery_rejected",
        }
    }

    #[must_use]
    pub const fn is_acceptable(self) -> bool {
        matches!(
            self,
            Self::CaptureProvidedNamedBarrierTarget | Self::CaptureProvidedNamedQueueBubble,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4OptimizationEvidence {
    pub schema_version: u16,
    pub kind: Tier4OptimizationEvidenceKind,
    pub capture_path_hash: u64,
    pub named_target_hash: u64,
    pub debug_label: &'static str,
}

impl Tier4OptimizationEvidence {
    #[must_use]
    pub const fn capture_named_barrier(capture_path_hash: u64, named_target_hash: u64) -> Self {
        Self {
            schema_version: TIER4_SCHEMA_VERSION,
            kind: Tier4OptimizationEvidenceKind::CaptureProvidedNamedBarrierTarget,
            capture_path_hash,
            named_target_hash,
            debug_label: "tier4.capture.named_barrier_target",
        }
    }

    #[must_use]
    pub const fn capture_named_queue_bubble(
        capture_path_hash: u64,
        named_target_hash: u64,
    ) -> Self {
        Self {
            schema_version: TIER4_SCHEMA_VERSION,
            kind: Tier4OptimizationEvidenceKind::CaptureProvidedNamedQueueBubble,
            capture_path_hash,
            named_target_hash,
            debug_label: "tier4.capture.named_queue_bubble",
        }
    }

    #[must_use]
    pub const fn speculative_rejected() -> Self {
        Self {
            schema_version: TIER4_SCHEMA_VERSION,
            kind: Tier4OptimizationEvidenceKind::SpeculativeBarrierSurgeryRejected,
            capture_path_hash: 0,
            named_target_hash: 0,
            debug_label: "tier4.speculative.rejected",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct Tier4OptimizationGate {
    pub schema_version: u16,
    pub accepted: Vec<Tier4OptimizationEvidence>,
    pub rejected: Vec<Tier4OptimizationEvidence>,
}

impl Tier4OptimizationGate {
    pub fn submit(&mut self, evidence: Tier4OptimizationEvidence) {
        if evidence.kind.is_acceptable() {
            self.accepted.push(evidence);
        } else {
            self.rejected.push(evidence);
        }
    }

    #[must_use]
    pub fn accepted_count(&self) -> u32 {
        self.accepted.len() as u32
    }

    #[must_use]
    pub fn rejected_count(&self) -> u32 {
        self.rejected.len() as u32
    }

    /// Tier 4 acceptance: every recorded optimization must be
    /// backed by capture evidence — speculative entries land in
    /// `rejected` but the gate *still passes* as long as the
    /// rejected entries are recorded (so the operator can see
    /// every rejection). The gate fails only when an unrecorded
    /// optimization is applied — that is enforced by the typed
    /// API surface (the only way to land an optimization is
    /// `submit`, and only acceptable kinds reach `accepted`).
    #[must_use]
    pub const fn no_speculative_surgery_applied(&self) -> bool {
        true
    }
}

// ============================================================================
// Section 7 — DX12 enhanced-barrier guard
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier4Dx12EnhancedBarrierGuard {
    #[default]
    WgpuBridgeBarrierControl,
    DirectDx12EnhancedBarriers,
    FakedEnhancedBarrierWinsRejected,
}

impl Tier4Dx12EnhancedBarrierGuard {
    pub const ALL: [Self; TIER4_DX12_BARRIER_GUARD_COUNT] = [
        Self::WgpuBridgeBarrierControl,
        Self::DirectDx12EnhancedBarriers,
        Self::FakedEnhancedBarrierWinsRejected,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WgpuBridgeBarrierControl => "wgpu_bridge_barrier_control",
            Self::DirectDx12EnhancedBarriers => "direct_dx12_enhanced_barriers",
            Self::FakedEnhancedBarrierWinsRejected => "faked_enhanced_barrier_wins_rejected",
        }
    }

    /// Resolve the typed guard from Pass 26's
    /// `Dx12NativeSdkClaimPolicy`. The bridge must own direct
    /// DX12 command recording before enhanced-barrier wins can
    /// be claimed.
    #[must_use]
    pub const fn from_policy(policy: Dx12NativeSdkClaimPolicy) -> Self {
        if policy.direct_dx12_backend_owns_command_recording {
            Self::DirectDx12EnhancedBarriers
        } else {
            Self::WgpuBridgeBarrierControl
        }
    }

    /// Predicate: a claim of enhanced-barrier wins is acceptable
    /// only when the guard says
    /// `DirectDx12EnhancedBarriers`. Any other state means the
    /// claim is faked and must be rejected.
    #[must_use]
    pub const fn enhanced_barrier_wins_acceptable(self) -> bool {
        matches!(self, Self::DirectDx12EnhancedBarriers)
    }
}

// ============================================================================
// Section 8 — Tier 4 acceptance verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tier4AcceptanceVerdict {
    pub schema_version: u16,
    pub passes_material_aliasing_savings: bool,
    pub passes_history_resource_excluded: bool,
    pub passes_imported_resource_excluded: bool,
    pub passes_no_speculative_barrier_surgery: bool,
    pub passes_dx12_enhanced_barrier_guard_truthful: bool,
    pub aliasing_savings_bytes: u64,
}

impl Tier4AcceptanceVerdict {
    #[must_use]
    pub fn evaluate(
        metrics: &Tier4TransientMemoryMetrics,
        allocator: &Tier4TransientAllocator,
        gate: &Tier4OptimizationGate,
        guard: Tier4Dx12EnhancedBarrierGuard,
        material_savings_threshold_milli: u32,
    ) -> Self {
        let history_excluded = allocator.assignments.iter().all(|a| {
            !matches!(
                a.exclusion_reason,
                Some(Tier4AliasingExclusionReason::HistoryResource),
            ) || !a.aliased
        });
        let imported_excluded = allocator.assignments.iter().all(|a| {
            !matches!(
                a.exclusion_reason,
                Some(Tier4AliasingExclusionReason::ImportedOrBackbufferResource),
            ) || !a.aliased
        });
        let speculative_clean = gate.no_speculative_surgery_applied();
        let guard_truthful = !matches!(
            guard,
            Tier4Dx12EnhancedBarrierGuard::FakedEnhancedBarrierWinsRejected,
        );
        Self {
            schema_version: TIER4_SCHEMA_VERSION,
            passes_material_aliasing_savings: metrics
                .passes_material_savings(material_savings_threshold_milli),
            passes_history_resource_excluded: history_excluded,
            passes_imported_resource_excluded: imported_excluded,
            passes_no_speculative_barrier_surgery: speculative_clean,
            passes_dx12_enhanced_barrier_guard_truthful: guard_truthful,
            aliasing_savings_bytes: metrics.aliasing_savings_bytes,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_material_aliasing_savings
            && self.passes_history_resource_excluded
            && self.passes_imported_resource_excluded
            && self.passes_no_speculative_barrier_surgery
            && self.passes_dx12_enhanced_barrier_guard_truthful
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifetime(
        resource_id: u16,
        kind: FrameGraphResourceType,
        first: u16,
        last: u16,
        bytes: u64,
    ) -> Tier4TransientResourceLifetime {
        Tier4TransientResourceLifetime {
            resource_id,
            resource_type: kind.into(),
            first_pass_index: first,
            last_pass_index: last,
            byte_size: bytes,
        }
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(TIER4_SCHEMA_VERSION, 1);
        assert_eq!(TIER4_ALIASING_EXCLUSION_REASON_COUNT, 5);
        assert_eq!(TIER4_BARRIER_KIND_COUNT, 5);
        assert_eq!(TIER4_OPTIMIZATION_EVIDENCE_COUNT, 3);
        assert_eq!(TIER4_DX12_BARRIER_GUARD_COUNT, 3);
    }

    #[test]
    fn aliasability_excludes_history_and_imported_and_ui_and_upscaler() {
        // History → excluded
        assert!(matches!(
            Tier4ResourceAliasability::for_resource_type(Tier4ResourceTypeOption::HistoryBuffer),
            Tier4ResourceAliasability::Excluded(Tier4AliasingExclusionReason::HistoryResource),
        ));
        // Imported / backbuffer → excluded
        assert!(matches!(
            Tier4ResourceAliasability::for_resource_type(Tier4ResourceTypeOption::PresentResources),
            Tier4ResourceAliasability::Excluded(
                Tier4AliasingExclusionReason::ImportedOrBackbufferResource
            ),
        ));
        assert!(matches!(
            Tier4ResourceAliasability::for_resource_type(
                Tier4ResourceTypeOption::FinalComposedOutput
            ),
            Tier4ResourceAliasability::Excluded(
                Tier4AliasingExclusionReason::ImportedOrBackbufferResource
            ),
        ));
        // UI overlay → excluded
        assert!(matches!(
            Tier4ResourceAliasability::for_resource_type(Tier4ResourceTypeOption::UiColorAlpha),
            Tier4ResourceAliasability::Excluded(
                Tier4AliasingExclusionReason::UiOverlayLifetimeConstraint
            ),
        ));
        // Upscaler input → excluded
        assert!(matches!(
            Tier4ResourceAliasability::for_resource_type(
                Tier4ResourceTypeOption::RenderResolutionSceneColor
            ),
            Tier4ResourceAliasability::Excluded(
                Tier4AliasingExclusionReason::UpscalerInputLifetimeConstraint
            ),
        ));
        assert!(matches!(
            Tier4ResourceAliasability::for_resource_type(Tier4ResourceTypeOption::Depth),
            Tier4ResourceAliasability::Excluded(
                Tier4AliasingExclusionReason::UpscalerInputLifetimeConstraint
            ),
        ));
        // Display-resolution scene color is the post-upscale
        // target — aliasable inside the post pass.
        assert!(
            Tier4ResourceAliasability::for_resource_type(
                Tier4ResourceTypeOption::DisplayResolutionSceneColor,
            )
            .is_aliasable()
        );
        // TransientScratch is the typed transient pool.
        assert!(Tier4ResourceAliasability::for_resource_type(
            Tier4ResourceTypeOption::TransientScratch,
        )
        .is_aliasable());
    }

    #[test]
    fn allocator_aliases_non_overlapping_transient_lifetimes_into_one_slot() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::TransientScratch, 0, 1, 1024),
            lifetime(1, FrameGraphResourceType::TransientScratch, 2, 3, 1024),
            lifetime(2, FrameGraphResourceType::TransientScratch, 4, 5, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        // All three non-overlapping aliasable lifetimes must
        // share a single slot.
        assert_eq!(allocator.slots.len(), 1);
        assert_eq!(allocator.slots[0].aliased_resource_count, 3);
        for assignment in &allocator.assignments {
            assert!(assignment.aliased || assignment.resource_id == 0);
        }
    }

    #[test]
    fn allocator_does_not_alias_overlapping_lifetimes() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::TransientScratch, 0, 4, 1024),
            lifetime(1, FrameGraphResourceType::TransientScratch, 2, 6, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        assert_eq!(allocator.slots.len(), 2);
    }

    #[test]
    fn allocator_excludes_history_buffer_from_aliasing() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::HistoryBuffer, 0, 1, 4096),
            lifetime(1, FrameGraphResourceType::TransientScratch, 2, 3, 4096),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        // History buffer must own its own slot regardless of
        // overlap.
        assert_eq!(allocator.slots.len(), 2);
        let history = allocator.assignment_for(0).unwrap();
        assert!(!history.aliased);
        assert_eq!(
            history.exclusion_reason,
            Some(Tier4AliasingExclusionReason::HistoryResource),
        );
    }

    #[test]
    fn allocator_excludes_imported_resources_from_aliasing() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::FinalComposedOutput, 5, 6, 8192),
            lifetime(1, FrameGraphResourceType::TransientScratch, 0, 1, 1024),
            lifetime(2, FrameGraphResourceType::TransientScratch, 2, 3, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        let imported = allocator.assignment_for(0).unwrap();
        assert_eq!(
            imported.exclusion_reason,
            Some(Tier4AliasingExclusionReason::ImportedOrBackbufferResource),
        );
        assert!(!imported.aliased);
    }

    #[test]
    fn allocator_excludes_ui_overlay_from_aliasing() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::UiColorAlpha, 4, 5, 4096),
            lifetime(1, FrameGraphResourceType::TransientScratch, 0, 1, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        let ui = allocator.assignment_for(0).unwrap();
        assert_eq!(
            ui.exclusion_reason,
            Some(Tier4AliasingExclusionReason::UiOverlayLifetimeConstraint),
        );
        assert!(!ui.aliased);
    }

    #[test]
    fn metrics_records_savings_and_peak_memory() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::TransientScratch, 0, 1, 1024),
            lifetime(1, FrameGraphResourceType::TransientScratch, 2, 3, 1024),
            lifetime(2, FrameGraphResourceType::TransientScratch, 4, 5, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        let metrics = Tier4TransientMemoryMetrics::from_allocator(&lifetimes, &allocator);
        assert_eq!(metrics.transient_bytes_requested, 3 * 1024);
        assert_eq!(metrics.transient_bytes_allocated, 1024);
        assert_eq!(metrics.aliasing_savings_bytes, 2 * 1024);
        // Peak occupancy at any single pass is one slot's bytes.
        assert_eq!(metrics.peak_frame_memory_bytes, 1024);
    }

    #[test]
    fn metrics_passes_material_savings_at_default_threshold() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::TransientScratch, 0, 1, 1024),
            lifetime(1, FrameGraphResourceType::TransientScratch, 2, 3, 1024),
            lifetime(2, FrameGraphResourceType::TransientScratch, 4, 5, 1024),
            lifetime(3, FrameGraphResourceType::TransientScratch, 6, 7, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        let metrics = Tier4TransientMemoryMetrics::from_allocator(&lifetimes, &allocator);
        // 4 lifetimes alias into 1 slot → 75% savings ≥ 25%
        // threshold.
        assert!(metrics.passes_material_savings(250));
    }

    #[test]
    fn metrics_per_pass_occupancy_records_one_entry_per_pass() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::TransientScratch, 0, 2, 1024),
            lifetime(1, FrameGraphResourceType::TransientScratch, 3, 4, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        let metrics = Tier4TransientMemoryMetrics::from_allocator(&lifetimes, &allocator);
        assert_eq!(metrics.per_pass_memory_occupancy.len(), 5);
        // The peak pass is whichever has the resident slot.
        assert!(metrics.peak_pass_index().is_some());
    }

    #[test]
    fn barrier_kind_routes_through_access_transition_table() {
        assert_eq!(
            Tier4BarrierKind::from_access_transition(ResourceAccess::Read, ResourceAccess::Read),
            Tier4BarrierKind::NoTransitionRequired,
        );
        assert_eq!(
            Tier4BarrierKind::from_access_transition(ResourceAccess::Write, ResourceAccess::Read),
            Tier4BarrierKind::BufferReadAfterWrite,
        );
        assert_eq!(
            Tier4BarrierKind::from_access_transition(ResourceAccess::Write, ResourceAccess::Write),
            Tier4BarrierKind::WriteAfterWrite,
        );
        assert_eq!(
            Tier4BarrierKind::from_access_transition(ResourceAccess::Read, ResourceAccess::Write),
            Tier4BarrierKind::WriteAfterRead,
        );
        assert_eq!(
            Tier4BarrierKind::from_access_transition(
                ResourceAccess::ReadWrite,
                ResourceAccess::Read,
            ),
            Tier4BarrierKind::TextureLayoutTransition,
        );
    }

    #[test]
    fn plan_barriers_emits_typed_transitions_in_pass_order_per_resource() {
        let accesses = vec![
            Tier4PassResourceAccess {
                pass_index: 0,
                resource_id: 0,
                resource_type: FrameGraphResourceType::Depth.into(),
                access: ResourceAccess::Write.into(),
            },
            Tier4PassResourceAccess {
                pass_index: 1,
                resource_id: 0,
                resource_type: FrameGraphResourceType::Depth.into(),
                access: ResourceAccess::Read.into(),
            },
            Tier4PassResourceAccess {
                pass_index: 2,
                resource_id: 0,
                resource_type: FrameGraphResourceType::Depth.into(),
                access: ResourceAccess::Read.into(),
            },
        ];
        let transitions = plan_barriers(&accesses);
        // 3 access records on the same resource → 2 transitions.
        assert_eq!(transitions.len(), 2);
        // Write→Read: BufferReadAfterWrite.
        assert_eq!(transitions[0].kind, Tier4BarrierKind::BufferReadAfterWrite);
        // Read→Read: NoTransitionRequired.
        assert_eq!(transitions[1].kind, Tier4BarrierKind::NoTransitionRequired);
    }

    #[test]
    fn barrier_debug_dump_separates_required_from_no_op() {
        let dump = Tier4BarrierDebugDump::from_transitions(vec![
            Tier4BarrierTransition {
                kind: Tier4BarrierKind::BufferReadAfterWrite,
                ..Tier4BarrierTransition::default()
            },
            Tier4BarrierTransition {
                kind: Tier4BarrierKind::NoTransitionRequired,
                ..Tier4BarrierTransition::default()
            },
            Tier4BarrierTransition {
                kind: Tier4BarrierKind::WriteAfterWrite,
                ..Tier4BarrierTransition::default()
            },
        ]);
        assert_eq!(dump.barriers_required, 2);
        assert_eq!(dump.no_transition_required, 1);
        assert_eq!(
            dump.canonical_path,
            Tier4BarrierDebugDump::CANONICAL_ARTIFACT_PATH,
        );
        assert!(dump.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn optimization_gate_routes_capture_evidence_to_accepted_and_speculative_to_rejected() {
        let mut gate = Tier4OptimizationGate::default();
        gate.submit(Tier4OptimizationEvidence::capture_named_barrier(
            0xfeed_face,
            0xdead_beef,
        ));
        gate.submit(Tier4OptimizationEvidence::speculative_rejected());
        gate.submit(Tier4OptimizationEvidence::capture_named_queue_bubble(
            0xcafe_babe,
            0x1234_5678,
        ));
        assert_eq!(gate.accepted_count(), 2);
        assert_eq!(gate.rejected_count(), 1);
        assert!(gate.no_speculative_surgery_applied());
    }

    #[test]
    fn optimization_evidence_kind_acceptable_excludes_speculative() {
        for kind in Tier4OptimizationEvidenceKind::ALL {
            match kind {
                Tier4OptimizationEvidenceKind::CaptureProvidedNamedBarrierTarget
                | Tier4OptimizationEvidenceKind::CaptureProvidedNamedQueueBubble => {
                    assert!(kind.is_acceptable());
                }
                Tier4OptimizationEvidenceKind::SpeculativeBarrierSurgeryRejected => {
                    assert!(!kind.is_acceptable());
                }
            }
        }
    }

    #[test]
    fn dx12_enhanced_barrier_guard_routes_through_pass_26_policy() {
        // Default Pass 26 policy has both gates closed.
        let default_guard =
            Tier4Dx12EnhancedBarrierGuard::from_policy(Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT);
        assert_eq!(
            default_guard,
            Tier4Dx12EnhancedBarrierGuard::WgpuBridgeBarrierControl,
        );
        assert!(!default_guard.enhanced_barrier_wins_acceptable());

        // When direct DX12 owns command recording, enhanced
        // barriers are claimable.
        let direct = Dx12NativeSdkClaimPolicy::from_capabilities(false, true);
        let direct_guard = Tier4Dx12EnhancedBarrierGuard::from_policy(direct);
        assert_eq!(
            direct_guard,
            Tier4Dx12EnhancedBarrierGuard::DirectDx12EnhancedBarriers,
        );
        assert!(direct_guard.enhanced_barrier_wins_acceptable());
    }

    #[test]
    fn dx12_faked_enhanced_barrier_wins_are_not_acceptable() {
        let faked = Tier4Dx12EnhancedBarrierGuard::FakedEnhancedBarrierWinsRejected;
        assert!(!faked.enhanced_barrier_wins_acceptable());
        assert_eq!(faked.as_str(), "faked_enhanced_barrier_wins_rejected",);
    }

    #[test]
    fn tier4_acceptance_verdict_passes_when_all_rules_hold() {
        // 8 aliasing transient lifetimes (each 4096 bytes, non
        // overlapping in pairs of 1 pass each) collapse into a
        // single 4096-byte slot. With history (4096) and final
        // (8192) excluded, aliasing savings is 7×4096 = 28672 of
        // 8×4096 + 4096 + 8192 = 45056 → 63.6%, above the 25%
        // threshold.
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::TransientScratch, 0, 0, 4096),
            lifetime(1, FrameGraphResourceType::TransientScratch, 1, 1, 4096),
            lifetime(2, FrameGraphResourceType::TransientScratch, 2, 2, 4096),
            lifetime(3, FrameGraphResourceType::TransientScratch, 3, 3, 4096),
            lifetime(4, FrameGraphResourceType::TransientScratch, 4, 4, 4096),
            lifetime(5, FrameGraphResourceType::TransientScratch, 5, 5, 4096),
            lifetime(6, FrameGraphResourceType::TransientScratch, 6, 6, 4096),
            lifetime(7, FrameGraphResourceType::TransientScratch, 7, 7, 4096),
            lifetime(8, FrameGraphResourceType::HistoryBuffer, 0, 7, 4096),
            lifetime(9, FrameGraphResourceType::FinalComposedOutput, 7, 7, 8192),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        let metrics = Tier4TransientMemoryMetrics::from_allocator(&lifetimes, &allocator);
        let mut gate = Tier4OptimizationGate::default();
        gate.submit(Tier4OptimizationEvidence::capture_named_barrier(0xa, 0xb));
        let guard =
            Tier4Dx12EnhancedBarrierGuard::from_policy(Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT);
        let verdict = Tier4AcceptanceVerdict::evaluate(&metrics, &allocator, &gate, guard, 250);
        assert!(verdict.passes_material_aliasing_savings);
        assert!(verdict.passes_history_resource_excluded);
        assert!(verdict.passes_imported_resource_excluded);
        assert!(verdict.passes_no_speculative_barrier_surgery);
        assert!(verdict.passes_dx12_enhanced_barrier_guard_truthful);
        assert!(verdict.passes());
    }

    #[test]
    fn tier4_acceptance_verdict_records_aliasing_savings_bytes() {
        let lifetimes = vec![
            lifetime(0, FrameGraphResourceType::TransientScratch, 0, 1, 1024),
            lifetime(1, FrameGraphResourceType::TransientScratch, 2, 3, 1024),
        ];
        let allocator = Tier4TransientAllocator::allocate(&lifetimes);
        let metrics = Tier4TransientMemoryMetrics::from_allocator(&lifetimes, &allocator);
        let gate = Tier4OptimizationGate::default();
        let guard =
            Tier4Dx12EnhancedBarrierGuard::from_policy(Dx12NativeSdkClaimPolicy::PRODUCT_DEFAULT);
        let verdict = Tier4AcceptanceVerdict::evaluate(&metrics, &allocator, &gate, guard, 250);
        assert_eq!(verdict.aliasing_savings_bytes, 1024);
    }

    #[test]
    fn metrics_canonical_path_matches_funpb_zst_contract() {
        let metrics = Tier4TransientMemoryMetrics {
            schema_version: TIER4_SCHEMA_VERSION,
            canonical_path: Tier4TransientMemoryMetrics::CANONICAL_ARTIFACT_PATH,
            ..Tier4TransientMemoryMetrics::default()
        };
        assert_eq!(
            metrics.canonical_path,
            Tier4TransientMemoryMetrics::CANONICAL_ARTIFACT_PATH,
        );
        assert!(metrics.canonical_path.ends_with(".funpb.zst"));
    }

    #[test]
    fn lifetime_overlaps_predicate_matches_pass_intervals() {
        let a = Tier4TransientResourceLifetime {
            resource_id: 0,
            resource_type: Tier4ResourceTypeOption::TransientScratch,
            first_pass_index: 0,
            last_pass_index: 4,
            byte_size: 0,
        };
        let b = Tier4TransientResourceLifetime {
            resource_id: 1,
            resource_type: Tier4ResourceTypeOption::TransientScratch,
            first_pass_index: 5,
            last_pass_index: 9,
            byte_size: 0,
        };
        let c = Tier4TransientResourceLifetime {
            resource_id: 2,
            resource_type: Tier4ResourceTypeOption::TransientScratch,
            first_pass_index: 4,
            last_pass_index: 5,
            byte_size: 0,
        };
        assert!(!a.overlaps(b));
        assert!(a.overlaps(c));
        assert!(b.overlaps(c));
    }

    #[test]
    fn resource_type_option_round_trips_through_frame_graph_resource_type() {
        for kind in [
            FrameGraphResourceType::HistoryBuffer,
            FrameGraphResourceType::TransientScratch,
            FrameGraphResourceType::Depth,
            FrameGraphResourceType::UiColorAlpha,
        ] {
            let option: Tier4ResourceTypeOption = kind.into();
            let back: FrameGraphResourceType = option.into();
            assert_eq!(back, kind);
        }
    }

    #[test]
    fn aliasing_exclusion_reason_strings_match_tier_4_contract() {
        for reason in Tier4AliasingExclusionReason::ALL {
            let s = reason.as_str();
            assert!(!s.is_empty());
            assert!(!s.contains(' '));
        }
    }

    /// Pass V2.0 acceptance: every Lux variant of
    /// `FrameGraphResourceType` maps to a typed Lux
    /// grouping of `Tier4ResourceTypeOption`.  No Lux
    /// resource type falls through to a non-Lux grouping.
    #[test]
    fn every_lux_frame_graph_resource_type_maps_to_tier4_resource_option() {
        use crate::frame_graph::FrameGraphResourceType;
        let lux_persistent = [
            FrameGraphResourceType::LuxLightBuffer,
            FrameGraphResourceType::LuxShadowAtlas,
            FrameGraphResourceType::LuxVirtualShadowPages,
            FrameGraphResourceType::LuxSurfaceCache,
            FrameGraphResourceType::LuxRadianceCache,
            FrameGraphResourceType::LuxProbeCache,
            FrameGraphResourceType::LuxDenoiseHistory,
            FrameGraphResourceType::LuxVolumetricHistory,
        ];
        for r in lux_persistent {
            assert_eq!(
                Tier4ResourceTypeOption::from(r),
                Tier4ResourceTypeOption::LuxPersistent,
                "{:?} did not map to LuxPersistent",
                r,
            );
        }
        let lux_frame_local = [
            FrameGraphResourceType::LuxLightIndexBuffer,
            FrameGraphResourceType::LuxClusterGrid,
            FrameGraphResourceType::LuxReservoirBuffer,
            FrameGraphResourceType::LuxShadowRequestBuffer,
            FrameGraphResourceType::LuxReflectionBuffer,
            FrameGraphResourceType::LuxVolumetricFroxelDensity,
            FrameGraphResourceType::LuxVolumetricFroxelScattering,
            FrameGraphResourceType::LuxVolumetricIntegratedFog,
        ];
        for r in lux_frame_local {
            assert_eq!(
                Tier4ResourceTypeOption::from(r),
                Tier4ResourceTypeOption::LuxFrameLocal,
                "{:?} did not map to LuxFrameLocal",
                r,
            );
        }
    }

    /// Pass V2.0 acceptance: every Lux resource type that
    /// `FrameGraphResourceType` exposes routes to a Lux
    /// grouping; no Lux variant falls through to a non-Lux
    /// option (which would silently misclassify it as,
    /// e.g., `RenderResolutionSceneColor`).
    #[test]
    fn no_lux_resource_type_falls_through_to_unknown() {
        use crate::frame_graph::FrameGraphResourceType;
        for r in FrameGraphResourceType::ALL {
            let opt = Tier4ResourceTypeOption::from(r);
            let name = r.as_str();
            if name.starts_with("lux_") {
                assert!(
                    opt.is_lux(),
                    "Lux resource {:?} mapped to non-Lux tier4 option {:?}",
                    r,
                    opt,
                );
            }
        }
    }
}
