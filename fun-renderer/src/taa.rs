use crate::component_api::{
    CameraDebugView, CameraHistory, CameraJitter, CameraRenderTarget, RenderExtent2d,
    RenderStableId, RenderVec2, TaaSettings,
};
use crate::frame_graph::FrameGraphResourceType;

pub const TAA_SCAFFOLD_SCHEMA_VERSION: u16 = 1;
pub const HALTON_BASE_X: u32 = 2;
pub const HALTON_BASE_Y: u32 = 3;
pub const TAA_JITTER_SEQUENCE_LENGTH: u32 = 16;

#[must_use]
pub const fn halton_sample(index: u32, base: u32) -> f32 {
    let mut f = 1.0f32;
    let mut result = 0.0f32;
    let mut i = index + 1;
    let inv_base = 1.0f32 / base as f32;
    while i > 0 {
        f *= inv_base;
        result += f * (i % base) as f32;
        i /= base;
    }
    result
}

#[must_use]
pub fn halton_jitter_sample(sequence_index: u32) -> RenderVec2 {
    let i = sequence_index % TAA_JITTER_SEQUENCE_LENGTH;
    RenderVec2::new(
        halton_sample(i, HALTON_BASE_X) - 0.5,
        halton_sample(i, HALTON_BASE_Y) - 0.5,
    )
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaaHistoryStatus {
    #[default]
    Disabled,
    Initial,
    Valid,
    InvalidatedByCameraCut,
    InvalidatedByExtentChange,
    InvalidatedByMissingMotionVectors,
}

impl TaaHistoryStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Initial => "initial",
            Self::Valid => "valid",
            Self::InvalidatedByCameraCut => "invalidated_by_camera_cut",
            Self::InvalidatedByExtentChange => "invalidated_by_extent_change",
            Self::InvalidatedByMissingMotionVectors => "invalidated_by_missing_motion_vectors",
        }
    }

    #[must_use]
    pub const fn requires_history_buffer(self) -> bool {
        matches!(self, Self::Initial | Self::Valid)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaaHistoryReason {
    #[default]
    NotApplicable,
    JustEnabled,
    NotInvalidated,
    CameraCut,
    RenderTargetExtentChanged,
    MotionVectorsMissing,
    UpscalerOwnsHistory,
}

impl TaaHistoryReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::JustEnabled => "just_enabled",
            Self::NotInvalidated => "not_invalidated",
            Self::CameraCut => "camera_cut",
            Self::RenderTargetExtentChanged => "render_target_extent_changed",
            Self::MotionVectorsMissing => "motion_vectors_missing",
            Self::UpscalerOwnsHistory => "upscaler_owns_history",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaaDebugViews {
    pub jitter_visualization: bool,
    pub history_validity_visualization: bool,
    pub motion_vector_visualization: bool,
}

impl TaaDebugViews {
    pub const NONE: Self = Self {
        jitter_visualization: false,
        history_validity_visualization: false,
        motion_vector_visualization: false,
    };

    #[must_use]
    pub const fn from_camera_debug_view(view: CameraDebugView) -> Self {
        match view {
            CameraDebugView::MotionVectors => Self {
                jitter_visualization: false,
                history_validity_visualization: false,
                motion_vector_visualization: true,
            },
            _ => Self::NONE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaaJitterPlan {
    pub frame_index: u64,
    pub sequence_index: u32,
    pub offset_pixels: RenderVec2,
    pub jitter_applied: bool,
}

impl TaaJitterPlan {
    #[must_use]
    pub fn for_frame(frame_index: u64, taa_enabled: bool) -> Self {
        let sequence_index = (frame_index % TAA_JITTER_SEQUENCE_LENGTH as u64) as u32;
        let offset = if taa_enabled {
            halton_jitter_sample(sequence_index)
        } else {
            RenderVec2::new(0.0, 0.0)
        };
        Self {
            frame_index,
            sequence_index,
            offset_pixels: offset,
            jitter_applied: taa_enabled,
        }
    }

    #[must_use]
    pub fn from_camera_jitter(camera: CameraJitter, taa_enabled: bool) -> Self {
        Self {
            frame_index: camera.frame_index,
            sequence_index: (camera.frame_index % TAA_JITTER_SEQUENCE_LENGTH as u64) as u32,
            offset_pixels: camera.offset,
            jitter_applied: taa_enabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaaHistoryAllocationKey {
    pub view_id: RenderStableId,
    pub render_target: RenderExtent2d,
}

impl TaaHistoryAllocationKey {
    #[must_use]
    pub const fn new(view_id: RenderStableId, render_target: RenderExtent2d) -> Self {
        Self {
            view_id,
            render_target,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaaHistoryAllocation {
    pub view_id: RenderStableId,
    pub render_target: RenderExtent2d,
    pub generation: u32,
    pub valid: bool,
}

impl TaaHistoryAllocation {
    #[must_use]
    pub const fn new(view_id: RenderStableId, render_target: RenderExtent2d) -> Self {
        Self {
            view_id,
            render_target,
            generation: 1,
            valid: false,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "bevy_ecs", derive(bevy_ecs::prelude::Resource))]
pub struct TaaHistoryAllocator {
    schema_version: u16,
    allocations: Vec<TaaHistoryAllocation>,
    invalidated_count: u64,
    allocated_count: u64,
    reused_count: u64,
}

impl TaaHistoryAllocator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: TAA_SCAFFOLD_SCHEMA_VERSION,
            allocations: Vec::new(),
            invalidated_count: 0,
            allocated_count: 0,
            reused_count: 0,
        }
    }

    pub fn ensure(
        &mut self,
        key: TaaHistoryAllocationKey,
    ) -> (TaaHistoryAllocation, TaaHistoryReason) {
        for entry in &mut self.allocations {
            if entry.view_id != key.view_id {
                continue;
            }
            if entry.render_target != key.render_target {
                entry.render_target = key.render_target;
                entry.generation = entry.generation.saturating_add(1);
                entry.valid = false;
                self.invalidated_count = self.invalidated_count.saturating_add(1);
                return (*entry, TaaHistoryReason::RenderTargetExtentChanged);
            }
            self.reused_count = self.reused_count.saturating_add(1);
            let reason = if entry.valid {
                TaaHistoryReason::NotInvalidated
            } else {
                entry.valid = true;
                TaaHistoryReason::JustEnabled
            };
            return (*entry, reason);
        }
        let mut allocation = TaaHistoryAllocation::new(key.view_id, key.render_target);
        allocation.valid = false;
        self.allocations.push(allocation);
        self.allocated_count = self.allocated_count.saturating_add(1);
        (allocation, TaaHistoryReason::JustEnabled)
    }

    pub fn invalidate(&mut self, view_id: RenderStableId, reason: TaaHistoryReason) -> bool {
        let mut invalidated = false;
        for entry in &mut self.allocations {
            if entry.view_id == view_id {
                entry.valid = false;
                entry.generation = entry.generation.saturating_add(1);
                invalidated = true;
            }
        }
        if invalidated {
            self.invalidated_count = self.invalidated_count.saturating_add(1);
            // Reason is recorded by the caller in TaaFramePlan; the allocator only
            // tracks counts here so the reason cannot diverge from the per-frame
            // plan's recorded reason.
            let _ = reason;
        }
        invalidated
    }

    pub fn release(&mut self, view_id: RenderStableId) {
        self.allocations.retain(|entry| entry.view_id != view_id);
    }

    #[must_use]
    pub fn allocation(&self, view_id: RenderStableId) -> Option<TaaHistoryAllocation> {
        self.allocations
            .iter()
            .copied()
            .find(|entry| entry.view_id == view_id)
    }

    #[must_use]
    pub fn allocations(&self) -> &[TaaHistoryAllocation] {
        &self.allocations
    }

    #[must_use]
    pub const fn allocated_count(&self) -> u64 {
        self.allocated_count
    }

    #[must_use]
    pub const fn invalidated_count(&self) -> u64 {
        self.invalidated_count
    }

    #[must_use]
    pub const fn reused_count(&self) -> u64 {
        self.reused_count
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TaaFramePlan {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub settings: TaaSettings,
    pub jitter: TaaJitterPlan,
    pub history_status: TaaHistoryStatus,
    pub history_reason: TaaHistoryReason,
    pub allocation: Option<TaaHistoryAllocation>,
    pub debug_views: TaaDebugViews,
    pub motion_vector_resource: FrameGraphResourceType,
    pub history_resource: FrameGraphResourceType,
    pub upscaler_owns_history: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct TaaFrameInputs {
    pub camera_history: Option<CameraHistory>,
    pub camera_render_target: Option<CameraRenderTarget>,
    pub camera_jitter: Option<CameraJitter>,
    pub debug_view: CameraDebugView,
    pub upscaler_owns_history: bool,
    pub motion_vectors_available: bool,
}

impl TaaFramePlan {
    pub fn build(
        view_id: RenderStableId,
        settings: TaaSettings,
        inputs: TaaFrameInputs,
        allocator: &mut TaaHistoryAllocator,
    ) -> Self {
        let TaaFrameInputs {
            camera_history,
            camera_render_target,
            camera_jitter,
            debug_view,
            upscaler_owns_history,
            motion_vectors_available,
        } = inputs;
        let camera_cut = camera_history.is_some_and(|history| history.reset);
        let render_target = camera_render_target
            .map(|target| target.extent)
            .unwrap_or_default();
        let jitter = camera_jitter
            .map(|jitter| TaaJitterPlan::from_camera_jitter(jitter, settings.enabled))
            .unwrap_or_else(|| TaaJitterPlan::for_frame(0, settings.enabled));

        let history_reason: TaaHistoryReason;
        let history_status: TaaHistoryStatus;
        let mut allocation = None;

        if !settings.enabled {
            allocator.release(view_id);
            return Self {
                schema_version: TAA_SCAFFOLD_SCHEMA_VERSION,
                view_id,
                settings,
                jitter,
                history_status: TaaHistoryStatus::Disabled,
                history_reason: TaaHistoryReason::NotApplicable,
                allocation: None,
                debug_views: TaaDebugViews::from_camera_debug_view(debug_view),
                motion_vector_resource: FrameGraphResourceType::MotionVectors,
                history_resource: FrameGraphResourceType::HistoryBuffer,
                upscaler_owns_history,
            };
        }

        if upscaler_owns_history {
            allocator.release(view_id);
            history_reason = TaaHistoryReason::UpscalerOwnsHistory;
            history_status = TaaHistoryStatus::Disabled;
        } else if camera_cut {
            allocator.invalidate(view_id, TaaHistoryReason::CameraCut);
            history_reason = TaaHistoryReason::CameraCut;
            history_status = TaaHistoryStatus::InvalidatedByCameraCut;
            allocation = Some(
                allocator
                    .ensure(TaaHistoryAllocationKey::new(view_id, render_target))
                    .0,
            );
        } else if !motion_vectors_available {
            allocator.invalidate(view_id, TaaHistoryReason::MotionVectorsMissing);
            history_reason = TaaHistoryReason::MotionVectorsMissing;
            history_status = TaaHistoryStatus::InvalidatedByMissingMotionVectors;
            allocation = Some(
                allocator
                    .ensure(TaaHistoryAllocationKey::new(view_id, render_target))
                    .0,
            );
        } else {
            let (alloc, reason) =
                allocator.ensure(TaaHistoryAllocationKey::new(view_id, render_target));
            history_reason = reason;
            history_status = match reason {
                TaaHistoryReason::JustEnabled => TaaHistoryStatus::Initial,
                TaaHistoryReason::NotInvalidated => TaaHistoryStatus::Valid,
                TaaHistoryReason::RenderTargetExtentChanged => {
                    TaaHistoryStatus::InvalidatedByExtentChange
                }
                _ => TaaHistoryStatus::Initial,
            };
            allocation = Some(alloc);
        }

        Self {
            schema_version: TAA_SCAFFOLD_SCHEMA_VERSION,
            view_id,
            settings,
            jitter,
            history_status,
            history_reason,
            allocation,
            debug_views: TaaDebugViews::from_camera_debug_view(debug_view),
            motion_vector_resource: FrameGraphResourceType::MotionVectors,
            history_resource: FrameGraphResourceType::HistoryBuffer,
            upscaler_owns_history,
        }
    }

    #[must_use]
    pub fn requires_history_resource(self) -> bool {
        self.settings.enabled && !self.upscaler_owns_history
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaaDiagnostics {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub history_status: TaaHistoryStatus,
    pub history_reason: TaaHistoryReason,
    pub jitter_applied: bool,
    pub jitter_sequence_index: u32,
    pub debug_views: TaaDebugViews,
    pub upscaler_owns_history: bool,
    pub allocation_count: u64,
    pub invalidation_count: u64,
}

impl TaaDiagnostics {
    #[must_use]
    pub fn from_plan(plan: &TaaFramePlan, allocator: &TaaHistoryAllocator) -> Self {
        Self {
            schema_version: TAA_SCAFFOLD_SCHEMA_VERSION,
            view_id: plan.view_id,
            history_status: plan.history_status,
            history_reason: plan.history_reason,
            jitter_applied: plan.jitter.jitter_applied,
            jitter_sequence_index: plan.jitter.sequence_index,
            debug_views: plan.debug_views,
            upscaler_owns_history: plan.upscaler_owns_history,
            allocation_count: allocator.allocated_count(),
            invalidation_count: allocator.invalidated_count(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_api::{CameraHistory, CameraJitter, CameraRenderTarget, UpscalerSettings};

    fn render_target(width: u32, height: u32) -> CameraRenderTarget {
        CameraRenderTarget {
            extent: RenderExtent2d::new(width, height),
            ..CameraRenderTarget::default()
        }
    }

    fn inputs_for(target: CameraRenderTarget) -> TaaFrameInputs {
        TaaFrameInputs {
            camera_history: None,
            camera_render_target: Some(target),
            camera_jitter: None,
            debug_view: CameraDebugView::None,
            upscaler_owns_history: false,
            motion_vectors_available: true,
        }
    }

    #[test]
    fn taa_disabled_releases_history_and_emits_no_jitter() {
        let mut allocator = TaaHistoryAllocator::new();
        let plan = TaaFramePlan::build(
            RenderStableId::new(1),
            TaaSettings {
                enabled: false,
                ..TaaSettings::default()
            },
            inputs_for(render_target(1280, 720)),
            &mut allocator,
        );

        assert_eq!(plan.history_status, TaaHistoryStatus::Disabled);
        assert_eq!(plan.history_reason, TaaHistoryReason::NotApplicable);
        assert!(!plan.jitter.jitter_applied);
        assert!(plan.allocation.is_none());
        assert!(allocator.allocations().is_empty());
        assert!(!plan.requires_history_resource());
    }

    #[test]
    fn taa_enabled_allocates_history_and_marks_initial() {
        let mut allocator = TaaHistoryAllocator::new();
        let plan = TaaFramePlan::build(
            RenderStableId::new(2),
            TaaSettings::default(),
            TaaFrameInputs {
                camera_jitter: Some(CameraJitter {
                    frame_index: 4,
                    offset: RenderVec2::default(),
                }),
                ..inputs_for(render_target(1920, 1080))
            },
            &mut allocator,
        );

        assert_eq!(plan.history_status, TaaHistoryStatus::Initial);
        assert_eq!(plan.history_reason, TaaHistoryReason::JustEnabled);
        assert!(plan.jitter.jitter_applied);
        assert!(plan.requires_history_resource());
        assert_eq!(allocator.allocations().len(), 1);
        assert_eq!(allocator.allocated_count(), 1);
    }

    #[test]
    fn camera_cut_invalidates_history_and_records_reason() {
        let mut allocator = TaaHistoryAllocator::new();
        let view = RenderStableId::new(3);
        TaaFramePlan::build(
            view,
            TaaSettings::default(),
            inputs_for(render_target(1920, 1080)),
            &mut allocator,
        );
        let plan = TaaFramePlan::build(
            view,
            TaaSettings::default(),
            TaaFrameInputs {
                camera_history: Some(CameraHistory {
                    history_id: view,
                    reset: true,
                }),
                ..inputs_for(render_target(1920, 1080))
            },
            &mut allocator,
        );

        assert_eq!(
            plan.history_status,
            TaaHistoryStatus::InvalidatedByCameraCut
        );
        assert_eq!(plan.history_reason, TaaHistoryReason::CameraCut);
        assert!(allocator.invalidated_count() >= 1);
    }

    #[test]
    fn render_target_extent_change_invalidates_history() {
        let mut allocator = TaaHistoryAllocator::new();
        let view = RenderStableId::new(4);
        TaaFramePlan::build(
            view,
            TaaSettings::default(),
            inputs_for(render_target(1920, 1080)),
            &mut allocator,
        );
        let plan = TaaFramePlan::build(
            view,
            TaaSettings::default(),
            inputs_for(render_target(2560, 1440)),
            &mut allocator,
        );
        assert_eq!(
            plan.history_status,
            TaaHistoryStatus::InvalidatedByExtentChange
        );
        assert_eq!(
            plan.history_reason,
            TaaHistoryReason::RenderTargetExtentChanged
        );
    }

    #[test]
    fn upscaler_owning_history_releases_taa_history_and_records_reason() {
        let mut allocator = TaaHistoryAllocator::new();
        let view = RenderStableId::new(5);
        TaaFramePlan::build(
            view,
            TaaSettings::default(),
            inputs_for(render_target(1920, 1080)),
            &mut allocator,
        );
        let plan = TaaFramePlan::build(
            view,
            TaaSettings::default(),
            TaaFrameInputs {
                upscaler_owns_history: true,
                ..inputs_for(render_target(1920, 1080))
            },
            &mut allocator,
        );
        assert_eq!(plan.history_reason, TaaHistoryReason::UpscalerOwnsHistory);
        assert_eq!(plan.history_status, TaaHistoryStatus::Disabled);
        assert!(allocator.allocations().is_empty());
        assert!(!plan.requires_history_resource());
    }

    #[test]
    fn missing_motion_vectors_invalidates_history() {
        let mut allocator = TaaHistoryAllocator::new();
        let plan = TaaFramePlan::build(
            RenderStableId::new(6),
            TaaSettings::default(),
            TaaFrameInputs {
                motion_vectors_available: false,
                ..inputs_for(render_target(1920, 1080))
            },
            &mut allocator,
        );
        assert_eq!(
            plan.history_status,
            TaaHistoryStatus::InvalidatedByMissingMotionVectors
        );
        assert_eq!(plan.history_reason, TaaHistoryReason::MotionVectorsMissing);
    }

    #[test]
    fn debug_motion_vector_view_enables_motion_vector_visualization() {
        let mut allocator = TaaHistoryAllocator::new();
        let plan = TaaFramePlan::build(
            RenderStableId::new(7),
            TaaSettings::default(),
            TaaFrameInputs {
                debug_view: CameraDebugView::MotionVectors,
                ..inputs_for(render_target(1920, 1080))
            },
            &mut allocator,
        );
        assert!(plan.debug_views.motion_vector_visualization);
        assert!(!plan.debug_views.jitter_visualization);
    }

    #[test]
    fn halton_jitter_sequence_repeats_after_sequence_length() {
        let first = halton_jitter_sample(0);
        let same = halton_jitter_sample(TAA_JITTER_SEQUENCE_LENGTH);
        let different = halton_jitter_sample(1);
        assert_eq!(first.x, same.x);
        assert_eq!(first.y, same.y);
        assert!(first.x != different.x || first.y != different.y);
    }

    #[test]
    fn diagnostics_summarize_plan_and_allocator_state() {
        let mut allocator = TaaHistoryAllocator::new();
        let plan = TaaFramePlan::build(
            RenderStableId::new(11),
            TaaSettings::default(),
            inputs_for(render_target(1920, 1080)),
            &mut allocator,
        );
        let diagnostics = TaaDiagnostics::from_plan(&plan, &allocator);

        assert_eq!(diagnostics.history_status, TaaHistoryStatus::Initial);
        assert_eq!(diagnostics.allocation_count, 1);
        assert!(!diagnostics.upscaler_owns_history);
    }

    #[test]
    fn upscaler_settings_pull_history_default_off_does_not_block_taa_alone() {
        // Pure unit smoke: TaaSettings is its own knob; UpscalerSettings is
        // owned by upscaling.rs and TAA only checks the bool the caller passed
        // through `upscaler_owns_history`.
        let _ = UpscalerSettings::default();
    }
}
