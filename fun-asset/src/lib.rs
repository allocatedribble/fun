#![forbid(unsafe_code)]

use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

pub const DEFAULT_ASSET_QUEUE_LIMIT: usize = 4096;
pub const DEFAULT_ASSET_EVENT_LIMIT: usize = 4096;
pub const DEFAULT_ASSET_MANIFEST_LIMIT: usize = 65_536;

macro_rules! typed_asset_index {
    ($name:ident) => {
        pub struct $name<T> {
            pub slot: u32,
            pub generation: u32,
            _marker: PhantomData<fn() -> T>,
        }

        impl<T> $name<T> {
            pub const INVALID: Self = Self::new(u32::MAX, 0);

            #[must_use]
            pub const fn new(slot: u32, generation: u32) -> Self {
                Self {
                    slot,
                    generation,
                    _marker: PhantomData,
                }
            }

            #[must_use]
            pub const fn first(slot: u32) -> Self {
                Self::new(slot, 1)
            }

            #[must_use]
            pub const fn is_valid(self) -> bool {
                self.slot != u32::MAX && self.generation != 0
            }

            #[must_use]
            pub const fn slot(self) -> u32 {
                self.slot
            }

            #[must_use]
            pub const fn generation(self) -> u32 {
                self.generation
            }
        }

        impl<T> Clone for $name<T> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<T> Copy for $name<T> {}

        impl<T> Default for $name<T> {
            fn default() -> Self {
                Self::INVALID
            }
        }

        impl<T> PartialEq for $name<T> {
            fn eq(&self, other: &Self) -> bool {
                self.slot == other.slot && self.generation == other.generation
            }
        }

        impl<T> Eq for $name<T> {}

        impl<T> PartialOrd for $name<T> {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl<T> Ord for $name<T> {
            fn cmp(&self, other: &Self) -> Ordering {
                self.slot
                    .cmp(&other.slot)
                    .then_with(|| self.generation.cmp(&other.generation))
            }
        }

        impl<T> Hash for $name<T> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.slot.hash(state);
                self.generation.hash(state);
            }
        }

        impl<T> fmt::Debug for $name<T> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($name))
                    .field("slot", &self.slot)
                    .field("generation", &self.generation)
                    .finish()
            }
        }
    };
}

typed_asset_index!(AssetId);
typed_asset_index!(AssetHandle);

impl<T> From<AssetId<T>> for AssetHandle<T> {
    fn from(value: AssetId<T>) -> Self {
        Self::new(value.slot, value.generation)
    }
}

impl<T> From<AssetHandle<T>> for AssetId<T> {
    fn from(value: AssetHandle<T>) -> Self {
        Self::new(value.slot, value.generation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct AssetKey {
    slot: u32,
    generation: u32,
}

impl AssetKey {
    const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }
}

impl<T> From<AssetId<T>> for AssetKey {
    fn from(value: AssetId<T>) -> Self {
        Self::new(value.slot, value.generation)
    }
}

impl<T> From<AssetHandle<T>> for AssetKey {
    fn from(value: AssetHandle<T>) -> Self {
        Self::new(value.slot, value.generation)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRecord<T> {
    pub value: T,
    pub load_state: AssetLoadState,
}

impl<T> AssetRecord<T> {
    #[must_use]
    pub const fn resident(value: T) -> Self {
        Self {
            value,
            load_state: AssetLoadState::Resident,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetStore<T> {
    capacity: usize,
    generations: Vec<u32>,
    free_slots: Vec<u32>,
    records: BTreeMap<AssetKey, AssetRecord<T>>,
}

impl<T> AssetStore<T> {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            generations: Vec::new(),
            free_slots: Vec::new(),
            records: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn insert(&mut self, value: T) -> Result<AssetHandle<T>, AssetStoreError> {
        self.insert_record(AssetRecord::resident(value))
    }

    pub fn insert_record(
        &mut self,
        record: AssetRecord<T>,
    ) -> Result<AssetHandle<T>, AssetStoreError> {
        if self.records.len() >= self.capacity {
            return Err(AssetStoreError::CapacityExceeded {
                capacity: self.capacity,
            });
        }

        let slot = self.allocate_slot()?;
        let generation = self.generations[slot as usize].max(1);
        let handle = AssetHandle::new(slot, generation);
        self.records.insert(AssetKey::from(handle), record);
        Ok(handle)
    }

    #[must_use]
    pub fn get(&self, handle: AssetHandle<T>) -> Option<&T> {
        self.records
            .get(&AssetKey::from(handle))
            .map(|record| &record.value)
    }

    #[must_use]
    pub fn get_record(&self, handle: AssetHandle<T>) -> Option<&AssetRecord<T>> {
        self.records.get(&AssetKey::from(handle))
    }

    pub fn get_mut(&mut self, handle: AssetHandle<T>) -> Option<&mut T> {
        self.records
            .get_mut(&AssetKey::from(handle))
            .map(|record| &mut record.value)
    }

    pub fn set_load_state(
        &mut self,
        handle: AssetHandle<T>,
        load_state: AssetLoadState,
    ) -> Result<(), AssetStoreError> {
        let Some(record) = self.records.get_mut(&AssetKey::from(handle)) else {
            return Err(AssetStoreError::UnknownAsset);
        };
        record.load_state = load_state;
        Ok(())
    }

    pub fn remove(&mut self, handle: AssetHandle<T>) -> Result<AssetRecord<T>, AssetStoreError> {
        if !handle.is_valid() {
            return Err(AssetStoreError::InvalidHandle);
        }

        let Some(record) = self.records.remove(&AssetKey::from(handle)) else {
            return Err(AssetStoreError::UnknownAsset);
        };

        let slot_index = handle.slot as usize;
        let Some(generation) = self.generations.get_mut(slot_index) else {
            return Err(AssetStoreError::UnknownAsset);
        };
        if *generation != handle.generation {
            return Err(AssetStoreError::StaleHandle);
        }
        *generation = next_generation(*generation);
        self.free_slots.push(handle.slot);
        Ok(record)
    }

    fn allocate_slot(&mut self) -> Result<u32, AssetStoreError> {
        if let Some(slot) = self.free_slots.pop() {
            return Ok(slot);
        }
        let slot = u32::try_from(self.generations.len()).map_err(|_| {
            AssetStoreError::CapacityExceeded {
                capacity: usize::MAX,
            }
        })?;
        self.generations.push(1);
        Ok(slot)
    }
}

fn next_generation(generation: u32) -> u32 {
    if generation == u32::MAX {
        1
    } else {
        generation.saturating_add(1).max(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetStoreError {
    CapacityExceeded { capacity: usize },
    InvalidHandle,
    StaleHandle,
    UnknownAsset,
}

impl fmt::Display for AssetStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityExceeded { .. } => f.write_str("asset store capacity exceeded"),
            Self::InvalidHandle => f.write_str("asset handle is invalid"),
            Self::StaleHandle => f.write_str("asset handle generation is stale"),
            Self::UnknownAsset => f.write_str("asset handle is not resident"),
        }
    }
}

impl std::error::Error for AssetStoreError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetRequestId(pub u64);

impl AssetRequestId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetPriority(pub i16);

impl AssetPriority {
    pub const LOW: Self = Self(-100);
    pub const NORMAL: Self = Self(0);
    pub const HIGH: Self = Self(100);
    pub const CRITICAL: Self = Self(1000);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Mesh,
    Material,
    Texture,
    Sampler,
    Shader,
    Scene,
    Audio,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetSource {
    VirtualPath(String),
    FilePath(String),
    Embedded(&'static str),
    Memory { label: String, byte_len: u64 },
    Generated(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetDecodePolicy {
    None,
    Cpu,
    GpuReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetLoadState {
    Queued,
    IoAdmitted,
    Reading,
    ReadComplete,
    Decoding,
    DecodeComplete,
    UploadQueued,
    UploadReady,
    Resident,
    Failed(AssetLoadErrorCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetLoadErrorCode {
    InvalidRequest,
    QueueFull,
    ManifestFull,
    ByteLimitExceeded,
    SourceUnavailable,
    DecodeRejected,
    UploadRejected,
    SchedulerRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLoadError {
    pub code: AssetLoadErrorCode,
    pub redacted_message: String,
}

impl AssetLoadError {
    #[must_use]
    pub fn new(code: AssetLoadErrorCode, redacted_message: impl Into<String>) -> Self {
        Self {
            code,
            redacted_message: redacted_message.into(),
        }
    }
}

impl fmt::Display for AssetLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.redacted_message)
    }
}

impl std::error::Error for AssetLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLoadRequest {
    pub request_id: AssetRequestId,
    pub source: AssetSource,
    pub kind: AssetKind,
    pub priority: AssetPriority,
    pub byte_limit: u64,
    pub decode_policy: AssetDecodePolicy,
}

impl AssetLoadRequest {
    #[must_use]
    pub const fn new(
        request_id: AssetRequestId,
        source: AssetSource,
        kind: AssetKind,
        byte_limit: u64,
    ) -> Self {
        Self {
            request_id,
            source,
            kind,
            priority: AssetPriority::NORMAL,
            byte_limit,
            decode_policy: AssetDecodePolicy::Cpu,
        }
    }

    #[must_use]
    pub const fn with_priority(mut self, priority: AssetPriority) -> Self {
        self.priority = priority;
        self
    }

    #[must_use]
    pub const fn with_decode_policy(mut self, decode_policy: AssetDecodePolicy) -> Self {
        self.decode_policy = decode_policy;
        self
    }

    #[must_use]
    pub fn scheduler_plan(&self) -> AssetSchedulerPlan {
        plan_asset_load_work(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetEvent {
    Requested {
        request_id: AssetRequestId,
        kind: AssetKind,
    },
    LoadStateChanged {
        request_id: AssetRequestId,
        state: AssetLoadState,
    },
    Resident {
        request_id: AssetRequestId,
    },
    Failed {
        request_id: AssetRequestId,
        error: AssetLoadError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLoadQueue {
    max_len: usize,
    requests: Vec<AssetLoadRequest>,
}

impl AssetLoadQueue {
    #[must_use]
    pub fn new(max_len: usize) -> Self {
        Self {
            max_len,
            requests: Vec::new(),
        }
    }

    #[must_use]
    pub fn max_len(&self) -> usize {
        self.max_len
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    pub fn try_push(&mut self, request: AssetLoadRequest) -> Result<(), AssetQueueError> {
        if self.requests.len() >= self.max_len {
            return Err(AssetQueueError::Full {
                max_len: self.max_len,
            });
        }
        if !request.request_id.is_valid() {
            return Err(AssetQueueError::InvalidRequestId);
        }
        if self
            .requests
            .iter()
            .any(|entry| entry.request_id == request.request_id)
        {
            return Err(AssetQueueError::DuplicateRequest {
                request_id: request.request_id,
            });
        }
        self.requests.push(request);
        Ok(())
    }

    #[must_use]
    pub fn pop_next(&mut self) -> Option<AssetLoadRequest> {
        let mut best_index = 0usize;
        let mut best_priority = self.requests.first()?.priority;
        for (index, request) in self.requests.iter().enumerate().skip(1) {
            if request.priority > best_priority {
                best_priority = request.priority;
                best_index = index;
            }
        }
        Some(self.requests.remove(best_index))
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssetLoadRequest> {
        self.requests.iter()
    }
}

impl Default for AssetLoadQueue {
    fn default() -> Self {
        Self::new(DEFAULT_ASSET_QUEUE_LIMIT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetQueueError {
    DuplicateRequest { request_id: AssetRequestId },
    Full { max_len: usize },
    InvalidRequestId,
}

impl fmt::Display for AssetQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRequest { .. } => f.write_str("asset load request is duplicated"),
            Self::Full { .. } => f.write_str("asset load queue is full"),
            Self::InvalidRequestId => f.write_str("asset load request id is invalid"),
        }
    }
}

impl std::error::Error for AssetQueueError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetEventQueue {
    max_len: usize,
    events: Vec<AssetEvent>,
}

impl AssetEventQueue {
    #[must_use]
    pub fn new(max_len: usize) -> Self {
        Self {
            max_len,
            events: Vec::new(),
        }
    }

    pub fn try_push(&mut self, event: AssetEvent) -> Result<(), AssetEventQueueError> {
        if self.events.len() >= self.max_len {
            return Err(AssetEventQueueError::Full {
                max_len: self.max_len,
            });
        }
        self.events.push(event);
        Ok(())
    }

    #[must_use]
    pub fn drain(&mut self) -> Vec<AssetEvent> {
        self.events.drain(..).collect()
    }
}

impl Default for AssetEventQueue {
    fn default() -> Self {
        Self::new(DEFAULT_ASSET_EVENT_LIMIT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetEventQueueError {
    Full { max_len: usize },
}

impl fmt::Display for AssetEventQueueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("asset event queue is full")
    }
}

impl std::error::Error for AssetEventQueueError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetManifest {
    max_entries: usize,
    entries: Vec<AssetManifestEntry>,
}

impl AssetManifest {
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            entries: Vec::new(),
        }
    }

    pub fn try_push(&mut self, entry: AssetManifestEntry) -> Result<(), AssetManifestError> {
        if self.entries.len() >= self.max_entries {
            return Err(AssetManifestError::Full {
                max_entries: self.max_entries,
            });
        }
        self.entries.push(entry);
        Ok(())
    }

    #[must_use]
    pub fn entries(&self) -> &[AssetManifestEntry] {
        &self.entries
    }
}

impl Default for AssetManifest {
    fn default() -> Self {
        Self::new(DEFAULT_ASSET_MANIFEST_LIMIT)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetManifestEntry {
    pub request_id: AssetRequestId,
    pub source: AssetSource,
    pub kind: AssetKind,
    pub byte_len: u64,
    pub content_hash: u64,
    pub priority: AssetPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetManifestError {
    Full { max_entries: usize },
}

impl fmt::Display for AssetManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("asset manifest is full")
    }
}

impl std::error::Error for AssetManifestError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetCache {
    max_entries: usize,
    states: BTreeMap<AssetRequestId, AssetLoadState>,
}

impl AssetCache {
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            states: BTreeMap::new(),
        }
    }

    pub fn set_state(
        &mut self,
        request_id: AssetRequestId,
        state: AssetLoadState,
    ) -> Result<(), AssetCacheError> {
        if !self.states.contains_key(&request_id) && self.states.len() >= self.max_entries {
            return Err(AssetCacheError::Full {
                max_entries: self.max_entries,
            });
        }
        self.states.insert(request_id, state);
        Ok(())
    }

    #[must_use]
    pub fn state(&self, request_id: AssetRequestId) -> Option<AssetLoadState> {
        self.states.get(&request_id).copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCacheError {
    Full { max_entries: usize },
}

impl fmt::Display for AssetCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("asset cache is full")
    }
}

impl std::error::Error for AssetCacheError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSchedulerStage {
    AdmitIo,
    BlockingFileIo,
    Decode,
    QueueUpload,
    PublishEvents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSchedulerLane {
    Scheduler,
    Blocking,
    Decode,
    RenderUpload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssetWorkItem {
    pub request_id: AssetRequestId,
    pub stage: AssetSchedulerStage,
    pub lane: AssetSchedulerLane,
    pub byte_budget: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSchedulerPlan {
    pub request_id: AssetRequestId,
    pub items: Vec<AssetWorkItem>,
}

#[must_use]
pub fn plan_asset_load_work(request: &AssetLoadRequest) -> AssetSchedulerPlan {
    let mut items = Vec::with_capacity(5);
    items.push(AssetWorkItem {
        request_id: request.request_id,
        stage: AssetSchedulerStage::AdmitIo,
        lane: AssetSchedulerLane::Scheduler,
        byte_budget: 0,
    });
    items.push(AssetWorkItem {
        request_id: request.request_id,
        stage: AssetSchedulerStage::BlockingFileIo,
        lane: AssetSchedulerLane::Blocking,
        byte_budget: request.byte_limit,
    });
    if request.decode_policy != AssetDecodePolicy::None {
        items.push(AssetWorkItem {
            request_id: request.request_id,
            stage: AssetSchedulerStage::Decode,
            lane: AssetSchedulerLane::Decode,
            byte_budget: request.byte_limit,
        });
    }
    items.push(AssetWorkItem {
        request_id: request.request_id,
        stage: AssetSchedulerStage::QueueUpload,
        lane: AssetSchedulerLane::RenderUpload,
        byte_budget: request.byte_limit,
    });
    items.push(AssetWorkItem {
        request_id: request.request_id,
        stage: AssetSchedulerStage::PublishEvents,
        lane: AssetSchedulerLane::Scheduler,
        byte_budget: 0,
    });
    AssetSchedulerPlan {
        request_id: request.request_id,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MeshAsset;
    struct TextureAsset;

    fn request(request_id: u64, priority: AssetPriority) -> AssetLoadRequest {
        AssetLoadRequest::new(
            AssetRequestId::new(request_id),
            AssetSource::VirtualPath(format!("asset://{request_id}")),
            AssetKind::Mesh,
            1024,
        )
        .with_priority(priority)
    }

    #[test]
    fn typed_handles_do_not_alias_across_asset_kinds() {
        let mesh = AssetHandle::<MeshAsset>::first(7);
        let texture = AssetHandle::<TextureAsset>::first(7);

        assert_eq!(mesh.slot(), texture.slot());
        assert_eq!(mesh.generation(), texture.generation());
        assert!(mesh.is_valid());
    }

    #[test]
    fn store_reuses_slots_with_generation_bump() {
        let mut store = AssetStore::new(1);
        let first = store.insert("mesh-a").unwrap();
        let removed = store.remove(first).unwrap();
        assert_eq!(removed.value, "mesh-a");

        let second = store.insert("mesh-b").unwrap();
        assert_eq!(first.slot(), second.slot());
        assert_ne!(first.generation(), second.generation());
        assert!(store.get(first).is_none());
        assert_eq!(store.get(second), Some(&"mesh-b"));
    }

    #[test]
    fn load_queue_is_bounded_and_relevance_prioritized() {
        let mut queue = AssetLoadQueue::new(2);
        queue.try_push(request(1, AssetPriority::LOW)).unwrap();
        queue.try_push(request(2, AssetPriority::CRITICAL)).unwrap();

        assert_eq!(
            queue.try_push(request(3, AssetPriority::HIGH)),
            Err(AssetQueueError::Full { max_len: 2 })
        );
        assert_eq!(queue.pop_next().unwrap().request_id, AssetRequestId::new(2));
        assert_eq!(queue.pop_next().unwrap().request_id, AssetRequestId::new(1));
    }

    #[test]
    fn scheduler_plan_keeps_blocking_decode_and_upload_explicit() {
        let plan = request(9, AssetPriority::NORMAL).scheduler_plan();
        assert!(plan.items.iter().any(|item| {
            item.stage == AssetSchedulerStage::BlockingFileIo
                && item.lane == AssetSchedulerLane::Blocking
        }));
        assert!(plan.items.iter().any(|item| {
            item.stage == AssetSchedulerStage::Decode && item.lane == AssetSchedulerLane::Decode
        }));
        assert!(plan.items.iter().any(|item| {
            item.stage == AssetSchedulerStage::QueueUpload
                && item.lane == AssetSchedulerLane::RenderUpload
        }));
    }
}
