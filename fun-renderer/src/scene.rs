use fun_scene::{
    GeometryRef, GlobalTransform, InheritedVisibility, LuxImportance, LuxLightKind,
    LuxShadowPolicy, MaterialRef, Renderable, RenderableFlags, SceneStableIdentity, Transform,
    ViewVisibility, VirtualGeometryAuthoring, VirtualGeometryMode, Visibility,
};

use crate::resource::{
    RendererResourceKind, ResourceFrameAllocationDiagnostics, UploadResourceKind,
};

pub const GPU_SCENE_DATABASE_SCHEMA_VERSION: u16 = 1;
pub const GPU_SCENE_SYNTHETIC_STABLE_ID_BASE: u64 = 1 << 63;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StableInstanceId(pub u64);

impl StableInstanceId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn synthetic(value: u64) -> Self {
        Self(GPU_SCENE_SYNTHETIC_STABLE_ID_BASE | (value & !GPU_SCENE_SYNTHETIC_STABLE_ID_BASE))
    }

    #[must_use]
    pub const fn from_scene_identity(identity: SceneStableIdentity) -> Self {
        Self(identity.0.0)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneRecordKind {
    View,
    Instance,
    Mesh,
    Material,
    Light,
    PageMetadata,
}

impl SceneRecordKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Instance => "instance",
            Self::Mesh => "mesh",
            Self::Material => "material",
            Self::Light => "light",
            Self::PageMetadata => "page_metadata",
        }
    }

    #[must_use]
    pub const fn upload_site(self) -> &'static str {
        match self {
            Self::View => "fun_renderer.gpu_scene.views",
            Self::Instance => "fun_renderer.gpu_scene.instances",
            Self::Mesh => "fun_renderer.gpu_scene.meshes",
            Self::Material => "fun_renderer.gpu_scene.materials",
            Self::Light => "fun_renderer.gpu_scene.lights",
            Self::PageMetadata => "fun_renderer.gpu_scene.page_metadata",
        }
    }
}

macro_rules! scene_handle {
    ($name:ident) => {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name {
            pub index: u32,
            pub generation: u32,
        }

        impl $name {
            pub const INVALID: Self = Self {
                index: u32::MAX,
                generation: 0,
            };

            #[must_use]
            pub const fn new(index: u32, generation: u32) -> Self {
                Self { index, generation }
            }

            #[must_use]
            pub const fn is_valid(self) -> bool {
                self.index != u32::MAX && self.generation != 0
            }
        }
    };
}

scene_handle!(GpuViewHandle);
scene_handle!(GpuInstanceHandle);
scene_handle!(GpuMeshHandle);
scene_handle!(GpuMaterialHandle);
scene_handle!(GpuLightHandle);
scene_handle!(GpuPageMetadataHandle);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuVec2 {
    pub x: f32,
    pub y: f32,
}

impl GpuVec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };
}

impl Default for GpuVec2 {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuVec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl GpuVec3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub const ONE: Self = Self {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };
}

impl Default for GpuVec3 {
    fn default() -> Self {
        Self::ZERO
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuQuat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl GpuQuat {
    pub const IDENTITY: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };
}

impl Default for GpuQuat {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuTransform {
    pub translation: GpuVec3,
    pub rotation: GpuQuat,
    pub scale: GpuVec3,
}

impl GpuTransform {
    pub const IDENTITY: Self = Self {
        translation: GpuVec3::ZERO,
        rotation: GpuQuat::IDENTITY,
        scale: GpuVec3::ONE,
    };
}

impl Default for GpuTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl From<&Transform> for GpuTransform {
    fn from(transform: &Transform) -> Self {
        Self {
            translation: GpuVec3 {
                x: transform.translation.x,
                y: transform.translation.y,
                z: transform.translation.z,
            },
            rotation: GpuQuat {
                x: transform.rotation.x,
                y: transform.rotation.y,
                z: transform.rotation.z,
                w: transform.rotation.w,
            },
            scale: GpuVec3 {
                x: transform.scale.x,
                y: transform.scale.y,
                z: transform.scale.z,
            },
        }
    }
}

impl From<Transform> for GpuTransform {
    fn from(transform: Transform) -> Self {
        Self::from(&transform)
    }
}

impl From<&GlobalTransform> for GpuTransform {
    fn from(transform: &GlobalTransform) -> Self {
        Self {
            translation: GpuVec3 {
                x: transform.translation.x,
                y: transform.translation.y,
                z: transform.translation.z,
            },
            rotation: GpuQuat {
                x: transform.rotation.x,
                y: transform.rotation.y,
                z: transform.rotation.z,
                w: transform.rotation.w,
            },
            scale: GpuVec3 {
                x: transform.scale.x,
                y: transform.scale.y,
                z: transform.scale.z,
            },
        }
    }
}

impl From<GlobalTransform> for GpuTransform {
    fn from(transform: GlobalTransform) -> Self {
        Self::from(&transform)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuMat4 {
    pub columns: [[f32; 4]; 4],
}

impl GpuMat4 {
    pub const IDENTITY: Self = Self {
        columns: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
}

impl Default for GpuMat4 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuViewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl GpuViewport {
    pub const DEFAULT: Self = Self {
        x: 0,
        y: 0,
        width: 1,
        height: 1,
    };
}

impl Default for GpuViewport {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuHdrMetadata {
    pub exposure: f32,
    pub max_luminance_nits: f32,
    pub hdr_output: bool,
}

impl Default for GpuHdrMetadata {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            max_luminance_nits: 1000.0,
            hdr_output: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuViewRecord {
    pub handle: GpuViewHandle,
    pub camera_transform: GpuTransform,
    pub projection: GpuMat4,
    pub viewport: GpuViewport,
    pub jitter: GpuVec2,
    pub previous_view_projection: GpuMat4,
    pub current_view_projection: GpuMat4,
    pub exposure: GpuHdrMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuInstanceFlags(pub u32);

impl GpuInstanceFlags {
    pub const NONE: Self = Self(0);
    pub const STATIC: Self = Self(1 << 0);
    pub const DYNAMIC: Self = Self(1 << 1);
    pub const SHADOW_CASTER: Self = Self(1 << 2);
    pub const SHADOW_RECEIVER: Self = Self(1 << 3);
    pub const VIRTUAL_GEOMETRY: Self = Self(1 << 4);
    pub const PROCEDURAL_SOURCE: Self = Self(1 << 5);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl Default for GpuInstanceFlags {
    fn default() -> Self {
        Self::NONE
    }
}

impl From<RenderableFlags> for GpuInstanceFlags {
    fn from(flags: RenderableFlags) -> Self {
        let mut bits = 0;
        if flags.contains(RenderableFlags::STATIC) {
            bits |= Self::STATIC.0;
        }
        if flags.contains(RenderableFlags::DYNAMIC) {
            bits |= Self::DYNAMIC.0;
        }
        if flags.contains(RenderableFlags::SHADOW_CASTER) {
            bits |= Self::SHADOW_CASTER.0;
        }
        if flags.contains(RenderableFlags::SHADOW_RECEIVER) {
            bits |= Self::SHADOW_RECEIVER.0;
        }
        if flags.contains(RenderableFlags::PROCEDURAL_SOURCE) {
            bits |= Self::PROCEDURAL_SOURCE.0;
        }
        Self(bits)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GpuVisibilityState {
    #[default]
    Unknown,
    Visible,
    Culled,
    Hidden,
}

impl GpuVisibilityState {
    #[must_use]
    pub const fn from_fun_scene_visibility(
        local: Visibility,
        inherited: InheritedVisibility,
        view: ViewVisibility,
    ) -> Self {
        if matches!(local, Visibility::Hidden) || !inherited.visible {
            Self::Hidden
        } else if view.visible {
            Self::Visible
        } else {
            Self::Culled
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuInstanceRecord {
    pub handle: GpuInstanceHandle,
    pub stable_id: StableInstanceId,
    pub transform: GpuTransform,
    pub previous_transform: GpuTransform,
    pub mesh: GpuMeshHandle,
    pub material: GpuMaterialHandle,
    pub flags: GpuInstanceFlags,
    pub visibility: GpuVisibilityState,
    pub page_metadata: GpuPageMetadataHandle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuBufferRef(pub u64);

impl GpuBufferRef {
    pub const NONE: Self = Self(0);
}

impl Default for GpuBufferRef {
    fn default() -> Self {
        Self::NONE
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuVirtualGeometryRef(pub u64);

impl GpuVirtualGeometryRef {
    pub const NONE: Self = Self(0);
}

impl Default for GpuVirtualGeometryRef {
    fn default() -> Self {
        Self::NONE
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuMeshRecord {
    pub handle: GpuMeshHandle,
    pub source_geometry: GeometryRef,
    pub classic_mesh_buffer: GpuBufferRef,
    pub virtual_geometry: GpuVirtualGeometryRef,
    pub fallback_mesh: GpuBufferRef,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GpuMaterialClass {
    #[default]
    Opaque,
    AlphaTested,
    AlphaBlended,
    Emissive,
    Ui,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuTextureTableIndices {
    pub base_color: u32,
    pub normal: u32,
    pub orm: u32,
    pub emissive: u32,
}

impl GpuTextureTableIndices {
    pub const NONE: Self = Self {
        base_color: u32::MAX,
        normal: u32::MAX,
        orm: u32::MAX,
        emissive: u32::MAX,
    };
}

impl Default for GpuTextureTableIndices {
    fn default() -> Self {
        Self::NONE
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuMaterialFlags {
    pub alpha_tested: bool,
    pub alpha_blended: bool,
    pub specular: bool,
    pub emissive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuMaterialRecord {
    pub handle: GpuMaterialHandle,
    pub source_material: MaterialRef,
    pub class: GpuMaterialClass,
    pub textures: GpuTextureTableIndices,
    pub bindless_base_index: u32,
    pub flags: GpuMaterialFlags,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GpuLightType {
    Directional,
    #[default]
    Punctual,
    Area,
    EmissiveCandidate,
    Probe,
}

impl From<LuxLightKind> for GpuLightType {
    fn from(kind: LuxLightKind) -> Self {
        match kind {
            LuxLightKind::Directional => Self::Directional,
            LuxLightKind::Punctual => Self::Punctual,
            LuxLightKind::Area => Self::Area,
            LuxLightKind::EmissiveCandidate => Self::EmissiveCandidate,
            LuxLightKind::Probe => Self::Probe,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GpuShadowPolicy {
    None,
    StaticMap,
    #[default]
    VirtualDemandPaged,
    VirtualDirectionalClipmap,
    RayTraced,
}

impl From<LuxShadowPolicy> for GpuShadowPolicy {
    fn from(policy: LuxShadowPolicy) -> Self {
        match policy {
            LuxShadowPolicy::None => Self::None,
            LuxShadowPolicy::StaticMap => Self::StaticMap,
            LuxShadowPolicy::VirtualDemandPaged => Self::VirtualDemandPaged,
            LuxShadowPolicy::VirtualDirectionalClipmap => Self::VirtualDirectionalClipmap,
            LuxShadowPolicy::RayTraced => Self::RayTraced,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GpuImportanceHint {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

impl From<LuxImportance> for GpuImportanceHint {
    fn from(importance: LuxImportance) -> Self {
        match importance {
            LuxImportance::Low => Self::Low,
            LuxImportance::Normal => Self::Normal,
            LuxImportance::High => Self::High,
            LuxImportance::Critical => Self::Critical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuLightRecord {
    pub handle: GpuLightHandle,
    pub stable_id: StableInstanceId,
    pub light_type: GpuLightType,
    pub transform: GpuTransform,
    pub color_rgba: [f32; 4],
    pub intensity_lux: f32,
    pub radius: f32,
    pub cone_angle_radians: f32,
    pub shadow_policy: GpuShadowPolicy,
    pub importance: GpuImportanceHint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuPageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuPageMetadataRecord {
    pub handle: GpuPageMetadataHandle,
    pub stable_id: StableInstanceId,
    pub geometry_page_id: GpuPageId,
    pub shadow_page_id: GpuPageId,
    pub texture_residency_id: GpuPageId,
    pub gi_cache_id: GpuPageId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuSceneDirtyRange {
    pub record_kind: SceneRecordKind,
    pub start: u32,
    pub end_exclusive: u32,
    pub byte_count: u64,
}

impl GpuSceneDirtyRange {
    #[must_use]
    pub const fn record_count(self) -> u32 {
        self.end_exclusive.saturating_sub(self.start)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuSceneUploadReport {
    pub upload_bytes: u64,
    pub upload_record_count: u32,
    pub dirty_range_count: u32,
    pub uploaded_generation: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GpuSceneDatabaseDiagnostics {
    pub schema_version: u16,
    pub view_count: u32,
    pub instance_count: u32,
    pub mesh_count: u32,
    pub material_count: u32,
    pub light_count: u32,
    pub page_metadata_count: u32,
    pub dirty_range_count: u32,
    pub created_record_count: u32,
    pub updated_record_count: u32,
    pub removed_record_count: u32,
    pub compaction_count: u32,
    pub stale_handle_reject_count: u32,
    pub upload_bytes: u64,
    pub uploaded_record_count: u32,
    pub scene_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpuInstanceInput {
    pub stable_id: StableInstanceId,
    pub transform: GpuTransform,
    pub mesh: GpuMeshHandle,
    pub material: GpuMaterialHandle,
    pub flags: GpuInstanceFlags,
    pub visibility: GpuVisibilityState,
    pub page_metadata: GpuPageMetadataHandle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SceneSlot<T> {
    generation: u32,
    record: Option<T>,
}

impl<T> Default for SceneSlot<T> {
    fn default() -> Self {
        Self {
            generation: 1,
            record: None,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
#[cfg_attr(feature = "fun_ecs", derive(fun_ecs::Resource))]
pub struct GpuSceneDatabase {
    views: Vec<SceneSlot<GpuViewRecord>>,
    instances: Vec<SceneSlot<GpuInstanceRecord>>,
    meshes: Vec<SceneSlot<GpuMeshRecord>>,
    materials: Vec<SceneSlot<GpuMaterialRecord>>,
    lights: Vec<SceneSlot<GpuLightRecord>>,
    page_metadata: Vec<SceneSlot<GpuPageMetadataRecord>>,
    stable_instances: Vec<(StableInstanceId, GpuInstanceHandle)>,
    stable_lights: Vec<(StableInstanceId, GpuLightHandle)>,
    source_meshes: Vec<(u32, GpuMeshHandle)>,
    source_materials: Vec<(u32, GpuMaterialHandle)>,
    stable_pages: Vec<(StableInstanceId, GpuPageMetadataHandle)>,
    dirty_ranges: Vec<GpuSceneDirtyRange>,
    diagnostics: GpuSceneDatabaseDiagnostics,
    upload_generation: u64,
}

impl GpuSceneDatabase {
    #[must_use]
    pub fn diagnostics(&self) -> GpuSceneDatabaseDiagnostics {
        GpuSceneDatabaseDiagnostics {
            schema_version: GPU_SCENE_DATABASE_SCHEMA_VERSION,
            view_count: live_count(&self.views),
            instance_count: live_count(&self.instances),
            mesh_count: live_count(&self.meshes),
            material_count: live_count(&self.materials),
            light_count: live_count(&self.lights),
            page_metadata_count: live_count(&self.page_metadata),
            dirty_range_count: self.dirty_ranges.len() as u32,
            ..self.diagnostics
        }
    }

    #[must_use]
    pub fn dirty_ranges(&self) -> &[GpuSceneDirtyRange] {
        &self.dirty_ranges
    }

    #[must_use]
    pub fn static_scene_ready(&self) -> bool {
        let diagnostics = self.diagnostics();
        diagnostics.instance_count != 0
            && diagnostics.mesh_count != 0
            && diagnostics.material_count != 0
    }

    #[must_use]
    pub fn instance(&self, handle: GpuInstanceHandle) -> Option<&GpuInstanceRecord> {
        record_for_handle(&self.instances, handle.index, handle.generation)
    }

    #[must_use]
    pub fn mesh(&self, handle: GpuMeshHandle) -> Option<&GpuMeshRecord> {
        record_for_handle(&self.meshes, handle.index, handle.generation)
    }

    #[must_use]
    pub fn material(&self, handle: GpuMaterialHandle) -> Option<&GpuMaterialRecord> {
        record_for_handle(&self.materials, handle.index, handle.generation)
    }

    #[must_use]
    pub fn light(&self, handle: GpuLightHandle) -> Option<&GpuLightRecord> {
        record_for_handle(&self.lights, handle.index, handle.generation)
    }

    #[must_use]
    pub fn instance_handle_for_stable(
        &self,
        stable_id: StableInstanceId,
    ) -> Option<GpuInstanceHandle> {
        self.stable_instances
            .iter()
            .find(|(candidate, _)| *candidate == stable_id)
            .map(|(_, handle)| *handle)
            .filter(|handle| self.instance(*handle).is_some())
    }

    #[must_use]
    pub fn light_handle_for_stable(&self, stable_id: StableInstanceId) -> Option<GpuLightHandle> {
        self.stable_lights
            .iter()
            .find(|(candidate, _)| *candidate == stable_id)
            .map(|(_, handle)| *handle)
            .filter(|handle| self.light(*handle).is_some())
    }

    pub fn upsert_view(&mut self, mut record: GpuViewRecord) -> GpuViewHandle {
        if record.handle.is_valid() && self.view_handle_valid(record.handle) {
            let handle = record.handle;
            if let Some(slot) = self.views.get_mut(handle.index as usize) {
                record.handle = handle;
                slot.record = Some(record);
                self.note_updated(SceneRecordKind::View, handle.index);
                return handle;
            }
        }

        let handle = self.alloc_view_handle();
        record.handle = handle;
        self.views[handle.index as usize].record = Some(record);
        self.note_created(SceneRecordKind::View, handle.index);
        handle
    }

    pub fn upsert_mesh_from_fun_scene(&mut self, geometry: GeometryRef) -> GpuMeshHandle {
        if !geometry.is_valid() {
            return GpuMeshHandle::INVALID;
        }
        if let Some(handle) = self
            .source_meshes
            .iter()
            .find(|(source, _)| *source == geometry.0)
            .map(|(_, handle)| *handle)
            .filter(|handle| self.mesh(*handle).is_some())
        {
            return handle;
        }

        let handle = self.alloc_mesh_handle();
        let source = u64::from(geometry.0);
        self.meshes[handle.index as usize].record = Some(GpuMeshRecord {
            handle,
            source_geometry: geometry,
            classic_mesh_buffer: GpuBufferRef(source),
            virtual_geometry: GpuVirtualGeometryRef(source << 32),
            fallback_mesh: GpuBufferRef(source | (1 << 31)),
        });
        self.source_meshes.push((geometry.0, handle));
        self.note_created(SceneRecordKind::Mesh, handle.index);
        handle
    }

    pub fn upsert_material_from_fun_scene(&mut self, material: MaterialRef) -> GpuMaterialHandle {
        if !material.is_valid() {
            return GpuMaterialHandle::INVALID;
        }
        if let Some(handle) = self
            .source_materials
            .iter()
            .find(|(source, _)| *source == material.0)
            .map(|(_, handle)| *handle)
            .filter(|handle| self.material(*handle).is_some())
        {
            return handle;
        }

        let handle = self.alloc_material_handle();
        self.materials[handle.index as usize].record = Some(GpuMaterialRecord {
            handle,
            source_material: material,
            class: GpuMaterialClass::Opaque,
            textures: GpuTextureTableIndices::NONE,
            bindless_base_index: material.0,
            flags: GpuMaterialFlags {
                specular: true,
                ..Default::default()
            },
        });
        self.source_materials.push((material.0, handle));
        self.note_created(SceneRecordKind::Material, handle.index);
        handle
    }

    pub fn upsert_page_metadata_from_fun_scene(
        &mut self,
        stable_id: StableInstanceId,
        authoring: &VirtualGeometryAuthoring,
    ) -> GpuPageMetadataHandle {
        if matches!(authoring.mode, VirtualGeometryMode::Disabled) {
            return GpuPageMetadataHandle::INVALID;
        }
        if let Some(handle) = self
            .stable_pages
            .iter()
            .find(|(candidate, _)| *candidate == stable_id)
            .map(|(_, handle)| *handle)
            .filter(|handle| self.page_metadata_record(*handle).is_some())
        {
            self.note_updated(SceneRecordKind::PageMetadata, handle.index);
            return handle;
        }

        let handle = self.alloc_page_metadata_handle();
        let seed = stable_id.0;
        self.page_metadata[handle.index as usize].record = Some(GpuPageMetadataRecord {
            handle,
            stable_id,
            geometry_page_id: GpuPageId(seed ^ 0x1000),
            shadow_page_id: GpuPageId(seed ^ 0x2000),
            texture_residency_id: GpuPageId(seed ^ 0x3000),
            gi_cache_id: GpuPageId(seed ^ 0x4000),
        });
        self.stable_pages.push((stable_id, handle));
        self.note_created(SceneRecordKind::PageMetadata, handle.index);
        handle
    }

    #[must_use]
    pub fn page_metadata_record(
        &self,
        handle: GpuPageMetadataHandle,
    ) -> Option<&GpuPageMetadataRecord> {
        record_for_handle(&self.page_metadata, handle.index, handle.generation)
    }

    pub fn upsert_instance(&mut self, input: GpuInstanceInput) -> GpuInstanceHandle {
        if !input.stable_id.is_valid() {
            self.diagnostics.stale_handle_reject_count =
                self.diagnostics.stale_handle_reject_count.saturating_add(1);
            return GpuInstanceHandle::INVALID;
        }

        if let Some(handle) = self.instance_handle_for_stable(input.stable_id) {
            let previous_transform = self
                .instance(handle)
                .map_or(input.transform, |record| record.transform);
            let record = GpuInstanceRecord {
                handle,
                stable_id: input.stable_id,
                transform: input.transform,
                previous_transform,
                mesh: input.mesh,
                material: input.material,
                flags: input.flags,
                visibility: input.visibility,
                page_metadata: input.page_metadata,
            };
            self.instances[handle.index as usize].record = Some(record);
            self.note_updated(SceneRecordKind::Instance, handle.index);
            return handle;
        }

        let handle = self.alloc_instance_handle();
        self.instances[handle.index as usize].record = Some(GpuInstanceRecord {
            handle,
            stable_id: input.stable_id,
            transform: input.transform,
            previous_transform: input.transform,
            mesh: input.mesh,
            material: input.material,
            flags: input.flags,
            visibility: input.visibility,
            page_metadata: input.page_metadata,
        });
        self.stable_instances.push((input.stable_id, handle));
        self.note_created(SceneRecordKind::Instance, handle.index);
        handle
    }

    pub fn upsert_renderable_from_fun_scene(
        &mut self,
        stable_id: StableInstanceId,
        renderable: &Renderable,
        transform: GpuTransform,
        virtual_geometry: Option<&VirtualGeometryAuthoring>,
    ) -> GpuInstanceHandle {
        self.upsert_renderable_from_fun_scene_with_visibility(
            stable_id,
            renderable,
            transform,
            virtual_geometry,
            GpuVisibilityState::Unknown,
        )
    }

    pub fn upsert_renderable_from_fun_scene_with_visibility(
        &mut self,
        stable_id: StableInstanceId,
        renderable: &Renderable,
        transform: GpuTransform,
        virtual_geometry: Option<&VirtualGeometryAuthoring>,
        visibility: GpuVisibilityState,
    ) -> GpuInstanceHandle {
        let mesh = self.upsert_mesh_from_fun_scene(renderable.geometry);
        let material = self.upsert_material_from_fun_scene(renderable.material);
        let page_metadata = virtual_geometry.map_or(GpuPageMetadataHandle::INVALID, |authoring| {
            self.upsert_page_metadata_from_fun_scene(stable_id, authoring)
        });
        let mut flags = GpuInstanceFlags::from(renderable.flags);
        if page_metadata.is_valid() {
            flags.0 |= GpuInstanceFlags::VIRTUAL_GEOMETRY.0;
        }
        self.upsert_instance(GpuInstanceInput {
            stable_id,
            transform,
            mesh,
            material,
            flags,
            visibility,
            page_metadata,
        })
    }

    pub fn upsert_light_from_fun_scene(
        &mut self,
        stable_id: StableInstanceId,
        light: &fun_scene::LuxLight,
        transform: GpuTransform,
    ) -> GpuLightHandle {
        if !stable_id.is_valid() {
            self.diagnostics.stale_handle_reject_count =
                self.diagnostics.stale_handle_reject_count.saturating_add(1);
            return GpuLightHandle::INVALID;
        }

        let color = light.color.to_srgba();
        let record_for_handle = |handle| GpuLightRecord {
            handle,
            stable_id,
            light_type: GpuLightType::from(light.kind),
            transform,
            color_rgba: [color.red, color.green, color.blue, color.alpha],
            intensity_lux: light.intensity_lux,
            radius: light.range,
            cone_angle_radians: 0.0,
            shadow_policy: GpuShadowPolicy::from(light.shadow_policy),
            importance: GpuImportanceHint::from(light.importance),
        };

        if let Some(handle) = self
            .stable_lights
            .iter()
            .find(|(candidate, _)| *candidate == stable_id)
            .map(|(_, handle)| *handle)
            .filter(|handle| self.light(*handle).is_some())
        {
            self.lights[handle.index as usize].record = Some(record_for_handle(handle));
            self.note_updated(SceneRecordKind::Light, handle.index);
            return handle;
        }

        let handle = self.alloc_light_handle();
        self.lights[handle.index as usize].record = Some(record_for_handle(handle));
        self.stable_lights.push((stable_id, handle));
        self.note_created(SceneRecordKind::Light, handle.index);
        handle
    }

    pub fn remove_instance_by_stable(&mut self, stable_id: StableInstanceId) -> bool {
        let Some(position) = self
            .stable_instances
            .iter()
            .position(|(candidate, _)| *candidate == stable_id)
        else {
            return false;
        };
        let (_, handle) = self.stable_instances.swap_remove(position);
        self.remove_instance(handle)
    }

    pub fn remove_instance(&mut self, handle: GpuInstanceHandle) -> bool {
        let Some(slot) = self.instances.get_mut(handle.index as usize) else {
            self.diagnostics.stale_handle_reject_count =
                self.diagnostics.stale_handle_reject_count.saturating_add(1);
            return false;
        };
        if slot.generation != handle.generation || slot.record.is_none() {
            self.diagnostics.stale_handle_reject_count =
                self.diagnostics.stale_handle_reject_count.saturating_add(1);
            return false;
        }
        slot.record = None;
        slot.generation = slot.generation.saturating_add(1).max(1);
        self.note_removed(SceneRecordKind::Instance, handle.index);
        true
    }

    pub fn compact_removed_records(&mut self) -> u32 {
        let before = self.instances.len()
            + self.meshes.len()
            + self.materials.len()
            + self.lights.len()
            + self.page_metadata.len()
            + self.views.len();
        trim_empty_tail(&mut self.instances);
        trim_empty_tail(&mut self.meshes);
        trim_empty_tail(&mut self.materials);
        trim_empty_tail(&mut self.lights);
        trim_empty_tail(&mut self.page_metadata);
        trim_empty_tail(&mut self.views);
        let after = self.instances.len()
            + self.meshes.len()
            + self.materials.len()
            + self.lights.len()
            + self.page_metadata.len()
            + self.views.len();
        let removed = before.saturating_sub(after) as u32;
        if removed != 0 {
            self.diagnostics.compaction_count =
                self.diagnostics.compaction_count.saturating_add(removed);
            self.diagnostics.scene_revision = self.diagnostics.scene_revision.saturating_add(1);
        }
        removed
    }

    pub fn upload_dirty_records(
        &mut self,
        diagnostics: &mut ResourceFrameAllocationDiagnostics,
    ) -> GpuSceneUploadReport {
        let mut report = GpuSceneUploadReport {
            dirty_range_count: self.dirty_ranges.len() as u32,
            uploaded_generation: self.upload_generation.saturating_add(1),
            ..Default::default()
        };
        for range in &self.dirty_ranges {
            report.upload_bytes = report.upload_bytes.saturating_add(range.byte_count);
            report.upload_record_count = report
                .upload_record_count
                .saturating_add(range.record_count());
            diagnostics.record_allocation(
                range.record_kind.upload_site(),
                RendererResourceKind::Upload(UploadResourceKind::TransientUploadBatches),
                range.byte_count,
                range.record_count(),
            );
        }
        self.upload_generation = report.uploaded_generation;
        self.diagnostics.upload_bytes = self
            .diagnostics
            .upload_bytes
            .saturating_add(report.upload_bytes);
        self.diagnostics.uploaded_record_count = self
            .diagnostics
            .uploaded_record_count
            .saturating_add(report.upload_record_count);
        self.dirty_ranges.clear();
        report
    }

    fn alloc_view_handle(&mut self) -> GpuViewHandle {
        let index = self.views.len() as u32;
        self.views.push(SceneSlot::default());
        GpuViewHandle::new(index, self.views[index as usize].generation)
    }

    fn alloc_instance_handle(&mut self) -> GpuInstanceHandle {
        let index = self.instances.len() as u32;
        self.instances.push(SceneSlot::default());
        GpuInstanceHandle::new(index, self.instances[index as usize].generation)
    }

    fn alloc_mesh_handle(&mut self) -> GpuMeshHandle {
        let index = self.meshes.len() as u32;
        self.meshes.push(SceneSlot::default());
        GpuMeshHandle::new(index, self.meshes[index as usize].generation)
    }

    fn alloc_material_handle(&mut self) -> GpuMaterialHandle {
        let index = self.materials.len() as u32;
        self.materials.push(SceneSlot::default());
        GpuMaterialHandle::new(index, self.materials[index as usize].generation)
    }

    fn alloc_light_handle(&mut self) -> GpuLightHandle {
        let index = self.lights.len() as u32;
        self.lights.push(SceneSlot::default());
        GpuLightHandle::new(index, self.lights[index as usize].generation)
    }

    fn alloc_page_metadata_handle(&mut self) -> GpuPageMetadataHandle {
        let index = self.page_metadata.len() as u32;
        self.page_metadata.push(SceneSlot::default());
        GpuPageMetadataHandle::new(index, self.page_metadata[index as usize].generation)
    }

    fn view_handle_valid(&self, handle: GpuViewHandle) -> bool {
        record_for_handle(&self.views, handle.index, handle.generation).is_some()
    }

    fn note_created(&mut self, kind: SceneRecordKind, index: u32) {
        self.diagnostics.created_record_count =
            self.diagnostics.created_record_count.saturating_add(1);
        self.mark_dirty(kind, index);
    }

    fn note_updated(&mut self, kind: SceneRecordKind, index: u32) {
        self.diagnostics.updated_record_count =
            self.diagnostics.updated_record_count.saturating_add(1);
        self.mark_dirty(kind, index);
    }

    fn note_removed(&mut self, kind: SceneRecordKind, index: u32) {
        self.diagnostics.removed_record_count =
            self.diagnostics.removed_record_count.saturating_add(1);
        self.mark_dirty(kind, index);
    }

    fn mark_dirty(&mut self, kind: SceneRecordKind, index: u32) {
        let byte_count = record_size(kind);
        if let Some(last) = self.dirty_ranges.last_mut()
            && last.record_kind == kind
            && last.end_exclusive == index
        {
            last.end_exclusive = index.saturating_add(1);
            last.byte_count = last.byte_count.saturating_add(byte_count);
        } else {
            self.dirty_ranges.push(GpuSceneDirtyRange {
                record_kind: kind,
                start: index,
                end_exclusive: index.saturating_add(1),
                byte_count,
            });
        }
        self.diagnostics.scene_revision = self.diagnostics.scene_revision.saturating_add(1);
    }
}

fn live_count<T>(slots: &[SceneSlot<T>]) -> u32 {
    slots.iter().filter(|slot| slot.record.is_some()).count() as u32
}

fn record_for_handle<T>(slots: &[SceneSlot<T>], index: u32, generation: u32) -> Option<&T> {
    let slot = slots.get(index as usize)?;
    (generation != 0 && slot.generation == generation)
        .then_some(slot.record.as_ref())
        .flatten()
}

fn trim_empty_tail<T>(slots: &mut Vec<SceneSlot<T>>) {
    while slots.last().is_some_and(|slot| slot.record.is_none()) {
        slots.pop();
    }
}

const fn record_size(kind: SceneRecordKind) -> u64 {
    match kind {
        SceneRecordKind::View => core::mem::size_of::<GpuViewRecord>() as u64,
        SceneRecordKind::Instance => core::mem::size_of::<GpuInstanceRecord>() as u64,
        SceneRecordKind::Mesh => core::mem::size_of::<GpuMeshRecord>() as u64,
        SceneRecordKind::Material => core::mem::size_of::<GpuMaterialRecord>() as u64,
        SceneRecordKind::Light => core::mem::size_of::<GpuLightRecord>() as u64,
        SceneRecordKind::PageMetadata => core::mem::size_of::<GpuPageMetadataRecord>() as u64,
    }
}

#[cfg(test)]
mod tests {
    use fun_scene::{LuxLight, RenderableFlags};

    use super::*;

    #[test]
    fn scene_database_upserts_fun_scene_renderable_records() {
        let mut database = GpuSceneDatabase::default();
        let stable = StableInstanceId::new(42);
        let renderable = Renderable::new(
            GeometryRef::new(3),
            MaterialRef::new(7),
            RenderableFlags::STATIC_WORLD,
        );
        let virtual_geometry = VirtualGeometryAuthoring {
            mode: VirtualGeometryMode::StaticClusterPages,
            ..Default::default()
        };

        let handle = database.upsert_renderable_from_fun_scene(
            stable,
            &renderable,
            GpuTransform::IDENTITY,
            Some(&virtual_geometry),
        );

        let diagnostics = database.diagnostics();
        assert!(handle.is_valid());
        assert_eq!(diagnostics.instance_count, 1);
        assert_eq!(diagnostics.mesh_count, 1);
        assert_eq!(diagnostics.material_count, 1);
        assert_eq!(diagnostics.page_metadata_count, 1);
        assert!(database.static_scene_ready());
        let instance = database.instance(handle).expect("instance should exist");
        assert_eq!(instance.stable_id, stable);
        assert!(instance.flags.contains(GpuInstanceFlags::VIRTUAL_GEOMETRY));
        assert!(instance.page_metadata.is_valid());
    }

    #[test]
    fn scene_database_updates_previous_transform_and_dirty_ranges() {
        let mut database = GpuSceneDatabase::default();
        let renderable = Renderable::new(
            GeometryRef::new(11),
            MaterialRef::new(13),
            RenderableFlags::DYNAMIC,
        );
        let stable = StableInstanceId::new(99);
        let first = GpuTransform {
            translation: GpuVec3 {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            ..GpuTransform::IDENTITY
        };
        let second = GpuTransform {
            translation: GpuVec3 {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
            ..GpuTransform::IDENTITY
        };

        let handle = database.upsert_renderable_from_fun_scene(stable, &renderable, first, None);
        let same_handle =
            database.upsert_renderable_from_fun_scene(stable, &renderable, second, None);

        assert_eq!(handle, same_handle);
        let instance = database.instance(handle).expect("instance should exist");
        assert_eq!(instance.previous_transform, first);
        assert_eq!(instance.transform, second);
        assert!(
            database
                .dirty_ranges()
                .iter()
                .any(|range| range.record_kind == SceneRecordKind::Instance)
        );
        assert_eq!(database.diagnostics().updated_record_count, 1);
    }

    #[test]
    fn scene_database_consumes_fun_scene_transform_and_visibility() {
        let mut database = GpuSceneDatabase::default();
        let renderable = Renderable::new(
            GeometryRef::new(17),
            MaterialRef::new(19),
            RenderableFlags::STATIC,
        );
        let local = Transform::from_xyz(1.0, 2.0, 3.0);
        let visibility = GpuVisibilityState::from_fun_scene_visibility(
            Visibility::Visible,
            InheritedVisibility::VISIBLE,
            ViewVisibility::VISIBLE,
        );

        let handle = database.upsert_renderable_from_fun_scene_with_visibility(
            StableInstanceId::new(123),
            &renderable,
            GpuTransform::from(local),
            None,
            visibility,
        );

        let instance = database.instance(handle).expect("instance should exist");
        assert_eq!(instance.transform.translation.x, 1.0);
        assert_eq!(instance.transform.translation.y, 2.0);
        assert_eq!(instance.transform.translation.z, 3.0);
        assert_eq!(instance.visibility, GpuVisibilityState::Visible);
        assert_eq!(
            GpuVisibilityState::from_fun_scene_visibility(
                Visibility::Hidden,
                InheritedVisibility::VISIBLE,
                ViewVisibility::VISIBLE,
            ),
            GpuVisibilityState::Hidden
        );
        assert_eq!(
            GpuVisibilityState::from_fun_scene_visibility(
                Visibility::Visible,
                InheritedVisibility::VISIBLE,
                ViewVisibility::CULLED,
            ),
            GpuVisibilityState::Culled
        );
    }

    #[test]
    fn scene_database_rejects_stale_generation_handles_after_remove() {
        let mut database = GpuSceneDatabase::default();
        let renderable = Renderable::new(
            GeometryRef::new(1),
            MaterialRef::new(1),
            RenderableFlags::STATIC,
        );
        let stable = StableInstanceId::new(7);
        let handle = database.upsert_renderable_from_fun_scene(
            stable,
            &renderable,
            GpuTransform::IDENTITY,
            None,
        );

        assert!(database.remove_instance(handle));
        assert!(database.instance(handle).is_none());
        assert!(!database.remove_instance(handle));
        assert_eq!(database.diagnostics().stale_handle_reject_count, 1);
        assert_eq!(database.compact_removed_records(), 1);
    }

    #[test]
    fn scene_database_uploads_dirty_records_through_resource_diagnostics() {
        let mut database = GpuSceneDatabase::default();
        let stable = StableInstanceId::new(8);
        let renderable = Renderable::new(
            GeometryRef::new(2),
            MaterialRef::new(4),
            RenderableFlags::STATIC,
        );
        database.upsert_renderable_from_fun_scene(
            stable,
            &renderable,
            GpuTransform::IDENTITY,
            None,
        );
        let mut resource_diagnostics = ResourceFrameAllocationDiagnostics::default();

        let report = database.upload_dirty_records(&mut resource_diagnostics);

        assert!(report.upload_bytes > 0);
        assert!(report.upload_record_count >= 3);
        assert_eq!(database.dirty_ranges().len(), 0);
        assert!(resource_diagnostics.upload_bytes >= report.upload_bytes);
        assert!(
            resource_diagnostics
                .top_allocation_sites
                .iter()
                .any(|site| site.stable_id == SceneRecordKind::Instance.upload_site())
        );
        assert_eq!(
            database.diagnostics().uploaded_record_count,
            report.upload_record_count
        );
    }

    #[test]
    fn scene_database_stores_fun_scene_light_declarations() {
        let mut database = GpuSceneDatabase::default();
        let stable = StableInstanceId::new(18);
        let light = LuxLight::directional(80_000.0);

        let handle = database.upsert_light_from_fun_scene(stable, &light, GpuTransform::IDENTITY);

        let record = database.light(handle).expect("light should exist");
        assert_eq!(record.stable_id, stable);
        assert_eq!(record.light_type, GpuLightType::Directional);
        assert_eq!(record.intensity_lux, 80_000.0);
        assert_eq!(record.importance, GpuImportanceHint::High);
        assert_eq!(database.diagnostics().light_count, 1);
    }

    #[test]
    fn scene_database_stores_view_projection_history_and_hdr_metadata() {
        let mut database = GpuSceneDatabase::default();
        let handle = database.upsert_view(GpuViewRecord {
            handle: GpuViewHandle::INVALID,
            camera_transform: GpuTransform::IDENTITY,
            projection: GpuMat4::IDENTITY,
            viewport: GpuViewport {
                x: 10,
                y: 20,
                width: 1920,
                height: 1080,
            },
            jitter: GpuVec2 { x: 0.25, y: -0.25 },
            previous_view_projection: GpuMat4::IDENTITY,
            current_view_projection: GpuMat4::IDENTITY,
            exposure: GpuHdrMetadata {
                exposure: 1.2,
                max_luminance_nits: 1200.0,
                hdr_output: true,
            },
        });

        assert!(handle.is_valid());
        assert_eq!(database.diagnostics().view_count, 1);
        assert!(
            database
                .dirty_ranges()
                .iter()
                .any(|range| range.record_kind == SceneRecordKind::View)
        );
    }
}
