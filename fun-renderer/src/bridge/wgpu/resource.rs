use crate::ir::{
    AddressMode, BufferDesc, BufferMemoryClass, BufferUsageFlags, FilterMode, SamplerDesc,
    TextureDesc, TextureDimension, TextureFormat, TextureUsageFlags,
};

#[must_use]
pub fn buffer_descriptor(desc: BufferDesc) -> ::wgpu::BufferDescriptor<'static> {
    ::wgpu::BufferDescriptor {
        label: Some(desc.stable_name),
        size: desc.size_bytes,
        usage: buffer_usages(desc.usage, desc.memory),
        mapped_at_creation: false,
    }
}

#[must_use]
pub fn texture_descriptor(desc: TextureDesc) -> ::wgpu::TextureDescriptor<'static> {
    ::wgpu::TextureDescriptor {
        label: Some(desc.stable_name),
        size: ::wgpu::Extent3d {
            width: desc.width,
            height: desc.height,
            depth_or_array_layers: desc.depth_or_layers,
        },
        mip_level_count: u32::from(desc.mip_levels),
        sample_count: u32::from(desc.sample_count),
        dimension: texture_dimension(desc.dimension),
        format: texture_format(desc.format),
        usage: texture_usages(desc.usage),
        view_formats: &[],
    }
}

#[must_use]
pub fn sampler_descriptor(desc: SamplerDesc) -> ::wgpu::SamplerDescriptor<'static> {
    ::wgpu::SamplerDescriptor {
        label: Some(desc.stable_name),
        address_mode_u: address_mode(desc.address_u),
        address_mode_v: address_mode(desc.address_v),
        address_mode_w: address_mode(desc.address_w),
        mag_filter: filter_mode(desc.mag_filter),
        min_filter: filter_mode(desc.min_filter),
        mipmap_filter: mipmap_filter_mode(desc.mip_filter),
        ..Default::default()
    }
}

#[must_use]
pub fn buffer_usages(usage: BufferUsageFlags, memory: BufferMemoryClass) -> ::wgpu::BufferUsages {
    let mut wgpu_usage = ::wgpu::BufferUsages::empty();
    if usage.contains(BufferUsageFlags::VERTEX) {
        wgpu_usage |= ::wgpu::BufferUsages::VERTEX;
    }
    if usage.contains(BufferUsageFlags::INDEX) {
        wgpu_usage |= ::wgpu::BufferUsages::INDEX;
    }
    if usage.contains(BufferUsageFlags::UNIFORM) {
        wgpu_usage |= ::wgpu::BufferUsages::UNIFORM;
    }
    if usage.contains(BufferUsageFlags::STORAGE) {
        wgpu_usage |= ::wgpu::BufferUsages::STORAGE;
    }
    if usage.contains(BufferUsageFlags::INDIRECT) {
        wgpu_usage |= ::wgpu::BufferUsages::INDIRECT;
    }
    if usage.contains(BufferUsageFlags::COPY_SRC) {
        wgpu_usage |= ::wgpu::BufferUsages::COPY_SRC;
    }
    if usage.contains(BufferUsageFlags::COPY_DST) {
        wgpu_usage |= ::wgpu::BufferUsages::COPY_DST;
    }
    match memory {
        BufferMemoryClass::Upload => wgpu_usage | ::wgpu::BufferUsages::COPY_SRC,
        BufferMemoryClass::DeviceLocal => wgpu_usage,
        BufferMemoryClass::Readback => wgpu_usage | ::wgpu::BufferUsages::COPY_DST,
    }
}

#[must_use]
pub fn texture_usages(usage: TextureUsageFlags) -> ::wgpu::TextureUsages {
    let mut wgpu_usage = ::wgpu::TextureUsages::empty();
    if usage.contains(TextureUsageFlags::SAMPLED) {
        wgpu_usage |= ::wgpu::TextureUsages::TEXTURE_BINDING;
    }
    if usage.contains(TextureUsageFlags::RENDER_TARGET)
        || usage.contains(TextureUsageFlags::DEPTH_TARGET)
    {
        wgpu_usage |= ::wgpu::TextureUsages::RENDER_ATTACHMENT;
    }
    if usage.contains(TextureUsageFlags::STORAGE) {
        wgpu_usage |= ::wgpu::TextureUsages::STORAGE_BINDING;
    }
    if usage.contains(TextureUsageFlags::COPY_SRC) {
        wgpu_usage |= ::wgpu::TextureUsages::COPY_SRC;
    }
    if usage.contains(TextureUsageFlags::COPY_DST) {
        wgpu_usage |= ::wgpu::TextureUsages::COPY_DST;
    }
    wgpu_usage
}

#[must_use]
pub const fn texture_dimension(dimension: TextureDimension) -> ::wgpu::TextureDimension {
    match dimension {
        TextureDimension::D1 => ::wgpu::TextureDimension::D1,
        TextureDimension::D2 | TextureDimension::Cube => ::wgpu::TextureDimension::D2,
        TextureDimension::D3 => ::wgpu::TextureDimension::D3,
    }
}

#[must_use]
pub const fn texture_format(format: TextureFormat) -> ::wgpu::TextureFormat {
    match format {
        TextureFormat::Undefined => ::wgpu::TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8Srgb => ::wgpu::TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba8Unorm => ::wgpu::TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba16Float => ::wgpu::TextureFormat::Rgba16Float,
        TextureFormat::Rg16Float => ::wgpu::TextureFormat::Rg16Float,
        TextureFormat::R32Uint => ::wgpu::TextureFormat::R32Uint,
        TextureFormat::Depth32Float => ::wgpu::TextureFormat::Depth32Float,
        TextureFormat::Depth24Stencil8 => ::wgpu::TextureFormat::Depth24PlusStencil8,
    }
}

#[must_use]
pub const fn filter_mode(mode: FilterMode) -> ::wgpu::FilterMode {
    match mode {
        FilterMode::Nearest => ::wgpu::FilterMode::Nearest,
        FilterMode::Linear => ::wgpu::FilterMode::Linear,
    }
}

#[must_use]
pub const fn mipmap_filter_mode(mode: FilterMode) -> ::wgpu::MipmapFilterMode {
    match mode {
        FilterMode::Nearest => ::wgpu::MipmapFilterMode::Nearest,
        FilterMode::Linear => ::wgpu::MipmapFilterMode::Linear,
    }
}

#[must_use]
pub const fn address_mode(mode: AddressMode) -> ::wgpu::AddressMode {
    match mode {
        AddressMode::ClampToEdge => ::wgpu::AddressMode::ClampToEdge,
        AddressMode::Repeat => ::wgpu::AddressMode::Repeat,
        AddressMode::MirrorRepeat => ::wgpu::AddressMode::MirrorRepeat,
    }
}
