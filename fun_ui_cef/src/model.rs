use std::collections::VecDeque;

pub const DEFAULT_UI_PATCH_SCHEMA_REVISION: u32 = 1;
pub const DEFAULT_MAX_PATCHES_PER_BATCH: usize = 256;
pub const DEFAULT_MAX_PENDING_PATCH_BATCHES: usize = 8;
pub const MAX_UI_PATCH_TEXT_BYTES: usize = 2 * 1024;
pub const MAX_UI_PATCH_BINARY_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum GameUiChannel {
    Hud,
    Match,
    Player,
    Loadout,
    Scoreboard,
    Chat,
    Diagnostics,
    Settings,
}

impl GameUiChannel {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Hud => "hud",
            Self::Match => "match",
            Self::Player => "player",
            Self::Loadout => "loadout",
            Self::Scoreboard => "scoreboard",
            Self::Chat => "chat",
            Self::Diagnostics => "diagnostics",
            Self::Settings => "settings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum GameUiFieldKey {
    Health,
    Armor,
    AmmoInMagazine,
    AmmoReserve,
    MatchTimerMs,
    ObjectiveLabel,
    PlayerName,
    LoadoutItem,
    ScoreboardRow,
    ChatRow,
    LoadingState,
    DiagnosticsSummary,
    SettingValue,
    BrowserLoaded,
    CompositorVisible,
    StreamExpectedChunks,
    StreamReceivedChunks,
    Route,
    ModalReason,
    FocusMode,
}

impl GameUiFieldKey {
    #[must_use]
    pub const fn id(self) -> u16 {
        match self {
            Self::Health => 1,
            Self::Armor => 2,
            Self::AmmoInMagazine => 3,
            Self::AmmoReserve => 4,
            Self::MatchTimerMs => 5,
            Self::ObjectiveLabel => 6,
            Self::PlayerName => 7,
            Self::LoadoutItem => 8,
            Self::ScoreboardRow => 9,
            Self::ChatRow => 10,
            Self::LoadingState => 11,
            Self::DiagnosticsSummary => 12,
            Self::SettingValue => 13,
            Self::BrowserLoaded => 14,
            Self::CompositorVisible => 15,
            Self::StreamExpectedChunks => 16,
            Self::StreamReceivedChunks => 17,
            Self::Route => 18,
            Self::ModalReason => 19,
            Self::FocusMode => 20,
        }
    }
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct UiPatchSequence(pub u64);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct UiRowId(pub u64);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct UiRowRevision(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiPatchValue {
    Bool { value: bool },
    U16 { value: u16 },
    U32 { value: u32 },
    I32 { value: i32 },
    Text { value: String },
    Binary { bytes: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiPatchOp {
    SetField {
        value: UiPatchValue,
    },
    RowInserted {
        row_id: UiRowId,
        row_revision: UiRowRevision,
        value: UiPatchValue,
    },
    RowRemoved {
        row_id: UiRowId,
    },
    RowChanged {
        row_id: UiRowId,
        row_revision: UiRowRevision,
        value: UiPatchValue,
    },
    RowReordered {
        row_id: UiRowId,
        after: Option<UiRowId>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct UiPatchRecord {
    pub channel: GameUiChannel,
    pub field: GameUiFieldKey,
    pub field_id: u16,
    pub sequence: UiPatchSequence,
    pub op: UiPatchOp,
}

impl UiPatchRecord {
    #[must_use]
    pub const fn identity(&self) -> UiPatchIdentity {
        UiPatchIdentity {
            channel: self.channel,
            field: self.field,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UiPatchIdentity {
    pub channel: GameUiChannel,
    pub field: GameUiFieldKey,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct UiPatchBatch {
    pub schema_revision: u32,
    pub sequence: UiPatchSequence,
    pub patches: Vec<UiPatchRecord>,
}

impl UiPatchBatch {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }
}

pub trait CefUiModel {
    const CHANNEL: GameUiChannel;
    const SCHEMA_REVISION: u32;

    fn write_initial_snapshot(&self, out: &mut UiPatchWriter);
    fn write_patch(&self, previous: &Self, out: &mut UiPatchWriter);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiPatchWriteError {
    TooManyPatches,
    TextTooLarge,
    BinaryTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiPatchWriter {
    schema_revision: u32,
    sequence: UiPatchSequence,
    patches: Vec<UiPatchRecord>,
    scratch: Vec<u8>,
    max_patches: usize,
    dropped_patch_count: u64,
}

impl UiPatchWriter {
    #[must_use]
    pub fn new(schema_revision: u32) -> Self {
        Self {
            schema_revision,
            sequence: UiPatchSequence(0),
            patches: Vec::with_capacity(DEFAULT_MAX_PATCHES_PER_BATCH),
            scratch: Vec::with_capacity(MAX_UI_PATCH_BINARY_BYTES),
            max_patches: DEFAULT_MAX_PATCHES_PER_BATCH,
            dropped_patch_count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.patches.clear();
        self.scratch.clear();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.patches.len()
    }

    #[must_use]
    pub const fn dropped_patch_count(&self) -> u64 {
        self.dropped_patch_count
    }

    pub fn set_bool(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        value: bool,
    ) -> Result<(), UiPatchWriteError> {
        self.push_set(channel, field, UiPatchValue::Bool { value })
    }

    pub fn set_u16(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        value: u16,
    ) -> Result<(), UiPatchWriteError> {
        self.push_set(channel, field, UiPatchValue::U16 { value })
    }

    pub fn set_u32(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        value: u32,
    ) -> Result<(), UiPatchWriteError> {
        self.push_set(channel, field, UiPatchValue::U32 { value })
    }

    pub fn set_i32(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        value: i32,
    ) -> Result<(), UiPatchWriteError> {
        self.push_set(channel, field, UiPatchValue::I32 { value })
    }

    pub fn set_text(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        value: &str,
    ) -> Result<(), UiPatchWriteError> {
        if value.len() > MAX_UI_PATCH_TEXT_BYTES {
            self.dropped_patch_count = self.dropped_patch_count.saturating_add(1);
            return Err(UiPatchWriteError::TextTooLarge);
        }
        self.push_set(
            channel,
            field,
            UiPatchValue::Text {
                value: value.to_owned(),
            },
        )
    }

    pub fn set_binary(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        bytes: &[u8],
    ) -> Result<(), UiPatchWriteError> {
        if bytes.len() > MAX_UI_PATCH_BINARY_BYTES {
            self.dropped_patch_count = self.dropped_patch_count.saturating_add(1);
            return Err(UiPatchWriteError::BinaryTooLarge);
        }
        self.scratch.clear();
        self.scratch.extend_from_slice(bytes);
        self.push_set(
            channel,
            field,
            UiPatchValue::Binary {
                bytes: self.scratch.clone(),
            },
        )
    }

    pub fn push_list_op(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        op: UiPatchOp,
    ) -> Result<(), UiPatchWriteError> {
        self.push_record(channel, field, op)
    }

    #[must_use]
    pub fn finish_batch(&mut self) -> Option<UiPatchBatch> {
        if self.patches.is_empty() {
            return None;
        }
        let batch = UiPatchBatch {
            schema_revision: self.schema_revision,
            sequence: self.sequence,
            patches: self.patches.clone(),
        };
        self.clear();
        Some(batch)
    }

    fn push_set(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        value: UiPatchValue,
    ) -> Result<(), UiPatchWriteError> {
        self.push_record(channel, field, UiPatchOp::SetField { value })
    }

    fn push_record(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        op: UiPatchOp,
    ) -> Result<(), UiPatchWriteError> {
        if self.patches.len() >= self.max_patches {
            self.dropped_patch_count = self.dropped_patch_count.saturating_add(1);
            return Err(UiPatchWriteError::TooManyPatches);
        }
        self.sequence = UiPatchSequence(self.sequence.0.saturating_add(1));
        self.patches.push(UiPatchRecord {
            channel,
            field,
            field_id: field.id(),
            sequence: self.sequence,
            op,
        });
        Ok(())
    }
}

impl Default for UiPatchWriter {
    fn default() -> Self {
        Self::new(DEFAULT_UI_PATCH_SCHEMA_REVISION)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UiPatchBackpressureQueue {
    batches: VecDeque<UiPatchBatch>,
    max_batches: usize,
    dropped_batch_count: u64,
    coalesced_patch_count: u64,
}

impl UiPatchBackpressureQueue {
    #[must_use]
    pub fn new(max_batches: usize) -> Self {
        Self {
            batches: VecDeque::with_capacity(max_batches),
            max_batches,
            dropped_batch_count: 0,
            coalesced_patch_count: 0,
        }
    }

    pub fn push_batch(&mut self, batch: UiPatchBatch) {
        if batch.is_empty() {
            return;
        }
        self.coalesce_older_patches(&batch);
        if self.batches.len() >= self.max_batches {
            self.dropped_batch_count = self.dropped_batch_count.saturating_add(1);
            self.batches.pop_front();
        }
        self.batches.push_back(batch);
    }

    pub fn pop_batch(&mut self) -> Option<UiPatchBatch> {
        self.batches.pop_front()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.batches.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    #[must_use]
    pub const fn dropped_batch_count(&self) -> u64 {
        self.dropped_batch_count
    }

    #[must_use]
    pub const fn coalesced_patch_count(&self) -> u64 {
        self.coalesced_patch_count
    }

    fn coalesce_older_patches(&mut self, incoming: &UiPatchBatch) {
        for incoming_patch in &incoming.patches {
            let identity = incoming_patch.identity();
            for batch in &mut self.batches {
                let before = batch.patches.len();
                batch.patches.retain(|patch| patch.identity() != identity);
                let removed = before.saturating_sub(batch.patches.len());
                self.coalesced_patch_count =
                    self.coalesced_patch_count.saturating_add(removed as u64);
            }
        }
        self.batches.retain(|batch| !batch.patches.is_empty());
    }
}

impl Default for UiPatchBackpressureQueue {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_PENDING_PATCH_BATCHES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_emits_stable_field_ids_without_dynamic_channels() {
        let mut writer = UiPatchWriter::default();
        writer
            .set_u16(GameUiChannel::Hud, GameUiFieldKey::Health, 75)
            .expect("small patch");
        let batch = writer.finish_batch().expect("patch batch");

        assert_eq!(batch.patches[0].channel.as_wire_str(), "hud");
        assert_eq!(batch.patches[0].field_id, GameUiFieldKey::Health.id());
    }

    #[test]
    fn writer_rejects_oversized_text() {
        let mut writer = UiPatchWriter::default();
        let value = "x".repeat(MAX_UI_PATCH_TEXT_BYTES + 1);

        assert_eq!(
            writer.set_text(GameUiChannel::Chat, GameUiFieldKey::ChatRow, &value),
            Err(UiPatchWriteError::TextTooLarge)
        );
        assert_eq!(writer.dropped_patch_count(), 1);
    }

    #[test]
    fn queue_coalesces_older_patch_for_same_channel_field() {
        let mut writer = UiPatchWriter::default();
        writer
            .set_u16(GameUiChannel::Hud, GameUiFieldKey::Health, 90)
            .expect("first health");
        let first = writer.finish_batch().expect("first batch");
        writer
            .set_u16(GameUiChannel::Hud, GameUiFieldKey::Health, 80)
            .expect("second health");
        let second = writer.finish_batch().expect("second batch");
        let mut queue = UiPatchBackpressureQueue::default();

        queue.push_batch(first);
        queue.push_batch(second);

        assert_eq!(queue.coalesced_patch_count(), 1);
        assert_eq!(queue.len(), 1);
        let batch = queue.pop_batch().expect("coalesced batch");
        assert_eq!(batch.patches.len(), 1);
        assert_eq!(batch.patches[0].sequence, UiPatchSequence(2));
    }

    #[test]
    fn queue_drops_oldest_when_full() {
        let mut queue = UiPatchBackpressureQueue::new(1);
        for (field, value) in [
            (GameUiFieldKey::MatchTimerMs, 1_u32),
            (GameUiFieldKey::StreamReceivedChunks, 2),
        ] {
            let mut writer = UiPatchWriter::default();
            writer
                .set_u32(GameUiChannel::Match, field, value)
                .expect("timer patch");
            queue.push_batch(writer.finish_batch().expect("batch"));
        }

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.dropped_batch_count(), 1);
    }
}
