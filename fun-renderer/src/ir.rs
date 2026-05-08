use crate::backend::{
    BackendCapabilityReport, BackendFeature, BackendFeatureSupport, MissingFeatureReason,
};

pub const RENDERER_IR_SCHEMA_VERSION: u16 = 1;
pub const SHADER_IR_SCHEMA_VERSION: u16 = 1;
pub const PIPELINE_IR_SCHEMA_VERSION: u16 = 1;
pub const MATERIAL_BINDING_IR_SCHEMA_VERSION: u16 = 1;
pub const GRAPH_IR_SCHEMA_VERSION: u16 = 1;
pub const MAX_BINDINGS_PER_LAYOUT: usize = 8;
pub const MAX_BINDINGS_PER_TABLE: usize = 8;
pub const MAX_BIND_LAYOUTS_PER_PIPELINE: usize = 4;
pub const MAX_RENDER_TARGETS_PER_PASS: usize = 4;
pub const MAX_GRAPH_RESOURCE_USES_PER_PASS: usize = 8;
pub const PUBLIC_RENDERER_IR_FORBIDDEN_TERMS: [&str; 10] = [
    "wgpu",
    "wgpu_core",
    "wgpu_hal",
    "ID3D12",
    "VkDevice",
    "MTLDevice",
    "CommandEncoder",
    "DescriptorHeap",
    "QueueHandle",
    "DeviceHandle",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererIrSchemaVersions {
    pub shader_ir_schema: u16,
    pub pipeline_ir_schema: u16,
    pub material_binding_schema: u16,
    pub graph_ir_schema: u16,
}

pub const RENDERER_IR_SCHEMA_VERSIONS: RendererIrSchemaVersions = RendererIrSchemaVersions {
    shader_ir_schema: SHADER_IR_SCHEMA_VERSION,
    pipeline_ir_schema: PIPELINE_IR_SCHEMA_VERSION,
    material_binding_schema: MATERIAL_BINDING_IR_SCHEMA_VERSION,
    graph_ir_schema: GRAPH_IR_SCHEMA_VERSION,
};

macro_rules! ir_id {
    ($name:ident) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);

        impl $name {
            pub const INVALID: Self = Self(u32::MAX);

            #[must_use]
            pub const fn new(index: u32) -> Self {
                Self(index)
            }

            #[must_use]
            pub const fn is_valid(self) -> bool {
                self.0 != u32::MAX
            }
        }
    };
}

ir_id!(IrBufferId);
ir_id!(IrTextureId);
ir_id!(IrSamplerId);
ir_id!(IrBindLayoutId);
ir_id!(IrBindTableId);
ir_id!(IrShaderModuleId);
ir_id!(IrPipelineLayoutId);
ir_id!(IrRenderPipelineId);
ir_id!(IrComputePipelineId);
ir_id!(IrPipelineFamilyId);
ir_id!(IrGraphPassId);
ir_id!(IrGraphResourceId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IrResourceKind {
    Buffer,
    Texture,
    Sampler,
    RenderTarget,
    DepthTarget,
    Transient,
    Imported,
}

impl IrResourceKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buffer => "buffer",
            Self::Texture => "texture",
            Self::Sampler => "sampler",
            Self::RenderTarget => "render_target",
            Self::DepthTarget => "depth_target",
            Self::Transient => "transient",
            Self::Imported => "imported",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IrResourceRef {
    pub kind: IrResourceKind,
    pub index: u32,
}

impl IrResourceRef {
    pub const INVALID: Self = Self {
        kind: IrResourceKind::Buffer,
        index: u32::MAX,
    };

    #[must_use]
    pub const fn buffer(id: IrBufferId) -> Self {
        Self {
            kind: IrResourceKind::Buffer,
            index: id.0,
        }
    }

    #[must_use]
    pub const fn texture(id: IrTextureId) -> Self {
        Self {
            kind: IrResourceKind::Texture,
            index: id.0,
        }
    }

    #[must_use]
    pub const fn sampler(id: IrSamplerId) -> Self {
        Self {
            kind: IrResourceKind::Sampler,
            index: id.0,
        }
    }

    #[must_use]
    pub const fn render_target(id: IrTextureId) -> Self {
        Self {
            kind: IrResourceKind::RenderTarget,
            index: id.0,
        }
    }

    #[must_use]
    pub const fn depth_target(id: IrTextureId) -> Self {
        Self {
            kind: IrResourceKind::DepthTarget,
            index: id.0,
        }
    }

    #[must_use]
    pub const fn graph_resource(kind: IrResourceKind, id: IrGraphResourceId) -> Self {
        Self { kind, index: id.0 }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.index != u32::MAX
    }
}

impl Default for IrResourceKind {
    fn default() -> Self {
        Self::Buffer
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferMemoryClass {
    Upload,
    #[default]
    DeviceLocal,
    Readback,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferUsageFlags(pub u16);

impl BufferUsageFlags {
    pub const NONE: Self = Self(0);
    pub const VERTEX: Self = Self(1 << 0);
    pub const INDEX: Self = Self(1 << 1);
    pub const UNIFORM: Self = Self(1 << 2);
    pub const STORAGE: Self = Self(1 << 3);
    pub const INDIRECT: Self = Self(1 << 4);
    pub const COPY_SRC: Self = Self(1 << 5);
    pub const COPY_DST: Self = Self(1 << 6);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferDesc {
    pub id: IrBufferId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub size_bytes: u64,
    pub usage: BufferUsageFlags,
    pub memory: BufferMemoryClass,
}

impl BufferDesc {
    #[must_use]
    pub const fn new(
        id: IrBufferId,
        stable_name: &'static str,
        size_bytes: u64,
        usage: BufferUsageFlags,
        memory: BufferMemoryClass,
    ) -> Self {
        Self {
            id,
            schema_version: RENDERER_IR_SCHEMA_VERSION,
            stable_name,
            size_bytes,
            usage,
            memory,
        }
    }

    #[must_use]
    pub fn fingerprint(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            self.stable_name,
            &[
                u64::from(self.schema_version),
                u64::from(self.id.0),
                self.size_bytes,
                u64::from(self.usage.0),
                self.memory as u64,
            ],
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureDimension {
    D1,
    #[default]
    D2,
    D3,
    Cube,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    #[default]
    Undefined,
    Rgba8Srgb,
    Rgba8Unorm,
    Rgba16Float,
    Rg16Float,
    R32Uint,
    Depth32Float,
    Depth24Stencil8,
}

impl TextureFormat {
    #[must_use]
    pub const fn is_color(self) -> bool {
        matches!(
            self,
            Self::Rgba8Srgb
                | Self::Rgba8Unorm
                | Self::Rgba16Float
                | Self::Rg16Float
                | Self::R32Uint
        )
    }

    #[must_use]
    pub const fn is_depth(self) -> bool {
        matches!(self, Self::Depth32Float | Self::Depth24Stencil8)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureUsageFlags(pub u16);

impl TextureUsageFlags {
    pub const NONE: Self = Self(0);
    pub const SAMPLED: Self = Self(1 << 0);
    pub const RENDER_TARGET: Self = Self(1 << 1);
    pub const DEPTH_TARGET: Self = Self(1 << 2);
    pub const STORAGE: Self = Self(1 << 3);
    pub const COPY_SRC: Self = Self(1 << 4);
    pub const COPY_DST: Self = Self(1 << 5);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureDesc {
    pub id: IrTextureId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub dimension: TextureDimension,
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub mip_levels: u16,
    pub sample_count: u8,
    pub format: TextureFormat,
    pub usage: TextureUsageFlags,
}

impl TextureDesc {
    #[must_use]
    pub const fn new_2d(
        id: IrTextureId,
        stable_name: &'static str,
        width: u32,
        height: u32,
        format: TextureFormat,
        usage: TextureUsageFlags,
    ) -> Self {
        Self {
            id,
            schema_version: RENDERER_IR_SCHEMA_VERSION,
            stable_name,
            dimension: TextureDimension::D2,
            width,
            height,
            depth_or_layers: 1,
            mip_levels: 1,
            sample_count: 1,
            format,
            usage,
        }
    }

    #[must_use]
    pub fn fingerprint(self) -> DescriptorFingerprint {
        DescriptorFingerprint::from_label_and_words(
            self.stable_name,
            &[
                u64::from(self.schema_version),
                u64::from(self.id.0),
                self.dimension as u64,
                u64::from(self.width),
                u64::from(self.height),
                u64::from(self.depth_or_layers),
                u64::from(self.mip_levels),
                u64::from(self.sample_count),
                self.format as u64,
                u64::from(self.usage.0),
            ],
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterMode {
    Nearest,
    #[default]
    Linear,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddressMode {
    #[default]
    ClampToEdge,
    Repeat,
    MirrorRepeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    pub id: IrSamplerId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub min_filter: FilterMode,
    pub mag_filter: FilterMode,
    pub mip_filter: FilterMode,
    pub address_u: AddressMode,
    pub address_v: AddressMode,
    pub address_w: AddressMode,
}

impl Default for SamplerDesc {
    fn default() -> Self {
        Self {
            id: IrSamplerId::INVALID,
            schema_version: RENDERER_IR_SCHEMA_VERSION,
            stable_name: "",
            min_filter: FilterMode::Linear,
            mag_filter: FilterMode::Linear,
            mip_filter: FilterMode::Linear,
            address_u: AddressMode::ClampToEdge,
            address_v: AddressMode::ClampToEdge,
            address_w: AddressMode::ClampToEdge,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadOp {
    Load,
    #[default]
    Clear,
    DontCare,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StoreOp {
    #[default]
    Store,
    DontCare,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderTargetDesc {
    pub texture: IrTextureId,
    pub format: TextureFormat,
    pub sample_count: u8,
    pub load: LoadOp,
    pub store: StoreOp,
    pub clear_color: [f32; 4],
}

impl Default for RenderTargetDesc {
    fn default() -> Self {
        Self {
            texture: IrTextureId::INVALID,
            format: TextureFormat::Undefined,
            sample_count: 1,
            load: LoadOp::Clear,
            store: StoreOp::Store,
            clear_color: [0.0, 0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthTargetDesc {
    pub texture: IrTextureId,
    pub format: TextureFormat,
    pub sample_count: u8,
    pub load: LoadOp,
    pub store: StoreOp,
    pub clear_depth: f32,
}

impl Default for DepthTargetDesc {
    fn default() -> Self {
        Self {
            texture: IrTextureId::INVALID,
            format: TextureFormat::Undefined,
            sample_count: 1,
            load: LoadOp::Clear,
            store: StoreOp::Store,
            clear_depth: 1.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceLifetime {
    #[default]
    Frame,
    Graph,
    Persistent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransientResourceDesc {
    pub resource: IrGraphResourceId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub backing: IrResourceRef,
    pub lifetime: ResourceLifetime,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportedResourceSource {
    #[default]
    Swapchain,
    CefGpuSurface,
    VendorSdk,
    ExternalTool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImportedResourceDesc {
    pub resource: IrGraphResourceId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub backing: IrResourceRef,
    pub source: ImportedResourceSource,
    pub readable: bool,
    pub writable: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingType {
    UniformBuffer,
    StorageBuffer,
    SampledTexture,
    StorageTexture,
    Sampler,
    #[default]
    RootConstant,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderStageMask(pub u8);

impl ShaderStageMask {
    pub const NONE: Self = Self(0);
    pub const VERTEX: Self = Self(1 << 0);
    pub const FRAGMENT: Self = Self(1 << 1);
    pub const COMPUTE: Self = Self(1 << 2);
    pub const MESH: Self = Self(1 << 3);
    pub const ALL_GRAPHICS: Self = Self(Self::VERTEX.0 | Self::FRAGMENT.0 | Self::MESH.0);

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
pub struct BindSlotDesc {
    pub binding: u8,
    pub binding_type: BindingType,
    pub stages: ShaderStageMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindLayoutDesc {
    pub id: IrBindLayoutId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub slots: [BindSlotDesc; MAX_BINDINGS_PER_LAYOUT],
    pub slot_count: u8,
}

impl Default for BindLayoutDesc {
    fn default() -> Self {
        Self {
            id: IrBindLayoutId::INVALID,
            schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
            stable_name: "",
            slots: [BindSlotDesc::default(); MAX_BINDINGS_PER_LAYOUT],
            slot_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingResourceRef {
    pub binding: u8,
    pub binding_type: BindingType,
    pub resource: IrResourceRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindTableDesc {
    pub id: IrBindTableId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub layout: IrBindLayoutId,
    pub bindings: [BindingResourceRef; MAX_BINDINGS_PER_TABLE],
    pub binding_count: u8,
}

impl Default for BindTableDesc {
    fn default() -> Self {
        Self {
            id: IrBindTableId::INVALID,
            schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
            stable_name: "",
            layout: IrBindLayoutId::INVALID,
            bindings: [BindingResourceRef::default(); MAX_BINDINGS_PER_TABLE],
            binding_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindlessTableDesc {
    pub id: IrBindTableId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub resource_kind: IrResourceKind,
    pub capacity: u32,
    pub resident_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootConstantDesc {
    pub slot: u8,
    pub byte_offset: u16,
    pub byte_count: u16,
    pub stages: ShaderStageMask,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialBindingDesc {
    pub material_table: IrBindTableId,
    pub material_layout: IrBindLayoutId,
    pub material_schema_version: u16,
    pub feature_mask_bits: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewBindingDesc {
    pub view_table: IrBindTableId,
    pub view_layout: IrBindLayoutId,
    pub view_index: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneBindingDesc {
    pub scene_table: IrBindTableId,
    pub scene_layout: IrBindLayoutId,
    pub object_table: IrResourceRef,
    pub transform_table: IrResourceRef,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderSourceLanguage {
    #[default]
    Wgsl,
    NagaIr,
    SpirV,
    Hlsl,
    Msl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderModuleDesc {
    pub id: IrShaderModuleId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub language: ShaderSourceLanguage,
    pub entry_point: &'static str,
    pub stages: ShaderStageMask,
    pub source_digest: u64,
    pub requires_naga_translation: bool,
}

impl ShaderModuleDesc {
    #[must_use]
    pub const fn wgsl(
        id: IrShaderModuleId,
        stable_name: &'static str,
        entry_point: &'static str,
        stages: ShaderStageMask,
        source_digest: u64,
    ) -> Self {
        Self {
            id,
            schema_version: SHADER_IR_SCHEMA_VERSION,
            stable_name,
            language: ShaderSourceLanguage::Wgsl,
            entry_point,
            stages,
            source_digest,
            requires_naga_translation: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineLayoutDesc {
    pub id: IrPipelineLayoutId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub bind_layouts: [IrBindLayoutId; MAX_BIND_LAYOUTS_PER_PIPELINE],
    pub bind_layout_count: u8,
    pub root_constants: [RootConstantDesc; MAX_BIND_LAYOUTS_PER_PIPELINE],
    pub root_constant_count: u8,
}

impl Default for PipelineLayoutDesc {
    fn default() -> Self {
        Self {
            id: IrPipelineLayoutId::INVALID,
            schema_version: PIPELINE_IR_SCHEMA_VERSION,
            stable_name: "",
            bind_layouts: [IrBindLayoutId::INVALID; MAX_BIND_LAYOUTS_PER_PIPELINE],
            bind_layout_count: 0,
            root_constants: [RootConstantDesc::default(); MAX_BIND_LAYOUTS_PER_PIPELINE],
            root_constant_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveTopology {
    #[default]
    TriangleList,
    TriangleStrip,
    LineList,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    #[default]
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPipelineDesc {
    pub id: IrRenderPipelineId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub layout: IrPipelineLayoutId,
    pub vertex_shader: IrShaderModuleId,
    pub fragment_shader: IrShaderModuleId,
    pub color_formats: [TextureFormat; MAX_RENDER_TARGETS_PER_PASS],
    pub color_target_count: u8,
    pub depth_format: TextureFormat,
    pub sample_count: u8,
    pub topology: PrimitiveTopology,
    pub cull_mode: CullMode,
    pub requires_mesh_shader: bool,
}

impl Default for RenderPipelineDesc {
    fn default() -> Self {
        Self {
            id: IrRenderPipelineId::INVALID,
            schema_version: PIPELINE_IR_SCHEMA_VERSION,
            stable_name: "",
            layout: IrPipelineLayoutId::INVALID,
            vertex_shader: IrShaderModuleId::INVALID,
            fragment_shader: IrShaderModuleId::INVALID,
            color_formats: [TextureFormat::Undefined; MAX_RENDER_TARGETS_PER_PASS],
            color_target_count: 0,
            depth_format: TextureFormat::Undefined,
            sample_count: 1,
            topology: PrimitiveTopology::TriangleList,
            cull_mode: CullMode::Back,
            requires_mesh_shader: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComputePipelineDesc {
    pub id: IrComputePipelineId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub layout: IrPipelineLayoutId,
    pub compute_shader: IrShaderModuleId,
    pub requires_work_graphs: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineVariantKey {
    pub family: IrPipelineFamilyId,
    pub quality_tier: u8,
    pub msaa_samples: u8,
    pub material_features: u32,
    pub backend_mask: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineFamily {
    pub id: IrPipelineFamilyId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub render_pipeline: IrRenderPipelineId,
    pub compute_pipeline: IrComputePipelineId,
    pub variant_axis_mask: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PacketRange {
    pub start: u32,
    pub count: u32,
}

impl PacketRange {
    #[must_use]
    pub const fn empty() -> Self {
        Self { start: 0, count: 0 }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.count == 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawPacket {
    pub object_index: u32,
    pub mesh_index: u32,
    pub material_index: u32,
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
    pub instance_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DispatchPacket {
    pub group_count_x: u32,
    pub group_count_y: u32,
    pub group_count_z: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IndirectDrawPacket {
    pub args_buffer: IrBufferId,
    pub args_offset_bytes: u64,
    pub packet_count: u32,
    pub stride_bytes: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPassCommand {
    pub pass: IrGraphPassId,
    pub pipeline: IrRenderPipelineId,
    pub material_bindings: MaterialBindingDesc,
    pub view_bindings: ViewBindingDesc,
    pub scene_bindings: SceneBindingDesc,
    pub draw_packets: PacketRange,
    pub indirect_draw_packets: PacketRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComputePassCommand {
    pub pass: IrGraphPassId,
    pub pipeline: IrComputePipelineId,
    pub bind_table_range: PacketRange,
    pub dispatch_packets: PacketRange,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CopyCommand {
    pub src: IrResourceRef,
    pub dst: IrResourceRef,
    pub src_offset_bytes: u64,
    pub dst_offset_bytes: u64,
    pub byte_count: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphPassKind {
    #[default]
    Render,
    Compute,
    Copy,
    Present,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceAccess {
    #[default]
    Read,
    Write,
    ReadWrite,
    Present,
}

impl ResourceAccess {
    #[must_use]
    pub const fn reads(self) -> bool {
        matches!(self, Self::Read | Self::ReadWrite | Self::Present)
    }

    #[must_use]
    pub const fn writes(self) -> bool {
        matches!(self, Self::Write | Self::ReadWrite)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphResourceUse {
    pub resource: IrGraphResourceId,
    pub resource_ref: IrResourceRef,
    pub access: ResourceAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphPassDesc {
    pub id: IrGraphPassId,
    pub schema_version: u16,
    pub stable_name: &'static str,
    pub kind: GraphPassKind,
    pub uses: [GraphResourceUse; MAX_GRAPH_RESOURCE_USES_PER_PASS],
    pub use_count: u8,
    pub render_command: Option<RenderPassCommand>,
    pub compute_command: Option<ComputePassCommand>,
    pub copy_command: Option<CopyCommand>,
}

impl Default for GraphPassDesc {
    fn default() -> Self {
        Self {
            id: IrGraphPassId::INVALID,
            schema_version: GRAPH_IR_SCHEMA_VERSION,
            stable_name: "",
            kind: GraphPassKind::Render,
            uses: [GraphResourceUse::default(); MAX_GRAPH_RESOURCE_USES_PER_PASS],
            use_count: 0,
            render_command: None,
            compute_command: None,
            copy_command: None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GraphEdge {
    pub from: IrGraphPassId,
    pub to: IrGraphPassId,
    pub resource: IrGraphResourceId,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassExecutionPlan {
    pub pass: IrGraphPassId,
    pub order: u16,
    pub command_kind: GraphPassKind,
    pub render_command: Option<RenderPassCommand>,
    pub compute_command: Option<ComputePassCommand>,
    pub copy_command: Option<CopyCommand>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CompiledGraph {
    pub schema_version: u16,
    pub passes: Vec<PassExecutionPlan>,
    pub edges: Vec<GraphEdge>,
    pub graph_fingerprint: DescriptorFingerprint,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct RendererIrCatalog {
    pub schema_versions: RendererIrSchemaVersions,
    pub buffers: Vec<BufferDesc>,
    pub textures: Vec<TextureDesc>,
    pub samplers: Vec<SamplerDesc>,
    pub render_targets: Vec<RenderTargetDesc>,
    pub depth_targets: Vec<DepthTargetDesc>,
    pub transients: Vec<TransientResourceDesc>,
    pub imports: Vec<ImportedResourceDesc>,
    pub bind_layouts: Vec<BindLayoutDesc>,
    pub bind_tables: Vec<BindTableDesc>,
    pub bindless_tables: Vec<BindlessTableDesc>,
    pub shaders: Vec<ShaderModuleDesc>,
    pub pipeline_layouts: Vec<PipelineLayoutDesc>,
    pub render_pipelines: Vec<RenderPipelineDesc>,
    pub compute_pipelines: Vec<ComputePipelineDesc>,
    pub pipeline_families: Vec<PipelineFamily>,
    pub graph_passes: Vec<GraphPassDesc>,
    pub graph_edges: Vec<GraphEdge>,
    pub draw_packets: Vec<DrawPacket>,
    pub dispatch_packets: Vec<DispatchPacket>,
    pub indirect_draw_packets: Vec<IndirectDrawPacket>,
}

impl Default for RendererIrSchemaVersions {
    fn default() -> Self {
        RENDERER_IR_SCHEMA_VERSIONS
    }
}

impl RendererIrCatalog {
    #[must_use]
    pub fn has_resource(&self, resource: IrResourceRef) -> bool {
        if !resource.is_valid() {
            return false;
        }
        match resource.kind {
            IrResourceKind::Buffer => self.buffers.iter().any(|desc| desc.id.0 == resource.index),
            IrResourceKind::Texture => self.textures.iter().any(|desc| desc.id.0 == resource.index),
            IrResourceKind::Sampler => self.samplers.iter().any(|desc| desc.id.0 == resource.index),
            IrResourceKind::RenderTarget => self
                .render_targets
                .iter()
                .any(|desc| desc.texture.0 == resource.index),
            IrResourceKind::DepthTarget => self
                .depth_targets
                .iter()
                .any(|desc| desc.texture.0 == resource.index),
            IrResourceKind::Transient => self
                .transients
                .iter()
                .any(|desc| desc.resource.0 == resource.index),
            IrResourceKind::Imported => self
                .imports
                .iter()
                .any(|desc| desc.resource.0 == resource.index),
        }
    }

    #[must_use]
    pub fn has_bind_layout(&self, layout: IrBindLayoutId) -> bool {
        layout.is_valid() && self.bind_layouts.iter().any(|desc| desc.id == layout)
    }

    #[must_use]
    pub fn has_bind_table(&self, table: IrBindTableId) -> bool {
        table.is_valid() && self.bind_tables.iter().any(|desc| desc.id == table)
    }

    #[must_use]
    pub fn has_shader(&self, shader: IrShaderModuleId) -> bool {
        shader.is_valid() && self.shaders.iter().any(|desc| desc.id == shader)
    }

    #[must_use]
    pub fn has_pipeline_layout(&self, layout: IrPipelineLayoutId) -> bool {
        layout.is_valid() && self.pipeline_layouts.iter().any(|desc| desc.id == layout)
    }

    #[must_use]
    pub fn has_render_pipeline(&self, pipeline: IrRenderPipelineId) -> bool {
        pipeline.is_valid() && self.render_pipelines.iter().any(|desc| desc.id == pipeline)
    }

    #[must_use]
    pub fn has_compute_pipeline(&self, pipeline: IrComputePipelineId) -> bool {
        pipeline.is_valid()
            && self
                .compute_pipelines
                .iter()
                .any(|desc| desc.id == pipeline)
    }

    #[must_use]
    pub fn has_graph_pass(&self, pass: IrGraphPassId) -> bool {
        pass.is_valid() && self.graph_passes.iter().any(|desc| desc.id == pass)
    }

    #[must_use]
    pub fn graph_pass(&self, pass: IrGraphPassId) -> Option<&GraphPassDesc> {
        self.graph_passes.iter().find(|desc| desc.id == pass)
    }

    #[must_use]
    pub fn compile_graph(
        &self,
        backend: BackendCapabilityReport,
    ) -> Result<CompiledGraph, IrValidationReport> {
        let report = validate_renderer_ir(self, backend);
        if !report.valid {
            return Err(report);
        }
        let mut plans = Vec::with_capacity(self.graph_passes.len());
        for (order, pass) in self.graph_passes.iter().enumerate() {
            plans.push(PassExecutionPlan {
                pass: pass.id,
                order: order as u16,
                command_kind: pass.kind,
                render_command: pass.render_command,
                compute_command: pass.compute_command,
                copy_command: pass.copy_command,
            });
        }
        let mut edges = self.graph_edges.clone();
        edges.sort_by_key(|edge| (edge.from.0, edge.to.0, edge.resource.0));
        Ok(CompiledGraph {
            schema_version: GRAPH_IR_SCHEMA_VERSION,
            passes: plans,
            edges,
            graph_fingerprint: graph_fingerprint(&self.graph_passes, &self.graph_edges),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrValidationCode {
    SchemaVersionMismatch,
    MissingResource,
    InvalidFormat,
    InvalidSampleCount,
    LayoutMismatch,
    GraphDependencyMismatch,
    PassReadsUnwrittenResource,
    PassWritesUnconsumedResource,
    UnsupportedBackendFeature,
}

impl IrValidationCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaVersionMismatch => "schema_version_mismatch",
            Self::MissingResource => "missing_resource",
            Self::InvalidFormat => "invalid_format",
            Self::InvalidSampleCount => "invalid_sample_count",
            Self::LayoutMismatch => "layout_mismatch",
            Self::GraphDependencyMismatch => "graph_dependency_mismatch",
            Self::PassReadsUnwrittenResource => "pass_reads_unwritten_resource",
            Self::PassWritesUnconsumedResource => "pass_writes_unconsumed_resource",
            Self::UnsupportedBackendFeature => "unsupported_backend_feature",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrValidationError {
    pub code: IrValidationCode,
    pub subject: &'static str,
    pub detail: &'static str,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IrValidationReport {
    pub valid: bool,
    pub errors: Vec<IrValidationError>,
}

impl IrValidationReport {
    fn push(&mut self, code: IrValidationCode, subject: &'static str, detail: &'static str) {
        self.valid = false;
        self.errors.push(IrValidationError {
            code,
            subject,
            detail,
        });
    }

    #[must_use]
    pub fn contains(&self, code: IrValidationCode) -> bool {
        self.errors.iter().any(|error| error.code == code)
    }
}

#[must_use]
pub fn validate_renderer_ir(
    catalog: &RendererIrCatalog,
    backend: BackendCapabilityReport,
) -> IrValidationReport {
    let mut report = IrValidationReport {
        valid: true,
        errors: Vec::new(),
    };
    validate_schema_versions(catalog, &mut report);
    validate_resources(catalog, &mut report);
    validate_bindings(catalog, &mut report);
    validate_pipelines(catalog, backend, &mut report);
    validate_graph(catalog, backend, &mut report);
    report
}

fn validate_schema_versions(catalog: &RendererIrCatalog, report: &mut IrValidationReport) {
    if catalog.schema_versions != RENDERER_IR_SCHEMA_VERSIONS {
        report.push(
            IrValidationCode::SchemaVersionMismatch,
            "renderer_ir_schema_versions",
            "catalog schema versions must match the compiled renderer IR versions",
        );
    }
}

fn validate_resources(catalog: &RendererIrCatalog, report: &mut IrValidationReport) {
    for buffer in &catalog.buffers {
        if !buffer.id.is_valid() || buffer.size_bytes == 0 || buffer.usage == BufferUsageFlags::NONE
        {
            report.push(
                IrValidationCode::MissingResource,
                buffer.stable_name,
                "buffer descriptors need a valid id, non-zero size, and non-empty usage",
            );
        }
    }

    for texture in &catalog.textures {
        if texture.format == TextureFormat::Undefined
            || texture.width == 0
            || texture.height == 0
            || texture.depth_or_layers == 0
            || texture.mip_levels == 0
        {
            report.push(
                IrValidationCode::InvalidFormat,
                texture.stable_name,
                "texture descriptors need a valid format and non-zero extent/mips",
            );
        }
        if !valid_sample_count(texture.sample_count) {
            report.push(
                IrValidationCode::InvalidSampleCount,
                texture.stable_name,
                "texture sample count must be 1, 2, 4, or 8",
            );
        }
    }

    for target in &catalog.render_targets {
        if !target.format.is_color() || !valid_sample_count(target.sample_count) {
            report.push(
                IrValidationCode::InvalidFormat,
                "render_target",
                "render targets need color formats and valid sample counts",
            );
        }
        if !catalog.has_resource(IrResourceRef::texture(target.texture)) {
            report.push(
                IrValidationCode::MissingResource,
                "render_target",
                "render target texture must exist",
            );
        }
    }

    for target in &catalog.depth_targets {
        if !target.format.is_depth() || !valid_sample_count(target.sample_count) {
            report.push(
                IrValidationCode::InvalidFormat,
                "depth_target",
                "depth targets need depth formats and valid sample counts",
            );
        }
        if !catalog.has_resource(IrResourceRef::texture(target.texture)) {
            report.push(
                IrValidationCode::MissingResource,
                "depth_target",
                "depth target texture must exist",
            );
        }
    }
}

fn validate_bindings(catalog: &RendererIrCatalog, report: &mut IrValidationReport) {
    for layout in &catalog.bind_layouts {
        if layout.slot_count as usize > MAX_BINDINGS_PER_LAYOUT {
            report.push(
                IrValidationCode::LayoutMismatch,
                layout.stable_name,
                "bind layout slot_count exceeds fixed slot storage",
            );
        }
    }

    for table in &catalog.bind_tables {
        let Some(layout) = catalog
            .bind_layouts
            .iter()
            .find(|layout| layout.id == table.layout)
        else {
            report.push(
                IrValidationCode::LayoutMismatch,
                table.stable_name,
                "bind table layout is missing",
            );
            continue;
        };

        if table.binding_count > layout.slot_count {
            report.push(
                IrValidationCode::LayoutMismatch,
                table.stable_name,
                "bind table has more bindings than its layout",
            );
        }

        for binding in table.bindings.iter().take(table.binding_count as usize) {
            let Some(slot) = layout
                .slots
                .iter()
                .take(layout.slot_count as usize)
                .find(|slot| slot.binding == binding.binding)
            else {
                report.push(
                    IrValidationCode::LayoutMismatch,
                    table.stable_name,
                    "bind table binding has no matching layout slot",
                );
                continue;
            };
            if slot.binding_type != binding.binding_type {
                report.push(
                    IrValidationCode::LayoutMismatch,
                    table.stable_name,
                    "bind table binding type does not match layout",
                );
            }
            if !binding_resource_matches_type(binding.resource, binding.binding_type) {
                report.push(
                    IrValidationCode::LayoutMismatch,
                    table.stable_name,
                    "binding resource kind does not match binding type",
                );
            }
            if !catalog.has_resource(binding.resource) {
                report.push(
                    IrValidationCode::MissingResource,
                    table.stable_name,
                    "binding references a missing resource",
                );
            }
        }
    }
}

fn validate_pipelines(
    catalog: &RendererIrCatalog,
    backend: BackendCapabilityReport,
    report: &mut IrValidationReport,
) {
    for layout in &catalog.pipeline_layouts {
        for bind_layout in layout
            .bind_layouts
            .iter()
            .copied()
            .take(layout.bind_layout_count as usize)
        {
            if !catalog.has_bind_layout(bind_layout) {
                report.push(
                    IrValidationCode::LayoutMismatch,
                    layout.stable_name,
                    "pipeline layout references a missing bind layout",
                );
            }
        }
    }

    for shader in &catalog.shaders {
        if shader.schema_version != SHADER_IR_SCHEMA_VERSION {
            report.push(
                IrValidationCode::SchemaVersionMismatch,
                shader.stable_name,
                "shader module schema version mismatch",
            );
        }
        if shader.requires_naga_translation
            && !backend_feature_supported(backend, BackendFeature::NagaShaderTranslation)
        {
            report.push(
                IrValidationCode::UnsupportedBackendFeature,
                shader.stable_name,
                "shader requires Naga translation but backend report does not support it",
            );
        }
        if shader.stages.contains(ShaderStageMask::MESH)
            && !backend_feature_supported(backend, BackendFeature::MeshShaders)
        {
            report.push(
                IrValidationCode::UnsupportedBackendFeature,
                shader.stable_name,
                "mesh shader stage requires backend mesh shader support",
            );
        }
    }

    for pipeline in &catalog.render_pipelines {
        if !catalog.has_pipeline_layout(pipeline.layout) {
            report.push(
                IrValidationCode::LayoutMismatch,
                pipeline.stable_name,
                "render pipeline layout is missing",
            );
        }
        if !catalog.has_shader(pipeline.vertex_shader)
            || !catalog.has_shader(pipeline.fragment_shader)
        {
            report.push(
                IrValidationCode::MissingResource,
                pipeline.stable_name,
                "render pipeline shader module is missing",
            );
        }
        if pipeline.color_target_count == 0
            || pipeline
                .color_formats
                .iter()
                .take(pipeline.color_target_count as usize)
                .any(|format| !format.is_color())
        {
            report.push(
                IrValidationCode::InvalidFormat,
                pipeline.stable_name,
                "render pipeline needs at least one color target with a color format",
            );
        }
        if pipeline.depth_format != TextureFormat::Undefined && !pipeline.depth_format.is_depth() {
            report.push(
                IrValidationCode::InvalidFormat,
                pipeline.stable_name,
                "render pipeline depth format must be a depth format",
            );
        }
        if !valid_sample_count(pipeline.sample_count) {
            report.push(
                IrValidationCode::InvalidSampleCount,
                pipeline.stable_name,
                "render pipeline sample count must be 1, 2, 4, or 8",
            );
        }
        if pipeline.requires_mesh_shader
            && !backend_feature_supported(backend, BackendFeature::MeshShaders)
        {
            report.push(
                IrValidationCode::UnsupportedBackendFeature,
                pipeline.stable_name,
                "render pipeline requires mesh shaders",
            );
        }
    }

    for pipeline in &catalog.compute_pipelines {
        if !catalog.has_pipeline_layout(pipeline.layout)
            || !catalog.has_shader(pipeline.compute_shader)
        {
            report.push(
                IrValidationCode::MissingResource,
                pipeline.stable_name,
                "compute pipeline layout or shader is missing",
            );
        }
        if pipeline.requires_work_graphs
            && !backend_feature_supported(backend, BackendFeature::NativeCommandEncoderAccess)
        {
            report.push(
                IrValidationCode::UnsupportedBackendFeature,
                pipeline.stable_name,
                "work graph dispatch requires native command encoder support",
            );
        }
    }
}

fn validate_graph(
    catalog: &RendererIrCatalog,
    backend: BackendCapabilityReport,
    report: &mut IrValidationReport,
) {
    let mut writes: Vec<(IrGraphResourceId, IrGraphPassId)> = Vec::new();
    let mut reads: Vec<IrGraphResourceId> = Vec::new();

    for pass in &catalog.graph_passes {
        if pass.schema_version != GRAPH_IR_SCHEMA_VERSION {
            report.push(
                IrValidationCode::SchemaVersionMismatch,
                pass.stable_name,
                "graph pass schema version mismatch",
            );
        }
        for use_desc in pass.uses.iter().take(pass.use_count as usize) {
            if !catalog.has_resource(use_desc.resource_ref) {
                report.push(
                    IrValidationCode::MissingResource,
                    pass.stable_name,
                    "graph pass references a missing resource",
                );
            }
            if use_desc.access.reads()
                && !is_imported_resource(catalog, use_desc.resource)
                && !writes
                    .iter()
                    .any(|(resource, _)| *resource == use_desc.resource)
            {
                report.push(
                    IrValidationCode::PassReadsUnwrittenResource,
                    pass.stable_name,
                    "pass reads a graph resource before any writer or import",
                );
            }
            if use_desc.access.reads() {
                reads.push(use_desc.resource);
            }
            if use_desc.access.writes() {
                writes.push((use_desc.resource, pass.id));
            }
        }

        if let Some(command) = pass.render_command {
            if !catalog.has_render_pipeline(command.pipeline) {
                report.push(
                    IrValidationCode::MissingResource,
                    pass.stable_name,
                    "render pass command references a missing render pipeline",
                );
            }
            if !command.indirect_draw_packets.is_empty()
                && !backend_feature_supported(backend, BackendFeature::DrawPackets)
            {
                report.push(
                    IrValidationCode::UnsupportedBackendFeature,
                    pass.stable_name,
                    "indirect draw packets require backend draw packet support",
                );
            }
        }
        if let Some(command) = pass.compute_command {
            if !catalog.has_compute_pipeline(command.pipeline) {
                report.push(
                    IrValidationCode::MissingResource,
                    pass.stable_name,
                    "compute pass command references a missing compute pipeline",
                );
            }
        }
    }

    for (resource, writer) in writes.iter().copied() {
        if !reads.contains(&resource) && !is_presented_resource(catalog, resource) {
            let subject = catalog
                .graph_pass(writer)
                .map_or("graph_pass", |pass| pass.stable_name);
            report.push(
                IrValidationCode::PassWritesUnconsumedResource,
                subject,
                "pass writes a graph resource that no later pass reads or presents",
            );
        }
    }

    for edge in &catalog.graph_edges {
        let Some(from) = catalog.graph_pass(edge.from) else {
            report.push(
                IrValidationCode::GraphDependencyMismatch,
                "graph_edge",
                "edge source pass is missing",
            );
            continue;
        };
        let Some(to) = catalog.graph_pass(edge.to) else {
            report.push(
                IrValidationCode::GraphDependencyMismatch,
                "graph_edge",
                "edge destination pass is missing",
            );
            continue;
        };
        let from_writes = from
            .uses
            .iter()
            .take(from.use_count as usize)
            .any(|use_desc| use_desc.resource == edge.resource && use_desc.access.writes());
        let to_reads = to
            .uses
            .iter()
            .take(to.use_count as usize)
            .any(|use_desc| use_desc.resource == edge.resource && use_desc.access.reads());
        if !from_writes || !to_reads {
            report.push(
                IrValidationCode::GraphDependencyMismatch,
                "graph_edge",
                "edge must connect a writer pass to a reader pass for the same resource",
            );
        }
    }
}

fn valid_sample_count(sample_count: u8) -> bool {
    matches!(sample_count, 1 | 2 | 4 | 8)
}

fn binding_resource_matches_type(resource: IrResourceRef, binding_type: BindingType) -> bool {
    match binding_type {
        BindingType::UniformBuffer | BindingType::StorageBuffer => {
            resource.kind == IrResourceKind::Buffer
        }
        BindingType::SampledTexture | BindingType::StorageTexture => {
            matches!(
                resource.kind,
                IrResourceKind::Texture | IrResourceKind::RenderTarget
            )
        }
        BindingType::Sampler => resource.kind == IrResourceKind::Sampler,
        BindingType::RootConstant => true,
    }
}

fn backend_feature_supported(backend: BackendCapabilityReport, feature: BackendFeature) -> bool {
    backend
        .support_for(feature)
        .unwrap_or(BackendFeatureSupport {
            feature,
            supported: false,
            missing_reason: Some(MissingFeatureReason::FeatureNotReportedByBridge),
        })
        .supported
}

fn is_imported_resource(catalog: &RendererIrCatalog, resource: IrGraphResourceId) -> bool {
    catalog
        .imports
        .iter()
        .any(|import| import.resource == resource && import.readable)
}

fn is_presented_resource(catalog: &RendererIrCatalog, resource: IrGraphResourceId) -> bool {
    catalog.graph_passes.iter().any(|pass| {
        pass.uses
            .iter()
            .take(pass.use_count as usize)
            .any(|use_desc| {
                use_desc.resource == resource && use_desc.access == ResourceAccess::Present
            })
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DescriptorFingerprint(pub u64);

impl DescriptorFingerprint {
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn from_label_and_words(label: &'static str, words: &[u64]) -> Self {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in label.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        for word in words {
            for byte in word.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        Self(hash)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IrDescriptorKind {
    #[default]
    ResourceCreation,
    PipelineCreation,
    PassExecution,
    GraphCompile,
}

impl IrDescriptorKind {
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ResourceCreation => 0,
            Self::PipelineCreation => 1,
            Self::PassExecution => 2,
            Self::GraphCompile => 3,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IrDescriptorKey {
    pub kind: IrDescriptorKind,
    pub index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BridgeTranslationReason {
    FirstTranslation,
    DescriptorChanged,
    DescriptorUnchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BridgeTranslationDecision {
    pub key: IrDescriptorKey,
    pub translated: bool,
    pub reason: BridgeTranslationReason,
}

pub trait RendererIrBridgeTranslator {
    fn translate_resource_creation(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision;

    fn translate_pipeline_creation(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision;

    fn translate_pass_execution(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision;

    fn translate_graph_compile(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IrBridgeTranslationCache {
    entries: Vec<(IrDescriptorKey, DescriptorFingerprint)>,
    pub translated_by_kind: [u32; 4],
    pub skipped_unchanged: u32,
}

impl IrBridgeTranslationCache {
    fn translate(
        &mut self,
        mut key: IrDescriptorKey,
        expected_kind: IrDescriptorKind,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        key.kind = expected_kind;
        if let Some((_, existing)) = self
            .entries
            .iter_mut()
            .find(|(candidate, _)| *candidate == key)
        {
            if *existing == fingerprint {
                self.skipped_unchanged = self.skipped_unchanged.saturating_add(1);
                return BridgeTranslationDecision {
                    key,
                    translated: false,
                    reason: BridgeTranslationReason::DescriptorUnchanged,
                };
            }
            *existing = fingerprint;
            self.translated_by_kind[expected_kind.index()] =
                self.translated_by_kind[expected_kind.index()].saturating_add(1);
            return BridgeTranslationDecision {
                key,
                translated: true,
                reason: BridgeTranslationReason::DescriptorChanged,
            };
        }

        self.entries.push((key, fingerprint));
        self.entries
            .sort_by_key(|(key, _)| (key.kind.index(), key.index));
        self.translated_by_kind[expected_kind.index()] =
            self.translated_by_kind[expected_kind.index()].saturating_add(1);
        BridgeTranslationDecision {
            key,
            translated: true,
            reason: BridgeTranslationReason::FirstTranslation,
        }
    }
}

impl RendererIrBridgeTranslator for IrBridgeTranslationCache {
    fn translate_resource_creation(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(key, IrDescriptorKind::ResourceCreation, fingerprint)
    }

    fn translate_pipeline_creation(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(key, IrDescriptorKind::PipelineCreation, fingerprint)
    }

    fn translate_pass_execution(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(key, IrDescriptorKind::PassExecution, fingerprint)
    }

    fn translate_graph_compile(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(key, IrDescriptorKind::GraphCompile, fingerprint)
    }
}

fn graph_fingerprint(passes: &[GraphPassDesc], edges: &[GraphEdge]) -> DescriptorFingerprint {
    let mut words = Vec::with_capacity(passes.len() * 4 + edges.len() * 3);
    for pass in passes {
        words.push(u64::from(pass.id.0));
        words.push(pass.kind as u64);
        words.push(u64::from(pass.use_count));
        words.push(pass.stable_name.len() as u64);
    }
    for edge in edges {
        words.push(u64::from(edge.from.0));
        words.push(u64::from(edge.to.0));
        words.push(u64::from(edge.resource.0));
    }
    DescriptorFingerprint::from_label_and_words("compiled_graph", &words)
}

#[cfg(test)]
mod tests {
    use core::mem::size_of;

    use super::*;
    use crate::backend::BackendCapabilityReport;

    fn basic_catalog() -> RendererIrCatalog {
        let scene_color = IrTextureId::new(0);
        let depth = IrTextureId::new(1);
        let object_buffer = IrBufferId::new(0);
        let sampler = IrSamplerId::new(0);
        let view_layout = IrBindLayoutId::new(0);
        let material_layout = IrBindLayoutId::new(1);
        let view_table = IrBindTableId::new(0);
        let material_table = IrBindTableId::new(1);
        let pipeline_layout = IrPipelineLayoutId::new(0);
        let vertex_shader = IrShaderModuleId::new(0);
        let fragment_shader = IrShaderModuleId::new(1);
        let render_pipeline = IrRenderPipelineId::new(0);
        let scene_resource = IrGraphResourceId::new(0);
        let pass_write = IrGraphPassId::new(0);
        let pass_read = IrGraphPassId::new(1);

        let mut view_layout_desc = BindLayoutDesc {
            id: view_layout,
            stable_name: "view_layout",
            slot_count: 1,
            ..BindLayoutDesc::default()
        };
        view_layout_desc.slots[0] = BindSlotDesc {
            binding: 0,
            binding_type: BindingType::UniformBuffer,
            stages: ShaderStageMask::ALL_GRAPHICS,
        };

        let mut material_layout_desc = BindLayoutDesc {
            id: material_layout,
            stable_name: "material_layout",
            slot_count: 2,
            ..BindLayoutDesc::default()
        };
        material_layout_desc.slots[0] = BindSlotDesc {
            binding: 0,
            binding_type: BindingType::SampledTexture,
            stages: ShaderStageMask::FRAGMENT,
        };
        material_layout_desc.slots[1] = BindSlotDesc {
            binding: 1,
            binding_type: BindingType::Sampler,
            stages: ShaderStageMask::FRAGMENT,
        };

        let mut view_table_desc = BindTableDesc {
            id: view_table,
            stable_name: "view_table",
            layout: view_layout,
            binding_count: 1,
            ..BindTableDesc::default()
        };
        view_table_desc.bindings[0] = BindingResourceRef {
            binding: 0,
            binding_type: BindingType::UniformBuffer,
            resource: IrResourceRef::buffer(object_buffer),
        };

        let mut material_table_desc = BindTableDesc {
            id: material_table,
            stable_name: "material_table",
            layout: material_layout,
            binding_count: 2,
            ..BindTableDesc::default()
        };
        material_table_desc.bindings[0] = BindingResourceRef {
            binding: 0,
            binding_type: BindingType::SampledTexture,
            resource: IrResourceRef::texture(scene_color),
        };
        material_table_desc.bindings[1] = BindingResourceRef {
            binding: 1,
            binding_type: BindingType::Sampler,
            resource: IrResourceRef::sampler(sampler),
        };

        let mut pipeline_layout_desc = PipelineLayoutDesc {
            id: pipeline_layout,
            stable_name: "pbr_layout",
            bind_layout_count: 2,
            ..PipelineLayoutDesc::default()
        };
        pipeline_layout_desc.bind_layouts[0] = view_layout;
        pipeline_layout_desc.bind_layouts[1] = material_layout;

        let render_command = RenderPassCommand {
            pass: pass_write,
            pipeline: render_pipeline,
            material_bindings: MaterialBindingDesc {
                material_table,
                material_layout,
                material_schema_version: MATERIAL_BINDING_IR_SCHEMA_VERSION,
                feature_mask_bits: 0,
            },
            view_bindings: ViewBindingDesc {
                view_table,
                view_layout,
                view_index: 0,
            },
            scene_bindings: SceneBindingDesc {
                scene_table: view_table,
                scene_layout: view_layout,
                object_table: IrResourceRef::buffer(object_buffer),
                transform_table: IrResourceRef::buffer(object_buffer),
            },
            draw_packets: PacketRange { start: 0, count: 1 },
            indirect_draw_packets: PacketRange::empty(),
        };

        let mut write_pass = GraphPassDesc {
            id: pass_write,
            stable_name: "scene_opaque",
            kind: GraphPassKind::Render,
            use_count: 1,
            render_command: Some(render_command),
            ..GraphPassDesc::default()
        };
        write_pass.uses[0] = GraphResourceUse {
            resource: scene_resource,
            resource_ref: IrResourceRef::render_target(scene_color),
            access: ResourceAccess::Write,
        };

        let mut read_pass = GraphPassDesc {
            id: pass_read,
            stable_name: "present",
            kind: GraphPassKind::Present,
            use_count: 1,
            ..GraphPassDesc::default()
        };
        read_pass.uses[0] = GraphResourceUse {
            resource: scene_resource,
            resource_ref: IrResourceRef::render_target(scene_color),
            access: ResourceAccess::Present,
        };

        RendererIrCatalog {
            buffers: vec![BufferDesc::new(
                object_buffer,
                "object_table",
                4096,
                BufferUsageFlags::UNIFORM.union(BufferUsageFlags::STORAGE),
                BufferMemoryClass::DeviceLocal,
            )],
            textures: vec![
                TextureDesc::new_2d(
                    scene_color,
                    "scene_color",
                    1920,
                    1080,
                    TextureFormat::Rgba16Float,
                    TextureUsageFlags::RENDER_TARGET.union(TextureUsageFlags::SAMPLED),
                ),
                TextureDesc::new_2d(
                    depth,
                    "depth",
                    1920,
                    1080,
                    TextureFormat::Depth32Float,
                    TextureUsageFlags::DEPTH_TARGET,
                ),
            ],
            samplers: vec![SamplerDesc {
                id: sampler,
                stable_name: "linear_sampler",
                ..SamplerDesc::default()
            }],
            render_targets: vec![RenderTargetDesc {
                texture: scene_color,
                format: TextureFormat::Rgba16Float,
                sample_count: 1,
                ..RenderTargetDesc::default()
            }],
            depth_targets: vec![DepthTargetDesc {
                texture: depth,
                format: TextureFormat::Depth32Float,
                sample_count: 1,
                ..DepthTargetDesc::default()
            }],
            bind_layouts: vec![view_layout_desc, material_layout_desc],
            bind_tables: vec![view_table_desc, material_table_desc],
            shaders: vec![
                ShaderModuleDesc::wgsl(
                    vertex_shader,
                    "scene_vs",
                    "vs_main",
                    ShaderStageMask::VERTEX,
                    11,
                ),
                ShaderModuleDesc::wgsl(
                    fragment_shader,
                    "scene_fs",
                    "fs_main",
                    ShaderStageMask::FRAGMENT,
                    12,
                ),
            ],
            pipeline_layouts: vec![pipeline_layout_desc],
            render_pipelines: vec![RenderPipelineDesc {
                id: render_pipeline,
                stable_name: "opaque_pipeline",
                layout: pipeline_layout,
                vertex_shader,
                fragment_shader,
                color_formats: [
                    TextureFormat::Rgba16Float,
                    TextureFormat::Undefined,
                    TextureFormat::Undefined,
                    TextureFormat::Undefined,
                ],
                color_target_count: 1,
                depth_format: TextureFormat::Depth32Float,
                sample_count: 1,
                ..RenderPipelineDesc::default()
            }],
            graph_passes: vec![write_pass, read_pass],
            graph_edges: vec![GraphEdge {
                from: pass_write,
                to: pass_read,
                resource: scene_resource,
            }],
            draw_packets: vec![DrawPacket {
                object_index: 1,
                mesh_index: 2,
                material_index: 3,
                first_index: 0,
                index_count: 36,
                base_vertex: 0,
                first_instance: 0,
                instance_count: 1,
            }],
            ..RendererIrCatalog::default()
        }
    }

    #[test]
    fn renderer_ir_schema_versions_are_explicit() {
        assert_eq!(RENDERER_IR_SCHEMA_VERSIONS.shader_ir_schema, 1);
        assert_eq!(RENDERER_IR_SCHEMA_VERSIONS.pipeline_ir_schema, 1);
        assert_eq!(RENDERER_IR_SCHEMA_VERSIONS.material_binding_schema, 1);
        assert_eq!(RENDERER_IR_SCHEMA_VERSIONS.graph_ir_schema, 1);
    }

    #[test]
    fn hot_packet_types_are_small_copy_records() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<DrawPacket>();
        assert_copy::<DispatchPacket>();
        assert_copy::<IndirectDrawPacket>();
        assert!(size_of::<DrawPacket>() <= 32);
        assert!(size_of::<DispatchPacket>() <= 12);
        assert!(size_of::<IndirectDrawPacket>() <= 24);
        assert!(size_of::<RenderPassCommand>() <= 96);
    }

    #[test]
    fn valid_ir_catalog_compiles_graph_for_wgpu_bridge() {
        let catalog = basic_catalog();
        let report = validate_renderer_ir(&catalog, BackendCapabilityReport::WGPU_DX12);
        assert!(report.valid, "{:?}", report.errors);
        let compiled = catalog
            .compile_graph(BackendCapabilityReport::WGPU_DX12)
            .expect("valid graph should compile");
        assert_eq!(compiled.schema_version, GRAPH_IR_SCHEMA_VERSION);
        assert_eq!(compiled.passes.len(), 2);
        assert_eq!(compiled.edges.len(), 1);
        assert_ne!(compiled.graph_fingerprint, DescriptorFingerprint::default());
    }

    #[test]
    fn ir_validation_rejects_invalid_formats_and_sample_counts() {
        let mut catalog = basic_catalog();
        catalog.textures[0].format = TextureFormat::Undefined;
        catalog.textures[0].sample_count = 3;
        let report = validate_renderer_ir(&catalog, BackendCapabilityReport::WGPU_DX12);
        assert!(!report.valid);
        assert!(report.contains(IrValidationCode::InvalidFormat));
        assert!(report.contains(IrValidationCode::InvalidSampleCount));
    }

    #[test]
    fn ir_validation_rejects_layout_mismatch() {
        let mut catalog = basic_catalog();
        catalog.bind_tables[0].bindings[0].binding_type = BindingType::Sampler;
        let report = validate_renderer_ir(&catalog, BackendCapabilityReport::WGPU_DX12);
        assert!(!report.valid);
        assert!(report.contains(IrValidationCode::LayoutMismatch));
    }

    #[test]
    fn ir_validation_rejects_missing_resources_and_unwritten_reads() {
        let mut catalog = basic_catalog();
        catalog.graph_passes.swap(0, 1);
        catalog.buffers.clear();
        let report = validate_renderer_ir(&catalog, BackendCapabilityReport::WGPU_DX12);
        assert!(!report.valid);
        assert!(report.contains(IrValidationCode::MissingResource));
        assert!(report.contains(IrValidationCode::PassReadsUnwrittenResource));
    }

    #[test]
    fn ir_validation_rejects_unconsumed_writes_and_bad_edges() {
        let mut catalog = basic_catalog();
        catalog.graph_passes.pop();
        catalog.graph_edges[0].to = IrGraphPassId::new(99);
        let report = validate_renderer_ir(&catalog, BackendCapabilityReport::WGPU_DX12);
        assert!(!report.valid);
        assert!(report.contains(IrValidationCode::PassWritesUnconsumedResource));
        assert!(report.contains(IrValidationCode::GraphDependencyMismatch));
    }

    #[test]
    fn ir_validation_reports_unsupported_backend_features() {
        let mut catalog = basic_catalog();
        catalog.render_pipelines[0].requires_mesh_shader = true;
        let report = validate_renderer_ir(&catalog, BackendCapabilityReport::WGPU_DX12);
        assert!(!report.valid);
        assert!(report.contains(IrValidationCode::UnsupportedBackendFeature));
    }

    #[test]
    fn bridge_translation_cache_skips_unchanged_descriptors() {
        let mut cache = IrBridgeTranslationCache::default();
        let key = IrDescriptorKey {
            kind: IrDescriptorKind::ResourceCreation,
            index: 4,
        };
        let first = cache.translate_resource_creation(key, DescriptorFingerprint::from_u64(10));
        let second = cache.translate_resource_creation(key, DescriptorFingerprint::from_u64(10));
        let third = cache.translate_resource_creation(key, DescriptorFingerprint::from_u64(11));

        assert!(first.translated);
        assert!(!second.translated);
        assert_eq!(second.reason, BridgeTranslationReason::DescriptorUnchanged);
        assert!(third.translated);
        assert_eq!(third.reason, BridgeTranslationReason::DescriptorChanged);
        assert_eq!(
            cache.translated_by_kind[IrDescriptorKind::ResourceCreation.index()],
            2
        );
        assert_eq!(cache.skipped_unchanged, 1);
    }

    #[test]
    fn bridge_translation_interface_is_descriptor_granular() {
        let mut cache = IrBridgeTranslationCache::default();
        for (kind, index) in [
            (IrDescriptorKind::ResourceCreation, 0),
            (IrDescriptorKind::PipelineCreation, 1),
            (IrDescriptorKind::PassExecution, 2),
            (IrDescriptorKind::GraphCompile, 3),
        ] {
            let key = IrDescriptorKey { kind, index };
            let decision = match kind {
                IrDescriptorKind::ResourceCreation => {
                    cache.translate_resource_creation(key, DescriptorFingerprint::from_u64(100))
                }
                IrDescriptorKind::PipelineCreation => {
                    cache.translate_pipeline_creation(key, DescriptorFingerprint::from_u64(101))
                }
                IrDescriptorKind::PassExecution => {
                    cache.translate_pass_execution(key, DescriptorFingerprint::from_u64(102))
                }
                IrDescriptorKind::GraphCompile => {
                    cache.translate_graph_compile(key, DescriptorFingerprint::from_u64(103))
                }
            };
            assert!(decision.translated);
            assert_eq!(decision.key.kind, kind);
        }
        assert_eq!(cache.translated_by_kind, [1, 1, 1, 1]);
    }

    #[test]
    fn public_ir_names_do_not_leak_backend_handles() {
        for name in [
            "BufferDesc",
            "TextureDesc",
            "SamplerDesc",
            "RenderTargetDesc",
            "DepthTargetDesc",
            "TransientResourceDesc",
            "ImportedResourceDesc",
            "BindLayoutDesc",
            "BindTableDesc",
            "BindlessTableDesc",
            "RootConstantDesc",
            "MaterialBindingDesc",
            "ViewBindingDesc",
            "SceneBindingDesc",
            "ShaderModuleDesc",
            "PipelineLayoutDesc",
            "RenderPipelineDesc",
            "ComputePipelineDesc",
            "PipelineVariantKey",
            "PipelineFamily",
            "RenderPassCommand",
            "ComputePassCommand",
            "CopyCommand",
            "DrawPacket",
            "DispatchPacket",
            "IndirectDrawPacket",
            "GraphPassDesc",
            "GraphResourceUse",
            "GraphEdge",
            "CompiledGraph",
            "PassExecutionPlan",
        ] {
            for forbidden in PUBLIC_RENDERER_IR_FORBIDDEN_TERMS {
                assert!(
                    !name.contains(forbidden),
                    "{name} leaked forbidden backend term {forbidden}"
                );
            }
        }
    }
}
