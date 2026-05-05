use crate::{
    FunUploadBudgetDecision, FunUploadBudgetTracker, FunUploadSubsystem, FunUploadWriteIntent,
    InstanceDirtyRange, InstanceRange, UPLOAD_CEF_CPU_DIRTY_RECT, UPLOAD_CEF_CPU_FULL_FRAME,
    UPLOAD_INSTANCE_DIRTY_RANGE, UploadWriteLabel,
};

pub const FUN_UPLOAD_RANGE_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_STATIC_SLAB_RECORDS: u32 = 1024;
pub const DEFAULT_DYNAMIC_UPLOAD_SLAB_BYTES: u64 = 256 * 1024;
pub const DEFAULT_TEXTURE_DIRTY_RECT_CAP: u32 = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PersistentBufferGrowthPolicy {
    #[default]
    PowerOfTwo,
    FixedSlab {
        slab_records: u32,
    },
}

impl PersistentBufferGrowthPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PowerOfTwo => "power_of_two",
            Self::FixedSlab { .. } => "fixed_slab",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistentBufferCapacityPlan {
    pub previous_capacity_records: u32,
    pub required_records: u32,
    pub next_capacity_records: u32,
    pub grew: bool,
    pub never_shrink: bool,
}

pub fn persistent_buffer_capacity_plan(
    previous_capacity_records: u32,
    required_records: u32,
    policy: PersistentBufferGrowthPolicy,
) -> PersistentBufferCapacityPlan {
    let next_capacity_records = match policy {
        PersistentBufferGrowthPolicy::PowerOfTwo => {
            if required_records <= previous_capacity_records {
                previous_capacity_records
            } else {
                required_records.next_power_of_two()
            }
        }
        PersistentBufferGrowthPolicy::FixedSlab { slab_records } => {
            let slab_records = slab_records.max(1);
            if required_records <= previous_capacity_records {
                previous_capacity_records
            } else {
                required_records.div_ceil(slab_records) * slab_records
            }
        }
    };
    PersistentBufferCapacityPlan {
        previous_capacity_records,
        required_records,
        next_capacity_records,
        grew: next_capacity_records > previous_capacity_records,
        never_shrink: next_capacity_records >= previous_capacity_records,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistentBufferUploadPath {
    Noop,
    RangeUpdate,
    FullBufferOnCreateOrResize,
    Defer,
    Reject,
}

impl PersistentBufferUploadPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Noop => "noop",
            Self::RangeUpdate => "range_update",
            Self::FullBufferOnCreateOrResize => "full_buffer_on_create_or_resize",
            Self::Defer => "defer",
            Self::Reject => "reject",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistentBufferRangeUploadPlan {
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub range: InstanceRange,
    pub bytes: u64,
    pub buffer_size: u64,
    pub buffer_created_or_resized: bool,
    pub budget_decision: FunUploadBudgetDecision,
    pub path: PersistentBufferUploadPath,
    pub use_upload_arena: bool,
}

pub fn plan_persistent_buffer_range_upload(
    budget: &mut FunUploadBudgetTracker,
    label: UploadWriteLabel,
    subsystem: FunUploadSubsystem,
    range: InstanceRange,
    bytes: u64,
    buffer_size: u64,
    buffer_created_or_resized: bool,
) -> PersistentBufferRangeUploadPlan {
    if bytes == 0 || range.is_empty() {
        return PersistentBufferRangeUploadPlan {
            label,
            subsystem,
            range,
            bytes,
            buffer_size,
            buffer_created_or_resized,
            budget_decision: FunUploadBudgetDecision::Admit,
            path: PersistentBufferUploadPath::Noop,
            use_upload_arena: false,
        };
    }
    let decision = budget.decide(FunUploadWriteIntent::large_buffer(
        label,
        subsystem,
        bytes,
        range.byte_offset(),
        buffer_size,
        bytes,
        buffer_created_or_resized,
    ));
    let path = match decision {
        FunUploadBudgetDecision::Admit if buffer_created_or_resized && bytes == buffer_size => {
            PersistentBufferUploadPath::FullBufferOnCreateOrResize
        }
        FunUploadBudgetDecision::Admit => PersistentBufferUploadPath::RangeUpdate,
        FunUploadBudgetDecision::Defer => PersistentBufferUploadPath::Defer,
        FunUploadBudgetDecision::RejectOversized | FunUploadBudgetDecision::RejectUnaligned => {
            PersistentBufferUploadPath::Reject
        }
        FunUploadBudgetDecision::FallbackRawWrite => PersistentBufferUploadPath::RangeUpdate,
    };
    PersistentBufferRangeUploadPlan {
        label,
        subsystem,
        range,
        bytes,
        buffer_size,
        buffer_created_or_resized,
        budget_decision: decision,
        path,
        use_upload_arena: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicUploadSlabAllocation {
    pub frame_index: u64,
    pub ring_slot: u8,
    pub offset_bytes: u64,
    pub bytes: u64,
    pub slab_capacity_bytes: u64,
    pub grew: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicRingUploadSlabs<const N: usize> {
    default_slab_bytes: u64,
    capacities: [u64; N],
    used: [u64; N],
}

impl<const N: usize> DynamicRingUploadSlabs<N> {
    pub fn new(default_slab_bytes: u64) -> Self {
        Self {
            default_slab_bytes,
            capacities: [0; N],
            used: [0; N],
        }
    }

    pub fn begin_frame(&mut self, frame_index: u64) -> u8 {
        let slot = ring_slot::<N>(frame_index);
        self.used[slot as usize] = 0;
        slot
    }

    pub fn allocate(&mut self, frame_index: u64, bytes: u64) -> DynamicUploadSlabAllocation {
        let slot = ring_slot::<N>(frame_index);
        let slot_index = slot as usize;
        let offset_bytes = self.used[slot_index];
        let required = offset_bytes.saturating_add(bytes);
        let previous = self.capacities[slot_index];
        let mut grew = false;
        if required > previous {
            self.capacities[slot_index] = self.default_slab_bytes.max(required).next_power_of_two();
            grew = self.capacities[slot_index] > previous;
        }
        self.used[slot_index] = required;
        DynamicUploadSlabAllocation {
            frame_index,
            ring_slot: slot,
            offset_bytes,
            bytes,
            slab_capacity_bytes: self.capacities[slot_index],
            grew,
        }
    }

    pub const fn capacity_bytes(&self, slot: u8) -> u64 {
        self.capacities[slot as usize]
    }

    pub const fn used_bytes(&self, slot: u8) -> u64 {
        self.used[slot as usize]
    }
}

fn ring_slot<const N: usize>(frame_index: u64) -> u8 {
    debug_assert!(N > 0);
    (frame_index as usize % N.max(1)) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstanceSoAField {
    Transform,
    MaterialParams,
    Bounds,
    AnimationSkinning,
}

impl InstanceSoAField {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transform => "transform",
            Self::MaterialParams => "material_params",
            Self::Bounds => "bounds",
            Self::AnimationSkinning => "animation_skinning",
        }
    }

    pub const fn expected_update_frequency(self) -> &'static str {
        match self {
            Self::Transform => "per_dynamic_motion",
            Self::MaterialParams => "per_material_variant",
            Self::Bounds => "per_lod_or_residency",
            Self::AnimationSkinning => "per_animation_frame",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceSoADirtyRange {
    pub field: InstanceSoAField,
    pub dirty: InstanceDirtyRange,
}

pub fn merge_instance_dirty_ranges(mut ranges: Vec<InstanceDirtyRange>) -> Vec<InstanceDirtyRange> {
    ranges.sort_by_key(|dirty| (dirty.range.start, dirty.reason));
    let mut merged = Vec::<InstanceDirtyRange>::with_capacity(ranges.len());
    for dirty in ranges {
        if dirty.range.is_empty() {
            continue;
        }
        if let Some(last) = merged.last_mut()
            && last.reason == dirty.reason
            && last.range.end() >= dirty.range.start
        {
            let end = last.range.end().max(dirty.range.end());
            last.range.count = end.saturating_sub(last.range.start);
            last.byte_offset = last.range.byte_offset();
            last.byte_len = last.range.byte_len();
            continue;
        }
        merged.push(dirty);
    }
    merged
}

pub fn plan_instance_soa_uploads(
    field: InstanceSoAField,
    ranges: Vec<InstanceDirtyRange>,
) -> Vec<InstanceSoADirtyRange> {
    merge_instance_dirty_ranges(ranges)
        .into_iter()
        .map(|dirty| InstanceSoADirtyRange { field, dirty })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextureDirtyRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl TextureDirtyRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub const fn bytes(self, bytes_per_pixel: u32) -> u64 {
        self.width as u64 * self.height as u64 * bytes_per_pixel as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureUploadPolicy {
    pub max_dirty_rects_per_frame: u32,
    pub bytes_per_pixel: u32,
    pub full_frame_allowed_on_resize: bool,
    pub allow_full_frame_when_dirty_rects_explode: bool,
}

impl Default for TextureUploadPolicy {
    fn default() -> Self {
        Self {
            max_dirty_rects_per_frame: DEFAULT_TEXTURE_DIRTY_RECT_CAP,
            bytes_per_pixel: 4,
            full_frame_allowed_on_resize: true,
            allow_full_frame_when_dirty_rects_explode: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureUploadPath {
    NoopUnchanged,
    DirtyRects,
    FullFrameOnResize,
    Defer,
    RejectFullFrame,
}

impl TextureUploadPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoopUnchanged => "noop_unchanged",
            Self::DirtyRects => "dirty_rects",
            Self::FullFrameOnResize => "full_frame_on_resize",
            Self::Defer => "defer",
            Self::RejectFullFrame => "reject_full_frame",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureUploadPlan {
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub path: TextureUploadPath,
    pub uploaded_rects: Vec<TextureDirtyRect>,
    pub deferred_rects: Vec<TextureDirtyRect>,
    pub bytes: u64,
    pub budget_decision: FunUploadBudgetDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureUploadRequest<'a> {
    pub label: UploadWriteLabel,
    pub subsystem: FunUploadSubsystem,
    pub texture_width: u32,
    pub texture_height: u32,
    pub dirty_rects: &'a [TextureDirtyRect],
    pub resized: bool,
    pub policy: TextureUploadPolicy,
}

pub fn plan_dynamic_texture_upload(
    budget: &mut FunUploadBudgetTracker,
    request: TextureUploadRequest<'_>,
) -> TextureUploadPlan {
    if request.resized && request.policy.full_frame_allowed_on_resize {
        let bytes = request.texture_width as u64
            * request.texture_height as u64
            * request.policy.bytes_per_pixel as u64;
        let decision = budget.decide(FunUploadWriteIntent::texture(
            request.label,
            request.subsystem,
            bytes,
        ));
        return TextureUploadPlan {
            label: request.label,
            subsystem: request.subsystem,
            path: path_for_texture_decision(decision, TextureUploadPath::FullFrameOnResize),
            uploaded_rects: vec![TextureDirtyRect::new(
                0,
                0,
                request.texture_width,
                request.texture_height,
            )],
            deferred_rects: Vec::new(),
            bytes: moved_bytes_for_upload_decision(bytes, decision),
            budget_decision: decision,
        };
    }

    let clean_rects = request
        .dirty_rects
        .iter()
        .copied()
        .filter(|rect| !rect.is_empty())
        .collect::<Vec<_>>();
    if clean_rects.is_empty() {
        return TextureUploadPlan {
            label: request.label,
            subsystem: request.subsystem,
            path: TextureUploadPath::NoopUnchanged,
            uploaded_rects: Vec::new(),
            deferred_rects: Vec::new(),
            bytes: 0,
            budget_decision: FunUploadBudgetDecision::Admit,
        };
    }

    if clean_rects.len() as u32 > request.policy.max_dirty_rects_per_frame
        && !request.policy.allow_full_frame_when_dirty_rects_explode
    {
        let uploaded_rects = clean_rects
            .iter()
            .copied()
            .take(request.policy.max_dirty_rects_per_frame as usize)
            .collect::<Vec<_>>();
        let deferred_rects = clean_rects
            .iter()
            .copied()
            .skip(request.policy.max_dirty_rects_per_frame as usize)
            .collect::<Vec<_>>();
        let bytes = uploaded_rects
            .iter()
            .map(|rect| rect.bytes(request.policy.bytes_per_pixel))
            .sum::<u64>();
        let decision = budget.decide(FunUploadWriteIntent::texture(
            request.label,
            request.subsystem,
            bytes,
        ));
        return TextureUploadPlan {
            label: request.label,
            subsystem: request.subsystem,
            path: path_for_texture_decision(decision, TextureUploadPath::DirtyRects),
            uploaded_rects,
            deferred_rects,
            bytes: moved_bytes_for_upload_decision(bytes, decision),
            budget_decision: decision,
        };
    }

    let bytes = clean_rects
        .iter()
        .map(|rect| rect.bytes(request.policy.bytes_per_pixel))
        .sum::<u64>();
    let decision = budget.decide(FunUploadWriteIntent::texture(
        request.label,
        request.subsystem,
        bytes,
    ));
    TextureUploadPlan {
        label: request.label,
        subsystem: request.subsystem,
        path: path_for_texture_decision(decision, TextureUploadPath::DirtyRects),
        uploaded_rects: clean_rects,
        deferred_rects: Vec::new(),
        bytes: moved_bytes_for_upload_decision(bytes, decision),
        budget_decision: decision,
    }
}

pub fn plan_cef_cpu_dirty_rect_upload(
    budget: &mut FunUploadBudgetTracker,
    texture_width: u32,
    texture_height: u32,
    dirty_rects: &[TextureDirtyRect],
    resized: bool,
    page_changed: bool,
    mut policy: TextureUploadPolicy,
) -> TextureUploadPlan {
    policy.allow_full_frame_when_dirty_rects_explode = false;
    if !page_changed && !resized {
        return TextureUploadPlan {
            label: UPLOAD_CEF_CPU_DIRTY_RECT,
            subsystem: FunUploadSubsystem::CefCpuPaint,
            path: TextureUploadPath::NoopUnchanged,
            uploaded_rects: Vec::new(),
            deferred_rects: dirty_rects.to_vec(),
            bytes: 0,
            budget_decision: FunUploadBudgetDecision::Admit,
        };
    }
    if resized {
        return plan_dynamic_texture_upload(
            budget,
            TextureUploadRequest {
                label: UPLOAD_CEF_CPU_FULL_FRAME,
                subsystem: FunUploadSubsystem::CefCpuPaint,
                texture_width,
                texture_height,
                dirty_rects,
                resized,
                policy,
            },
        );
    }
    plan_cef_dirty_rects_only(budget, dirty_rects, policy)
}

fn plan_cef_dirty_rects_only(
    budget: &mut FunUploadBudgetTracker,
    dirty_rects: &[TextureDirtyRect],
    policy: TextureUploadPolicy,
) -> TextureUploadPlan {
    let clean_rects = dirty_rects
        .iter()
        .copied()
        .filter(|rect| !rect.is_empty())
        .collect::<Vec<_>>();
    if clean_rects.is_empty() {
        return TextureUploadPlan {
            label: UPLOAD_CEF_CPU_DIRTY_RECT,
            subsystem: FunUploadSubsystem::CefCpuPaint,
            path: TextureUploadPath::NoopUnchanged,
            uploaded_rects: Vec::new(),
            deferred_rects: Vec::new(),
            bytes: 0,
            budget_decision: FunUploadBudgetDecision::Admit,
        };
    }
    let uploaded_rects = clean_rects
        .iter()
        .copied()
        .take(policy.max_dirty_rects_per_frame as usize)
        .collect::<Vec<_>>();
    let deferred_rects = clean_rects
        .iter()
        .copied()
        .skip(policy.max_dirty_rects_per_frame as usize)
        .collect::<Vec<_>>();
    let bytes = uploaded_rects
        .iter()
        .map(|rect| rect.bytes(policy.bytes_per_pixel))
        .sum::<u64>();
    let decision = budget.decide(FunUploadWriteIntent::cef_cpu(
        UPLOAD_CEF_CPU_DIRTY_RECT,
        bytes,
    ));
    TextureUploadPlan {
        label: UPLOAD_CEF_CPU_DIRTY_RECT,
        subsystem: FunUploadSubsystem::CefCpuPaint,
        path: path_for_texture_decision(decision, TextureUploadPath::DirtyRects),
        uploaded_rects,
        deferred_rects,
        bytes: moved_bytes_for_upload_decision(bytes, decision),
        budget_decision: decision,
    }
}

const fn path_for_texture_decision(
    decision: FunUploadBudgetDecision,
    admitted_path: TextureUploadPath,
) -> TextureUploadPath {
    match decision {
        FunUploadBudgetDecision::Admit | FunUploadBudgetDecision::FallbackRawWrite => admitted_path,
        FunUploadBudgetDecision::Defer => TextureUploadPath::Defer,
        FunUploadBudgetDecision::RejectOversized | FunUploadBudgetDecision::RejectUnaligned => {
            TextureUploadPath::RejectFullFrame
        }
    }
}

const fn moved_bytes_for_upload_decision(bytes: u64, decision: FunUploadBudgetDecision) -> u64 {
    match decision {
        FunUploadBudgetDecision::Admit | FunUploadBudgetDecision::FallbackRawWrite => bytes,
        FunUploadBudgetDecision::Defer
        | FunUploadBudgetDecision::RejectOversized
        | FunUploadBudgetDecision::RejectUnaligned => 0,
    }
}

pub const fn instance_dirty_range_intent(bytes: u64, offset: u64) -> FunUploadWriteIntent {
    FunUploadWriteIntent::large_buffer(
        UPLOAD_INSTANCE_DIRTY_RANGE,
        FunUploadSubsystem::MeshletInstance,
        bytes,
        offset,
        0,
        bytes,
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FunUploadBudget, InstanceDirtyReason, UPLOAD_TEXTURE_DIRTY_RECT};

    #[test]
    fn static_buffer_capacity_grows_power_of_two_and_never_shrinks() {
        let grown =
            persistent_buffer_capacity_plan(8, 17, PersistentBufferGrowthPolicy::PowerOfTwo);
        let smaller = persistent_buffer_capacity_plan(
            grown.next_capacity_records,
            9,
            PersistentBufferGrowthPolicy::PowerOfTwo,
        );

        assert_eq!(grown.next_capacity_records, 32);
        assert!(grown.grew);
        assert_eq!(smaller.next_capacity_records, 32);
        assert!(smaller.never_shrink);
    }

    #[test]
    fn static_buffer_capacity_can_use_fixed_slabs() {
        let plan = persistent_buffer_capacity_plan(
            0,
            1_025,
            PersistentBufferGrowthPolicy::FixedSlab {
                slab_records: DEFAULT_STATIC_SLAB_RECORDS,
            },
        );

        assert_eq!(plan.next_capacity_records, 2_048);
    }

    #[test]
    fn dynamic_ring_slabs_reuse_slot_without_shrinking() {
        let mut slabs = DynamicRingUploadSlabs::<3>::new(DEFAULT_DYNAMIC_UPLOAD_SLAB_BYTES);
        slabs.begin_frame(0);
        let first = slabs.allocate(0, 32 * 1024);
        slabs.begin_frame(3);
        let second = slabs.allocate(3, 16 * 1024);

        assert_eq!(first.ring_slot, second.ring_slot);
        assert_eq!(second.offset_bytes, 0);
        assert_eq!(second.slab_capacity_bytes, first.slab_capacity_bytes);
        assert!(!second.grew);
    }

    #[test]
    fn instance_dirty_ranges_merge_adjacent_ranges() {
        let ranges = vec![
            InstanceDirtyRange::new(
                InstanceRange::new(4, 2),
                InstanceDirtyReason::DynamicTransformChanged,
            ),
            InstanceDirtyRange::new(
                InstanceRange::new(6, 3),
                InstanceDirtyReason::DynamicTransformChanged,
            ),
            InstanceDirtyRange::new(
                InstanceRange::new(20, 1),
                InstanceDirtyReason::MaterialVariantChanged,
            ),
        ];

        let merged = merge_instance_dirty_ranges(ranges);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].range, InstanceRange::new(4, 5));
        assert_eq!(merged[1].range, InstanceRange::new(20, 1));
    }

    #[test]
    fn soa_uploads_keep_field_frequency_separate() {
        let uploads = plan_instance_soa_uploads(
            InstanceSoAField::MaterialParams,
            vec![InstanceDirtyRange::new(
                InstanceRange::new(1, 1),
                InstanceDirtyReason::MaterialVariantChanged,
            )],
        );

        assert_eq!(uploads[0].field, InstanceSoAField::MaterialParams);
        assert_eq!(
            uploads[0].field.expected_update_frequency(),
            "per_material_variant"
        );
    }

    #[test]
    fn dynamic_texture_upload_uses_dirty_rects_and_caps_calls() {
        let mut budget = FunUploadBudgetTracker::new(FunUploadBudget {
            texture_budget_bytes: 1_000_000,
            ..Default::default()
        });
        let rects = [
            TextureDirtyRect::new(0, 0, 8, 8),
            TextureDirtyRect::new(8, 0, 8, 8),
            TextureDirtyRect::new(16, 0, 8, 8),
        ];
        let plan = plan_dynamic_texture_upload(
            &mut budget,
            TextureUploadRequest {
                label: UPLOAD_TEXTURE_DIRTY_RECT,
                subsystem: FunUploadSubsystem::TextureAsset,
                texture_width: 1024,
                texture_height: 1024,
                dirty_rects: &rects,
                resized: false,
                policy: TextureUploadPolicy {
                    max_dirty_rects_per_frame: 2,
                    ..Default::default()
                },
            },
        );

        assert_eq!(plan.path, TextureUploadPath::DirtyRects);
        assert_eq!(plan.uploaded_rects.len(), 2);
        assert_eq!(plan.deferred_rects.len(), 1);
        assert_eq!(plan.bytes, 2 * 8 * 8 * 4);
    }

    #[test]
    fn cef_cpu_upload_is_noop_when_page_is_unchanged() {
        let mut budget = FunUploadBudgetTracker::new(FunUploadBudget::default());
        let plan = plan_cef_cpu_dirty_rect_upload(
            &mut budget,
            1920,
            1080,
            &[TextureDirtyRect::new(0, 0, 1920, 1080)],
            false,
            false,
            TextureUploadPolicy::default(),
        );

        assert_eq!(plan.path, TextureUploadPath::NoopUnchanged);
        assert_eq!(plan.bytes, 0);
        assert_eq!(budget.usage().cef_bytes, 0);
    }

    #[test]
    fn cef_cpu_upload_uses_dirty_rects_instead_of_full_frame_when_changed() {
        let mut budget = FunUploadBudgetTracker::new(FunUploadBudget::default());
        let plan = plan_cef_cpu_dirty_rect_upload(
            &mut budget,
            1920,
            1080,
            &[TextureDirtyRect::new(10, 10, 100, 50)],
            false,
            true,
            TextureUploadPolicy::default(),
        );

        assert_eq!(plan.label, UPLOAD_CEF_CPU_DIRTY_RECT);
        assert_eq!(plan.path, TextureUploadPath::DirtyRects);
        assert_eq!(plan.bytes, 100 * 50 * 4);
        assert!(plan.uploaded_rects[0].width < 1920);
    }
}
