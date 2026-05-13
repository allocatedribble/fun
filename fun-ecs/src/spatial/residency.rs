use crate::Resource;

use crate::{EcsSpatialPageKey, EcsStreamPriority};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsPageResidencyState {
    #[default]
    Cold = 0,
    Requested = 1,
    SourceQueued = 2,
    SourceLoading = 3,
    SourceReady = 4,
    Decoding = 5,
    CpuDecoded = 6,
    DerivedBuilding = 7,
    HandoffQueued = 8,
    ExternalPublishing = 9,
    ExternalResidentCoarse = 10,
    ExternalResidentFine = 11,
    FullyReady = 12,
    Retiring = 13,
    Evicted = 14,
    Failed = 15,
}

impl EcsPageResidencyState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::Requested => "requested",
            Self::SourceQueued => "source_queued",
            Self::SourceLoading => "source_loading",
            Self::SourceReady => "source_ready",
            Self::Decoding => "decoding",
            Self::CpuDecoded => "cpu_decoded",
            Self::DerivedBuilding => "derived_building",
            Self::HandoffQueued => "handoff_queued",
            Self::ExternalPublishing => "external_publishing",
            Self::ExternalResidentCoarse => "external_resident_coarse",
            Self::ExternalResidentFine => "external_resident_fine",
            Self::FullyReady => "fully_ready",
            Self::Retiring => "retiring",
            Self::Evicted => "evicted",
            Self::Failed => "failed",
        }
    }

    #[must_use]
    pub const fn is_external_resident(self) -> bool {
        matches!(
            self,
            Self::ExternalResidentCoarse | Self::ExternalResidentFine | Self::FullyReady
        )
    }

    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Evicted | Self::Failed)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsPageFailureCode {
    #[default]
    SourceUnavailable = 0,
    SourceTimeout = 1,
    DecodeRejected = 2,
    DerivedArtifactFailed = 3,
    ExternalPublishFailed = 4,
    BudgetExceeded = 5,
    StaleGeneration = 6,
}

impl EcsPageFailureCode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SourceUnavailable => "source_unavailable",
            Self::SourceTimeout => "source_timeout",
            Self::DecodeRejected => "decode_rejected",
            Self::DerivedArtifactFailed => "derived_artifact_failed",
            Self::ExternalPublishFailed => "external_publish_failed",
            Self::BudgetExceeded => "budget_exceeded",
            Self::StaleGeneration => "stale_generation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsPageResidencyRecord {
    pub key: EcsSpatialPageKey,
    pub state: EcsPageResidencyState,
    pub priority: EcsStreamPriority,
    pub source_epoch: u32,
    pub dirty_epoch: u32,
    pub artifact_epoch: u32,
    pub generation: u32,
    pub last_requested_frame: u32,
    pub last_visible_frame: u32,
    pub last_used_frame: u32,
    pub cpu_bytes: u32,
    pub external_bytes: u32,
    pub fallback: Option<EcsSpatialPageKey>,
    pub failure: Option<EcsPageFailureCode>,
}

impl EcsPageResidencyRecord {
    #[must_use]
    pub const fn new(key: EcsSpatialPageKey, priority: EcsStreamPriority, generation: u32) -> Self {
        Self {
            key,
            state: EcsPageResidencyState::Cold,
            priority,
            source_epoch: 0,
            dirty_epoch: 0,
            artifact_epoch: 0,
            generation,
            last_requested_frame: 0,
            last_visible_frame: 0,
            last_used_frame: 0,
            cpu_bytes: 0,
            external_bytes: 0,
            fallback: None,
            failure: None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsPageResidencyMap {
    pub keys: Vec<EcsSpatialPageKey>,
    pub states: Vec<EcsPageResidencyState>,
    pub generations: Vec<u32>,
}

impl EcsPageResidencyMap {
    pub fn upsert(
        &mut self,
        key: EcsSpatialPageKey,
        state: EcsPageResidencyState,
        generation: u32,
    ) {
        if let Some(index) = self.keys.iter().position(|candidate| *candidate == key) {
            self.states[index] = state;
            self.generations[index] = generation;
            return;
        }
        self.keys.push(key);
        self.states.push(state);
        self.generations.push(generation);
    }

    #[must_use]
    pub fn state(&self, key: EcsSpatialPageKey) -> Option<EcsPageResidencyState> {
        self.keys
            .iter()
            .position(|candidate| *candidate == key)
            .map(|index| self.states[index])
    }

    #[must_use]
    pub fn is_consistent(&self) -> bool {
        let len = self.keys.len();
        self.states.len() == len && self.generations.len() == len
    }
}
