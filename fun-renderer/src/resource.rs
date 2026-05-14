use crate::{
    backend::RendererGenerationalIndex,
    ir::{
        BufferMemoryClass, BufferUsageFlags, IrGraphPassId, IrGraphResourceId, IrResourceRef,
        ResourceAccess, TextureFormat, TextureUsageFlags,
    },
};

pub use crate::ir::{BufferDesc, SamplerDesc, TextureDesc};

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
    NativeUiSharedTextures,
    SwapchainResources,
    VendorSdkResources,
}

impl ImportedResourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeUiSharedTextures => "native_ui_shared_textures",
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererResourceId {
    raw: RendererGenerationalIndex,
}

impl RendererResourceId {
    pub const INVALID: Self = Self {
        raw: RendererGenerationalIndex::INVALID,
    };

    #[must_use]
    pub const fn new(slot: u32, generation: u32) -> Self {
        Self {
            raw: RendererGenerationalIndex::new(slot, generation),
        }
    }

    #[must_use]
    pub const fn first(slot: u32) -> Self {
        Self {
            raw: RendererGenerationalIndex::first(slot),
        }
    }

    #[must_use]
    pub const fn raw(self) -> RendererGenerationalIndex {
        self.raw
    }

    #[must_use]
    pub const fn slot(self) -> u32 {
        self.raw.slot
    }

    #[must_use]
    pub const fn generation(self) -> u32 {
        self.raw.generation
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.raw.is_valid()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RendererResourceLifetimeClass {
    #[default]
    Persistent,
    FrameLocal,
    GraphTransient,
    Imported,
    ExternalProducer,
    Readback,
    Upload,
    DebugOnly,
}

impl RendererResourceLifetimeClass {
    pub const ALL: [Self; 8] = [
        Self::Persistent,
        Self::FrameLocal,
        Self::GraphTransient,
        Self::Imported,
        Self::ExternalProducer,
        Self::Readback,
        Self::Upload,
        Self::DebugOnly,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Persistent => "persistent",
            Self::FrameLocal => "frame_local",
            Self::GraphTransient => "graph_transient",
            Self::Imported => "imported",
            Self::ExternalProducer => "external_producer",
            Self::Readback => "readback",
            Self::Upload => "upload",
            Self::DebugOnly => "debug_only",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererResourceUsageFlags(pub u32);

impl RendererResourceUsageFlags {
    pub const NONE: Self = Self(0);
    pub const VERTEX: Self = Self(1 << 0);
    pub const INDEX: Self = Self(1 << 1);
    pub const UNIFORM: Self = Self(1 << 2);
    pub const STORAGE: Self = Self(1 << 3);
    pub const INDIRECT: Self = Self(1 << 4);
    pub const SAMPLED: Self = Self(1 << 5);
    pub const RENDER_TARGET: Self = Self(1 << 6);
    pub const DEPTH_TARGET: Self = Self(1 << 7);
    pub const COPY_SRC: Self = Self(1 << 8);
    pub const COPY_DST: Self = Self(1 << 9);
    pub const UPLOAD: Self = Self(1 << 10);
    pub const READBACK: Self = Self(1 << 11);
    pub const IMPORTED: Self = Self(1 << 12);
    pub const EXTERNAL_PRODUCER: Self = Self(1 << 13);
    pub const PRESENT: Self = Self(1 << 14);
    pub const DEBUG_ONLY: Self = Self(1 << 15);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalTextureProducer {
    #[default]
    Swapchain,
    NativeUi,
    VendorSdk,
    CaptureTool,
    DirectBackend,
}

impl ExternalTextureProducer {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Swapchain => "swapchain",
            Self::NativeUi => "native_ui",
            Self::VendorSdk => "vendor_sdk",
            Self::CaptureTool => "capture_tool",
            Self::DirectBackend => "direct_backend",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExternalTextureDesc {
    pub schema_version: u16,
    pub texture: TextureDesc,
    pub producer: ExternalTextureProducer,
    pub readable_by_renderer: bool,
    pub writable_by_renderer: bool,
}

impl ExternalTextureDesc {
    #[must_use]
    pub const fn new(
        texture: TextureDesc,
        producer: ExternalTextureProducer,
        readable_by_renderer: bool,
        writable_by_renderer: bool,
    ) -> Self {
        Self {
            schema_version: RENDERER_RESOURCE_SCHEMA_VERSION,
            texture,
            producer,
            readable_by_renderer,
            writable_by_renderer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransientTextureDesc {
    pub schema_version: u16,
    pub texture: TextureDesc,
    pub graph_resource: IrGraphResourceId,
    pub aliasable: bool,
}

impl TransientTextureDesc {
    #[must_use]
    pub const fn new(texture: TextureDesc, graph_resource: IrGraphResourceId) -> Self {
        Self {
            schema_version: RENDERER_RESOURCE_SCHEMA_VERSION,
            texture,
            graph_resource,
            aliasable: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReadbackBufferDesc {
    pub schema_version: u16,
    pub buffer: BufferDesc,
    pub row_stride_bytes: u32,
    pub debug_only: bool,
}

impl ReadbackBufferDesc {
    #[must_use]
    pub const fn new(buffer: BufferDesc, row_stride_bytes: u32) -> Self {
        Self {
            schema_version: RENDERER_RESOURCE_SCHEMA_VERSION,
            buffer,
            row_stride_bytes,
            debug_only: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UploadBufferDesc {
    pub schema_version: u16,
    pub buffer: BufferDesc,
    pub ring_index: u16,
    pub frame_local: bool,
}

impl UploadBufferDesc {
    #[must_use]
    pub const fn new(buffer: BufferDesc, ring_index: u16) -> Self {
        Self {
            schema_version: RENDERER_RESOURCE_SCHEMA_VERSION,
            buffer,
            ring_index,
            frame_local: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererResourceDescriptorKind {
    #[default]
    Buffer,
    Texture,
    Sampler,
    ExternalTexture,
    TransientTexture,
    ReadbackBuffer,
    UploadBuffer,
}

impl RendererResourceDescriptorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buffer => "buffer",
            Self::Texture => "texture",
            Self::Sampler => "sampler",
            Self::ExternalTexture => "external_texture",
            Self::TransientTexture => "transient_texture",
            Self::ReadbackBuffer => "readback_buffer",
            Self::UploadBuffer => "upload_buffer",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererResourceDescriptor {
    Buffer(BufferDesc),
    Texture(TextureDesc),
    Sampler(SamplerDesc),
    ExternalTexture(ExternalTextureDesc),
    TransientTexture(TransientTextureDesc),
    ReadbackBuffer(ReadbackBufferDesc),
    UploadBuffer(UploadBufferDesc),
}

impl RendererResourceDescriptor {
    #[must_use]
    pub const fn kind(self) -> RendererResourceDescriptorKind {
        match self {
            Self::Buffer(_) => RendererResourceDescriptorKind::Buffer,
            Self::Texture(_) => RendererResourceDescriptorKind::Texture,
            Self::Sampler(_) => RendererResourceDescriptorKind::Sampler,
            Self::ExternalTexture(_) => RendererResourceDescriptorKind::ExternalTexture,
            Self::TransientTexture(_) => RendererResourceDescriptorKind::TransientTexture,
            Self::ReadbackBuffer(_) => RendererResourceDescriptorKind::ReadbackBuffer,
            Self::UploadBuffer(_) => RendererResourceDescriptorKind::UploadBuffer,
        }
    }

    #[must_use]
    pub const fn stable_name(self) -> &'static str {
        match self {
            Self::Buffer(desc) => desc.stable_name,
            Self::Texture(desc) => desc.stable_name,
            Self::Sampler(desc) => desc.stable_name,
            Self::ExternalTexture(desc) => desc.texture.stable_name,
            Self::TransientTexture(desc) => desc.texture.stable_name,
            Self::ReadbackBuffer(desc) => desc.buffer.stable_name,
            Self::UploadBuffer(desc) => desc.buffer.stable_name,
        }
    }

    #[must_use]
    pub const fn default_lifetime(self) -> RendererResourceLifetimeClass {
        match self {
            Self::Buffer(desc) => match desc.memory {
                BufferMemoryClass::Upload => RendererResourceLifetimeClass::Upload,
                BufferMemoryClass::Readback => RendererResourceLifetimeClass::Readback,
                BufferMemoryClass::DeviceLocal => RendererResourceLifetimeClass::Persistent,
            },
            Self::Texture(_) | Self::Sampler(_) => RendererResourceLifetimeClass::Persistent,
            Self::ExternalTexture(_) => RendererResourceLifetimeClass::ExternalProducer,
            Self::TransientTexture(_) => RendererResourceLifetimeClass::GraphTransient,
            Self::ReadbackBuffer(desc) if desc.debug_only => {
                RendererResourceLifetimeClass::DebugOnly
            }
            Self::ReadbackBuffer(_) => RendererResourceLifetimeClass::Readback,
            Self::UploadBuffer(_) => RendererResourceLifetimeClass::Upload,
        }
    }

    #[must_use]
    pub const fn usage_flags(self) -> RendererResourceUsageFlags {
        match self {
            Self::Buffer(desc) => buffer_usage_flags(desc.usage, desc.memory),
            Self::Texture(desc) => texture_usage_flags(desc.usage),
            Self::Sampler(_) => RendererResourceUsageFlags::SAMPLED,
            Self::ExternalTexture(desc) => texture_usage_flags(desc.texture.usage)
                .union(RendererResourceUsageFlags::IMPORTED)
                .union(RendererResourceUsageFlags::EXTERNAL_PRODUCER),
            Self::TransientTexture(desc) => texture_usage_flags(desc.texture.usage),
            Self::ReadbackBuffer(desc) => buffer_usage_flags(desc.buffer.usage, desc.buffer.memory)
                .union(RendererResourceUsageFlags::READBACK)
                .union(if desc.debug_only {
                    RendererResourceUsageFlags::DEBUG_ONLY
                } else {
                    RendererResourceUsageFlags::NONE
                }),
            Self::UploadBuffer(desc) => buffer_usage_flags(desc.buffer.usage, desc.buffer.memory)
                .union(RendererResourceUsageFlags::UPLOAD),
        }
    }

    #[must_use]
    pub const fn memory_size_bytes(self) -> u64 {
        match self {
            Self::Buffer(desc) => desc.size_bytes,
            Self::Texture(desc) => texture_size_bytes(desc),
            Self::Sampler(_) => 0,
            Self::ExternalTexture(desc) => texture_size_bytes(desc.texture),
            Self::TransientTexture(desc) => texture_size_bytes(desc.texture),
            Self::ReadbackBuffer(desc) => desc.buffer.size_bytes,
            Self::UploadBuffer(desc) => desc.buffer.size_bytes,
        }
    }
}

#[must_use]
pub const fn buffer_usage_flags(
    usage: BufferUsageFlags,
    memory: BufferMemoryClass,
) -> RendererResourceUsageFlags {
    let mut flags = RendererResourceUsageFlags::NONE;
    if usage.contains(BufferUsageFlags::VERTEX) {
        flags = flags.union(RendererResourceUsageFlags::VERTEX);
    }
    if usage.contains(BufferUsageFlags::INDEX) {
        flags = flags.union(RendererResourceUsageFlags::INDEX);
    }
    if usage.contains(BufferUsageFlags::UNIFORM) {
        flags = flags.union(RendererResourceUsageFlags::UNIFORM);
    }
    if usage.contains(BufferUsageFlags::STORAGE) {
        flags = flags.union(RendererResourceUsageFlags::STORAGE);
    }
    if usage.contains(BufferUsageFlags::INDIRECT) {
        flags = flags.union(RendererResourceUsageFlags::INDIRECT);
    }
    if usage.contains(BufferUsageFlags::COPY_SRC) {
        flags = flags.union(RendererResourceUsageFlags::COPY_SRC);
    }
    if usage.contains(BufferUsageFlags::COPY_DST) {
        flags = flags.union(RendererResourceUsageFlags::COPY_DST);
    }
    match memory {
        BufferMemoryClass::Upload => flags.union(RendererResourceUsageFlags::UPLOAD),
        BufferMemoryClass::Readback => flags.union(RendererResourceUsageFlags::READBACK),
        BufferMemoryClass::DeviceLocal => flags,
    }
}

#[must_use]
pub const fn texture_usage_flags(usage: TextureUsageFlags) -> RendererResourceUsageFlags {
    let mut flags = RendererResourceUsageFlags::NONE;
    if usage.contains(TextureUsageFlags::SAMPLED) {
        flags = flags.union(RendererResourceUsageFlags::SAMPLED);
    }
    if usage.contains(TextureUsageFlags::RENDER_TARGET) {
        flags = flags.union(RendererResourceUsageFlags::RENDER_TARGET);
    }
    if usage.contains(TextureUsageFlags::DEPTH_TARGET) {
        flags = flags.union(RendererResourceUsageFlags::DEPTH_TARGET);
    }
    if usage.contains(TextureUsageFlags::STORAGE) {
        flags = flags.union(RendererResourceUsageFlags::STORAGE);
    }
    if usage.contains(TextureUsageFlags::COPY_SRC) {
        flags = flags.union(RendererResourceUsageFlags::COPY_SRC);
    }
    if usage.contains(TextureUsageFlags::COPY_DST) {
        flags = flags.union(RendererResourceUsageFlags::COPY_DST);
    }
    flags
}

#[must_use]
pub const fn texture_size_bytes(desc: TextureDesc) -> u64 {
    (desc.width as u64)
        .saturating_mul(desc.height as u64)
        .saturating_mul(desc.depth_or_layers as u64)
        .saturating_mul(desc.mip_levels as u64)
        .saturating_mul(desc.sample_count as u64)
        .saturating_mul(texture_format_bytes_per_unit(desc.format) as u64)
}

#[must_use]
pub const fn texture_format_bytes_per_unit(format: TextureFormat) -> u32 {
    match format {
        TextureFormat::Undefined => 0,
        TextureFormat::Rgba8Srgb | TextureFormat::Rgba8Unorm | TextureFormat::R32Uint => 4,
        TextureFormat::Rgba16Float => 8,
        TextureFormat::Rg16Float => 4,
        TextureFormat::Depth32Float => 4,
        TextureFormat::Depth24Stencil8 => 4,
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceBridgeKind {
    #[default]
    None,
    Wgpu,
    DirectDx12,
    DirectVulkan,
    DirectMetal,
    Null,
}

impl ResourceBridgeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Wgpu => "wgpu",
            Self::DirectDx12 => "direct_dx12",
            Self::DirectVulkan => "direct_vulkan",
            Self::DirectMetal => "direct_metal",
            Self::Null => "null",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BridgeResourceHandleKind {
    #[default]
    None,
    Buffer,
    Texture,
    Sampler,
    ExternalTexture,
}

impl BridgeResourceHandleKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Buffer => "buffer",
            Self::Texture => "texture",
            Self::Sampler => "sampler",
            Self::ExternalTexture => "external_texture",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BridgeResourceRealization {
    pub bridge: ResourceBridgeKind,
    pub handle_kind: BridgeResourceHandleKind,
    pub slot: u32,
    pub generation: u32,
}

impl BridgeResourceRealization {
    pub const NONE: Self = Self {
        bridge: ResourceBridgeKind::None,
        handle_kind: BridgeResourceHandleKind::None,
        slot: u32::MAX,
        generation: 0,
    };

    #[must_use]
    pub const fn new(
        bridge: ResourceBridgeKind,
        handle_kind: BridgeResourceHandleKind,
        slot: u32,
        generation: u32,
    ) -> Self {
        Self {
            bridge,
            handle_kind,
            slot,
            generation,
        }
    }

    #[must_use]
    pub const fn is_realized(self) -> bool {
        !matches!(self.bridge, ResourceBridgeKind::None)
            && !matches!(self.handle_kind, BridgeResourceHandleKind::None)
            && self.slot != u32::MAX
            && self.generation != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererResourceResidencyState {
    #[default]
    Declared,
    Creating,
    Resident,
    External,
    Retiring,
    Retired,
    Lost,
}

impl RendererResourceResidencyState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Declared => "declared",
            Self::Creating => "creating",
            Self::Resident => "resident",
            Self::External => "external",
            Self::Retiring => "retiring",
            Self::Retired => "retired",
            Self::Lost => "lost",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererResourceGraphState {
    pub graph_resource: IrGraphResourceId,
    pub resource_ref: IrResourceRef,
    pub access: ResourceAccess,
    pub last_pass: IrGraphPassId,
}

impl Default for RendererResourceGraphState {
    fn default() -> Self {
        Self {
            graph_resource: IrGraphResourceId::INVALID,
            resource_ref: IrResourceRef::INVALID,
            access: ResourceAccess::Read,
            last_pass: IrGraphPassId::INVALID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererResourceRecord {
    pub id: RendererResourceId,
    pub descriptor: RendererResourceDescriptor,
    pub lifetime: RendererResourceLifetimeClass,
    pub usage: RendererResourceUsageFlags,
    pub current_graph_state: RendererResourceGraphState,
    pub bridge_realization: BridgeResourceRealization,
    pub memory_size_bytes: u64,
    pub residency_state: RendererResourceResidencyState,
    pub debug_name: &'static str,
    pub last_used_frame: u64,
    pub retirement_fence: u64,
}

impl RendererResourceRecord {
    #[must_use]
    pub fn new(
        id: RendererResourceId,
        descriptor: RendererResourceDescriptor,
        lifetime: RendererResourceLifetimeClass,
        debug_name: &'static str,
        frame_index: u64,
    ) -> Self {
        let residency_state = match lifetime {
            RendererResourceLifetimeClass::Imported
            | RendererResourceLifetimeClass::ExternalProducer => {
                RendererResourceResidencyState::External
            }
            _ => RendererResourceResidencyState::Declared,
        };
        Self {
            id,
            descriptor,
            lifetime,
            usage: descriptor.usage_flags(),
            current_graph_state: RendererResourceGraphState::default(),
            bridge_realization: BridgeResourceRealization::NONE,
            memory_size_bytes: descriptor.memory_size_bytes(),
            residency_state,
            debug_name,
            last_used_frame: frame_index,
            retirement_fence: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceMemoryHighWaterMarks {
    pub persistent_bytes: u64,
    pub transient_bytes: u64,
    pub upload_bytes: u64,
    pub readback_bytes: u64,
    pub imported_external_bytes: u64,
    pub debug_bytes: u64,
    pub live_resources: u32,
}

impl ResourceMemoryHighWaterMarks {
    pub const ZERO: Self = Self {
        persistent_bytes: 0,
        transient_bytes: 0,
        upload_bytes: 0,
        readback_bytes: 0,
        imported_external_bytes: 0,
        debug_bytes: 0,
        live_resources: 0,
    };
}

impl Default for ResourceMemoryHighWaterMarks {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceMemoryTelemetry {
    pub frame_index: u64,
    pub persistent_bytes: u64,
    pub transient_bytes: u64,
    pub upload_bytes: u64,
    pub readback_bytes: u64,
    pub imported_external_bytes: u64,
    pub debug_bytes: u64,
    pub high_water_marks: ResourceMemoryHighWaterMarks,
    pub resources_created: u32,
    pub resources_destroyed: u32,
    pub resources_retired_by_fence: u32,
    pub live_resources: u32,
    pub bridge_realization_count: u32,
}

impl Default for ResourceMemoryTelemetry {
    fn default() -> Self {
        Self {
            frame_index: 0,
            persistent_bytes: 0,
            transient_bytes: 0,
            upload_bytes: 0,
            readback_bytes: 0,
            imported_external_bytes: 0,
            debug_bytes: 0,
            high_water_marks: ResourceMemoryHighWaterMarks::ZERO,
            resources_created: 0,
            resources_destroyed: 0,
            resources_retired_by_fence: 0,
            live_resources: 0,
            bridge_realization_count: 0,
        }
    }
}

impl ResourceMemoryTelemetry {
    pub fn begin_frame(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.resources_created = 0;
        self.resources_destroyed = 0;
        self.resources_retired_by_fence = 0;
    }

    pub fn record_create(&mut self, lifetime: RendererResourceLifetimeClass, bytes: u64) {
        self.live_resources = self.live_resources.saturating_add(1);
        self.resources_created = self.resources_created.saturating_add(1);
        self.add_bytes(lifetime, bytes);
        self.high_water_marks.live_resources = self
            .high_water_marks
            .live_resources
            .max(self.live_resources);
    }

    pub fn record_destroy(&mut self, lifetime: RendererResourceLifetimeClass, bytes: u64) {
        self.live_resources = self.live_resources.saturating_sub(1);
        self.resources_destroyed = self.resources_destroyed.saturating_add(1);
        self.remove_bytes(lifetime, bytes);
    }

    pub fn record_realized(&mut self) {
        self.bridge_realization_count = self.bridge_realization_count.saturating_add(1);
    }

    pub fn record_unrealized(&mut self) {
        self.bridge_realization_count = self.bridge_realization_count.saturating_sub(1);
    }

    pub fn record_retired_by_fence(&mut self) {
        self.resources_retired_by_fence = self.resources_retired_by_fence.saturating_add(1);
    }

    fn add_bytes(&mut self, lifetime: RendererResourceLifetimeClass, bytes: u64) {
        match lifetime {
            RendererResourceLifetimeClass::Persistent => {
                self.persistent_bytes = self.persistent_bytes.saturating_add(bytes);
                self.high_water_marks.persistent_bytes = self
                    .high_water_marks
                    .persistent_bytes
                    .max(self.persistent_bytes);
            }
            RendererResourceLifetimeClass::FrameLocal
            | RendererResourceLifetimeClass::GraphTransient => {
                self.transient_bytes = self.transient_bytes.saturating_add(bytes);
                self.high_water_marks.transient_bytes = self
                    .high_water_marks
                    .transient_bytes
                    .max(self.transient_bytes);
            }
            RendererResourceLifetimeClass::Imported
            | RendererResourceLifetimeClass::ExternalProducer => {
                self.imported_external_bytes = self.imported_external_bytes.saturating_add(bytes);
                self.high_water_marks.imported_external_bytes = self
                    .high_water_marks
                    .imported_external_bytes
                    .max(self.imported_external_bytes);
            }
            RendererResourceLifetimeClass::Readback => {
                self.readback_bytes = self.readback_bytes.saturating_add(bytes);
                self.high_water_marks.readback_bytes = self
                    .high_water_marks
                    .readback_bytes
                    .max(self.readback_bytes);
            }
            RendererResourceLifetimeClass::Upload => {
                self.upload_bytes = self.upload_bytes.saturating_add(bytes);
                self.high_water_marks.upload_bytes =
                    self.high_water_marks.upload_bytes.max(self.upload_bytes);
            }
            RendererResourceLifetimeClass::DebugOnly => {
                self.debug_bytes = self.debug_bytes.saturating_add(bytes);
                self.high_water_marks.debug_bytes =
                    self.high_water_marks.debug_bytes.max(self.debug_bytes);
            }
        }
    }

    fn remove_bytes(&mut self, lifetime: RendererResourceLifetimeClass, bytes: u64) {
        match lifetime {
            RendererResourceLifetimeClass::Persistent => {
                self.persistent_bytes = self.persistent_bytes.saturating_sub(bytes);
            }
            RendererResourceLifetimeClass::FrameLocal
            | RendererResourceLifetimeClass::GraphTransient => {
                self.transient_bytes = self.transient_bytes.saturating_sub(bytes);
            }
            RendererResourceLifetimeClass::Imported
            | RendererResourceLifetimeClass::ExternalProducer => {
                self.imported_external_bytes = self.imported_external_bytes.saturating_sub(bytes);
            }
            RendererResourceLifetimeClass::Readback => {
                self.readback_bytes = self.readback_bytes.saturating_sub(bytes);
            }
            RendererResourceLifetimeClass::Upload => {
                self.upload_bytes = self.upload_bytes.saturating_sub(bytes);
            }
            RendererResourceLifetimeClass::DebugOnly => {
                self.debug_bytes = self.debug_bytes.saturating_sub(bytes);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererResourceRegistryError {
    InvalidResourceId,
    StaleResourceId,
    ResourceSlotEmpty,
    ResourceAlreadyRetiring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RendererResourceSlot {
    generation: u32,
    record: Option<RendererResourceRecord>,
}

impl RendererResourceSlot {
    const EMPTY: Self = Self {
        generation: 1,
        record: None,
    };
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RendererResourceRegistry {
    slots: Vec<RendererResourceSlot>,
    free_slots: Vec<u32>,
    telemetry: ResourceMemoryTelemetry,
}

impl RendererResourceRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.telemetry.begin_frame(frame_index);
    }

    pub fn create(
        &mut self,
        descriptor: RendererResourceDescriptor,
        lifetime: RendererResourceLifetimeClass,
        debug_name: &'static str,
    ) -> RendererResourceId {
        let slot = self.free_slots.pop().unwrap_or(self.slots.len() as u32);
        if slot as usize == self.slots.len() {
            self.slots.push(RendererResourceSlot::EMPTY);
        }
        let generation = self.slots[slot as usize].generation;
        let id = RendererResourceId::new(slot, generation);
        let record = RendererResourceRecord::new(
            id,
            descriptor,
            lifetime,
            if debug_name.is_empty() {
                descriptor.stable_name()
            } else {
                debug_name
            },
            self.telemetry.frame_index,
        );
        self.telemetry
            .record_create(record.lifetime, record.memory_size_bytes);
        self.slots[slot as usize].record = Some(record);
        id
    }

    pub fn create_with_default_lifetime(
        &mut self,
        descriptor: RendererResourceDescriptor,
    ) -> RendererResourceId {
        self.create(
            descriptor,
            descriptor.default_lifetime(),
            descriptor.stable_name(),
        )
    }

    pub fn get(
        &self,
        id: RendererResourceId,
    ) -> Result<RendererResourceRecord, RendererResourceRegistryError> {
        self.slot(id)?
            .record
            .ok_or(RendererResourceRegistryError::ResourceSlotEmpty)
    }

    pub fn update_graph_state(
        &mut self,
        id: RendererResourceId,
        state: RendererResourceGraphState,
        frame_index: u64,
    ) -> Result<(), RendererResourceRegistryError> {
        let record = self.record_mut(id)?;
        record.current_graph_state = state;
        record.last_used_frame = frame_index;
        Ok(())
    }

    pub fn set_bridge_realization(
        &mut self,
        id: RendererResourceId,
        realization: BridgeResourceRealization,
    ) -> Result<(), RendererResourceRegistryError> {
        let old_realized = self.get(id)?.bridge_realization.is_realized();
        let new_realized = realization.is_realized();
        let record = self.record_mut(id)?;
        record.bridge_realization = realization;
        record.residency_state = if new_realized {
            RendererResourceResidencyState::Resident
        } else {
            RendererResourceResidencyState::Declared
        };
        if !old_realized && new_realized {
            self.telemetry.record_realized();
        } else if old_realized && !new_realized {
            self.telemetry.record_unrealized();
        }
        Ok(())
    }

    pub fn retire(
        &mut self,
        id: RendererResourceId,
        retirement_fence: u64,
    ) -> Result<(), RendererResourceRegistryError> {
        let record = self.record_mut(id)?;
        if matches!(
            record.residency_state,
            RendererResourceResidencyState::Retiring
        ) {
            return Err(RendererResourceRegistryError::ResourceAlreadyRetiring);
        }
        record.retirement_fence = retirement_fence;
        record.residency_state = RendererResourceResidencyState::Retiring;
        Ok(())
    }

    pub fn destroy_now(
        &mut self,
        id: RendererResourceId,
    ) -> Result<(), RendererResourceRegistryError> {
        let slot_index = self.checked_slot_index(id)?;
        self.destroy_slot(slot_index, false)
    }

    pub fn destroy_retired_by_fence(&mut self, completed_fence: u64) -> u32 {
        let mut destroyed = 0u32;
        let mut slot_index = 0usize;
        while slot_index < self.slots.len() {
            let should_destroy = self.slots[slot_index].record.is_some_and(|record| {
                matches!(
                    record.residency_state,
                    RendererResourceResidencyState::Retiring
                ) && record.retirement_fence != 0
                    && record.retirement_fence <= completed_fence
            });
            if should_destroy && self.destroy_slot(slot_index as u32, true).is_ok() {
                destroyed = destroyed.saturating_add(1);
            }
            slot_index += 1;
        }
        destroyed
    }

    #[must_use]
    pub const fn memory_telemetry(&self) -> ResourceMemoryTelemetry {
        self.telemetry
    }

    #[must_use]
    pub fn live_resource_count(&self) -> u32 {
        self.telemetry.live_resources
    }

    fn slot(
        &self,
        id: RendererResourceId,
    ) -> Result<RendererResourceSlot, RendererResourceRegistryError> {
        let slot_index = self.checked_slot_index(id)?;
        Ok(self.slots[slot_index as usize])
    }

    fn record_mut(
        &mut self,
        id: RendererResourceId,
    ) -> Result<&mut RendererResourceRecord, RendererResourceRegistryError> {
        let slot_index = self.checked_slot_index(id)?;
        self.slots[slot_index as usize]
            .record
            .as_mut()
            .ok_or(RendererResourceRegistryError::ResourceSlotEmpty)
    }

    fn checked_slot_index(
        &self,
        id: RendererResourceId,
    ) -> Result<u32, RendererResourceRegistryError> {
        if !id.is_valid() {
            return Err(RendererResourceRegistryError::InvalidResourceId);
        }
        let Some(slot) = self.slots.get(id.slot() as usize) else {
            return Err(RendererResourceRegistryError::InvalidResourceId);
        };
        if slot.generation != id.generation() {
            return Err(RendererResourceRegistryError::StaleResourceId);
        }
        Ok(id.slot())
    }

    fn destroy_slot(
        &mut self,
        slot_index: u32,
        retired_by_fence: bool,
    ) -> Result<(), RendererResourceRegistryError> {
        let slot = &mut self.slots[slot_index as usize];
        let Some(mut record) = slot.record.take() else {
            return Err(RendererResourceRegistryError::ResourceSlotEmpty);
        };
        record.residency_state = RendererResourceResidencyState::Retired;
        self.telemetry
            .record_destroy(record.lifetime, record.memory_size_bytes);
        if record.bridge_realization.is_realized() {
            self.telemetry.record_unrealized();
        }
        if retired_by_fence {
            self.telemetry.record_retired_by_fence();
        }
        slot.generation = RendererGenerationalIndex::new(slot_index, slot.generation)
            .next_generation()
            .generation;
        self.free_slots.push(slot_index);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RendererResourceOwner {
    FunRenderer,
    FunRenderBridge,
    RetiredEngineGeneric,
    RetiredEngineLowLevel,
    RuntimeClient,
    FunUiNativeUi,
    VendorSdk,
}

impl RendererResourceOwner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunRenderer => crate::FUN_RENDERER_CRATE_NAME,
            Self::FunRenderBridge => crate::FUN_RENDER_DONOR_PACKAGE_NAME,
            Self::RetiredEngineGeneric => "retired_engine_generic_render_resource",
            Self::RetiredEngineLowLevel => "retired_engine_low_level",
            Self::RuntimeClient => "runtime_client",
            Self::FunUiNativeUi => "fun_ui_native_ui",
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
    pub force_retired_engine_prepare_stage_helpers_through_upload_arena: bool,
    pub semantic_owner_required_before_generic_helper_migration: bool,
    pub top_small_buffer_offenders_are_generic_retired_engine_helpers: bool,
}

pub const RENDERER_RESOURCE_OWNERSHIP_POLICY: RendererResourceOwnershipPolicy =
    RendererResourceOwnershipPolicy {
        policy_owner: RendererResourceOwner::FunRenderer,
        compatibility_shim_owner: RendererResourceOwner::FunRenderBridge,
        hidden_transient_allocations_allowed_in_major_passes: false,
        force_retired_engine_prepare_stage_helpers_through_upload_arena: false,
        semantic_owner_required_before_generic_helper_migration: true,
        top_small_buffer_offenders_are_generic_retired_engine_helpers: true,
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
        stable_id: "retired_engine.dynamic_uniform_buffer.write_buffer_with.uniform_buffer_311",
        module: "retired_engine_render::render_resource::DynamicUniformBuffer",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::RetiredEngineGeneric,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::BenchmarkedHotUploadOffender,
        phase: ResourceOwnershipPhase::ObserveOnly,
        benchmark_artifact: PASS5_DX12_PARITY_ARTIFACT,
        migration_gate: "split_semantic_owner_before_upload_arena_migration",
    },
    RendererResourceAllocationSite {
        stable_id: "retired_engine.raw_buffer_vec.write_buffer.buffer_vec_183",
        module: "retired_engine_render::render_resource::RawBufferVec",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::RetiredEngineGeneric,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::BenchmarkedHotUploadOffender,
        phase: ResourceOwnershipPhase::ObserveOnly,
        benchmark_artifact: PASS5_DX12_PARITY_ARTIFACT,
        migration_gate: "batch_or_split_semantic_owner_before_upload_arena_migration",
    },
    RendererResourceAllocationSite {
        stable_id: "retired_engine.dynamic_uniform_buffer.write_buffer.uniform_buffer_140",
        module: "retired_engine_render::render_resource::DynamicUniformBuffer",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::RetiredEngineGeneric,
        intended_owner: RendererResourceOwner::FunRenderer,
        priority: ResourceMigrationPriority::BenchmarkedHotUploadOffender,
        phase: ResourceOwnershipPhase::ObserveOnly,
        benchmark_artifact: PASS5_DX12_PARITY_ARTIFACT,
        migration_gate: "split_semantic_owner_before_upload_arena_migration",
    },
    RendererResourceAllocationSite {
        stable_id: "retired_engine.raw_buffer_vec.write_buffer.buffer_vec_442",
        module: "retired_engine_render::render_resource::RawBufferVec",
        kind: RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
        current_owner: RendererResourceOwner::RetiredEngineGeneric,
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
            RendererResourceKind::Imported(ImportedResourceKind::NativeUiSharedTextures).class(),
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
        assert!(!policy.force_retired_engine_prepare_stage_helpers_through_upload_arena);
        assert!(policy.semantic_owner_required_before_generic_helper_migration);
        assert!(policy.top_small_buffer_offenders_are_generic_retired_engine_helpers);
    }

    #[test]
    fn resource_hot_upload_kill_list_names_retired_engine_generic_offenders_and_artifacts() {
        assert!(HOT_UPLOAD_KILL_LIST.iter().any(|site| {
            site.module == "retired_engine_render::render_resource::DynamicUniformBuffer"
                && site.benchmark_artifact == PASS5_DX12_PARITY_ARTIFACT
                && site.phase == ResourceOwnershipPhase::ObserveOnly
        }));
        assert!(HOT_UPLOAD_KILL_LIST.iter().any(|site| {
            site.module == "retired_engine_render::render_resource::RawBufferVec"
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
            "import.native_ui",
            RendererResourceKind::Imported(ImportedResourceKind::NativeUiSharedTextures),
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

    #[test]
    fn pass10_resource_descriptors_are_backend_neutral() {
        assert_eq!(RendererResourceLifetimeClass::ALL.len(), 8);
        assert_eq!(
            RendererResourceLifetimeClass::ExternalProducer.as_str(),
            "external_producer"
        );

        let texture = TextureDesc::new_2d(
            crate::ir::IrTextureId::new(1),
            "scene.color",
            64,
            32,
            TextureFormat::Rgba16Float,
            TextureUsageFlags::RENDER_TARGET.union(TextureUsageFlags::SAMPLED),
        );
        let transient = TransientTextureDesc::new(texture, IrGraphResourceId::new(7));
        let external =
            ExternalTextureDesc::new(texture, ExternalTextureProducer::NativeUi, true, false);
        let readback = ReadbackBufferDesc::new(
            BufferDesc::new(
                crate::ir::IrBufferId::new(2),
                "readback.histogram",
                4096,
                BufferUsageFlags::COPY_DST,
                BufferMemoryClass::Readback,
            ),
            256,
        );
        let upload = UploadBufferDesc::new(
            BufferDesc::new(
                crate::ir::IrBufferId::new(3),
                "upload.material_patch",
                2048,
                BufferUsageFlags::COPY_SRC,
                BufferMemoryClass::Upload,
            ),
            1,
        );

        assert_eq!(
            RendererResourceDescriptor::TransientTexture(transient).default_lifetime(),
            RendererResourceLifetimeClass::GraphTransient
        );
        assert_eq!(
            RendererResourceDescriptor::ExternalTexture(external).default_lifetime(),
            RendererResourceLifetimeClass::ExternalProducer
        );
        assert_eq!(
            RendererResourceDescriptor::ReadbackBuffer(readback).default_lifetime(),
            RendererResourceLifetimeClass::Readback
        );
        assert_eq!(
            RendererResourceDescriptor::UploadBuffer(upload).default_lifetime(),
            RendererResourceLifetimeClass::Upload
        );
        assert!(
            RendererResourceDescriptor::ExternalTexture(external)
                .usage_flags()
                .contains(RendererResourceUsageFlags::EXTERNAL_PRODUCER)
        );
        assert_eq!(
            RendererResourceDescriptor::Texture(texture).memory_size_bytes(),
            64 * 32 * 8
        );
    }

    #[test]
    fn renderer_resource_registry_tracks_identity_lifetime_and_memory() {
        let mut registry = RendererResourceRegistry::new();
        registry.begin_frame(5);
        let scene_color = TextureDesc::new_2d(
            crate::ir::IrTextureId::new(10),
            "scene.color",
            128,
            64,
            TextureFormat::Rgba8Unorm,
            TextureUsageFlags::RENDER_TARGET.union(TextureUsageFlags::SAMPLED),
        );
        let upload = UploadBufferDesc::new(
            BufferDesc::new(
                crate::ir::IrBufferId::new(11),
                "upload.object_table",
                1024,
                BufferUsageFlags::COPY_SRC,
                BufferMemoryClass::Upload,
            ),
            0,
        );

        let color_id =
            registry.create_with_default_lifetime(RendererResourceDescriptor::Texture(scene_color));
        let upload_id =
            registry.create_with_default_lifetime(RendererResourceDescriptor::UploadBuffer(upload));
        let color_realization = BridgeResourceRealization::new(
            ResourceBridgeKind::Wgpu,
            BridgeResourceHandleKind::Texture,
            3,
            1,
        );

        registry
            .set_bridge_realization(color_id, color_realization)
            .expect("resource id should be live");
        registry
            .update_graph_state(
                color_id,
                RendererResourceGraphState {
                    graph_resource: IrGraphResourceId::new(4),
                    resource_ref: IrResourceRef::render_target(scene_color.id),
                    access: ResourceAccess::Write,
                    last_pass: IrGraphPassId::new(9),
                },
                6,
            )
            .expect("graph state should update for live resource");

        let telemetry = registry.memory_telemetry();
        assert_eq!(telemetry.persistent_bytes, 128 * 64 * 4);
        assert_eq!(telemetry.upload_bytes, 1024);
        assert_eq!(telemetry.resources_created, 2);
        assert_eq!(telemetry.bridge_realization_count, 1);
        assert_eq!(registry.get(color_id).unwrap().last_used_frame, 6);
        assert_eq!(
            registry.get(color_id).unwrap().current_graph_state.access,
            ResourceAccess::Write
        );

        registry
            .retire(color_id, 12)
            .expect("retire should mark fence");
        registry.begin_frame(7);
        assert_eq!(registry.destroy_retired_by_fence(11), 0);
        assert_eq!(registry.destroy_retired_by_fence(12), 1);
        let telemetry = registry.memory_telemetry();
        assert_eq!(telemetry.resources_destroyed, 1);
        assert_eq!(telemetry.resources_retired_by_fence, 1);
        assert_eq!(telemetry.bridge_realization_count, 0);
        assert_eq!(telemetry.persistent_bytes, 0);
        assert_eq!(telemetry.upload_bytes, 1024);
        assert_eq!(
            registry.get(color_id),
            Err(RendererResourceRegistryError::StaleResourceId)
        );
        assert!(registry.get(upload_id).is_ok());
    }

    #[test]
    fn renderer_resource_registry_rejects_stale_reused_ids() {
        let mut registry = RendererResourceRegistry::new();
        let first = registry.create_with_default_lifetime(RendererResourceDescriptor::Buffer(
            BufferDesc::new(
                crate::ir::IrBufferId::new(1),
                "scene.object_table",
                128,
                BufferUsageFlags::STORAGE,
                BufferMemoryClass::DeviceLocal,
            ),
        ));
        registry
            .destroy_now(first)
            .expect("first resource destroyed");
        let second = registry.create_with_default_lifetime(RendererResourceDescriptor::Buffer(
            BufferDesc::new(
                crate::ir::IrBufferId::new(2),
                "scene.transform_table",
                256,
                BufferUsageFlags::STORAGE,
                BufferMemoryClass::DeviceLocal,
            ),
        ));

        assert_eq!(first.slot(), second.slot());
        assert_ne!(first.generation(), second.generation());
        assert_eq!(
            registry.get(first),
            Err(RendererResourceRegistryError::StaleResourceId)
        );
        assert_eq!(registry.get(second).unwrap().memory_size_bytes, 256);
    }

    #[test]
    fn resource_memory_telemetry_tracks_pass10_lifetime_buckets() {
        let mut registry = RendererResourceRegistry::new();
        registry.begin_frame(3);
        let transient_texture = TextureDesc::new_2d(
            crate::ir::IrTextureId::new(21),
            "graph.transient_color",
            16,
            16,
            TextureFormat::Rgba8Unorm,
            TextureUsageFlags::RENDER_TARGET,
        );
        let external_texture = TextureDesc::new_2d(
            crate::ir::IrTextureId::new(22),
            "native_ui.external_color",
            8,
            8,
            TextureFormat::Rgba8Unorm,
            TextureUsageFlags::SAMPLED,
        );
        let mut debug_readback = ReadbackBufferDesc::new(
            BufferDesc::new(
                crate::ir::IrBufferId::new(23),
                "debug.capture_readback",
                512,
                BufferUsageFlags::COPY_DST,
                BufferMemoryClass::Readback,
            ),
            64,
        );
        debug_readback.debug_only = true;

        registry.create(
            RendererResourceDescriptor::TransientTexture(TransientTextureDesc::new(
                transient_texture,
                IrGraphResourceId::new(30),
            )),
            RendererResourceLifetimeClass::FrameLocal,
            "graph.transient_color",
        );
        registry.create_with_default_lifetime(RendererResourceDescriptor::ExternalTexture(
            ExternalTextureDesc::new(
                external_texture,
                ExternalTextureProducer::NativeUi,
                true,
                false,
            ),
        ));
        let debug_id = registry.create_with_default_lifetime(
            RendererResourceDescriptor::ReadbackBuffer(debug_readback),
        );

        let telemetry = registry.memory_telemetry();
        assert_eq!(telemetry.transient_bytes, 16 * 16 * 4);
        assert_eq!(telemetry.imported_external_bytes, 8 * 8 * 4);
        assert_eq!(telemetry.debug_bytes, 512);
        assert_eq!(telemetry.high_water_marks.debug_bytes, 512);

        registry.destroy_now(debug_id).unwrap();
        let telemetry = registry.memory_telemetry();
        assert_eq!(telemetry.debug_bytes, 0);
        assert_eq!(telemetry.resources_destroyed, 1);
        assert_eq!(telemetry.high_water_marks.debug_bytes, 512);
    }
}
