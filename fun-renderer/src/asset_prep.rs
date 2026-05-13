//! Renderer asset preparation, upload scheduling, residency, and fallbacks.
//!
//! Pass 21 makes asset prep ECS-driven: `RendererAssetEvents` (changed
//! meshes/materials/textures) feed the lifecycle states tracked here, and the
//! upload scheduler batches the resulting uploads. The renderer never reaches
//! through this module to backend handles — every record carries logical IDs,
//! byte counts, and typed fallback identifiers, so the bridge stays the only
//! owner of the actual GPU resources.
//!
//! The design rules from `AGENTS.md` apply:
//! - asset prep observes the registry rather than running per-entity work,
//! - upload systems batch before backend submission,
//! - cleanup retires by fence completion (modelled here as the
//!   `evictable_after_fence_value` field).

use std::collections::BTreeMap;

use fun_ecs::Resource;

use crate::component_api::{RenderMaterialAssetId, RenderMeshAssetId, RenderTextureAssetId};
use crate::extraction::RendererAssetEvents;

pub const ASSET_PREP_SCHEMA_VERSION: u16 = 1;
pub const ASSET_UPLOAD_SCHEDULER_DEFAULT_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

/// Asset-level lifecycle state. The renderer observes this; entities never
/// hold a backend handle directly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererAssetLifecycleState {
    #[default]
    Unprepared,
    CpuPrepared,
    UploadQueued,
    GpuResident,
    Evictable,
    Evicted,
    Failed,
}

impl RendererAssetLifecycleState {
    pub const ALL: [Self; 7] = [
        Self::Unprepared,
        Self::CpuPrepared,
        Self::UploadQueued,
        Self::GpuResident,
        Self::Evictable,
        Self::Evicted,
        Self::Failed,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unprepared => "unprepared",
            Self::CpuPrepared => "cpu_prepared",
            Self::UploadQueued => "upload_queued",
            Self::GpuResident => "gpu_resident",
            Self::Evictable => "evictable",
            Self::Evicted => "evicted",
            Self::Failed => "failed",
        }
    }

    #[must_use]
    pub const fn is_resident(self) -> bool {
        matches!(self, Self::GpuResident | Self::Evictable)
    }

    #[must_use]
    pub const fn is_failed(self) -> bool {
        matches!(self, Self::Failed)
    }

    #[must_use]
    pub const fn requires_fallback(self) -> bool {
        matches!(
            self,
            Self::Unprepared
                | Self::CpuPrepared
                | Self::UploadQueued
                | Self::Evicted
                | Self::Failed
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererAssetLifecycleTransition {
    AssetChangedEvent,
    CpuPrepCompleted,
    UploadEnqueued,
    UploadCompleted,
    EvictionPolicyMarkedEvictable,
    Evicted,
    Failed { reason: AssetPrepFailureReason },
    ResurrectAfterEviction,
}

impl RendererAssetLifecycleTransition {
    #[must_use]
    pub const fn target_state(
        self,
        current: RendererAssetLifecycleState,
    ) -> RendererAssetLifecycleState {
        match self {
            Self::AssetChangedEvent => RendererAssetLifecycleState::Unprepared,
            Self::CpuPrepCompleted => RendererAssetLifecycleState::CpuPrepared,
            Self::UploadEnqueued => RendererAssetLifecycleState::UploadQueued,
            Self::UploadCompleted => RendererAssetLifecycleState::GpuResident,
            Self::EvictionPolicyMarkedEvictable => RendererAssetLifecycleState::Evictable,
            Self::Evicted => RendererAssetLifecycleState::Evicted,
            Self::Failed { .. } => RendererAssetLifecycleState::Failed,
            Self::ResurrectAfterEviction => match current {
                RendererAssetLifecycleState::Evicted => RendererAssetLifecycleState::Unprepared,
                _ => current,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetPrepFailureReason {
    SourceMissing,
    SourceCorrupt,
    SchemaMismatch,
    ByteBudgetExceeded,
    UploadFailed,
    DependencyFailed,
    UnsupportedFormat,
}

impl AssetPrepFailureReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceMissing => "source_missing",
            Self::SourceCorrupt => "source_corrupt",
            Self::SchemaMismatch => "schema_mismatch",
            Self::ByteBudgetExceeded => "byte_budget_exceeded",
            Self::UploadFailed => "upload_failed",
            Self::DependencyFailed => "dependency_failed",
            Self::UnsupportedFormat => "unsupported_format",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererAssetKind {
    #[default]
    Mesh,
    Texture,
    Material,
    Shader,
    Pipeline,
}

impl RendererAssetKind {
    pub const ALL: [Self; 5] = [
        Self::Mesh,
        Self::Texture,
        Self::Material,
        Self::Shader,
        Self::Pipeline,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mesh => "mesh",
            Self::Texture => "texture",
            Self::Material => "material",
            Self::Shader => "shader",
            Self::Pipeline => "pipeline",
        }
    }
}

/// Mesh prep record: vertex/index buffer sizes, bounds, and meshlet metadata
/// scaffold. The actual buffer handles live in the bridge — only the byte
/// counts and logical references appear here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MeshAssetPrepRecord {
    pub mesh: RenderMeshAssetId,
    pub state: RendererAssetLifecycleState,
    pub vertex_buffer_bytes: u64,
    pub index_buffer_bytes: u64,
    pub meshlet_metadata_bytes: u64,
    pub bounds_radius_milli: u32,
    pub last_failure_reason: Option<AssetPrepFailureReason>,
    pub last_used_frame: u64,
    pub evictable_after_fence_value: u64,
}

impl MeshAssetPrepRecord {
    #[must_use]
    pub const fn new(mesh: RenderMeshAssetId) -> Self {
        Self {
            mesh,
            state: RendererAssetLifecycleState::Unprepared,
            vertex_buffer_bytes: 0,
            index_buffer_bytes: 0,
            meshlet_metadata_bytes: 0,
            bounds_radius_milli: 0,
            last_failure_reason: None,
            last_used_frame: 0,
            evictable_after_fence_value: 0,
        }
    }

    #[must_use]
    pub const fn total_bytes(self) -> u64 {
        self.vertex_buffer_bytes
            .saturating_add(self.index_buffer_bytes)
            .saturating_add(self.meshlet_metadata_bytes)
    }
}

/// Texture prep record: format metadata, mip count, sRGB/linear policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureAssetPrepRecord {
    pub texture: RenderTextureAssetId,
    pub state: RendererAssetLifecycleState,
    pub width: u32,
    pub height: u32,
    pub mip_count: u8,
    pub format: TextureAssetFormat,
    pub color_policy: TextureColorPolicy,
    pub mip_bytes: u64,
    pub last_failure_reason: Option<AssetPrepFailureReason>,
    pub last_used_frame: u64,
    pub evictable_after_fence_value: u64,
}

impl TextureAssetPrepRecord {
    #[must_use]
    pub const fn new(texture: RenderTextureAssetId) -> Self {
        Self {
            texture,
            state: RendererAssetLifecycleState::Unprepared,
            width: 0,
            height: 0,
            mip_count: 0,
            format: TextureAssetFormat::Rgba8Srgb,
            color_policy: TextureColorPolicy::Srgb,
            mip_bytes: 0,
            last_failure_reason: None,
            last_used_frame: 0,
            evictable_after_fence_value: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureAssetFormat {
    #[default]
    Rgba8Srgb,
    Rgba8Unorm,
    Rgba16Float,
    Rg16Float,
    Bc7Srgb,
    Bc7Unorm,
    Bc5Unorm,
    Depth32Float,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureColorPolicy {
    #[default]
    Srgb,
    LinearScene,
    LinearData,
    HdrScene,
}

/// Material prep record: standard material table entry + texture references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialAssetPrepRecord {
    pub material: RenderMaterialAssetId,
    pub state: RendererAssetLifecycleState,
    pub feature_mask: u32,
    pub texture_reference_count: u8,
    pub material_buffer_bytes: u64,
    pub last_failure_reason: Option<AssetPrepFailureReason>,
    pub last_used_frame: u64,
    pub evictable_after_fence_value: u64,
}

impl MaterialAssetPrepRecord {
    #[must_use]
    pub const fn new(material: RenderMaterialAssetId) -> Self {
        Self {
            material,
            state: RendererAssetLifecycleState::Unprepared,
            feature_mask: 0,
            texture_reference_count: 0,
            material_buffer_bytes: 0,
            last_failure_reason: None,
            last_used_frame: 0,
            evictable_after_fence_value: 0,
        }
    }
}

/// Shader prep record: schema, reflection, translation cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderAssetPrepRecord {
    pub shader_id: u64,
    pub state: RendererAssetLifecycleState,
    pub schema_version: u16,
    pub reflection_signature: u64,
    pub translation_cache_key: u64,
    pub last_failure_reason: Option<AssetPrepFailureReason>,
}

impl ShaderAssetPrepRecord {
    #[must_use]
    pub const fn new(shader_id: u64) -> Self {
        Self {
            shader_id,
            state: RendererAssetLifecycleState::Unprepared,
            schema_version: ASSET_PREP_SCHEMA_VERSION,
            reflection_signature: 0,
            translation_cache_key: 0,
            last_failure_reason: None,
        }
    }
}

/// Pipeline prep record: warmup variant tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineAssetPrepRecord {
    pub pipeline_id: u64,
    pub state: RendererAssetLifecycleState,
    pub variant_count: u16,
    pub warmup_completed: bool,
    pub last_failure_reason: Option<AssetPrepFailureReason>,
}

impl PipelineAssetPrepRecord {
    #[must_use]
    pub const fn new(pipeline_id: u64) -> Self {
        Self {
            pipeline_id,
            state: RendererAssetLifecycleState::Unprepared,
            variant_count: 0,
            warmup_completed: false,
            last_failure_reason: None,
        }
    }
}

/// Fallback assets used while real assets are not yet `GpuResident`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetFallbackChain {
    pub schema_version: u16,
    pub fallback_mesh: RenderMeshAssetId,
    pub fallback_material: RenderMaterialAssetId,
    pub fallback_texture: RenderTextureAssetId,
}

impl AssetFallbackChain {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: ASSET_PREP_SCHEMA_VERSION,
        fallback_mesh: RenderMeshAssetId::INVALID,
        fallback_material: RenderMaterialAssetId::INVALID,
        fallback_texture: RenderTextureAssetId::INVALID,
    };

    #[must_use]
    pub const fn with_mesh(mut self, mesh: RenderMeshAssetId) -> Self {
        self.fallback_mesh = mesh;
        self
    }

    #[must_use]
    pub const fn with_material(mut self, material: RenderMaterialAssetId) -> Self {
        self.fallback_material = material;
        self
    }

    #[must_use]
    pub const fn with_texture(mut self, texture: RenderTextureAssetId) -> Self {
        self.fallback_texture = texture;
        self
    }
}

impl Default for AssetFallbackChain {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetFallbackReason {
    #[default]
    NotApplicable,
    NotYetCpuPrepared,
    NotYetUploadQueued,
    NotYetGpuResident,
    Evicted,
    Failed,
}

impl AssetFallbackReason {
    #[must_use]
    pub const fn from_state(state: RendererAssetLifecycleState) -> Self {
        match state {
            RendererAssetLifecycleState::Unprepared => Self::NotYetCpuPrepared,
            RendererAssetLifecycleState::CpuPrepared => Self::NotYetUploadQueued,
            RendererAssetLifecycleState::UploadQueued => Self::NotYetGpuResident,
            RendererAssetLifecycleState::Evicted => Self::Evicted,
            RendererAssetLifecycleState::Failed => Self::Failed,
            RendererAssetLifecycleState::GpuResident | RendererAssetLifecycleState::Evictable => {
                Self::NotApplicable
            }
        }
    }

    #[must_use]
    pub const fn requires_fallback(self) -> bool {
        !matches!(self, Self::NotApplicable)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "not_applicable",
            Self::NotYetCpuPrepared => "not_yet_cpu_prepared",
            Self::NotYetUploadQueued => "not_yet_upload_queued",
            Self::NotYetGpuResident => "not_yet_gpu_resident",
            Self::Evicted => "evicted",
            Self::Failed => "failed",
        }
    }
}

/// Upload scheduler that batches uploads before bridge submission.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct AssetUploadScheduler {
    schema_version: u16,
    budget_bytes_per_frame: u64,
    queued_mesh_bytes: u64,
    queued_texture_bytes: u64,
    queued_material_bytes: u64,
    queued_other_bytes: u64,
    queued_uploads: Vec<AssetUploadEntry>,
    submitted_batches: u64,
    submitted_bytes: u64,
    deferred_uploads_due_to_budget: u64,
}

impl Default for AssetUploadScheduler {
    fn default() -> Self {
        Self::with_budget(ASSET_UPLOAD_SCHEDULER_DEFAULT_BUDGET_BYTES)
    }
}

impl AssetUploadScheduler {
    #[must_use]
    pub const fn with_budget(budget_bytes_per_frame: u64) -> Self {
        Self {
            schema_version: ASSET_PREP_SCHEMA_VERSION,
            budget_bytes_per_frame,
            queued_mesh_bytes: 0,
            queued_texture_bytes: 0,
            queued_material_bytes: 0,
            queued_other_bytes: 0,
            queued_uploads: Vec::new(),
            submitted_batches: 0,
            submitted_bytes: 0,
            deferred_uploads_due_to_budget: 0,
        }
    }

    pub fn enqueue(&mut self, entry: AssetUploadEntry) -> AssetUploadEnqueueResult {
        let projected = self.queued_total_bytes().saturating_add(entry.byte_count);
        if projected > self.budget_bytes_per_frame {
            self.deferred_uploads_due_to_budget =
                self.deferred_uploads_due_to_budget.saturating_add(1);
            return AssetUploadEnqueueResult::DeferredDueToBudget {
                budget: self.budget_bytes_per_frame,
                projected_bytes: projected,
            };
        }
        match entry.kind {
            RendererAssetKind::Mesh => {
                self.queued_mesh_bytes = self.queued_mesh_bytes.saturating_add(entry.byte_count);
            }
            RendererAssetKind::Texture => {
                self.queued_texture_bytes =
                    self.queued_texture_bytes.saturating_add(entry.byte_count);
            }
            RendererAssetKind::Material => {
                self.queued_material_bytes =
                    self.queued_material_bytes.saturating_add(entry.byte_count);
            }
            RendererAssetKind::Shader | RendererAssetKind::Pipeline => {
                self.queued_other_bytes = self.queued_other_bytes.saturating_add(entry.byte_count);
            }
        }
        self.queued_uploads.push(entry);
        AssetUploadEnqueueResult::Queued {
            queued_total_bytes: self.queued_total_bytes(),
        }
    }

    pub fn drain_for_submission(&mut self) -> AssetUploadBatch {
        let entries = std::mem::take(&mut self.queued_uploads);
        let mesh_bytes = self.queued_mesh_bytes;
        let texture_bytes = self.queued_texture_bytes;
        let material_bytes = self.queued_material_bytes;
        let other_bytes = self.queued_other_bytes;
        let total = mesh_bytes
            .saturating_add(texture_bytes)
            .saturating_add(material_bytes)
            .saturating_add(other_bytes);
        self.queued_mesh_bytes = 0;
        self.queued_texture_bytes = 0;
        self.queued_material_bytes = 0;
        self.queued_other_bytes = 0;
        self.submitted_batches = self.submitted_batches.saturating_add(1);
        self.submitted_bytes = self.submitted_bytes.saturating_add(total);
        AssetUploadBatch {
            schema_version: ASSET_PREP_SCHEMA_VERSION,
            entries,
            mesh_bytes,
            texture_bytes,
            material_bytes,
            other_bytes,
        }
    }

    #[must_use]
    pub const fn budget_bytes_per_frame(&self) -> u64 {
        self.budget_bytes_per_frame
    }

    #[must_use]
    pub fn queued_total_bytes(&self) -> u64 {
        self.queued_mesh_bytes
            .saturating_add(self.queued_texture_bytes)
            .saturating_add(self.queued_material_bytes)
            .saturating_add(self.queued_other_bytes)
    }

    #[must_use]
    pub const fn submitted_batches(&self) -> u64 {
        self.submitted_batches
    }

    #[must_use]
    pub const fn submitted_bytes(&self) -> u64 {
        self.submitted_bytes
    }

    #[must_use]
    pub const fn deferred_uploads_due_to_budget(&self) -> u64 {
        self.deferred_uploads_due_to_budget
    }

    #[must_use]
    pub fn queued_byte_attribution(&self) -> AssetUploadByteAttribution {
        AssetUploadByteAttribution {
            mesh_bytes: self.queued_mesh_bytes,
            texture_bytes: self.queued_texture_bytes,
            material_bytes: self.queued_material_bytes,
            other_bytes: self.queued_other_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetUploadEntry {
    pub kind: RendererAssetKind,
    pub asset_logical_id: u64,
    pub byte_count: u64,
    pub frame_index: u64,
    pub callback_timestamp_ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetUploadEnqueueResult {
    Queued { queued_total_bytes: u64 },
    DeferredDueToBudget { budget: u64, projected_bytes: u64 },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AssetUploadBatch {
    pub schema_version: u16,
    pub entries: Vec<AssetUploadEntry>,
    pub mesh_bytes: u64,
    pub texture_bytes: u64,
    pub material_bytes: u64,
    pub other_bytes: u64,
}

impl AssetUploadBatch {
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.mesh_bytes
            .saturating_add(self.texture_bytes)
            .saturating_add(self.material_bytes)
            .saturating_add(self.other_bytes)
    }

    #[must_use]
    pub fn entry_count(&self) -> u32 {
        u32::try_from(self.entries.len()).unwrap_or(u32::MAX)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetUploadByteAttribution {
    pub mesh_bytes: u64,
    pub texture_bytes: u64,
    pub material_bytes: u64,
    pub other_bytes: u64,
}

/// Residency telemetry: resident bytes by kind, upload bytes/latency,
/// evictions, fallback counts.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct RendererResidencyTelemetry {
    pub schema_version: u16,
    pub resident_mesh_bytes: u64,
    pub resident_texture_bytes: u64,
    pub resident_material_bytes: u64,
    pub upload_bytes: u64,
    pub upload_batch_count: u64,
    pub upload_latency_total_ns: u64,
    pub upload_latency_max_ns: u64,
    pub evictions: u64,
    pub fallback_mesh_count: u64,
    pub fallback_material_count: u64,
    pub fallback_texture_count: u64,
    pub asset_failure_count: u64,
}

impl RendererResidencyTelemetry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: ASSET_PREP_SCHEMA_VERSION,
            resident_mesh_bytes: 0,
            resident_texture_bytes: 0,
            resident_material_bytes: 0,
            upload_bytes: 0,
            upload_batch_count: 0,
            upload_latency_total_ns: 0,
            upload_latency_max_ns: 0,
            evictions: 0,
            fallback_mesh_count: 0,
            fallback_material_count: 0,
            fallback_texture_count: 0,
            asset_failure_count: 0,
        }
    }

    pub fn record_upload_batch(&mut self, batch: &AssetUploadBatch, latency_ns: u64) {
        self.upload_batch_count = self.upload_batch_count.saturating_add(1);
        self.upload_bytes = self.upload_bytes.saturating_add(batch.total_bytes());
        self.upload_latency_total_ns = self.upload_latency_total_ns.saturating_add(latency_ns);
        if latency_ns > self.upload_latency_max_ns {
            self.upload_latency_max_ns = latency_ns;
        }
    }

    pub fn record_eviction(&mut self) {
        self.evictions = self.evictions.saturating_add(1);
    }

    pub fn record_fallback(&mut self, kind: RendererAssetKind) {
        match kind {
            RendererAssetKind::Mesh => {
                self.fallback_mesh_count = self.fallback_mesh_count.saturating_add(1);
            }
            RendererAssetKind::Material => {
                self.fallback_material_count = self.fallback_material_count.saturating_add(1);
            }
            RendererAssetKind::Texture => {
                self.fallback_texture_count = self.fallback_texture_count.saturating_add(1);
            }
            RendererAssetKind::Shader | RendererAssetKind::Pipeline => {}
        }
    }

    pub fn record_failure(&mut self) {
        self.asset_failure_count = self.asset_failure_count.saturating_add(1);
    }

    pub fn record_resident_bytes(&mut self, kind: RendererAssetKind, bytes: u64) {
        match kind {
            RendererAssetKind::Mesh => self.resident_mesh_bytes = bytes,
            RendererAssetKind::Texture => self.resident_texture_bytes = bytes,
            RendererAssetKind::Material => self.resident_material_bytes = bytes,
            RendererAssetKind::Shader | RendererAssetKind::Pipeline => {}
        }
    }
}

/// Renderer asset registry: holds per-asset prep records keyed by logical
/// asset id, plus the upload scheduler and telemetry.
#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct RendererAssetRegistry {
    schema_version: u16,
    meshes: BTreeMap<u64, MeshAssetPrepRecord>,
    textures: BTreeMap<u64, TextureAssetPrepRecord>,
    materials: BTreeMap<u64, MaterialAssetPrepRecord>,
    shaders: BTreeMap<u64, ShaderAssetPrepRecord>,
    pipelines: BTreeMap<u64, PipelineAssetPrepRecord>,
    fallback_chain: AssetFallbackChain,
}

impl RendererAssetRegistry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            schema_version: ASSET_PREP_SCHEMA_VERSION,
            meshes: BTreeMap::new(),
            textures: BTreeMap::new(),
            materials: BTreeMap::new(),
            shaders: BTreeMap::new(),
            pipelines: BTreeMap::new(),
            fallback_chain: AssetFallbackChain::PRODUCT_DEFAULT,
        }
    }

    pub fn register_mesh(&mut self, mesh: RenderMeshAssetId) -> &mut MeshAssetPrepRecord {
        let key = asset_key_for_mesh(mesh);
        self.meshes
            .entry(key)
            .or_insert_with(|| MeshAssetPrepRecord::new(mesh))
    }

    pub fn register_texture(
        &mut self,
        texture: RenderTextureAssetId,
    ) -> &mut TextureAssetPrepRecord {
        let key = asset_key_for_texture(texture);
        self.textures
            .entry(key)
            .or_insert_with(|| TextureAssetPrepRecord::new(texture))
    }

    pub fn register_material(
        &mut self,
        material: RenderMaterialAssetId,
    ) -> &mut MaterialAssetPrepRecord {
        let key = asset_key_for_material(material);
        self.materials
            .entry(key)
            .or_insert_with(|| MaterialAssetPrepRecord::new(material))
    }

    pub fn register_shader(&mut self, shader_id: u64) -> &mut ShaderAssetPrepRecord {
        self.shaders
            .entry(shader_id)
            .or_insert_with(|| ShaderAssetPrepRecord::new(shader_id))
    }

    pub fn register_pipeline(&mut self, pipeline_id: u64) -> &mut PipelineAssetPrepRecord {
        self.pipelines
            .entry(pipeline_id)
            .or_insert_with(|| PipelineAssetPrepRecord::new(pipeline_id))
    }

    pub fn apply_transition(
        &mut self,
        target: RendererAssetTarget,
        transition: RendererAssetLifecycleTransition,
    ) -> RendererAssetLifecycleState {
        match target {
            RendererAssetTarget::Mesh(mesh) => {
                let record = self.register_mesh(mesh);
                record.state = transition.target_state(record.state);
                if let RendererAssetLifecycleTransition::Failed { reason } = transition {
                    record.last_failure_reason = Some(reason);
                }
                record.state
            }
            RendererAssetTarget::Texture(texture) => {
                let record = self.register_texture(texture);
                record.state = transition.target_state(record.state);
                if let RendererAssetLifecycleTransition::Failed { reason } = transition {
                    record.last_failure_reason = Some(reason);
                }
                record.state
            }
            RendererAssetTarget::Material(material) => {
                let record = self.register_material(material);
                record.state = transition.target_state(record.state);
                if let RendererAssetLifecycleTransition::Failed { reason } = transition {
                    record.last_failure_reason = Some(reason);
                }
                record.state
            }
            RendererAssetTarget::Shader(id) => {
                let record = self.register_shader(id);
                record.state = transition.target_state(record.state);
                if let RendererAssetLifecycleTransition::Failed { reason } = transition {
                    record.last_failure_reason = Some(reason);
                }
                record.state
            }
            RendererAssetTarget::Pipeline(id) => {
                let record = self.register_pipeline(id);
                record.state = transition.target_state(record.state);
                if let RendererAssetLifecycleTransition::Failed { reason } = transition {
                    record.last_failure_reason = Some(reason);
                }
                record.state
            }
        }
    }

    /// Drives lifecycle transitions from `RendererAssetEvents` (changed
    /// meshes/materials/textures). Each changed asset is moved back to
    /// `Unprepared` so the renderer's prep system picks it up.
    pub fn ingest_asset_events(&mut self, events: &RendererAssetEvents) -> u32 {
        let mut transition_count = 0u32;
        for mesh in &events.changed_meshes {
            self.apply_transition(
                RendererAssetTarget::Mesh(*mesh),
                RendererAssetLifecycleTransition::AssetChangedEvent,
            );
            transition_count = transition_count.saturating_add(1);
        }
        for material in &events.changed_materials {
            self.apply_transition(
                RendererAssetTarget::Material(*material),
                RendererAssetLifecycleTransition::AssetChangedEvent,
            );
            transition_count = transition_count.saturating_add(1);
        }
        for texture in &events.changed_textures {
            self.apply_transition(
                RendererAssetTarget::Texture(*texture),
                RendererAssetLifecycleTransition::AssetChangedEvent,
            );
            transition_count = transition_count.saturating_add(1);
        }
        transition_count
    }

    #[must_use]
    pub fn mesh(&self, mesh: RenderMeshAssetId) -> Option<MeshAssetPrepRecord> {
        self.meshes.get(&asset_key_for_mesh(mesh)).copied()
    }

    #[must_use]
    pub fn texture(&self, texture: RenderTextureAssetId) -> Option<TextureAssetPrepRecord> {
        self.textures.get(&asset_key_for_texture(texture)).copied()
    }

    #[must_use]
    pub fn material(&self, material: RenderMaterialAssetId) -> Option<MaterialAssetPrepRecord> {
        self.materials
            .get(&asset_key_for_material(material))
            .copied()
    }

    #[must_use]
    pub fn shader(&self, id: u64) -> Option<ShaderAssetPrepRecord> {
        self.shaders.get(&id).copied()
    }

    #[must_use]
    pub fn pipeline(&self, id: u64) -> Option<PipelineAssetPrepRecord> {
        self.pipelines.get(&id).copied()
    }

    #[must_use]
    pub fn fallback_chain(&self) -> AssetFallbackChain {
        self.fallback_chain
    }

    pub fn set_fallback_chain(&mut self, chain: AssetFallbackChain) {
        self.fallback_chain = chain;
    }

    /// Resolves the actual asset to draw with. Returns the requested asset
    /// when `GpuResident` / `Evictable`, otherwise the fallback. The
    /// reason field is populated so a missing asset is visible in
    /// telemetry rather than silently presenting a black draw.
    #[must_use]
    pub fn resolve_mesh(
        &self,
        mesh: RenderMeshAssetId,
    ) -> RendererAssetResolution<RenderMeshAssetId> {
        let state = self
            .mesh(mesh)
            .map(|record| record.state)
            .unwrap_or(RendererAssetLifecycleState::Unprepared);
        let reason = AssetFallbackReason::from_state(state);
        if reason.requires_fallback() {
            RendererAssetResolution::Fallback {
                requested: mesh,
                fallback: self.fallback_chain.fallback_mesh,
                reason,
            }
        } else {
            RendererAssetResolution::Hit { resolved: mesh }
        }
    }

    #[must_use]
    pub fn resolve_texture(
        &self,
        texture: RenderTextureAssetId,
    ) -> RendererAssetResolution<RenderTextureAssetId> {
        let state = self
            .texture(texture)
            .map(|record| record.state)
            .unwrap_or(RendererAssetLifecycleState::Unprepared);
        let reason = AssetFallbackReason::from_state(state);
        if reason.requires_fallback() {
            RendererAssetResolution::Fallback {
                requested: texture,
                fallback: self.fallback_chain.fallback_texture,
                reason,
            }
        } else {
            RendererAssetResolution::Hit { resolved: texture }
        }
    }

    #[must_use]
    pub fn resolve_material(
        &self,
        material: RenderMaterialAssetId,
    ) -> RendererAssetResolution<RenderMaterialAssetId> {
        let state = self
            .material(material)
            .map(|record| record.state)
            .unwrap_or(RendererAssetLifecycleState::Unprepared);
        let reason = AssetFallbackReason::from_state(state);
        if reason.requires_fallback() {
            RendererAssetResolution::Fallback {
                requested: material,
                fallback: self.fallback_chain.fallback_material,
                reason,
            }
        } else {
            RendererAssetResolution::Hit { resolved: material }
        }
    }

    #[must_use]
    pub fn resident_mesh_bytes(&self) -> u64 {
        self.meshes
            .values()
            .filter(|record| record.state.is_resident())
            .map(|record| record.total_bytes())
            .sum()
    }

    #[must_use]
    pub fn resident_texture_bytes(&self) -> u64 {
        self.textures
            .values()
            .filter(|record| record.state.is_resident())
            .map(|record| record.mip_bytes)
            .sum()
    }

    #[must_use]
    pub fn resident_material_bytes(&self) -> u64 {
        self.materials
            .values()
            .filter(|record| record.state.is_resident())
            .map(|record| record.material_buffer_bytes)
            .sum()
    }

    pub fn refresh_residency_telemetry(&self, telemetry: &mut RendererResidencyTelemetry) {
        telemetry.resident_mesh_bytes = self.resident_mesh_bytes();
        telemetry.resident_texture_bytes = self.resident_texture_bytes();
        telemetry.resident_material_bytes = self.resident_material_bytes();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererAssetTarget {
    Mesh(RenderMeshAssetId),
    Texture(RenderTextureAssetId),
    Material(RenderMaterialAssetId),
    Shader(u64),
    Pipeline(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererAssetResolution<T: Copy> {
    Hit {
        resolved: T,
    },
    Fallback {
        requested: T,
        fallback: T,
        reason: AssetFallbackReason,
    },
}

impl<T: Copy + Eq> RendererAssetResolution<T> {
    #[must_use]
    pub const fn is_hit(&self) -> bool {
        matches!(self, Self::Hit { .. })
    }

    #[must_use]
    pub fn fallback_reason(&self) -> Option<AssetFallbackReason> {
        match self {
            Self::Hit { .. } => None,
            Self::Fallback { reason, .. } => Some(*reason),
        }
    }

    #[must_use]
    pub fn resolved_id(&self) -> T {
        match self {
            Self::Hit { resolved } => *resolved,
            Self::Fallback { fallback, .. } => *fallback,
        }
    }
}

#[must_use]
const fn asset_key_for_mesh(mesh: RenderMeshAssetId) -> u64 {
    pack_asset_key(mesh.slot, mesh.generation)
}

#[must_use]
const fn asset_key_for_texture(texture: RenderTextureAssetId) -> u64 {
    pack_asset_key(texture.slot, texture.generation)
}

#[must_use]
const fn asset_key_for_material(material: RenderMaterialAssetId) -> u64 {
    pack_asset_key(material.slot, material.generation)
}

#[must_use]
const fn pack_asset_key(slot: u32, generation: u32) -> u64 {
    ((slot as u64) << 32) | (generation as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh_id(slot: u32) -> RenderMeshAssetId {
        RenderMeshAssetId::first(slot)
    }

    fn texture_id(slot: u32) -> RenderTextureAssetId {
        RenderTextureAssetId::first(slot)
    }

    fn material_id(slot: u32) -> RenderMaterialAssetId {
        RenderMaterialAssetId::first(slot)
    }

    #[test]
    fn lifecycle_state_transitions_walk_through_unprepared_to_resident() {
        let mut registry = RendererAssetRegistry::new();
        let target = RendererAssetTarget::Mesh(mesh_id(7));

        let s0 =
            registry.apply_transition(target, RendererAssetLifecycleTransition::AssetChangedEvent);
        assert_eq!(s0, RendererAssetLifecycleState::Unprepared);

        let s1 =
            registry.apply_transition(target, RendererAssetLifecycleTransition::CpuPrepCompleted);
        assert_eq!(s1, RendererAssetLifecycleState::CpuPrepared);

        let s2 =
            registry.apply_transition(target, RendererAssetLifecycleTransition::UploadEnqueued);
        assert_eq!(s2, RendererAssetLifecycleState::UploadQueued);

        let s3 =
            registry.apply_transition(target, RendererAssetLifecycleTransition::UploadCompleted);
        assert_eq!(s3, RendererAssetLifecycleState::GpuResident);

        let s4 = registry.apply_transition(
            target,
            RendererAssetLifecycleTransition::EvictionPolicyMarkedEvictable,
        );
        assert_eq!(s4, RendererAssetLifecycleState::Evictable);

        let s5 = registry.apply_transition(target, RendererAssetLifecycleTransition::Evicted);
        assert_eq!(s5, RendererAssetLifecycleState::Evicted);

        let s6 = registry.apply_transition(
            target,
            RendererAssetLifecycleTransition::ResurrectAfterEviction,
        );
        assert_eq!(s6, RendererAssetLifecycleState::Unprepared);
    }

    #[test]
    fn failed_transition_records_typed_failure_reason() {
        let mut registry = RendererAssetRegistry::new();
        let mesh = mesh_id(11);
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh),
            RendererAssetLifecycleTransition::Failed {
                reason: AssetPrepFailureReason::SourceCorrupt,
            },
        );
        let record = registry.mesh(mesh).expect("mesh record");
        assert_eq!(record.state, RendererAssetLifecycleState::Failed);
        assert_eq!(
            record.last_failure_reason,
            Some(AssetPrepFailureReason::SourceCorrupt)
        );
    }

    #[test]
    fn renderer_asset_events_drive_lifecycle_transitions_back_to_unprepared() {
        let mut registry = RendererAssetRegistry::new();
        // Pre-populate one mesh as GpuResident so we can observe the
        // transition back to Unprepared after the asset event arrives.
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh_id(3)),
            RendererAssetLifecycleTransition::CpuPrepCompleted,
        );
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh_id(3)),
            RendererAssetLifecycleTransition::UploadEnqueued,
        );
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh_id(3)),
            RendererAssetLifecycleTransition::UploadCompleted,
        );
        assert!(registry.mesh(mesh_id(3)).unwrap().state.is_resident());

        let events = RendererAssetEvents {
            changed_meshes: vec![mesh_id(3)],
            changed_materials: vec![material_id(4)],
            changed_textures: vec![texture_id(5)],
        };
        let count = registry.ingest_asset_events(&events);
        assert_eq!(count, 3);

        for (kind, state) in [
            ("mesh", registry.mesh(mesh_id(3)).unwrap().state),
            ("material", registry.material(material_id(4)).unwrap().state),
            ("texture", registry.texture(texture_id(5)).unwrap().state),
        ] {
            assert_eq!(
                state,
                RendererAssetLifecycleState::Unprepared,
                "{kind} should be back in Unprepared after asset event"
            );
        }
    }

    #[test]
    fn resolve_mesh_returns_fallback_when_not_resident() {
        let mut registry = RendererAssetRegistry::new();
        registry.set_fallback_chain(AssetFallbackChain::PRODUCT_DEFAULT.with_mesh(mesh_id(99)));
        let resolution = registry.resolve_mesh(mesh_id(7));
        assert!(!resolution.is_hit());
        assert_eq!(
            resolution.fallback_reason(),
            Some(AssetFallbackReason::NotYetCpuPrepared)
        );
        assert_eq!(resolution.resolved_id(), mesh_id(99));

        // Promote to resident, then resolution becomes a hit.
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh_id(7)),
            RendererAssetLifecycleTransition::CpuPrepCompleted,
        );
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh_id(7)),
            RendererAssetLifecycleTransition::UploadEnqueued,
        );
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh_id(7)),
            RendererAssetLifecycleTransition::UploadCompleted,
        );
        let resolution = registry.resolve_mesh(mesh_id(7));
        assert!(resolution.is_hit());
        assert_eq!(resolution.resolved_id(), mesh_id(7));
    }

    #[test]
    fn upload_scheduler_batches_uploads_and_attributes_bytes_by_kind() {
        let mut scheduler = AssetUploadScheduler::with_budget(1024);
        for i in 0..3 {
            assert!(matches!(
                scheduler.enqueue(AssetUploadEntry {
                    kind: RendererAssetKind::Mesh,
                    asset_logical_id: i,
                    byte_count: 100,
                    frame_index: 1,
                    callback_timestamp_ns: 1_000,
                }),
                AssetUploadEnqueueResult::Queued { .. }
            ));
        }
        scheduler.enqueue(AssetUploadEntry {
            kind: RendererAssetKind::Texture,
            asset_logical_id: 11,
            byte_count: 256,
            frame_index: 1,
            callback_timestamp_ns: 1_500,
        });
        scheduler.enqueue(AssetUploadEntry {
            kind: RendererAssetKind::Material,
            asset_logical_id: 21,
            byte_count: 32,
            frame_index: 1,
            callback_timestamp_ns: 2_000,
        });

        let attribution = scheduler.queued_byte_attribution();
        assert_eq!(attribution.mesh_bytes, 300);
        assert_eq!(attribution.texture_bytes, 256);
        assert_eq!(attribution.material_bytes, 32);
        assert_eq!(scheduler.queued_total_bytes(), 588);

        let batch = scheduler.drain_for_submission();
        assert_eq!(batch.entry_count(), 5);
        assert_eq!(batch.total_bytes(), 588);
        assert_eq!(scheduler.queued_total_bytes(), 0);
        assert_eq!(scheduler.submitted_batches(), 1);
        assert_eq!(scheduler.submitted_bytes(), 588);
    }

    #[test]
    fn upload_scheduler_defers_uploads_when_budget_would_be_exceeded() {
        let mut scheduler = AssetUploadScheduler::with_budget(100);
        let _ = scheduler.enqueue(AssetUploadEntry {
            kind: RendererAssetKind::Mesh,
            asset_logical_id: 1,
            byte_count: 80,
            frame_index: 1,
            callback_timestamp_ns: 0,
        });
        let result = scheduler.enqueue(AssetUploadEntry {
            kind: RendererAssetKind::Mesh,
            asset_logical_id: 2,
            byte_count: 50,
            frame_index: 1,
            callback_timestamp_ns: 100,
        });
        assert!(matches!(
            result,
            AssetUploadEnqueueResult::DeferredDueToBudget { .. }
        ));
        assert_eq!(scheduler.deferred_uploads_due_to_budget(), 1);
        assert_eq!(scheduler.queued_total_bytes(), 80);
    }

    #[test]
    fn residency_telemetry_records_uploads_evictions_and_fallbacks_by_kind() {
        let mut telemetry = RendererResidencyTelemetry::new();
        let batch = AssetUploadBatch {
            schema_version: ASSET_PREP_SCHEMA_VERSION,
            entries: Vec::new(),
            mesh_bytes: 1024,
            texture_bytes: 2048,
            material_bytes: 32,
            other_bytes: 16,
        };
        telemetry.record_upload_batch(&batch, 250_000);
        telemetry.record_upload_batch(&batch, 120_000);
        telemetry.record_eviction();
        telemetry.record_fallback(RendererAssetKind::Mesh);
        telemetry.record_fallback(RendererAssetKind::Texture);
        telemetry.record_failure();

        assert_eq!(telemetry.upload_batch_count, 2);
        assert_eq!(telemetry.upload_bytes, 6240);
        assert_eq!(telemetry.upload_latency_total_ns, 370_000);
        assert_eq!(telemetry.upload_latency_max_ns, 250_000);
        assert_eq!(telemetry.evictions, 1);
        assert_eq!(telemetry.fallback_mesh_count, 1);
        assert_eq!(telemetry.fallback_texture_count, 1);
        assert_eq!(telemetry.asset_failure_count, 1);
    }

    #[test]
    fn refresh_residency_telemetry_pulls_resident_bytes_by_kind_from_registry() {
        let mut registry = RendererAssetRegistry::new();
        let mesh = mesh_id(1);
        let record = registry.register_mesh(mesh);
        record.vertex_buffer_bytes = 4_096;
        record.index_buffer_bytes = 2_048;
        record.meshlet_metadata_bytes = 256;

        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh),
            RendererAssetLifecycleTransition::CpuPrepCompleted,
        );
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh),
            RendererAssetLifecycleTransition::UploadEnqueued,
        );
        registry.apply_transition(
            RendererAssetTarget::Mesh(mesh),
            RendererAssetLifecycleTransition::UploadCompleted,
        );

        let mut telemetry = RendererResidencyTelemetry::new();
        registry.refresh_residency_telemetry(&mut telemetry);
        assert_eq!(telemetry.resident_mesh_bytes, 6_400);
    }

    #[test]
    fn fallback_reason_is_not_applicable_when_state_is_resident_or_evictable() {
        for state in [
            RendererAssetLifecycleState::GpuResident,
            RendererAssetLifecycleState::Evictable,
        ] {
            assert_eq!(
                AssetFallbackReason::from_state(state),
                AssetFallbackReason::NotApplicable
            );
            assert!(!AssetFallbackReason::from_state(state).requires_fallback());
        }
    }

    #[test]
    fn fallback_reason_is_required_for_every_non_resident_state() {
        for state in [
            RendererAssetLifecycleState::Unprepared,
            RendererAssetLifecycleState::CpuPrepared,
            RendererAssetLifecycleState::UploadQueued,
            RendererAssetLifecycleState::Evicted,
            RendererAssetLifecycleState::Failed,
        ] {
            let reason = AssetFallbackReason::from_state(state);
            assert!(reason.requires_fallback(), "{}", state.as_str());
        }
    }

    #[test]
    fn lifecycle_state_helpers_classify_resident_failed_and_fallback_correctly() {
        assert!(RendererAssetLifecycleState::GpuResident.is_resident());
        assert!(RendererAssetLifecycleState::Evictable.is_resident());
        assert!(!RendererAssetLifecycleState::Unprepared.is_resident());
        assert!(RendererAssetLifecycleState::Failed.is_failed());
        assert!(RendererAssetLifecycleState::Unprepared.requires_fallback());
        assert!(!RendererAssetLifecycleState::GpuResident.requires_fallback());
    }

    #[test]
    fn shader_and_pipeline_records_track_state_independently_of_meshes() {
        let mut registry = RendererAssetRegistry::new();
        registry.apply_transition(
            RendererAssetTarget::Shader(0xABCD),
            RendererAssetLifecycleTransition::CpuPrepCompleted,
        );
        registry.apply_transition(
            RendererAssetTarget::Pipeline(0xBEEF),
            RendererAssetLifecycleTransition::UploadCompleted,
        );

        let shader = registry.shader(0xABCD).expect("shader");
        let pipeline = registry.pipeline(0xBEEF).expect("pipeline");
        assert_eq!(shader.state, RendererAssetLifecycleState::CpuPrepared);
        assert_eq!(pipeline.state, RendererAssetLifecycleState::GpuResident);
    }

    #[test]
    fn product_default_fallback_chain_is_invalid_until_explicitly_assigned() {
        let chain = AssetFallbackChain::PRODUCT_DEFAULT;
        assert!(!chain.fallback_mesh.is_valid());
        assert!(!chain.fallback_material.is_valid());
        assert!(!chain.fallback_texture.is_valid());

        let custom = AssetFallbackChain::PRODUCT_DEFAULT
            .with_mesh(mesh_id(1))
            .with_material(material_id(2))
            .with_texture(texture_id(3));
        assert!(custom.fallback_mesh.is_valid());
        assert!(custom.fallback_material.is_valid());
        assert!(custom.fallback_texture.is_valid());
    }
}
