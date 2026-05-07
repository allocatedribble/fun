pub const RENDERER_RESOURCE_SCHEMA_VERSION: u16 = 1;
pub const RESOURCE_DIAGNOSTIC_TOP_SITE_COUNT: usize = 4;
pub const PASS5_DX12_PARITY_ARTIFACT: &str = "target/dx12-parity/current/dx12_parity.funpb.zst";
pub const PASS5_UPLOAD_BENCHMARK_BASELINE_ARTIFACT: &str =
    "target/benchmarks/client/20260506-005505-031/benchmark.funpb.zst";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RendererResourceClass {
    Upload,
    Transient,
    Persistent,
    Imported,
    ReadbackDebug,
}

impl RendererResourceClass {
    pub const ALL: [Self; 5] = [
        Self::Upload,
        Self::Transient,
        Self::Persistent,
        Self::Imported,
        Self::ReadbackDebug,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upload => "upload",
            Self::Transient => "transient",
            Self::Persistent => "persistent",
            Self::Imported => "imported",
            Self::ReadbackDebug => "readback_debug",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UploadResourceKind {
    StagingBufferPages,
    RingAllocations,
    TransientUploadBatches,
}

impl UploadResourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StagingBufferPages => "staging_buffer_pages",
            Self::RingAllocations => "ring_allocations",
            Self::TransientUploadBatches => "transient_upload_batches",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TransientResourceKind {
    FrameLifetimeTextures,
    FrameLifetimeBuffers,
    PassLocalScratch,
}

impl TransientResourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrameLifetimeTextures => "frame_lifetime_textures",
            Self::FrameLifetimeBuffers => "frame_lifetime_buffers",
            Self::PassLocalScratch => "pass_local_scratch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PersistentResourceKind {
    MaterialTables,
    MeshTables,
    PagePools,
    ShadowPagePools,
    GiRadianceCaches,
    TextureResidencyPools,
}

impl PersistentResourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MaterialTables => "material_tables",
            Self::MeshTables => "mesh_tables",
            Self::PagePools => "page_pools",
            Self::ShadowPagePools => "shadow_page_pools",
            Self::GiRadianceCaches => "gi_radiance_caches",
            Self::TextureResidencyPools => "texture_residency_pools",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImportedResourceKind {
    CefSharedTextures,
    SwapchainResources,
    VendorSdkResources,
}

impl ImportedResourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CefSharedTextures => "cef_shared_textures",
            Self::SwapchainResources => "swapchain_resources",
            Self::VendorSdkResources => "vendor_sdk_resources",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReadbackDebugResourceKind {
    DiagnosticsReadback,
    Screenshots,
    BenchmarkCaptures,
}

impl ReadbackDebugResourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DiagnosticsReadback => "diagnostics_readback",
            Self::Screenshots => "screenshots",
            Self::BenchmarkCaptures => "benchmark_captures",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RendererResourceKind {
    Upload(UploadResourceKind),
    Transient(TransientResourceKind),
    Persistent(PersistentResourceKind),
    Imported(ImportedResourceKind),
    ReadbackDebug(ReadbackDebugResourceKind),
}

impl RendererResourceKind {
    #[must_use]
    pub const fn class(self) -> RendererResourceClass {
        match self {
            Self::Upload(_) => RendererResourceClass::Upload,
            Self::Transient(_) => RendererResourceClass::Transient,
            Self::Persistent(_) => RendererResourceClass::Persistent,
            Self::Imported(_) => RendererResourceClass::Imported,
            Self::ReadbackDebug(_) => RendererResourceClass::ReadbackDebug,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Upload(kind) => kind.as_str(),
            Self::Transient(kind) => kind.as_str(),
            Self::Persistent(kind) => kind.as_str(),
            Self::Imported(kind) => kind.as_str(),
            Self::ReadbackDebug(kind) => kind.as_str(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RendererResourceOwner {
    FunRenderer,
    FunRenderBridge,
    BevyGeneric,
    BevyLowLevel,
    GameClient,
    FunUiCef,
    VendorSdk,
}

impl RendererResourceOwner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunRenderer => crate::FUN_RENDERER_CRATE_NAME,
            Self::FunRenderBridge => crate::FUN_RENDER_BRIDGE_PACKAGE_NAME,
            Self::BevyGeneric => "bevy_generic_render_resource",
            Self::BevyLowLevel => "bevy_low_level",
            Self::GameClient => "game_client",
            Self::FunUiCef => "fun_ui_cef",
            Self::VendorSdk => "vendor_sdk",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceOwnershipPhase {
    #[default]
    ObserveOnly,
    BridgeCompatibilityShim,
    RendererOwnedPolicy,
    RendererOwnedAllocation,
}

impl ResourceOwnershipPhase {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ObserveOnly => "observe_only",
            Self::BridgeCompatibilityShim => "bridge_compatibility_shim",
            Self::RendererOwnedPolicy => "renderer_owned_policy",
            Self::RendererOwnedAllocation => "renderer_owned_allocation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceMigrationPriority {
    BenchmarkedHotUploadOffender,
    StaticTables,
    DynamicPerFrameUploads,
    RareDebugUploads,
}

impl ResourceMigrationPriority {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BenchmarkedHotUploadOffender => "benchmarked_hot_upload_offender",
            Self::StaticTables => "static_tables",
            Self::DynamicPerFrameUploads => "dynamic_per_frame_uploads",
            Self::RareDebugUploads => "rare_debug_uploads",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererResourceOwnershipPolicy {
    pub policy_owner: RendererResourceOwner,
    pub compatibility_shim_owner: RendererResourceOwner,
    pub hidden_transient_allocations_allowed_in_major_passes: bool,
    pub force_bevy_prepare_stage_helpers_through_upload_arena: bool,
    pub semantic_owner_required_before_generic_helper_migration: bool,
    pub top_small_buffer_offenders_are_generic_bevy_helpers: bool,
}

pub const RENDERER_RESOURCE_OWNERSHIP_POLICY: RendererResourceOwnershipPolicy =
    RendererResourceOwnershipPolicy {
        policy_owner: RendererResourceOwner::FunRenderer,
        compatibility_shim_owner: RendererResourceOwner::FunRenderBridge,
        hidden_transient_allocations_allowed_in_major_passes: false,
        force_bevy_prepare_stage_helpers_through_upload_arena: false,
        semantic_owner_required_before_generic_helper_migration: true,
        top_small_buffer_offenders_are_generic_bevy_helpers: true,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererResourceClassDescriptor {
    pub stable_id: &'static str,
    pub class: RendererResourceClass,
    pub owner: RendererResourceOwner,
    pub allocation_diagnostics_required: bool,
}

pub const RENDERER_RESOURCE_CLASS_DESCRIPTORS: [RendererResourceClassDescriptor; 5] = [
    RendererResourceClassDescriptor {
        stable_id: "fun_renderer.resource_class.upload",
        class: RendererResourceClass::Upload,
        owner: RendererResourceOwner::FunRenderer,
        allocation_diagnostics_required: true,
    },
    RendererResourceClassDescriptor {
        stable_id: "fun_renderer.resource_class.transient",
        class: RendererResourceClass::Transient,
        owner: RendererResourceOwner::FunRenderer,
        allocation_diagnostics_required: true,
    },
    RendererResourceClassDescriptor {
        stable_id: "fun_renderer.resource_class.persistent",
        class: RendererResourceClass::Persistent,
        owner: RendererResourceOwner::FunRenderer,
        allocation_diagnostics_required: true,
    },
    RendererResourceClassDescriptor {
        stable_id: "fun_renderer.resource_class.imported",
        class: RendererResourceClass::Imported,
        owner: RendererResourceOwner::FunRenderer,
        allocation_diagnostics_required: true,
    },
    RendererResourceClassDescriptor {
        stable_id: "fun_renderer.resource_class.readback_debug",
        class: RendererResourceClass::ReadbackDebug,
        owner: RendererResourceOwner::FunRenderer,
        allocation_diagnostics_required: true,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererResourceAllocationSite {
    pub stable_id: &'static str,
    pub module: &'static str,
    pub kind: RendererResourceKind,
    pub current_owner: RendererResourceOwner,
    pub intended_owner: RendererResourceOwner,
    pub priority: ResourceMigrationPriority,
    pub phase: ResourceOwnershipPhase,
    pub benchmark_artifact: &'static str,
    pub migration_gate: &'static str,
}

pub const FUN_UPLOAD_ARENA_ALLOCATION_SITE: RendererResourceAllocationSite =
    RendererResourceAllocationSite {
        stable_id: "fun_render.upload_arena.staging_belt",
        module: "fun_render::upload_arena",
        kind: RendererResourceKind::Upload(UploadResourceKind::StagingBufferPages),
        current_owner: RendererResourceOwner::FunRenderBridge,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::DynamicPerFrameUploads,
        phase: ResourceOwnershipPhase::BridgeCompatibilityShim,
        benchmark_artifact: PASS5_UPLOAD_BENCHMARK_BASELINE_ARTIFACT,
        migration_gate: "existing_command_encoder_and_semantic_owner_required",
    };

pub const HOT_UPLOAD_KILL_LIST: [RendererResourceAllocationSite; 4] = [
    RendererResourceAllocationSite {
        stable_id: "bevy.dynamic_uniform_buffer.write_buffer_with.uniform_buffer_311",
        module: "bevy_render::render_resource::DynamicUniformBuffer",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::BevyGeneric,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::BenchmarkedHotUploadOffender,
        phase: ResourceOwnershipPhase::ObserveOnly,
        benchmark_artifact: PASS5_DX12_PARITY_ARTIFACT,
        migration_gate: "split_semantic_owner_before_upload_arena_migration",
    },
    RendererResourceAllocationSite {
        stable_id: "bevy.raw_buffer_vec.write_buffer.buffer_vec_183",
        module: "bevy_render::render_resource::RawBufferVec",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::BevyGeneric,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::BenchmarkedHotUploadOffender,
        phase: ResourceOwnershipPhase::ObserveOnly,
        benchmark_artifact: PASS5_DX12_PARITY_ARTIFACT,
        migration_gate: "batch_or_split_semantic_owner_before_upload_arena_migration",
    },
    RendererResourceAllocationSite {
        stable_id: "bevy.dynamic_uniform_buffer.write_buffer.uniform_buffer_140",
        module: "bevy_render::render_resource::DynamicUniformBuffer",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::BevyGeneric,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::BenchmarkedHotUploadOffender,
        phase: ResourceOwnershipPhase::ObserveOnly,
        benchmark_artifact: PASS5_DX12_PARITY_ARTIFACT,
        migration_gate: "split_semantic_owner_before_upload_arena_migration",
    },
    RendererResourceAllocationSite {
        stable_id: "bevy.raw_buffer_vec.write_buffer.buffer_vec_442",
        module: "bevy_render::render_resource::RawBufferVec",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::BevyGeneric,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::BenchmarkedHotUploadOffender,
        phase: ResourceOwnershipPhase::ObserveOnly,
        benchmark_artifact: PASS5_DX12_PARITY_ARTIFACT,
        migration_gate: "batch_or_split_semantic_owner_before_upload_arena_migration",
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceHighWaterMarks {
    pub upload_bytes: u64,
    pub transient_bytes: u64,
    pub persistent_bytes: u64,
    pub imported_resource_count: u32,
    pub readback_bytes: u64,
}

impl ResourceHighWaterMarks {
    pub const ZERO: Self = Self {
        upload_bytes: 0,
        transient_bytes: 0,
        persistent_bytes: 0,
        imported_resource_count: 0,
        readback_bytes: 0,
    };
}

impl Default for ResourceHighWaterMarks {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceAllocationSiteSummary {
    pub stable_id: &'static str,
    pub kind: RendererResourceKind,
    pub bytes: u64,
    pub allocations: u32,
}

impl ResourceAllocationSiteSummary {
    pub const EMPTY: Self = Self {
        stable_id: "",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        bytes: 0,
        allocations: 0,
    };

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.stable_id.is_empty() && self.allocations == 0 && self.bytes == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceFrameAllocationDiagnostics {
    pub frame_index: u64,
    pub upload_bytes: u64,
    pub upload_allocation_count: u32,
    pub transient_bytes: u64,
    pub transient_allocation_count: u32,
    pub persistent_bytes: u64,
    pub persistent_allocation_count: u32,
    pub imported_resource_count: u32,
    pub readback_bytes: u64,
    pub readback_allocation_count: u32,
    pub high_water_marks: ResourceHighWaterMarks,
    pub top_allocation_sites: [ResourceAllocationSiteSummary; RESOURCE_DIAGNOSTIC_TOP_SITE_COUNT],
}

impl Default for ResourceFrameAllocationDiagnostics {
    fn default() -> Self {
        Self {
            frame_index: 0,
            upload_bytes: 0,
            upload_allocation_count: 0,
            transient_bytes: 0,
            transient_allocation_count: 0,
            persistent_bytes: 0,
            persistent_allocation_count: 0,
            imported_resource_count: 0,
            readback_bytes: 0,
            readback_allocation_count: 0,
            high_water_marks: ResourceHighWaterMarks::ZERO,
            top_allocation_sites: [ResourceAllocationSiteSummary::EMPTY;
                RESOURCE_DIAGNOSTIC_TOP_SITE_COUNT],
        }
    }
}

impl ResourceFrameAllocationDiagnostics {
    pub fn begin_frame(&mut self, frame_index: u64) {
        let high_water_marks = self.high_water_marks;
        *self = Self {
            frame_index,
            high_water_marks,
            ..Default::default()
        };
    }

    pub fn record_allocation(
        &mut self,
        stable_id: &'static str,
        kind: RendererResourceKind,
        bytes: u64,
        allocations: u32,
    ) {
        if allocations == 0 && bytes == 0 {
            return;
        }
        match kind.class() {
            RendererResourceClass::Upload => {
                self.upload_bytes = self.upload_bytes.saturating_add(bytes);
                self.upload_allocation_count =
                    self.upload_allocation_count.saturating_add(allocations);
                self.high_water_marks.upload_bytes =
                    self.high_water_marks.upload_bytes.max(self.upload_bytes);
            }
            RendererResourceClass::Transient => {
                self.transient_bytes = self.transient_bytes.saturating_add(bytes);
                self.transient_allocation_count =
                    self.transient_allocation_count.saturating_add(allocations);
                self.high_water_marks.transient_bytes = self
                    .high_water_marks
                    .transient_bytes
                    .max(self.transient_bytes);
            }
            RendererResourceClass::Persistent => {
                self.persistent_bytes = self.persistent_bytes.saturating_add(bytes);
                self.persistent_allocation_count =
                    self.persistent_allocation_count.saturating_add(allocations);
                self.high_water_marks.persistent_bytes = self
                    .high_water_marks
                    .persistent_bytes
                    .max(self.persistent_bytes);
            }
            RendererResourceClass::Imported => {
                self.imported_resource_count =
                    self.imported_resource_count.saturating_add(allocations);
                self.high_water_marks.imported_resource_count = self
                    .high_water_marks
                    .imported_resource_count
                    .max(self.imported_resource_count);
            }
            RendererResourceClass::ReadbackDebug => {
                self.readback_bytes = self.readback_bytes.saturating_add(bytes);
                self.readback_allocation_count =
                    self.readback_allocation_count.saturating_add(allocations);
                self.high_water_marks.readback_bytes = self
                    .high_water_marks
                    .readback_bytes
                    .max(self.readback_bytes);
            }
        }
        self.record_top_site(stable_id, kind, bytes, allocations);
    }

    fn record_top_site(
        &mut self,
        stable_id: &'static str,
        kind: RendererResourceKind,
        bytes: u64,
        allocations: u32,
    ) {
        if stable_id.is_empty() {
            return;
        }

        if let Some(site) = self
            .top_allocation_sites
            .iter_mut()
            .find(|site| site.stable_id == stable_id)
        {
            site.bytes = site.bytes.saturating_add(bytes);
            site.allocations = site.allocations.saturating_add(allocations);
            self.sort_top_sites();
            return;
        }

        if let Some(site) = self
            .top_allocation_sites
            .iter_mut()
            .find(|site| site.is_empty())
        {
            *site = ResourceAllocationSiteSummary {
                stable_id,
                kind,
                bytes,
                allocations,
            };
            self.sort_top_sites();
            return;
        }

        let mut replacement_index = 0usize;
        for index in 1..self.top_allocation_sites.len() {
            let candidate = self.top_allocation_sites[index];
            let replacement = self.top_allocation_sites[replacement_index];
            if candidate.bytes < replacement.bytes
                || (candidate.bytes == replacement.bytes
                    && candidate.allocations < replacement.allocations)
            {
                replacement_index = index;
            }
        }
        let replacement = self.top_allocation_sites[replacement_index];
        if bytes > replacement.bytes
            || (bytes == replacement.bytes && allocations > replacement.allocations)
        {
            self.top_allocation_sites[replacement_index] = ResourceAllocationSiteSummary {
                stable_id,
                kind,
                bytes,
                allocations,
            };
            self.sort_top_sites();
        }
    }

    fn sort_top_sites(&mut self) {
        let len = self.top_allocation_sites.len();
        let mut outer = 0usize;
        while outer < len {
            let mut inner = outer + 1;
            while inner < len {
                let left = self.top_allocation_sites[outer];
                let right = self.top_allocation_sites[inner];
                if site_should_sort_before(right, left) {
                    self.top_allocation_sites[outer] = right;
                    self.top_allocation_sites[inner] = left;
                }
                inner += 1;
            }
            outer += 1;
        }
    }
}

fn site_should_sort_before(
    left: ResourceAllocationSiteSummary,
    right: ResourceAllocationSiteSummary,
) -> bool {
    if left.is_empty() {
        return false;
    }
    if right.is_empty() {
        return true;
    }
    left.bytes > right.bytes
        || (left.bytes == right.bytes && left.allocations > right.allocations)
        || (left.bytes == right.bytes
            && left.allocations == right.allocations
            && left.stable_id < right.stable_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_classes_cover_pass5_taxonomy() {
        assert_eq!(RendererResourceClass::ALL.len(), 5);
        assert_eq!(RENDERER_RESOURCE_CLASS_DESCRIPTORS.len(), 5);
        assert!(
            RENDERER_RESOURCE_CLASS_DESCRIPTORS
                .iter()
                .all(
                    |descriptor| descriptor.owner == RendererResourceOwner::FunRenderer
                        && descriptor.allocation_diagnostics_required
                )
        );

        assert_eq!(
            RendererResourceKind::Upload(UploadResourceKind::RingAllocations).class(),
            RendererResourceClass::Upload
        );
        assert_eq!(
            RendererResourceKind::Transient(TransientResourceKind::PassLocalScratch).class(),
            RendererResourceClass::Transient
        );
        assert_eq!(
            RendererResourceKind::Persistent(PersistentResourceKind::GiRadianceCaches).class(),
            RendererResourceClass::Persistent
        );
        assert_eq!(
            RendererResourceKind::Imported(ImportedResourceKind::CefSharedTextures).class(),
            RendererResourceClass::Imported
        );
        assert_eq!(
            RendererResourceKind::ReadbackDebug(ReadbackDebugResourceKind::BenchmarkCaptures)
                .class(),
            RendererResourceClass::ReadbackDebug
        );
    }

    #[test]
    fn resource_policy_preserves_upload_arena_restraint() {
        let policy = core::hint::black_box(RENDERER_RESOURCE_OWNERSHIP_POLICY);

        assert_eq!(policy.policy_owner, RendererResourceOwner::FunRenderer);
        assert_eq!(
            policy.compatibility_shim_owner,
            RendererResourceOwner::FunRenderBridge
        );
        assert!(!policy.hidden_transient_allocations_allowed_in_major_passes);
        assert!(!policy.force_bevy_prepare_stage_helpers_through_upload_arena);
        assert!(policy.semantic_owner_required_before_generic_helper_migration);
        assert!(policy.top_small_buffer_offenders_are_generic_bevy_helpers);
    }

    #[test]
    fn resource_hot_upload_kill_list_names_bevy_generic_offenders_and_artifacts() {
        assert!(HOT_UPLOAD_KILL_LIST.iter().any(|site| {
            site.module == "bevy_render::render_resource::DynamicUniformBuffer"
                && site.benchmark_artifact == PASS5_DX12_PARITY_ARTIFACT
                && site.phase == ResourceOwnershipPhase::ObserveOnly
        }));
        assert!(HOT_UPLOAD_KILL_LIST.iter().any(|site| {
            site.module == "bevy_render::render_resource::RawBufferVec"
                && site.priority == ResourceMigrationPriority::BenchmarkedHotUploadOffender
                && site.migration_gate.contains("semantic_owner")
        }));
        assert_eq!(
            FUN_UPLOAD_ARENA_ALLOCATION_SITE.phase,
            ResourceOwnershipPhase::BridgeCompatibilityShim
        );
        assert_eq!(
            FUN_UPLOAD_ARENA_ALLOCATION_SITE.intended_owner,
            RendererResourceOwner::FunRenderer
        );
    }

    #[test]
    fn resource_frame_allocation_diagnostics_track_top_sites_and_high_water() {
        let mut diagnostics = ResourceFrameAllocationDiagnostics::default();
        diagnostics.record_allocation(
            "upload.hot",
            RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
            1024,
            2,
        );
        diagnostics.record_allocation(
            "transient.scratch",
            RendererResourceKind::Transient(TransientResourceKind::PassLocalScratch),
            4096,
            1,
        );
        diagnostics.record_allocation(
            "import.cef",
            RendererResourceKind::Imported(ImportedResourceKind::CefSharedTextures),
            0,
            3,
        );
        diagnostics.record_allocation(
            "upload.hot",
            RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
            512,
            1,
        );

        assert_eq!(diagnostics.upload_bytes, 1536);
        assert_eq!(diagnostics.upload_allocation_count, 3);
        assert_eq!(diagnostics.transient_bytes, 4096);
        assert_eq!(diagnostics.imported_resource_count, 3);
        assert_eq!(
            diagnostics.top_allocation_sites[0].stable_id,
            "transient.scratch"
        );
        assert_eq!(diagnostics.top_allocation_sites[1].stable_id, "upload.hot");
        assert_eq!(diagnostics.high_water_marks.upload_bytes, 1536);
        assert_eq!(diagnostics.high_water_marks.transient_bytes, 4096);

        diagnostics.begin_frame(9);
        assert_eq!(diagnostics.frame_index, 9);
        assert_eq!(diagnostics.upload_bytes, 0);
        assert_eq!(diagnostics.high_water_marks.upload_bytes, 1536);
    }
}
