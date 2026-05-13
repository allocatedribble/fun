use crate::backend::NativeBackend;
use crate::frame_graph::{
    FrameGraphPassDescriptor, FrameGraphPassHandle, FrameGraphPassRole, FrameGraphPassType,
    FrameGraphResourceHandle, RendererFrameGraph,
};

pub const QUEUE_SCHEDULER_SCHEMA_VERSION: u16 = 1;
pub const QUEUE_SCHEDULER_MAX_PASSES: usize = 64;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderQueueKind {
    #[default]
    Direct,
    Compute,
    Copy,
    Presentation,
}

impl RenderQueueKind {
    pub const ALL: [Self; 4] = [Self::Direct, Self::Compute, Self::Copy, Self::Presentation];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Compute => "compute",
            Self::Copy => "copy",
            Self::Presentation => "presentation",
        }
    }

    #[must_use]
    pub const fn slot(self) -> u8 {
        match self {
            Self::Direct => 0,
            Self::Compute => 1,
            Self::Copy => 2,
            Self::Presentation => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderQueueAvailability {
    Available,
    EmulatedOnDirect,
    NotSupportedByBackend,
}

impl RenderQueueAvailability {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::EmulatedOnDirect => "emulated_on_direct",
            Self::NotSupportedByBackend => "not_supported_by_backend",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderQueueCapabilities {
    pub schema_version: u16,
    pub native_backend: NativeBackend,
    pub direct: RenderQueueAvailability,
    pub compute: RenderQueueAvailability,
    pub copy: RenderQueueAvailability,
    pub presentation: RenderQueueAvailability,
}

impl RenderQueueCapabilities {
    pub const PUBLIC_WGPU_BRIDGE_DX12_DEFAULTS: Self = Self {
        schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
        native_backend: NativeBackend::Dx12,
        direct: RenderQueueAvailability::Available,
        // wgpu's public API exposes a single queue today. Async compute and
        // copy queues are emulated on the direct queue until the bridge gets
        // multi-queue plumbing — Pass 18 records that truthfully so callers
        // do not infer overlap that the bridge cannot actually deliver.
        compute: RenderQueueAvailability::EmulatedOnDirect,
        copy: RenderQueueAvailability::EmulatedOnDirect,
        presentation: RenderQueueAvailability::Available,
    };

    #[must_use]
    pub const fn for_native_backend(backend: NativeBackend) -> Self {
        let availability = match backend {
            NativeBackend::Dx12 | NativeBackend::Vulkan | NativeBackend::Metal => {
                RenderQueueAvailability::EmulatedOnDirect
            }
            NativeBackend::Unknown => RenderQueueAvailability::NotSupportedByBackend,
        };
        let presentation = if matches!(backend, NativeBackend::Unknown) {
            RenderQueueAvailability::NotSupportedByBackend
        } else {
            RenderQueueAvailability::Available
        };
        Self {
            schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
            native_backend: backend,
            direct: presentation,
            compute: availability,
            copy: availability,
            presentation,
        }
    }

    #[must_use]
    pub const fn availability_for(&self, kind: RenderQueueKind) -> RenderQueueAvailability {
        match kind {
            RenderQueueKind::Direct => self.direct,
            RenderQueueKind::Compute => self.compute,
            RenderQueueKind::Copy => self.copy,
            RenderQueueKind::Presentation => self.presentation,
        }
    }

    #[must_use]
    pub const fn supports_async(&self, kind: RenderQueueKind) -> bool {
        matches!(
            self.availability_for(kind),
            RenderQueueAvailability::Available
        )
    }
}

impl Default for RenderQueueCapabilities {
    fn default() -> Self {
        Self::PUBLIC_WGPU_BRIDGE_DX12_DEFAULTS
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderQueuePolicy {
    #[default]
    DirectOnly,
    DirectAndCopy,
    DirectComputeAndCopy,
}

impl RenderQueuePolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectOnly => "direct_only",
            Self::DirectAndCopy => "direct_and_copy",
            Self::DirectComputeAndCopy => "direct_compute_and_copy",
        }
    }

    #[must_use]
    pub const fn allows_copy_queue(self) -> bool {
        matches!(self, Self::DirectAndCopy | Self::DirectComputeAndCopy)
    }

    #[must_use]
    pub const fn allows_compute_queue(self) -> bool {
        matches!(self, Self::DirectComputeAndCopy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueueAssignmentReason {
    PresentationContract,
    GraphicsRenderPass,
    ComputeWork,
    CopyImport,
    Readback,
    VendorSdkBoundary,
    PostProcessFinal,
    UiComposite,
    DemotedDueToCapability,
    DemotedDueToPolicy,
    DefaultDirect,
}

impl QueueAssignmentReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PresentationContract => "presentation_contract",
            Self::GraphicsRenderPass => "graphics_render_pass",
            Self::ComputeWork => "compute_work",
            Self::CopyImport => "copy_import",
            Self::Readback => "readback",
            Self::VendorSdkBoundary => "vendor_sdk_boundary",
            Self::PostProcessFinal => "post_process_final",
            Self::UiComposite => "ui_composite",
            Self::DemotedDueToCapability => "demoted_due_to_capability",
            Self::DemotedDueToPolicy => "demoted_due_to_policy",
            Self::DefaultDirect => "default_direct",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderQueueAssignment {
    pub schema_version: u16,
    pub pass: FrameGraphPassHandle,
    pub stable_id: &'static str,
    pub pass_type: FrameGraphPassType,
    pub role: FrameGraphPassRole,
    pub preferred_queue: RenderQueueKind,
    pub assigned_queue: RenderQueueKind,
    pub assignment_reason: QueueAssignmentReason,
}

impl RenderQueueAssignment {
    #[must_use]
    pub const fn was_demoted(&self) -> bool {
        !matches!(
            (self.preferred_queue, self.assigned_queue),
            (RenderQueueKind::Direct, RenderQueueKind::Direct)
                | (RenderQueueKind::Compute, RenderQueueKind::Compute)
                | (RenderQueueKind::Copy, RenderQueueKind::Copy)
                | (RenderQueueKind::Presentation, RenderQueueKind::Presentation)
        )
    }

    #[must_use]
    pub const fn assigned_to_direct_queue(&self) -> bool {
        matches!(self.assigned_queue, RenderQueueKind::Direct)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrossQueueWaitKind {
    DirectAfterCompute,
    DirectAfterCopy,
    ComputeAfterCopy,
    PresentationAfterDirect,
    PresentationAfterCompute,
    PresentationAfterCopy,
}

impl CrossQueueWaitKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectAfterCompute => "direct_after_compute",
            Self::DirectAfterCopy => "direct_after_copy",
            Self::ComputeAfterCopy => "compute_after_copy",
            Self::PresentationAfterDirect => "presentation_after_direct",
            Self::PresentationAfterCompute => "presentation_after_compute",
            Self::PresentationAfterCopy => "presentation_after_copy",
        }
    }

    #[must_use]
    pub const fn from_pair(producer: RenderQueueKind, consumer: RenderQueueKind) -> Option<Self> {
        match (producer, consumer) {
            (RenderQueueKind::Compute, RenderQueueKind::Direct) => Some(Self::DirectAfterCompute),
            (RenderQueueKind::Copy, RenderQueueKind::Direct) => Some(Self::DirectAfterCopy),
            (RenderQueueKind::Copy, RenderQueueKind::Compute) => Some(Self::ComputeAfterCopy),
            (RenderQueueKind::Direct, RenderQueueKind::Presentation) => {
                Some(Self::PresentationAfterDirect)
            }
            (RenderQueueKind::Compute, RenderQueueKind::Presentation) => {
                Some(Self::PresentationAfterCompute)
            }
            (RenderQueueKind::Copy, RenderQueueKind::Presentation) => {
                Some(Self::PresentationAfterCopy)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CrossQueueWait {
    pub schema_version: u16,
    pub kind: CrossQueueWaitKind,
    pub producer_pass: FrameGraphPassHandle,
    pub consumer_pass: FrameGraphPassHandle,
    pub resource: FrameGraphResourceHandle,
    pub fence_signal_value: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OverlapBlockReason {
    NotApplicable,
    SerialDependency,
    QueueEmulatedOnDirect,
    PolicyForcesDirectOnly,
    PresentationOrdering,
    SharedResourceWriteCollision,
    InsufficientGpuTimeBudget,
}

impl OverlapBlockReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::SerialDependency => "serial_dependency",
            Self::QueueEmulatedOnDirect => "queue_emulated_on_direct",
            Self::PolicyForcesDirectOnly => "policy_forces_direct_only",
            Self::PresentationOrdering => "presentation_ordering",
            Self::SharedResourceWriteCollision => "shared_resource_write_collision",
            Self::InsufficientGpuTimeBudget => "insufficient_gpu_time_budget",
        }
    }

    #[must_use]
    pub const fn would_overlap(self) -> bool {
        matches!(self, Self::NotApplicable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlapOpportunity {
    pub schema_version: u16,
    pub primary: FrameGraphPassHandle,
    pub secondary: FrameGraphPassHandle,
    pub primary_queue: RenderQueueKind,
    pub secondary_queue: RenderQueueKind,
    pub block_reason: OverlapBlockReason,
}

impl OverlapOpportunity {
    #[must_use]
    pub const fn would_overlap(&self) -> bool {
        self.block_reason.would_overlap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "fun_ecs", derive(fun_ecs::Resource))]
pub struct RenderQueueScheduler {
    schema_version: u16,
    capabilities: RenderQueueCapabilities,
    policy: RenderQueuePolicy,
    assignments: Vec<RenderQueueAssignment>,
    cross_queue_waits: Vec<CrossQueueWait>,
    overlap_opportunities: Vec<OverlapOpportunity>,
}

impl RenderQueueScheduler {
    #[must_use]
    pub const fn new(capabilities: RenderQueueCapabilities, policy: RenderQueuePolicy) -> Self {
        Self {
            schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
            capabilities,
            policy,
            assignments: Vec::new(),
            cross_queue_waits: Vec::new(),
            overlap_opportunities: Vec::new(),
        }
    }

    pub fn compile_from_frame_graph(&mut self, graph: &RendererFrameGraph) {
        self.assignments.clear();
        self.cross_queue_waits.clear();
        self.overlap_opportunities.clear();

        for pass in graph.passes() {
            if !pass.descriptor.enabled {
                continue;
            }
            let preferred = preferred_queue_for_pass(pass.descriptor);
            let (assigned, reason) =
                self.demote_for_capability_and_policy(preferred, pass.descriptor);
            self.assignments.push(RenderQueueAssignment {
                schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
                pass: pass.handle,
                stable_id: pass.descriptor.stable_id,
                pass_type: pass.descriptor.pass_type,
                role: pass.descriptor.role,
                preferred_queue: preferred,
                assigned_queue: assigned,
                assignment_reason: reason,
            });
        }

        // Cross-queue waits and overlap opportunities are derived from the
        // resource read/write graph: when pass B reads a resource pass A
        // writes and they are assigned to different queues, the consumer
        // must wait on a fence the producer signalled.
        let mut next_fence_value: u64 = 1;
        for (consumer_index, consumer_pass) in graph.passes().iter().enumerate() {
            if !consumer_pass.descriptor.enabled {
                continue;
            }
            let consumer_assignment = match self
                .assignments
                .iter()
                .find(|assignment| assignment.pass == consumer_pass.handle)
            {
                Some(assignment) => *assignment,
                None => continue,
            };
            for read in &consumer_pass.reads {
                for producer_pass in graph.passes().iter().take(consumer_index) {
                    if !producer_pass.descriptor.enabled {
                        continue;
                    }
                    if !producer_pass.writes.contains(read) {
                        continue;
                    }
                    let Some(producer_assignment) = self
                        .assignments
                        .iter()
                        .find(|assignment| assignment.pass == producer_pass.handle)
                        .copied()
                    else {
                        continue;
                    };
                    if producer_assignment.assigned_queue == consumer_assignment.assigned_queue {
                        // Same-queue dependencies are encoded in the linear
                        // submission order; no cross-queue fence required.
                        let block_reason = same_queue_block_reason(
                            producer_assignment,
                            consumer_assignment,
                            *read,
                        );
                        self.overlap_opportunities.push(OverlapOpportunity {
                            schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
                            primary: producer_assignment.pass,
                            secondary: consumer_assignment.pass,
                            primary_queue: producer_assignment.assigned_queue,
                            secondary_queue: consumer_assignment.assigned_queue,
                            block_reason,
                        });
                        continue;
                    }
                    let Some(kind) = CrossQueueWaitKind::from_pair(
                        producer_assignment.assigned_queue,
                        consumer_assignment.assigned_queue,
                    ) else {
                        continue;
                    };
                    let fence_signal_value = next_fence_value;
                    next_fence_value = next_fence_value.saturating_add(1);
                    self.cross_queue_waits.push(CrossQueueWait {
                        schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
                        kind,
                        producer_pass: producer_assignment.pass,
                        consumer_pass: consumer_assignment.pass,
                        resource: *read,
                        fence_signal_value,
                    });
                    self.overlap_opportunities.push(OverlapOpportunity {
                        schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
                        primary: producer_assignment.pass,
                        secondary: consumer_assignment.pass,
                        primary_queue: producer_assignment.assigned_queue,
                        secondary_queue: consumer_assignment.assigned_queue,
                        block_reason: OverlapBlockReason::SerialDependency,
                    });
                }
            }
        }
    }

    #[must_use]
    pub fn assignments(&self) -> &[RenderQueueAssignment] {
        &self.assignments
    }

    #[must_use]
    pub fn cross_queue_waits(&self) -> &[CrossQueueWait] {
        &self.cross_queue_waits
    }

    #[must_use]
    pub fn overlap_opportunities(&self) -> &[OverlapOpportunity] {
        &self.overlap_opportunities
    }

    #[must_use]
    pub const fn capabilities(&self) -> RenderQueueCapabilities {
        self.capabilities
    }

    #[must_use]
    pub const fn policy(&self) -> RenderQueuePolicy {
        self.policy
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn assignments_for(&self, kind: RenderQueueKind) -> Vec<RenderQueueAssignment> {
        self.assignments
            .iter()
            .copied()
            .filter(|assignment| assignment.assigned_queue == kind)
            .collect()
    }

    #[must_use]
    pub fn assignment_for_pass(&self, pass: FrameGraphPassHandle) -> Option<RenderQueueAssignment> {
        self.assignments
            .iter()
            .copied()
            .find(|assignment| assignment.pass == pass)
    }

    fn demote_for_capability_and_policy(
        &self,
        preferred: RenderQueueKind,
        descriptor: FrameGraphPassDescriptor,
    ) -> (RenderQueueKind, QueueAssignmentReason) {
        let capability = self.capabilities.availability_for(preferred);
        let policy_allows = match preferred {
            RenderQueueKind::Direct | RenderQueueKind::Presentation => true,
            RenderQueueKind::Copy => self.policy.allows_copy_queue(),
            RenderQueueKind::Compute => self.policy.allows_compute_queue(),
        };
        if !policy_allows {
            return (
                RenderQueueKind::Direct,
                QueueAssignmentReason::DemotedDueToPolicy,
            );
        }
        match capability {
            RenderQueueAvailability::Available => {
                (preferred, default_assignment_reason(descriptor))
            }
            RenderQueueAvailability::EmulatedOnDirect
            | RenderQueueAvailability::NotSupportedByBackend => (
                RenderQueueKind::Direct,
                QueueAssignmentReason::DemotedDueToCapability,
            ),
        }
    }
}

#[must_use]
pub const fn preferred_queue_for_pass(descriptor: FrameGraphPassDescriptor) -> RenderQueueKind {
    match descriptor.pass_type {
        FrameGraphPassType::Presentation => RenderQueueKind::Presentation,
        FrameGraphPassType::Compute => RenderQueueKind::Compute,
        FrameGraphPassType::CopyImport | FrameGraphPassType::Readback => RenderQueueKind::Copy,
        FrameGraphPassType::Render | FrameGraphPassType::VendorSdk => RenderQueueKind::Direct,
    }
}

#[must_use]
const fn default_assignment_reason(descriptor: FrameGraphPassDescriptor) -> QueueAssignmentReason {
    match descriptor.pass_type {
        FrameGraphPassType::Presentation => QueueAssignmentReason::PresentationContract,
        FrameGraphPassType::Render => match descriptor.role {
            FrameGraphPassRole::Compose => QueueAssignmentReason::UiComposite,
            FrameGraphPassRole::PostProcessFinalOutputTransform => {
                QueueAssignmentReason::PostProcessFinal
            }
            _ => QueueAssignmentReason::GraphicsRenderPass,
        },
        FrameGraphPassType::Compute => QueueAssignmentReason::ComputeWork,
        FrameGraphPassType::CopyImport => QueueAssignmentReason::CopyImport,
        FrameGraphPassType::Readback => QueueAssignmentReason::Readback,
        FrameGraphPassType::VendorSdk => QueueAssignmentReason::VendorSdkBoundary,
    }
}

#[must_use]
const fn same_queue_block_reason(
    producer: RenderQueueAssignment,
    _consumer: RenderQueueAssignment,
    resource: FrameGraphResourceHandle,
) -> OverlapBlockReason {
    let _ = resource; // Reserved for shared-resource collision tracking.
    if matches!(producer.assigned_queue, RenderQueueKind::Presentation) {
        OverlapBlockReason::PresentationOrdering
    } else {
        OverlapBlockReason::SerialDependency
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CpuRendererSetTiming {
    pub set: crate::FunRendererFrameGraphStage,
    pub elapsed_ns: u64,
    pub system_count: u32,
}

impl CpuRendererSetTiming {
    #[must_use]
    pub const fn new(
        set: crate::FunRendererFrameGraphStage,
        elapsed_ns: u64,
        system_count: u32,
    ) -> Self {
        Self {
            set,
            elapsed_ns,
            system_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuPassTiming {
    pub pass: FrameGraphPassHandle,
    pub queue: RenderQueueKind,
    pub elapsed_ns: u64,
    pub overlap_with_other_queue_ns: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuQueueTiming {
    pub queue: RenderQueueKind,
    pub busy_ns: u64,
    pub idle_ns: u64,
    pub cross_queue_wait_ns: u64,
    pub overlapped_with_other_queues_ns: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QueueTimingReport {
    pub schema_version: u16,
    pub frame_index: u64,
    pub cpu_set_timings: Vec<CpuRendererSetTiming>,
    pub gpu_pass_timings: Vec<GpuPassTiming>,
    pub gpu_queue_timings: Vec<GpuQueueTiming>,
    pub cross_queue_wait_count: u32,
    pub blocked_overlap_count: u32,
    pub realised_overlap_count: u32,
}

impl QueueTimingReport {
    #[must_use]
    pub fn from_scheduler(scheduler: &RenderQueueScheduler, frame_index: u64) -> Self {
        let cross_queue_wait_count =
            u32::try_from(scheduler.cross_queue_waits.len()).unwrap_or(u32::MAX);
        let realised_overlap_count = scheduler
            .overlap_opportunities
            .iter()
            .filter(|opportunity| opportunity.would_overlap())
            .count();
        let blocked_overlap_count = scheduler
            .overlap_opportunities
            .len()
            .saturating_sub(realised_overlap_count);
        Self {
            schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
            frame_index,
            cpu_set_timings: Vec::new(),
            gpu_pass_timings: Vec::new(),
            gpu_queue_timings: Vec::new(),
            cross_queue_wait_count,
            blocked_overlap_count: u32::try_from(blocked_overlap_count).unwrap_or(u32::MAX),
            realised_overlap_count: u32::try_from(realised_overlap_count).unwrap_or(u32::MAX),
        }
    }

    pub fn record_cpu_set(&mut self, timing: CpuRendererSetTiming) {
        self.cpu_set_timings.push(timing);
    }

    pub fn record_gpu_pass(&mut self, timing: GpuPassTiming) {
        self.gpu_pass_timings.push(timing);
    }

    pub fn record_gpu_queue(&mut self, timing: GpuQueueTiming) {
        self.gpu_queue_timings.push(timing);
    }

    #[must_use]
    pub fn total_gpu_busy_ns(&self) -> u64 {
        self.gpu_queue_timings
            .iter()
            .map(|timing| timing.busy_ns)
            .sum()
    }

    #[must_use]
    pub fn total_cross_queue_wait_ns(&self) -> u64 {
        self.gpu_queue_timings
            .iter()
            .map(|timing| timing.cross_queue_wait_ns)
            .sum()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UploadBatchPlan {
    pub schema_version: u16,
    pub texture_upload_count: u32,
    pub buffer_upload_count: u32,
    pub readback_count: u32,
    pub native_ui_copy_count: u32,
    pub batched_before_submission: bool,
    pub uses_copy_queue: bool,
}

impl UploadBatchPlan {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
        texture_upload_count: 0,
        buffer_upload_count: 0,
        readback_count: 0,
        native_ui_copy_count: 0,
        batched_before_submission: true,
        uses_copy_queue: false,
    };

    #[must_use]
    pub const fn for_policy(policy: RenderQueuePolicy) -> Self {
        Self {
            schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
            texture_upload_count: 0,
            buffer_upload_count: 0,
            readback_count: 0,
            native_ui_copy_count: 0,
            batched_before_submission: true,
            uses_copy_queue: policy.allows_copy_queue(),
        }
    }
}

impl Default for UploadBatchPlan {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FenceRetirementSource {
    #[default]
    GpuFenceCompleted,
    FrameNumberGuess,
}

impl FenceRetirementSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuFenceCompleted => "gpu_fence_completed",
            Self::FrameNumberGuess => "frame_number_guess",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceRetirementContract {
    pub schema_version: u16,
    pub source: FenceRetirementSource,
    pub frame_number_guess_allowed: bool,
}

impl ResourceRetirementContract {
    pub const PRODUCT_FENCE_DRIVEN: Self = Self {
        schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
        source: FenceRetirementSource::GpuFenceCompleted,
        frame_number_guess_allowed: false,
    };

    #[must_use]
    pub const fn debug_frame_number_only() -> Self {
        Self {
            schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
            source: FenceRetirementSource::FrameNumberGuess,
            frame_number_guess_allowed: true,
        }
    }
}

impl Default for ResourceRetirementContract {
    fn default() -> Self {
        Self::PRODUCT_FENCE_DRIVEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame_graph::{RendererFrameDescription, RendererFrameGraph};

    fn dx12_scheduler(policy: RenderQueuePolicy) -> RenderQueueScheduler {
        RenderQueueScheduler::new(
            RenderQueueCapabilities::for_native_backend(NativeBackend::Dx12),
            policy,
        )
    }

    #[test]
    fn render_queue_kinds_have_stable_slots_and_string_names() {
        for kind in RenderQueueKind::ALL {
            let _ = kind.as_str();
            assert!(kind.slot() < 4);
        }
        assert_eq!(RenderQueueKind::Direct.slot(), 0);
        assert_eq!(RenderQueueKind::Compute.slot(), 1);
        assert_eq!(RenderQueueKind::Copy.slot(), 2);
        assert_eq!(RenderQueueKind::Presentation.slot(), 3);
    }

    #[test]
    fn dx12_default_capabilities_emulate_compute_and_copy_on_direct() {
        let capabilities = RenderQueueCapabilities::for_native_backend(NativeBackend::Dx12);
        assert_eq!(capabilities.direct, RenderQueueAvailability::Available);
        assert_eq!(
            capabilities.compute,
            RenderQueueAvailability::EmulatedOnDirect
        );
        assert_eq!(capabilities.copy, RenderQueueAvailability::EmulatedOnDirect);
        assert_eq!(
            capabilities.presentation,
            RenderQueueAvailability::Available
        );
        assert!(!capabilities.supports_async(RenderQueueKind::Compute));
    }

    #[test]
    fn unknown_native_backend_reports_no_queues_available() {
        let capabilities = RenderQueueCapabilities::for_native_backend(NativeBackend::Unknown);
        assert_eq!(
            capabilities.direct,
            RenderQueueAvailability::NotSupportedByBackend
        );
        assert_eq!(
            capabilities.presentation,
            RenderQueueAvailability::NotSupportedByBackend
        );
    }

    #[test]
    fn direct_only_policy_demotes_compute_and_copy_passes_to_direct() {
        let mut scheduler = dx12_scheduler(RenderQueuePolicy::DirectOnly);
        let description = RendererFrameDescription::static_scene_with_ui(0)
            .with_virtual_resources(true)
            .with_diagnostics_readback(true);
        let graph = RendererFrameGraph::from_frame_description(description);
        scheduler.compile_from_frame_graph(&graph);

        for assignment in scheduler.assignments() {
            // Every pass must end up on the direct queue (or the presentation
            // queue for the present pass) under the conservative DirectOnly
            // policy, with the demotion reason recorded.
            assert!(matches!(
                assignment.assigned_queue,
                RenderQueueKind::Direct | RenderQueueKind::Presentation
            ));
            if !matches!(
                assignment.preferred_queue,
                RenderQueueKind::Direct | RenderQueueKind::Presentation
            ) {
                assert_eq!(
                    assignment.assignment_reason,
                    QueueAssignmentReason::DemotedDueToPolicy
                );
            }
        }
        // Cross-queue waits under DirectOnly may only be the Direct -> Present
        // hand-off; Compute/Copy queues must not appear because every non-
        // Direct/Presentation pass was demoted to Direct.
        for wait in scheduler.cross_queue_waits() {
            assert_eq!(wait.kind, CrossQueueWaitKind::PresentationAfterDirect);
        }
        for opportunity in scheduler.overlap_opportunities() {
            assert!(matches!(
                opportunity.block_reason,
                OverlapBlockReason::SerialDependency | OverlapBlockReason::PresentationOrdering
            ));
            assert!(!opportunity.would_overlap());
        }
    }

    #[test]
    fn direct_and_copy_policy_keeps_compute_on_direct_but_lets_copy_attempt_copy_queue() {
        let mut scheduler = dx12_scheduler(RenderQueuePolicy::DirectAndCopy);
        let graph = RendererFrameGraph::from_frame_description(
            RendererFrameDescription::static_scene_with_ui(0),
        );
        scheduler.compile_from_frame_graph(&graph);

        for assignment in scheduler.assignments() {
            if matches!(assignment.preferred_queue, RenderQueueKind::Compute) {
                assert_eq!(assignment.assigned_queue, RenderQueueKind::Direct);
                assert_eq!(
                    assignment.assignment_reason,
                    QueueAssignmentReason::DemotedDueToPolicy
                );
            }
            if matches!(assignment.preferred_queue, RenderQueueKind::Copy) {
                // wgpu bridge today emulates the copy queue on the direct
                // queue, so even when policy allows it the capability layer
                // demotes the pass and records the reason.
                assert_eq!(assignment.assigned_queue, RenderQueueKind::Direct);
                assert_eq!(
                    assignment.assignment_reason,
                    QueueAssignmentReason::DemotedDueToCapability
                );
            }
        }
    }

    #[test]
    fn capability_overrides_let_compute_queue_assignments_land_when_available() {
        let mut scheduler = RenderQueueScheduler::new(
            RenderQueueCapabilities {
                schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
                native_backend: NativeBackend::Dx12,
                direct: RenderQueueAvailability::Available,
                compute: RenderQueueAvailability::Available,
                copy: RenderQueueAvailability::Available,
                presentation: RenderQueueAvailability::Available,
            },
            RenderQueuePolicy::DirectComputeAndCopy,
        );
        let description = RendererFrameDescription::static_scene_with_ui(0)
            .with_virtual_resources(true)
            .with_diagnostics_readback(true);
        let graph = RendererFrameGraph::from_frame_description(description);
        scheduler.compile_from_frame_graph(&graph);

        let compute_assignments = scheduler.assignments_for(RenderQueueKind::Compute);
        let copy_assignments = scheduler.assignments_for(RenderQueueKind::Copy);
        assert!(!compute_assignments.is_empty());
        assert!(!copy_assignments.is_empty());

        // Cross-queue waits must be recorded whenever the consumer reads a
        // resource produced on a different queue.
        assert!(!scheduler.cross_queue_waits().is_empty());
        for wait in scheduler.cross_queue_waits() {
            assert_ne!(wait.fence_signal_value, 0);
        }
        // Overlap opportunities must classify cross-queue dependencies as
        // serial rather than as realisable overlap.
        let blocked = scheduler
            .overlap_opportunities()
            .iter()
            .filter(|opportunity| !opportunity.would_overlap())
            .count();
        assert!(blocked > 0);
    }

    #[test]
    fn cross_queue_wait_kind_routes_pairs_to_typed_variants() {
        assert_eq!(
            CrossQueueWaitKind::from_pair(RenderQueueKind::Compute, RenderQueueKind::Direct),
            Some(CrossQueueWaitKind::DirectAfterCompute)
        );
        assert_eq!(
            CrossQueueWaitKind::from_pair(RenderQueueKind::Copy, RenderQueueKind::Direct),
            Some(CrossQueueWaitKind::DirectAfterCopy)
        );
        assert_eq!(
            CrossQueueWaitKind::from_pair(RenderQueueKind::Direct, RenderQueueKind::Direct),
            None,
        );
    }

    #[test]
    fn timing_report_aggregates_overlap_and_block_counts_from_scheduler() {
        let mut scheduler = RenderQueueScheduler::new(
            RenderQueueCapabilities {
                schema_version: QUEUE_SCHEDULER_SCHEMA_VERSION,
                native_backend: NativeBackend::Dx12,
                direct: RenderQueueAvailability::Available,
                compute: RenderQueueAvailability::Available,
                copy: RenderQueueAvailability::Available,
                presentation: RenderQueueAvailability::Available,
            },
            RenderQueuePolicy::DirectComputeAndCopy,
        );
        let graph = RendererFrameGraph::from_frame_description(
            RendererFrameDescription::static_scene_with_ui(11).with_virtual_resources(true),
        );
        scheduler.compile_from_frame_graph(&graph);

        let mut report = QueueTimingReport::from_scheduler(&scheduler, 11);
        report.record_gpu_queue(GpuQueueTiming {
            queue: RenderQueueKind::Direct,
            busy_ns: 4_000_000,
            idle_ns: 1_000_000,
            cross_queue_wait_ns: 250_000,
            overlapped_with_other_queues_ns: 0,
        });
        report.record_gpu_queue(GpuQueueTiming {
            queue: RenderQueueKind::Compute,
            busy_ns: 2_000_000,
            idle_ns: 0,
            cross_queue_wait_ns: 500_000,
            overlapped_with_other_queues_ns: 1_500_000,
        });

        assert_eq!(report.frame_index, 11);
        assert_eq!(
            report.cross_queue_wait_count as usize,
            scheduler.cross_queue_waits().len()
        );
        assert!(report.blocked_overlap_count > 0);
        assert_eq!(report.total_gpu_busy_ns(), 6_000_000);
        assert_eq!(report.total_cross_queue_wait_ns(), 750_000);
    }

    #[test]
    fn upload_batch_plan_uses_copy_queue_only_when_policy_allows() {
        let direct_only = UploadBatchPlan::for_policy(RenderQueuePolicy::DirectOnly);
        let direct_and_copy = UploadBatchPlan::for_policy(RenderQueuePolicy::DirectAndCopy);
        assert!(!direct_only.uses_copy_queue);
        assert!(direct_and_copy.uses_copy_queue);
        assert!(direct_only.batched_before_submission);
        assert!(direct_and_copy.batched_before_submission);
    }

    #[test]
    fn resource_retirement_contract_defaults_to_gpu_fence_driven_not_frame_number_guess() {
        let contract = ResourceRetirementContract::default();
        assert_eq!(contract.source, FenceRetirementSource::GpuFenceCompleted);
        assert!(!contract.frame_number_guess_allowed);

        let debug = ResourceRetirementContract::debug_frame_number_only();
        assert_eq!(debug.source, FenceRetirementSource::FrameNumberGuess);
        assert!(debug.frame_number_guess_allowed);
    }

    #[test]
    fn assignment_for_pass_returns_recorded_assignment() {
        let mut scheduler = dx12_scheduler(RenderQueuePolicy::DirectOnly);
        let graph = RendererFrameGraph::from_frame_description(
            RendererFrameDescription::static_scene_with_ui(0),
        );
        scheduler.compile_from_frame_graph(&graph);

        let present = graph
            .pass_handle_for_role(FrameGraphPassRole::Present)
            .expect("present pass exists");
        let assignment = scheduler
            .assignment_for_pass(present)
            .expect("present pass assignment");
        assert_eq!(assignment.assigned_queue, RenderQueueKind::Presentation);
        assert_eq!(
            assignment.assignment_reason,
            QueueAssignmentReason::PresentationContract
        );
    }

    #[test]
    fn overlap_opportunity_block_reasons_are_inspectable() {
        let block = OverlapBlockReason::SerialDependency;
        assert_eq!(block.as_str(), "serial_dependency");
        assert!(!block.would_overlap());
        assert!(OverlapBlockReason::NotApplicable.would_overlap());
    }
}
