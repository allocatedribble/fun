use bevy::{prelude::*, render::render_resource::ShaderType};

use crate::{
    FunDrawBucketKey, FunMaterialSignature, FunMeshletFormat, FunRenderPath,
    FunShaderPipelineSignature, InstanceRange, StaticRenderBatch, signature_bucket_u32,
};

pub const FUN_GPU_VISIBILITY_SCHEMA_VERSION: u16 = 1;
pub const GPU_VISIBILITY_WORKGROUP_SIZE: u32 = 64;

pub const GPU_VIS_OBJECT_STATIC_OPAQUE: u32 = 1 << 0;
pub const GPU_VIS_OBJECT_OCCLUDER: u32 = 1 << 1;
pub const GPU_VIS_OBJECT_MESHLET: u32 = 1 << 2;
pub const GPU_VIS_OBJECT_RASTER: u32 = 1 << 3;
pub const GPU_VIS_OBJECT_TRANSPARENT: u32 = 1 << 4;
pub const GPU_VIS_OBJECT_FOLIAGE: u32 = 1 << 5;
pub const GPU_VIS_OBJECT_PARTICLE: u32 = 1 << 6;
pub const GPU_VIS_OBJECT_SKINNED: u32 = 1 << 7;
pub const GPU_VIS_OBJECT_CEF_UI: u32 = 1 << 8;
pub const GPU_VIS_OBJECT_DEBUG: u32 = 1 << 9;
pub const GPU_VIS_OBJECT_VIEWMODEL: u32 = 1 << 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpuVisibilityStage {
    FrustumCullCellsAndBatches,
    CullInstancesOrMeshlets,
    SelectLodOrClusterLevel,
    CompactVisibleIds,
    BuildIndirectDrawArgs,
}

impl GpuVisibilityStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FrustumCullCellsAndBatches => "frustum_cull_cells_and_batches",
            Self::CullInstancesOrMeshlets => "cull_instances_or_meshlets",
            Self::SelectLodOrClusterLevel => "select_lod_or_cluster_level",
            Self::CompactVisibleIds => "compact_visible_ids",
            Self::BuildIndirectDrawArgs => "build_indirect_draw_args",
        }
    }
}

pub const GPU_VISIBILITY_STATIC_OPAQUE_STAGES: &[GpuVisibilityStage] = &[
    GpuVisibilityStage::FrustumCullCellsAndBatches,
    GpuVisibilityStage::CullInstancesOrMeshlets,
    GpuVisibilityStage::SelectLodOrClusterLevel,
    GpuVisibilityStage::CompactVisibleIds,
    GpuVisibilityStage::BuildIndirectDrawArgs,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StaticOpaqueVisibilityPath {
    GpuDriven,
    CpuStaticCellCulling,
    FallbackDirect,
}

impl StaticOpaqueVisibilityPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuDriven => "gpu_driven",
            Self::CpuStaticCellCulling => "cpu_static_cell_culling",
            Self::FallbackDirect => "fallback_direct",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StaticOpaqueVisibilityFallbackReason {
    UnsupportedBackend,
    DebugMode,
    EmptyScene,
    TinyScene,
    BuffersNotResident,
    NonStaticOpaqueExcluded,
}

impl StaticOpaqueVisibilityFallbackReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedBackend => "unsupported_backend",
            Self::DebugMode => "debug_mode",
            Self::EmptyScene => "empty_scene",
            Self::TinyScene => "tiny_scene",
            Self::BuffersNotResident => "buffers_not_resident",
            Self::NonStaticOpaqueExcluded => "non_static_opaque_excluded",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GpuVisibilityObjectId(pub u32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GpuVisibilityCellId(pub u32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GpuVisibilityBatchId(pub u32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GpuMaterialBucket(pub u32);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GpuMeshClusterRange {
    pub start: u32,
    pub count: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct GpuObjectRecord {
    pub ids_flags: [u32; 4],
    pub bounds_center_radius: [f32; 4],
    pub instance_range_path: [u32; 4],
    pub material_mesh_range: [u32; 4],
}

impl GpuObjectRecord {
    pub const SIZE_BYTES: u64 = std::mem::size_of::<Self>() as u64;
    pub const ALIGN_BYTES: u64 = std::mem::align_of::<Self>() as u64;

    pub fn from_static_batch(
        object_id: GpuVisibilityObjectId,
        batch_id: GpuVisibilityBatchId,
        batch: &StaticRenderBatch,
    ) -> Option<Self> {
        let instance_range = batch.instance_range?;
        let (center, radius) =
            center_radius_from_bounds(batch.aggregate_bounds.min, batch.aggregate_bounds.max);
        let material_bucket = signature_bucket_u32(batch.key.render_batch_key.material_signature.0);
        let pipeline_bucket = signature_bucket_u32(batch.key.render_batch_key.pipeline_signature.0);
        let mut flags = GPU_VIS_OBJECT_STATIC_OPAQUE;
        if batch.key.shadow_participation {
            flags |= GPU_VIS_OBJECT_OCCLUDER;
        }
        if batch.key.render_path.uses_meshlet() {
            flags |= GPU_VIS_OBJECT_MESHLET;
        } else if batch.key.render_path.emits_visible_raster() {
            flags |= GPU_VIS_OBJECT_RASTER;
        }
        Some(Self {
            ids_flags: [object_id.0, batch.cell_id.0, batch_id.0, flags],
            bounds_center_radius: [center.x, center.y, center.z, radius],
            instance_range_path: [
                instance_range.start,
                instance_range.count,
                gpu_render_path_code(batch.key.render_path),
                0,
            ],
            material_mesh_range: [
                material_bucket,
                batch.key.catalog_asset_id,
                1,
                pipeline_bucket,
            ],
        })
    }

    pub const fn object_id(self) -> GpuVisibilityObjectId {
        GpuVisibilityObjectId(self.ids_flags[0])
    }

    pub const fn cell_id(self) -> GpuVisibilityCellId {
        GpuVisibilityCellId(self.ids_flags[1])
    }

    pub const fn batch_id(self) -> GpuVisibilityBatchId {
        GpuVisibilityBatchId(self.ids_flags[2])
    }

    pub const fn flags(self) -> u32 {
        self.ids_flags[3]
    }

    pub const fn instance_range(self) -> InstanceRange {
        InstanceRange {
            start: self.instance_range_path[0],
            count: self.instance_range_path[1],
        }
    }

    pub const fn render_path(self) -> FunRenderPath {
        fun_render_path_from_code(self.instance_range_path[2])
    }

    pub const fn material_bucket(self) -> GpuMaterialBucket {
        GpuMaterialBucket(self.material_mesh_range[0])
    }

    pub const fn mesh_cluster_range(self) -> GpuMeshClusterRange {
        GpuMeshClusterRange {
            start: self.material_mesh_range[1],
            count: self.material_mesh_range[2],
        }
    }

    pub const fn pipeline_bucket(self) -> u32 {
        self.material_mesh_range[3]
    }

    pub const fn is_static_opaque_world(self) -> bool {
        let flags = self.flags();
        flags & GPU_VIS_OBJECT_STATIC_OPAQUE != 0
            && flags
                & (GPU_VIS_OBJECT_TRANSPARENT
                    | GPU_VIS_OBJECT_FOLIAGE
                    | GPU_VIS_OBJECT_PARTICLE
                    | GPU_VIS_OBJECT_SKINNED
                    | GPU_VIS_OBJECT_CEF_UI
                    | GPU_VIS_OBJECT_DEBUG
                    | GPU_VIS_OBJECT_VIEWMODEL)
                == 0
    }

    pub fn excluded(flags: u32) -> Self {
        Self {
            ids_flags: [0, 0, 0, flags],
            ..Default::default()
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, PartialEq, ShaderType)]
pub struct GpuInstanceRecord {
    pub transform_0: [f32; 4],
    pub transform_1: [f32; 4],
    pub transform_2: [f32; 4],
    pub material_mesh_flags: [u32; 4],
}

impl Default for GpuInstanceRecord {
    fn default() -> Self {
        Self {
            transform_0: [1.0, 0.0, 0.0, 0.0],
            transform_1: [0.0, 1.0, 0.0, 0.0],
            transform_2: [0.0, 0.0, 1.0, 0.0],
            material_mesh_flags: [0; 4],
        }
    }
}

impl GpuInstanceRecord {
    pub const SIZE_BYTES: u64 = std::mem::size_of::<Self>() as u64;
    pub const ALIGN_BYTES: u64 = std::mem::align_of::<Self>() as u64;

    pub fn from_transform(
        transform: Mat4,
        material_bucket: GpuMaterialBucket,
        mesh_index: u32,
        flags: u32,
    ) -> Self {
        let cols = transform.to_cols_array_2d();
        Self {
            transform_0: cols[0],
            transform_1: cols[1],
            transform_2: cols[2],
            material_mesh_flags: [material_bucket.0, mesh_index, flags, 0],
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct GpuBoundsRecord {
    pub center_radius: [f32; 4],
    pub cone_axis_cutoff: [f32; 4],
}

impl GpuBoundsRecord {
    pub const SIZE_BYTES: u64 = std::mem::size_of::<Self>() as u64;
    pub const ALIGN_BYTES: u64 = std::mem::align_of::<Self>() as u64;

    pub const fn sphere(center: [f32; 3], radius: f32) -> Self {
        Self {
            center_radius: [center[0], center[1], center[2], radius],
            cone_axis_cutoff: [0.0, 0.0, 0.0, -1.0],
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct GpuVisibilityViewConstants {
    pub frustum_planes: [[f32; 4]; 6],
    pub camera_position_lod: [f32; 4],
    pub thresholds: [u32; 4],
}

impl GpuVisibilityViewConstants {
    pub const SIZE_BYTES: u64 = std::mem::size_of::<Self>() as u64;
    pub const ALIGN_BYTES: u64 = std::mem::align_of::<Self>() as u64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuDrawBucket {
    pub bucket_key: FunDrawBucketKey,
    pub material_bucket: GpuMaterialBucket,
    pub mesh_range: GpuMeshClusterRange,
    pub indirect_arg_offset: u32,
    pub visible_range: InstanceRange,
}

impl GpuDrawBucket {
    pub const fn cpu_submit_draws(self) -> u32 {
        1
    }
}

pub fn draw_bucket_key_for_object(object: GpuObjectRecord) -> FunDrawBucketKey {
    let meshlet_format = if object.flags() & GPU_VIS_OBJECT_MESHLET != 0 {
        FunMeshletFormat::MeshletMesh
    } else {
        FunMeshletFormat::RasterMesh
    };
    FunDrawBucketKey::opaque_static(
        FunMaterialSignature(object.material_bucket().0 as u64),
        FunShaderPipelineSignature(object.pipeline_bucket() as u64),
        meshlet_format,
        object.render_path(),
    )
}

pub const fn gpu_render_path_code(render_path: FunRenderPath) -> u32 {
    match render_path {
        FunRenderPath::StandardRaster => 1,
        FunRenderPath::InstancedRaster => 2,
        FunRenderPath::GpuCulledIndirect => 3,
        FunRenderPath::MeshletStaticDense => 4,
        FunRenderPath::MeshletDynamicDense => 5,
        FunRenderPath::RayProxyOnly => 6,
        FunRenderPath::Viewmodel => 7,
        FunRenderPath::CefUi => 8,
        FunRenderPath::DebugOnly => 9,
    }
}

pub const fn fun_render_path_from_code(code: u32) -> FunRenderPath {
    match code {
        2 => FunRenderPath::InstancedRaster,
        3 => FunRenderPath::GpuCulledIndirect,
        4 => FunRenderPath::MeshletStaticDense,
        5 => FunRenderPath::MeshletDynamicDense,
        6 => FunRenderPath::RayProxyOnly,
        7 => FunRenderPath::Viewmodel,
        8 => FunRenderPath::CefUi,
        9 => FunRenderPath::DebugOnly,
        _ => FunRenderPath::StandardRaster,
    }
}

fn center_radius_from_bounds(min: [f32; 3], max: [f32; 3]) -> (Vec3, f32) {
    let min = Vec3::from_array(min);
    let max = Vec3::from_array(max);
    let center = (min + max) * 0.5;
    let radius = (max - center).length();
    (center, radius)
}
