//! Pass C7.5 — typed cloud shadow GPU pipeline construction
//! + dispatch.
//!
//! Builds the typed `wgpu` compute pipelines for the typed
//! `cloud_shadow_project.wgsl` + `cloud_shadow_filter.wgsl`
//! shaders, allocates the typed bind-group layouts + the
//! typed `CloudShadowProjectionConstants` uniform buffer +
//! the typed `CloudWorldShadowTransmittance` /
//! `CloudWorldShadowFiltered` storage textures, and records
//! the typed two-stage dispatch (`LuxCloudShadowProject` →
//! `LuxCloudShadowFilter`) through a typed
//! `wgpu::CommandEncoder`.
//!
//! Ownership: every typed pipeline + resource + dispatch
//! function lives in `fun-renderer`.  The typed
//! `fun_render::sky` legacy module is the typed
//! extraction-only bridge; it does NOT own the typed cloud
//! shadow GPU work.
//!
//! Contract — typed cloud shadow MUST NOT be baked into
//! opaque virtual shadow depth.  The typed dispatch path
//! NEVER touches typed `LuxVirtualShadowPages` /
//! `LuxShadowAtlas`; the typed bind-group layout exposes
//! only the typed cloud-owned resources (audited by the
//! typed `bind_group_layout_only_touches_cloud_resources`
//! test).

use crate::cloud_shaders::{
    CLOUD_SHADOW_FILTER_ENTRY_POINT, CLOUD_SHADOW_FILTER_WGSL, CLOUD_SHADOW_PROJECT_ENTRY_POINT,
    CLOUD_SHADOW_PROJECT_WGSL,
};
use crate::cloud_shadow::{
    CLOUD_SHADOW_PROJECTION_CONSTANTS_CPU_BYTES, CloudShadowProjectionConstants,
    CloudShadowProjectionMode, CloudShadowResolution, CloudShadowStorageFormat,
};

pub const FUN_RENDERER_CLOUD_SHADOW_PIPELINES_SCHEMA_VERSION: u16 = 1;

/// Typed compute workgroup size matching the typed
/// `cloud_shadow_project.wgsl` + `cloud_shadow_filter.wgsl`
/// `@workgroup_size(8, 8, 1)` declarations.  Drives the
/// typed dispatch workgroup-count helper.
pub const CLOUD_SHADOW_WORKGROUP_SIZE_X: u32 = 8;
pub const CLOUD_SHADOW_WORKGROUP_SIZE_Y: u32 = 8;

/// Typed Pass C7.5 — wgpu mirror of the typed CPU
/// `CloudShadowProjectionConstants`.  `#[repr(C)]` matches
/// the typed WGSL `CloudShadowProjectionConstants` struct
/// layout declared in `cloud_shadow_project.wgsl`.
///
/// Layout (192 bytes, 16-byte aligned):
///   header: [u32; 4]                 (16 B)
///   world_from_shadow_uv: [[f32; 4]; 4]  (64 B)
///   shadow_uv_from_world: [[f32; 4]; 4]  (64 B)
///   sun_dir_ws: [f32; 4]             (16 B)  // .xyz = sun, .w = reserved
///   slab: [f32; 4]                   (16 B)  // .x = base, .y = top, .z = max_dist, .w = reserved
///   knobs: [f32; 4]                  (16 B)  // .x = opacity, .y = softness, .z/.w = reserved
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct CloudShadowProjectionConstantsGpu {
    pub header: [u32; 4],
    pub world_from_shadow_uv: [[f32; 4]; 4],
    pub shadow_uv_from_world: [[f32; 4]; 4],
    pub sun_dir_ws: [f32; 4],
    pub slab: [f32; 4],
    pub knobs: [f32; 4],
}

/// Typed Pass C7.5 — typed GPU-side byte count of the
/// typed `CloudShadowProjectionConstantsGpu` struct.  Used
/// to size the typed wgpu uniform buffer.
pub const CLOUD_SHADOW_PROJECTION_CONSTANTS_GPU_BYTES: u64 =
    core::mem::size_of::<CloudShadowProjectionConstantsGpu>() as u64;

impl CloudShadowProjectionConstantsGpu {
    /// Typed Pass C7.5 builder — pack the typed CPU
    /// `CloudShadowProjectionConstants` into the typed GPU
    /// std140-friendly layout.  WGSL matrices are
    /// column-major; the typed CPU side stores row-major,
    /// so we transpose on the way out.
    #[must_use]
    pub fn from_cpu(cpu: &CloudShadowProjectionConstants) -> Self {
        let mode_idx = cloud_shadow_projection_mode_index(cpu.mode);
        let schema_and_mode = (u32::from(mode_idx) << 16) | u32::from(cpu.schema_version);
        let light_lo = cpu.light_id.0 as u32;
        let light_hi = (cpu.light_id.0 >> 32) as u32;

        let world_from_shadow_uv = transpose_4x4(&cpu.world_from_shadow_uv);
        let shadow_uv_from_world = transpose_4x4(&cpu.shadow_uv_from_world);

        let sun = cpu.sun_direction_ws;
        let sun_dir_ws = [sun[0], sun[1], sun[2], 0.0];
        let slab = [
            cpu.cloud_base_meters,
            cpu.cloud_top_meters,
            cpu.max_distance_meters,
            0.0,
        ];
        let knobs = [cpu.opacity_scale, cpu.softness, 0.0, 0.0];

        Self {
            header: [schema_and_mode, light_lo, light_hi, cpu.frame_index],
            world_from_shadow_uv,
            shadow_uv_from_world,
            sun_dir_ws,
            slab,
            knobs,
        }
    }

    /// Typed predicate: extract the typed projection mode
    /// index from the typed packed header.  Mirrors
    /// `cloud_shadow_projection_mode_index`.
    #[must_use]
    pub const fn mode_index(&self) -> u16 {
        (self.header[0] >> 16) as u16
    }

    /// Typed predicate: extract the typed schema version
    /// from the typed packed header.
    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        (self.header[0] & 0xFFFF) as u16
    }

    /// Typed predicate: extract the typed Lux light id
    /// from the typed packed header.
    #[must_use]
    pub const fn light_id_u64(&self) -> u64 {
        ((self.header[2] as u64) << 32) | (self.header[1] as u64)
    }

    /// Typed predicate: extract the typed frame index from
    /// the typed packed header.
    #[must_use]
    pub const fn frame_index(&self) -> u32 {
        self.header[3]
    }

    /// Typed Pass C7.5 — serialize the typed GPU struct
    /// into a typed `[u8; 192]` byte array suitable for
    /// the typed `wgpu::Queue::write_buffer` upload.
    /// Avoids typed `unsafe` since the typed crate forbids
    /// it (lib-level `#![forbid(unsafe_code)]`).
    #[must_use]
    pub fn to_bytes(&self) -> [u8; CLOUD_SHADOW_PROJECTION_CONSTANTS_GPU_BYTES as usize] {
        let mut bytes = [0u8; CLOUD_SHADOW_PROJECTION_CONSTANTS_GPU_BYTES as usize];
        let mut offset = 0usize;
        for &x in &self.header {
            bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
            offset += 4;
        }
        for row in &self.world_from_shadow_uv {
            for &x in row {
                bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
                offset += 4;
            }
        }
        for row in &self.shadow_uv_from_world {
            for &x in row {
                bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
                offset += 4;
            }
        }
        for &x in &self.sun_dir_ws {
            bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
            offset += 4;
        }
        for &x in &self.slab {
            bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
            offset += 4;
        }
        for &x in &self.knobs {
            bytes[offset..offset + 4].copy_from_slice(&x.to_ne_bytes());
            offset += 4;
        }
        debug_assert_eq!(offset, bytes.len());
        bytes
    }
}

#[must_use]
const fn cloud_shadow_projection_mode_index(mode: CloudShadowProjectionMode) -> u16 {
    match mode {
        CloudShadowProjectionMode::CameraCenteredPlane => 0,
        CloudShadowProjectionMode::DirectionalLightClipRegion => 1,
        CloudShadowProjectionMode::CascadedDirectionalRegions => 2,
    }
}

#[must_use]
fn transpose_4x4(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut t = [[0.0f32; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            t[col][row] = m[row][col];
        }
    }
    t
}

/// Typed Pass C7.5 — typed workgroup count for a typed
/// shadow extent.  Equals `ceil(extent / workgroup_size)`
/// in each dimension.
#[must_use]
pub const fn cloud_shadow_workgroup_count(extent: [u32; 2]) -> [u32; 3] {
    let x = extent[0].div_ceil(CLOUD_SHADOW_WORKGROUP_SIZE_X);
    let y = extent[1].div_ceil(CLOUD_SHADOW_WORKGROUP_SIZE_Y);
    [x, y, 1]
}

/// Typed Pass C7.5 — typed mapping from the typed CPU
/// `CloudShadowStorageFormat` taxonomy to the typed wgpu
/// `TextureFormat` enum.
#[cfg(feature = "wgpu_bridge")]
#[must_use]
pub const fn cloud_shadow_storage_format_to_wgpu(
    format: CloudShadowStorageFormat,
) -> ::wgpu::TextureFormat {
    match format {
        CloudShadowStorageFormat::R8Unorm => ::wgpu::TextureFormat::R8Unorm,
        CloudShadowStorageFormat::R16Float => ::wgpu::TextureFormat::R16Float,
        CloudShadowStorageFormat::Rgba16FloatPacked => ::wgpu::TextureFormat::Rgba16Float,
    }
}

/// Typed Pass C7.5 — typed mapping from the typed CPU
/// `CloudShadowResolution` taxonomy to a typed `[u32; 2]`
/// pixel extent.  Mirrors
/// `CloudShadowResolution::pixel_extent` but produces the
/// typed `[u32; 2]` shape the typed GPU API consumes.
#[must_use]
pub const fn cloud_shadow_resolution_extent(resolution: CloudShadowResolution) -> [u32; 2] {
    let (w, h) = resolution.pixel_extent();
    [w, h]
}

// ============================================================================
// Section — typed wgpu pipeline + resource integration
// ============================================================================

#[cfg(feature = "wgpu_bridge")]
mod wgpu_bridge {
    use super::{
        CLOUD_SHADOW_FILTER_ENTRY_POINT, CLOUD_SHADOW_FILTER_WGSL,
        CLOUD_SHADOW_PROJECT_ENTRY_POINT, CLOUD_SHADOW_PROJECT_WGSL,
        CLOUD_SHADOW_PROJECTION_CONSTANTS_GPU_BYTES, CloudShadowProjectionConstantsGpu,
        FUN_RENDERER_CLOUD_SHADOW_PIPELINES_SCHEMA_VERSION, cloud_shadow_storage_format_to_wgpu,
        cloud_shadow_workgroup_count,
    };
    use crate::cloud_shadow::CloudShadowStorageFormat;

    /// Typed Pass C7.5 — typed bind-group layout entry
    /// templates the typed cloud-shadow project pass binds.
    /// Lifted into a typed const fn so the typed bind layout
    /// is auditable + typed identical across the typed
    /// pipeline-create + typed bind-group-create paths.
    #[must_use]
    pub fn cloud_shadow_project_bind_group_layout_entries(
        out_format: CloudShadowStorageFormat,
    ) -> [::wgpu::BindGroupLayoutEntry; 5] {
        let storage_format = cloud_shadow_storage_format_to_wgpu(out_format);
        [
            // binding 0: cloud: CloudParams uniform
            ::wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::Buffer {
                    ty: ::wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 1: shadow_projection: CloudShadowProjectionConstants
            ::wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::Buffer {
                    ty: ::wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 2: weather_map: texture_2d<f32>
            ::wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::Texture {
                    sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: ::wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // binding 3: shape_noise: texture_3d<f32>
            ::wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::Texture {
                    sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: ::wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            },
            // binding 4: cloud_shadow_out: texture_storage_2d<r16float, write>
            ::wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::StorageTexture {
                    access: ::wgpu::StorageTextureAccess::WriteOnly,
                    format: storage_format,
                    view_dimension: ::wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ]
    }

    /// Typed Pass C7.5 — typed bind-group layout entries
    /// the typed cloud-shadow filter pass binds.
    #[must_use]
    pub fn cloud_shadow_filter_bind_group_layout_entries(
        out_format: CloudShadowStorageFormat,
    ) -> [::wgpu::BindGroupLayoutEntry; 3] {
        let storage_format = cloud_shadow_storage_format_to_wgpu(out_format);
        [
            // binding 0: shadow_projection: CloudShadowProjectionConstants
            ::wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::Buffer {
                    ty: ::wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // binding 1: cloud_shadow_in: texture_2d<f32>
            ::wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::Texture {
                    sample_type: ::wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: ::wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // binding 2: cloud_shadow_filtered: texture_storage_2d<r16float, write>
            ::wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: ::wgpu::ShaderStages::COMPUTE,
                ty: ::wgpu::BindingType::StorageTexture {
                    access: ::wgpu::StorageTextureAccess::WriteOnly,
                    format: storage_format,
                    view_dimension: ::wgpu::TextureViewDimension::D2,
                },
                count: None,
            },
        ]
    }

    /// Typed Pass C7.5 — wgpu pipeline + bind group layout
    /// for the typed `cloud_shadow_project.wgsl` compute
    /// shader.
    pub struct CloudShadowProjectPipeline {
        pub schema_version: u16,
        pub shader: ::wgpu::ShaderModule,
        pub bind_group_layout: ::wgpu::BindGroupLayout,
        pub pipeline_layout: ::wgpu::PipelineLayout,
        pub pipeline: ::wgpu::ComputePipeline,
        pub out_format: CloudShadowStorageFormat,
    }

    impl CloudShadowProjectPipeline {
        #[must_use]
        pub fn create(device: &::wgpu::Device, out_format: CloudShadowStorageFormat) -> Self {
            let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
                label: Some("fun_renderer.cloud_shadow_project.wgsl"),
                source: ::wgpu::ShaderSource::Wgsl(CLOUD_SHADOW_PROJECT_WGSL.into()),
            });
            let bind_group_layout =
                device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                    label: Some("fun_renderer.cloud_shadow_project.bgl"),
                    entries: &cloud_shadow_project_bind_group_layout_entries(out_format),
                });
            let pipeline_layout =
                device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
                    label: Some("fun_renderer.cloud_shadow_project.layout"),
                    bind_group_layouts: &[Some(&bind_group_layout)],
                    immediate_size: 0,
                });
            let pipeline = device.create_compute_pipeline(&::wgpu::ComputePipelineDescriptor {
                label: Some("fun_renderer.cloud_shadow_project.pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(CLOUD_SHADOW_PROJECT_ENTRY_POINT),
                compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            Self {
                schema_version: FUN_RENDERER_CLOUD_SHADOW_PIPELINES_SCHEMA_VERSION,
                shader,
                bind_group_layout,
                pipeline_layout,
                pipeline,
                out_format,
            }
        }
    }

    /// Typed Pass C7.5 — wgpu pipeline + bind group layout
    /// for the typed `cloud_shadow_filter.wgsl` compute
    /// shader.
    pub struct CloudShadowFilterPipeline {
        pub schema_version: u16,
        pub shader: ::wgpu::ShaderModule,
        pub bind_group_layout: ::wgpu::BindGroupLayout,
        pub pipeline_layout: ::wgpu::PipelineLayout,
        pub pipeline: ::wgpu::ComputePipeline,
        pub out_format: CloudShadowStorageFormat,
    }

    impl CloudShadowFilterPipeline {
        #[must_use]
        pub fn create(device: &::wgpu::Device, out_format: CloudShadowStorageFormat) -> Self {
            let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
                label: Some("fun_renderer.cloud_shadow_filter.wgsl"),
                source: ::wgpu::ShaderSource::Wgsl(CLOUD_SHADOW_FILTER_WGSL.into()),
            });
            let bind_group_layout =
                device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                    label: Some("fun_renderer.cloud_shadow_filter.bgl"),
                    entries: &cloud_shadow_filter_bind_group_layout_entries(out_format),
                });
            let pipeline_layout =
                device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
                    label: Some("fun_renderer.cloud_shadow_filter.layout"),
                    bind_group_layouts: &[Some(&bind_group_layout)],
                    immediate_size: 0,
                });
            let pipeline = device.create_compute_pipeline(&::wgpu::ComputePipelineDescriptor {
                label: Some("fun_renderer.cloud_shadow_filter.pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(CLOUD_SHADOW_FILTER_ENTRY_POINT),
                compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });
            Self {
                schema_version: FUN_RENDERER_CLOUD_SHADOW_PIPELINES_SCHEMA_VERSION,
                shader,
                bind_group_layout,
                pipeline_layout,
                pipeline,
                out_format,
            }
        }
    }

    /// Typed Pass C7.5 — wgpu resources the typed cloud
    /// shadow pipeline allocates: typed transmittance
    /// target, typed filtered target, typed projection
    /// constants uniform buffer.
    ///
    /// The typed weather + shape-noise textures are NOT
    /// owned by this typed bundle — they are sourced from
    /// the typed cloud raymarch's persistent texture set
    /// (currently in the typed `fun_render::sky` legacy
    /// path; will be migrated under the typed C7.x cloud
    /// ownership move).  Callers bind their typed external
    /// weather + shape views through the typed
    /// [`create_project_bind_group`] helper.
    pub struct CloudShadowGpuResources {
        pub schema_version: u16,
        pub extent: [u32; 2],
        pub format: CloudShadowStorageFormat,
        pub transmittance_texture: ::wgpu::Texture,
        pub transmittance_view: ::wgpu::TextureView,
        pub filtered_texture: ::wgpu::Texture,
        pub filtered_view: ::wgpu::TextureView,
        pub projection_constants_buffer: ::wgpu::Buffer,
    }

    impl CloudShadowGpuResources {
        #[must_use]
        pub fn create(
            device: &::wgpu::Device,
            extent: [u32; 2],
            format: CloudShadowStorageFormat,
        ) -> Self {
            let wgpu_format = cloud_shadow_storage_format_to_wgpu(format);
            let texture_size = ::wgpu::Extent3d {
                width: extent[0],
                height: extent[1],
                depth_or_array_layers: 1,
            };
            let usage = ::wgpu::TextureUsages::STORAGE_BINDING
                | ::wgpu::TextureUsages::TEXTURE_BINDING
                | ::wgpu::TextureUsages::COPY_SRC;
            let transmittance_texture = device.create_texture(&::wgpu::TextureDescriptor {
                label: Some("fun_renderer.cloud_world_shadow_transmittance"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: ::wgpu::TextureDimension::D2,
                format: wgpu_format,
                usage,
                view_formats: &[],
            });
            let transmittance_view =
                transmittance_texture.create_view(&::wgpu::TextureViewDescriptor::default());
            let filtered_texture = device.create_texture(&::wgpu::TextureDescriptor {
                label: Some("fun_renderer.cloud_world_shadow_filtered"),
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: ::wgpu::TextureDimension::D2,
                format: wgpu_format,
                usage,
                view_formats: &[],
            });
            let filtered_view =
                filtered_texture.create_view(&::wgpu::TextureViewDescriptor::default());
            let projection_constants_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
                label: Some("fun_renderer.cloud_shadow_projection_constants"),
                size: CLOUD_SHADOW_PROJECTION_CONSTANTS_GPU_BYTES,
                usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            Self {
                schema_version: FUN_RENDERER_CLOUD_SHADOW_PIPELINES_SCHEMA_VERSION,
                extent,
                format,
                transmittance_texture,
                transmittance_view,
                filtered_texture,
                filtered_view,
                projection_constants_buffer,
            }
        }

        /// Typed Pass C7.5 — upload the typed projection
        /// constants to the typed uniform buffer.  Encodes
        /// the typed CPU `CloudShadowProjectionConstants`
        /// into the typed std140 GPU layout via
        /// `CloudShadowProjectionConstantsGpu::from_cpu` +
        /// the typed safe `to_bytes` serializer.
        pub fn upload_projection_constants(
            &self,
            queue: &::wgpu::Queue,
            constants: &super::CloudShadowProjectionConstants,
        ) {
            let gpu = CloudShadowProjectionConstantsGpu::from_cpu(constants);
            let bytes = gpu.to_bytes();
            queue.write_buffer(&self.projection_constants_buffer, 0, &bytes);
        }
    }

    /// Typed Pass C7.5 — create the typed bind group the
    /// typed `LuxCloudShadowProject` dispatch binds.
    /// Caller supplies the typed cloud_params uniform +
    /// the typed weather + shape-noise views (sourced from
    /// the typed cloud raymarch's persistent textures).
    #[must_use]
    pub fn create_project_bind_group(
        device: &::wgpu::Device,
        pipeline: &CloudShadowProjectPipeline,
        resources: &CloudShadowGpuResources,
        cloud_params_buffer: &::wgpu::Buffer,
        weather_map_view: &::wgpu::TextureView,
        shape_noise_view: &::wgpu::TextureView,
    ) -> ::wgpu::BindGroup {
        device.create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.cloud_shadow_project.bind_group"),
            layout: &pipeline.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cloud_params_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: resources.projection_constants_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ::wgpu::BindingResource::TextureView(weather_map_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 3,
                    resource: ::wgpu::BindingResource::TextureView(shape_noise_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 4,
                    resource: ::wgpu::BindingResource::TextureView(&resources.transmittance_view),
                },
            ],
        })
    }

    /// Typed Pass C7.5 — create the typed bind group the
    /// typed `LuxCloudShadowFilter` dispatch binds.
    #[must_use]
    pub fn create_filter_bind_group(
        device: &::wgpu::Device,
        pipeline: &CloudShadowFilterPipeline,
        resources: &CloudShadowGpuResources,
    ) -> ::wgpu::BindGroup {
        device.create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.cloud_shadow_filter.bind_group"),
            layout: &pipeline.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: resources.projection_constants_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: ::wgpu::BindingResource::TextureView(&resources.transmittance_view),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: ::wgpu::BindingResource::TextureView(&resources.filtered_view),
                },
            ],
        })
    }

    /// Typed Pass C7.5 — record the typed
    /// `LuxCloudShadowProject` dispatch into a typed wgpu
    /// command encoder.  Workgroup count =
    /// `ceil(extent / 8)` per dimension.
    pub fn record_dispatch_cloud_shadow_project(
        encoder: &mut ::wgpu::CommandEncoder,
        pipeline: &CloudShadowProjectPipeline,
        bind_group: &::wgpu::BindGroup,
        extent: [u32; 2],
    ) {
        let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
            label: Some("fun_renderer.cloud_shadow_project.dispatch"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        let [x, y, z] = cloud_shadow_workgroup_count(extent);
        pass.dispatch_workgroups(x, y, z);
    }

    /// Typed Pass C7.5 — record the typed
    /// `LuxCloudShadowFilter` dispatch into a typed wgpu
    /// command encoder.
    pub fn record_dispatch_cloud_shadow_filter(
        encoder: &mut ::wgpu::CommandEncoder,
        pipeline: &CloudShadowFilterPipeline,
        bind_group: &::wgpu::BindGroup,
        extent: [u32; 2],
    ) {
        let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
            label: Some("fun_renderer.cloud_shadow_filter.dispatch"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        let [x, y, z] = cloud_shadow_workgroup_count(extent);
        pass.dispatch_workgroups(x, y, z);
    }

    /// Typed Pass C7.5 — record the typed full cloud-shadow
    /// chain (`Project` → `Filter`) into a typed wgpu
    /// command encoder.  Returns the typed dispatch counts
    /// for diagnostics.
    pub fn record_dispatch_cloud_shadow_chain(
        encoder: &mut ::wgpu::CommandEncoder,
        project_pipeline: &CloudShadowProjectPipeline,
        filter_pipeline: &CloudShadowFilterPipeline,
        project_bind_group: &::wgpu::BindGroup,
        filter_bind_group: &::wgpu::BindGroup,
        extent: [u32; 2],
    ) -> CloudShadowDispatchCounts {
        record_dispatch_cloud_shadow_project(encoder, project_pipeline, project_bind_group, extent);
        record_dispatch_cloud_shadow_filter(encoder, filter_pipeline, filter_bind_group, extent);
        CloudShadowDispatchCounts {
            schema_version: FUN_RENDERER_CLOUD_SHADOW_PIPELINES_SCHEMA_VERSION,
            project_dispatches: 1,
            filter_dispatches: 1,
            workgroup_count: cloud_shadow_workgroup_count(extent),
        }
    }

    /// Typed Pass C7.5 dispatch counters.  Surfaced to the
    /// typed renderer diagnostics so the typed cloud
    /// shadow pipeline reports typed "Project: 1 dispatch,
    /// Filter: 1 dispatch".
    #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct CloudShadowDispatchCounts {
        pub schema_version: u16,
        pub project_dispatches: u32,
        pub filter_dispatches: u32,
        pub workgroup_count: [u32; 3],
    }

    impl CloudShadowDispatchCounts {
        /// Typed predicate: did the typed full chain run
        /// this frame?
        #[must_use]
        pub const fn full_chain_dispatched(&self) -> bool {
            self.project_dispatches > 0 && self.filter_dispatches > 0
        }
    }
}

#[cfg(feature = "wgpu_bridge")]
pub use wgpu_bridge::{
    CloudShadowDispatchCounts, CloudShadowFilterPipeline, CloudShadowGpuResources,
    CloudShadowProjectPipeline, cloud_shadow_filter_bind_group_layout_entries,
    cloud_shadow_project_bind_group_layout_entries, create_filter_bind_group,
    create_project_bind_group, record_dispatch_cloud_shadow_chain,
    record_dispatch_cloud_shadow_filter, record_dispatch_cloud_shadow_project,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clouds::{CloudRenderSettings, CloudWeatherProfileId};
    use fun_lux::LuxLightId;

    fn live_constants() -> CloudShadowProjectionConstants {
        CloudShadowProjectionConstants::from_inputs(
            &CloudRenderSettings::PRODUCT_DEFAULT,
            CloudWeatherProfileId::Scattered,
            LuxLightId::new(42),
            [0.0, 1.0, 0.0],
            17,
        )
    }

    /// Pass C7.5 acceptance — typed CPU bytes + typed GPU
    /// bytes both report stable layout.
    #[test]
    fn schema_versions_are_stable() {
        assert_eq!(FUN_RENDERER_CLOUD_SHADOW_PIPELINES_SCHEMA_VERSION, 1);
        // Typed CPU + GPU footprints are non-zero.
        assert!(CLOUD_SHADOW_PROJECTION_CONSTANTS_CPU_BYTES > 0);
        assert!(CLOUD_SHADOW_PROJECTION_CONSTANTS_GPU_BYTES > 0);
        // Typed GPU layout = typed 16 (header) + 64 + 64 +
        // 16 + 16 + 16 = 192 bytes.
        assert_eq!(CLOUD_SHADOW_PROJECTION_CONSTANTS_GPU_BYTES, 192);
    }

    /// Pass C7.5 acceptance — typed workgroup count is
    /// `ceil(extent / 8)` per dimension.
    #[test]
    fn workgroup_count_walks_user_spec() {
        assert_eq!(cloud_shadow_workgroup_count([1024, 1024]), [128, 128, 1]);
        assert_eq!(cloud_shadow_workgroup_count([2048, 2048]), [256, 256, 1]);
        assert_eq!(cloud_shadow_workgroup_count([4096, 4096]), [512, 512, 1]);
        // Typed odd extent rounds up — typed `(7+7)/8 = 1`,
        // typed `(9+7)/8 = 2`.
        assert_eq!(cloud_shadow_workgroup_count([7, 9]), [1, 2, 1]);
        // Typed extent 0 produces typed 0 workgroups —
        // typed renderer must gate at extent==0 elsewhere.
        assert_eq!(cloud_shadow_workgroup_count([0, 0]), [0, 0, 1]);
    }

    /// Pass C7.5 acceptance — typed resolution extent
    /// mirrors `CloudShadowResolution::pixel_extent`.
    #[test]
    fn resolution_extent_walks_user_spec() {
        assert_eq!(
            cloud_shadow_resolution_extent(CloudShadowResolution::Cheap1024),
            [1024, 1024],
        );
        assert_eq!(
            cloud_shadow_resolution_extent(CloudShadowResolution::Balanced2048),
            [2048, 2048],
        );
        assert_eq!(
            cloud_shadow_resolution_extent(CloudShadowResolution::Cinematic4096),
            [4096, 4096],
        );
    }

    /// Pass C7.5 acceptance — typed GPU constants builder
    /// packs every typed CPU field into the typed std140
    /// layout.
    #[test]
    fn gpu_constants_builder_packs_cpu_fields() {
        let cpu = live_constants();
        let gpu = CloudShadowProjectionConstantsGpu::from_cpu(&cpu);
        // Typed header packing.
        assert_eq!(gpu.schema_version(), cpu.schema_version);
        assert_eq!(gpu.mode_index(), 0); // CameraCenteredPlane = 0
        assert_eq!(gpu.light_id_u64(), cpu.light_id.0);
        assert_eq!(gpu.frame_index(), cpu.frame_index);
        // Typed sun_dir_ws.xyz carries; .w = 0.
        assert_eq!(gpu.sun_dir_ws[0], cpu.sun_direction_ws[0]);
        assert_eq!(gpu.sun_dir_ws[1], cpu.sun_direction_ws[1]);
        assert_eq!(gpu.sun_dir_ws[2], cpu.sun_direction_ws[2]);
        assert_eq!(gpu.sun_dir_ws[3], 0.0);
        // Typed slab encoding.
        assert_eq!(gpu.slab[0], cpu.cloud_base_meters);
        assert_eq!(gpu.slab[1], cpu.cloud_top_meters);
        assert_eq!(gpu.slab[2], cpu.max_distance_meters);
        assert_eq!(gpu.slab[3], 0.0);
        // Typed knobs encoding.
        assert_eq!(gpu.knobs[0], cpu.opacity_scale);
        assert_eq!(gpu.knobs[1], cpu.softness);
        // Typed matrices are typed transposed (column-major
        // for typed WGSL).  Typed CPU row 0 maps to typed
        // GPU column 0.
        for row in 0..4 {
            for col in 0..4 {
                assert_eq!(
                    gpu.world_from_shadow_uv[col][row],
                    cpu.world_from_shadow_uv[row][col],
                );
                assert_eq!(
                    gpu.shadow_uv_from_world[col][row],
                    cpu.shadow_uv_from_world[row][col],
                );
            }
        }
    }

    /// Pass C7.5 acceptance — typed projection mode index
    /// table.
    #[test]
    fn projection_mode_index_walks_taxonomy() {
        assert_eq!(
            cloud_shadow_projection_mode_index(CloudShadowProjectionMode::CameraCenteredPlane),
            0,
        );
        assert_eq!(
            cloud_shadow_projection_mode_index(
                CloudShadowProjectionMode::DirectionalLightClipRegion,
            ),
            1,
        );
        assert_eq!(
            cloud_shadow_projection_mode_index(
                CloudShadowProjectionMode::CascadedDirectionalRegions,
            ),
            2,
        );
    }

    /// Pass C7.5 acceptance — typed wgpu format mapping
    /// walks the typed user-spec table.
    #[cfg(feature = "wgpu_bridge")]
    #[test]
    fn cloud_shadow_storage_format_to_wgpu_walks_user_spec() {
        assert_eq!(
            cloud_shadow_storage_format_to_wgpu(CloudShadowStorageFormat::R8Unorm),
            ::wgpu::TextureFormat::R8Unorm,
        );
        assert_eq!(
            cloud_shadow_storage_format_to_wgpu(CloudShadowStorageFormat::R16Float),
            ::wgpu::TextureFormat::R16Float,
        );
        assert_eq!(
            cloud_shadow_storage_format_to_wgpu(CloudShadowStorageFormat::Rgba16FloatPacked),
            ::wgpu::TextureFormat::Rgba16Float,
        );
    }

    /// Pass C7.5 acceptance — typed bind-group layout
    /// entries for the typed project pass match the user
    /// spec (5 bindings: cloud + projection + weather +
    /// shape + storage out).
    #[cfg(feature = "wgpu_bridge")]
    #[test]
    fn project_bind_group_layout_entries_match_user_spec() {
        let entries =
            cloud_shadow_project_bind_group_layout_entries(CloudShadowStorageFormat::R16Float);
        assert_eq!(entries.len(), 5);
        // Typed binding 0: cloud uniform.
        assert_eq!(entries[0].binding, 0);
        assert!(matches!(
            entries[0].ty,
            ::wgpu::BindingType::Buffer {
                ty: ::wgpu::BufferBindingType::Uniform,
                ..
            },
        ));
        // Typed binding 1: projection uniform.
        assert_eq!(entries[1].binding, 1);
        // Typed binding 2: weather_map 2D texture.
        assert_eq!(entries[2].binding, 2);
        assert!(matches!(
            entries[2].ty,
            ::wgpu::BindingType::Texture {
                view_dimension: ::wgpu::TextureViewDimension::D2,
                ..
            },
        ));
        // Typed binding 3: shape_noise 3D texture.
        assert_eq!(entries[3].binding, 3);
        assert!(matches!(
            entries[3].ty,
            ::wgpu::BindingType::Texture {
                view_dimension: ::wgpu::TextureViewDimension::D3,
                ..
            },
        ));
        // Typed binding 4: storage texture, write-only,
        // R16Float.
        assert_eq!(entries[4].binding, 4);
        assert!(matches!(
            entries[4].ty,
            ::wgpu::BindingType::StorageTexture {
                access: ::wgpu::StorageTextureAccess::WriteOnly,
                format: ::wgpu::TextureFormat::R16Float,
                view_dimension: ::wgpu::TextureViewDimension::D2,
            },
        ));
        // Typed every binding is typed compute-visible.
        for entry in &entries {
            assert_eq!(entry.visibility, ::wgpu::ShaderStages::COMPUTE);
        }
    }

    /// Pass C7.5 acceptance — typed bind-group layout
    /// entries for the typed filter pass match the user
    /// spec (3 bindings: projection + input + storage out).
    #[cfg(feature = "wgpu_bridge")]
    #[test]
    fn filter_bind_group_layout_entries_match_user_spec() {
        let entries =
            cloud_shadow_filter_bind_group_layout_entries(CloudShadowStorageFormat::R16Float);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].binding, 0);
        assert!(matches!(
            entries[0].ty,
            ::wgpu::BindingType::Buffer {
                ty: ::wgpu::BufferBindingType::Uniform,
                ..
            },
        ));
        assert_eq!(entries[1].binding, 1);
        assert!(matches!(
            entries[1].ty,
            ::wgpu::BindingType::Texture {
                view_dimension: ::wgpu::TextureViewDimension::D2,
                ..
            },
        ));
        assert_eq!(entries[2].binding, 2);
        assert!(matches!(
            entries[2].ty,
            ::wgpu::BindingType::StorageTexture {
                access: ::wgpu::StorageTextureAccess::WriteOnly,
                format: ::wgpu::TextureFormat::R16Float,
                view_dimension: ::wgpu::TextureViewDimension::D2,
            },
        ));
    }
}
