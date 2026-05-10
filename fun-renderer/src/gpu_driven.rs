use crate::backend::NativeBackend;
use crate::frame_graph::FrameGraphResourceType;

pub const GPU_DRIVEN_SCHEMA_VERSION: u16 = 1;
pub const GPU_DRIVEN_CULLING_WORKGROUP_SIZE: u32 = 64;
pub const GPU_DRIVEN_DEFAULT_MAX_DRAW_COUNT: u32 = 1 << 18;
pub const GPU_DRIVEN_INDIRECT_ARG_STRIDE: u32 = 20;
pub const GPU_DRIVEN_DRAW_COUNT_STRIDE: u32 = 4;
pub const GPU_DRIVEN_HI_Z_DEFAULT_LEVELS: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDrivenBufferKind {
    VisibleInstanceList,
    DrawArguments,
    DrawCount,
    MaterialIdTable,
    ObjectIdTable,
    HiZPyramid,
    PreviousFrameOcclusion,
    DebugCounters,
}

impl GpuDrivenBufferKind {
    pub const ALL: [Self; 8] = [
        Self::VisibleInstanceList,
        Self::DrawArguments,
        Self::DrawCount,
        Self::MaterialIdTable,
        Self::ObjectIdTable,
        Self::HiZPyramid,
        Self::PreviousFrameOcclusion,
        Self::DebugCounters,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VisibleInstanceList => "visible_instance_list",
            Self::DrawArguments => "draw_arguments",
            Self::DrawCount => "draw_count",
            Self::MaterialIdTable => "material_id_table",
            Self::ObjectIdTable => "object_id_table",
            Self::HiZPyramid => "hi_z_pyramid",
            Self::PreviousFrameOcclusion => "previous_frame_occlusion",
            Self::DebugCounters => "debug_counters",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenBufferLayout {
    pub kind: GpuDrivenBufferKind,
    pub stable_label: &'static str,
    pub element_stride: u32,
    pub element_count: u32,
}

impl GpuDrivenBufferLayout {
    #[must_use]
    pub const fn new(
        kind: GpuDrivenBufferKind,
        stable_label: &'static str,
        element_stride: u32,
        element_count: u32,
    ) -> Self {
        Self {
            kind,
            stable_label,
            element_stride,
            element_count,
        }
    }

    #[must_use]
    pub const fn byte_size(self) -> u64 {
        (self.element_stride as u64).saturating_mul(self.element_count as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenBufferSet {
    pub schema_version: u16,
    pub max_draw_count: u32,
    pub visible_instance_list: GpuDrivenBufferLayout,
    pub draw_arguments: GpuDrivenBufferLayout,
    pub draw_count: GpuDrivenBufferLayout,
    pub material_id_table: GpuDrivenBufferLayout,
    pub object_id_table: GpuDrivenBufferLayout,
    pub debug_counters: GpuDrivenBufferLayout,
}

impl GpuDrivenBufferSet {
    #[must_use]
    pub const fn for_max_draw_count(max_draw_count: u32) -> Self {
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            max_draw_count,
            visible_instance_list: GpuDrivenBufferLayout::new(
                GpuDrivenBufferKind::VisibleInstanceList,
                "fun_renderer.gpu_driven.visible_instance_list",
                4,
                max_draw_count,
            ),
            draw_arguments: GpuDrivenBufferLayout::new(
                GpuDrivenBufferKind::DrawArguments,
                "fun_renderer.gpu_driven.draw_arguments",
                GPU_DRIVEN_INDIRECT_ARG_STRIDE,
                max_draw_count,
            ),
            draw_count: GpuDrivenBufferLayout::new(
                GpuDrivenBufferKind::DrawCount,
                "fun_renderer.gpu_driven.draw_count",
                GPU_DRIVEN_DRAW_COUNT_STRIDE,
                1,
            ),
            material_id_table: GpuDrivenBufferLayout::new(
                GpuDrivenBufferKind::MaterialIdTable,
                "fun_renderer.gpu_driven.material_id_table",
                4,
                max_draw_count,
            ),
            object_id_table: GpuDrivenBufferLayout::new(
                GpuDrivenBufferKind::ObjectIdTable,
                "fun_renderer.gpu_driven.object_id_table",
                4,
                max_draw_count,
            ),
            debug_counters: GpuDrivenBufferLayout::new(
                GpuDrivenBufferKind::DebugCounters,
                "fun_renderer.gpu_driven.debug_counters",
                4,
                GpuDrivenDebugCounter::SLOT_COUNT,
            ),
        }
    }

    #[must_use]
    pub const fn product_default() -> Self {
        Self::for_max_draw_count(GPU_DRIVEN_DEFAULT_MAX_DRAW_COUNT)
    }

    #[must_use]
    pub fn total_byte_size(self) -> u64 {
        self.visible_instance_list.byte_size()
            + self.draw_arguments.byte_size()
            + self.draw_count.byte_size()
            + self.material_id_table.byte_size()
            + self.object_id_table.byte_size()
            + self.debug_counters.byte_size()
    }
}

impl Default for GpuDrivenBufferSet {
    fn default() -> Self {
        Self::product_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDrivenIndirectCommandSignature {
    Dx12ExecuteIndirectDrawIndexed,
    VulkanDrawIndexedIndirectCount,
    MetalDrawIndexedIndirectCommands,
    NotApplicableForBackend,
}

impl GpuDrivenIndirectCommandSignature {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dx12ExecuteIndirectDrawIndexed => "dx12_execute_indirect_draw_indexed",
            Self::VulkanDrawIndexedIndirectCount => "vulkan_draw_indexed_indirect_count",
            Self::MetalDrawIndexedIndirectCommands => "metal_draw_indexed_indirect_commands",
            Self::NotApplicableForBackend => "not_applicable_for_backend",
        }
    }

    #[must_use]
    pub const fn for_native_backend(backend: NativeBackend) -> Self {
        match backend {
            NativeBackend::Dx12 => Self::Dx12ExecuteIndirectDrawIndexed,
            NativeBackend::Vulkan => Self::VulkanDrawIndexedIndirectCount,
            NativeBackend::Metal => Self::MetalDrawIndexedIndirectCommands,
            NativeBackend::Unknown => Self::NotApplicableForBackend,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenIndirectPath {
    pub schema_version: u16,
    pub native_backend: NativeBackend,
    pub command_signature: GpuDrivenIndirectCommandSignature,
    pub uses_draw_indirect_count: bool,
    pub uses_root_constants_for_draw_id: bool,
}

impl GpuDrivenIndirectPath {
    #[must_use]
    pub const fn for_native_backend(backend: NativeBackend) -> Self {
        let command_signature = GpuDrivenIndirectCommandSignature::for_native_backend(backend);
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            native_backend: backend,
            command_signature,
            uses_draw_indirect_count: !matches!(
                command_signature,
                GpuDrivenIndirectCommandSignature::NotApplicableForBackend
            ),
            uses_root_constants_for_draw_id: matches!(backend, NativeBackend::Dx12),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDrivenDebugCounter {
    InstancesTested,
    InstancesVisible,
    InstancesFrustumRejected,
    InstancesOcclusionRejected,
    InstancesUncertain,
    ClustersTested,
    ClustersVisible,
    ClustersRejected,
}

impl GpuDrivenDebugCounter {
    pub const SLOT_COUNT: u32 = 8;
    pub const ALL: [Self; 8] = [
        Self::InstancesTested,
        Self::InstancesVisible,
        Self::InstancesFrustumRejected,
        Self::InstancesOcclusionRejected,
        Self::InstancesUncertain,
        Self::ClustersTested,
        Self::ClustersVisible,
        Self::ClustersRejected,
    ];

    #[must_use]
    pub const fn slot(self) -> u32 {
        match self {
            Self::InstancesTested => 0,
            Self::InstancesVisible => 1,
            Self::InstancesFrustumRejected => 2,
            Self::InstancesOcclusionRejected => 3,
            Self::InstancesUncertain => 4,
            Self::ClustersTested => 5,
            Self::ClustersVisible => 6,
            Self::ClustersRejected => 7,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstancesTested => "instances_tested",
            Self::InstancesVisible => "instances_visible",
            Self::InstancesFrustumRejected => "instances_frustum_rejected",
            Self::InstancesOcclusionRejected => "instances_occlusion_rejected",
            Self::InstancesUncertain => "instances_uncertain",
            Self::ClustersTested => "clusters_tested",
            Self::ClustersVisible => "clusters_visible",
            Self::ClustersRejected => "clusters_rejected",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenCullingPassPlan {
    pub schema_version: u16,
    pub workgroup_size: u32,
    pub workgroup_count: u32,
    pub max_instance_count: u32,
    pub reads_object_bounds: bool,
    pub reads_camera_planes: bool,
    pub writes_visible_instance_list: bool,
    pub writes_draw_arguments: bool,
    pub writes_draw_count: bool,
    pub writes_debug_counters: bool,
}

impl GpuDrivenCullingPassPlan {
    #[must_use]
    pub const fn for_instance_count(max_instance_count: u32) -> Self {
        let workgroup_count = max_instance_count.div_ceil(GPU_DRIVEN_CULLING_WORKGROUP_SIZE);
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            workgroup_size: GPU_DRIVEN_CULLING_WORKGROUP_SIZE,
            workgroup_count,
            max_instance_count,
            reads_object_bounds: true,
            reads_camera_planes: true,
            writes_visible_instance_list: true,
            writes_draw_arguments: true,
            writes_draw_count: true,
            writes_debug_counters: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDrivenOcclusionPolicy {
    #[default]
    Conservative,
    Aggressive,
    DisabledForDebug,
}

impl GpuDrivenOcclusionPolicy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Conservative => "conservative",
            Self::Aggressive => "aggressive",
            Self::DisabledForDebug => "disabled_for_debug",
        }
    }

    #[must_use]
    pub const fn classifies_uncertain_as_visible(self) -> bool {
        matches!(self, Self::Conservative | Self::DisabledForDebug)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenHiZPyramidPlan {
    pub schema_version: u16,
    pub mip_levels: u8,
    pub base_resource_type: FrameGraphResourceType,
    pub history_resource_type: FrameGraphResourceType,
    pub policy: GpuDrivenOcclusionPolicy,
    pub previous_frame_required: bool,
}

impl GpuDrivenHiZPyramidPlan {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: GPU_DRIVEN_SCHEMA_VERSION,
        mip_levels: GPU_DRIVEN_HI_Z_DEFAULT_LEVELS,
        base_resource_type: FrameGraphResourceType::Depth,
        history_resource_type: FrameGraphResourceType::HistoryBuffer,
        policy: GpuDrivenOcclusionPolicy::Conservative,
        previous_frame_required: true,
    };

    #[must_use]
    pub const fn with_policy(mut self, policy: GpuDrivenOcclusionPolicy) -> Self {
        self.policy = policy;
        self.previous_frame_required =
            !matches!(policy, GpuDrivenOcclusionPolicy::DisabledForDebug);
        self
    }
}

impl Default for GpuDrivenHiZPyramidPlan {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuDrivenClusterPath {
    #[default]
    ComputeFallback,
    MeshShaderWhereSupported,
    MeshShaderRequired,
}

impl GpuDrivenClusterPath {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComputeFallback => "compute_fallback",
            Self::MeshShaderWhereSupported => "mesh_shader_where_supported",
            Self::MeshShaderRequired => "mesh_shader_required",
        }
    }

    #[must_use]
    pub const fn requires_mesh_shader(self) -> bool {
        matches!(self, Self::MeshShaderRequired)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenClusterScaffold {
    pub schema_version: u16,
    pub path: GpuDrivenClusterPath,
    pub mesh_shader_supported: bool,
    pub uses_meshlet_metadata_placeholder: bool,
    pub uses_cluster_bounds: bool,
    pub uses_material_ranges: bool,
}

impl GpuDrivenClusterScaffold {
    #[must_use]
    pub const fn from_capabilities(mesh_shader_supported: bool) -> Self {
        let path = if mesh_shader_supported {
            GpuDrivenClusterPath::MeshShaderWhereSupported
        } else {
            GpuDrivenClusterPath::ComputeFallback
        };
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            path,
            mesh_shader_supported,
            uses_meshlet_metadata_placeholder: true,
            uses_cluster_bounds: true,
            uses_material_ranges: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenPlan {
    pub schema_version: u16,
    pub native_backend: NativeBackend,
    pub buffer_set: GpuDrivenBufferSet,
    pub indirect_path: GpuDrivenIndirectPath,
    pub frustum_culling: GpuDrivenCullingPassPlan,
    pub hi_z_pyramid: GpuDrivenHiZPyramidPlan,
    pub cluster_scaffold: GpuDrivenClusterScaffold,
}

impl GpuDrivenPlan {
    #[must_use]
    pub fn product_for_backend(
        native_backend: NativeBackend,
        max_instance_count: u32,
        mesh_shader_supported: bool,
    ) -> Self {
        let buffer_set = GpuDrivenBufferSet::for_max_draw_count(max_instance_count);
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            native_backend,
            buffer_set,
            indirect_path: GpuDrivenIndirectPath::for_native_backend(native_backend),
            frustum_culling: GpuDrivenCullingPassPlan::for_instance_count(max_instance_count),
            hi_z_pyramid: GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT,
            cluster_scaffold: GpuDrivenClusterScaffold::from_capabilities(mesh_shader_supported),
        }
    }

    #[must_use]
    pub fn with_occlusion_policy(mut self, policy: GpuDrivenOcclusionPolicy) -> Self {
        self.hi_z_pyramid = self.hi_z_pyramid.with_policy(policy);
        self
    }

    #[must_use]
    pub fn schedule_culling_before_draw(&self) -> bool {
        self.frustum_culling.workgroup_count > 0
            && self.frustum_culling.writes_draw_arguments
            && self.frustum_culling.writes_draw_count
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenDebugCounterSnapshot {
    pub schema_version: u16,
    pub instances_tested: u32,
    pub instances_visible: u32,
    pub instances_frustum_rejected: u32,
    pub instances_occlusion_rejected: u32,
    pub instances_uncertain: u32,
    pub clusters_tested: u32,
    pub clusters_visible: u32,
    pub clusters_rejected: u32,
}

impl GpuDrivenDebugCounterSnapshot {
    #[must_use]
    pub const fn from_slots(slots: [u32; 8]) -> Self {
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            instances_tested: slots[GpuDrivenDebugCounter::InstancesTested.slot() as usize],
            instances_visible: slots[GpuDrivenDebugCounter::InstancesVisible.slot() as usize],
            instances_frustum_rejected: slots
                [GpuDrivenDebugCounter::InstancesFrustumRejected.slot() as usize],
            instances_occlusion_rejected: slots
                [GpuDrivenDebugCounter::InstancesOcclusionRejected.slot() as usize],
            instances_uncertain: slots[GpuDrivenDebugCounter::InstancesUncertain.slot() as usize],
            clusters_tested: slots[GpuDrivenDebugCounter::ClustersTested.slot() as usize],
            clusters_visible: slots[GpuDrivenDebugCounter::ClustersVisible.slot() as usize],
            clusters_rejected: slots[GpuDrivenDebugCounter::ClustersRejected.slot() as usize],
        }
    }

    #[must_use]
    pub fn budget_consistent(self) -> bool {
        let resolved = self
            .instances_visible
            .saturating_add(self.instances_frustum_rejected)
            .saturating_add(self.instances_occlusion_rejected)
            .saturating_add(self.instances_uncertain);
        resolved <= self.instances_tested
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenParityComparison {
    pub schema_version: u16,
    pub direct_draw_count: u32,
    pub gpu_driven_draw_count: u32,
    pub direct_triangle_count: u64,
    pub gpu_driven_triangle_count: u64,
    pub matched: bool,
}

impl GpuDrivenParityComparison {
    #[must_use]
    pub fn from_counts(
        direct_draw_count: u32,
        gpu_driven_draw_count: u32,
        direct_triangle_count: u64,
        gpu_driven_triangle_count: u64,
    ) -> Self {
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            direct_draw_count,
            gpu_driven_draw_count,
            direct_triangle_count,
            gpu_driven_triangle_count,
            matched: direct_draw_count == gpu_driven_draw_count
                && direct_triangle_count == gpu_driven_triangle_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuDrivenMetrics {
    pub schema_version: u16,
    pub native_backend: NativeBackend,
    pub buffer_byte_size: u64,
    pub frustum_workgroup_count: u32,
    pub debug_counters: GpuDrivenDebugCounterSnapshot,
    pub parity: GpuDrivenParityComparison,
    pub occlusion_policy: GpuDrivenOcclusionPolicy,
    pub cluster_path: GpuDrivenClusterPath,
}

impl GpuDrivenMetrics {
    #[must_use]
    pub fn from_plan(plan: &GpuDrivenPlan) -> Self {
        Self {
            schema_version: GPU_DRIVEN_SCHEMA_VERSION,
            native_backend: plan.native_backend,
            buffer_byte_size: plan.buffer_set.total_byte_size(),
            frustum_workgroup_count: plan.frustum_culling.workgroup_count,
            debug_counters: GpuDrivenDebugCounterSnapshot::default(),
            parity: GpuDrivenParityComparison::default(),
            occlusion_policy: plan.hi_z_pyramid.policy,
            cluster_path: plan.cluster_scaffold.path,
        }
    }

    #[must_use]
    pub fn with_debug_counters(mut self, counters: GpuDrivenDebugCounterSnapshot) -> Self {
        self.debug_counters = counters;
        self
    }

    #[must_use]
    pub fn with_parity(mut self, parity: GpuDrivenParityComparison) -> Self {
        self.parity = parity;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_set_layouts_use_stable_strides_for_indirect_args_and_count() {
        let set = GpuDrivenBufferSet::for_max_draw_count(1024);
        assert_eq!(
            set.draw_arguments.element_stride,
            GPU_DRIVEN_INDIRECT_ARG_STRIDE
        );
        assert_eq!(set.draw_count.element_stride, GPU_DRIVEN_DRAW_COUNT_STRIDE);
        assert_eq!(set.draw_count.element_count, 1);
        assert_eq!(set.visible_instance_list.element_count, 1024);
        assert_eq!(set.material_id_table.element_count, 1024);
        assert_eq!(set.object_id_table.element_count, 1024);
        assert_eq!(
            set.debug_counters.element_count,
            GpuDrivenDebugCounter::SLOT_COUNT
        );
    }

    #[test]
    fn indirect_command_signature_routes_to_per_backend_choice() {
        assert_eq!(
            GpuDrivenIndirectCommandSignature::for_native_backend(NativeBackend::Dx12),
            GpuDrivenIndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed
        );
        assert_eq!(
            GpuDrivenIndirectCommandSignature::for_native_backend(NativeBackend::Vulkan),
            GpuDrivenIndirectCommandSignature::VulkanDrawIndexedIndirectCount
        );
        assert_eq!(
            GpuDrivenIndirectCommandSignature::for_native_backend(NativeBackend::Metal),
            GpuDrivenIndirectCommandSignature::MetalDrawIndexedIndirectCommands
        );
        assert_eq!(
            GpuDrivenIndirectCommandSignature::for_native_backend(NativeBackend::Unknown),
            GpuDrivenIndirectCommandSignature::NotApplicableForBackend
        );
    }

    #[test]
    fn dx12_indirect_path_uses_root_constants_for_draw_id() {
        let path = GpuDrivenIndirectPath::for_native_backend(NativeBackend::Dx12);
        assert!(path.uses_root_constants_for_draw_id);
        assert!(path.uses_draw_indirect_count);
    }

    #[test]
    fn vulkan_and_metal_indirect_paths_keep_draw_indirect_count_but_no_root_constants() {
        let vulkan = GpuDrivenIndirectPath::for_native_backend(NativeBackend::Vulkan);
        let metal = GpuDrivenIndirectPath::for_native_backend(NativeBackend::Metal);
        assert!(vulkan.uses_draw_indirect_count);
        assert!(!vulkan.uses_root_constants_for_draw_id);
        assert!(metal.uses_draw_indirect_count);
        assert!(!metal.uses_root_constants_for_draw_id);
    }

    #[test]
    fn frustum_pass_plan_dispatches_one_workgroup_per_64_instances() {
        let plan = GpuDrivenCullingPassPlan::for_instance_count(1024);
        assert_eq!(plan.workgroup_size, GPU_DRIVEN_CULLING_WORKGROUP_SIZE);
        assert_eq!(plan.workgroup_count, 1024 / 64);

        let plan_partial = GpuDrivenCullingPassPlan::for_instance_count(65);
        assert_eq!(plan_partial.workgroup_count, 2);
    }

    #[test]
    fn hi_z_pyramid_default_is_conservative_and_requires_previous_frame() {
        let plan = GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT;
        assert_eq!(plan.policy, GpuDrivenOcclusionPolicy::Conservative);
        assert!(plan.previous_frame_required);
        assert!(plan.policy.classifies_uncertain_as_visible());
    }

    #[test]
    fn occlusion_policy_aggressive_drops_uncertain_visibility_classification() {
        let plan = GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT
            .with_policy(GpuDrivenOcclusionPolicy::Aggressive);
        assert!(plan.previous_frame_required);
        assert!(!plan.policy.classifies_uncertain_as_visible());
    }

    #[test]
    fn occlusion_disabled_for_debug_skips_previous_frame_requirement() {
        let plan = GpuDrivenHiZPyramidPlan::PRODUCT_DEFAULT
            .with_policy(GpuDrivenOcclusionPolicy::DisabledForDebug);
        assert!(!plan.previous_frame_required);
    }

    #[test]
    fn cluster_scaffold_routes_through_compute_fallback_when_mesh_shader_is_absent() {
        let no_mesh = GpuDrivenClusterScaffold::from_capabilities(false);
        let with_mesh = GpuDrivenClusterScaffold::from_capabilities(true);
        assert_eq!(no_mesh.path, GpuDrivenClusterPath::ComputeFallback);
        assert_eq!(
            with_mesh.path,
            GpuDrivenClusterPath::MeshShaderWhereSupported
        );
        assert!(no_mesh.uses_meshlet_metadata_placeholder);
        assert!(with_mesh.uses_cluster_bounds);
        assert!(with_mesh.uses_material_ranges);
    }

    #[test]
    fn debug_counter_slots_cover_tested_visible_rejected_uncertain_taxonomy() {
        let mut slots = [0u32; 8];
        slots[GpuDrivenDebugCounter::InstancesTested.slot() as usize] = 100;
        slots[GpuDrivenDebugCounter::InstancesVisible.slot() as usize] = 60;
        slots[GpuDrivenDebugCounter::InstancesFrustumRejected.slot() as usize] = 25;
        slots[GpuDrivenDebugCounter::InstancesOcclusionRejected.slot() as usize] = 10;
        slots[GpuDrivenDebugCounter::InstancesUncertain.slot() as usize] = 5;
        let snapshot = GpuDrivenDebugCounterSnapshot::from_slots(slots);

        assert_eq!(snapshot.instances_tested, 100);
        assert_eq!(snapshot.instances_visible, 60);
        assert_eq!(snapshot.instances_frustum_rejected, 25);
        assert_eq!(snapshot.instances_occlusion_rejected, 10);
        assert_eq!(snapshot.instances_uncertain, 5);
        assert!(snapshot.budget_consistent());
    }

    #[test]
    fn debug_counters_inconsistent_when_visible_plus_rejected_exceeds_tested() {
        let mut slots = [0u32; 8];
        slots[GpuDrivenDebugCounter::InstancesTested.slot() as usize] = 50;
        slots[GpuDrivenDebugCounter::InstancesVisible.slot() as usize] = 60;
        let snapshot = GpuDrivenDebugCounterSnapshot::from_slots(slots);
        assert!(!snapshot.budget_consistent());
    }

    #[test]
    fn product_plan_for_dx12_picks_execute_indirect_and_meshlet_compute_fallback_default() {
        let plan = GpuDrivenPlan::product_for_backend(NativeBackend::Dx12, 4096, false);

        assert_eq!(
            plan.indirect_path.command_signature,
            GpuDrivenIndirectCommandSignature::Dx12ExecuteIndirectDrawIndexed
        );
        assert_eq!(
            plan.cluster_scaffold.path,
            GpuDrivenClusterPath::ComputeFallback
        );
        assert_eq!(plan.frustum_culling.workgroup_count, 4096 / 64);
        assert!(plan.schedule_culling_before_draw());
    }

    #[test]
    fn parity_comparison_flags_mismatched_draw_or_triangle_counts() {
        let matched = GpuDrivenParityComparison::from_counts(120, 120, 240_000, 240_000);
        assert!(matched.matched);

        let draw_mismatch = GpuDrivenParityComparison::from_counts(120, 119, 240_000, 240_000);
        assert!(!draw_mismatch.matched);

        let triangle_mismatch = GpuDrivenParityComparison::from_counts(120, 120, 240_000, 239_900);
        assert!(!triangle_mismatch.matched);
    }

    #[test]
    fn metrics_aggregate_buffer_size_workgroup_count_and_optional_counters() {
        let plan = GpuDrivenPlan::product_for_backend(NativeBackend::Dx12, 256, true);
        let counters = GpuDrivenDebugCounterSnapshot::from_slots([10, 7, 2, 1, 0, 0, 0, 0]);
        let parity = GpuDrivenParityComparison::from_counts(7, 7, 1_400, 1_400);
        let metrics = GpuDrivenMetrics::from_plan(&plan)
            .with_debug_counters(counters)
            .with_parity(parity);

        assert_eq!(metrics.native_backend, NativeBackend::Dx12);
        assert_eq!(metrics.frustum_workgroup_count, 4);
        assert_eq!(metrics.debug_counters, counters);
        assert!(metrics.parity.matched);
        assert!(metrics.buffer_byte_size > 0);
    }

    #[test]
    fn buffer_set_total_byte_size_is_finite_and_dominated_by_indirect_args() {
        let set = GpuDrivenBufferSet::for_max_draw_count(1 << 16);
        let total = set.total_byte_size();
        assert!(total > 0);
        // Indirect arg buffer is the largest single contributor at 20 bytes per
        // draw, dwarfing the 4-byte material/object/visible-instance buffers
        // and the 4-byte single-draw-count slot.
        assert!(set.draw_arguments.byte_size() > set.visible_instance_list.byte_size());
    }
}
