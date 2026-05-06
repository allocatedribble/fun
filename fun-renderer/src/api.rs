use crate::{FunRendererBackend, FunRendererRuntimeBackend};

pub const RENDERER_CORE_API_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererFeatureToggles {
    pub legacy: bool,
    pub new_core: bool,
    pub dx12: bool,
    pub vulkan: bool,
    pub cef_gpu_only: bool,
    pub upscale: bool,
    pub dlss: bool,
    pub fsr: bool,
    pub frame_generation: bool,
    pub experimental_ml: bool,
}

impl RendererFeatureToggles {
    pub const COMPILED: Self = Self {
        legacy: cfg!(feature = "legacy_renderer"),
        new_core: cfg!(feature = "fun_renderer_core"),
        dx12: cfg!(feature = "dx12_native_interop"),
        vulkan: cfg!(feature = "vulkan_backend"),
        cef_gpu_only: cfg!(feature = "cef_gpu_only"),
        upscale: cfg!(feature = "upscaling"),
        dlss: cfg!(feature = "dlss"),
        fsr: cfg!(feature = "fsr"),
        frame_generation: cfg!(feature = "frame_generation"),
        experimental_ml: cfg!(feature = "experimental_renderer_ml"),
    };

    #[must_use]
    pub const fn compiled() -> Self {
        Self::COMPILED
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RendererCoreSettings {
    pub runtime_backend: FunRendererRuntimeBackend,
    pub backend: FunRendererBackend,
    pub clear_color: [f32; 4],
    pub features: RendererFeatureToggles,
}

impl RendererCoreSettings {
    pub const DEFAULT_CLEAR_COLOR: [f32; 4] = [0.015, 0.017, 0.021, 1.0];

    #[must_use]
    pub const fn new(
        runtime_backend: FunRendererRuntimeBackend,
        backend: FunRendererBackend,
        features: RendererFeatureToggles,
    ) -> Self {
        Self {
            runtime_backend,
            backend,
            clear_color: Self::DEFAULT_CLEAR_COLOR,
            features,
        }
    }

    #[must_use]
    pub const fn compiled_default() -> Self {
        Self::new(
            FunRendererRuntimeBackend::Fun,
            FunRendererBackend::Dx12,
            RendererFeatureToggles::COMPILED,
        )
    }
}

impl Default for RendererCoreSettings {
    fn default() -> Self {
        Self::compiled_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub backend: FunRendererBackend,
    pub native_handles: bool,
    pub shared_textures: bool,
    pub timeline_semaphores: bool,
    pub mesh_shaders: bool,
    pub ray_tracing: bool,
}

impl BackendCapabilities {
    #[must_use]
    pub const fn minimal(backend: FunRendererBackend) -> Self {
        Self {
            backend,
            native_handles: false,
            shared_textures: false,
            timeline_semaphores: false,
            mesh_shaders: false,
            ray_tracing: false,
        }
    }
}

pub trait DeviceBackend {
    fn backend(&self) -> FunRendererBackend;
    fn capabilities(&self) -> BackendCapabilities;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassHandle(pub u16);

impl PassHandle {
    pub const INVALID: Self = Self(u16::MAX);

    #[must_use]
    pub const fn new(index: u16) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassKind {
    ClearColor,
    GpuSceneUpdate,
    VirtualGeometry,
    VirtualShadow,
    Lighting,
    Upscale,
    UiComposite,
    Present,
}

impl PassKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClearColor => "clear_color",
            Self::GpuSceneUpdate => "gpu_scene_update",
            Self::VirtualGeometry => "virtual_geometry",
            Self::VirtualShadow => "virtual_shadow",
            Self::Lighting => "lighting",
            Self::Upscale => "upscale",
            Self::UiComposite => "ui_composite",
            Self::Present => "present",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassDescriptor {
    pub stable_id: &'static str,
    pub kind: PassKind,
    pub enabled: bool,
}

impl PassDescriptor {
    #[must_use]
    pub const fn new(stable_id: &'static str, kind: PassKind) -> Self {
        Self {
            stable_id,
            kind,
            enabled: true,
        }
    }
}

pub trait FrameGraphInterface {
    fn register_pass(&mut self, pass: PassDescriptor) -> PassHandle;
    fn pass_count(&self) -> usize;
}

pub trait PassRegistry {
    fn register_renderer_pass(&mut self, pass: PassDescriptor) -> PassHandle;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceHandle(pub u32);

impl ResourceHandle {
    pub const INVALID: Self = Self(u32::MAX);

    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Buffer,
    Texture,
    DescriptorTable,
    AccelerationStructure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceRequest {
    pub kind: ResourceKind,
    pub bytes: u64,
    pub transient: bool,
}

pub trait ResourceAllocator {
    fn allocate(&mut self, request: ResourceRequest) -> ResourceHandle;
    fn allocation_count(&self) -> u32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneInstanceId(pub u32);

impl SceneInstanceId {
    pub const INVALID: Self = Self(u32::MAX);

    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SceneInstanceRecord {
    pub stable_history_key: u64,
    pub geometry_ref: u32,
    pub material_ref: u32,
}

pub trait SceneDatabase {
    fn upsert_instance(&mut self, instance: SceneInstanceRecord) -> SceneInstanceId;
    fn remove_instance(&mut self, id: SceneInstanceId) -> bool;
    fn instance_count(&self) -> u32;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearColorFrame {
    pub frame_index: u64,
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentResult {
    pub submitted: bool,
    pub frame_index: u64,
}

pub trait Presentation {
    fn present_clear_color(&mut self, frame: ClearColorFrame) -> PresentResult;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCoreInterfaceMap {
    pub settings: bool,
    pub backend_capability_model: bool,
    pub frame_graph_shell: bool,
    pub gpu_scene_database_shell: bool,
    pub resource_allocator_shell: bool,
    pub upload_arena_ownership_seam: bool,
    pub cef_compositor_interface: bool,
    pub upscaler_frame_generation_interface: bool,
    pub pass_diagnostics: bool,
}

pub const RENDERER_CORE_INTERFACE_MAP: RendererCoreInterfaceMap = RendererCoreInterfaceMap {
    settings: true,
    backend_capability_model: true,
    frame_graph_shell: true,
    gpu_scene_database_shell: true,
    resource_allocator_shell: true,
    upload_arena_ownership_seam: true,
    cef_compositor_interface: true,
    upscaler_frame_generation_interface: true,
    pass_diagnostics: true,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadArenaOwnershipSeam {
    pub owner_crate: &'static str,
    pub legacy_bridge_borrow_allowed: bool,
    pub stable_id: &'static str,
}

pub const RENDERER_UPLOAD_ARENA_SEAM: UploadArenaOwnershipSeam = UploadArenaOwnershipSeam {
    owner_crate: crate::FUN_RENDERER_CRATE_NAME,
    legacy_bridge_borrow_allowed: true,
    stable_id: "fun_renderer.upload_arena.ownership_seam",
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefCompositorInterface {
    pub gpu_shared_texture_required: bool,
    pub cpu_runtime_upload_fallback_allowed: bool,
    pub owner_crate: &'static str,
}

pub const RENDERER_CEF_COMPOSITOR_INTERFACE: CefCompositorInterface = CefCompositorInterface {
    gpu_shared_texture_required: true,
    cpu_runtime_upload_fallback_allowed: false,
    owner_crate: crate::FUN_RENDERER_CRATE_NAME,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpscaleFrameGenerationInterface {
    pub scene_color_ui_color_separate: bool,
    pub hudless_scene_color_required: bool,
    pub depth_required: bool,
    pub motion_vectors_required: bool,
    pub present_time_resource_lifetimes_required: bool,
}

pub const RENDERER_UPSCALE_FRAME_GENERATION_INTERFACE: UpscaleFrameGenerationInterface =
    UpscaleFrameGenerationInterface {
        scene_color_ui_color_separate: true,
        hudless_scene_color_required: true,
        depth_required: true,
        motion_vectors_required: true,
        present_time_resource_lifetimes_required: true,
    };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCoreBootReport {
    pub runtime_backend: FunRendererRuntimeBackend,
    pub backend: FunRendererBackend,
    pub registered_passes: u16,
    pub produced_clear_color_frame: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererCoreLifecycle {
    Initialized,
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCoreDiagnostics {
    pub runtime_backend: FunRendererRuntimeBackend,
    pub backend: FunRendererBackend,
    pub capabilities: BackendCapabilities,
    pub registered_passes: u16,
    pub allocation_count: u32,
    pub instance_count: u32,
    pub presented_frame_count: u64,
    pub lifecycle: RendererCoreLifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererCoreShutdownReport {
    pub runtime_backend: FunRendererRuntimeBackend,
    pub backend: FunRendererBackend,
    pub released_scene_instances: u32,
    pub presented_frame_count: u64,
    pub clean_shutdown: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoopRendererCore {
    settings: RendererCoreSettings,
    frame_index: u64,
    passes: Vec<PassDescriptor>,
    allocations: u32,
    instances: Vec<SceneInstanceRecord>,
    presented_frame_count: u64,
}

impl NoopRendererCore {
    #[must_use]
    pub fn boot(settings: RendererCoreSettings) -> (Self, RendererCoreBootReport) {
        let mut core = Self {
            settings,
            frame_index: 0,
            passes: Vec::new(),
            allocations: 0,
            instances: Vec::new(),
            presented_frame_count: 0,
        };
        core.register_pass(PassDescriptor::new(
            "fun_renderer.pass.clear_color",
            PassKind::ClearColor,
        ));
        core.register_pass(PassDescriptor::new(
            "fun_renderer.pass.present",
            PassKind::Present,
        ));
        let report = RendererCoreBootReport {
            runtime_backend: core.settings.runtime_backend,
            backend: core.settings.backend,
            registered_passes: core.pass_count() as u16,
            produced_clear_color_frame: true,
        };
        (core, report)
    }

    #[must_use]
    pub const fn settings(&self) -> RendererCoreSettings {
        self.settings
    }

    #[must_use]
    pub fn produce_clear_color_frame(&mut self) -> ClearColorFrame {
        self.frame_index = self.frame_index.saturating_add(1);
        ClearColorFrame {
            frame_index: self.frame_index,
            color: self.settings.clear_color,
        }
    }

    #[must_use]
    pub const fn presented_frame_count(&self) -> u64 {
        self.presented_frame_count
    }

    #[must_use]
    pub fn diagnostics(&self) -> RendererCoreDiagnostics {
        let registered_passes = match u16::try_from(self.passes.len()) {
            Ok(value) => value,
            Err(_) => u16::MAX,
        };
        RendererCoreDiagnostics {
            runtime_backend: self.settings.runtime_backend,
            backend: self.settings.backend,
            capabilities: self.capabilities(),
            registered_passes,
            allocation_count: self.allocations,
            instance_count: self.instance_count(),
            presented_frame_count: self.presented_frame_count,
            lifecycle: RendererCoreLifecycle::Initialized,
        }
    }

    #[must_use]
    pub fn shutdown(self) -> RendererCoreShutdownReport {
        RendererCoreShutdownReport {
            runtime_backend: self.settings.runtime_backend,
            backend: self.settings.backend,
            released_scene_instances: self.instances.len() as u32,
            presented_frame_count: self.presented_frame_count,
            clean_shutdown: true,
        }
    }
}

impl DeviceBackend for NoopRendererCore {
    fn backend(&self) -> FunRendererBackend {
        self.settings.backend
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::minimal(self.settings.backend)
    }
}

impl FrameGraphInterface for NoopRendererCore {
    fn register_pass(&mut self, pass: PassDescriptor) -> PassHandle {
        let Ok(index) = u16::try_from(self.passes.len()) else {
            return PassHandle::INVALID;
        };
        self.passes.push(pass);
        PassHandle::new(index)
    }

    fn pass_count(&self) -> usize {
        self.passes.len()
    }
}

impl PassRegistry for NoopRendererCore {
    fn register_renderer_pass(&mut self, pass: PassDescriptor) -> PassHandle {
        self.register_pass(pass)
    }
}

impl ResourceAllocator for NoopRendererCore {
    fn allocate(&mut self, request: ResourceRequest) -> ResourceHandle {
        if request.bytes == 0 {
            return ResourceHandle::INVALID;
        }
        let handle = ResourceHandle::new(self.allocations);
        self.allocations = self.allocations.saturating_add(1);
        handle
    }

    fn allocation_count(&self) -> u32 {
        self.allocations
    }
}

impl SceneDatabase for NoopRendererCore {
    fn upsert_instance(&mut self, instance: SceneInstanceRecord) -> SceneInstanceId {
        if let Some(index) = self
            .instances
            .iter()
            .position(|candidate| candidate.stable_history_key == instance.stable_history_key)
        {
            self.instances[index] = instance;
            return SceneInstanceId::new(index as u32);
        }
        let Ok(index) = u32::try_from(self.instances.len()) else {
            return SceneInstanceId::INVALID;
        };
        self.instances.push(instance);
        SceneInstanceId::new(index)
    }

    fn remove_instance(&mut self, id: SceneInstanceId) -> bool {
        let Ok(index) = usize::try_from(id.0) else {
            return false;
        };
        if index >= self.instances.len() {
            return false;
        }
        self.instances.swap_remove(index);
        true
    }

    fn instance_count(&self) -> u32 {
        self.instances.len() as u32
    }
}

impl Presentation for NoopRendererCore {
    fn present_clear_color(&mut self, frame: ClearColorFrame) -> PresentResult {
        self.presented_frame_count = self.presented_frame_count.saturating_add(1);
        PresentResult {
            submitted: true,
            frame_index: frame.frame_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_feature_toggles_expose_canonical_boundary_flags() {
        let toggles = RendererFeatureToggles::compiled();

        assert_eq!(toggles.new_core, cfg!(feature = "fun_renderer_core"));
        assert_eq!(toggles.dx12, cfg!(feature = "dx12_native_interop"));
        assert_eq!(toggles.cef_gpu_only, cfg!(feature = "cef_gpu_only"));
        assert_eq!(toggles.dlss, cfg!(feature = "dlss"));
        assert_eq!(toggles.frame_generation, cfg!(feature = "frame_generation"));
    }

    #[test]
    fn noop_core_boots_and_presents_clear_color_frame() {
        let (mut renderer, report) = NoopRendererCore::boot(RendererCoreSettings::default());

        assert_eq!(report.runtime_backend, FunRendererRuntimeBackend::Fun);
        assert_eq!(report.backend, FunRendererBackend::Dx12);
        assert_eq!(report.registered_passes, 2);
        assert!(report.produced_clear_color_frame);

        let frame = renderer.produce_clear_color_frame();
        let result = renderer.present_clear_color(frame);

        assert!(result.submitted);
        assert_eq!(result.frame_index, 1);
        assert_eq!(renderer.presented_frame_count(), 1);

        let diagnostics = renderer.diagnostics();
        assert_eq!(diagnostics.lifecycle, RendererCoreLifecycle::Initialized);
        assert_eq!(diagnostics.registered_passes, 2);

        let shutdown = renderer.shutdown();
        assert!(shutdown.clean_shutdown);
        assert_eq!(shutdown.presented_frame_count, 1);
    }

    #[test]
    fn core_interfaces_cover_scene_database_and_allocator_seams() {
        let (mut renderer, _) = NoopRendererCore::boot(RendererCoreSettings::default());
        let handle = renderer.allocate(ResourceRequest {
            kind: ResourceKind::Buffer,
            bytes: 256,
            transient: true,
        });
        assert!(handle.is_valid());
        assert_eq!(renderer.allocation_count(), 1);

        let instance = renderer.upsert_instance(SceneInstanceRecord {
            stable_history_key: 99,
            geometry_ref: 3,
            material_ref: 7,
        });

        assert_eq!(renderer.instance_count(), 1);
        assert!(renderer.remove_instance(instance));
        assert_eq!(renderer.instance_count(), 0);
    }

    #[test]
    fn core_boundary_exposes_required_noop_interfaces() {
        let map = core::hint::black_box(RENDERER_CORE_INTERFACE_MAP);

        assert!(map.settings);
        assert!(map.backend_capability_model);
        assert!(map.frame_graph_shell);
        assert!(map.gpu_scene_database_shell);
        assert!(map.resource_allocator_shell);
        assert!(map.upload_arena_ownership_seam);
        assert!(map.cef_compositor_interface);
        assert!(map.upscaler_frame_generation_interface);
        assert!(map.pass_diagnostics);

        assert_eq!(
            RENDERER_UPLOAD_ARENA_SEAM.owner_crate,
            crate::FUN_RENDERER_CRATE_NAME
        );
        assert!(RENDERER_UPLOAD_ARENA_SEAM.legacy_bridge_borrow_allowed);
        assert!(RENDERER_CEF_COMPOSITOR_INTERFACE.gpu_shared_texture_required);
        assert!(!RENDERER_CEF_COMPOSITOR_INTERFACE.cpu_runtime_upload_fallback_allowed);
        assert!(RENDERER_UPSCALE_FRAME_GENERATION_INTERFACE.scene_color_ui_color_separate);
        assert!(RENDERER_UPSCALE_FRAME_GENERATION_INTERFACE.hudless_scene_color_required);
    }
}
