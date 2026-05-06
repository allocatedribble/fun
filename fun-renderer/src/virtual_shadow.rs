use std::{collections::BTreeMap, fmt::Write as _, io, path::Path};

pub const VIRTUAL_SHADOW_SCHEMA_VERSION: u16 = 1;
pub const VIRTUAL_SHADOW_BENCHMARK_ARTIFACT_ENV: &str =
    "FUN_RENDERER_VIRTUAL_SHADOW_BENCHMARK_ARTIFACT";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowLightId(pub u64);

impl ShadowLightId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowReceiverId(pub u64);

impl ShadowReceiverId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowPageTableId(pub u64);

impl ShadowPageTableId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowVirtualPageId {
    pub table: ShadowPageTableId,
    pub page_index: u32,
}

impl ShadowVirtualPageId {
    #[must_use]
    pub const fn new(table: ShadowPageTableId, page_index: u32) -> Self {
        Self { table, page_index }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowPhysicalPageSlot {
    pub index: u32,
    pub generation: u32,
}

impl ShadowPhysicalPageSlot {
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
        if !self.is_valid() {
            return Self::INVALID;
        }
        let generation = if self.generation == u32::MAX {
            1
        } else {
            self.generation + 1
        };
        Self {
            index: self.index,
            generation,
        }
    }
}

impl Default for ShadowPhysicalPageSlot {
    fn default() -> Self {
        Self::INVALID
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowPageGeneration(pub u32);

impl ShadowPageGeneration {
    #[must_use]
    pub const fn next(self) -> Self {
        if self.0 == u32::MAX {
            Self(1)
        } else {
            Self(self.0 + 1)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShadowPageKind {
    DirectionalClipmap,
    LocalLight,
}

impl ShadowPageKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectionalClipmap => "directional_clipmap",
            Self::LocalLight => "local_light",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShadowPageState {
    #[default]
    Invalidated,
    Requested,
    Refreshing,
    Resident,
}

impl ShadowPageState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invalidated => "invalidated",
            Self::Requested => "requested",
            Self::Refreshing => "refreshing",
            Self::Resident => "resident",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShadowInvalidationReason {
    CameraMoved,
    ReceiverMoved,
    OccluderMoved,
    LightMoved,
    LightIntensityChanged,
    TimeOfDayChanged,
    GeometryPageChanged,
    ProceduralChunkInvalidated,
}

impl ShadowInvalidationReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CameraMoved => "camera_moved",
            Self::ReceiverMoved => "receiver_moved",
            Self::OccluderMoved => "occluder_moved",
            Self::LightMoved => "light_moved",
            Self::LightIntensityChanged => "light_intensity_changed",
            Self::TimeOfDayChanged => "time_of_day_changed",
            Self::GeometryPageChanged => "geometry_page_changed",
            Self::ProceduralChunkInvalidated => "procedural_chunk_invalidated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ShadowRefreshReason {
    VisibleReceiverDemand,
    CacheMiss,
    PolicyBudget,
    Forced,
}

impl ShadowRefreshReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VisibleReceiverDemand => "visible_receiver_demand",
            Self::CacheMiss => "cache_miss",
            Self::PolicyBudget => "policy_budget",
            Self::Forced => "forced",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShadowPagePriorityInputs {
    pub visible_receiver_demand: u16,
    pub screen_coverage: u16,
    pub contrast: u16,
    pub temporal_instability: u16,
    pub gameplay_salience: u16,
    pub editor_focus: u16,
    pub light_importance: u16,
}

impl ShadowPagePriorityInputs {
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            visible_receiver_demand: 0,
            screen_coverage: 0,
            contrast: 0,
            temporal_instability: 0,
            gameplay_salience: 0,
            editor_focus: 0,
            light_importance: 0,
        }
    }

    #[must_use]
    pub fn score(self) -> ShadowPagePriorityScore {
        ShadowPagePriorityScore::compute(self)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ShadowPagePriorityScore {
    pub value: u16,
    pub inputs: ShadowPagePriorityInputs,
}

impl ShadowPagePriorityScore {
    #[must_use]
    pub fn compute(inputs: ShadowPagePriorityInputs) -> Self {
        let value = u32::from(inputs.visible_receiver_demand) * 4
            + u32::from(inputs.screen_coverage) * 3
            + u32::from(inputs.contrast) * 2
            + u32::from(inputs.temporal_instability) * 2
            + u32::from(inputs.gameplay_salience) * 4
            + u32::from(inputs.editor_focus) * 5
            + u32::from(inputs.light_importance) * 4;
        Self {
            value: value.min(u32::from(u16::MAX)) as u16,
            inputs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowPageRequest {
    pub virtual_page: ShadowVirtualPageId,
    pub kind: ShadowPageKind,
    pub light_id: ShadowLightId,
    pub receiver_id: ShadowReceiverId,
    pub estimated_render_cost: u16,
    pub priority_inputs: ShadowPagePriorityInputs,
}

impl ShadowPageRequest {
    #[must_use]
    pub const fn directional(
        virtual_page: ShadowVirtualPageId,
        light_id: ShadowLightId,
        receiver_id: ShadowReceiverId,
        priority_inputs: ShadowPagePriorityInputs,
    ) -> Self {
        Self {
            virtual_page,
            kind: ShadowPageKind::DirectionalClipmap,
            light_id,
            receiver_id,
            estimated_render_cost: 1,
            priority_inputs,
        }
    }

    #[must_use]
    pub const fn local(
        virtual_page: ShadowVirtualPageId,
        light_id: ShadowLightId,
        receiver_id: ShadowReceiverId,
        priority_inputs: ShadowPagePriorityInputs,
    ) -> Self {
        Self {
            virtual_page,
            kind: ShadowPageKind::LocalLight,
            light_id,
            receiver_id,
            estimated_render_cost: 1,
            priority_inputs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowPageRecord {
    pub virtual_page: ShadowVirtualPageId,
    pub kind: ShadowPageKind,
    pub light_id: ShadowLightId,
    pub receiver_id: ShadowReceiverId,
    pub physical_slot: ShadowPhysicalPageSlot,
    pub state: ShadowPageState,
    pub generation: ShadowPageGeneration,
    pub priority: ShadowPagePriorityScore,
    pub last_refreshed_frame: Option<u64>,
    pub invalidation_reason: Option<ShadowInvalidationReason>,
}

impl ShadowPageRecord {
    #[must_use]
    pub fn from_request(request: &ShadowPageRequest, priority: ShadowPagePriorityScore) -> Self {
        Self {
            virtual_page: request.virtual_page,
            kind: request.kind,
            light_id: request.light_id,
            receiver_id: request.receiver_id,
            physical_slot: ShadowPhysicalPageSlot::INVALID,
            state: ShadowPageState::Requested,
            generation: ShadowPageGeneration::default(),
            priority,
            last_refreshed_frame: None,
            invalidation_reason: None,
        }
    }

    #[must_use]
    pub const fn is_refresh_candidate(self) -> bool {
        matches!(
            self.state,
            ShadowPageState::Requested | ShadowPageState::Invalidated
        )
    }

    #[must_use]
    pub const fn is_resident(self) -> bool {
        matches!(self.state, ShadowPageState::Resident) && self.physical_slot.is_valid()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowStorageConfig {
    pub directional_physical_pages: u32,
    pub local_physical_pages: u32,
    pub max_directional_refreshes_per_frame: u32,
    pub max_local_refreshes_per_frame: u32,
}

impl ShadowStorageConfig {
    pub const DEFAULT: Self = Self {
        directional_physical_pages: 256,
        local_physical_pages: 512,
        max_directional_refreshes_per_frame: 32,
        max_local_refreshes_per_frame: 64,
    };

    #[must_use]
    pub const fn stress_test(
        directional_physical_pages: u32,
        local_physical_pages: u32,
        max_directional_refreshes_per_frame: u32,
        max_local_refreshes_per_frame: u32,
    ) -> Self {
        Self {
            directional_physical_pages,
            local_physical_pages,
            max_directional_refreshes_per_frame,
            max_local_refreshes_per_frame,
        }
    }
}

impl Default for ShadowStorageConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShadowStorageDiagnostics {
    pub frame_index: u64,
    pub refreshed_pages: u32,
    pub resident_pages: u32,
    pub invalidated_pages: u32,
    pub shadow_page_misses: u32,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub cache_hit_ratio_per_mille: u16,
    pub per_light_page_budget: BTreeMap<ShadowLightId, u32>,
    pub directional_clipmap_pressure_per_mille: u16,
    pub local_light_pressure_per_mille: u16,
    pub directional_resident_pages: u32,
    pub local_resident_pages: u32,
    pub directional_budget_pages: u32,
    pub local_budget_pages: u32,
    pub directional_refresh_budget: u32,
    pub local_refresh_budget: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShadowRefreshReport {
    pub refreshed_pages: u32,
    pub deferred_pages: u32,
    pub shadow_page_misses: u32,
    pub cache_hits: u32,
    pub cache_misses: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowPageDebugArtifact {
    pub schema_version: u16,
    pub content: String,
}

impl ShadowPageDebugArtifact {
    #[must_use]
    pub fn from_storage(storage: &VirtualShadowStorage) -> Self {
        let diagnostics = storage.diagnostics();
        let mut content = String::new();
        let _ = writeln!(
            content,
            "virtual_shadow_schema_version={VIRTUAL_SHADOW_SCHEMA_VERSION}"
        );
        let _ = writeln!(content, "frame_index={}", diagnostics.frame_index);
        let _ = writeln!(content, "refreshed_pages={}", diagnostics.refreshed_pages);
        let _ = writeln!(content, "resident_pages={}", diagnostics.resident_pages);
        let _ = writeln!(
            content,
            "invalidated_pages={}",
            diagnostics.invalidated_pages
        );
        let _ = writeln!(
            content,
            "shadow_page_misses={}",
            diagnostics.shadow_page_misses
        );
        let _ = writeln!(content, "cache_hits={}", diagnostics.cache_hits);
        let _ = writeln!(content, "cache_misses={}", diagnostics.cache_misses);
        let _ = writeln!(
            content,
            "cache_hit_ratio_per_mille={}",
            diagnostics.cache_hit_ratio_per_mille
        );
        let _ = writeln!(
            content,
            "directional_clipmap_pressure_per_mille={}",
            diagnostics.directional_clipmap_pressure_per_mille
        );
        let _ = writeln!(
            content,
            "local_light_pressure_per_mille={}",
            diagnostics.local_light_pressure_per_mille
        );
        for (light_id, pages) in &diagnostics.per_light_page_budget {
            let _ = writeln!(content, "light_page_budget id={} pages={pages}", light_id.0);
        }
        for record in storage.records.values() {
            let reason = record
                .invalidation_reason
                .map_or("none", ShadowInvalidationReason::as_str);
            let _ = writeln!(
                content,
                "page table={} index={} kind={} state={} priority={} slot={} generation={} reason={}",
                record.virtual_page.table.0,
                record.virtual_page.page_index,
                record.kind.as_str(),
                record.state.as_str(),
                record.priority.value,
                record.physical_slot.index,
                record.generation.0,
                reason
            );
        }
        Self {
            schema_version: VIRTUAL_SHADOW_SCHEMA_VERSION,
            content,
        }
    }
}

pub fn write_shadow_page_artifact(
    path: impl AsRef<Path>,
    artifact: &ShadowPageDebugArtifact,
) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, &artifact.content)
}

#[derive(Debug, Clone)]
pub struct VirtualShadowStorage {
    config: ShadowStorageConfig,
    frame_index: u64,
    records: BTreeMap<ShadowVirtualPageId, ShadowPageRecord>,
    directional_free_slots: Vec<ShadowPhysicalPageSlot>,
    local_free_slots: Vec<ShadowPhysicalPageSlot>,
    diagnostics: ShadowStorageDiagnostics,
}

impl VirtualShadowStorage {
    #[must_use]
    pub fn new(config: ShadowStorageConfig) -> Self {
        let mut storage = Self {
            config,
            frame_index: 0,
            records: BTreeMap::new(),
            directional_free_slots: physical_page_pool(config.directional_physical_pages),
            local_free_slots: physical_page_pool(config.local_physical_pages),
            diagnostics: ShadowStorageDiagnostics {
                directional_budget_pages: config.directional_physical_pages,
                local_budget_pages: config.local_physical_pages,
                directional_refresh_budget: config.max_directional_refreshes_per_frame,
                local_refresh_budget: config.max_local_refreshes_per_frame,
                ..ShadowStorageDiagnostics::default()
            },
        };
        storage.recompute_residency_diagnostics();
        storage
    }

    #[must_use]
    pub fn config(&self) -> ShadowStorageConfig {
        self.config
    }

    #[must_use]
    pub fn records(&self) -> &BTreeMap<ShadowVirtualPageId, ShadowPageRecord> {
        &self.records
    }

    #[must_use]
    pub fn diagnostics(&self) -> &ShadowStorageDiagnostics {
        &self.diagnostics
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.diagnostics.frame_index = frame_index;
        self.diagnostics.refreshed_pages = 0;
        self.diagnostics.invalidated_pages = 0;
        self.diagnostics.shadow_page_misses = 0;
        self.diagnostics.cache_hits = 0;
        self.diagnostics.cache_misses = 0;
        self.diagnostics.cache_hit_ratio_per_mille = 0;
        self.recompute_residency_diagnostics();
    }

    pub fn submit_request(&mut self, request: ShadowPageRequest) {
        let priority = request.priority_inputs.score();
        match self.records.get_mut(&request.virtual_page) {
            Some(record) => {
                record.light_id = request.light_id;
                record.receiver_id = request.receiver_id;
                record.priority = priority;
                match record.state {
                    ShadowPageState::Resident if record.physical_slot.is_valid() => {
                        self.diagnostics.cache_hits = self.diagnostics.cache_hits.saturating_add(1);
                    }
                    ShadowPageState::Refreshing => {
                        self.diagnostics.cache_misses =
                            self.diagnostics.cache_misses.saturating_add(1);
                    }
                    ShadowPageState::Requested | ShadowPageState::Invalidated => {
                        self.diagnostics.cache_misses =
                            self.diagnostics.cache_misses.saturating_add(1);
                        record.state = ShadowPageState::Requested;
                    }
                    ShadowPageState::Resident => {
                        self.diagnostics.cache_misses =
                            self.diagnostics.cache_misses.saturating_add(1);
                        record.state = ShadowPageState::Requested;
                    }
                }
            }
            None => {
                self.records.insert(
                    request.virtual_page,
                    ShadowPageRecord::from_request(&request, priority),
                );
                self.diagnostics.shadow_page_misses =
                    self.diagnostics.shadow_page_misses.saturating_add(1);
                self.diagnostics.cache_misses = self.diagnostics.cache_misses.saturating_add(1);
            }
        }
        self.update_cache_ratio();
        self.recompute_residency_diagnostics();
    }

    pub fn invalidate_by_light(
        &mut self,
        light_id: ShadowLightId,
        reason: ShadowInvalidationReason,
    ) -> u32 {
        let mut invalidated = 0_u32;
        for record in self.records.values_mut() {
            if record.light_id == light_id && record.state != ShadowPageState::Invalidated {
                record.state = ShadowPageState::Invalidated;
                record.invalidation_reason = Some(reason);
                invalidated = invalidated.saturating_add(1);
            }
        }
        self.diagnostics.invalidated_pages = self
            .diagnostics
            .invalidated_pages
            .saturating_add(invalidated);
        self.recompute_residency_diagnostics();
        invalidated
    }

    pub fn invalidate_by_receiver(
        &mut self,
        receiver_id: ShadowReceiverId,
        reason: ShadowInvalidationReason,
    ) -> u32 {
        let mut invalidated = 0_u32;
        for record in self.records.values_mut() {
            if record.receiver_id == receiver_id && record.state != ShadowPageState::Invalidated {
                record.state = ShadowPageState::Invalidated;
                record.invalidation_reason = Some(reason);
                invalidated = invalidated.saturating_add(1);
            }
        }
        self.diagnostics.invalidated_pages = self
            .diagnostics
            .invalidated_pages
            .saturating_add(invalidated);
        self.recompute_residency_diagnostics();
        invalidated
    }

    pub fn invalidate_by_kind(
        &mut self,
        kind: ShadowPageKind,
        reason: ShadowInvalidationReason,
    ) -> u32 {
        let mut invalidated = 0_u32;
        for record in self.records.values_mut() {
            if record.kind == kind && record.state != ShadowPageState::Invalidated {
                record.state = ShadowPageState::Invalidated;
                record.invalidation_reason = Some(reason);
                invalidated = invalidated.saturating_add(1);
            }
        }
        self.diagnostics.invalidated_pages = self
            .diagnostics
            .invalidated_pages
            .saturating_add(invalidated);
        self.recompute_residency_diagnostics();
        invalidated
    }

    #[must_use]
    pub fn refresh_requested_pages(&mut self) -> ShadowRefreshReport {
        let mut candidates: Vec<PendingShadowPage> = self
            .records
            .iter()
            .filter_map(|(id, record)| {
                record
                    .is_refresh_candidate()
                    .then_some(PendingShadowPage::from_record(*id, *record))
            })
            .collect();
        candidates.sort_by(PendingShadowPage::compare_refresh_order);

        let mut report = ShadowRefreshReport::default();
        let mut directional_refreshes = 0_u32;
        let mut local_refreshes = 0_u32;

        for candidate in candidates {
            match candidate.kind {
                ShadowPageKind::DirectionalClipmap
                    if directional_refreshes >= self.config.max_directional_refreshes_per_frame =>
                {
                    report.deferred_pages = report.deferred_pages.saturating_add(1);
                    continue;
                }
                ShadowPageKind::LocalLight
                    if local_refreshes >= self.config.max_local_refreshes_per_frame =>
                {
                    report.deferred_pages = report.deferred_pages.saturating_add(1);
                    continue;
                }
                _ => {}
            }

            let needs_slot = self
                .records
                .get(&candidate.virtual_page)
                .is_some_and(|record| !record.physical_slot.is_valid());
            let allocated_slot = if needs_slot {
                self.allocate_slot(candidate.kind)
            } else {
                None
            };
            if needs_slot && allocated_slot.is_none() {
                report.deferred_pages = report.deferred_pages.saturating_add(1);
                report.shadow_page_misses = report.shadow_page_misses.saturating_add(1);
                self.diagnostics.shadow_page_misses =
                    self.diagnostics.shadow_page_misses.saturating_add(1);
                continue;
            }

            if let Some(record) = self.records.get_mut(&candidate.virtual_page) {
                if let Some(slot) = allocated_slot {
                    record.physical_slot = slot;
                } else {
                    record.physical_slot = record.physical_slot.next_generation();
                }
                record.state = ShadowPageState::Refreshing;
                record.generation = record.generation.next();
                record.last_refreshed_frame = Some(self.frame_index);
                record.invalidation_reason = None;
                record.state = ShadowPageState::Resident;
                report.refreshed_pages = report.refreshed_pages.saturating_add(1);
                self.diagnostics.refreshed_pages =
                    self.diagnostics.refreshed_pages.saturating_add(1);
                match record.kind {
                    ShadowPageKind::DirectionalClipmap => {
                        directional_refreshes = directional_refreshes.saturating_add(1);
                    }
                    ShadowPageKind::LocalLight => {
                        local_refreshes = local_refreshes.saturating_add(1);
                    }
                }
            }
        }

        report.cache_hits = self.diagnostics.cache_hits;
        report.cache_misses = self.diagnostics.cache_misses;
        self.recompute_residency_diagnostics();
        report
    }

    fn allocate_slot(&mut self, kind: ShadowPageKind) -> Option<ShadowPhysicalPageSlot> {
        match kind {
            ShadowPageKind::DirectionalClipmap => self.directional_free_slots.pop(),
            ShadowPageKind::LocalLight => self.local_free_slots.pop(),
        }
    }

    fn recompute_residency_diagnostics(&mut self) {
        self.diagnostics.per_light_page_budget.clear();
        self.diagnostics.directional_resident_pages = 0;
        self.diagnostics.local_resident_pages = 0;
        for record in self.records.values() {
            if !record.is_resident() {
                continue;
            }
            *self
                .diagnostics
                .per_light_page_budget
                .entry(record.light_id)
                .or_default() += 1;
            match record.kind {
                ShadowPageKind::DirectionalClipmap => {
                    self.diagnostics.directional_resident_pages = self
                        .diagnostics
                        .directional_resident_pages
                        .saturating_add(1);
                }
                ShadowPageKind::LocalLight => {
                    self.diagnostics.local_resident_pages =
                        self.diagnostics.local_resident_pages.saturating_add(1);
                }
            }
        }
        self.diagnostics.resident_pages = self
            .diagnostics
            .directional_resident_pages
            .saturating_add(self.diagnostics.local_resident_pages);
        self.diagnostics.directional_budget_pages = self.config.directional_physical_pages;
        self.diagnostics.local_budget_pages = self.config.local_physical_pages;
        self.diagnostics.directional_refresh_budget =
            self.config.max_directional_refreshes_per_frame;
        self.diagnostics.local_refresh_budget = self.config.max_local_refreshes_per_frame;
        self.diagnostics.directional_clipmap_pressure_per_mille = pressure_per_mille(
            self.diagnostics.directional_resident_pages,
            self.config.directional_physical_pages,
        );
        self.diagnostics.local_light_pressure_per_mille = pressure_per_mille(
            self.diagnostics.local_resident_pages,
            self.config.local_physical_pages,
        );
        self.update_cache_ratio();
    }

    fn update_cache_ratio(&mut self) {
        let total = self
            .diagnostics
            .cache_hits
            .saturating_add(self.diagnostics.cache_misses);
        self.diagnostics.cache_hit_ratio_per_mille = if total == 0 {
            0
        } else {
            ((u64::from(self.diagnostics.cache_hits) * 1000) / u64::from(total)) as u16
        };
    }
}

impl Default for VirtualShadowStorage {
    fn default() -> Self {
        Self::new(ShadowStorageConfig::DEFAULT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingShadowPage {
    virtual_page: ShadowVirtualPageId,
    kind: ShadowPageKind,
    priority: ShadowPagePriorityScore,
}

impl PendingShadowPage {
    const fn from_record(virtual_page: ShadowVirtualPageId, record: ShadowPageRecord) -> Self {
        Self {
            virtual_page,
            kind: record.kind,
            priority: record.priority,
        }
    }

    fn compare_refresh_order(left: &Self, right: &Self) -> std::cmp::Ordering {
        right
            .priority
            .value
            .cmp(&left.priority.value)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.virtual_page.cmp(&right.virtual_page))
    }
}

fn physical_page_pool(count: u32) -> Vec<ShadowPhysicalPageSlot> {
    (0..count)
        .rev()
        .map(|index| ShadowPhysicalPageSlot::new(index, 1))
        .collect()
}

fn pressure_per_mille(used: u32, capacity: u32) -> u16 {
    if capacity == 0 {
        return if used > 0 { 1000 } else { 0 };
    }
    ((u64::from(used) * 1000) / u64::from(capacity)).min(1000) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(index: u32) -> ShadowVirtualPageId {
        ShadowVirtualPageId::new(ShadowPageTableId::new(7), index)
    }

    fn priority(
        visible_receiver_demand: u16,
        screen_coverage: u16,
        light_importance: u16,
    ) -> ShadowPagePriorityInputs {
        ShadowPagePriorityInputs {
            visible_receiver_demand,
            screen_coverage,
            contrast: 24,
            temporal_instability: 12,
            gameplay_salience: 0,
            editor_focus: 0,
            light_importance,
        }
    }

    #[test]
    fn directional_light_stress_scene_refreshes_receiver_demand_pages() {
        let mut storage = VirtualShadowStorage::new(ShadowStorageConfig::stress_test(16, 0, 4, 0));
        storage.begin_frame(100);

        for page_index in 0..12 {
            storage.submit_request(ShadowPageRequest::directional(
                page(page_index),
                ShadowLightId::new(1),
                ShadowReceiverId::new(u64::from(page_index) + 100),
                priority(page_index as u16 * 10, 80, 120),
            ));
        }

        let report = storage.refresh_requested_pages();

        assert_eq!(report.refreshed_pages, 4);
        assert_eq!(report.deferred_pages, 8);
        assert_eq!(storage.diagnostics().directional_resident_pages, 4);
        assert_eq!(storage.diagnostics().local_resident_pages, 0);
        assert!(storage.diagnostics().directional_clipmap_pressure_per_mille > 0);
    }

    #[test]
    fn local_light_stress_scene_uses_sparse_budget_not_linear_atlas() {
        let mut storage = VirtualShadowStorage::new(ShadowStorageConfig::stress_test(0, 8, 0, 3));
        storage.begin_frame(200);

        for page_index in 0..24 {
            storage.submit_request(ShadowPageRequest::local(
                page(page_index),
                ShadowLightId::new(u64::from(page_index) + 1),
                ShadowReceiverId::new(9),
                priority(200_u16.saturating_sub(page_index as u16), 60, 20),
            ));
        }

        let report = storage.refresh_requested_pages();

        assert_eq!(report.refreshed_pages, 3);
        assert_eq!(report.deferred_pages, 21);
        assert_eq!(storage.diagnostics().local_resident_pages, 3);
        assert_eq!(storage.diagnostics().per_light_page_budget.len(), 3);
        assert!(storage.diagnostics().local_light_pressure_per_mille <= 1000);
    }

    #[test]
    fn invalidation_reasons_mark_pages_without_policy_logic() {
        let mut storage = VirtualShadowStorage::new(ShadowStorageConfig::stress_test(4, 4, 4, 4));
        storage.begin_frame(1);
        storage.submit_request(ShadowPageRequest::directional(
            page(1),
            ShadowLightId::new(44),
            ShadowReceiverId::new(2),
            priority(100, 100, 100),
        ));
        assert_eq!(storage.refresh_requested_pages().refreshed_pages, 1);

        let invalidated = storage
            .invalidate_by_light(ShadowLightId::new(44), ShadowInvalidationReason::LightMoved);

        let record = storage.records().get(&page(1)).expect("page should exist");
        assert_eq!(invalidated, 1);
        assert_eq!(record.state, ShadowPageState::Invalidated);
        assert_eq!(
            record.invalidation_reason,
            Some(ShadowInvalidationReason::LightMoved)
        );

        storage.submit_request(ShadowPageRequest::directional(
            page(1),
            ShadowLightId::new(44),
            ShadowReceiverId::new(2),
            priority(100, 100, 100),
        ));
        assert_eq!(storage.refresh_requested_pages().refreshed_pages, 1);
        assert_eq!(
            storage
                .records()
                .get(&page(1))
                .and_then(|record| record.invalidation_reason),
            None
        );
    }

    #[test]
    fn cache_hits_and_misses_are_measured() {
        let mut storage = VirtualShadowStorage::new(ShadowStorageConfig::stress_test(4, 4, 4, 4));
        storage.begin_frame(1);
        storage.submit_request(ShadowPageRequest::local(
            page(3),
            ShadowLightId::new(5),
            ShadowReceiverId::new(6),
            priority(100, 20, 10),
        ));
        assert_eq!(storage.diagnostics().cache_misses, 1);
        assert_eq!(storage.refresh_requested_pages().refreshed_pages, 1);

        storage.begin_frame(2);
        storage.submit_request(ShadowPageRequest::local(
            page(3),
            ShadowLightId::new(5),
            ShadowReceiverId::new(6),
            priority(100, 20, 10),
        ));

        assert_eq!(storage.diagnostics().cache_hits, 1);
        assert_eq!(storage.diagnostics().cache_misses, 0);
        assert_eq!(storage.diagnostics().cache_hit_ratio_per_mille, 1000);
    }

    #[test]
    fn shadow_page_artifact_records_directional_and_local_pressure() {
        let mut storage = VirtualShadowStorage::new(ShadowStorageConfig::stress_test(4, 4, 4, 4));
        storage.begin_frame(300);
        storage.submit_request(ShadowPageRequest::directional(
            page(10),
            ShadowLightId::new(1),
            ShadowReceiverId::new(1),
            priority(120, 90, 200),
        ));
        storage.submit_request(ShadowPageRequest::local(
            page(11),
            ShadowLightId::new(2),
            ShadowReceiverId::new(2),
            priority(100, 80, 100),
        ));
        let report = storage.refresh_requested_pages();
        assert_eq!(report.refreshed_pages, 2);

        let artifact = ShadowPageDebugArtifact::from_storage(&storage);
        assert_eq!(artifact.schema_version, VIRTUAL_SHADOW_SCHEMA_VERSION);
        assert!(
            artifact
                .content
                .contains("directional_clipmap_pressure_per_mille=")
        );
        assert!(artifact.content.contains("local_light_pressure_per_mille="));
        assert!(artifact.content.contains("cache_hit_ratio_per_mille="));
        assert!(artifact.content.contains("kind=directional_clipmap"));
        assert!(artifact.content.contains("kind=local_light"));

        if let Ok(path) = std::env::var(VIRTUAL_SHADOW_BENCHMARK_ARTIFACT_ENV) {
            write_shadow_page_artifact(path, &artifact)
                .expect("virtual shadow benchmark artifact should be writable");
        }
    }
}
