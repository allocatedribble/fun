//! Pass 25 — GPU-Driven Rendering runtime.
//!
//! Pass 17 installed the typed planning surface in
//! `gpu_driven.rs` (buffer set, indirect path, culling pass plan,
//! Hi-Z pyramid plan, cluster scaffold). Pass 25 adds the runtime
//! IR the renderer produces and the bridge consumes:
//!
//! 1. `IndirectDrawIrCommand` — typed IR record naming the render
//!    queue bucket, the indirect command signature, and the
//!    typed buffer references the bridge dereferences at submit
//!    time. The renderer never holds wgpu objects; the IR is the
//!    only handoff.
//! 2. `GpuCullingPassBindings` — typed binding layout for the
//!    GPU frustum-culling compute pass: object bounds buffer,
//!    camera plane buffer, visible instance list, indirect args
//!    buffer, draw count buffer, debug counters.
//! 3. `HiZPyramidRuntime` — mip-chain descriptor + previous-frame
//!    source binding + conservative fallback selector. Extends
//!    `GpuDrivenHiZPyramidPlan` from Pass 17 with the per-mip
//!    extent table the bridge needs to allocate the pyramid.
//! 4. `MeshletMetadataRecord` — typed per-meshlet header
//!    (cluster bounds, vertex/triangle ranges, material slot).
//!    The asset-prep registry from Pass 21 carries a count
//!    field; Pass 25 turns that count into the typed record list
//!    the cluster scaffold consumes.
//! 5. `MeshShaderCapabilityBranch` — typed branch (MeshShader /
//!    ComputeFallback / VendorSpecific) so the bridge can select
//!    the right path without re-reading capabilities at recording
//!    time.
//! 6. `GpuDrivenParityValidation` — typed CPU/direct-vs-GPU
//!    parity check the operator can run as a regression gate.
//! 7. `GpuDrivenRuntimeMetrics` — Bevy Resource that aggregates
//!    per-frame culling, occlusion, and meshlet counts.
//!
//! Design rules:
//!
//! 1. Renderer scales by typed table writes, not per-draw bridge
//!    translation. The IR list is dense and produced once per
//!    frame.
//! 2. Backend-specific indirect mechanics
//!    (`Dx12ExecuteIndirectDrawIndexed`, `VulkanDrawIndexedIndirectCount`,
//!    `MetalDrawIndexedIndirectCommands`) are named in the IR but
//!    only resolved by the bridge. The renderer does not
//!    encode a backend-specific argument layout.
//! 3. Conservative occlusion fallback. Anything the Hi-Z pass
//!    cannot prove invisible stays visible; the conservative
//!    branch is the default and is asserted by tests.

use bevy_ecs::prelude::Resource;

use crate::backend::NativeBackend;
use crate::gpu_driven::{
    GPU_DRIVEN_DRAW_COUNT_STRIDE, GPU_DRIVEN_INDIRECT_ARG_STRIDE, GpuDrivenBufferKind,
    GpuDrivenBufferSet, GpuDrivenClusterPath, GpuDrivenClusterScaffold, GpuDrivenCullingPassPlan,
    GpuDrivenDebugCounterSnapshot, GpuDrivenHiZPyramidPlan, GpuDrivenIndirectCommandSignature,
    GpuDrivenMetrics, GpuDrivenOcclusionPolicy, GpuDrivenParityComparison, GpuDrivenPlan,
};

pub const GPU_DRIVEN_RUNTIME_SCHEMA_VERSION: u16 = 1;

pub const INDIRECT_DRAW_IR_BUCKET_COUNT: usize = 7;
pub const GPU_CULLING_BINDING_COUNT: usize = 6;
pub const HI_Z_DEFAULT_MIP_LEVELS: u8 = 8;
pub const MESHLET_DEFAULT_TRIANGLE_LIMIT: u32 = 124;
pub const MESHLET_DEFAULT_VERTEX_LIMIT: u32 = 64;

// ============================================================================
// Section 1 — Indirect draw IR
// ============================================================================

/// Render queue bucket the indirect command targets. Mirrors
/// `RenderQueueBucket` from `scene_streaming` but is renderer-IR
/// owned so the IR module does not depend on the streaming
/// module's `Resource` derives.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndirectDrawBucket {
    #[default]
    Depth,
    Opaque,
    AlphaTest,
    Transparent,
    Shadow,
    Ui,
    Debug,
}

impl IndirectDrawBucket {
    pub const ALL: [Self; INDIRECT_DRAW_IR_BUCKET_COUNT] = [
        Self::Depth,
        Self::Opaque,
        Self::AlphaTest,
        Self::Transparent,
        Self::Shadow,
        Self::Ui,
        Self::Debug,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Depth => 0,
            Self::Opaque => 1,
            Self::AlphaTest => 2,
            Self::Transparent => 3,
            Self::Shadow => 4,
            Self::Ui => 5,
            Self::Debug => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Depth => "depth",
            Self::Opaque => "opaque",
            Self::AlphaTest => "alpha_test",
            Self::Transparent => "transparent",
            Self::Shadow => "shadow",
            Self::Ui => "ui",
            Self::Debug => "debug",
        }
    }

    /// The shadow / depth / debug buckets opt out of GPU-driven
    /// indirect path by default — they are recorded directly
    /// because the early-z, shadow caster filter, and overlay
    /// rules are not yet wired through the GPU culling pass.
    #[must_use]
    pub const fn participates_in_gpu_driven_indirect_default(self) -> bool {
        matches!(
            self,
            Self::Opaque | Self::AlphaTest | Self::Transparent | Self::Shadow,
        )
    }
}

/// Typed reference to a renderer-owned buffer slot. The bridge
/// resolves the slot to a wgpu buffer at submission time; the
/// renderer never holds the wgpu handle.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndirectBufferRef {
    pub kind: IndirectBufferKind,
    pub byte_offset: u64,
    pub byte_size: u64,
    pub stride: u32,
}

impl IndirectBufferRef {
    pub const fn empty() -> Self {
        Self {
            kind: IndirectBufferKind::Args,
            byte_offset: 0,
            byte_size: 0,
            stride: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndirectBufferKind {
    #[default]
    Args,
    Count,
    VisibleInstances,
    DebugCounters,
}

impl IndirectBufferKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Args => "args",
            Self::Count => "count",
            Self::VisibleInstances => "visible_instances",
            Self::DebugCounters => "debug_counters",
        }
    }

    #[must_use]
    pub const fn from_gpu_driven_kind(kind: GpuDrivenBufferKind) -> Option<Self> {
        match kind {
            GpuDrivenBufferKind::DrawArguments => Some(Self::Args),
            GpuDrivenBufferKind::DrawCount => Some(Self::Count),
            GpuDrivenBufferKind::VisibleInstanceList => Some(Self::VisibleInstances),
            GpuDrivenBufferKind::DebugCounters => Some(Self::DebugCounters),
            _ => None,
        }
    }
}

/// Single typed indirect draw command. The bridge consumes a slice
/// of these per frame; each command names the bucket, the
/// signature the backend should use, the args/count buffer slots,
/// and the maximum draw count the GPU side may produce. The
/// command is *backend-independent* in shape — the
/// `command_signature` is metadata the bridge uses to pick the
/// correct backend-specific encode path.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndirectDrawIrCommand {
    pub schema_version: u16,
    pub bucket: IndirectDrawBucket,
    pub command_signature: IndirectCommandSignature,
    pub args_buffer: IndirectBufferRef,
    pub count_buffer: IndirectBufferRef,
    pub visible_instance_buffer: IndirectBufferRef,
    pub max_draw_count: u32,
    pub debug_label: &'static str,
}

impl IndirectDrawIrCommand {
    #[must_use]
    pub fn new(
        bucket: IndirectDrawBucket,
        signature: IndirectCommandSignature,
        max_draw_count: u32,
    ) -> Self {
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            bucket,
            command_signature: signature,
            args_buffer: IndirectBufferRef::empty(),
            count_buffer: IndirectBufferRef::empty(),
            visible_instance_buffer: IndirectBufferRef::empty(),
            max_draw_count,
            debug_label: "gpu_driven.indirect_command",
        }
    }

    #[must_use]
    pub const fn with_args(mut self, args: IndirectBufferRef) -> Self {
        self.args_buffer = args;
        self
    }

    #[must_use]
    pub const fn with_count(mut self, count: IndirectBufferRef) -> Self {
        self.count_buffer = count;
        self
    }

    #[must_use]
    pub const fn with_visible_instances(mut self, list: IndirectBufferRef) -> Self {
        self.visible_instance_buffer = list;
        self
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.max_draw_count > 0
            && self.args_buffer.byte_size > 0
            && self.count_buffer.byte_size > 0
            && matches!(self.args_buffer.kind, IndirectBufferKind::Args)
            && matches!(self.count_buffer.kind, IndirectBufferKind::Count)
    }
}

/// Renderer-IR-owned indirect command signature. Pass 17's
/// `GpuDrivenIndirectCommandSignature` is the *planning* type;
/// this is the *runtime* type the bridge dispatches on, kept
/// renderer-IR-owned so the IR list does not pull a backend
/// dependency in.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndirectCommandSignature {
    #[default]
    NotApplicableForBackend,
    Dx12ExecuteIndirectDrawIndexed,
    VulkanDrawIndexedIndirectCount,
    MetalDrawIndexedIndirectCommands,
}

impl IndirectCommandSignature {
    #[must_use]
    pub const fn from_planning(value: GpuDrivenIndirectCommandSignature) -> Self {
        match value {
            GpuDrivenIndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed => {
                Self::Dx12ExecuteIndirectDrawIndexed
            }
            GpuDrivenIndirectCommandSignature::VulkanDrawIndexedIndirectCount => {
                Self::VulkanDrawIndexedIndirectCount
            }
            GpuDrivenIndirectCommandSignature::MetalDrawIndexedIndirectCommands => {
                Self::MetalDrawIndexedIndirectCommands
            }
            GpuDrivenIndirectCommandSignature::NotApplicableForBackend => {
                Self::NotApplicableForBackend
            }
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicableForBackend => "not_applicable_for_backend",
            Self::Dx12ExecuteIndirectDrawIndexed => "dx12_execute_indirect_draw_indexed",
            Self::VulkanDrawIndexedIndirectCount => "vulkan_draw_indexed_indirect_count",
            Self::MetalDrawIndexedIndirectCommands => "metal_draw_indexed_indirect_commands",
        }
    }

    #[must_use]
    pub const fn supports_indirect_count(self) -> bool {
        matches!(
            self,
            Self::Dx12ExecuteIndirectDrawIndexed
                | Self::VulkanDrawIndexedIndirectCount
                | Self::MetalDrawIndexedIndirectCommands,
        )
    }
}

/// Dense IR list of indirect draw commands per frame. The bridge
/// reads this slice once per frame; the renderer never iterates
/// the ECS during recording.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Resource)]
pub struct IndirectDrawIrList {
    pub schema_version: u16,
    pub native_backend: NativeBackend,
    pub commands: Vec<IndirectDrawIrCommand>,
    pub bucket_counts: [u32; INDIRECT_DRAW_IR_BUCKET_COUNT],
}

impl Default for IndirectDrawIrList {
    fn default() -> Self {
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            native_backend: NativeBackend::Unknown,
            commands: Vec::new(),
            bucket_counts: [0; INDIRECT_DRAW_IR_BUCKET_COUNT],
        }
    }
}

impl IndirectDrawIrList {
    pub fn clear(&mut self) {
        self.commands.clear();
        self.bucket_counts = [0; INDIRECT_DRAW_IR_BUCKET_COUNT];
    }

    pub fn record(&mut self, command: IndirectDrawIrCommand) {
        self.bucket_counts[command.bucket.index()] =
            self.bucket_counts[command.bucket.index()].saturating_add(1);
        self.commands.push(command);
    }

    #[must_use]
    pub fn count_for(&self, bucket: IndirectDrawBucket) -> u32 {
        self.bucket_counts[bucket.index()]
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.commands.len() as u32
    }

    pub fn record_for_buffer_set(
        &mut self,
        buffer_set: GpuDrivenBufferSet,
        signature: IndirectCommandSignature,
    ) {
        let args_byte_size = (buffer_set.draw_arguments.element_stride as u64)
            .saturating_mul(buffer_set.draw_arguments.element_count as u64);
        let count_byte_size = (buffer_set.draw_count.element_stride as u64)
            .saturating_mul(buffer_set.draw_count.element_count as u64);
        let visible_byte_size = (buffer_set.visible_instance_list.element_stride as u64)
            .saturating_mul(buffer_set.visible_instance_list.element_count as u64);
        for bucket in IndirectDrawBucket::ALL {
            if !bucket.participates_in_gpu_driven_indirect_default() {
                continue;
            }
            let command = IndirectDrawIrCommand::new(bucket, signature, buffer_set.max_draw_count)
                .with_args(IndirectBufferRef {
                    kind: IndirectBufferKind::Args,
                    byte_offset: 0,
                    byte_size: args_byte_size,
                    stride: GPU_DRIVEN_INDIRECT_ARG_STRIDE,
                })
                .with_count(IndirectBufferRef {
                    kind: IndirectBufferKind::Count,
                    byte_offset: 0,
                    byte_size: count_byte_size,
                    stride: GPU_DRIVEN_DRAW_COUNT_STRIDE,
                })
                .with_visible_instances(IndirectBufferRef {
                    kind: IndirectBufferKind::VisibleInstances,
                    byte_offset: 0,
                    byte_size: visible_byte_size,
                    stride: buffer_set.visible_instance_list.element_stride,
                });
            self.record(command);
        }
    }
}

// ============================================================================
// Section 2 — GPU culling pass binding layout
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuCullingBindingSlot {
    #[default]
    ObjectBounds,
    CameraPlanes,
    VisibleInstanceList,
    DrawArguments,
    DrawCount,
    DebugCounters,
}

impl GpuCullingBindingSlot {
    pub const ALL: [Self; GPU_CULLING_BINDING_COUNT] = [
        Self::ObjectBounds,
        Self::CameraPlanes,
        Self::VisibleInstanceList,
        Self::DrawArguments,
        Self::DrawCount,
        Self::DebugCounters,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ObjectBounds => 0,
            Self::CameraPlanes => 1,
            Self::VisibleInstanceList => 2,
            Self::DrawArguments => 3,
            Self::DrawCount => 4,
            Self::DebugCounters => 5,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObjectBounds => "object_bounds",
            Self::CameraPlanes => "camera_planes",
            Self::VisibleInstanceList => "visible_instance_list",
            Self::DrawArguments => "draw_arguments",
            Self::DrawCount => "draw_count",
            Self::DebugCounters => "debug_counters",
        }
    }

    #[must_use]
    pub const fn is_read(self) -> bool {
        matches!(self, Self::ObjectBounds | Self::CameraPlanes)
    }

    #[must_use]
    pub const fn is_write(self) -> bool {
        matches!(
            self,
            Self::VisibleInstanceList | Self::DrawArguments | Self::DrawCount | Self::DebugCounters,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuCullingPassBindings {
    pub schema_version: u16,
    pub plan: GpuDrivenCullingPassPlan,
    pub bindings: [GpuCullingBindingSlotEntry; GPU_CULLING_BINDING_COUNT],
}

impl GpuCullingPassBindings {
    #[must_use]
    pub fn for_buffer_set(buffer_set: GpuDrivenBufferSet) -> Self {
        let mut bindings = [GpuCullingBindingSlotEntry::default(); GPU_CULLING_BINDING_COUNT];
        bindings[GpuCullingBindingSlot::ObjectBounds.index()] = GpuCullingBindingSlotEntry {
            slot: GpuCullingBindingSlot::ObjectBounds,
            buffer: IndirectBufferRef {
                kind: IndirectBufferKind::VisibleInstances,
                byte_offset: 0,
                // Object bounds buffer is sized by instance count
                // (each entry is the object's world AABB:
                // 8 floats = 32 bytes).
                byte_size: 32u64.saturating_mul(buffer_set.max_draw_count as u64),
                stride: 32,
            },
            access: GpuCullingBindingAccess::Read,
        };
        bindings[GpuCullingBindingSlot::CameraPlanes.index()] = GpuCullingBindingSlotEntry {
            slot: GpuCullingBindingSlot::CameraPlanes,
            buffer: IndirectBufferRef {
                kind: IndirectBufferKind::DebugCounters,
                byte_offset: 0,
                // 6 planes * 16 bytes (vec4) = 96 bytes.
                byte_size: 96,
                stride: 16,
            },
            access: GpuCullingBindingAccess::Read,
        };
        bindings[GpuCullingBindingSlot::VisibleInstanceList.index()] = GpuCullingBindingSlotEntry {
            slot: GpuCullingBindingSlot::VisibleInstanceList,
            buffer: IndirectBufferRef {
                kind: IndirectBufferKind::VisibleInstances,
                byte_offset: 0,
                byte_size: (buffer_set.visible_instance_list.element_stride as u64)
                    .saturating_mul(buffer_set.visible_instance_list.element_count as u64),
                stride: buffer_set.visible_instance_list.element_stride,
            },
            access: GpuCullingBindingAccess::Write,
        };
        bindings[GpuCullingBindingSlot::DrawArguments.index()] = GpuCullingBindingSlotEntry {
            slot: GpuCullingBindingSlot::DrawArguments,
            buffer: IndirectBufferRef {
                kind: IndirectBufferKind::Args,
                byte_offset: 0,
                byte_size: (buffer_set.draw_arguments.element_stride as u64)
                    .saturating_mul(buffer_set.draw_arguments.element_count as u64),
                stride: buffer_set.draw_arguments.element_stride,
            },
            access: GpuCullingBindingAccess::Write,
        };
        bindings[GpuCullingBindingSlot::DrawCount.index()] = GpuCullingBindingSlotEntry {
            slot: GpuCullingBindingSlot::DrawCount,
            buffer: IndirectBufferRef {
                kind: IndirectBufferKind::Count,
                byte_offset: 0,
                byte_size: (buffer_set.draw_count.element_stride as u64)
                    .saturating_mul(buffer_set.draw_count.element_count as u64),
                stride: buffer_set.draw_count.element_stride,
            },
            access: GpuCullingBindingAccess::AtomicWrite,
        };
        bindings[GpuCullingBindingSlot::DebugCounters.index()] = GpuCullingBindingSlotEntry {
            slot: GpuCullingBindingSlot::DebugCounters,
            buffer: IndirectBufferRef {
                kind: IndirectBufferKind::DebugCounters,
                byte_offset: 0,
                byte_size: (buffer_set.debug_counters.element_stride as u64)
                    .saturating_mul(buffer_set.debug_counters.element_count as u64),
                stride: buffer_set.debug_counters.element_stride,
            },
            access: GpuCullingBindingAccess::AtomicWrite,
        };
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            plan: GpuDrivenCullingPassPlan::for_instance_count(buffer_set.max_draw_count),
            bindings,
        }
    }

    #[must_use]
    pub fn binding(&self, slot: GpuCullingBindingSlot) -> GpuCullingBindingSlotEntry {
        self.bindings[slot.index()]
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuCullingBindingSlotEntry {
    pub slot: GpuCullingBindingSlot,
    pub buffer: IndirectBufferRef,
    pub access: GpuCullingBindingAccess,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuCullingBindingAccess {
    #[default]
    Read,
    Write,
    AtomicWrite,
    ReadWrite,
}

impl GpuCullingBindingAccess {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::AtomicWrite => "atomic_write",
            Self::ReadWrite => "read_write",
        }
    }
}

// ============================================================================
// Section 3 — Hi-Z pyramid runtime + occlusion fallback
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HiZMipExtent {
    pub level: u8,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Resource)]
pub struct HiZPyramidRuntime {
    pub schema_version: u16,
    pub plan: GpuDrivenHiZPyramidPlan,
    pub base_extent_px: u32,
    pub mip_extents: Vec<HiZMipExtent>,
    pub previous_frame_available: bool,
    pub conservative_fallback_active: bool,
}

impl HiZPyramidRuntime {
    #[must_use]
    pub fn new(plan: GpuDrivenHiZPyramidPlan, base_extent_px: u32) -> Self {
        let mip_extents = build_mip_chain(base_extent_px, plan.mip_levels);
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            plan,
            base_extent_px,
            mip_extents,
            previous_frame_available: false,
            conservative_fallback_active: matches!(
                plan.policy,
                GpuDrivenOcclusionPolicy::Conservative,
            ),
        }
    }

    pub fn note_previous_frame(&mut self, available: bool) {
        self.previous_frame_available = available;
        if !available {
            self.conservative_fallback_active = true;
        } else {
            self.conservative_fallback_active =
                matches!(self.plan.policy, GpuDrivenOcclusionPolicy::Conservative,);
        }
    }

    #[must_use]
    pub fn classifies_uncertain_as_visible(&self) -> bool {
        self.conservative_fallback_active || self.plan.policy.classifies_uncertain_as_visible()
    }

    #[must_use]
    pub fn smallest_mip(&self) -> Option<HiZMipExtent> {
        self.mip_extents.iter().copied().last()
    }

    #[must_use]
    pub fn mip_count(&self) -> u8 {
        self.mip_extents.len() as u8
    }
}

impl Default for HiZPyramidRuntime {
    fn default() -> Self {
        Self::new(GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT, 1024)
    }
}

#[must_use]
fn build_mip_chain(base_extent_px: u32, requested_levels: u8) -> Vec<HiZMipExtent> {
    let mut extents = Vec::with_capacity(requested_levels as usize);
    let mut width = base_extent_px;
    let mut height = base_extent_px;
    for level in 0..requested_levels {
        if width == 0 || height == 0 {
            break;
        }
        extents.push(HiZMipExtent {
            level,
            width_px: width,
            height_px: height,
        });
        width = (width / 2).max(1);
        height = (height / 2).max(1);
        if width == 1 && height == 1 && level + 1 < requested_levels {
            extents.push(HiZMipExtent {
                level: level + 1,
                width_px: 1,
                height_px: 1,
            });
            break;
        }
    }
    extents
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Resource)]
pub struct OcclusionFeedbackBuffer {
    pub schema_version: u16,
    pub byte_size: u64,
    pub element_stride: u32,
    pub element_count: u32,
    pub previous_frame_valid: bool,
}

impl OcclusionFeedbackBuffer {
    #[must_use]
    pub const fn for_max_instance_count(max_instance_count: u32) -> Self {
        let stride = 4u32;
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            byte_size: (stride as u64).saturating_mul(max_instance_count as u64),
            element_stride: stride,
            element_count: max_instance_count,
            previous_frame_valid: false,
        }
    }

    pub fn invalidate(&mut self) {
        self.previous_frame_valid = false;
    }

    pub fn mark_valid(&mut self) {
        self.previous_frame_valid = true;
    }
}

impl Default for OcclusionFeedbackBuffer {
    fn default() -> Self {
        Self::for_max_instance_count(1 << 16)
    }
}

// ============================================================================
// Section 4 — Meshlet metadata + mesh shader vs compute branch
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MeshletClusterBounds {
    pub center: [f32; 3],
    pub radius: f32,
    pub cone_axis: [f32; 3],
    pub cone_cutoff: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct MeshletMetadataRecord {
    pub schema_version: u16,
    pub mesh_id: u32,
    pub meshlet_index: u32,
    pub vertex_offset: u32,
    pub vertex_count: u32,
    pub triangle_offset: u32,
    pub triangle_count: u32,
    pub material_slot: u16,
    pub bounds: MeshletClusterBounds,
}

impl MeshletMetadataRecord {
    #[must_use]
    pub const fn is_within_default_limits(&self) -> bool {
        self.vertex_count <= MESHLET_DEFAULT_VERTEX_LIMIT
            && self.triangle_count <= MESHLET_DEFAULT_TRIANGLE_LIMIT
    }
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct MeshletMetadataTable {
    pub schema_version: u16,
    pub records: Vec<MeshletMetadataRecord>,
    pub records_by_mesh: Vec<(u32, u32)>, // (mesh_id, count)
    pub rejected_oversize_count: u32,
}

impl MeshletMetadataTable {
    pub fn clear(&mut self) {
        self.records.clear();
        self.records_by_mesh.clear();
        self.rejected_oversize_count = 0;
    }

    pub fn record(&mut self, meshlet: MeshletMetadataRecord) -> bool {
        if !meshlet.is_within_default_limits() {
            self.rejected_oversize_count = self.rejected_oversize_count.saturating_add(1);
            return false;
        }
        self.records.push(meshlet);
        if let Some((_, count)) = self
            .records_by_mesh
            .iter_mut()
            .find(|(mesh_id, _)| *mesh_id == meshlet.mesh_id)
        {
            *count = count.saturating_add(1);
        } else {
            self.records_by_mesh.push((meshlet.mesh_id, 1));
        }
        true
    }

    #[must_use]
    pub fn count_for_mesh(&self, mesh_id: u32) -> u32 {
        self.records_by_mesh
            .iter()
            .find(|(candidate, _)| *candidate == mesh_id)
            .map(|(_, count)| *count)
            .unwrap_or(0)
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.records.len() as u32
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeshShaderCapabilityBranch {
    #[default]
    ComputeFallback,
    MeshShader,
    VendorSpecific,
}

impl MeshShaderCapabilityBranch {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComputeFallback => "compute_fallback",
            Self::MeshShader => "mesh_shader",
            Self::VendorSpecific => "vendor_specific",
        }
    }

    #[must_use]
    pub const fn from_capability(mesh_shader_supported: bool) -> Self {
        if mesh_shader_supported {
            Self::MeshShader
        } else {
            Self::ComputeFallback
        }
    }

    #[must_use]
    pub const fn from_cluster_path(path: GpuDrivenClusterPath) -> Self {
        match path {
            GpuDrivenClusterPath::ComputeFallback => Self::ComputeFallback,
            GpuDrivenClusterPath::MeshShaderWhereSupported => Self::MeshShader,
            GpuDrivenClusterPath::MeshShaderRequired => Self::MeshShader,
        }
    }

    #[must_use]
    pub const fn requires_compute_fallback(self) -> bool {
        matches!(self, Self::ComputeFallback)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshletRuntimePlan {
    pub schema_version: u16,
    pub branch: MeshShaderCapabilityBranch,
    pub scaffold: GpuDrivenClusterScaffold,
    pub default_vertex_limit: u32,
    pub default_triangle_limit: u32,
    pub workgroup_size_x: u8,
}

impl MeshletRuntimePlan {
    #[must_use]
    pub fn for_capabilities(mesh_shader_supported: bool) -> Self {
        let scaffold = GpuDrivenClusterScaffold::from_capabilities(mesh_shader_supported);
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            branch: MeshShaderCapabilityBranch::from_capability(mesh_shader_supported),
            scaffold,
            default_vertex_limit: MESHLET_DEFAULT_VERTEX_LIMIT,
            default_triangle_limit: MESHLET_DEFAULT_TRIANGLE_LIMIT,
            workgroup_size_x: 32,
        }
    }
}

impl Default for MeshletRuntimePlan {
    fn default() -> Self {
        Self::for_capabilities(false)
    }
}

// ============================================================================
// Section 5 — Parity validation + runtime metrics
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDrivenParityVerdict {
    #[default]
    NotEvaluated,
    Match,
    DrawCountMismatch,
    TriangleCountMismatch,
    BothMismatch,
    BudgetInconsistent,
}

impl GpuDrivenParityVerdict {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotEvaluated => "not_evaluated",
            Self::Match => "match",
            Self::DrawCountMismatch => "draw_count_mismatch",
            Self::TriangleCountMismatch => "triangle_count_mismatch",
            Self::BothMismatch => "both_mismatch",
            Self::BudgetInconsistent => "budget_inconsistent",
        }
    }

    #[must_use]
    pub const fn is_match(self) -> bool {
        matches!(self, Self::Match)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenParityValidation {
    pub schema_version: u16,
    pub comparison: GpuDrivenParityComparison,
    pub debug_counters: GpuDrivenDebugCounterSnapshot,
    pub verdict: GpuDrivenParityVerdict,
}

impl GpuDrivenParityValidation {
    #[must_use]
    pub fn evaluate(
        comparison: GpuDrivenParityComparison,
        debug_counters: GpuDrivenDebugCounterSnapshot,
    ) -> Self {
        let draw_match = comparison.direct_draw_count == comparison.gpu_driven_draw_count;
        let triangle_match =
            comparison.direct_triangle_count == comparison.gpu_driven_triangle_count;
        let budget_consistent = debug_counters.budget_consistent();
        let verdict = if !budget_consistent {
            GpuDrivenParityVerdict::BudgetInconsistent
        } else if draw_match && triangle_match {
            GpuDrivenParityVerdict::Match
        } else if !draw_match && !triangle_match {
            GpuDrivenParityVerdict::BothMismatch
        } else if !draw_match {
            GpuDrivenParityVerdict::DrawCountMismatch
        } else {
            GpuDrivenParityVerdict::TriangleCountMismatch
        };
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            comparison,
            debug_counters,
            verdict,
        }
    }

    #[must_use]
    pub fn passes(&self) -> bool {
        self.verdict.is_match()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct GpuDrivenRuntimeMetrics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub native_backend: NativeBackend,
    pub indirect_command_count: u32,
    pub indirect_buckets_used: u32,
    pub culling_workgroup_count: u32,
    pub hi_z_mip_count: u8,
    pub previous_frame_available: bool,
    pub conservative_fallback_active: bool,
    pub meshlet_record_count: u32,
    pub meshlet_oversize_rejected: u32,
    pub mesh_shader_branch: MeshShaderCapabilityBranch,
    pub debug_counters: GpuDrivenDebugCounterSnapshot,
    pub parity: GpuDrivenParityValidation,
    pub cpu_culling_setup_ns: u64,
    pub gpu_culling_pass_ns: u64,
    pub indirect_dispatch_ns: u64,
}

impl Default for GpuDrivenRuntimeMetrics {
    fn default() -> Self {
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            frame_index: 0,
            native_backend: NativeBackend::Unknown,
            indirect_command_count: 0,
            indirect_buckets_used: 0,
            culling_workgroup_count: 0,
            hi_z_mip_count: 0,
            previous_frame_available: false,
            conservative_fallback_active: false,
            meshlet_record_count: 0,
            meshlet_oversize_rejected: 0,
            mesh_shader_branch: MeshShaderCapabilityBranch::ComputeFallback,
            debug_counters: GpuDrivenDebugCounterSnapshot::default(),
            parity: GpuDrivenParityValidation::default(),
            cpu_culling_setup_ns: 0,
            gpu_culling_pass_ns: 0,
            indirect_dispatch_ns: 0,
        }
    }
}

impl GpuDrivenRuntimeMetrics {
    #[must_use]
    pub fn from_state(
        plan: &GpuDrivenPlan,
        ir: &IndirectDrawIrList,
        hi_z: &HiZPyramidRuntime,
        meshlets: &MeshletMetadataTable,
        meshlet_plan: MeshletRuntimePlan,
    ) -> Self {
        let buckets_used = ir.bucket_counts.iter().filter(|count| **count > 0).count() as u32;
        Self {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            frame_index: 0,
            native_backend: plan.native_backend,
            indirect_command_count: ir.total(),
            indirect_buckets_used: buckets_used,
            culling_workgroup_count: plan.frustum_culling.workgroup_count,
            hi_z_mip_count: hi_z.mip_count(),
            previous_frame_available: hi_z.previous_frame_available,
            conservative_fallback_active: hi_z.conservative_fallback_active,
            meshlet_record_count: meshlets.total(),
            meshlet_oversize_rejected: meshlets.rejected_oversize_count,
            mesh_shader_branch: meshlet_plan.branch,
            debug_counters: GpuDrivenDebugCounterSnapshot::default(),
            parity: GpuDrivenParityValidation::default(),
            cpu_culling_setup_ns: 0,
            gpu_culling_pass_ns: 0,
            indirect_dispatch_ns: 0,
        }
    }

    pub fn record_debug_counters(&mut self, counters: GpuDrivenDebugCounterSnapshot) {
        self.debug_counters = counters;
    }

    pub fn record_parity(&mut self, validation: GpuDrivenParityValidation) {
        self.parity = validation;
    }

    pub fn record_cpu_culling_setup_ns(&mut self, ns: u64) {
        self.cpu_culling_setup_ns = self.cpu_culling_setup_ns.saturating_add(ns);
    }

    pub fn record_gpu_culling_pass_ns(&mut self, ns: u64) {
        self.gpu_culling_pass_ns = self.gpu_culling_pass_ns.saturating_add(ns);
    }

    pub fn record_indirect_dispatch_ns(&mut self, ns: u64) {
        self.indirect_dispatch_ns = self.indirect_dispatch_ns.saturating_add(ns);
    }

    /// Convert into the existing Pass-17 `GpuDrivenMetrics` shape so
    /// pre-existing telemetry consumers see the new debug counters
    /// and parity result. Pass 17's metrics carry the cluster path
    /// and occlusion policy directly; we map through the plan and
    /// runtime parity verdict.
    #[must_use]
    pub fn to_planning_metrics(&self, plan: &GpuDrivenPlan) -> GpuDrivenMetrics {
        GpuDrivenMetrics::from_plan(plan)
            .with_debug_counters(self.debug_counters)
            .with_parity(self.parity.comparison)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_driven::{
        GpuDrivenBufferSet, GpuDrivenIndirectCommandSignature, GpuDrivenOcclusionPolicy,
        GpuDrivenPlan,
    };

    fn dx12_plan() -> GpuDrivenPlan {
        GpuDrivenPlan::product_for_backend(NativeBackend::Dx12, 1024, false)
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(GPU_DRIVEN_RUNTIME_SCHEMA_VERSION, 1);
        assert_eq!(INDIRECT_DRAW_IR_BUCKET_COUNT, 7);
        assert_eq!(GPU_CULLING_BINDING_COUNT, 6);
    }

    #[test]
    fn indirect_bucket_default_participation_excludes_ui_and_debug() {
        assert!(IndirectDrawBucket::Opaque.participates_in_gpu_driven_indirect_default());
        assert!(IndirectDrawBucket::AlphaTest.participates_in_gpu_driven_indirect_default());
        assert!(IndirectDrawBucket::Transparent.participates_in_gpu_driven_indirect_default());
        assert!(IndirectDrawBucket::Shadow.participates_in_gpu_driven_indirect_default());
        assert!(!IndirectDrawBucket::Ui.participates_in_gpu_driven_indirect_default());
        assert!(!IndirectDrawBucket::Debug.participates_in_gpu_driven_indirect_default());
        assert!(!IndirectDrawBucket::Depth.participates_in_gpu_driven_indirect_default());
    }

    #[test]
    fn indirect_signature_supports_indirect_count_for_all_native_backends() {
        let dx12 = IndirectCommandSignature::from_planning(
            GpuDrivenIndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed,
        );
        let vulkan = IndirectCommandSignature::from_planning(
            GpuDrivenIndirectCommandSignature::VulkanDrawIndexedIndirectCount,
        );
        let metal = IndirectCommandSignature::from_planning(
            GpuDrivenIndirectCommandSignature::MetalDrawIndexedIndirectCommands,
        );
        let unknown = IndirectCommandSignature::from_planning(
            GpuDrivenIndirectCommandSignature::NotApplicableForBackend,
        );
        assert!(dx12.supports_indirect_count());
        assert!(vulkan.supports_indirect_count());
        assert!(metal.supports_indirect_count());
        assert!(!unknown.supports_indirect_count());
    }

    #[test]
    fn indirect_buffer_kind_round_trips_through_gpu_driven_kind() {
        assert_eq!(
            IndirectBufferKind::from_gpu_driven_kind(GpuDrivenBufferKind::DrawArguments),
            Some(IndirectBufferKind::Args),
        );
        assert_eq!(
            IndirectBufferKind::from_gpu_driven_kind(GpuDrivenBufferKind::DrawCount),
            Some(IndirectBufferKind::Count),
        );
        assert_eq!(
            IndirectBufferKind::from_gpu_driven_kind(GpuDrivenBufferKind::VisibleInstanceList),
            Some(IndirectBufferKind::VisibleInstances),
        );
        assert_eq!(
            IndirectBufferKind::from_gpu_driven_kind(GpuDrivenBufferKind::HiZPyramid),
            None,
        );
    }

    #[test]
    fn indirect_command_is_invalid_without_args_and_count_buffers() {
        let cmd = IndirectDrawIrCommand::new(
            IndirectDrawBucket::Opaque,
            IndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed,
            128,
        );
        assert!(!cmd.is_valid());
    }

    #[test]
    fn indirect_command_built_for_buffer_set_is_valid() {
        let buffer_set = GpuDrivenBufferSet::for_max_draw_count(256);
        let mut list = IndirectDrawIrList::default();
        list.record_for_buffer_set(
            buffer_set,
            IndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed,
        );
        assert!(list.total() > 0);
        for command in &list.commands {
            assert!(command.is_valid());
        }
    }

    #[test]
    fn indirect_list_records_only_default_participating_buckets() {
        let buffer_set = GpuDrivenBufferSet::for_max_draw_count(256);
        let mut list = IndirectDrawIrList::default();
        list.record_for_buffer_set(
            buffer_set,
            IndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed,
        );
        assert_eq!(list.count_for(IndirectDrawBucket::Opaque), 1);
        assert_eq!(list.count_for(IndirectDrawBucket::AlphaTest), 1);
        assert_eq!(list.count_for(IndirectDrawBucket::Transparent), 1);
        assert_eq!(list.count_for(IndirectDrawBucket::Shadow), 1);
        assert_eq!(list.count_for(IndirectDrawBucket::Ui), 0);
        assert_eq!(list.count_for(IndirectDrawBucket::Debug), 0);
        assert_eq!(list.count_for(IndirectDrawBucket::Depth), 0);
    }

    #[test]
    fn culling_pass_bindings_assign_correct_access_per_slot() {
        let buffer_set = GpuDrivenBufferSet::for_max_draw_count(256);
        let bindings = GpuCullingPassBindings::for_buffer_set(buffer_set);
        assert!(matches!(
            bindings.binding(GpuCullingBindingSlot::ObjectBounds).access,
            GpuCullingBindingAccess::Read,
        ));
        assert!(matches!(
            bindings.binding(GpuCullingBindingSlot::CameraPlanes).access,
            GpuCullingBindingAccess::Read,
        ));
        assert!(matches!(
            bindings
                .binding(GpuCullingBindingSlot::VisibleInstanceList)
                .access,
            GpuCullingBindingAccess::Write,
        ));
        assert!(matches!(
            bindings.binding(GpuCullingBindingSlot::DrawCount).access,
            GpuCullingBindingAccess::AtomicWrite,
        ));
    }

    #[test]
    fn culling_pass_workgroup_count_covers_max_instance_count() {
        let buffer_set = GpuDrivenBufferSet::for_max_draw_count(1_000);
        let bindings = GpuCullingPassBindings::for_buffer_set(buffer_set);
        assert!(bindings.plan.workgroup_count >= 16);
        assert_eq!(bindings.plan.max_instance_count, 1_000);
    }

    #[test]
    fn hi_z_pyramid_default_has_eight_mip_levels() {
        let runtime = HiZPyramidRuntime::default();
        assert_eq!(runtime.plan.mip_levels, HI_Z_DEFAULT_MIP_LEVELS);
        assert!(runtime.mip_count() <= HI_Z_DEFAULT_MIP_LEVELS);
        assert!(runtime.mip_count() > 0);
    }

    #[test]
    fn hi_z_mip_chain_halves_extents_each_level() {
        let runtime = HiZPyramidRuntime::new(GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT, 256);
        let extents = &runtime.mip_extents;
        for window in extents.windows(2) {
            let prev = window[0];
            let next = window[1];
            assert!(next.width_px <= prev.width_px);
            assert!(next.height_px <= prev.height_px);
        }
    }

    #[test]
    fn hi_z_no_previous_frame_forces_conservative_fallback() {
        let mut runtime = HiZPyramidRuntime::new(
            GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT
                .with_policy(GpuDrivenOcclusionPolicy::Aggressive),
            256,
        );
        runtime.note_previous_frame(false);
        assert!(runtime.conservative_fallback_active);
        assert!(runtime.classifies_uncertain_as_visible());
    }

    #[test]
    fn hi_z_aggressive_policy_with_previous_frame_disables_conservative_fallback() {
        let mut runtime = HiZPyramidRuntime::new(
            GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT
                .with_policy(GpuDrivenOcclusionPolicy::Aggressive),
            256,
        );
        runtime.note_previous_frame(true);
        assert!(!runtime.conservative_fallback_active);
        assert!(!runtime.classifies_uncertain_as_visible());
    }

    #[test]
    fn occlusion_feedback_buffer_default_size_matches_max_instance_count() {
        let buffer = OcclusionFeedbackBuffer::for_max_instance_count(1024);
        assert_eq!(buffer.element_count, 1024);
        assert_eq!(buffer.byte_size, 1024 * 4);
        assert!(!buffer.previous_frame_valid);
    }

    #[test]
    fn meshlet_metadata_table_records_within_limits() {
        let mut table = MeshletMetadataTable::default();
        let record = MeshletMetadataRecord {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            mesh_id: 1,
            meshlet_index: 0,
            vertex_offset: 0,
            vertex_count: MESHLET_DEFAULT_VERTEX_LIMIT,
            triangle_offset: 0,
            triangle_count: MESHLET_DEFAULT_TRIANGLE_LIMIT,
            material_slot: 0,
            bounds: MeshletClusterBounds::default(),
        };
        assert!(table.record(record));
        assert_eq!(table.total(), 1);
        assert_eq!(table.count_for_mesh(1), 1);
    }

    #[test]
    fn meshlet_oversize_record_is_rejected() {
        let mut table = MeshletMetadataTable::default();
        let record = MeshletMetadataRecord {
            schema_version: GPU_DRIVEN_RUNTIME_SCHEMA_VERSION,
            mesh_id: 1,
            meshlet_index: 0,
            vertex_offset: 0,
            vertex_count: MESHLET_DEFAULT_VERTEX_LIMIT + 1,
            triangle_offset: 0,
            triangle_count: 64,
            material_slot: 0,
            bounds: MeshletClusterBounds::default(),
        };
        assert!(!table.record(record));
        assert_eq!(table.rejected_oversize_count, 1);
    }

    #[test]
    fn mesh_shader_branch_routes_to_compute_when_unsupported() {
        assert_eq!(
            MeshShaderCapabilityBranch::from_capability(false),
            MeshShaderCapabilityBranch::ComputeFallback,
        );
        assert!(MeshShaderCapabilityBranch::ComputeFallback.requires_compute_fallback());
    }

    #[test]
    fn mesh_shader_branch_routes_to_mesh_shader_when_supported() {
        assert_eq!(
            MeshShaderCapabilityBranch::from_capability(true),
            MeshShaderCapabilityBranch::MeshShader,
        );
        assert!(!MeshShaderCapabilityBranch::MeshShader.requires_compute_fallback());
    }

    #[test]
    fn meshlet_runtime_plan_uses_default_limits() {
        let plan = MeshletRuntimePlan::for_capabilities(false);
        assert_eq!(plan.default_vertex_limit, MESHLET_DEFAULT_VERTEX_LIMIT);
        assert_eq!(plan.default_triangle_limit, MESHLET_DEFAULT_TRIANGLE_LIMIT);
        assert_eq!(plan.branch, MeshShaderCapabilityBranch::ComputeFallback);
    }

    #[test]
    fn parity_match_is_recorded_when_counts_align() {
        let comparison = GpuDrivenParityComparison::from_counts(128, 128, 4_000, 4_000);
        let counters = GpuDrivenDebugCounterSnapshot::from_slots([100, 90, 5, 3, 2, 0, 0, 0]);
        let validation = GpuDrivenParityValidation::evaluate(comparison, counters);
        assert_eq!(validation.verdict, GpuDrivenParityVerdict::Match);
        assert!(validation.passes());
    }

    #[test]
    fn parity_draw_count_mismatch_is_classified() {
        let comparison = GpuDrivenParityComparison::from_counts(128, 100, 4_000, 4_000);
        let counters = GpuDrivenDebugCounterSnapshot::from_slots([100, 90, 5, 3, 2, 0, 0, 0]);
        let validation = GpuDrivenParityValidation::evaluate(comparison, counters);
        assert_eq!(
            validation.verdict,
            GpuDrivenParityVerdict::DrawCountMismatch,
        );
    }

    #[test]
    fn parity_budget_inconsistent_is_classified() {
        let comparison = GpuDrivenParityComparison::from_counts(128, 128, 4_000, 4_000);
        let counters = GpuDrivenDebugCounterSnapshot::from_slots([10, 90, 5, 3, 2, 0, 0, 0]);
        // visible + frustum + occluded + uncertain = 100 > tested = 10 -> inconsistent
        let validation = GpuDrivenParityValidation::evaluate(comparison, counters);
        assert_eq!(
            validation.verdict,
            GpuDrivenParityVerdict::BudgetInconsistent,
        );
    }

    #[test]
    fn runtime_metrics_aggregate_state_from_plan_and_resources() {
        let plan = dx12_plan();
        let mut ir = IndirectDrawIrList::default();
        ir.record_for_buffer_set(
            plan.buffer_set,
            IndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed,
        );
        let hi_z = HiZPyramidRuntime::new(plan.hi_z_pyramid, 1024);
        let meshlets = MeshletMetadataTable::default();
        let meshlet_plan = MeshletRuntimePlan::for_capabilities(false);
        let metrics =
            GpuDrivenRuntimeMetrics::from_state(&plan, &ir, &hi_z, &meshlets, meshlet_plan);
        assert_eq!(metrics.indirect_command_count, ir.total());
        assert_eq!(metrics.native_backend, NativeBackend::Dx12);
        assert_eq!(metrics.hi_z_mip_count, hi_z.mip_count());
        assert!(metrics.conservative_fallback_active);
    }

    #[test]
    fn runtime_metrics_record_timings_accumulate() {
        let mut metrics = GpuDrivenRuntimeMetrics::default();
        metrics.record_cpu_culling_setup_ns(1_000);
        metrics.record_cpu_culling_setup_ns(2_000);
        metrics.record_gpu_culling_pass_ns(500);
        metrics.record_indirect_dispatch_ns(100);
        assert_eq!(metrics.cpu_culling_setup_ns, 3_000);
        assert_eq!(metrics.gpu_culling_pass_ns, 500);
        assert_eq!(metrics.indirect_dispatch_ns, 100);
    }

    #[test]
    fn runtime_metrics_to_planning_metrics_carries_debug_and_parity() {
        let plan = dx12_plan();
        let mut metrics = GpuDrivenRuntimeMetrics::default();
        let counters = GpuDrivenDebugCounterSnapshot::from_slots([100, 90, 5, 3, 2, 0, 0, 0]);
        let comparison = GpuDrivenParityComparison::from_counts(128, 128, 4_000, 4_000);
        let validation = GpuDrivenParityValidation::evaluate(comparison, counters);
        metrics.record_debug_counters(counters);
        metrics.record_parity(validation);
        let lifted = metrics.to_planning_metrics(&plan);
        assert_eq!(lifted.debug_counters, counters);
        assert_eq!(lifted.parity, comparison);
    }

    #[test]
    fn ir_list_clear_resets_counts() {
        let buffer_set = GpuDrivenBufferSet::for_max_draw_count(256);
        let mut list = IndirectDrawIrList::default();
        list.record_for_buffer_set(
            buffer_set,
            IndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed,
        );
        assert!(list.total() > 0);
        list.clear();
        assert_eq!(list.total(), 0);
        for bucket in IndirectDrawBucket::ALL {
            assert_eq!(list.count_for(bucket), 0);
        }
    }

    #[test]
    fn culling_binding_slot_read_write_classification_matches_pass_25_contract() {
        for slot in GpuCullingBindingSlot::ALL {
            match slot {
                GpuCullingBindingSlot::ObjectBounds | GpuCullingBindingSlot::CameraPlanes => {
                    assert!(slot.is_read());
                    assert!(!slot.is_write());
                }
                _ => {
                    assert!(slot.is_write());
                    assert!(!slot.is_read());
                }
            }
        }
    }

    #[test]
    fn gpu_driven_planning_metrics_round_trip_carries_cluster_path() {
        let plan = GpuDrivenPlan::product_for_backend(NativeBackend::Vulkan, 512, true);
        let lifted = GpuDrivenMetrics::from_plan(&plan);
        assert_eq!(lifted.cluster_path, plan.cluster_scaffold.path);
        assert_eq!(lifted.native_backend, NativeBackend::Vulkan);
    }
}
