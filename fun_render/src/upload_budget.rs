use bevy::render::render_resource::COPY_BUFFER_ALIGNMENT;

use crate::UploadWriteLabel;

const FULL_BUFFER_DIRTY_RATIO_THRESHOLD_PER_MILLE: u64 = 750;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunUploadBudget {
    pub frame_index: u64,
    pub small_buffer_budget_bytes: u64,
    pub large_buffer_budget_bytes: u64,
    pub texture_budget_bytes: u64,
    pub cef_budget_bytes: u64,
    pub readback_budget_bytes: u64,
    pub max_upload_calls_per_frame: u32,
    pub max_texture_uploads_per_frame: u32,
    pub max_deferred_frames: u8,
}

impl Default for FunUploadBudget {
    fn default() -> Self {
        Self {
            frame_index: 0,
            small_buffer_budget_bytes: 256 * 1024,
            large_buffer_budget_bytes: 4 * 1024 * 1024,
            texture_budget_bytes: 16 * 1024 * 1024,
            cef_budget_bytes: 8 * 1024 * 1024,
            readback_budget_bytes: 0,
            max_upload_calls_per_frame: 512,
            max_texture_uploads_per_frame: 64,
            max_deferred_frames: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunUploadBudgetDecision {
    Admit,
    Defer,
    FallbackRawWrite,
    RejectOversized,
    RejectUnaligned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunUploadSubsystem {
    WorldStream,
    MeshletInstance,
    MeshletMaterial,
    MeshletVisibility,
    StaticMeshAsset,
    DynamicMesh,
    MaterialUniform,
    TextureAsset,
    CefCpuPaint,
    CefGpuInterop,
    Clouds,
    Solari,
    Dlss,
    Readback,
    DebugOverlay,
}

impl FunUploadSubsystem {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorldStream => "world_stream",
            Self::MeshletInstance => "meshlet_instance",
            Self::MeshletMaterial => "meshlet_material",
            Self::MeshletVisibility => "meshlet_visibility",
            Self::StaticMeshAsset => "static_mesh_asset",
            Self::DynamicMesh => "dynamic_mesh",
            Self::MaterialUniform => "material_uniform",
            Self::TextureAsset => "texture_asset",
            Self::CefCpuPaint => "cef_cpu_paint",
            Self::CefGpuInterop => "cef_gpu_interop",
            Self::Clouds => "clouds",
            Self::Solari => "solari",
            Self::Dlss => "dlss",
            Self::Readback => "readback",
            Self::DebugOverlay => "debug_overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunUploadBudgetClass {
    SmallBuffer,
    LargeBuffer,
    Texture,
    Cef,
    Readback,
}

impl FunUploadBudgetClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SmallBuffer => "small_buffer",
            Self::LargeBuffer => "large_buffer",
            Self::Texture => "texture",
            Self::Cef => "cef",
            Self::Readback => "readback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunUploadWriteIntent {
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub budget_class: FunUploadBudgetClass,
    pub bytes: u64,
    pub offset: u64,
    pub buffer_size: u64,
    pub dirty_bytes: u64,
    pub buffer_created_or_resized: bool,
}

impl FunUploadWriteIntent {
    pub const fn small_buffer(
        label: UploadWriteLabel,
        subsystem: FunUploadSubsystem,
        bytes: u64,
        offset: u64,
    ) -> Self {
        Self {
            label,
            subsystem,
            budget_class: FunUploadBudgetClass::SmallBuffer,
            bytes,
            offset,
            buffer_size: 0,
            dirty_bytes: bytes,
            buffer_created_or_resized: false,
        }
    }

    pub const fn large_buffer(
        label: UploadWriteLabel,
        subsystem: FunUploadSubsystem,
        bytes: u64,
        offset: u64,
        buffer_size: u64,
        dirty_bytes: u64,
        buffer_created_or_resized: bool,
    ) -> Self {
        Self {
            label,
            subsystem,
            budget_class: FunUploadBudgetClass::LargeBuffer,
            bytes,
            offset,
            buffer_size,
            dirty_bytes,
            buffer_created_or_resized,
        }
    }

    pub const fn texture(
        label: UploadWriteLabel,
        subsystem: FunUploadSubsystem,
        bytes: u64,
    ) -> Self {
        Self {
            label,
            subsystem,
            budget_class: FunUploadBudgetClass::Texture,
            bytes,
            offset: 0,
            buffer_size: 0,
            dirty_bytes: bytes,
            buffer_created_or_resized: false,
        }
    }

    pub const fn cef_cpu(label: UploadWriteLabel, bytes: u64) -> Self {
        Self {
            label,
            subsystem: FunUploadSubsystem::CefCpuPaint,
            budget_class: FunUploadBudgetClass::Cef,
            bytes,
            offset: 0,
            buffer_size: 0,
            dirty_bytes: bytes,
            buffer_created_or_resized: false,
        }
    }

    pub const fn readback(label: UploadWriteLabel, bytes: u64) -> Self {
        Self {
            label,
            subsystem: FunUploadSubsystem::Readback,
            budget_class: FunUploadBudgetClass::Readback,
            bytes,
            offset: 0,
            buffer_size: 0,
            dirty_bytes: bytes,
            buffer_created_or_resized: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FunUploadBudgetUsage {
    pub total_calls: u32,
    pub texture_uploads: u32,
    pub small_buffer_bytes: u64,
    pub large_buffer_bytes: u64,
    pub texture_bytes: u64,
    pub cef_bytes: u64,
    pub readback_bytes: u64,
    pub deferred_calls: u32,
    pub deferred_bytes: u64,
    pub raw_write_fallbacks: u32,
    pub budget_exceeded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunUploadBudgetTracker {
    budget: FunUploadBudget,
    usage: FunUploadBudgetUsage,
}

impl FunUploadBudgetTracker {
    pub const fn new(budget: FunUploadBudget) -> Self {
        Self {
            budget,
            usage: FunUploadBudgetUsage {
                total_calls: 0,
                texture_uploads: 0,
                small_buffer_bytes: 0,
                large_buffer_bytes: 0,
                texture_bytes: 0,
                cef_bytes: 0,
                readback_bytes: 0,
                deferred_calls: 0,
                deferred_bytes: 0,
                raw_write_fallbacks: 0,
                budget_exceeded: false,
            },
        }
    }

    pub const fn budget(&self) -> FunUploadBudget {
        self.budget
    }

    pub const fn usage(&self) -> FunUploadBudgetUsage {
        self.usage
    }

    pub fn start_frame(&mut self, frame_index: u64) {
        self.budget.frame_index = frame_index;
        self.usage = FunUploadBudgetUsage::default();
    }

    pub fn decide(&mut self, intent: FunUploadWriteIntent) -> FunUploadBudgetDecision {
        if intent.bytes == 0 {
            return FunUploadBudgetDecision::Admit;
        }

        if self.usage.total_calls >= self.budget.max_upload_calls_per_frame {
            return self.record_defer(intent.bytes);
        }

        if matches!(
            intent.budget_class,
            FunUploadBudgetClass::SmallBuffer | FunUploadBudgetClass::LargeBuffer
        ) && !aligned_buffer_write(intent.offset, intent.bytes)
        {
            self.usage.raw_write_fallbacks = self.usage.raw_write_fallbacks.saturating_add(1);
            return FunUploadBudgetDecision::FallbackRawWrite;
        }

        if is_full_buffer_write(intent) && !full_buffer_write_allowed(intent) {
            return self.record_defer(intent.bytes);
        }

        if matches!(intent.budget_class, FunUploadBudgetClass::Texture) {
            if self.usage.texture_uploads >= self.budget.max_texture_uploads_per_frame {
                return self.record_defer(intent.bytes);
            }
            self.usage.texture_uploads = self.usage.texture_uploads.saturating_add(1);
        }

        let Some(remaining) = self.remaining_bytes(intent.budget_class) else {
            return self.record_reject_oversized();
        };

        if intent.bytes > remaining {
            if self.current_bytes(intent.budget_class) == 0
                && intent.bytes > self.class_budget(intent.budget_class)
            {
                return self.record_reject_oversized();
            }
            return self.record_defer(intent.bytes);
        }

        self.usage.total_calls = self.usage.total_calls.saturating_add(1);
        self.add_class_bytes(intent.budget_class, intent.bytes);
        FunUploadBudgetDecision::Admit
    }

    fn record_defer(&mut self, bytes: u64) -> FunUploadBudgetDecision {
        self.usage.budget_exceeded = true;
        self.usage.deferred_calls = self.usage.deferred_calls.saturating_add(1);
        self.usage.deferred_bytes = self.usage.deferred_bytes.saturating_add(bytes);
        FunUploadBudgetDecision::Defer
    }

    fn record_reject_oversized(&mut self) -> FunUploadBudgetDecision {
        self.usage.budget_exceeded = true;
        FunUploadBudgetDecision::RejectOversized
    }

    const fn class_budget(&self, budget_class: FunUploadBudgetClass) -> u64 {
        match budget_class {
            FunUploadBudgetClass::SmallBuffer => self.budget.small_buffer_budget_bytes,
            FunUploadBudgetClass::LargeBuffer => self.budget.large_buffer_budget_bytes,
            FunUploadBudgetClass::Texture => self.budget.texture_budget_bytes,
            FunUploadBudgetClass::Cef => self.budget.cef_budget_bytes,
            FunUploadBudgetClass::Readback => self.budget.readback_budget_bytes,
        }
    }

    const fn current_bytes(&self, budget_class: FunUploadBudgetClass) -> u64 {
        match budget_class {
            FunUploadBudgetClass::SmallBuffer => self.usage.small_buffer_bytes,
            FunUploadBudgetClass::LargeBuffer => self.usage.large_buffer_bytes,
            FunUploadBudgetClass::Texture => self.usage.texture_bytes,
            FunUploadBudgetClass::Cef => self.usage.cef_bytes,
            FunUploadBudgetClass::Readback => self.usage.readback_bytes,
        }
    }

    const fn remaining_bytes(&self, budget_class: FunUploadBudgetClass) -> Option<u64> {
        self.class_budget(budget_class)
            .checked_sub(self.current_bytes(budget_class))
    }

    fn add_class_bytes(&mut self, budget_class: FunUploadBudgetClass, bytes: u64) {
        match budget_class {
            FunUploadBudgetClass::SmallBuffer => {
                self.usage.small_buffer_bytes = self.usage.small_buffer_bytes.saturating_add(bytes);
            }
            FunUploadBudgetClass::LargeBuffer => {
                self.usage.large_buffer_bytes = self.usage.large_buffer_bytes.saturating_add(bytes);
            }
            FunUploadBudgetClass::Texture => {
                self.usage.texture_bytes = self.usage.texture_bytes.saturating_add(bytes);
            }
            FunUploadBudgetClass::Cef => {
                self.usage.cef_bytes = self.usage.cef_bytes.saturating_add(bytes);
            }
            FunUploadBudgetClass::Readback => {
                self.usage.readback_bytes = self.usage.readback_bytes.saturating_add(bytes);
            }
        }
    }
}

const fn aligned_buffer_write(offset: u64, bytes: u64) -> bool {
    offset.is_multiple_of(COPY_BUFFER_ALIGNMENT) && bytes.is_multiple_of(COPY_BUFFER_ALIGNMENT)
}

const fn is_full_buffer_write(intent: FunUploadWriteIntent) -> bool {
    intent.buffer_size != 0 && intent.offset == 0 && intent.bytes >= intent.buffer_size
}

const fn full_buffer_write_allowed(intent: FunUploadWriteIntent) -> bool {
    if intent.buffer_created_or_resized {
        return true;
    }
    if intent.buffer_size == 0 {
        return false;
    }
    intent.dirty_bytes.saturating_mul(1000)
        >= intent
            .buffer_size
            .saturating_mul(FULL_BUFFER_DIRTY_RATIO_THRESHOLD_PER_MILLE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        UPLOAD_CEF_CPU_FULL_FRAME, UPLOAD_MESHLET_INSTANCE_RANGE, UPLOAD_MESHLET_MATERIAL_RANGE,
    };

    #[test]
    fn budget_admits_small_aligned_write() {
        let mut tracker = FunUploadBudgetTracker::new(FunUploadBudget {
            small_buffer_budget_bytes: COPY_BUFFER_ALIGNMENT * 4,
            ..Default::default()
        });

        assert_eq!(
            tracker.decide(FunUploadWriteIntent::small_buffer(
                UPLOAD_MESHLET_INSTANCE_RANGE,
                FunUploadSubsystem::MeshletInstance,
                COPY_BUFFER_ALIGNMENT,
                0,
            )),
            FunUploadBudgetDecision::Admit
        );
        assert_eq!(tracker.usage().small_buffer_bytes, COPY_BUFFER_ALIGNMENT);
    }

    #[test]
    fn budget_defers_when_frame_budget_exceeded() {
        let mut tracker = FunUploadBudgetTracker::new(FunUploadBudget {
            small_buffer_budget_bytes: COPY_BUFFER_ALIGNMENT,
            ..Default::default()
        });

        assert_eq!(
            tracker.decide(FunUploadWriteIntent::small_buffer(
                UPLOAD_MESHLET_INSTANCE_RANGE,
                FunUploadSubsystem::MeshletInstance,
                COPY_BUFFER_ALIGNMENT,
                0,
            )),
            FunUploadBudgetDecision::Admit
        );
        assert_eq!(
            tracker.decide(FunUploadWriteIntent::small_buffer(
                UPLOAD_MESHLET_MATERIAL_RANGE,
                FunUploadSubsystem::MeshletMaterial,
                COPY_BUFFER_ALIGNMENT,
                0,
            )),
            FunUploadBudgetDecision::Defer
        );
        assert!(tracker.usage().budget_exceeded);
        assert_eq!(tracker.usage().deferred_bytes, COPY_BUFFER_ALIGNMENT);
    }

    #[test]
    fn budget_counts_cef_separately_from_world_uploads() {
        let mut tracker = FunUploadBudgetTracker::new(FunUploadBudget {
            texture_budget_bytes: 0,
            cef_budget_bytes: 4096,
            ..Default::default()
        });

        assert_eq!(
            tracker.decide(FunUploadWriteIntent::cef_cpu(
                UPLOAD_CEF_CPU_FULL_FRAME,
                4096,
            )),
            FunUploadBudgetDecision::Admit
        );
        assert_eq!(tracker.usage().cef_bytes, 4096);
        assert_eq!(tracker.usage().texture_bytes, 0);
        assert_eq!(tracker.usage().texture_uploads, 0);
    }

    #[test]
    fn full_buffer_write_requires_resize_or_high_dirty_ratio() {
        let mut tracker = FunUploadBudgetTracker::new(FunUploadBudget {
            large_buffer_budget_bytes: 16_384,
            ..Default::default()
        });

        let low_dirty_full_write = FunUploadWriteIntent::large_buffer(
            UPLOAD_MESHLET_MATERIAL_RANGE,
            FunUploadSubsystem::MeshletMaterial,
            4096,
            0,
            4096,
            512,
            false,
        );
        assert_eq!(
            tracker.decide(low_dirty_full_write),
            FunUploadBudgetDecision::Defer
        );

        tracker.start_frame(1);
        assert_eq!(
            tracker.decide(FunUploadWriteIntent {
                dirty_bytes: 4096,
                ..low_dirty_full_write
            }),
            FunUploadBudgetDecision::Admit
        );

        tracker.start_frame(2);
        assert_eq!(
            tracker.decide(FunUploadWriteIntent {
                dirty_bytes: 512,
                buffer_created_or_resized: true,
                ..low_dirty_full_write
            }),
            FunUploadBudgetDecision::Admit
        );
    }

    #[test]
    fn dirty_range_write_prefers_staging_belt() {
        let mut tracker = FunUploadBudgetTracker::new(FunUploadBudget {
            small_buffer_budget_bytes: COPY_BUFFER_ALIGNMENT * 4,
            large_buffer_budget_bytes: 16_384,
            ..Default::default()
        });

        assert_eq!(
            tracker.decide(FunUploadWriteIntent::large_buffer(
                UPLOAD_MESHLET_INSTANCE_RANGE,
                FunUploadSubsystem::MeshletInstance,
                4096,
                0,
                4096,
                COPY_BUFFER_ALIGNMENT,
                false,
            )),
            FunUploadBudgetDecision::Defer
        );

        tracker.start_frame(1);
        assert_eq!(
            tracker.decide(FunUploadWriteIntent::small_buffer(
                UPLOAD_MESHLET_INSTANCE_RANGE,
                FunUploadSubsystem::MeshletInstance,
                COPY_BUFFER_ALIGNMENT,
                COPY_BUFFER_ALIGNMENT,
            )),
            FunUploadBudgetDecision::Admit
        );
        assert_eq!(tracker.usage().small_buffer_bytes, COPY_BUFFER_ALIGNMENT);
        assert_eq!(tracker.usage().large_buffer_bytes, 0);
    }
}
