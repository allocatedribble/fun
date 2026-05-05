use std::collections::BTreeMap;

use bevy::render::{
    render_resource::{Buffer, BufferAddress, BufferSize, COPY_BUFFER_ALIGNMENT, CommandEncoder},
    renderer::RenderDevice,
};
use wgpu::util::StagingBelt;

use crate::{
    FunUploadBudgetClass, FunUploadBudgetDecision, FunUploadBudgetTracker, FunUploadFrameReport,
    FunUploadFrameReportBuilder, FunUploadSubsystem, FunUploadWriteIntent,
};

const FUN_UPLOAD_ARENA_CHUNK_BYTES: BufferAddress = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UploadWriteLabel(pub &'static str);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadArenaError {
    UnalignedOffset {
        label: UploadWriteLabel,
        offset: BufferAddress,
    },
    UnalignedSize {
        label: UploadWriteLabel,
        bytes: u64,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunUploadArenaLabelStats {
    pub calls: u64,
    pub bytes: u64,
    pub raw_write_fallbacks: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunUploadArenaStats {
    pub frame_index: u64,
    pub write_buffer_calls: u64,
    pub write_buffer_bytes: u64,
    pub raw_write_fallbacks: u64,
    pub label_stats: BTreeMap<UploadWriteLabel, FunUploadArenaLabelStats>,
}

pub struct FunUploadArena {
    frame_index: u64,
    small_write_belt: StagingBelt,
    stats: FunUploadArenaStats,
}

pub struct FunUploadArenaWriteRequest<'a> {
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub budget_class: FunUploadBudgetClass,
    pub encoder: &'a mut CommandEncoder,
    pub target: &'a Buffer,
    pub offset: BufferAddress,
    pub data: &'a [u8],
    pub buffer_size: u64,
    pub dirty_bytes: u64,
    pub buffer_created_or_resized: bool,
}

impl FunUploadArena {
    pub fn new(render_device: &RenderDevice) -> Self {
        Self {
            frame_index: 0,
            small_write_belt: StagingBelt::new(
                render_device.wgpu_device().clone(),
                FUN_UPLOAD_ARENA_CHUNK_BYTES,
            ),
            stats: FunUploadArenaStats::default(),
        }
    }

    pub fn write_buffer_tracked(
        &mut self,
        label: UploadWriteLabel,
        encoder: &mut CommandEncoder,
        target: &Buffer,
        offset: BufferAddress,
        data: &[u8],
    ) -> Result<(), UploadArenaError> {
        let Some(size) = self.validate_write_request_or_record(label, offset, data.len() as u64)?
        else {
            return Ok(());
        };
        {
            let mut view = self
                .small_write_belt
                .write_buffer(encoder, target, offset, size);
            view.copy_from_slice(data);
        }
        self.record_write(label, data.len() as u64);
        Ok(())
    }

    pub fn write_buffer_budgeted(
        &mut self,
        budget: &mut FunUploadBudgetTracker,
        report: &mut FunUploadFrameReportBuilder,
        request: FunUploadArenaWriteRequest<'_>,
    ) -> Result<FunUploadBudgetDecision, UploadArenaError> {
        let intent = FunUploadWriteIntent {
            label: request.label,
            subsystem: request.subsystem,
            budget_class: request.budget_class,
            bytes: request.data.len() as u64,
            offset: request.offset,
            buffer_size: request.buffer_size,
            dirty_bytes: request.dirty_bytes,
            buffer_created_or_resized: request.buffer_created_or_resized,
        };
        let decision = budget.decide(intent);
        match decision {
            FunUploadBudgetDecision::Admit => {
                self.write_buffer_tracked(
                    request.label,
                    request.encoder,
                    request.target,
                    request.offset,
                    request.data,
                )?;
                report.record_intent(intent, decision);
            }
            FunUploadBudgetDecision::FallbackRawWrite => {
                self.record_raw_write_fallback(request.label);
                report.record_intent(intent, decision);
            }
            FunUploadBudgetDecision::Defer
            | FunUploadBudgetDecision::RejectOversized
            | FunUploadBudgetDecision::RejectUnaligned => {
                report.record_intent(intent, decision);
            }
        }
        Ok(decision)
    }

    pub fn finish(&mut self) {
        self.small_write_belt.finish();
    }

    pub fn recall_completed(&mut self) {
        self.small_write_belt.recall();
        self.frame_index = self.frame_index.saturating_add(1);
        self.stats = FunUploadArenaStats {
            frame_index: self.frame_index,
            ..Default::default()
        };
    }

    pub fn stats(&self) -> &FunUploadArenaStats {
        &self.stats
    }

    pub fn frame_report(&self, top_label_limit: usize) -> FunUploadFrameReport {
        FunUploadFrameReport::from_arena_stats(&self.stats, top_label_limit)
    }

    fn record_write(&mut self, label: UploadWriteLabel, bytes: u64) {
        self.stats.write_buffer_calls = self.stats.write_buffer_calls.saturating_add(1);
        self.stats.write_buffer_bytes = self.stats.write_buffer_bytes.saturating_add(bytes);
        let label_stats = self.stats.label_stats.entry(label).or_default();
        label_stats.calls = label_stats.calls.saturating_add(1);
        label_stats.bytes = label_stats.bytes.saturating_add(bytes);
    }

    fn record_raw_write_fallback(&mut self, label: UploadWriteLabel) {
        self.stats.raw_write_fallbacks = self.stats.raw_write_fallbacks.saturating_add(1);
        let label_stats = self.stats.label_stats.entry(label).or_default();
        label_stats.raw_write_fallbacks = label_stats.raw_write_fallbacks.saturating_add(1);
    }

    fn validate_write_request_or_record(
        &mut self,
        label: UploadWriteLabel,
        offset: BufferAddress,
        bytes: u64,
    ) -> Result<Option<BufferSize>, UploadArenaError> {
        match validate_write_request(label, offset, bytes) {
            Ok(size) => Ok(size),
            Err(error) => {
                self.record_raw_write_fallback(label);
                Err(error)
            }
        }
    }
}

fn validate_write_request(
    label: UploadWriteLabel,
    offset: BufferAddress,
    bytes: u64,
) -> Result<Option<BufferSize>, UploadArenaError> {
    if bytes == 0 {
        return Ok(None);
    }
    if !offset.is_multiple_of(COPY_BUFFER_ALIGNMENT) {
        return Err(UploadArenaError::UnalignedOffset { label, offset });
    }
    if !bytes.is_multiple_of(COPY_BUFFER_ALIGNMENT) {
        return Err(UploadArenaError::UnalignedSize { label, bytes });
    }
    Ok(Some(
        BufferSize::new(bytes).expect("nonzero bytes produce BufferSize"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_request_rejects_unaligned_offset() {
        let label = UploadWriteLabel("test.unaligned_offset");
        assert_eq!(
            validate_write_request(label, 2, COPY_BUFFER_ALIGNMENT),
            Err(UploadArenaError::UnalignedOffset { label, offset: 2 })
        );
    }

    #[test]
    fn write_request_rejects_unaligned_size() {
        let label = UploadWriteLabel("test.unaligned_size");
        assert_eq!(
            validate_write_request(label, 0, 3),
            Err(UploadArenaError::UnalignedSize { label, bytes: 3 })
        );
    }

    #[test]
    fn write_request_accepts_aligned_size() {
        let label = UploadWriteLabel("test.aligned");
        let result = validate_write_request(label, 0, COPY_BUFFER_ALIGNMENT)
            .expect("aligned write should validate")
            .expect("nonempty write should have a size");
        assert_eq!(result.get(), COPY_BUFFER_ALIGNMENT);
    }

    #[test]
    fn write_request_allows_empty_noop() {
        assert_eq!(
            validate_write_request(UploadWriteLabel("test.empty"), 0, 0),
            Ok(None)
        );
    }
}
