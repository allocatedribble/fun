use crate::component_api::{NativeUiFrameToken, RenderStableId};

use super::native_ui::{
    RendererNativeUiAlphaMode, RendererNativeUiDirtyRect, RendererNativeUiExtent,
    RendererNativeUiFailClosedReason, RendererNativeUiFrameId, RendererNativeUiImportSyncStatus,
    RendererNativeUiImportedFrame, RendererNativeUiTransportMode,
};

pub const NATIVE_UI_PRODUCER_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeUiProducerSubmissionState {
    Submitted,
    Replaced,
    Aged,
    Consumed,
    Rejected,
}

impl NativeUiProducerSubmissionState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Replaced => "replaced",
            Self::Aged => "aged",
            Self::Consumed => "consumed",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeUiProducerRejection {
    UnknownProducer,
    StaleSubmission,
    InvalidFrameId,
    InvalidExtent,
    NonGpuTransportRejected,
}

impl NativeUiProducerRejection {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnknownProducer => "unknown_producer",
            Self::StaleSubmission => "stale_submission",
            Self::InvalidFrameId => "invalid_frame_id",
            Self::InvalidExtent => "invalid_extent",
            Self::NonGpuTransportRejected => "non_gpu_transport_rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiProducerSubmission {
    pub producer: RenderStableId,
    pub frame: RendererNativeUiImportedFrame,
}

impl NativeUiProducerSubmission {
    #[must_use]
    pub const fn new(producer: RenderStableId, frame: RendererNativeUiImportedFrame) -> Self {
        Self { producer, frame }
    }

    #[must_use]
    pub const fn token(self, fence_value: u64) -> NativeUiFrameToken {
        NativeUiFrameToken::new(self.producer, self.frame.frame_id.0, fence_value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiProducerEntry {
    pub producer: RenderStableId,
    pub latest_frame: Option<RendererNativeUiImportedFrame>,
    pub latest_token: NativeUiFrameToken,
    pub last_consumed_frame_id: RendererNativeUiFrameId,
    pub submitted_frame_count: u64,
    pub replaced_frame_count: u64,
    pub aged_frame_count: u64,
    pub rejected_count: u64,
}

impl NativeUiProducerEntry {
    #[must_use]
    pub const fn new(producer: RenderStableId) -> Self {
        Self {
            producer,
            latest_frame: None,
            latest_token: NativeUiFrameToken::INVALID,
            last_consumed_frame_id: RendererNativeUiFrameId::INVALID,
            submitted_frame_count: 0,
            replaced_frame_count: 0,
            aged_frame_count: 0,
            rejected_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "bevy_ecs", derive(bevy_ecs::prelude::Resource))]
pub struct NativeUiProducerEvents {
    schema_version: u16,
    next_fence_value: u64,
    entries: Vec<NativeUiProducerEntry>,
    aged_frames: u64,
    replaced_frames: u64,
    rejected_submissions: u64,
    submitted_frames: u64,
    last_submission_state: Option<NativeUiProducerSubmissionState>,
}

impl NativeUiProducerEvents {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: NATIVE_UI_PRODUCER_SCHEMA_VERSION,
            next_fence_value: 1,
            entries: Vec::new(),
            aged_frames: 0,
            replaced_frames: 0,
            rejected_submissions: 0,
            submitted_frames: 0,
            last_submission_state: None,
        }
    }

    pub fn register_producer(&mut self, producer: RenderStableId) -> &mut NativeUiProducerEntry {
        if !self.entries.iter().any(|entry| entry.producer == producer) {
            self.entries.push(NativeUiProducerEntry::new(producer));
        }
        self.entries
            .iter_mut()
            .find(|entry| entry.producer == producer)
            .expect("producer was just inserted")
    }

    pub fn submit(
        &mut self,
        submission: NativeUiProducerSubmission,
    ) -> Result<NativeUiFrameToken, NativeUiProducerRejection> {
        if !submission.producer.is_valid() {
            self.rejected_submissions = self.rejected_submissions.saturating_add(1);
            self.last_submission_state = Some(NativeUiProducerSubmissionState::Rejected);
            return Err(NativeUiProducerRejection::UnknownProducer);
        }
        if !submission.frame.frame_id.is_valid() {
            self.record_rejection_for(
                submission.producer,
                NativeUiProducerRejection::InvalidFrameId,
            );
            return Err(NativeUiProducerRejection::InvalidFrameId);
        }
        if !submission.frame.extent.is_valid() {
            self.record_rejection_for(
                submission.producer,
                NativeUiProducerRejection::InvalidExtent,
            );
            return Err(NativeUiProducerRejection::InvalidExtent);
        }
        if !submission.frame.transport.is_gpu_transport() {
            self.record_rejection_for(
                submission.producer,
                NativeUiProducerRejection::NonGpuTransportRejected,
            );
            return Err(NativeUiProducerRejection::NonGpuTransportRejected);
        }

        let fence_value = self.next_fence_value;
        let token = submission.token(fence_value);
        let (replaced, stale) = {
            let entry = self.register_producer(submission.producer);
            let replaced = matches!(entry.latest_frame, Some(prior) if prior.frame_id < submission.frame.frame_id);
            let stale = matches!(
                entry.latest_frame,
                Some(prior) if prior.frame_id >= submission.frame.frame_id
            );
            if stale {
                entry.rejected_count = entry.rejected_count.saturating_add(1);
            } else {
                if replaced {
                    entry.replaced_frame_count = entry.replaced_frame_count.saturating_add(1);
                }
                entry.latest_frame = Some(submission.frame);
                entry.latest_token = token;
                entry.submitted_frame_count = entry.submitted_frame_count.saturating_add(1);
            }
            (replaced, stale)
        };
        if stale {
            self.rejected_submissions = self.rejected_submissions.saturating_add(1);
            self.last_submission_state = Some(NativeUiProducerSubmissionState::Rejected);
            return Err(NativeUiProducerRejection::StaleSubmission);
        }
        if replaced {
            self.replaced_frames = self.replaced_frames.saturating_add(1);
        }
        self.submitted_frames = self.submitted_frames.saturating_add(1);
        self.next_fence_value = self.next_fence_value.saturating_add(1);
        self.last_submission_state = Some(if replaced {
            NativeUiProducerSubmissionState::Replaced
        } else {
            NativeUiProducerSubmissionState::Submitted
        });
        Ok(token)
    }

    pub fn consume_latest(&mut self, producer: RenderStableId) -> NativeUiProducerConsumeOutcome {
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.producer == producer)
        else {
            return NativeUiProducerConsumeOutcome::ProducerUnknown;
        };
        let Some(frame) = entry.latest_frame else {
            return NativeUiProducerConsumeOutcome::NoFrameAvailable {
                last_consumed_frame_id: entry.last_consumed_frame_id,
                latest_token: entry.latest_token,
            };
        };
        if frame.frame_id <= entry.last_consumed_frame_id {
            return NativeUiProducerConsumeOutcome::ReusePrevious {
                latest_token: entry.latest_token,
                last_consumed_frame_id: entry.last_consumed_frame_id,
            };
        }
        entry.last_consumed_frame_id = frame.frame_id;
        self.last_submission_state = Some(NativeUiProducerSubmissionState::Consumed);
        NativeUiProducerConsumeOutcome::FreshFrame {
            frame,
            token: entry.latest_token,
        }
    }

    pub fn age_unconsumed(&mut self, max_unconsumed_frame_count: u64) -> u64 {
        let mut aged = 0u64;
        for entry in &mut self.entries {
            let Some(frame) = entry.latest_frame else {
                continue;
            };
            let unconsumed_age = frame
                .frame_id
                .0
                .saturating_sub(entry.last_consumed_frame_id.0);
            if unconsumed_age > max_unconsumed_frame_count {
                aged = aged.saturating_add(1);
                entry.aged_frame_count = entry.aged_frame_count.saturating_add(1);
                entry.latest_frame = None;
            }
        }
        if aged > 0 {
            self.aged_frames = self.aged_frames.saturating_add(aged);
            self.last_submission_state = Some(NativeUiProducerSubmissionState::Aged);
        }
        aged
    }

    #[must_use]
    pub fn entry(&self, producer: RenderStableId) -> Option<&NativeUiProducerEntry> {
        self.entries.iter().find(|entry| entry.producer == producer)
    }

    #[must_use]
    pub fn entries(&self) -> &[NativeUiProducerEntry] {
        &self.entries
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub const fn submitted_frames(&self) -> u64 {
        self.submitted_frames
    }

    #[must_use]
    pub const fn replaced_frames(&self) -> u64 {
        self.replaced_frames
    }

    #[must_use]
    pub const fn aged_frames(&self) -> u64 {
        self.aged_frames
    }

    #[must_use]
    pub const fn rejected_submissions(&self) -> u64 {
        self.rejected_submissions
    }

    #[must_use]
    pub const fn last_submission_state(&self) -> Option<NativeUiProducerSubmissionState> {
        self.last_submission_state
    }

    fn record_rejection_for(
        &mut self,
        producer: RenderStableId,
        _reason: NativeUiProducerRejection,
    ) {
        let entry = self.register_producer(producer);
        entry.rejected_count = entry.rejected_count.saturating_add(1);
        self.rejected_submissions = self.rejected_submissions.saturating_add(1);
        self.last_submission_state = Some(NativeUiProducerSubmissionState::Rejected);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeUiProducerConsumeOutcome {
    FreshFrame {
        frame: RendererNativeUiImportedFrame,
        token: NativeUiFrameToken,
    },
    ReusePrevious {
        latest_token: NativeUiFrameToken,
        last_consumed_frame_id: RendererNativeUiFrameId,
    },
    NoFrameAvailable {
        last_consumed_frame_id: RendererNativeUiFrameId,
        latest_token: NativeUiFrameToken,
    },
    ProducerUnknown,
}

impl NativeUiProducerConsumeOutcome {
    #[must_use]
    pub const fn import_sync_status(self) -> RendererNativeUiImportSyncStatus {
        match self {
            Self::FreshFrame { .. } => RendererNativeUiImportSyncStatus::CopiedIntoRendererTexture,
            Self::ReusePrevious { .. } => RendererNativeUiImportSyncStatus::ReusedPreviousFrame,
            Self::NoFrameAvailable { .. } | Self::ProducerUnknown => {
                RendererNativeUiImportSyncStatus::NotImported
            }
        }
    }

    #[must_use]
    pub const fn fail_closed_reason(self) -> Option<RendererNativeUiFailClosedReason> {
        match self {
            Self::ProducerUnknown => {
                Some(RendererNativeUiFailClosedReason::SharedTextureUnavailable)
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn frame(self) -> Option<RendererNativeUiImportedFrame> {
        match self {
            Self::FreshFrame { frame, .. } => Some(frame),
            _ => None,
        }
    }

    #[must_use]
    pub const fn token(self) -> NativeUiFrameToken {
        match self {
            Self::FreshFrame { token, .. }
            | Self::ReusePrevious {
                latest_token: token,
                ..
            }
            | Self::NoFrameAvailable {
                latest_token: token,
                ..
            } => token,
            Self::ProducerUnknown => NativeUiFrameToken::INVALID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSubmissionTimings {
    pub callback_timestamp_ns: u64,
    pub import_begin_timestamp_ns: u64,
    pub import_complete_timestamp_ns: u64,
    pub copied_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSubmissionDirtyRects {
    pub count: u32,
    pub union: Option<RendererNativeUiDirtyRect>,
}

#[must_use]
pub fn build_gpu_submission(
    producer: RenderStableId,
    frame_id: u64,
    extent: RendererNativeUiExtent,
    alpha_mode: RendererNativeUiAlphaMode,
    timings: GpuSubmissionTimings,
    dirty: GpuSubmissionDirtyRects,
) -> NativeUiProducerSubmission {
    let GpuSubmissionTimings {
        callback_timestamp_ns,
        import_begin_timestamp_ns,
        import_complete_timestamp_ns,
        copied_bytes,
    } = timings;
    let GpuSubmissionDirtyRects {
        count: dirty_rect_count,
        union: dirty_rect_union,
    } = dirty;
    NativeUiProducerSubmission::new(
        producer,
        RendererNativeUiImportedFrame {
            frame_id: RendererNativeUiFrameId(frame_id),
            extent,
            transport: RendererNativeUiTransportMode::D3d11On12SharedTexture,
            alpha_mode,
            dirty_rect_count,
            dirty_rect_union,
            callback_timestamp_ns,
            import_begin_timestamp_ns,
            import_complete_timestamp_ns,
            copied_bytes,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submission(producer: u64, frame_id: u64) -> NativeUiProducerSubmission {
        build_gpu_submission(
            RenderStableId::new(producer),
            frame_id,
            RendererNativeUiExtent::new(1280, 720),
            RendererNativeUiAlphaMode::Premultiplied,
            GpuSubmissionTimings {
                callback_timestamp_ns: 1_000,
                import_begin_timestamp_ns: 1_500,
                import_complete_timestamp_ns: 2_000,
                copied_bytes: 1280 * 720 * 4,
            },
            GpuSubmissionDirtyRects {
                count: 1,
                union: Some(RendererNativeUiDirtyRect::new(0, 0, 1280, 720)),
            },
        )
    }

    #[test]
    fn producer_records_latest_frame_token_with_increasing_fence() {
        let mut events = NativeUiProducerEvents::new();

        let first = events.submit(submission(7, 1)).expect("first submit");
        let second = events.submit(submission(7, 2)).expect("second submit");

        assert_eq!(first.frame_index, 1);
        assert_eq!(second.frame_index, 2);
        assert!(second.fence_value > first.fence_value);
        assert_eq!(events.submitted_frames(), 2);
        assert_eq!(events.replaced_frames(), 1);
        assert_eq!(
            events.last_submission_state(),
            Some(NativeUiProducerSubmissionState::Replaced)
        );
    }

    #[test]
    fn consume_latest_returns_fresh_then_reuse_when_no_new_frame() {
        let mut events = NativeUiProducerEvents::new();
        events.submit(submission(9, 5)).expect("submit");

        let fresh = events.consume_latest(RenderStableId::new(9));
        assert!(matches!(
            fresh,
            NativeUiProducerConsumeOutcome::FreshFrame { .. }
        ));
        assert_eq!(
            fresh.import_sync_status(),
            RendererNativeUiImportSyncStatus::CopiedIntoRendererTexture
        );

        let reuse = events.consume_latest(RenderStableId::new(9));
        assert!(matches!(
            reuse,
            NativeUiProducerConsumeOutcome::ReusePrevious { .. }
        ));
        assert_eq!(
            reuse.import_sync_status(),
            RendererNativeUiImportSyncStatus::ReusedPreviousFrame
        );
        assert_eq!(reuse.token().frame_index, 5);
    }

    #[test]
    fn unknown_producer_consume_reports_producer_unknown() {
        let mut events = NativeUiProducerEvents::new();
        let outcome = events.consume_latest(RenderStableId::new(42));
        assert_eq!(outcome, NativeUiProducerConsumeOutcome::ProducerUnknown);
        assert_eq!(
            outcome.import_sync_status(),
            RendererNativeUiImportSyncStatus::NotImported
        );
        assert_eq!(
            outcome.fail_closed_reason(),
            Some(RendererNativeUiFailClosedReason::SharedTextureUnavailable)
        );
    }

    #[test]
    fn cpu_on_paint_submission_is_rejected_at_producer_boundary() {
        let mut events = NativeUiProducerEvents::new();
        let mut sub = submission(1, 1);
        sub.frame.transport = RendererNativeUiTransportMode::CpuOnPaint;
        let err = events.submit(sub).expect_err("cpu transport must reject");
        assert_eq!(err, NativeUiProducerRejection::NonGpuTransportRejected);
        assert_eq!(events.rejected_submissions(), 1);
        assert_eq!(events.submitted_frames(), 0);
    }

    #[test]
    fn out_of_order_submissions_are_rejected_as_stale_without_overwriting_latest() {
        let mut events = NativeUiProducerEvents::new();
        let token_two = events.submit(submission(11, 2)).expect("submit 2");
        let err = events
            .submit(submission(11, 1))
            .expect_err("frame 1 after 2 must be stale");

        assert_eq!(err, NativeUiProducerRejection::StaleSubmission);
        let entry = events.entry(RenderStableId::new(11)).expect("entry");
        assert_eq!(
            entry.latest_frame.unwrap().frame_id,
            RendererNativeUiFrameId(2)
        );
        assert_eq!(entry.latest_token, token_two);
        assert_eq!(entry.rejected_count, 1);
    }

    #[test]
    fn age_unconsumed_drops_frames_older_than_budget_and_reports_count() {
        let mut events = NativeUiProducerEvents::new();
        events.submit(submission(3, 10)).expect("submit");

        let aged = events.age_unconsumed(8);

        assert_eq!(aged, 1);
        assert_eq!(events.aged_frames(), 1);
        let entry = events.entry(RenderStableId::new(3)).expect("entry");
        assert!(entry.latest_frame.is_none());
        assert_eq!(entry.aged_frame_count, 1);
        assert_eq!(
            events.last_submission_state(),
            Some(NativeUiProducerSubmissionState::Aged)
        );
    }
}
