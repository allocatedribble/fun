use crate::ir::{BindLayoutDesc, BindSlotDesc, BindingType, ShaderStageMask};

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
