use core::{hash::Hash, marker::PhantomData};

pub const BACKEND_CONTRACT_SCHEMA_VERSION: u16 = 1;
pub const BACKEND_FEATURE_COUNT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendBridgeType {
    Wgpu,
    DirectDx12,
    DirectVulkan,
    DirectMetal,
    Null,
}

impl BackendBridgeType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Wgpu => "wgpu",
            Self::DirectDx12 => "direct_dx12",
            Self::DirectVulkan => "direct_vulkan",
            Self::DirectMetal => "direct_metal",
            Self::Null => "null",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeBackend {
    Dx12,
    Vulkan,
    Metal,
    Unknown,
}

impl NativeBackend {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
            Self::Metal => "metal",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendImplementationStage {
    BridgeScaffold,
    DirectScaffold,
    Null,
}

impl BackendImplementationStage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BridgeScaffold => "bridge_scaffold",
            Self::DirectScaffold => "direct_scaffold",
            Self::Null => "null",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendDispatchMode {
    StaticProduction,
    DynamicTooling,
}

impl BackendDispatchMode {
    #[must_use]
    pub const fn production_perf_evidence_allowed(self) -> bool {
        matches!(self, Self::StaticProduction)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicBackendDispatchScope {
    CoarsePhaseBoundaryOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuBackendTruthStatus {
    MatchesRequestedBackend,
    Mismatch,
    NotWgpu,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuBackendTruth {
    pub requested_backend: NativeBackend,
    pub adapter_backend: NativeBackend,
    pub status: WgpuBackendTruthStatus,
}

impl WgpuBackendTruth {
    pub const NOT_WGPU: Self = Self {
        requested_backend: NativeBackend::Unknown,
        adapter_backend: NativeBackend::Unknown,
        status: WgpuBackendTruthStatus::NotWgpu,
    };

    pub const UNKNOWN: Self = Self {
        requested_backend: NativeBackend::Unknown,
        adapter_backend: NativeBackend::Unknown,
        status: WgpuBackendTruthStatus::Unknown,
    };

    #[must_use]
    pub const fn matched(backend: NativeBackend) -> Self {
        Self {
            requested_backend: backend,
            adapter_backend: backend,
            status: WgpuBackendTruthStatus::MatchesRequestedBackend,
        }
    }

    #[must_use]
    pub const fn mismatch(
        requested_backend: NativeBackend,
        adapter_backend: NativeBackend,
    ) -> Self {
        Self {
            requested_backend,
            adapter_backend,
            status: WgpuBackendTruthStatus::Mismatch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuCoreValidationStatus {
    Enabled,
    Disabled,
    NotWgpu,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuHalNativeHandleSupport {
    Available,
    Partial,
    Unavailable,
    NotWgpu,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NagaShaderTranslationStatus {
    Available,
    Unavailable,
    NotRequired,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeCommandEncoderAvailability {
    Available,
    BridgeDoesNotExpose,
    BackendScaffoldOnly,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendFeature {
    DrawPackets,
    ResourceResidency,
    TimestampQueries,
    TimelineFences,
    SharedTextureImport,
    NativeCommandEncoderAccess,
    MeshShaders,
    RayTracing,
    NagaShaderTranslation,
    WgpuCoreValidation,
}

impl BackendFeature {
    pub const ALL: [Self; BACKEND_FEATURE_COUNT] = [
        Self::DrawPackets,
        Self::ResourceResidency,
        Self::TimestampQueries,
        Self::TimelineFences,
        Self::SharedTextureImport,
        Self::NativeCommandEncoderAccess,
        Self::MeshShaders,
        Self::RayTracing,
        Self::NagaShaderTranslation,
        Self::WgpuCoreValidation,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DrawPackets => "draw_packets",
            Self::ResourceResidency => "resource_residency",
            Self::TimestampQueries => "timestamp_queries",
            Self::TimelineFences => "timeline_fences",
            Self::SharedTextureImport => "shared_texture_import",
            Self::NativeCommandEncoderAccess => "native_command_encoder_access",
            Self::MeshShaders => "mesh_shaders",
            Self::RayTracing => "ray_tracing",
            Self::NagaShaderTranslation => "naga_shader_translation",
            Self::WgpuCoreValidation => "wgpu_core_validation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MissingFeatureReason {
    NullBackend,
    WgpuBridgeDoesNotExposeNativeEncoder,
    NativeInteropNotProven,
    DirectBackendScaffoldOnly,
    FeatureNotReportedByBridge,
    NotApplicableForBridge,
}

impl MissingFeatureReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NullBackend => "null_backend",
            Self::WgpuBridgeDoesNotExposeNativeEncoder => {
                "wgpu_bridge_does_not_expose_native_encoder"
            }
            Self::NativeInteropNotProven => "native_interop_not_proven",
            Self::DirectBackendScaffoldOnly => "direct_backend_scaffold_only",
            Self::FeatureNotReportedByBridge => "feature_not_reported_by_bridge",
            Self::NotApplicableForBridge => "not_applicable_for_bridge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendFeatureSupport {
    pub feature: BackendFeature,
    pub supported: bool,
    pub missing_reason: Option<MissingFeatureReason>,
}

impl BackendFeatureSupport {
    #[must_use]
    pub const fn supported(feature: BackendFeature) -> Self {
        Self {
            feature,
            supported: true,
            missing_reason: None,
        }
    }

    #[must_use]
    pub const fn missing(feature: BackendFeature, reason: MissingFeatureReason) -> Self {
        Self {
            feature,
            supported: false,
            missing_reason: Some(reason),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendFeatureSet {
    pub features: [BackendFeatureSupport; BACKEND_FEATURE_COUNT],
}

impl BackendFeatureSet {
    pub const NULL: Self = Self {
        features: [
            BackendFeatureSupport::missing(
                BackendFeature::DrawPackets,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::ResourceResidency,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::TimestampQueries,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::TimelineFences,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::SharedTextureImport,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::NativeCommandEncoderAccess,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::MeshShaders,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::RayTracing,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::NagaShaderTranslation,
                MissingFeatureReason::NullBackend,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::WgpuCoreValidation,
                MissingFeatureReason::NullBackend,
            ),
        ],
    };

    pub const WGPU_BRIDGE: Self = Self {
        features: [
            BackendFeatureSupport::supported(BackendFeature::DrawPackets),
            BackendFeatureSupport::supported(BackendFeature::ResourceResidency),
            BackendFeatureSupport::supported(BackendFeature::TimestampQueries),
            BackendFeatureSupport::missing(
                BackendFeature::TimelineFences,
                MissingFeatureReason::NativeInteropNotProven,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::SharedTextureImport,
                MissingFeatureReason::NativeInteropNotProven,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::NativeCommandEncoderAccess,
                MissingFeatureReason::WgpuBridgeDoesNotExposeNativeEncoder,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::MeshShaders,
                MissingFeatureReason::FeatureNotReportedByBridge,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::RayTracing,
                MissingFeatureReason::FeatureNotReportedByBridge,
            ),
            BackendFeatureSupport::supported(BackendFeature::NagaShaderTranslation),
            BackendFeatureSupport::supported(BackendFeature::WgpuCoreValidation),
        ],
    };

    pub const DIRECT_SCAFFOLD: Self = Self {
        features: [
            BackendFeatureSupport::missing(
                BackendFeature::DrawPackets,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::ResourceResidency,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::TimestampQueries,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::TimelineFences,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::SharedTextureImport,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::NativeCommandEncoderAccess,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::MeshShaders,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::RayTracing,
                MissingFeatureReason::DirectBackendScaffoldOnly,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::NagaShaderTranslation,
                MissingFeatureReason::NotApplicableForBridge,
            ),
            BackendFeatureSupport::missing(
                BackendFeature::WgpuCoreValidation,
                MissingFeatureReason::NotApplicableForBridge,
            ),
        ],
    };

    #[must_use]
    pub fn support_for(self, feature: BackendFeature) -> Option<BackendFeatureSupport> {
        self.features
            .iter()
            .copied()
            .find(|support| support.feature == feature)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendCapabilityReport {
    pub schema_version: u16,
    pub bridge_type: BackendBridgeType,
    pub implementation_stage: BackendImplementationStage,
    pub dispatch_mode: BackendDispatchMode,
    pub actual_native_backend: NativeBackend,
    pub wgpu_backend_truth: WgpuBackendTruth,
    pub wgpu_core_validation: WgpuCoreValidationStatus,
    pub wgpu_hal_native_handle_support: WgpuHalNativeHandleSupport,
    pub naga_shader_translation: NagaShaderTranslationStatus,
    pub command_encoder_availability: NativeCommandEncoderAvailability,
    pub features: BackendFeatureSet,
}

impl BackendCapabilityReport {
    pub const NULL: Self = Self {
        schema_version: BACKEND_CONTRACT_SCHEMA_VERSION,
        bridge_type: BackendBridgeType::Null,
        implementation_stage: BackendImplementationStage::Null,
        dispatch_mode: BackendDispatchMode::StaticProduction,
        actual_native_backend: NativeBackend::Unknown,
        wgpu_backend_truth: WgpuBackendTruth::UNKNOWN,
        wgpu_core_validation: WgpuCoreValidationStatus::Unknown,
        wgpu_hal_native_handle_support: WgpuHalNativeHandleSupport::Unknown,
        naga_shader_translation: NagaShaderTranslationStatus::Unknown,
        command_encoder_availability: NativeCommandEncoderAvailability::Unavailable,
        features: BackendFeatureSet::NULL,
    };

    pub const WGPU_DX12: Self = Self::wgpu_bridge(NativeBackend::Dx12);
    pub const WGPU_VULKAN: Self = Self::wgpu_bridge(NativeBackend::Vulkan);
    pub const WGPU_METAL: Self = Self::wgpu_bridge(NativeBackend::Metal);

    pub const DIRECT_DX12: Self =
        Self::direct_scaffold(BackendBridgeType::DirectDx12, NativeBackend::Dx12);
    pub const DIRECT_VULKAN: Self =
        Self::direct_scaffold(BackendBridgeType::DirectVulkan, NativeBackend::Vulkan);
    pub const DIRECT_METAL: Self =
        Self::direct_scaffold(BackendBridgeType::DirectMetal, NativeBackend::Metal);

    #[must_use]
    pub const fn wgpu_bridge(native_backend: NativeBackend) -> Self {
        Self {
            schema_version: BACKEND_CONTRACT_SCHEMA_VERSION,
            bridge_type: BackendBridgeType::Wgpu,
            implementation_stage: BackendImplementationStage::BridgeScaffold,
            dispatch_mode: BackendDispatchMode::StaticProduction,
            actual_native_backend: native_backend,
            wgpu_backend_truth: WgpuBackendTruth::matched(native_backend),
            wgpu_core_validation: WgpuCoreValidationStatus::Enabled,
            wgpu_hal_native_handle_support: WgpuHalNativeHandleSupport::Partial,
            naga_shader_translation: NagaShaderTranslationStatus::Available,
            command_encoder_availability: NativeCommandEncoderAvailability::BridgeDoesNotExpose,
            features: BackendFeatureSet::WGPU_BRIDGE,
        }
    }

    #[must_use]
    pub const fn direct_scaffold(
        bridge_type: BackendBridgeType,
        native_backend: NativeBackend,
    ) -> Self {
        Self {
            schema_version: BACKEND_CONTRACT_SCHEMA_VERSION,
            bridge_type,
            implementation_stage: BackendImplementationStage::DirectScaffold,
            dispatch_mode: BackendDispatchMode::StaticProduction,
            actual_native_backend: native_backend,
            wgpu_backend_truth: WgpuBackendTruth::NOT_WGPU,
            wgpu_core_validation: WgpuCoreValidationStatus::NotWgpu,
            wgpu_hal_native_handle_support: WgpuHalNativeHandleSupport::NotWgpu,
            naga_shader_translation: NagaShaderTranslationStatus::NotRequired,
            command_encoder_availability: NativeCommandEncoderAvailability::BackendScaffoldOnly,
            features: BackendFeatureSet::DIRECT_SCAFFOLD,
        }
    }

    #[must_use]
    pub const fn with_dispatch_mode(mut self, dispatch_mode: BackendDispatchMode) -> Self {
        self.dispatch_mode = dispatch_mode;
        self
    }

    #[must_use]
    pub fn support_for(self, feature: BackendFeature) -> Option<BackendFeatureSupport> {
        self.features.support_for(feature)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendNativeInteropStatus {
    pub native_device: WgpuHalNativeHandleSupport,
    pub native_queue: WgpuHalNativeHandleSupport,
    pub native_command_encoder: NativeCommandEncoderAvailability,
}

impl BackendNativeInteropStatus {
    #[must_use]
    pub const fn from_report(report: BackendCapabilityReport) -> Self {
        Self {
            native_device: report.wgpu_hal_native_handle_support,
            native_queue: report.wgpu_hal_native_handle_support,
            native_command_encoder: report.command_encoder_availability,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererGenerationalIndex {
    pub slot: u32,
    pub generation: u32,
}

impl RendererGenerationalIndex {
    pub const INVALID: Self = Self {
        slot: u32::MAX,
        generation: 0,
    };

    #[must_use]
    pub const fn new(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }

    #[must_use]
    pub const fn first(slot: u32) -> Self {
        Self {
            slot,
            generation: 1,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.slot != u32::MAX && self.generation != 0
    }

    #[must_use]
    pub const fn matches_generation(self, generation: u32) -> bool {
        self.is_valid() && self.generation == generation
    }

    #[must_use]
    pub const fn next_generation(self) -> Self {
        let next = if self.generation == u32::MAX {
            1
        } else {
            self.generation + 1
        };
        Self {
            slot: self.slot,
            generation: next,
        }
    }
}

pub trait RendererHandleKind {
    const KIND: &'static str;
}

pub struct TypedRendererHandle<TKind: RendererHandleKind> {
    raw: RendererGenerationalIndex,
    _kind: PhantomData<fn() -> TKind>,
}

impl<TKind: RendererHandleKind> TypedRendererHandle<TKind> {
    pub const INVALID: Self = Self {
        raw: RendererGenerationalIndex::INVALID,
        _kind: PhantomData,
    };

    #[must_use]
    pub const fn new(slot: u32, generation: u32) -> Self {
        Self {
            raw: RendererGenerationalIndex::new(slot, generation),
            _kind: PhantomData,
        }
    }

    #[must_use]
    pub const fn first(slot: u32) -> Self {
        Self {
            raw: RendererGenerationalIndex::first(slot),
            _kind: PhantomData,
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

    #[must_use]
    pub const fn kind(self) -> &'static str {
        TKind::KIND
    }
}

impl<TKind: RendererHandleKind> Copy for TypedRendererHandle<TKind> {}

impl<TKind: RendererHandleKind> Clone for TypedRendererHandle<TKind> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<TKind: RendererHandleKind> PartialEq for TypedRendererHandle<TKind> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<TKind: RendererHandleKind> Eq for TypedRendererHandle<TKind> {}

impl<TKind: RendererHandleKind> Hash for TypedRendererHandle<TKind> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<TKind: RendererHandleKind> core::fmt::Debug for TypedRendererHandle<TKind> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TypedRendererHandle")
            .field("kind", &TKind::KIND)
            .field("slot", &self.raw.slot)
            .field("generation", &self.raw.generation)
            .finish()
    }
}

pub struct BufferHandleKind<TUsage>(PhantomData<fn() -> TUsage>);
pub struct TextureHandleKind<TUsage>(PhantomData<fn() -> TUsage>);
pub struct PipelineHandleKind<TPhase>(PhantomData<fn() -> TPhase>);
pub struct BindTableHandleKind<TLayout>(PhantomData<fn() -> TLayout>);
pub struct GraphResourceHandleKind<TKind>(PhantomData<fn() -> TKind>);
pub struct MaterialHandleKind;
pub struct MeshHandleKind;

impl<TUsage> RendererHandleKind for BufferHandleKind<TUsage> {
    const KIND: &'static str = "buffer";
}

impl<TUsage> RendererHandleKind for TextureHandleKind<TUsage> {
    const KIND: &'static str = "texture";
}

impl<TPhase> RendererHandleKind for PipelineHandleKind<TPhase> {
    const KIND: &'static str = "pipeline";
}

impl<TLayout> RendererHandleKind for BindTableHandleKind<TLayout> {
    const KIND: &'static str = "bind_table";
}

impl<TKind> RendererHandleKind for GraphResourceHandleKind<TKind> {
    const KIND: &'static str = "graph_resource";
}

impl RendererHandleKind for MaterialHandleKind {
    const KIND: &'static str = "material";
}

impl RendererHandleKind for MeshHandleKind {
    const KIND: &'static str = "mesh";
}

pub type BufferId<TUsage> = TypedRendererHandle<BufferHandleKind<TUsage>>;
pub type TextureId<TUsage> = TypedRendererHandle<TextureHandleKind<TUsage>>;
pub type PipelineId<TPhase> = TypedRendererHandle<PipelineHandleKind<TPhase>>;
pub type BindTableId<TLayout> = TypedRendererHandle<BindTableHandleKind<TLayout>>;
pub type GraphResourceId<TKind> = TypedRendererHandle<GraphResourceHandleKind<TKind>>;
pub type MaterialId = TypedRendererHandle<MaterialHandleKind>;
pub type MeshId = TypedRendererHandle<MeshHandleKind>;

pub struct VertexBufferUsage;
pub struct IndexBufferUsage;
pub struct UniformBufferUsage;
pub struct SceneColorTextureUsage;
pub struct DepthTextureUsage;
pub struct OpaquePhase;
pub struct TransparentPhase;
pub struct UiPhase;
pub struct ViewBindLayout;
pub struct MaterialBindLayout;
pub struct GraphImportedKind;
pub struct GraphTransientKind;

pub trait RendererBackend: Sized + 'static {
    const NAME: &'static str;
    const CAPABILITY_REPORT: BackendCapabilityReport;

    type Device;
    type Queue;
    type Surface;
    type Swapchain;
    type Buffer;
    type Texture;
    type Sampler;
    type Pipeline;
    type BindTable;
    type ShaderModule;
    type CommandEncoder<'frame>
    where
        Self: 'frame;
    type Fence;
    type TimestampQuery;
}

pub trait BackendDevice: RendererBackend {
    #[must_use]
    fn device_capabilities(_: &Self::Device) -> BackendCapabilityReport {
        Self::CAPABILITY_REPORT
    }
}

pub trait BackendSurface: RendererBackend {}

pub trait BackendQueue: RendererBackend {}

pub trait BackendResource: RendererBackend {}

pub trait BackendCommandEncoder: RendererBackend {
    #[must_use]
    fn command_encoder_availability() -> NativeCommandEncoderAvailability {
        Self::CAPABILITY_REPORT.command_encoder_availability
    }
}

pub trait BackendPipeline: RendererBackend {}

pub trait BackendShader: RendererBackend {
    #[must_use]
    fn shader_translation_status() -> NagaShaderTranslationStatus {
        Self::CAPABILITY_REPORT.naga_shader_translation
    }
}

pub trait BackendTiming: RendererBackend {
    #[must_use]
    fn timestamp_queries_available() -> bool {
        Self::CAPABILITY_REPORT
            .support_for(BackendFeature::TimestampQueries)
            .is_some_and(|support| support.supported)
    }
}

pub trait BackendDiagnostics: RendererBackend {
    #[must_use]
    fn capability_report() -> BackendCapabilityReport {
        Self::CAPABILITY_REPORT
    }
}

pub trait BackendNativeInterop: RendererBackend {
    type NativeDevice;
    type NativeQueue;
    type NativeSurface;
    type NativeCommandEncoder<'frame>
    where
        Self: 'frame;

    #[must_use]
    fn native_interop_status() -> BackendNativeInteropStatus {
        BackendNativeInteropStatus::from_report(Self::CAPABILITY_REPORT)
    }
}

pub struct WgpuBridge<HalBridge> {
    _hal_bridge: PhantomData<fn() -> HalBridge>,
}

pub struct Dx12HalBridge;
pub struct VulkanHalBridge;
pub struct MetalHalBridge;

pub type WgpuDx12Backend = WgpuBridge<Dx12HalBridge>;
pub type WgpuVulkanBackend = WgpuBridge<VulkanHalBridge>;
pub type WgpuMetalBackend = WgpuBridge<MetalHalBridge>;

pub struct DirectDx12Backend;
pub struct DirectVulkanBackend;
pub struct DirectMetalBackend;
pub struct NullBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpaqueBackendHandle<B> {
    raw: u64,
    _backend: PhantomData<fn() -> B>,
}

impl<B> OpaqueBackendHandle<B> {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self {
            raw,
            _backend: PhantomData,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.raw
    }
}

pub type BackendDeviceHandle<B> = OpaqueBackendHandle<(B, DeviceHandleKind)>;
pub type BackendQueueHandle<B> = OpaqueBackendHandle<(B, QueueHandleKind)>;
pub type BackendSurfaceHandle<B> = OpaqueBackendHandle<(B, SurfaceHandleKind)>;
pub type BackendSwapchainHandle<B> = OpaqueBackendHandle<(B, SwapchainHandleKind)>;
pub type BackendBufferHandle<B> = OpaqueBackendHandle<(B, BufferBackendHandleKind)>;
pub type BackendTextureHandle<B> = OpaqueBackendHandle<(B, TextureBackendHandleKind)>;
pub type BackendSamplerHandle<B> = OpaqueBackendHandle<(B, SamplerHandleKind)>;
pub type BackendPipelineHandle<B> = OpaqueBackendHandle<(B, PipelineBackendHandleKind)>;
pub type BackendBindTableHandle<B> = OpaqueBackendHandle<(B, BackendBindTableHandleKind)>;
pub type BackendShaderModuleHandle<B> = OpaqueBackendHandle<(B, ShaderModuleHandleKind)>;
pub type BackendFenceHandle<B> = OpaqueBackendHandle<(B, FenceHandleKind)>;
pub type BackendTimestampQueryHandle<B> = OpaqueBackendHandle<(B, TimestampQueryHandleKind)>;

pub struct DeviceHandleKind;
pub struct QueueHandleKind;
pub struct SurfaceHandleKind;
pub struct SwapchainHandleKind;
pub struct BufferBackendHandleKind;
pub struct TextureBackendHandleKind;
pub struct SamplerHandleKind;
pub struct PipelineBackendHandleKind;
pub struct BackendBindTableHandleKind;
pub struct ShaderModuleHandleKind;
pub struct FenceHandleKind;
pub struct TimestampQueryHandleKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendCommandEncoderHandle<'frame, B> {
    raw: u64,
    _backend: PhantomData<fn() -> B>,
    _frame: PhantomData<&'frame mut ()>,
}

impl<'frame, B> BackendCommandEncoderHandle<'frame, B> {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self {
            raw,
            _backend: PhantomData,
            _frame: PhantomData,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.raw
    }
}

pub struct NativeInteropUnavailable;
pub struct Dx12NativeDeviceHandle;
pub struct Dx12NativeQueueHandle;
pub struct Dx12NativeSurfaceHandle;
pub struct Dx12NativeCommandEncoderHandle;
pub struct VulkanNativeDeviceHandle;
pub struct VulkanNativeQueueHandle;
pub struct VulkanNativeSurfaceHandle;
pub struct VulkanNativeCommandEncoderHandle;
pub struct MetalNativeDeviceHandle;
pub struct MetalNativeQueueHandle;
pub struct MetalNativeSurfaceHandle;
pub struct MetalNativeCommandEncoderHandle;

macro_rules! impl_backend {
    (
        $backend:ty,
        $name:literal,
        $report:expr,
        $native_device:ty,
        $native_queue:ty,
        $native_surface:ty,
        $native_encoder:ty
    ) => {
        impl RendererBackend for $backend {
            const NAME: &'static str = $name;
            const CAPABILITY_REPORT: BackendCapabilityReport = $report;

            type Device = BackendDeviceHandle<Self>;
            type Queue = BackendQueueHandle<Self>;
            type Surface = BackendSurfaceHandle<Self>;
            type Swapchain = BackendSwapchainHandle<Self>;
            type Buffer = BackendBufferHandle<Self>;
            type Texture = BackendTextureHandle<Self>;
            type Sampler = BackendSamplerHandle<Self>;
            type Pipeline = BackendPipelineHandle<Self>;
            type BindTable = BackendBindTableHandle<Self>;
            type ShaderModule = BackendShaderModuleHandle<Self>;
            type CommandEncoder<'frame>
                = BackendCommandEncoderHandle<'frame, Self>
            where
                Self: 'frame;
            type Fence = BackendFenceHandle<Self>;
            type TimestampQuery = BackendTimestampQueryHandle<Self>;
        }

        impl BackendDevice for $backend {}
        impl BackendSurface for $backend {}
        impl BackendQueue for $backend {}
        impl BackendResource for $backend {}
        impl BackendCommandEncoder for $backend {}
        impl BackendPipeline for $backend {}
        impl BackendShader for $backend {}
        impl BackendTiming for $backend {}
        impl BackendDiagnostics for $backend {}

        impl BackendNativeInterop for $backend {
            type NativeDevice = $native_device;
            type NativeQueue = $native_queue;
            type NativeSurface = $native_surface;
            type NativeCommandEncoder<'frame>
                = $native_encoder
            where
                Self: 'frame;
        }
    };
}

impl_backend!(
    WgpuBridge<Dx12HalBridge>,
    "wgpu_dx12",
    BackendCapabilityReport::WGPU_DX12,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable
);
impl_backend!(
    WgpuBridge<VulkanHalBridge>,
    "wgpu_vulkan",
    BackendCapabilityReport::WGPU_VULKAN,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable
);
impl_backend!(
    WgpuBridge<MetalHalBridge>,
    "wgpu_metal",
    BackendCapabilityReport::WGPU_METAL,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable
);
impl_backend!(
    DirectDx12Backend,
    "direct_dx12",
    BackendCapabilityReport::DIRECT_DX12,
    Dx12NativeDeviceHandle,
    Dx12NativeQueueHandle,
    Dx12NativeSurfaceHandle,
    Dx12NativeCommandEncoderHandle
);
impl_backend!(
    DirectVulkanBackend,
    "direct_vulkan",
    BackendCapabilityReport::DIRECT_VULKAN,
    VulkanNativeDeviceHandle,
    VulkanNativeQueueHandle,
    VulkanNativeSurfaceHandle,
    VulkanNativeCommandEncoderHandle
);
impl_backend!(
    DirectMetalBackend,
    "direct_metal",
    BackendCapabilityReport::DIRECT_METAL,
    MetalNativeDeviceHandle,
    MetalNativeQueueHandle,
    MetalNativeSurfaceHandle,
    MetalNativeCommandEncoderHandle
);
impl_backend!(
    NullBackend,
    "null",
    BackendCapabilityReport::NULL,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable,
    NativeInteropUnavailable
);

pub trait StaticBackendSelection {
    type Backend: RendererBackend;

    #[must_use]
    fn capability_report() -> BackendCapabilityReport {
        <Self::Backend as RendererBackend>::CAPABILITY_REPORT
    }
}

pub struct WindowsDx12ProductionBackend;
pub struct VulkanProductionBackend;
pub struct MetalProductionBackend;
pub struct NullToolingBackend;

impl StaticBackendSelection for WindowsDx12ProductionBackend {
    type Backend = WgpuDx12Backend;
}

impl StaticBackendSelection for VulkanProductionBackend {
    type Backend = WgpuVulkanBackend;
}

impl StaticBackendSelection for MetalProductionBackend {
    type Backend = WgpuMetalBackend;
}

impl StaticBackendSelection for NullToolingBackend {
    type Backend = NullBackend;
}

#[cfg(target_os = "windows")]
pub type DefaultProductionBackendSelection = WindowsDx12ProductionBackend;
#[cfg(target_os = "macos")]
pub type DefaultProductionBackendSelection = MetalProductionBackend;
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub type DefaultProductionBackendSelection = VulkanProductionBackend;

pub type DefaultProductionRenderer =
    Renderer<<DefaultProductionBackendSelection as StaticBackendSelection>::Backend>;

pub struct Renderer<B: RendererBackend> {
    _backend: PhantomData<fn() -> B>,
}

impl<B: RendererBackend> Renderer<B> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            _backend: PhantomData,
        }
    }

    #[must_use]
    pub const fn backend_name(self) -> &'static str {
        B::NAME
    }

    #[must_use]
    pub const fn capability_report(self) -> BackendCapabilityReport {
        B::CAPABILITY_REPORT
    }
}

impl<B: RendererBackend> Default for Renderer<B> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicRendererBackend {
    WgpuDx12,
    WgpuVulkan,
    WgpuMetal,
    DirectDx12,
    DirectVulkan,
    DirectMetal,
    Null,
}

impl DynamicRendererBackend {
    #[must_use]
    pub const fn dispatch_scope(self) -> DynamicBackendDispatchScope {
        DynamicBackendDispatchScope::CoarsePhaseBoundaryOnly
    }

    #[must_use]
    pub const fn production_perf_evidence_allowed(self) -> bool {
        false
    }

    #[must_use]
    pub const fn capability_report(self) -> BackendCapabilityReport {
        match self {
            Self::WgpuDx12 => BackendCapabilityReport::WGPU_DX12,
            Self::WgpuVulkan => BackendCapabilityReport::WGPU_VULKAN,
            Self::WgpuMetal => BackendCapabilityReport::WGPU_METAL,
            Self::DirectDx12 => BackendCapabilityReport::DIRECT_DX12,
            Self::DirectVulkan => BackendCapabilityReport::DIRECT_VULKAN,
            Self::DirectMetal => BackendCapabilityReport::DIRECT_METAL,
            Self::Null => BackendCapabilityReport::NULL,
        }
        .with_dispatch_mode(BackendDispatchMode::DynamicTooling)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_high_level_renderer<B: RendererBackend + BackendDiagnostics>()
    -> BackendCapabilityReport {
        Renderer::<B>::new().capability_report()
    }

    #[test]
    fn static_renderer_is_monomorphized_by_backend_type() {
        let dx12 = compile_high_level_renderer::<WgpuDx12Backend>();
        let vulkan = compile_high_level_renderer::<WgpuVulkanBackend>();
        let metal = compile_high_level_renderer::<WgpuMetalBackend>();

        assert_eq!(dx12.bridge_type, BackendBridgeType::Wgpu);
        assert_eq!(dx12.actual_native_backend, NativeBackend::Dx12);
        assert_eq!(vulkan.actual_native_backend, NativeBackend::Vulkan);
        assert_eq!(metal.actual_native_backend, NativeBackend::Metal);
        assert_eq!(
            core::mem::size_of::<Renderer<WgpuDx12Backend>>(),
            0,
            "production renderer selector must not store a trait object"
        );
    }

    #[test]
    fn wgpu_capability_report_separates_bridge_backend_and_missing_native_encoder() {
        let report = Renderer::<WgpuDx12Backend>::new().capability_report();
        let native_encoder = report
            .support_for(BackendFeature::NativeCommandEncoderAccess)
            .expect("feature table must cover native command encoder access");

        assert_eq!(report.bridge_type, BackendBridgeType::Wgpu);
        assert_eq!(report.actual_native_backend, NativeBackend::Dx12);
        assert_eq!(
            report.wgpu_backend_truth.status,
            WgpuBackendTruthStatus::MatchesRequestedBackend
        );
        assert_eq!(
            report.wgpu_core_validation,
            WgpuCoreValidationStatus::Enabled
        );
        assert_eq!(
            report.wgpu_hal_native_handle_support,
            WgpuHalNativeHandleSupport::Partial
        );
        assert_eq!(
            report.naga_shader_translation,
            NagaShaderTranslationStatus::Available
        );
        assert_eq!(
            report.command_encoder_availability,
            NativeCommandEncoderAvailability::BridgeDoesNotExpose
        );
        assert!(!native_encoder.supported);
        assert_eq!(
            native_encoder.missing_reason,
            Some(MissingFeatureReason::WgpuBridgeDoesNotExposeNativeEncoder)
        );
    }

    #[test]
    fn typed_handles_are_generation_checked_and_kind_stable() {
        let vertex = BufferId::<VertexBufferUsage>::first(7);
        let stale = BufferId::<VertexBufferUsage>::new(7, 2);
        let material = MaterialId::first(7);
        let color = TextureId::<SceneColorTextureUsage>::first(3);

        assert!(vertex.is_valid());
        assert_eq!(vertex.kind(), "buffer");
        assert_eq!(material.kind(), "material");
        assert_eq!(color.kind(), "texture");
        assert!(vertex.raw().matches_generation(1));
        assert!(!vertex.raw().matches_generation(stale.generation()));
        assert_eq!(vertex.raw().next_generation(), stale.raw());
    }

    #[test]
    fn static_backend_selection_chooses_dx12_for_windows_production() {
        type Selected = <WindowsDx12ProductionBackend as StaticBackendSelection>::Backend;
        let renderer = Renderer::<Selected>::new();

        assert_eq!(renderer.backend_name(), "wgpu_dx12");
        assert_eq!(
            WindowsDx12ProductionBackend::capability_report().dispatch_mode,
            BackendDispatchMode::StaticProduction
        );
    }

    #[test]
    fn dynamic_backend_wrapper_is_tooling_only() {
        let dynamic = DynamicRendererBackend::WgpuDx12;
        let report = dynamic.capability_report();

        assert_eq!(
            dynamic.dispatch_scope(),
            DynamicBackendDispatchScope::CoarsePhaseBoundaryOnly
        );
        assert!(!dynamic.production_perf_evidence_allowed());
        assert_eq!(report.dispatch_mode, BackendDispatchMode::DynamicTooling);
        assert!(!report.dispatch_mode.production_perf_evidence_allowed());
    }

    #[test]
    fn direct_backends_are_explicit_scaffolds_not_wgpu_bridges() {
        for (backend, native) in [
            (DynamicRendererBackend::DirectDx12, NativeBackend::Dx12),
            (DynamicRendererBackend::DirectVulkan, NativeBackend::Vulkan),
            (DynamicRendererBackend::DirectMetal, NativeBackend::Metal),
        ] {
            let report = backend.capability_report();
            let draw_packets = report
                .support_for(BackendFeature::DrawPackets)
                .expect("feature table must cover draw packets");

            assert_eq!(report.actual_native_backend, native);
            assert_eq!(
                report.wgpu_backend_truth.status,
                WgpuBackendTruthStatus::NotWgpu
            );
            assert_eq!(
                report.implementation_stage,
                BackendImplementationStage::DirectScaffold
            );
            assert!(!draw_packets.supported);
            assert_eq!(
                draw_packets.missing_reason,
                Some(MissingFeatureReason::DirectBackendScaffoldOnly)
            );
        }
    }
}
