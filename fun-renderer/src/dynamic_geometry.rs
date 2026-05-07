use std::collections::BTreeMap;

use fun_scene::{
    DynamicGeometryAuthoring, DynamicGeometryClass, DynamicGeometryLifetime,
    DynamicGeometryUpdateHint, GeometryDeclaration, GeometryRef, MaterialRef, PagePriorityHint,
    ProceduralChunkOwner, Renderable, SceneGeometryKind,
};

pub const DYNAMIC_GEOMETRY_SCHEMA_VERSION: u16 = 1;
pub const DYNAMIC_GEOMETRY_BENCHMARK_ARTIFACT_ENV: &str =
    "FUN_RENDERER_DYNAMIC_GEOMETRY_BENCHMARK_ARTIFACT";

pub type DynamicMeshHandle = GeometryRef;
pub type DynamicMaterialHandle = MaterialRef;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DynamicGeometrySubmissionId(pub u64);

impl DynamicGeometrySubmissionId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DynamicGeometryBounds {
    pub center: [f32; 3],
    pub radius: f32,
}

impl DynamicGeometryBounds {
    #[must_use]
    pub const fn new(center: [f32; 3], radius: f32) -> Self {
        Self { center, radius }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DynamicGeometryTransform {
    pub translation: [f32; 3],
    pub rotation_xyzw: [f32; 4],
    pub scale: [f32; 3],
}

impl DynamicGeometryTransform {
    pub const IDENTITY: Self = Self {
        translation: [0.0, 0.0, 0.0],
        rotation_xyzw: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
    };

    #[must_use]
    pub const fn translated(x: f32, y: f32, z: f32) -> Self {
        Self {
            translation: [x, y, z],
            ..Self::IDENTITY
        }
    }
}

impl Default for DynamicGeometryTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DynamicGeometryPayload {
    pub vertex_count: u32,
    pub index_count: u32,
    pub vertex_bytes: u64,
    pub index_bytes: u64,
    pub skinning_joint_count: u16,
    pub dynamic_cluster_budget: u16,
}

impl DynamicGeometryPayload {
    #[must_use]
    pub const fn new(
        vertex_count: u32,
        index_count: u32,
        vertex_bytes: u64,
        index_bytes: u64,
    ) -> Self {
        Self {
            vertex_count,
            index_count,
            vertex_bytes,
            index_bytes,
            skinning_joint_count: 0,
            dynamic_cluster_budget: 0,
        }
    }

    #[must_use]
    pub const fn with_skinning(mut self, joint_count: u16) -> Self {
        self.skinning_joint_count = joint_count;
        self
    }

    #[must_use]
    pub const fn with_dynamic_cluster_budget(mut self, cluster_budget: u16) -> Self {
        self.dynamic_cluster_budget = cluster_budget;
        self
    }

    #[must_use]
    pub const fn upload_bytes(self) -> u64 {
        self.vertex_bytes.saturating_add(self.index_bytes)
    }

    #[must_use]
    pub const fn triangle_count(self) -> u32 {
        self.index_count / 3
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicClusterMode {
    #[default]
    ClassicMeshOnly,
    GenerateDynamicClusters,
    ProvidedDynamicClusters,
}

impl DynamicClusterMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClassicMeshOnly => "classic_mesh_only",
            Self::GenerateDynamicClusters => "generate_dynamic_clusters",
            Self::ProvidedDynamicClusters => "provided_dynamic_clusters",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DynamicDrawPath {
    #[default]
    ClassicMeshDraw,
    DynamicClusterIndirect,
}

impl DynamicDrawPath {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClassicMeshDraw => "classic_mesh_draw",
            Self::DynamicClusterIndirect => "dynamic_cluster_indirect",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DynamicGeometryDirtyFlags(pub u32);

impl DynamicGeometryDirtyFlags {
    pub const NONE: Self = Self(0);
    pub const TRANSFORM: Self = Self(1 << 0);
    pub const VERTEX_DATA: Self = Self(1 << 1);
    pub const INDEX_DATA: Self = Self(1 << 2);
    pub const MATERIAL: Self = Self(1 << 3);
    pub const SKINNING: Self = Self(1 << 4);
    pub const PROCEDURAL: Self = Self(1 << 5);
    pub const LIFETIME: Self = Self(1 << 6);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicGeometrySubmission {
    pub id: DynamicGeometrySubmissionId,
    pub mesh: DynamicMeshHandle,
    pub material: DynamicMaterialHandle,
    pub payload: DynamicGeometryPayload,
    pub bounds: DynamicGeometryBounds,
    pub transform: DynamicGeometryTransform,
    pub previous_transform: DynamicGeometryTransform,
    pub update_reason: DynamicGeometryUpdateHint,
    pub lifetime: DynamicGeometryLifetime,
    pub class: DynamicGeometryClass,
    pub priority_hint: PagePriorityHint,
    pub procedural_owner: ProceduralChunkOwner,
    pub cluster_mode: DynamicClusterMode,
}

impl DynamicGeometrySubmission {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_scene_declaration(
        id: DynamicGeometrySubmissionId,
        renderable: &Renderable,
        geometry: Option<&GeometryDeclaration>,
        authoring: &DynamicGeometryAuthoring,
        bounds: DynamicGeometryBounds,
        transform: DynamicGeometryTransform,
        previous_transform: DynamicGeometryTransform,
        payload: DynamicGeometryPayload,
    ) -> Self {
        let declaration_owner = geometry
            .map(|declaration| declaration.procedural_owner)
            .unwrap_or(ProceduralChunkOwner::NONE);
        let procedural_owner = if authoring.procedural_owner.is_valid() {
            authoring.procedural_owner
        } else {
            declaration_owner
        };
        let cluster_mode = if authoring.allow_dynamic_clusters {
            DynamicClusterMode::GenerateDynamicClusters
        } else {
            DynamicClusterMode::ClassicMeshOnly
        };

        Self {
            id,
            mesh: renderable.geometry,
            material: renderable.material,
            payload,
            bounds,
            transform,
            previous_transform,
            update_reason: authoring.update_hint,
            lifetime: authoring.lifetime,
            class: authoring.class,
            priority_hint: authoring.priority_hint,
            procedural_owner,
            cluster_mode,
        }
    }

    #[must_use]
    pub fn from_renderable(
        id: DynamicGeometrySubmissionId,
        renderable: &Renderable,
        class: DynamicGeometryClass,
        update_reason: DynamicGeometryUpdateHint,
        payload: DynamicGeometryPayload,
    ) -> Self {
        Self {
            id,
            mesh: renderable.geometry,
            material: renderable.material,
            payload,
            class,
            update_reason,
            ..Self::default()
        }
    }
}

impl Default for DynamicGeometrySubmission {
    fn default() -> Self {
        Self {
            id: DynamicGeometrySubmissionId::INVALID,
            mesh: GeometryRef::INVALID,
            material: MaterialRef::INVALID,
            payload: DynamicGeometryPayload::default(),
            bounds: DynamicGeometryBounds::default(),
            transform: DynamicGeometryTransform::IDENTITY,
            previous_transform: DynamicGeometryTransform::IDENTITY,
            update_reason: DynamicGeometryUpdateHint::Unknown,
            lifetime: DynamicGeometryLifetime::RuntimeDynamic,
            class: DynamicGeometryClass::GenericDynamic,
            priority_hint: PagePriorityHint::Normal,
            procedural_owner: ProceduralChunkOwner::NONE,
            cluster_mode: DynamicClusterMode::ClassicMeshOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicGeometryRecord {
    pub submission: DynamicGeometrySubmission,
    pub dirty: DynamicGeometryDirtyFlags,
    pub generation: u32,
    pub last_update_frame: u64,
    pub removed: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DynamicGeometrySubmissionReport {
    pub created: bool,
    pub updated: bool,
    pub removed: bool,
    pub dirty: DynamicGeometryDirtyFlags,
    pub upload_bytes: u64,
    pub cpu_update_time_us: u64,
    pub gpu_update_time_us: u64,
    pub skinning_cost_us: u64,
    pub procedural_invalidation_cost_us: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralGeometryUpdate {
    pub owner: ProceduralChunkOwner,
    pub new_revision: u64,
    pub reason: DynamicGeometryUpdateHint,
    pub vertex_bytes_delta: u64,
    pub index_bytes_delta: u64,
}

impl ProceduralGeometryUpdate {
    #[must_use]
    pub const fn new(
        owner: ProceduralChunkOwner,
        new_revision: u64,
        vertex_bytes_delta: u64,
        index_bytes_delta: u64,
    ) -> Self {
        Self {
            owner,
            new_revision,
            reason: DynamicGeometryUpdateHint::ProceduralChunkEdited,
            vertex_bytes_delta,
            index_bytes_delta,
        }
    }

    #[must_use]
    pub const fn upload_bytes(self) -> u64 {
        self.vertex_bytes_delta
            .saturating_add(self.index_bytes_delta)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralGeometryUpdateReport {
    pub matched_records: u32,
    pub new_revision: u64,
    pub upload_bytes: u64,
    pub dirty_records: u32,
    pub full_static_virtual_geometry_rebuilds: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DynamicGeometryDiagnostics {
    pub schema_version: u16,
    pub frame_index: u64,
    pub live_records: u32,
    pub submitted_records: u32,
    pub updated_records: u32,
    pub removed_records: u32,
    pub dirty_records: u32,
    pub upload_bytes: u64,
    pub cpu_update_time_us: u64,
    pub gpu_update_time_us: u64,
    pub dynamic_draw_count: u32,
    pub dynamic_dispatch_count: u32,
    pub classic_draw_count: u32,
    pub dynamic_cluster_draw_count: u32,
    pub skinning_cost_us: u64,
    pub procedural_invalidation_cost_us: u64,
    pub procedural_invalidated_records: u32,
    pub destruction_fragment_count: u32,
    pub procedural_chunk_count: u32,
    pub temporary_record_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicGeometryDrawPacket {
    pub id: DynamicGeometrySubmissionId,
    pub draw_path: DynamicDrawPath,
    pub mesh: DynamicMeshHandle,
    pub material: DynamicMaterialHandle,
    pub vertex_count: u32,
    pub index_count: u32,
    pub dynamic_cluster_count: u16,
    pub priority_hint: PagePriorityHint,
    pub class: DynamicGeometryClass,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct DynamicGeometryFramePlan {
    pub packets: Vec<DynamicGeometryDrawPacket>,
    pub diagnostics: DynamicGeometryDiagnostics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicGeometryDebugArtifact {
    pub schema_version: u16,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DynamicGeometryRuntimePolicy {
    pub dynamic_clusters_enabled: bool,
    pub classic_mesh_draws_enabled: bool,
    pub destruction_fragments_use_dynamic_clusters: bool,
    pub procedural_chunks_use_dynamic_clusters: bool,
    pub vehicles_use_dynamic_clusters: bool,
}

impl Default for DynamicGeometryRuntimePolicy {
    fn default() -> Self {
        Self {
            dynamic_clusters_enabled: true,
            classic_mesh_draws_enabled: true,
            destruction_fragments_use_dynamic_clusters: true,
            procedural_chunks_use_dynamic_clusters: true,
            vehicles_use_dynamic_clusters: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicGeometryDatabase {
    frame_index: u64,
    records: BTreeMap<DynamicGeometrySubmissionId, DynamicGeometryRecord>,
    diagnostics: DynamicGeometryDiagnostics,
}

impl DynamicGeometryDatabase {
    #[must_use]
    pub fn new() -> Self {
        let diagnostics = DynamicGeometryDiagnostics {
            schema_version: DYNAMIC_GEOMETRY_SCHEMA_VERSION,
            ..DynamicGeometryDiagnostics::default()
        };
        Self {
            frame_index: 0,
            records: BTreeMap::new(),
            diagnostics,
        }
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        self.frame_index = frame_index;
        self.rebuild_live_diagnostics();
        self.diagnostics.frame_index = frame_index;
        self.diagnostics.submitted_records = 0;
        self.diagnostics.updated_records = 0;
        self.diagnostics.removed_records = 0;
        self.diagnostics.dirty_records = 0;
        self.diagnostics.upload_bytes = 0;
        self.diagnostics.cpu_update_time_us = 0;
        self.diagnostics.gpu_update_time_us = 0;
        self.diagnostics.dynamic_draw_count = 0;
        self.diagnostics.dynamic_dispatch_count = 0;
        self.diagnostics.classic_draw_count = 0;
        self.diagnostics.dynamic_cluster_draw_count = 0;
        self.diagnostics.skinning_cost_us = 0;
        self.diagnostics.procedural_invalidation_cost_us = 0;
        self.diagnostics.procedural_invalidated_records = 0;
    }

    #[must_use]
    pub fn record_count(&self) -> usize {
        self.records
            .values()
            .filter(|record| !record.removed)
            .count()
    }

    #[must_use]
    pub fn record(&self, id: DynamicGeometrySubmissionId) -> Option<&DynamicGeometryRecord> {
        self.records.get(&id).filter(|record| !record.removed)
    }

    #[must_use]
    pub const fn diagnostics(&self) -> DynamicGeometryDiagnostics {
        self.diagnostics
    }

    pub fn submit(
        &mut self,
        submission: DynamicGeometrySubmission,
    ) -> DynamicGeometrySubmissionReport {
        if !submission.id.is_valid() {
            return DynamicGeometrySubmissionReport::default();
        }

        let previous = self.records.get(&submission.id).cloned();
        let created = previous.as_ref().is_none_or(|record| record.removed);
        let mut dirty = dirty_flags_for_update(submission.update_reason);
        if created {
            dirty = dirty
                .union(DynamicGeometryDirtyFlags::VERTEX_DATA)
                .union(DynamicGeometryDirtyFlags::INDEX_DATA)
                .union(DynamicGeometryDirtyFlags::MATERIAL)
                .union(DynamicGeometryDirtyFlags::TRANSFORM);
        } else if let Some(previous) = previous.as_ref() {
            dirty = dirty.union(dirty_flags_from_record_delta(previous, &submission));
        }

        let upload_bytes = upload_bytes_for_dirty(dirty, submission.payload);
        let skinning_cost_us = skinning_cost_for(&submission, dirty);
        let procedural_invalidation_cost_us = procedural_cost_for(&submission, dirty);
        let cpu_update_time_us = cpu_cost_for(&submission, dirty);
        let gpu_update_time_us = gpu_cost_for(&submission, dirty);
        let generation = previous
            .as_ref()
            .map(|record| record.generation.saturating_add(1))
            .unwrap_or(1);

        self.records.insert(
            submission.id,
            DynamicGeometryRecord {
                submission,
                dirty,
                generation,
                last_update_frame: self.frame_index,
                removed: false,
            },
        );

        let report = DynamicGeometrySubmissionReport {
            created,
            updated: !created,
            removed: false,
            dirty,
            upload_bytes,
            cpu_update_time_us,
            gpu_update_time_us,
            skinning_cost_us,
            procedural_invalidation_cost_us,
        };
        self.record_submission_report(report);
        self.rebuild_live_diagnostics();
        report
    }

    pub fn remove(&mut self, id: DynamicGeometrySubmissionId) -> DynamicGeometrySubmissionReport {
        let Some(record) = self.records.get_mut(&id) else {
            return DynamicGeometrySubmissionReport::default();
        };
        if record.removed {
            return DynamicGeometrySubmissionReport::default();
        }
        record.removed = true;
        record.dirty = DynamicGeometryDirtyFlags::LIFETIME;
        record.generation = record.generation.saturating_add(1);
        record.last_update_frame = self.frame_index;

        let report = DynamicGeometrySubmissionReport {
            removed: true,
            dirty: DynamicGeometryDirtyFlags::LIFETIME,
            cpu_update_time_us: 1,
            ..Default::default()
        };
        self.diagnostics.removed_records = self.diagnostics.removed_records.saturating_add(1);
        self.diagnostics.cpu_update_time_us = self
            .diagnostics
            .cpu_update_time_us
            .saturating_add(report.cpu_update_time_us);
        self.rebuild_live_diagnostics();
        report
    }

    pub fn apply_procedural_update(
        &mut self,
        update: ProceduralGeometryUpdate,
    ) -> ProceduralGeometryUpdateReport {
        if !update.owner.is_valid() {
            return ProceduralGeometryUpdateReport::default();
        }

        let mut report = ProceduralGeometryUpdateReport {
            new_revision: update.new_revision,
            ..Default::default()
        };
        for record in self.records.values_mut() {
            if record.removed
                || record.submission.procedural_owner.chunk_id != update.owner.chunk_id
            {
                continue;
            }
            report.matched_records = report.matched_records.saturating_add(1);
            report.dirty_records = report.dirty_records.saturating_add(1);
            report.upload_bytes = report.upload_bytes.saturating_add(update.upload_bytes());
            record.submission.procedural_owner.revision = update.new_revision;
            record.submission.update_reason = update.reason;
            record.dirty = record
                .dirty
                .union(DynamicGeometryDirtyFlags::PROCEDURAL)
                .union(DynamicGeometryDirtyFlags::VERTEX_DATA)
                .union(DynamicGeometryDirtyFlags::INDEX_DATA);
            record.generation = record.generation.saturating_add(1);
            record.last_update_frame = self.frame_index;
        }

        self.diagnostics.upload_bytes = self
            .diagnostics
            .upload_bytes
            .saturating_add(report.upload_bytes);
        self.diagnostics.dirty_records = self
            .diagnostics
            .dirty_records
            .saturating_add(report.dirty_records);
        self.diagnostics.procedural_invalidated_records = self
            .diagnostics
            .procedural_invalidated_records
            .saturating_add(report.dirty_records);
        self.diagnostics.procedural_invalidation_cost_us = self
            .diagnostics
            .procedural_invalidation_cost_us
            .saturating_add(u64::from(report.dirty_records).saturating_mul(4));
        self.diagnostics.cpu_update_time_us = self
            .diagnostics
            .cpu_update_time_us
            .saturating_add(u64::from(report.dirty_records).saturating_mul(2));
        self.rebuild_live_diagnostics();
        report
    }

    #[must_use]
    pub fn build_frame_plan(
        &mut self,
        policy: DynamicGeometryRuntimePolicy,
    ) -> DynamicGeometryFramePlan {
        let mut packets = Vec::new();
        let mut diagnostics = self.diagnostics;
        diagnostics.dynamic_draw_count = 0;
        diagnostics.dynamic_dispatch_count = 0;
        diagnostics.classic_draw_count = 0;
        diagnostics.dynamic_cluster_draw_count = 0;

        for record in self.records.values() {
            if record.removed {
                continue;
            }
            let draw_path = draw_path_for(&record.submission, policy);
            if draw_path == DynamicDrawPath::ClassicMeshDraw && !policy.classic_mesh_draws_enabled {
                continue;
            }
            diagnostics.dynamic_draw_count = diagnostics.dynamic_draw_count.saturating_add(1);
            match draw_path {
                DynamicDrawPath::ClassicMeshDraw => {
                    diagnostics.classic_draw_count =
                        diagnostics.classic_draw_count.saturating_add(1);
                }
                DynamicDrawPath::DynamicClusterIndirect => {
                    diagnostics.dynamic_cluster_draw_count =
                        diagnostics.dynamic_cluster_draw_count.saturating_add(1);
                    diagnostics.dynamic_dispatch_count =
                        diagnostics.dynamic_dispatch_count.saturating_add(1);
                }
            }
            packets.push(DynamicGeometryDrawPacket {
                id: record.submission.id,
                draw_path,
                mesh: record.submission.mesh,
                material: record.submission.material,
                vertex_count: record.submission.payload.vertex_count,
                index_count: record.submission.payload.index_count,
                dynamic_cluster_count: dynamic_cluster_count(record.submission.payload),
                priority_hint: record.submission.priority_hint,
                class: record.submission.class,
            });
        }

        self.diagnostics.dynamic_draw_count = diagnostics.dynamic_draw_count;
        self.diagnostics.dynamic_dispatch_count = diagnostics.dynamic_dispatch_count;
        self.diagnostics.classic_draw_count = diagnostics.classic_draw_count;
        self.diagnostics.dynamic_cluster_draw_count = diagnostics.dynamic_cluster_draw_count;
        DynamicGeometryFramePlan {
            packets,
            diagnostics,
        }
    }

    #[must_use]
    pub fn debug_artifact(&self) -> DynamicGeometryDebugArtifact {
        use core::fmt::Write as _;

        let diagnostics = self.diagnostics;
        let mut content = String::new();
        let _ = writeln!(
            content,
            "schema_version={} frame_index={} live_records={} submitted={} updated={} removed={} dirty_records={} upload_bytes={}",
            DYNAMIC_GEOMETRY_SCHEMA_VERSION,
            diagnostics.frame_index,
            diagnostics.live_records,
            diagnostics.submitted_records,
            diagnostics.updated_records,
            diagnostics.removed_records,
            diagnostics.dirty_records,
            diagnostics.upload_bytes
        );
        let _ = writeln!(
            content,
            "costs cpu_update_us={} gpu_update_us={} skinning_us={} procedural_invalidation_us={}",
            diagnostics.cpu_update_time_us,
            diagnostics.gpu_update_time_us,
            diagnostics.skinning_cost_us,
            diagnostics.procedural_invalidation_cost_us
        );
        let _ = writeln!(
            content,
            "draws total={} classic={} dynamic_clusters={} dispatches={}",
            diagnostics.dynamic_draw_count,
            diagnostics.classic_draw_count,
            diagnostics.dynamic_cluster_draw_count,
            diagnostics.dynamic_dispatch_count
        );
        let _ = writeln!(
            content,
            "classes destruction_fragments={} procedural_chunks={} temporary={}",
            diagnostics.destruction_fragment_count,
            diagnostics.procedural_chunk_count,
            diagnostics.temporary_record_count
        );
        let _ = writeln!(
            content,
            "procedural invalidated_records={} full_static_virtual_geometry_rebuilds=0",
            diagnostics.procedural_invalidated_records
        );
        DynamicGeometryDebugArtifact {
            schema_version: DYNAMIC_GEOMETRY_SCHEMA_VERSION,
            content,
        }
    }

    fn record_submission_report(&mut self, report: DynamicGeometrySubmissionReport) {
        self.diagnostics.submitted_records = self.diagnostics.submitted_records.saturating_add(1);
        if report.updated {
            self.diagnostics.updated_records = self.diagnostics.updated_records.saturating_add(1);
        }
        self.diagnostics.dirty_records = self.diagnostics.dirty_records.saturating_add(1);
        self.diagnostics.upload_bytes = self
            .diagnostics
            .upload_bytes
            .saturating_add(report.upload_bytes);
        self.diagnostics.cpu_update_time_us = self
            .diagnostics
            .cpu_update_time_us
            .saturating_add(report.cpu_update_time_us);
        self.diagnostics.gpu_update_time_us = self
            .diagnostics
            .gpu_update_time_us
            .saturating_add(report.gpu_update_time_us);
        self.diagnostics.skinning_cost_us = self
            .diagnostics
            .skinning_cost_us
            .saturating_add(report.skinning_cost_us);
        self.diagnostics.procedural_invalidation_cost_us = self
            .diagnostics
            .procedural_invalidation_cost_us
            .saturating_add(report.procedural_invalidation_cost_us);
    }

    fn rebuild_live_diagnostics(&mut self) {
        self.diagnostics.schema_version = DYNAMIC_GEOMETRY_SCHEMA_VERSION;
        self.diagnostics.frame_index = self.frame_index;
        self.diagnostics.live_records = 0;
        self.diagnostics.destruction_fragment_count = 0;
        self.diagnostics.procedural_chunk_count = 0;
        self.diagnostics.temporary_record_count = 0;

        for record in self.records.values() {
            if record.removed {
                continue;
            }
            self.diagnostics.live_records = self.diagnostics.live_records.saturating_add(1);
            match record.submission.class {
                DynamicGeometryClass::DestructionFragment => {
                    self.diagnostics.destruction_fragment_count = self
                        .diagnostics
                        .destruction_fragment_count
                        .saturating_add(1);
                }
                DynamicGeometryClass::ProceduralTerrainChunk => {
                    self.diagnostics.procedural_chunk_count =
                        self.diagnostics.procedural_chunk_count.saturating_add(1);
                }
                DynamicGeometryClass::TemporaryFxGeometry
                | DynamicGeometryClass::EditorGizmoDebugShape => {
                    self.diagnostics.temporary_record_count =
                        self.diagnostics.temporary_record_count.saturating_add(1);
                }
                DynamicGeometryClass::SkinnedPlayer
                | DynamicGeometryClass::Npc
                | DynamicGeometryClass::Vehicle
                | DynamicGeometryClass::Weapon
                | DynamicGeometryClass::GenericDynamic => {}
            }
        }
    }
}

impl Default for DynamicGeometryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub const fn dynamic_geometry_kind_accepts_runtime_updates(kind: SceneGeometryKind) -> bool {
    matches!(
        kind,
        SceneGeometryKind::Dynamic | SceneGeometryKind::ProceduralChunk
    )
}

fn dirty_flags_for_update(update: DynamicGeometryUpdateHint) -> DynamicGeometryDirtyFlags {
    match update {
        DynamicGeometryUpdateHint::Spawned => DynamicGeometryDirtyFlags::VERTEX_DATA
            .union(DynamicGeometryDirtyFlags::INDEX_DATA)
            .union(DynamicGeometryDirtyFlags::MATERIAL)
            .union(DynamicGeometryDirtyFlags::TRANSFORM),
        DynamicGeometryUpdateHint::TransformChanged
        | DynamicGeometryUpdateHint::EditorGizmoChanged => DynamicGeometryDirtyFlags::TRANSFORM,
        DynamicGeometryUpdateHint::SkinningPoseChanged => DynamicGeometryDirtyFlags::SKINNING,
        DynamicGeometryUpdateHint::VertexDataChanged => DynamicGeometryDirtyFlags::VERTEX_DATA,
        DynamicGeometryUpdateHint::IndexDataChanged => DynamicGeometryDirtyFlags::INDEX_DATA,
        DynamicGeometryUpdateHint::MaterialChanged => DynamicGeometryDirtyFlags::MATERIAL,
        DynamicGeometryUpdateHint::ProceduralChunkEdited => DynamicGeometryDirtyFlags::PROCEDURAL
            .union(DynamicGeometryDirtyFlags::VERTEX_DATA)
            .union(DynamicGeometryDirtyFlags::INDEX_DATA),
        DynamicGeometryUpdateHint::DestructionFractured => DynamicGeometryDirtyFlags::PROCEDURAL
            .union(DynamicGeometryDirtyFlags::VERTEX_DATA)
            .union(DynamicGeometryDirtyFlags::INDEX_DATA),
        DynamicGeometryUpdateHint::LifetimeExpired => DynamicGeometryDirtyFlags::LIFETIME,
        DynamicGeometryUpdateHint::Unknown => DynamicGeometryDirtyFlags::NONE,
    }
}

fn dirty_flags_from_record_delta(
    previous: &DynamicGeometryRecord,
    next: &DynamicGeometrySubmission,
) -> DynamicGeometryDirtyFlags {
    let mut dirty = DynamicGeometryDirtyFlags::NONE;
    if previous.submission.transform != next.transform {
        dirty = dirty.union(DynamicGeometryDirtyFlags::TRANSFORM);
    }
    if previous.submission.material != next.material {
        dirty = dirty.union(DynamicGeometryDirtyFlags::MATERIAL);
    }
    if previous.submission.payload.vertex_bytes != next.payload.vertex_bytes
        || previous.submission.payload.vertex_count != next.payload.vertex_count
    {
        dirty = dirty.union(DynamicGeometryDirtyFlags::VERTEX_DATA);
    }
    if previous.submission.payload.index_bytes != next.payload.index_bytes
        || previous.submission.payload.index_count != next.payload.index_count
    {
        dirty = dirty.union(DynamicGeometryDirtyFlags::INDEX_DATA);
    }
    if previous.submission.payload.skinning_joint_count != next.payload.skinning_joint_count {
        dirty = dirty.union(DynamicGeometryDirtyFlags::SKINNING);
    }
    if previous.submission.procedural_owner != next.procedural_owner {
        dirty = dirty.union(DynamicGeometryDirtyFlags::PROCEDURAL);
    }
    dirty
}

fn upload_bytes_for_dirty(
    dirty: DynamicGeometryDirtyFlags,
    payload: DynamicGeometryPayload,
) -> u64 {
    let mut bytes = 0_u64;
    if dirty.contains(DynamicGeometryDirtyFlags::VERTEX_DATA) {
        bytes = bytes.saturating_add(payload.vertex_bytes);
    }
    if dirty.contains(DynamicGeometryDirtyFlags::INDEX_DATA) {
        bytes = bytes.saturating_add(payload.index_bytes);
    }
    bytes
}

fn skinning_cost_for(
    submission: &DynamicGeometrySubmission,
    dirty: DynamicGeometryDirtyFlags,
) -> u64 {
    if !submission.class.skinned() && submission.payload.skinning_joint_count == 0 {
        return 0;
    }
    if !dirty.contains(DynamicGeometryDirtyFlags::SKINNING)
        && !dirty.contains(DynamicGeometryDirtyFlags::VERTEX_DATA)
    {
        return 0;
    }
    u64::from(submission.payload.skinning_joint_count.max(1))
        .saturating_mul(u64::from(submission.payload.vertex_count / 256).saturating_add(1))
}

fn procedural_cost_for(
    submission: &DynamicGeometrySubmission,
    dirty: DynamicGeometryDirtyFlags,
) -> u64 {
    if !dirty.contains(DynamicGeometryDirtyFlags::PROCEDURAL) {
        return 0;
    }
    let class_cost: u64 = match submission.class {
        DynamicGeometryClass::DestructionFragment => 3,
        DynamicGeometryClass::ProceduralTerrainChunk => 8,
        _ => 2,
    };
    class_cost
}

fn cpu_cost_for(submission: &DynamicGeometrySubmission, dirty: DynamicGeometryDirtyFlags) -> u64 {
    if dirty.is_empty() {
        return 0;
    }
    let base = 1_u64;
    let upload_units = upload_bytes_for_dirty(dirty, submission.payload) / (32 * 1024);
    let skinning_units = u64::from(submission.payload.skinning_joint_count) / 16;
    base.saturating_add(upload_units)
        .saturating_add(skinning_units)
}

fn gpu_cost_for(submission: &DynamicGeometrySubmission, dirty: DynamicGeometryDirtyFlags) -> u64 {
    if dirty.is_empty() {
        return 0;
    }
    let triangles = u64::from(submission.payload.triangle_count());
    let vertex_upload = if dirty.contains(DynamicGeometryDirtyFlags::VERTEX_DATA) {
        submission.payload.vertex_bytes / (64 * 1024)
    } else {
        0
    };
    (triangles / 4096).saturating_add(vertex_upload)
}

fn draw_path_for(
    submission: &DynamicGeometrySubmission,
    policy: DynamicGeometryRuntimePolicy,
) -> DynamicDrawPath {
    if !policy.dynamic_clusters_enabled || submission.payload.dynamic_cluster_budget == 0 {
        return DynamicDrawPath::ClassicMeshDraw;
    }
    match submission.cluster_mode {
        DynamicClusterMode::ClassicMeshOnly => DynamicDrawPath::ClassicMeshDraw,
        DynamicClusterMode::GenerateDynamicClusters
        | DynamicClusterMode::ProvidedDynamicClusters => {
            if class_uses_dynamic_clusters(submission.class, policy) {
                DynamicDrawPath::DynamicClusterIndirect
            } else {
                DynamicDrawPath::ClassicMeshDraw
            }
        }
    }
}

fn class_uses_dynamic_clusters(
    class: DynamicGeometryClass,
    policy: DynamicGeometryRuntimePolicy,
) -> bool {
    match class {
        DynamicGeometryClass::DestructionFragment => {
            policy.destruction_fragments_use_dynamic_clusters
        }
        DynamicGeometryClass::ProceduralTerrainChunk => {
            policy.procedural_chunks_use_dynamic_clusters
        }
        DynamicGeometryClass::Vehicle => policy.vehicles_use_dynamic_clusters,
        DynamicGeometryClass::SkinnedPlayer
        | DynamicGeometryClass::Npc
        | DynamicGeometryClass::Weapon
        | DynamicGeometryClass::TemporaryFxGeometry
        | DynamicGeometryClass::EditorGizmoDebugShape
        | DynamicGeometryClass::GenericDynamic => false,
    }
}

fn dynamic_cluster_count(payload: DynamicGeometryPayload) -> u16 {
    if payload.dynamic_cluster_budget == 0 {
        return 0;
    }
    let triangles = payload.triangle_count().max(1);
    let approximate = triangles
        .div_ceil(128)
        .min(u32::from(payload.dynamic_cluster_budget));
    approximate as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_scene::{GeometryDeclaration, RenderableFlags};

    fn renderable(geometry: u32, material: u32) -> Renderable {
        Renderable::new(
            GeometryRef::new(geometry),
            MaterialRef::new(material),
            RenderableFlags::DYNAMIC,
        )
    }

    #[test]
    fn dynamic_actor_scene_records_upload_dirty_and_skinning_cost() {
        let mut database = DynamicGeometryDatabase::new();
        database.begin_frame(1);
        let payload =
            DynamicGeometryPayload::new(4_096, 12_288, 192 * 1024, 48 * 1024).with_skinning(72);
        let submission = DynamicGeometrySubmission {
            id: DynamicGeometrySubmissionId::new(1),
            mesh: GeometryRef::new(11),
            material: MaterialRef::new(21),
            payload,
            class: DynamicGeometryClass::SkinnedPlayer,
            lifetime: DynamicGeometryLifetime::PersistentActor,
            update_reason: DynamicGeometryUpdateHint::Spawned,
            ..Default::default()
        };

        let report = database.submit(submission);

        assert!(report.created);
        assert!(
            report
                .dirty
                .contains(DynamicGeometryDirtyFlags::VERTEX_DATA)
        );
        assert_eq!(report.upload_bytes, payload.upload_bytes());
        assert!(report.skinning_cost_us > 0);
        assert_eq!(database.diagnostics().live_records, 1);
        assert_eq!(database.diagnostics().submitted_records, 1);
    }

    #[test]
    fn procedural_invalidation_scene_updates_chunk_without_full_rebuild() {
        let owner = ProceduralChunkOwner::new(42, 1);
        let mut database = DynamicGeometryDatabase::new();
        database.begin_frame(2);
        let payload = DynamicGeometryPayload::new(8_192, 24_576, 384 * 1024, 96 * 1024)
            .with_dynamic_cluster_budget(96);
        let submission = DynamicGeometrySubmission {
            id: DynamicGeometrySubmissionId::new(2),
            mesh: GeometryRef::new(31),
            material: MaterialRef::new(41),
            payload,
            class: DynamicGeometryClass::ProceduralTerrainChunk,
            lifetime: DynamicGeometryLifetime::StreamedChunk,
            procedural_owner: owner,
            cluster_mode: DynamicClusterMode::GenerateDynamicClusters,
            update_reason: DynamicGeometryUpdateHint::Spawned,
            ..Default::default()
        };
        database.submit(submission);
        database.begin_frame(3);

        let report =
            database.apply_procedural_update(ProceduralGeometryUpdate::new(owner, 2, 16_384, 4096));

        assert_eq!(report.matched_records, 1);
        assert_eq!(report.dirty_records, 1);
        assert_eq!(report.upload_bytes, 20_480);
        assert_eq!(report.full_static_virtual_geometry_rebuilds, 0);
        let record = database
            .record(DynamicGeometrySubmissionId::new(2))
            .expect("procedural record should remain live");
        assert_eq!(record.submission.procedural_owner.revision, 2);
        assert!(record.dirty.contains(DynamicGeometryDirtyFlags::PROCEDURAL));
        assert!(database.diagnostics().procedural_invalidation_cost_us > 0);
    }

    #[test]
    fn destruction_fragment_stress_test_writes_upload_artifact() {
        let mut database = DynamicGeometryDatabase::new();
        database.begin_frame(4);
        for index in 0..512_u64 {
            let payload = DynamicGeometryPayload::new(96, 288, 96 * 32, 288 * 4)
                .with_dynamic_cluster_budget(4);
            let submission = DynamicGeometrySubmission {
                id: DynamicGeometrySubmissionId::new(10_000 + index),
                mesh: GeometryRef::new(100 + index as u32),
                material: MaterialRef::new(9),
                payload,
                class: DynamicGeometryClass::DestructionFragment,
                lifetime: DynamicGeometryLifetime::DestructionDebris,
                update_reason: DynamicGeometryUpdateHint::DestructionFractured,
                cluster_mode: DynamicClusterMode::GenerateDynamicClusters,
                priority_hint: PagePriorityHint::High,
                ..Default::default()
            };
            database.submit(submission);
        }
        let plan = database.build_frame_plan(DynamicGeometryRuntimePolicy::default());
        let artifact = database.debug_artifact();
        if let Some(path) = std::env::var_os(DYNAMIC_GEOMETRY_BENCHMARK_ARTIFACT_ENV) {
            if let Some(parent) = std::path::Path::new(&path).parent() {
                std::fs::create_dir_all(parent)
                    .expect("dynamic geometry artifact parent should be creatable");
            }
            std::fs::write(path, artifact.content.as_bytes())
                .expect("dynamic geometry benchmark artifact should be writable");
        }

        assert_eq!(database.diagnostics().destruction_fragment_count, 512);
        assert_eq!(plan.diagnostics.dynamic_cluster_draw_count, 512);
        assert!(database.diagnostics().upload_bytes > 0);
        assert!(artifact.content.contains("upload_bytes="));
        assert!(artifact.content.contains("dirty_records="));
    }

    #[test]
    fn static_virtual_geometry_and_dynamic_geometry_coexist() {
        assert!(!dynamic_geometry_kind_accepts_runtime_updates(
            SceneGeometryKind::Static
        ));
        assert!(dynamic_geometry_kind_accepts_runtime_updates(
            SceneGeometryKind::Dynamic
        ));
        assert!(dynamic_geometry_kind_accepts_runtime_updates(
            SceneGeometryKind::ProceduralChunk
        ));

        let mut database = DynamicGeometryDatabase::new();
        database.begin_frame(5);
        let actor = DynamicGeometrySubmission::from_renderable(
            DynamicGeometrySubmissionId::new(77),
            &renderable(7, 8),
            DynamicGeometryClass::Vehicle,
            DynamicGeometryUpdateHint::Spawned,
            DynamicGeometryPayload::new(2_048, 6_144, 96 * 1024, 24 * 1024)
                .with_dynamic_cluster_budget(32),
        );
        database.submit(DynamicGeometrySubmission {
            cluster_mode: DynamicClusterMode::GenerateDynamicClusters,
            ..actor
        });
        let plan = database.build_frame_plan(DynamicGeometryRuntimePolicy::default());

        assert_eq!(plan.packets.len(), 1);
        assert_eq!(
            plan.packets[0].draw_path,
            DynamicDrawPath::DynamicClusterIndirect
        );
    }

    #[test]
    fn scene_declaration_submits_dynamic_geometry_without_private_folklore() {
        let renderable = renderable(12, 34);
        let owner = ProceduralChunkOwner::new(9, 1);
        let declaration = GeometryDeclaration {
            kind: SceneGeometryKind::ProceduralChunk,
            lighting_metadata: true,
            procedural_owner: owner,
        };
        let authoring = DynamicGeometryAuthoring {
            class: DynamicGeometryClass::ProceduralTerrainChunk,
            lifetime: DynamicGeometryLifetime::StreamedChunk,
            update_hint: DynamicGeometryUpdateHint::ProceduralChunkEdited,
            procedural_owner: ProceduralChunkOwner::NONE,
            allow_dynamic_clusters: true,
            priority_hint: PagePriorityHint::WorldCritical,
        };

        let submission = DynamicGeometrySubmission::from_scene_declaration(
            DynamicGeometrySubmissionId::new(9),
            &renderable,
            Some(&declaration),
            &authoring,
            DynamicGeometryBounds::new([1.0, 2.0, 3.0], 32.0),
            DynamicGeometryTransform::translated(1.0, 0.0, 0.0),
            DynamicGeometryTransform::IDENTITY,
            DynamicGeometryPayload::new(512, 1536, 32 * 1024, 6 * 1024)
                .with_dynamic_cluster_budget(16),
        );

        assert_eq!(submission.mesh, renderable.geometry);
        assert_eq!(submission.material, renderable.material);
        assert_eq!(submission.procedural_owner, owner);
        assert_eq!(
            submission.cluster_mode,
            DynamicClusterMode::GenerateDynamicClusters
        );
        assert_eq!(submission.priority_hint, PagePriorityHint::WorldCritical);
    }
}
