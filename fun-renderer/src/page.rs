use std::collections::BTreeMap;

pub const PAGE_SCHEDULER_SCHEMA_VERSION: u16 = 1;
pub const PAGE_OWNER_COUNT: usize = 6;
pub const PAGE_AGE_BUCKET_COUNT: usize = 5;
pub const PAGE_PRIORITY_BUCKET_COUNT: usize = 6;
pub const PAGE_SCHEDULER_BENCHMARK_ARTIFACT_ENV: &str =
    "FUN_RENDERER_PAGE_SCHEDULER_BENCHMARK_ARTIFACT";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PageOwner {
    VirtualGeometry,
    VirtualShadows,
    StreamedTextures,
    GiRadianceCache,
    MaterialCache,
    NeuralCache,
}

impl PageOwner {
    pub const ALL: [Self; PAGE_OWNER_COUNT] = [
        Self::VirtualGeometry,
        Self::VirtualShadows,
        Self::StreamedTextures,
        Self::GiRadianceCache,
        Self::MaterialCache,
        Self::NeuralCache,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VirtualGeometry => "virtual_geometry",
            Self::VirtualShadows => "virtual_shadows",
            Self::StreamedTextures => "streamed_textures",
            Self::GiRadianceCache => "gi_radiance_cache",
            Self::MaterialCache => "material_cache",
            Self::NeuralCache => "neural_cache",
        }
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::VirtualGeometry => 0,
            Self::VirtualShadows => 1,
            Self::StreamedTextures => 2,
            Self::GiRadianceCache => 3,
            Self::MaterialCache => 4,
            Self::NeuralCache => 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalPageId {
    pub owner: PageOwner,
    pub value: u64,
}

impl LogicalPageId {
    #[must_use]
    pub const fn new(owner: PageOwner, value: u64) -> Self {
        Self { owner, value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PhysicalPageSlot {
    pub index: u32,
    pub generation: u32,
}

impl PhysicalPageSlot {
    pub const INVALID: Self = Self {
        index: u32::MAX,
        generation: 0,
    };

    #[must_use]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.index != u32::MAX && self.generation != 0
    }

    #[must_use]
    pub const fn next_generation(self) -> Self {
        let next = if self.generation == u32::MAX {
            1
        } else {
            self.generation + 1
        };
        Self {
            index: self.index,
            generation: next,
        }
    }
}

impl Default for PhysicalPageSlot {
    fn default() -> Self {
        Self::INVALID
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageGeneration(pub u32);

impl PageGeneration {
    #[must_use]
    pub const fn next(self) -> Self {
        if self.0 == u32::MAX {
            Self(1)
        } else {
            Self(self.0 + 1)
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PageResidencyState {
    #[default]
    Missing,
    Requested,
    Uploading,
    Resident,
    Pinned,
    EvictionCandidate,
    Evicting,
    Invalidated,
}

impl PageResidencyState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Requested => "requested",
            Self::Uploading => "uploading",
            Self::Resident => "resident",
            Self::Pinned => "pinned",
            Self::EvictionCandidate => "eviction_candidate",
            Self::Evicting => "evicting",
            Self::Invalidated => "invalidated",
        }
    }

    #[must_use]
    pub const fn is_resident(self) -> bool {
        matches!(self, Self::Resident | Self::Pinned)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PageUploadState {
    #[default]
    NotQueued,
    Queued,
    Uploading,
    Uploaded,
    Failed,
}

impl PageUploadState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotQueued => "not_queued",
            Self::Queued => "queued",
            Self::Uploading => "uploading",
            Self::Uploaded => "uploaded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PageEvictionState {
    #[default]
    NotCandidate,
    Candidate,
    Scheduled,
    Evicting,
    Evicted,
}

impl PageEvictionState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotCandidate => "not_candidate",
            Self::Candidate => "candidate",
            Self::Scheduled => "scheduled",
            Self::Evicting => "evicting",
            Self::Evicted => "evicted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePriorityWeights {
    pub projected_area: u16,
    pub camera_proximity: u16,
    pub visibility_confidence: u16,
    pub temporal_instability: u16,
    pub motion_magnitude: u16,
    pub luminance_contrast_importance: u16,
    pub shadow_receiver_demand: u16,
    pub gameplay_salience: u16,
    pub editor_focus: u16,
}

impl Default for PagePriorityWeights {
    fn default() -> Self {
        Self {
            projected_area: 12,
            camera_proximity: 10,
            visibility_confidence: 10,
            temporal_instability: 7,
            motion_magnitude: 7,
            luminance_contrast_importance: 8,
            shadow_receiver_demand: 12,
            gameplay_salience: 14,
            editor_focus: 16,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PagePriorityInputs {
    pub projected_area: u16,
    pub camera_proximity: u16,
    pub visibility_confidence: u16,
    pub temporal_instability: u16,
    pub motion_magnitude: u16,
    pub luminance_contrast_importance: u16,
    pub shadow_receiver_demand: u16,
    pub gameplay_salience: u16,
    pub editor_focus: u16,
}

impl PagePriorityInputs {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            projected_area: 0,
            camera_proximity: 0,
            visibility_confidence: 0,
            temporal_instability: 0,
            motion_magnitude: 0,
            luminance_contrast_importance: 0,
            shadow_receiver_demand: 0,
            gameplay_salience: 0,
            editor_focus: 0,
        }
    }

    #[must_use]
    pub const fn editor_critical() -> Self {
        Self {
            projected_area: 220,
            camera_proximity: 210,
            visibility_confidence: 240,
            temporal_instability: 96,
            motion_magnitude: 80,
            luminance_contrast_importance: 160,
            shadow_receiver_demand: 192,
            gameplay_salience: 255,
            editor_focus: 255,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PagePriorityBreakdown {
    pub projected_area: u32,
    pub camera_proximity: u32,
    pub visibility_confidence: u32,
    pub temporal_instability: u32,
    pub motion_magnitude: u32,
    pub luminance_contrast_importance: u32,
    pub shadow_receiver_demand: u32,
    pub gameplay_salience: u32,
    pub editor_focus: u32,
    pub total: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PagePriorityScore {
    pub value: u16,
    pub inputs: PagePriorityInputs,
    pub breakdown: PagePriorityBreakdown,
}

impl PagePriorityScore {
    #[must_use]
    pub fn compute(inputs: PagePriorityInputs, weights: PagePriorityWeights) -> Self {
        let breakdown = PagePriorityBreakdown {
            projected_area: weighted(inputs.projected_area, weights.projected_area),
            camera_proximity: weighted(inputs.camera_proximity, weights.camera_proximity),
            visibility_confidence: weighted(
                inputs.visibility_confidence,
                weights.visibility_confidence,
            ),
            temporal_instability: weighted(
                inputs.temporal_instability,
                weights.temporal_instability,
            ),
            motion_magnitude: weighted(inputs.motion_magnitude, weights.motion_magnitude),
            luminance_contrast_importance: weighted(
                inputs.luminance_contrast_importance,
                weights.luminance_contrast_importance,
            ),
            shadow_receiver_demand: weighted(
                inputs.shadow_receiver_demand,
                weights.shadow_receiver_demand,
            ),
            gameplay_salience: weighted(inputs.gameplay_salience, weights.gameplay_salience),
            editor_focus: weighted(inputs.editor_focus, weights.editor_focus),
            total: 0,
        };
        let total = breakdown
            .projected_area
            .saturating_add(breakdown.camera_proximity)
            .saturating_add(breakdown.visibility_confidence)
            .saturating_add(breakdown.temporal_instability)
            .saturating_add(breakdown.motion_magnitude)
            .saturating_add(breakdown.luminance_contrast_importance)
            .saturating_add(breakdown.shadow_receiver_demand)
            .saturating_add(breakdown.gameplay_salience)
            .saturating_add(breakdown.editor_focus);
        Self {
            value: total.min(u16::MAX as u32) as u16,
            inputs,
            breakdown: PagePriorityBreakdown { total, ..breakdown },
        }
    }
}

fn weighted(value: u16, weight: u16) -> u32 {
    u32::from(value).saturating_mul(u32::from(weight))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    pub logical_id: LogicalPageId,
    pub estimated_upload_bytes: u64,
    pub priority_inputs: PagePriorityInputs,
}

impl PageRequest {
    #[must_use]
    pub const fn new(
        owner: PageOwner,
        value: u64,
        estimated_upload_bytes: u64,
        priority_inputs: PagePriorityInputs,
    ) -> Self {
        Self {
            logical_id: LogicalPageId::new(owner, value),
            estimated_upload_bytes,
            priority_inputs,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageOwnerFrameRequests {
    pub owner: PageOwner,
    pub requests: Vec<PageRequest>,
}

impl PageOwnerFrameRequests {
    #[must_use]
    pub fn new(owner: PageOwner, requests: Vec<PageRequest>) -> Self {
        Self { owner, requests }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PageRequestSubmissionReport {
    pub accepted: u32,
    pub rejected_owner_mismatch: u32,
    pub already_resident: u32,
    pub already_requested: u32,
    pub updated_priority: u32,
    pub newly_requested: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageRequestDecision {
    Requested,
    AlreadyRequested,
    AlreadyResident,
    UpdatedPriority,
    RejectedOwnerMismatch,
}

impl PageRequestDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::AlreadyRequested => "already_requested",
            Self::AlreadyResident => "already_resident",
            Self::UpdatedPriority => "updated_priority",
            Self::RejectedOwnerMismatch => "rejected_owner_mismatch",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRecord {
    pub logical_id: LogicalPageId,
    pub physical_slot: PhysicalPageSlot,
    pub owner: PageOwner,
    pub generation: PageGeneration,
    pub residency_state: PageResidencyState,
    pub last_used_frame: u64,
    pub priority: PagePriorityScore,
    pub upload_state: PageUploadState,
    pub eviction_state: PageEvictionState,
    pub estimated_upload_bytes: u64,
}

impl PageRecord {
    #[must_use]
    pub fn missing(logical_id: LogicalPageId, generation: PageGeneration) -> Self {
        Self {
            logical_id,
            physical_slot: PhysicalPageSlot::INVALID,
            owner: logical_id.owner,
            generation,
            residency_state: PageResidencyState::Missing,
            last_used_frame: 0,
            priority: PagePriorityScore::default(),
            upload_state: PageUploadState::NotQueued,
            eviction_state: PageEvictionState::NotCandidate,
            estimated_upload_bytes: 0,
        }
    }

    #[must_use]
    pub const fn is_resident(&self) -> bool {
        self.residency_state.is_resident()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSchedulerConfig {
    pub physical_slot_count: u32,
    pub max_upload_bytes_per_frame: u64,
    pub fault_storm_threshold: u32,
    pub priority_weights: PagePriorityWeights,
}

impl Default for PageSchedulerConfig {
    fn default() -> Self {
        Self {
            physical_slot_count: 4096,
            max_upload_bytes_per_frame: 64 * 1024 * 1024,
            fault_storm_threshold: 256,
            priority_weights: PagePriorityWeights::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PageAgeHeatmap {
    pub buckets: [u32; PAGE_AGE_BUCKET_COUNT],
}

impl PageAgeHeatmap {
    pub fn record_age(&mut self, age_frames: u64) {
        let index = if age_frames == 0 {
            0
        } else if age_frames <= 2 {
            1
        } else if age_frames <= 8 {
            2
        } else if age_frames <= 32 {
            3
        } else {
            4
        };
        self.buckets[index] = self.buckets[index].saturating_add(1);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PagePriorityHeatmap {
    pub buckets: [u32; PAGE_PRIORITY_BUCKET_COUNT],
}

impl PagePriorityHeatmap {
    pub fn record_priority(&mut self, priority: u16) {
        let index = if priority == 0 {
            0
        } else if priority <= 255 {
            1
        } else if priority <= 1023 {
            2
        } else if priority <= 4095 {
            3
        } else if priority <= 16_383 {
            4
        } else {
            5
        };
        self.buckets[index] = self.buckets[index].saturating_add(1);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PageSchedulerHighWaterMarks {
    pub resident_pages: u32,
    pub pinned_pages: u32,
    pub page_faults_per_frame: u32,
    pub evictions_per_frame: u32,
    pub upload_bytes_per_frame: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PageSchedulerDiagnostics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub page_faults_this_frame: u32,
    pub evictions_this_frame: u32,
    pub upload_bytes_this_frame: u64,
    pub upload_bytes_by_owner: [u64; PAGE_OWNER_COUNT],
    pub resident_pages_by_owner: [u32; PAGE_OWNER_COUNT],
    pub pinned_page_count: u32,
    pub page_age_heatmap: PageAgeHeatmap,
    pub priority_heatmap: PagePriorityHeatmap,
    pub fault_storm_detected: bool,
    pub high_water_marks: PageSchedulerHighWaterMarks,
}

impl PageSchedulerDiagnostics {
    pub fn begin_frame(&mut self, frame_index: u64) {
        let high_water_marks = self.high_water_marks;
        *self = Self {
            schema_version: PAGE_SCHEDULER_SCHEMA_VERSION,
            frame_index,
            high_water_marks,
            ..Default::default()
        };
    }

    fn record_fault(&mut self, threshold: u32) {
        self.page_faults_this_frame = self.page_faults_this_frame.saturating_add(1);
        self.high_water_marks.page_faults_per_frame = self
            .high_water_marks
            .page_faults_per_frame
            .max(self.page_faults_this_frame);
        self.fault_storm_detected |= self.page_faults_this_frame >= threshold;
    }

    fn record_upload(&mut self, owner: PageOwner, bytes: u64) {
        self.upload_bytes_this_frame = self.upload_bytes_this_frame.saturating_add(bytes);
        self.upload_bytes_by_owner[owner.index()] =
            self.upload_bytes_by_owner[owner.index()].saturating_add(bytes);
        self.high_water_marks.upload_bytes_per_frame = self
            .high_water_marks
            .upload_bytes_per_frame
            .max(self.upload_bytes_this_frame);
    }

    fn record_eviction(&mut self) {
        self.evictions_this_frame = self.evictions_this_frame.saturating_add(1);
        self.high_water_marks.evictions_per_frame = self
            .high_water_marks
            .evictions_per_frame
            .max(self.evictions_this_frame);
    }

    fn rebuild_population<'a>(
        &mut self,
        frame_index: u64,
        pages: impl Iterator<Item = &'a PageRecord>,
    ) {
        self.resident_pages_by_owner = [0; PAGE_OWNER_COUNT];
        self.pinned_page_count = 0;
        self.page_age_heatmap = PageAgeHeatmap::default();
        self.priority_heatmap = PagePriorityHeatmap::default();
        for page in pages {
            self.priority_heatmap.record_priority(page.priority.value);
            if page.residency_state.is_resident() {
                self.resident_pages_by_owner[page.owner.index()] =
                    self.resident_pages_by_owner[page.owner.index()].saturating_add(1);
                self.page_age_heatmap
                    .record_age(frame_index.saturating_sub(page.last_used_frame));
            }
            if page.residency_state == PageResidencyState::Pinned {
                self.pinned_page_count = self.pinned_page_count.saturating_add(1);
            }
        }
        let resident_total = self
            .resident_pages_by_owner
            .iter()
            .copied()
            .fold(0u32, u32::saturating_add);
        self.high_water_marks.resident_pages =
            self.high_water_marks.resident_pages.max(resident_total);
        self.high_water_marks.pinned_pages = self
            .high_water_marks
            .pinned_pages
            .max(self.pinned_page_count);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PageUploadScheduleReport {
    pub scheduled_pages: u32,
    pub deferred_pages: u32,
    pub upload_bytes: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PageEvictionReport {
    pub selected_candidates: u32,
    pub evicted_pages: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageSchedulerDebugArtifact {
    pub schema_version: u16,
    pub frame_index: u64,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageScheduler {
    config: PageSchedulerConfig,
    frame_index: u64,
    next_generation: PageGeneration,
    pages: BTreeMap<LogicalPageId, PageRecord>,
    free_slots: Vec<PhysicalPageSlot>,
    diagnostics: PageSchedulerDiagnostics,
}

impl PageScheduler {
    #[must_use]
    pub fn new(config: PageSchedulerConfig) -> Self {
        let mut free_slots = Vec::with_capacity(config.physical_slot_count as usize);
        for index in 0..config.physical_slot_count {
            free_slots.push(PhysicalPageSlot::new(index, 1));
        }
        free_slots.reverse();
        let mut diagnostics = PageSchedulerDiagnostics::default();
        diagnostics.begin_frame(0);
        Self {
            config,
            frame_index: 0,
            next_generation: PageGeneration(1),
            pages: BTreeMap::new(),
            free_slots,
            diagnostics,
        }
    }

    #[must_use]
    pub fn config(&self) -> PageSchedulerConfig {
        self.config
    }

    #[must_use]
    pub fn diagnostics(&self) -> PageSchedulerDiagnostics {
        self.diagnostics
    }

    #[must_use]
    pub fn page(&self, logical_id: LogicalPageId) -> Option<&PageRecord> {
        self.pages.get(&logical_id)
    }

    #[must_use]
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    #[must_use]
    pub fn free_slot_count(&self) -> usize {
        self.free_slots.len()
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.diagnostics.begin_frame(frame_index);
        self.refresh_diagnostics();
    }

    pub fn submit_owner_requests(
        &mut self,
        owner_requests: &PageOwnerFrameRequests,
    ) -> PageRequestSubmissionReport {
        let mut report = PageRequestSubmissionReport::default();
        for request in &owner_requests.requests {
            let decision = self.request_page_for_owner(owner_requests.owner, *request);
            match decision {
                PageRequestDecision::Requested => {
                    report.accepted = report.accepted.saturating_add(1);
                    report.newly_requested = report.newly_requested.saturating_add(1);
                }
                PageRequestDecision::AlreadyRequested => {
                    report.accepted = report.accepted.saturating_add(1);
                    report.already_requested = report.already_requested.saturating_add(1);
                }
                PageRequestDecision::AlreadyResident => {
                    report.accepted = report.accepted.saturating_add(1);
                    report.already_resident = report.already_resident.saturating_add(1);
                }
                PageRequestDecision::UpdatedPriority => {
                    report.accepted = report.accepted.saturating_add(1);
                    report.updated_priority = report.updated_priority.saturating_add(1);
                }
                PageRequestDecision::RejectedOwnerMismatch => {
                    report.rejected_owner_mismatch =
                        report.rejected_owner_mismatch.saturating_add(1);
                }
            }
        }
        self.refresh_diagnostics();
        report
    }

    pub fn request_page_for_owner(
        &mut self,
        owner: PageOwner,
        request: PageRequest,
    ) -> PageRequestDecision {
        if request.logical_id.owner != owner {
            return PageRequestDecision::RejectedOwnerMismatch;
        }

        let priority =
            PagePriorityScore::compute(request.priority_inputs, self.config.priority_weights);
        let frame_index = self.frame_index;
        let threshold = self.config.fault_storm_threshold;
        if !self.pages.contains_key(&request.logical_id) {
            let generation = self.next_generation;
            self.next_generation = self.next_generation.next();
            self.pages.insert(
                request.logical_id,
                PageRecord::missing(request.logical_id, generation),
            );
        }
        let record = self
            .pages
            .get_mut(&request.logical_id)
            .expect("page record should exist after insertion");
        record.priority = priority;
        record.estimated_upload_bytes = request.estimated_upload_bytes;

        match record.residency_state {
            PageResidencyState::Missing | PageResidencyState::Invalidated => {
                record.residency_state = PageResidencyState::Requested;
                record.upload_state = PageUploadState::Queued;
                record.eviction_state = PageEvictionState::NotCandidate;
                record.last_used_frame = frame_index;
                self.diagnostics.record_fault(threshold);
                PageRequestDecision::Requested
            }
            PageResidencyState::Requested | PageResidencyState::Uploading => {
                PageRequestDecision::AlreadyRequested
            }
            PageResidencyState::Resident | PageResidencyState::Pinned => {
                record.last_used_frame = frame_index;
                PageRequestDecision::AlreadyResident
            }
            PageResidencyState::EvictionCandidate | PageResidencyState::Evicting => {
                record.residency_state = PageResidencyState::Resident;
                record.eviction_state = PageEvictionState::NotCandidate;
                record.last_used_frame = frame_index;
                PageRequestDecision::UpdatedPriority
            }
        }
    }

    pub fn schedule_uploads(&mut self) -> PageUploadScheduleReport {
        let mut candidates: Vec<_> = self
            .pages
            .iter()
            .filter_map(|(id, page)| {
                (page.residency_state == PageResidencyState::Requested).then_some((
                    *id,
                    page.priority.value,
                    page.estimated_upload_bytes,
                ))
            })
            .collect();
        candidates.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| left.0.owner.cmp(&right.0.owner))
                .then_with(|| left.0.value.cmp(&right.0.value))
        });

        let mut report = PageUploadScheduleReport::default();
        for (id, _priority, bytes) in candidates {
            if report.upload_bytes.saturating_add(bytes) > self.config.max_upload_bytes_per_frame {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                continue;
            }
            let Some(slot) = self.allocate_physical_slot() else {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                continue;
            };
            let Some(page) = self.pages.get_mut(&id) else {
                continue;
            };
            page.physical_slot = slot;
            page.residency_state = PageResidencyState::Uploading;
            page.upload_state = PageUploadState::Uploading;
            page.eviction_state = PageEvictionState::NotCandidate;
            page.last_used_frame = self.frame_index;
            let owner = page.owner;
            report.scheduled_pages = report.scheduled_pages.saturating_add(1);
            report.upload_bytes = report.upload_bytes.saturating_add(bytes);
            self.diagnostics.record_upload(owner, bytes);
        }
        self.refresh_diagnostics();
        report
    }

    pub fn complete_upload(&mut self, logical_id: LogicalPageId) -> bool {
        let Some(page) = self.pages.get_mut(&logical_id) else {
            return false;
        };
        if page.residency_state != PageResidencyState::Uploading {
            return false;
        }
        page.residency_state = PageResidencyState::Resident;
        page.upload_state = PageUploadState::Uploaded;
        page.last_used_frame = self.frame_index;
        self.refresh_diagnostics();
        true
    }

    pub fn mark_used(&mut self, logical_id: LogicalPageId) -> bool {
        let Some(page) = self.pages.get_mut(&logical_id) else {
            return false;
        };
        if !page.residency_state.is_resident() {
            return false;
        }
        page.last_used_frame = self.frame_index;
        self.refresh_diagnostics();
        true
    }

    pub fn pin_page(&mut self, logical_id: LogicalPageId) -> bool {
        let Some(page) = self.pages.get_mut(&logical_id) else {
            return false;
        };
        if !page.residency_state.is_resident() {
            return false;
        }
        page.residency_state = PageResidencyState::Pinned;
        page.eviction_state = PageEvictionState::NotCandidate;
        self.refresh_diagnostics();
        true
    }

    pub fn unpin_page(&mut self, logical_id: LogicalPageId) -> bool {
        let Some(page) = self.pages.get_mut(&logical_id) else {
            return false;
        };
        if page.residency_state != PageResidencyState::Pinned {
            return false;
        }
        page.residency_state = PageResidencyState::Resident;
        self.refresh_diagnostics();
        true
    }

    pub fn invalidate_page(&mut self, logical_id: LogicalPageId) -> bool {
        let mut released_slot = None;
        let Some(page) = self.pages.get_mut(&logical_id) else {
            return false;
        };
        if page.physical_slot.is_valid() {
            released_slot = Some(page.physical_slot);
        }
        page.physical_slot = PhysicalPageSlot::INVALID;
        page.residency_state = PageResidencyState::Invalidated;
        page.upload_state = PageUploadState::NotQueued;
        page.eviction_state = PageEvictionState::NotCandidate;
        page.generation = page.generation.next();
        if let Some(slot) = released_slot {
            self.release_physical_slot(slot);
        }
        self.refresh_diagnostics();
        true
    }

    pub fn select_eviction_candidates(&mut self, target_count: u32) -> PageEvictionReport {
        let mut candidates: Vec<_> = self
            .pages
            .iter()
            .filter_map(|(id, page)| {
                (page.residency_state == PageResidencyState::Resident).then_some((
                    *id,
                    page.priority.value,
                    page.last_used_frame,
                ))
            })
            .collect();
        candidates.sort_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.0.owner.cmp(&right.0.owner))
                .then_with(|| left.0.value.cmp(&right.0.value))
        });

        let mut report = PageEvictionReport::default();
        for (id, _priority, _last_used) in candidates.into_iter().take(target_count as usize) {
            if let Some(page) = self.pages.get_mut(&id) {
                page.residency_state = PageResidencyState::EvictionCandidate;
                page.eviction_state = PageEvictionState::Candidate;
                report.selected_candidates = report.selected_candidates.saturating_add(1);
            }
        }
        self.refresh_diagnostics();
        report
    }

    pub fn evict_candidates(&mut self) -> PageEvictionReport {
        let candidates: Vec<_> = self
            .pages
            .iter()
            .filter_map(|(id, page)| {
                (page.residency_state == PageResidencyState::EvictionCandidate).then_some(*id)
            })
            .collect();
        let mut report = PageEvictionReport {
            selected_candidates: candidates.len() as u32,
            evicted_pages: 0,
        };
        for id in candidates {
            let mut released_slot = None;
            let Some(page) = self.pages.get_mut(&id) else {
                continue;
            };
            page.residency_state = PageResidencyState::Evicting;
            page.eviction_state = PageEvictionState::Evicting;
            if page.physical_slot.is_valid() {
                released_slot = Some(page.physical_slot);
            }
            page.physical_slot = PhysicalPageSlot::INVALID;
            page.residency_state = PageResidencyState::Missing;
            page.upload_state = PageUploadState::NotQueued;
            page.eviction_state = PageEvictionState::Evicted;
            page.generation = page.generation.next();
            if let Some(slot) = released_slot {
                self.release_physical_slot(slot);
            }
            report.evicted_pages = report.evicted_pages.saturating_add(1);
            self.diagnostics.record_eviction();
        }
        self.refresh_diagnostics();
        report
    }

    #[must_use]
    pub fn debug_artifact(&self) -> PageSchedulerDebugArtifact {
        use core::fmt::Write as _;

        let diagnostics = self.diagnostics();
        let mut content = String::new();
        let _ = writeln!(
            content,
            "schema_version={} frame_index={} pages={} free_slots={} faults={} evictions={} upload_bytes={} fault_storm={}",
            PAGE_SCHEDULER_SCHEMA_VERSION,
            diagnostics.frame_index,
            self.page_count(),
            self.free_slot_count(),
            diagnostics.page_faults_this_frame,
            diagnostics.evictions_this_frame,
            diagnostics.upload_bytes_this_frame,
            diagnostics.fault_storm_detected,
        );
        let _ = writeln!(content, "owners:");
        for owner in PageOwner::ALL {
            let index = owner.index();
            let _ = writeln!(
                content,
                "{} resident_pages={} upload_bytes={}",
                owner.as_str(),
                diagnostics.resident_pages_by_owner[index],
                diagnostics.upload_bytes_by_owner[index],
            );
        }
        let _ = writeln!(
            content,
            "age_heatmap={:?}",
            diagnostics.page_age_heatmap.buckets
        );
        let _ = writeln!(
            content,
            "priority_heatmap={:?}",
            diagnostics.priority_heatmap.buckets
        );
        PageSchedulerDebugArtifact {
            schema_version: PAGE_SCHEDULER_SCHEMA_VERSION,
            frame_index: diagnostics.frame_index,
            content,
        }
    }

    fn allocate_physical_slot(&mut self) -> Option<PhysicalPageSlot> {
        self.free_slots.pop()
    }

    fn release_physical_slot(&mut self, slot: PhysicalPageSlot) {
        self.free_slots.push(slot.next_generation());
    }

    fn refresh_diagnostics(&mut self) {
        self.diagnostics
            .rebuild_population(self.frame_index, self.pages.values());
    }
}

impl Default for PageScheduler {
    fn default() -> Self {
        Self::new(PageSchedulerConfig::default())
    }
}

impl Default for LogicalPageId {
    fn default() -> Self {
        Self {
            owner: PageOwner::VirtualGeometry,
            value: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(owner: PageOwner, value: u64, bytes: u64, salience: u16) -> PageRequest {
        let mut inputs = PagePriorityInputs::editor_critical();
        inputs.gameplay_salience = salience;
        inputs.editor_focus = salience / 2;
        PageRequest::new(owner, value, bytes, inputs)
    }

    #[test]
    fn page_identity_and_state_catalog_cover_virtual_resource_spine() {
        assert_eq!(PageOwner::ALL.len(), PAGE_OWNER_COUNT);
        assert_eq!(PageOwner::VirtualGeometry.as_str(), "virtual_geometry");
        assert_eq!(PageOwner::VirtualShadows.as_str(), "virtual_shadows");
        assert_eq!(PageOwner::StreamedTextures.as_str(), "streamed_textures");
        assert_eq!(PageOwner::GiRadianceCache.as_str(), "gi_radiance_cache");
        assert_eq!(PageOwner::MaterialCache.as_str(), "material_cache");
        assert_eq!(PageOwner::NeuralCache.as_str(), "neural_cache");

        assert_eq!(PageResidencyState::Missing.as_str(), "missing");
        assert_eq!(PageResidencyState::Requested.as_str(), "requested");
        assert_eq!(PageResidencyState::Uploading.as_str(), "uploading");
        assert_eq!(PageResidencyState::Resident.as_str(), "resident");
        assert_eq!(PageResidencyState::Pinned.as_str(), "pinned");
        assert_eq!(
            PageResidencyState::EvictionCandidate.as_str(),
            "eviction_candidate"
        );
        assert_eq!(PageResidencyState::Evicting.as_str(), "evicting");
        assert_eq!(PageResidencyState::Invalidated.as_str(), "invalidated");

        let id = LogicalPageId::new(PageOwner::VirtualGeometry, 42);
        let record = PageRecord::missing(id, PageGeneration(7));
        assert_eq!(record.logical_id, id);
        assert_eq!(record.owner, PageOwner::VirtualGeometry);
        assert_eq!(record.generation, PageGeneration(7));
        assert!(!record.physical_slot.is_valid());
    }

    #[test]
    fn priority_heuristic_is_deterministic_and_inspectable() {
        let low = PagePriorityScore::compute(
            PagePriorityInputs {
                projected_area: 8,
                camera_proximity: 8,
                visibility_confidence: 64,
                ..PagePriorityInputs::zero()
            },
            PagePriorityWeights::default(),
        );
        let high = PagePriorityScore::compute(
            PagePriorityInputs::editor_critical(),
            PagePriorityWeights::default(),
        );

        assert!(high.value > low.value);
        assert_eq!(
            high.breakdown.projected_area,
            weighted(220, PagePriorityWeights::default().projected_area)
        );
        assert_eq!(high.breakdown.total as u16, high.value);
        assert!(high.breakdown.editor_focus > high.breakdown.temporal_instability);
    }

    #[test]
    fn shared_scheduler_accepts_virtual_geometry_and_shadow_requests() {
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 4,
            max_upload_bytes_per_frame: 128 * 1024,
            fault_storm_threshold: 8,
            priority_weights: PagePriorityWeights::default(),
        });
        scheduler.begin_frame(12);

        let geometry = PageOwnerFrameRequests::new(
            PageOwner::VirtualGeometry,
            vec![
                request(PageOwner::VirtualGeometry, 1, 16 * 1024, 220),
                request(PageOwner::VirtualGeometry, 2, 16 * 1024, 200),
            ],
        );
        let shadows = PageOwnerFrameRequests::new(
            PageOwner::VirtualShadows,
            vec![request(PageOwner::VirtualShadows, 100, 32 * 1024, 255)],
        );

        assert_eq!(
            scheduler.submit_owner_requests(&geometry).newly_requested,
            2
        );
        assert_eq!(scheduler.submit_owner_requests(&shadows).newly_requested, 1);
        let upload = scheduler.schedule_uploads();
        assert_eq!(upload.scheduled_pages, 3);
        assert_eq!(upload.upload_bytes, 64 * 1024);

        assert!(scheduler.complete_upload(LogicalPageId::new(PageOwner::VirtualGeometry, 1)));
        assert!(scheduler.complete_upload(LogicalPageId::new(PageOwner::VirtualGeometry, 2)));
        assert!(scheduler.complete_upload(LogicalPageId::new(PageOwner::VirtualShadows, 100)));

        let diagnostics = scheduler.diagnostics();
        assert_eq!(
            diagnostics.resident_pages_by_owner[PageOwner::VirtualGeometry.index()],
            2
        );
        assert_eq!(
            diagnostics.resident_pages_by_owner[PageOwner::VirtualShadows.index()],
            1
        );
        assert_eq!(
            diagnostics.upload_bytes_by_owner[PageOwner::VirtualGeometry.index()],
            32 * 1024
        );
        assert_eq!(
            diagnostics.upload_bytes_by_owner[PageOwner::VirtualShadows.index()],
            32 * 1024
        );
    }

    #[test]
    fn residency_stress_test_preserves_pinned_pages_and_measures_fault_storm() {
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 3,
            max_upload_bytes_per_frame: 1024 * 1024,
            fault_storm_threshold: 4,
            priority_weights: PagePriorityWeights::default(),
        });
        scheduler.begin_frame(1);
        let requests = PageOwnerFrameRequests::new(
            PageOwner::VirtualGeometry,
            (0..6)
                .map(|page| request(PageOwner::VirtualGeometry, page, 4096, 64 + page as u16))
                .collect(),
        );
        let report = scheduler.submit_owner_requests(&requests);
        assert_eq!(report.newly_requested, 6);
        assert!(scheduler.diagnostics().fault_storm_detected);

        let upload = scheduler.schedule_uploads();
        assert_eq!(upload.scheduled_pages, 3);
        assert_eq!(upload.deferred_pages, 3);
        for page in 3..6 {
            assert!(
                scheduler.complete_upload(LogicalPageId::new(PageOwner::VirtualGeometry, page))
            );
        }
        let pinned = LogicalPageId::new(PageOwner::VirtualGeometry, 5);
        assert!(scheduler.pin_page(pinned));

        scheduler.begin_frame(64);
        assert!(scheduler.mark_used(pinned));
        let selected = scheduler.select_eviction_candidates(3);
        assert_eq!(selected.selected_candidates, 2);
        let evicted = scheduler.evict_candidates();
        assert_eq!(evicted.evicted_pages, 2);

        let diagnostics = scheduler.diagnostics();
        assert_eq!(diagnostics.pinned_page_count, 1);
        assert_eq!(
            diagnostics.resident_pages_by_owner[PageOwner::VirtualGeometry.index()],
            1
        );
        assert_eq!(diagnostics.evictions_this_frame, 2);
        assert_eq!(
            scheduler
                .page(pinned)
                .expect("pinned page should remain")
                .residency_state,
            PageResidencyState::Pinned
        );
        assert!(diagnostics.page_age_heatmap.buckets[0] >= 1);
    }

    #[test]
    fn rejects_owner_mismatch_before_page_state_mutation() {
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 1,
            ..PageSchedulerConfig::default()
        });
        scheduler.begin_frame(1);
        let mismatched = PageOwnerFrameRequests::new(
            PageOwner::VirtualGeometry,
            vec![PageRequest::new(
                PageOwner::VirtualShadows,
                7,
                4096,
                PagePriorityInputs::editor_critical(),
            )],
        );

        let report = scheduler.submit_owner_requests(&mismatched);
        assert_eq!(report.accepted, 0);
        assert_eq!(report.rejected_owner_mismatch, 1);
        assert_eq!(scheduler.page_count(), 0);
    }

    #[test]
    fn fault_eviction_benchmark_artifact_records_shared_owner_metrics() {
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 2,
            max_upload_bytes_per_frame: 1024 * 1024,
            fault_storm_threshold: 2,
            priority_weights: PagePriorityWeights::default(),
        });
        scheduler.begin_frame(7);
        scheduler.submit_owner_requests(&PageOwnerFrameRequests::new(
            PageOwner::VirtualGeometry,
            vec![request(PageOwner::VirtualGeometry, 1, 8192, 128)],
        ));
        scheduler.submit_owner_requests(&PageOwnerFrameRequests::new(
            PageOwner::VirtualShadows,
            vec![request(PageOwner::VirtualShadows, 2, 16_384, 255)],
        ));
        scheduler.schedule_uploads();
        assert!(scheduler.complete_upload(LogicalPageId::new(PageOwner::VirtualGeometry, 1)));
        assert!(scheduler.complete_upload(LogicalPageId::new(PageOwner::VirtualShadows, 2)));
        scheduler.select_eviction_candidates(1);
        scheduler.evict_candidates();

        let artifact = scheduler.debug_artifact();
        assert!(artifact.content.contains("fault_storm=true"));
        assert!(artifact.content.contains("virtual_geometry"));
        assert!(artifact.content.contains("virtual_shadows"));
        assert!(artifact.content.contains("evictions=1"));

        if let Some(path) = std::env::var_os(PAGE_SCHEDULER_BENCHMARK_ARTIFACT_ENV) {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent)
                    .expect("page scheduler artifact parent should be creatable");
            }
            std::fs::write(path, artifact.content.as_bytes())
                .expect("page scheduler benchmark artifact should be writable");
        }
    }
}
