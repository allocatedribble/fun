use crate::{
    binding::{
        BindLayout, BindTable, BindingBackendFeature, BindingCacheDecision, BindingCacheError,
        BindingTranslationCache, BindingTranslationPhase, BindlessTable, RootConstantMapping,
        RootConstants, WGPU_BINDING_MAPPING,
    },
    ir::{
        BindLayoutDesc, BindSlotDesc, BindingType, DescriptorFingerprint, IrBindLayoutId,
        IrBindTableId, IrResourceKind, ShaderStageMask,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedWgpuBindLayoutDescriptor {
    pub stable_name: &'static str,
    pub entries: [::wgpu::BindGroupLayoutEntry; crate::ir::MAX_BINDINGS_PER_LAYOUT],
    pub entry_count: u8,
}

impl PreparedWgpuBindLayoutDescriptor {
    #[must_use]
    pub fn as_wgpu(&self) -> ::wgpu::BindGroupLayoutDescriptor<'_> {
        ::wgpu::BindGroupLayoutDescriptor {
            label: Some(self.stable_name),
            entries: &self.entries[..usize::from(self.entry_count)],
        }
    }
}

#[must_use]
pub fn bind_layout_descriptor(desc: BindLayoutDesc) -> PreparedWgpuBindLayoutDescriptor {
    let mut entries = [empty_layout_entry(); crate::ir::MAX_BINDINGS_PER_LAYOUT];
    for (target, source) in entries
        .iter_mut()
        .zip(desc.slots.iter().take(desc.slot_count as usize))
    {
        *target = bind_group_layout_entry(*source);
    }
    PreparedWgpuBindLayoutDescriptor {
        stable_name: desc.stable_name,
        entries,
        entry_count: desc.slot_count,
    }
}

#[must_use]
pub fn bind_layout_descriptor_from_model(layout: BindLayout) -> PreparedWgpuBindLayoutDescriptor {
    bind_layout_descriptor(layout.as_ir())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedWgpuBindTableDescriptor {
    pub stable_name: &'static str,
    pub table: IrBindTableId,
    pub layout: IrBindLayoutId,
    pub stable_table_hash: DescriptorFingerprint,
    pub resource_hash: DescriptorFingerprint,
    pub resource_count: u8,
    pub maps_to_wgpu_bind_group: bool,
}

#[must_use]
pub fn bind_table_descriptor_from_model(table: BindTable) -> PreparedWgpuBindTableDescriptor {
    PreparedWgpuBindTableDescriptor {
        stable_name: table.stable_name,
        table: table.id,
        layout: table.layout,
        stable_table_hash: table.stable_hash(),
        resource_hash: table.resource_hash(),
        resource_count: table.binding_count,
        maps_to_wgpu_bind_group: true,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuBindingCapabilities {
    pub push_constants: bool,
    pub binding_arrays: bool,
}

impl WgpuBindingCapabilities {
    pub const CONSERVATIVE: Self = Self {
        push_constants: false,
        binding_arrays: false,
    };

    pub const FULL: Self = Self {
        push_constants: true,
        binding_arrays: true,
    };
}

impl Default for WgpuBindingCapabilities {
    fn default() -> Self {
        Self::CONSERVATIVE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuRootConstantPlan {
    pub mapping: RootConstantMapping,
    pub requires_push_constants_feature: bool,
}

#[must_use]
pub fn root_constant_plan(
    root_constants: RootConstants,
    capabilities: WgpuBindingCapabilities,
) -> WgpuRootConstantPlan {
    WgpuRootConstantPlan {
        mapping: root_constants.mapping(capabilities.push_constants),
        requires_push_constants_feature: capabilities.push_constants,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindlessTableUnavailableReason {
    BindingArraysUnsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuBindlessTablePlan {
    pub table: IrBindTableId,
    pub table_hash: DescriptorFingerprint,
    pub resource_kind: IrResourceKind,
    pub capacity: u32,
    pub resident_count: u32,
    pub feature_required: BindingBackendFeature,
    pub maps_to_binding_array: bool,
    pub unavailable_reason: Option<BindlessTableUnavailableReason>,
}

#[must_use]
pub fn bindless_table_plan(
    table: BindlessTable,
    capabilities: WgpuBindingCapabilities,
) -> WgpuBindlessTablePlan {
    WgpuBindlessTablePlan {
        table: table.id,
        table_hash: table.stable_hash(),
        resource_kind: table.resource_kind,
        capacity: table.capacity,
        resident_count: table.resident_count,
        feature_required: table.feature_required,
        maps_to_binding_array: capabilities.binding_arrays,
        unavailable_reason: if capabilities.binding_arrays {
            None
        } else {
            Some(BindlessTableUnavailableReason::BindingArraysUnsupported)
        },
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WgpuBindingBridgeCache {
    pub cache: BindingTranslationCache,
}

impl WgpuBindingBridgeCache {
    pub fn translate_layout(
        &mut self,
        layout: BindLayout,
        phase: BindingTranslationPhase,
    ) -> Result<(PreparedWgpuBindLayoutDescriptor, BindingCacheDecision), BindingCacheError> {
        let decision = self.cache.ensure_layout(&layout, phase)?;
        Ok((bind_layout_descriptor_from_model(layout), decision))
    }

    pub fn translate_table(
        &mut self,
        table: BindTable,
        phase: BindingTranslationPhase,
    ) -> Result<(PreparedWgpuBindTableDescriptor, BindingCacheDecision), BindingCacheError> {
        let decision = self.cache.ensure_table(&table, phase)?;
        Ok((bind_table_descriptor_from_model(table), decision))
    }

    pub fn translate_bindless_table(
        &mut self,
        table: BindlessTable,
        capabilities: WgpuBindingCapabilities,
        phase: BindingTranslationPhase,
    ) -> Result<(WgpuBindlessTablePlan, Option<BindingCacheDecision>), BindingCacheError> {
        let plan = bindless_table_plan(table, capabilities);
        if !plan.maps_to_binding_array {
            return Ok((plan, None));
        }
        let decision = self.cache.ensure_bindless_table(&table, phase)?;
        Ok((plan, Some(decision)))
    }

    pub fn translate_root_constants(
        &mut self,
        root_constants: RootConstants,
        capabilities: WgpuBindingCapabilities,
    ) -> WgpuRootConstantPlan {
        let plan = root_constant_plan(root_constants, capabilities);
        self.cache.record_root_constant_mapping(plan.mapping);
        plan
    }

    #[must_use]
    pub const fn mapping_contract(&self) -> crate::binding::BackendBindingMapping {
        WGPU_BINDING_MAPPING
    }
}

#[must_use]
pub fn bind_group_layout_entry(slot: BindSlotDesc) -> ::wgpu::BindGroupLayoutEntry {
    ::wgpu::BindGroupLayoutEntry {
        binding: u32::from(slot.binding),
        visibility: shader_stages(slot.stages),
        ty: binding_type(slot.binding_type),
        count: None,
    }
}

#[must_use]
pub fn shader_stages(stages: ShaderStageMask) -> ::wgpu::ShaderStages {
    let mut wgpu_stages = ::wgpu::ShaderStages::empty();
    if stages.contains(ShaderStageMask::VERTEX) {
        wgpu_stages |= ::wgpu::ShaderStages::VERTEX;
    }
    if stages.contains(ShaderStageMask::FRAGMENT) {
        wgpu_stages |= ::wgpu::ShaderStages::FRAGMENT;
    }
    if stages.contains(ShaderStageMask::COMPUTE) {
        wgpu_stages |= ::wgpu::ShaderStages::COMPUTE;
    }
    if stages.contains(ShaderStageMask::MESH) {
        wgpu_stages |= ::wgpu::ShaderStages::MESH;
    }
    wgpu_stages
}

#[must_use]
pub const fn binding_type(binding: BindingType) -> ::wgpu::BindingType {
    match binding {
        BindingType::UniformBuffer => ::wgpu::BindingType::Buffer {
            ty: ::wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        BindingType::StorageBuffer => ::wgpu::BindingType::Buffer {
            ty: ::wgpu::BufferBindingType::Storage { read_only: false },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        BindingType::SampledTexture => ::wgpu::BindingType::Texture {
            sample_type: ::wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: ::wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        BindingType::StorageTexture => ::wgpu::BindingType::StorageTexture {
            access: ::wgpu::StorageTextureAccess::WriteOnly,
            format: ::wgpu::TextureFormat::Rgba8Unorm,
            view_dimension: ::wgpu::TextureViewDimension::D2,
        },
        BindingType::Sampler => ::wgpu::BindingType::Sampler(::wgpu::SamplerBindingType::Filtering),
        BindingType::RootConstant => ::wgpu::BindingType::Buffer {
            ty: ::wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
    }
}

#[must_use]
pub const fn empty_layout_entry() -> ::wgpu::BindGroupLayoutEntry {
    ::wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: ::wgpu::ShaderStages::NONE,
        ty: ::wgpu::BindingType::Buffer {
            ty: ::wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        binding::{
            BindLayout, BindTable, BindingCacheKind, BindingDomain, BindlessTable,
            RootConstantFallbackCost,
        },
        ir::{
            BindLayoutDesc as IrBindLayoutDesc, BindingResourceRef,
            BindlessTableDesc as IrBindlessTableDesc, IrBufferId, IrResourceRef,
            MATERIAL_BINDING_IR_SCHEMA_VERSION,
        },
        resource::RendererResourceId,
    };

    fn view_layout() -> BindLayout {
        BindLayout::from_ir(
            IrBindLayoutDesc {
                id: IrBindLayoutId::new(0),
                schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                stable_name: "view.layout",
                slots: [
                    BindSlotDesc {
                        binding: 0,
                        binding_type: BindingType::UniformBuffer,
                        stages: ShaderStageMask::ALL_GRAPHICS,
                    },
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                    BindSlotDesc::default(),
                ],
                slot_count: 1,
            },
            BindingDomain::View,
        )
    }

    fn view_table(resource: RendererResourceId) -> BindTable {
        BindTable::from_ir(
            crate::ir::BindTableDesc {
                id: IrBindTableId::new(0),
                schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                stable_name: "view.table",
                layout: IrBindLayoutId::new(0),
                bindings: [
                    BindingResourceRef {
                        binding: 0,
                        binding_type: BindingType::UniformBuffer,
                        resource: IrResourceRef::buffer(IrBufferId::new(0)),
                    },
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                    BindingResourceRef::default(),
                ],
                binding_count: 1,
            },
            BindingDomain::View,
        )
        .with_renderer_resources(&[resource])
    }

    #[test]
    fn wgpu_binding_bridge_maps_layout_and_table_at_descriptor_granularity() {
        let mut cache = WgpuBindingBridgeCache::default();
        let layout = view_layout();
        let table = view_table(RendererResourceId::first(3));

        let (prepared_layout, layout_decision) = cache
            .translate_layout(layout, BindingTranslationPhase::Warmup)
            .expect("layout warmup translation should create once");
        let (prepared_table, table_decision) = cache
            .translate_table(table, BindingTranslationPhase::Warmup)
            .expect("table warmup translation should create once");

        assert_eq!(prepared_layout.as_wgpu().label, Some("view.layout"));
        assert_eq!(prepared_layout.entry_count, 1);
        assert!(matches!(layout_decision, BindingCacheDecision::Created(_)));
        assert_eq!(prepared_table.stable_name, "view.table");
        assert_eq!(prepared_table.resource_count, 1);
        assert!(prepared_table.maps_to_wgpu_bind_group);
        assert!(matches!(table_decision, BindingCacheDecision::Created(_)));
        assert_eq!(cache.cache.telemetry.layouts_created, 1);
        assert_eq!(cache.cache.telemetry.tables_created, 1);
    }

    #[test]
    fn wgpu_root_constants_use_push_constants_or_explicit_uniform_fallback_marker() {
        let root_constants =
            crate::binding::RootConstants::from_ranges(&[crate::binding::root_constant_slot(
                0,
                0,
                32,
                ShaderStageMask::VERTEX,
            )]);

        let native = root_constant_plan(root_constants, WgpuBindingCapabilities::FULL);
        let fallback = root_constant_plan(root_constants, WgpuBindingCapabilities::CONSERVATIVE);

        assert_eq!(native.mapping, RootConstantMapping::NativePushConstants);
        assert_eq!(
            fallback.mapping,
            RootConstantMapping::UniformFallback(RootConstantFallbackCost {
                fallback_uniform_bytes: 32,
                extra_bindings: 1,
                dynamic_update_required: true,
            })
        );
    }

    #[test]
    fn wgpu_bindless_table_requires_binding_array_capability() {
        let table = BindlessTable::from_ir(
            IrBindlessTableDesc {
                id: IrBindTableId::new(4),
                schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                stable_name: "material.bindless.textures",
                resource_kind: IrResourceKind::Texture,
                capacity: 1024,
                resident_count: 16,
            },
            BindingBackendFeature::BindingArrays,
        );

        let unsupported = bindless_table_plan(table, WgpuBindingCapabilities::CONSERVATIVE);
        let supported = bindless_table_plan(table, WgpuBindingCapabilities::FULL);

        assert!(!unsupported.maps_to_binding_array);
        assert_eq!(
            unsupported.unavailable_reason,
            Some(BindlessTableUnavailableReason::BindingArraysUnsupported)
        );
        assert!(supported.maps_to_binding_array);
        assert_eq!(supported.unavailable_reason, None);
    }

    #[test]
    fn wgpu_binding_cache_blocks_runtime_layout_creation_after_warmup() {
        let mut cache = WgpuBindingBridgeCache::default();
        let error = cache
            .translate_layout(view_layout(), BindingTranslationPhase::RuntimeMeasured)
            .expect_err("measured runtime layout creation must fail");

        assert!(matches!(
            error,
            BindingCacheError::RuntimeCreationAfterWarmup {
                kind: BindingCacheKind::Layout,
                ..
            }
        ));
        assert_eq!(cache.cache.telemetry.runtime_creation_failures, 1);
    }
}
