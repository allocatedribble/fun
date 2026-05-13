use crate::{
    backend::{
        BindTableId as RendererBindTableId, MaterialBindLayout, MaterialId,
        RendererGenerationalIndex, SceneColorTextureUsage, TextureId, ViewBindLayout,
    },
    ir::{
        BindLayoutDesc as IrBindLayoutDesc, BindSlotDesc, BindTableDesc as IrBindTableDesc,
        BindingResourceRef, BindingType, BindlessTableDesc as IrBindlessTableDesc,
        DescriptorFingerprint, IrBindLayoutId, IrBindTableId, IrResourceKind, IrResourceRef,
        MATERIAL_BINDING_IR_SCHEMA_VERSION, MAX_BINDINGS_PER_LAYOUT, MAX_BINDINGS_PER_TABLE,
        MaterialBindingDesc, RootConstantDesc, SceneBindingDesc, ShaderStageMask, ViewBindingDesc,
    },
    resource::RendererResourceId,
};

pub const BINDING_SCHEMA_VERSION: u16 = 1;
pub const MAX_ROOT_CONSTANT_RANGES_PER_PASS: usize = 4;
pub const MAX_BINDLESS_TABLES_PER_PASS: usize = 4;

pub type ViewBindTableId = RendererBindTableId<ViewBindLayout>;
pub type MaterialBindTableId = RendererBindTableId<MaterialBindLayout>;
pub type StableTextureBindingId = TextureId<SceneColorTextureUsage>;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingDomain {
    #[default]
    View,
    Scene,
    Material,
    Pass,
    Bindless,
}

impl BindingDomain {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Scene => "scene",
            Self::Material => "material",
            Self::Pass => "pass",
            Self::Bindless => "bindless",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindLayout {
    pub schema_version: u16,
    pub id: IrBindLayoutId,
    pub stable_name: &'static str,
    pub domain: BindingDomain,
    pub slots: [BindSlotDesc; MAX_BINDINGS_PER_LAYOUT],
    pub slot_count: u8,
}

impl BindLayout {
    #[must_use]
    pub fn from_ir(desc: IrBindLayoutDesc, domain: BindingDomain) -> Self {
        Self {
            schema_version: desc.schema_version,
            id: desc.id,
            stable_name: desc.stable_name,
            domain,
            slots: desc.slots,
            slot_count: desc.slot_count,
        }
    }

    #[must_use]
    pub fn as_ir(self) -> IrBindLayoutDesc {
        IrBindLayoutDesc {
            id: self.id,
            schema_version: self.schema_version,
            stable_name: self.stable_name,
            slots: self.slots,
            slot_count: self.slot_count,
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        let mut words = [0u64; 4 + MAX_BINDINGS_PER_LAYOUT * 3];
        words[0] = u64::from(self.schema_version);
        words[1] = u64::from(self.id.0);
        words[2] = binding_domain_code(self.domain);
        words[3] = u64::from(self.slot_count);
        for (slot_index, slot) in self
            .slots
            .iter()
            .take(usize::from(self.slot_count))
            .enumerate()
        {
            let base = 4 + slot_index * 3;
            words[base] = u64::from(slot.binding);
            words[base + 1] = binding_type_code(slot.binding_type);
            words[base + 2] = u64::from(slot.stages.0);
        }
        DescriptorFingerprint::from_label_and_words(self.stable_name, &words)
    }
}

impl Default for BindLayout {
    fn default() -> Self {
        Self::from_ir(IrBindLayoutDesc::default(), BindingDomain::View)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindTableResource {
    pub binding: u8,
    pub binding_type: BindingType,
    pub renderer_resource: RendererResourceId,
    pub ir_resource: IrResourceRef,
}

impl BindTableResource {
    pub const INVALID: Self = Self {
        binding: 0,
        binding_type: BindingType::RootConstant,
        renderer_resource: RendererResourceId::INVALID,
        ir_resource: IrResourceRef::INVALID,
    };

    #[must_use]
    pub const fn from_ir(binding: BindingResourceRef) -> Self {
        Self {
            binding: binding.binding,
            binding_type: binding.binding_type,
            renderer_resource: RendererResourceId::INVALID,
            ir_resource: binding.resource,
        }
    }

    #[must_use]
    pub const fn with_renderer_resource(mut self, renderer_resource: RendererResourceId) -> Self {
        self.renderer_resource = renderer_resource;
        self
    }

    #[must_use]
    pub const fn as_ir(self) -> BindingResourceRef {
        BindingResourceRef {
            binding: self.binding,
            binding_type: self.binding_type,
            resource: self.ir_resource,
        }
    }
}

impl Default for BindTableResource {
    fn default() -> Self {
        Self::INVALID
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindTable {
    pub schema_version: u16,
    pub id: IrBindTableId,
    pub stable_name: &'static str,
    pub layout: IrBindLayoutId,
    pub domain: BindingDomain,
    pub bindings: [BindTableResource; MAX_BINDINGS_PER_TABLE],
    pub binding_count: u8,
}

impl BindTable {
    #[must_use]
    pub fn from_ir(desc: IrBindTableDesc, domain: BindingDomain) -> Self {
        let mut bindings = [BindTableResource::INVALID; MAX_BINDINGS_PER_TABLE];
        for (target, source) in bindings
            .iter_mut()
            .zip(desc.bindings.iter().take(usize::from(desc.binding_count)))
        {
            *target = BindTableResource::from_ir(*source);
        }
        Self {
            schema_version: desc.schema_version,
            id: desc.id,
            stable_name: desc.stable_name,
            layout: desc.layout,
            domain,
            bindings,
            binding_count: desc.binding_count,
        }
    }

    #[must_use]
    pub fn with_renderer_resources(mut self, resources: &[RendererResourceId]) -> Self {
        for (binding, resource) in self
            .bindings
            .iter_mut()
            .take(usize::from(self.binding_count))
            .zip(resources.iter().copied())
        {
            binding.renderer_resource = resource;
        }
        self
    }

    #[must_use]
    pub fn as_ir(self) -> IrBindTableDesc {
        let mut bindings = [BindingResourceRef::default(); MAX_BINDINGS_PER_TABLE];
        for (target, source) in bindings
            .iter_mut()
            .zip(self.bindings.iter().take(usize::from(self.binding_count)))
        {
            *target = source.as_ir();
        }
        IrBindTableDesc {
            id: self.id,
            schema_version: self.schema_version,
            stable_name: self.stable_name,
            layout: self.layout,
            bindings,
            binding_count: self.binding_count,
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        let mut words = [0u64; 5 + MAX_BINDINGS_PER_TABLE * 3];
        words[0] = u64::from(self.schema_version);
        words[1] = u64::from(self.id.0);
        words[2] = u64::from(self.layout.0);
        words[3] = binding_domain_code(self.domain);
        words[4] = u64::from(self.binding_count);
        for (binding_index, binding) in self
            .bindings
            .iter()
            .take(usize::from(self.binding_count))
            .enumerate()
        {
            let base = 5 + binding_index * 3;
            words[base] = u64::from(binding.binding);
            words[base + 1] = binding_type_code(binding.binding_type);
            words[base + 2] = resource_kind_code(binding.ir_resource.kind);
        }
        DescriptorFingerprint::from_label_and_words(self.stable_name, &words)
    }

    #[must_use]
    pub fn resource_hash(self) -> DescriptorFingerprint {
        let mut words = [0u64; 2 + MAX_BINDINGS_PER_TABLE * 6];
        words[0] = u64::from(self.id.0);
        words[1] = u64::from(self.binding_count);
        for (binding_index, binding) in self
            .bindings
            .iter()
            .take(usize::from(self.binding_count))
            .enumerate()
        {
            let raw = binding.renderer_resource.raw();
            let base = 2 + binding_index * 6;
            words[base] = u64::from(binding.binding);
            words[base + 1] = binding_type_code(binding.binding_type);
            words[base + 2] = resource_kind_code(binding.ir_resource.kind);
            words[base + 3] = u64::from(binding.ir_resource.index);
            words[base + 4] = u64::from(raw.slot);
            words[base + 5] = u64::from(raw.generation);
        }
        DescriptorFingerprint::from_label_and_words(self.stable_name, &words)
    }
}

impl Default for BindTable {
    fn default() -> Self {
        Self::from_ir(IrBindTableDesc::default(), BindingDomain::Pass)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingBackendFeature {
    DescriptorHeapTables,
    DescriptorSets,
    ArgumentBuffers,
    BindingArrays,
    PushConstants,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindlessTable {
    pub schema_version: u16,
    pub id: IrBindTableId,
    pub stable_name: &'static str,
    pub resource_kind: IrResourceKind,
    pub capacity: u32,
    pub resident_count: u32,
    pub feature_required: BindingBackendFeature,
}

impl BindlessTable {
    #[must_use]
    pub const fn from_ir(
        desc: IrBindlessTableDesc,
        feature_required: BindingBackendFeature,
    ) -> Self {
        Self {
            schema_version: desc.schema_version,
            id: desc.id,
            stable_name: desc.stable_name,
            resource_kind: desc.resource_kind,
            capacity: desc.capacity,
            resident_count: desc.resident_count,
            feature_required,
        }
    }

    #[must_use]
    pub const fn as_ir(self) -> IrBindlessTableDesc {
        IrBindlessTableDesc {
            id: self.id,
            schema_version: self.schema_version,
            stable_name: self.stable_name,
            resource_kind: self.resource_kind,
            capacity: self.capacity,
            resident_count: self.resident_count,
        }
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            self.stable_name,
            &[
                u64::from(self.schema_version),
                u64::from(self.id.0),
                resource_kind_code(self.resource_kind),
                u64::from(self.capacity),
                u64::from(self.resident_count),
                backend_feature_code(self.feature_required),
            ],
        )
    }
}

impl Default for BindlessTable {
    fn default() -> Self {
        Self::from_ir(
            IrBindlessTableDesc {
                id: IrBindTableId::INVALID,
                schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                stable_name: "",
                resource_kind: IrResourceKind::Texture,
                capacity: 0,
                resident_count: 0,
            },
            BindingBackendFeature::BindingArrays,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootConstants {
    pub schema_version: u16,
    pub ranges: [RootConstantDesc; MAX_ROOT_CONSTANT_RANGES_PER_PASS],
    pub range_count: u8,
}

impl RootConstants {
    #[must_use]
    pub fn from_ranges(ranges: &[RootConstantDesc]) -> Self {
        let mut stored_ranges = [RootConstantDesc::default(); MAX_ROOT_CONSTANT_RANGES_PER_PASS];
        for (target, source) in stored_ranges
            .iter_mut()
            .zip(ranges.iter().take(MAX_ROOT_CONSTANT_RANGES_PER_PASS))
        {
            *target = *source;
        }
        Self {
            schema_version: BINDING_SCHEMA_VERSION,
            ranges: stored_ranges,
            range_count: ranges.len().min(MAX_ROOT_CONSTANT_RANGES_PER_PASS) as u8,
        }
    }

    #[must_use]
    pub fn total_byte_count(self) -> u32 {
        self.ranges
            .iter()
            .take(usize::from(self.range_count))
            .map(|range| u32::from(range.byte_count))
            .sum()
    }

    #[must_use]
    pub fn stable_hash(self) -> DescriptorFingerprint {
        let mut words = [0u64; 2 + MAX_ROOT_CONSTANT_RANGES_PER_PASS * 4];
        words[0] = u64::from(self.schema_version);
        words[1] = u64::from(self.range_count);
        for (range_index, range) in self
            .ranges
            .iter()
            .take(usize::from(self.range_count))
            .enumerate()
        {
            let base = 2 + range_index * 4;
            words[base] = u64::from(range.slot);
            words[base + 1] = u64::from(range.byte_offset);
            words[base + 2] = u64::from(range.byte_count);
            words[base + 3] = u64::from(range.stages.0);
        }
        DescriptorFingerprint::from_label_and_words("root_constants", &words)
    }

    #[must_use]
    pub fn mapping(self, push_constants_supported: bool) -> RootConstantMapping {
        if push_constants_supported {
            RootConstantMapping::NativePushConstants
        } else {
            RootConstantMapping::UniformFallback(RootConstantFallbackCost {
                fallback_uniform_bytes: self.total_byte_count(),
                extra_bindings: u8::from(self.range_count > 0),
                dynamic_update_required: self.range_count > 0,
            })
        }
    }
}

impl Default for RootConstants {
    fn default() -> Self {
        Self::from_ranges(&[])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootConstantFallbackCost {
    pub fallback_uniform_bytes: u32,
    pub extra_bindings: u8,
    pub dynamic_update_required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootConstantMapping {
    NativePushConstants,
    UniformFallback(RootConstantFallbackCost),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewBindings {
    pub desc: ViewBindingDesc,
    pub prepared_table: Option<ViewBindTableId>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneBindings {
    pub desc: SceneBindingDesc,
    pub prepared_table: Option<ViewBindTableId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialBindings {
    pub desc: MaterialBindingDesc,
    pub material_id: MaterialId,
    pub prepared_table: Option<MaterialBindTableId>,
    pub stable_texture_id: Option<StableTextureBindingId>,
}

impl Default for MaterialBindings {
    fn default() -> Self {
        Self {
            desc: MaterialBindingDesc::default(),
            material_id: MaterialId::INVALID,
            prepared_table: None,
            stable_texture_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassBindings {
    pub schema_version: u16,
    pub view: ViewBindings,
    pub scene: SceneBindings,
    pub material: MaterialBindings,
    pub pass_table: Option<BindTable>,
    pub root_constants: RootConstants,
    pub bindless_tables: [BindlessTable; MAX_BINDLESS_TABLES_PER_PASS],
    pub bindless_table_count: u8,
}

impl PassBindings {
    #[must_use]
    pub fn from_parts(
        view: ViewBindings,
        scene: SceneBindings,
        material: MaterialBindings,
        root_constants: RootConstants,
    ) -> Self {
        Self {
            schema_version: BINDING_SCHEMA_VERSION,
            view,
            scene,
            material,
            pass_table: None,
            root_constants,
            bindless_tables: [BindlessTable::default(); MAX_BINDLESS_TABLES_PER_PASS],
            bindless_table_count: 0,
        }
    }

    #[must_use]
    pub fn with_pass_table(mut self, table: BindTable) -> Self {
        self.pass_table = Some(table);
        self
    }

    #[must_use]
    pub fn with_bindless_tables(mut self, tables: &[BindlessTable]) -> Self {
        self.bindless_table_count = tables.len().min(MAX_BINDLESS_TABLES_PER_PASS) as u8;
        for (target, source) in self.bindless_tables.iter_mut().zip(tables.iter().copied()) {
            *target = source;
        }
        self
    }
}

impl Default for PassBindings {
    fn default() -> Self {
        Self::from_parts(
            ViewBindings::default(),
            SceneBindings::default(),
            MaterialBindings::default(),
            RootConstants::default(),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingBridgeKind {
    Wgpu,
    DirectDx12,
    DirectVulkan,
    DirectMetal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingLayoutRealization {
    WgpuBindGroupLayout,
    Dx12RootSignatureAndDescriptorTables,
    VulkanPipelineLayoutAndDescriptorSetLayouts,
    MetalArgumentBufferLayouts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingTableRealization {
    WgpuBindGroup,
    Dx12DescriptorHeapTable,
    VulkanDescriptorSet,
    MetalArgumentBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RootConstantsRealization {
    WgpuPushConstantsOrUniformFallback,
    Dx12RootConstants,
    VulkanPushConstants,
    MetalInlineConstantsOrArgumentBufferFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindlessRealization {
    WgpuBindingArrays,
    Dx12DescriptorHeapRange,
    VulkanDescriptorIndexing,
    MetalArgumentBufferArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendBindingMapping {
    pub bridge_kind: BindingBridgeKind,
    pub layout: BindingLayoutRealization,
    pub table: BindingTableRealization,
    pub root_constants: RootConstantsRealization,
    pub bindless: BindlessRealization,
    pub cache_key: BindingCacheKeyModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingCacheKeyModel {
    pub stable_layout_hash: bool,
    pub stable_table_hash: bool,
    pub dirty_resource_hash: bool,
    pub warmup_only_creation: bool,
}

impl BindingCacheKeyModel {
    pub const PRODUCTION: Self = Self {
        stable_layout_hash: true,
        stable_table_hash: true,
        dirty_resource_hash: true,
        warmup_only_creation: true,
    };
}

pub const WGPU_BINDING_MAPPING: BackendBindingMapping = BackendBindingMapping {
    bridge_kind: BindingBridgeKind::Wgpu,
    layout: BindingLayoutRealization::WgpuBindGroupLayout,
    table: BindingTableRealization::WgpuBindGroup,
    root_constants: RootConstantsRealization::WgpuPushConstantsOrUniformFallback,
    bindless: BindlessRealization::WgpuBindingArrays,
    cache_key: BindingCacheKeyModel::PRODUCTION,
};

pub const DX12_BINDING_MAPPING: BackendBindingMapping = BackendBindingMapping {
    bridge_kind: BindingBridgeKind::DirectDx12,
    layout: BindingLayoutRealization::Dx12RootSignatureAndDescriptorTables,
    table: BindingTableRealization::Dx12DescriptorHeapTable,
    root_constants: RootConstantsRealization::Dx12RootConstants,
    bindless: BindlessRealization::Dx12DescriptorHeapRange,
    cache_key: BindingCacheKeyModel::PRODUCTION,
};

pub const VULKAN_BINDING_MAPPING: BackendBindingMapping = BackendBindingMapping {
    bridge_kind: BindingBridgeKind::DirectVulkan,
    layout: BindingLayoutRealization::VulkanPipelineLayoutAndDescriptorSetLayouts,
    table: BindingTableRealization::VulkanDescriptorSet,
    root_constants: RootConstantsRealization::VulkanPushConstants,
    bindless: BindlessRealization::VulkanDescriptorIndexing,
    cache_key: BindingCacheKeyModel::PRODUCTION,
};

pub const METAL_BINDING_MAPPING: BackendBindingMapping = BackendBindingMapping {
    bridge_kind: BindingBridgeKind::DirectMetal,
    layout: BindingLayoutRealization::MetalArgumentBufferLayouts,
    table: BindingTableRealization::MetalArgumentBuffer,
    root_constants: RootConstantsRealization::MetalInlineConstantsOrArgumentBufferFallback,
    bindless: BindlessRealization::MetalArgumentBufferArray,
    cache_key: BindingCacheKeyModel::PRODUCTION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingTranslationPhase {
    Warmup,
    RuntimeMeasured,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingCacheKind {
    #[default]
    Layout,
    Table,
    BindlessTable,
    RootConstants,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedBindingHandle {
    pub kind: BindingCacheKind,
    pub raw: RendererGenerationalIndex,
}

impl PreparedBindingHandle {
    #[must_use]
    pub const fn first(kind: BindingCacheKind, slot: u32) -> Self {
        Self {
            kind,
            raw: RendererGenerationalIndex::first(slot),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingCacheEntry {
    pub kind: BindingCacheKind,
    pub handle: PreparedBindingHandle,
    pub stable_hash: DescriptorFingerprint,
    pub resource_hash: DescriptorFingerprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingCacheDecision {
    Created(PreparedBindingHandle),
    Reused(PreparedBindingHandle),
    DirtyUpdated(PreparedBindingHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingCacheError {
    RuntimeCreationAfterWarmup {
        kind: BindingCacheKind,
        stable_hash: DescriptorFingerprint,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingChurnTelemetry {
    pub frame_index: u64,
    pub layouts_created: u32,
    pub layouts_reused: u32,
    pub tables_created: u32,
    pub tables_reused: u32,
    pub bindless_tables_created: u32,
    pub bindless_tables_reused: u32,
    pub dirty_table_updates: u32,
    pub root_constant_fallbacks: u32,
    pub runtime_creation_failures: u32,
    pub per_entity_descriptor_allocations_rejected: u32,
    pub per_draw_bind_group_creations_rejected: u32,
    pub stable_material_updates: u32,
    pub stable_texture_updates: u32,
    pub prepared_binding_handle_draw_refs: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingChurnPolicy {
    pub per_entity_descriptor_allocation_allowed: bool,
    pub per_draw_bind_group_creation_allowed: bool,
    pub draw_packets_reference_prepared_binding_handles: bool,
    pub material_ids_stable: bool,
    pub texture_ids_stable: bool,
}

impl BindingChurnPolicy {
    pub const PRODUCTION: Self = Self {
        per_entity_descriptor_allocation_allowed: false,
        per_draw_bind_group_creation_allowed: false,
        draw_packets_reference_prepared_binding_handles: true,
        material_ids_stable: true,
        texture_ids_stable: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DescriptorChurnRejection {
    PerEntityDescriptorAllocation,
    PerDrawBindGroupCreation,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BindingTranslationCache {
    entries: Vec<BindingCacheEntry>,
    pub telemetry: BindingChurnTelemetry,
}

impl BindingTranslationCache {
    #[must_use]
    pub fn entries(&self) -> &[BindingCacheEntry] {
        &self.entries
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.telemetry.frame_index = frame_index;
    }

    pub fn ensure_layout(
        &mut self,
        layout: &BindLayout,
        phase: BindingTranslationPhase,
    ) -> Result<BindingCacheDecision, BindingCacheError> {
        self.ensure_entry(
            BindingCacheKind::Layout,
            layout.id.0,
            layout.stable_hash(),
            DescriptorFingerprint::default(),
            phase,
        )
    }

    pub fn ensure_table(
        &mut self,
        table: &BindTable,
        phase: BindingTranslationPhase,
    ) -> Result<BindingCacheDecision, BindingCacheError> {
        self.ensure_entry(
            BindingCacheKind::Table,
            table.id.0,
            table.stable_hash(),
            table.resource_hash(),
            phase,
        )
    }

    pub fn ensure_bindless_table(
        &mut self,
        table: &BindlessTable,
        phase: BindingTranslationPhase,
    ) -> Result<BindingCacheDecision, BindingCacheError> {
        self.ensure_entry(
            BindingCacheKind::BindlessTable,
            table.id.0,
            table.stable_hash(),
            DescriptorFingerprint::default(),
            phase,
        )
    }

    pub fn record_root_constant_mapping(&mut self, mapping: RootConstantMapping) {
        if matches!(mapping, RootConstantMapping::UniformFallback(_)) {
            self.telemetry.root_constant_fallbacks += 1;
        }
    }

    pub fn record_rejection(&mut self, rejection: DescriptorChurnRejection) {
        match rejection {
            DescriptorChurnRejection::PerEntityDescriptorAllocation => {
                self.telemetry.per_entity_descriptor_allocations_rejected += 1;
            }
            DescriptorChurnRejection::PerDrawBindGroupCreation => {
                self.telemetry.per_draw_bind_group_creations_rejected += 1;
            }
        }
    }

    fn ensure_entry(
        &mut self,
        kind: BindingCacheKind,
        slot: u32,
        stable_hash: DescriptorFingerprint,
        resource_hash: DescriptorFingerprint,
        phase: BindingTranslationPhase,
    ) -> Result<BindingCacheDecision, BindingCacheError> {
        if let Some(entry_index) = self
            .entries
            .iter()
            .position(|entry| entry.kind == kind && entry.handle.raw.slot == slot)
        {
            let entry = &mut self.entries[entry_index];
            if entry.stable_hash == stable_hash && entry.resource_hash == resource_hash {
                increment_reuse_counter(&mut self.telemetry, kind);
                return Ok(BindingCacheDecision::Reused(entry.handle));
            }
            if entry.stable_hash == stable_hash && kind == BindingCacheKind::Table {
                entry.resource_hash = resource_hash;
                self.telemetry.dirty_table_updates += 1;
                return Ok(BindingCacheDecision::DirtyUpdated(entry.handle));
            }
            if phase == BindingTranslationPhase::RuntimeMeasured {
                self.telemetry.runtime_creation_failures += 1;
                return Err(BindingCacheError::RuntimeCreationAfterWarmup { kind, stable_hash });
            }
            entry.stable_hash = stable_hash;
            entry.resource_hash = resource_hash;
            increment_create_counter(&mut self.telemetry, kind);
            return Ok(BindingCacheDecision::Created(entry.handle));
        }

        if phase == BindingTranslationPhase::RuntimeMeasured {
            self.telemetry.runtime_creation_failures += 1;
            return Err(BindingCacheError::RuntimeCreationAfterWarmup { kind, stable_hash });
        }

        let handle = PreparedBindingHandle::first(kind, slot);
        self.entries.push(BindingCacheEntry {
            kind,
            handle,
            stable_hash,
            resource_hash,
        });
        increment_create_counter(&mut self.telemetry, kind);
        Ok(BindingCacheDecision::Created(handle))
    }
}

fn increment_create_counter(telemetry: &mut BindingChurnTelemetry, kind: BindingCacheKind) {
    match kind {
        BindingCacheKind::Layout => telemetry.layouts_created += 1,
        BindingCacheKind::Table => telemetry.tables_created += 1,
        BindingCacheKind::BindlessTable => telemetry.bindless_tables_created += 1,
        BindingCacheKind::RootConstants => {}
    }
}

fn increment_reuse_counter(telemetry: &mut BindingChurnTelemetry, kind: BindingCacheKind) {
    match kind {
        BindingCacheKind::Layout => telemetry.layouts_reused += 1,
        BindingCacheKind::Table => telemetry.tables_reused += 1,
        BindingCacheKind::BindlessTable => telemetry.bindless_tables_reused += 1,
        BindingCacheKind::RootConstants => {}
    }
}

const fn binding_domain_code(domain: BindingDomain) -> u64 {
    match domain {
        BindingDomain::View => 0,
        BindingDomain::Scene => 1,
        BindingDomain::Material => 2,
        BindingDomain::Pass => 3,
        BindingDomain::Bindless => 4,
    }
}

const fn binding_type_code(binding_type: BindingType) -> u64 {
    match binding_type {
        BindingType::UniformBuffer => 0,
        BindingType::StorageBuffer => 1,
        BindingType::SampledTexture => 2,
        BindingType::StorageTexture => 3,
        BindingType::Sampler => 4,
        BindingType::RootConstant => 5,
    }
}

const fn resource_kind_code(kind: IrResourceKind) -> u64 {
    match kind {
        IrResourceKind::Buffer => 0,
        IrResourceKind::Texture => 1,
        IrResourceKind::Sampler => 2,
        IrResourceKind::RenderTarget => 3,
        IrResourceKind::DepthTarget => 4,
        IrResourceKind::Transient => 5,
        IrResourceKind::Imported => 6,
    }
}

const fn backend_feature_code(feature: BindingBackendFeature) -> u64 {
    match feature {
        BindingBackendFeature::DescriptorHeapTables => 0,
        BindingBackendFeature::DescriptorSets => 1,
        BindingBackendFeature::ArgumentBuffers => 2,
        BindingBackendFeature::BindingArrays => 3,
        BindingBackendFeature::PushConstants => 4,
    }
}

#[must_use]
pub const fn root_constant_slot(
    slot: u8,
    byte_offset: u16,
    byte_count: u16,
    stages: ShaderStageMask,
) -> RootConstantDesc {
    RootConstantDesc {
        slot,
        byte_offset,
        byte_count,
        stages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{IrBufferId, IrTextureId};

    fn material_layout() -> BindLayout {
        BindLayout::from_ir(
            IrBindLayoutDesc {
                id: IrBindLayoutId::new(2),
                schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                stable_name: "material.standard.layout",
                slots: [
                    BindSlotDesc {
                        binding: 0,
                        binding_type: BindingType::UniformBuffer,
                        stages: ShaderStageMask::ALL_GRAPHICS,
                    },
                    BindSlotDesc {
                        binding: 1,
                        binding_type: BindingType::SampledTexture,
                        stages: ShaderStageMask::FRAGMENT,
                    },
                    BindSlotDesc {
                        binding: 2,
                        binding_type: BindingType::Sampler,
                        stages: ShaderStageMask::FRAGMENT,
                    },
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                ],
                slot_count: 3,
            },
            BindingDomain::Material,
        )
    }

    fn material_table(texture: RendererResourceId) -> BindTable {
        BindTable::from_ir(
            IrBindTableDesc {
                id: IrBindTableId::new(7),
                schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                stable_name: "material.standard.table",
                layout: IrBindLayoutId::new(2),
                bindings: [
                    BindingResourceRef {
                        binding: 0,
                        binding_type: BindingType::UniformBuffer,
                        resource: IrResourceRef::buffer(IrBufferId::new(4)),
                    },
                    BindingResourceRef {
                        binding: 1,
                        binding_type: BindingType::SampledTexture,
                        resource: IrResourceRef::texture(IrTextureId::new(5)),
                    },
                    BindingResourceRef {
                        binding: 2,
                        binding_type: BindingType::Sampler,
                        resource: IrResourceRef::sampler(crate::ir::IrSamplerId::new(6)),
                    },
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                ],
                binding_count: 3,
            },
            BindingDomain::Material,
        )
        .with_renderer_resources(&[
            RendererResourceId::first(10),
            texture,
            RendererResourceId::first(12),
        ])
    }

    #[test]
    fn high_level_binding_model_covers_pass11_contract() {
        let layout = material_layout();
        let table = material_table(RendererResourceId::first(11));
        let root_constants = RootConstants::from_ranges(&[root_constant_slot(
            0,
            0,
            16,
            ShaderStageMask::ALL_GRAPHICS,
        )]);
        let material_bindings = MaterialBindings {
            desc: MaterialBindingDesc {
                material_table: table.id,
                material_layout: layout.id,
                material_schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                feature_mask_bits: 3,
            },
            material_id: MaterialId::first(9),
            prepared_table: Some(MaterialBindTableId::first(7)),
            stable_texture_id: Some(StableTextureBindingId::first(11)),
        };
        let pass_bindings = PassBindings::from_parts(
            ViewBindings::default(),
            SceneBindings::default(),
            material_bindings,
            root_constants,
        )
        .with_pass_table(table);

        assert_eq!(layout.domain, BindingDomain::Material);
        assert_eq!(table.as_ir().binding_count, 3);
        assert_eq!(pass_bindings.material.material_id.slot(), 9);
        assert_eq!(
            pass_bindings.material.stable_texture_id.map(|id| id.slot()),
            Some(11)
        );
        assert_ne!(layout.stable_hash(), DescriptorFingerprint::default());
        assert_ne!(table.resource_hash(), DescriptorFingerprint::default());
        assert_eq!(
            root_constants.mapping(false),
            RootConstantMapping::UniformFallback(RootConstantFallbackCost {
                fallback_uniform_bytes: 16,
                extra_bindings: 1,
                dynamic_update_required: true,
            })
        );
    }

    #[test]
    fn binding_cache_reuses_unchanged_and_dirty_updates_changed_table() {
        let layout = material_layout();
        let table = material_table(RendererResourceId::first(11));
        let changed_table = material_table(RendererResourceId::new(11, 2));
        let mut cache = BindingTranslationCache::default();

        assert!(matches!(
            cache.ensure_layout(&layout, BindingTranslationPhase::Warmup),
            Ok(BindingCacheDecision::Created(_))
        ));
        assert!(matches!(
            cache.ensure_layout(&layout, BindingTranslationPhase::RuntimeMeasured),
            Ok(BindingCacheDecision::Reused(_))
        ));
        assert!(matches!(
            cache.ensure_table(&table, BindingTranslationPhase::Warmup),
            Ok(BindingCacheDecision::Created(_))
        ));
        assert!(matches!(
            cache.ensure_table(&table, BindingTranslationPhase::RuntimeMeasured),
            Ok(BindingCacheDecision::Reused(_))
        ));
        assert!(matches!(
            cache.ensure_table(&changed_table, BindingTranslationPhase::RuntimeMeasured),
            Ok(BindingCacheDecision::DirtyUpdated(_))
        ));

        assert_eq!(cache.telemetry.layouts_created, 1);
        assert_eq!(cache.telemetry.layouts_reused, 1);
        assert_eq!(cache.telemetry.tables_created, 1);
        assert_eq!(cache.telemetry.tables_reused, 1);
        assert_eq!(cache.telemetry.dirty_table_updates, 1);
    }

    #[test]
    fn binding_cache_blocks_measured_runtime_creation_after_warmup() {
        let mut cache = BindingTranslationCache::default();
        let error = cache
            .ensure_layout(&material_layout(), BindingTranslationPhase::RuntimeMeasured)
            .expect_err("measured runtime creation must fail closed");

        assert!(matches!(
            error,
            BindingCacheError::RuntimeCreationAfterWarmup {
                kind: BindingCacheKind::Layout,
                ..
            }
        ));
        assert_eq!(cache.telemetry.runtime_creation_failures, 1);
    }

    #[test]
    fn descriptor_churn_policy_forbids_per_entity_and_per_draw_allocation() {
        let policy = BindingChurnPolicy::PRODUCTION;
        let mut cache = BindingTranslationCache::default();

        assert!(!policy.per_entity_descriptor_allocation_allowed);
        assert!(!policy.per_draw_bind_group_creation_allowed);
        assert!(policy.draw_packets_reference_prepared_binding_handles);
        assert!(policy.material_ids_stable);
        assert!(policy.texture_ids_stable);

        cache.record_rejection(DescriptorChurnRejection::PerEntityDescriptorAllocation);
        cache.record_rejection(DescriptorChurnRejection::PerDrawBindGroupCreation);

        assert_eq!(
            cache.telemetry.per_entity_descriptor_allocations_rejected,
            1
        );
        assert_eq!(cache.telemetry.per_draw_bind_group_creations_rejected, 1);
    }

    #[test]
    fn backend_mapping_keeps_native_descriptor_models_explicit() {
        assert_eq!(
            WGPU_BINDING_MAPPING.table,
            BindingTableRealization::WgpuBindGroup
        );
        assert_eq!(
            DX12_BINDING_MAPPING.table,
            BindingTableRealization::Dx12DescriptorHeapTable
        );
        assert_eq!(
            VULKAN_BINDING_MAPPING.table,
            BindingTableRealization::VulkanDescriptorSet
        );
        assert_eq!(
            METAL_BINDING_MAPPING.table,
            BindingTableRealization::MetalArgumentBuffer
        );
        const { assert!(DX12_BINDING_MAPPING.cache_key.warmup_only_creation) };
    }
}
