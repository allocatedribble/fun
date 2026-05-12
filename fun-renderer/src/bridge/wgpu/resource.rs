use crate::ir::{
    AddressMode, BufferDesc, BufferMemoryClass, BufferUsageFlags, FilterMode, SamplerDesc,
    TextureDesc, TextureDimension, TextureFormat, TextureUsageFlags,
};
use crate::resource::{
    BridgeResourceHandleKind, BridgeResourceRealization, RendererResourceDescriptor,
    RendererResourceDescriptorKind, RendererResourceId, ResourceBridgeKind,
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
pub fn renderer_buffer_descriptor(
    desc: RendererResourceDescriptor,
) -> Option<::wgpu::BufferDescriptor<'static>> {
    match desc {
        RendererResourceDescriptor::Buffer(buffer) => Some(buffer_descriptor(buffer)),
        RendererResourceDescriptor::ReadbackBuffer(readback) => {
            Some(buffer_descriptor(readback.buffer))
        }
        RendererResourceDescriptor::UploadBuffer(upload) => Some(buffer_descriptor(upload.buffer)),
        _ => None,
    }
}

#[must_use]
pub fn renderer_texture_descriptor(
    desc: RendererResourceDescriptor,
) -> Option<::wgpu::TextureDescriptor<'static>> {
    match desc {
        RendererResourceDescriptor::Texture(texture) => Some(texture_descriptor(texture)),
        RendererResourceDescriptor::ExternalTexture(external) => {
            Some(texture_descriptor(external.texture))
        }
        RendererResourceDescriptor::TransientTexture(transient) => {
            Some(texture_descriptor(transient.texture))
        }
        _ => None,
    }
}

#[must_use]
pub fn renderer_sampler_descriptor(
    desc: RendererResourceDescriptor,
) -> Option<::wgpu::SamplerDescriptor<'static>> {
    match desc {
        RendererResourceDescriptor::Sampler(sampler) => Some(sampler_descriptor(sampler)),
        _ => None,
    }
}

#[must_use]
pub const fn bridge_handle_kind(desc: RendererResourceDescriptor) -> BridgeResourceHandleKind {
    match desc {
        RendererResourceDescriptor::Buffer(_)
        | RendererResourceDescriptor::ReadbackBuffer(_)
        | RendererResourceDescriptor::UploadBuffer(_) => BridgeResourceHandleKind::Buffer,
        RendererResourceDescriptor::Texture(_)
        | RendererResourceDescriptor::TransientTexture(_) => BridgeResourceHandleKind::Texture,
        RendererResourceDescriptor::ExternalTexture(_) => BridgeResourceHandleKind::ExternalTexture,
        RendererResourceDescriptor::Sampler(_) => BridgeResourceHandleKind::Sampler,
    }
}

#[must_use]
pub const fn bridge_realization_for_slot(
    desc: RendererResourceDescriptor,
    slot: u32,
    generation: u32,
) -> BridgeResourceRealization {
    BridgeResourceRealization::new(
        ResourceBridgeKind::Wgpu,
        bridge_handle_kind(desc),
        slot,
        generation,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuBridgeResourceEntry {
    pub renderer_id: RendererResourceId,
    pub descriptor_kind: RendererResourceDescriptorKind,
    pub realization: BridgeResourceRealization,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WgpuBridgeResourceRealizationMap {
    entries: Vec<WgpuBridgeResourceEntry>,
}

impl WgpuBridgeResourceRealizationMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        renderer_id: RendererResourceId,
        desc: RendererResourceDescriptor,
    ) -> BridgeResourceRealization {
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.renderer_id == renderer_id)
        {
            return entry.realization;
        }
        let realization = bridge_realization_for_slot(desc, self.entries.len() as u32, 1);
        self.entries.push(WgpuBridgeResourceEntry {
            renderer_id,
            descriptor_kind: desc.kind(),
            realization,
        });
        realization
    }

    #[must_use]
    pub fn get(&self, renderer_id: RendererResourceId) -> Option<WgpuBridgeResourceEntry> {
        self.entries
            .iter()
            .copied()
            .find(|entry| entry.renderer_id == renderer_id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ir::{IrTextureId, TextureUsageFlags},
        resource::{
            RendererResourceDescriptor, RendererResourceDescriptorKind, RendererResourceRegistry,
        },
    };

    #[test]
    fn wgpu_bridge_realization_map_keeps_backend_objects_below_renderer_ids() {
        let texture = TextureDesc::new_2d(
            IrTextureId::new(8),
            "scene.hdr_color",
            320,
            180,
            TextureFormat::Rgba16Float,
            TextureUsageFlags::RENDER_TARGET.union(TextureUsageFlags::SAMPLED),
        );
        let descriptor = RendererResourceDescriptor::Texture(texture);
        let mut registry = RendererResourceRegistry::new();
        let renderer_id = registry.create_with_default_lifetime(descriptor);
        let mut map = WgpuBridgeResourceRealizationMap::new();

        let realization = map.insert(renderer_id, descriptor);
        registry
            .set_bridge_realization(renderer_id, realization)
            .expect("renderer resource id remains the public identity");

        assert_eq!(realization.bridge, ResourceBridgeKind::Wgpu);
        assert_eq!(realization.handle_kind, BridgeResourceHandleKind::Texture);
        assert_eq!(map.len(), 1);
        assert_eq!(
            map.get(renderer_id).unwrap().descriptor_kind,
            RendererResourceDescriptorKind::Texture
        );
        assert_eq!(
            registry.get(renderer_id).unwrap().bridge_realization,
            realization
        );
        assert!(renderer_texture_descriptor(descriptor).is_some());
        assert!(renderer_buffer_descriptor(descriptor).is_none());
    }

    #[test]
    fn wgpu_descriptor_helpers_cover_upload_readback_and_external_resources() {
        let upload =
            RendererResourceDescriptor::UploadBuffer(crate::resource::UploadBufferDesc::new(
                BufferDesc::new(
                    crate::ir::IrBufferId::new(9),
                    "upload.frame_ring",
                    2048,
                    BufferUsageFlags::COPY_SRC,
                    BufferMemoryClass::Upload,
                ),
                0,
            ));
        let readback =
            RendererResourceDescriptor::ReadbackBuffer(crate::resource::ReadbackBufferDesc::new(
                BufferDesc::new(
                    crate::ir::IrBufferId::new(10),
                    "readback.histogram",
                    4096,
                    BufferUsageFlags::COPY_DST,
                    BufferMemoryClass::Readback,
                ),
                256,
            ));
        let external =
            RendererResourceDescriptor::ExternalTexture(crate::resource::ExternalTextureDesc::new(
                TextureDesc::new_2d(
                    IrTextureId::new(11),
                    "native_ui.shared_texture",
                    1920,
                    1080,
                    TextureFormat::Rgba8Unorm,
                    TextureUsageFlags::SAMPLED.union(TextureUsageFlags::COPY_DST),
                ),
                crate::resource::ExternalTextureProducer::NativeUi,
                true,
                false,
            ));

        assert!(
            renderer_buffer_descriptor(upload)
                .unwrap()
                .usage
                .contains(::wgpu::BufferUsages::COPY_SRC)
        );
        assert!(
            renderer_buffer_descriptor(readback)
                .unwrap()
                .usage
                .contains(::wgpu::BufferUsages::COPY_DST)
        );
        assert_eq!(
            renderer_texture_descriptor(external).unwrap().label,
            Some("native_ui.shared_texture")
        );
        assert_eq!(
            bridge_handle_kind(external),
            BridgeResourceHandleKind::ExternalTexture
        );
    }
}
