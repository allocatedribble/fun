use crate::compute_culling::{FunGpuDrawIndirectArgs, FunGpuVisibilityCounters};

use super::types::{
    GpuBoundsRecord, GpuDrawBucket, GpuInstanceRecord, GpuObjectRecord, GpuVisibilityViewConstants,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpuVisibilityBufferLifetime {
    Persistent,
    PerFrame,
}

impl GpuVisibilityBufferLifetime {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Persistent => "persistent",
            Self::PerFrame => "per_frame",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpuVisibilityBufferKind {
    ObjectRecords,
    InstanceRecords,
    BoundsRecords,
    CellVisibility,
    BatchVisibility,
    InstanceVisibility,
    LodSelection,
    CompactVisibleIds,
    DrawBuckets,
    IndirectArgs,
    Counters,
    ViewConstants,
}

impl GpuVisibilityBufferKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObjectRecords => "object_records",
            Self::InstanceRecords => "instance_records",
            Self::BoundsRecords => "bounds_records",
            Self::CellVisibility => "cell_visibility",
            Self::BatchVisibility => "batch_visibility",
            Self::InstanceVisibility => "instance_visibility",
            Self::LodSelection => "lod_selection",
            Self::CompactVisibleIds => "compact_visible_ids",
            Self::DrawBuckets => "draw_buckets",
            Self::IndirectArgs => "indirect_args",
            Self::Counters => "counters",
            Self::ViewConstants => "view_constants",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuVisibilityBufferDescriptor {
    pub kind: GpuVisibilityBufferKind,
    pub lifetime: GpuVisibilityBufferLifetime,
    pub stride_bytes: u64,
    pub alignment_bytes: u64,
    pub full_buffer_write_allowed: bool,
    pub normal_readback_allowed: bool,
}

pub const GPU_VISIBILITY_BUFFER_PLAN: &[GpuVisibilityBufferDescriptor] = &[
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::ObjectRecords,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: GpuObjectRecord::SIZE_BYTES,
        alignment_bytes: GpuObjectRecord::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::InstanceRecords,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: GpuInstanceRecord::SIZE_BYTES,
        alignment_bytes: GpuInstanceRecord::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::BoundsRecords,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: GpuBoundsRecord::SIZE_BYTES,
        alignment_bytes: GpuBoundsRecord::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::CellVisibility,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: 4,
        alignment_bytes: 4,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::BatchVisibility,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: 4,
        alignment_bytes: 4,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::InstanceVisibility,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: 4,
        alignment_bytes: 4,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::LodSelection,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: 4,
        alignment_bytes: 4,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::CompactVisibleIds,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: 4,
        alignment_bytes: 4,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::DrawBuckets,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: std::mem::size_of::<GpuDrawBucket>() as u64,
        alignment_bytes: 8,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::IndirectArgs,
        lifetime: GpuVisibilityBufferLifetime::Persistent,
        stride_bytes: FunGpuDrawIndirectArgs::SIZE_BYTES,
        alignment_bytes: FunGpuDrawIndirectArgs::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::Counters,
        lifetime: GpuVisibilityBufferLifetime::PerFrame,
        stride_bytes: FunGpuVisibilityCounters::SIZE_BYTES,
        alignment_bytes: FunGpuVisibilityCounters::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: true,
    },
    GpuVisibilityBufferDescriptor {
        kind: GpuVisibilityBufferKind::ViewConstants,
        lifetime: GpuVisibilityBufferLifetime::PerFrame,
        stride_bytes: GpuVisibilityViewConstants::SIZE_BYTES,
        alignment_bytes: GpuVisibilityViewConstants::ALIGN_BYTES,
        full_buffer_write_allowed: false,
        normal_readback_allowed: false,
    },
];

pub fn gpu_visibility_buffer_descriptor(
    kind: GpuVisibilityBufferKind,
) -> Option<GpuVisibilityBufferDescriptor> {
    GPU_VISIBILITY_BUFFER_PLAN
        .iter()
        .copied()
        .find(|descriptor| descriptor.kind == kind)
}
