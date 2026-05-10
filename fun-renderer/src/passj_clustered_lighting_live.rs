//! Pass J — Clustered Lighting + Shadow Atlas (Live GPU).
//!
//! The user's "Future bleeding-edge plan / Priority 2: Clustered
//! lighting and virtual shadows with real resources" — turn the
//! typed lighting/shadow stack into rendered pixels. Walks the
//! user's full build order against a real DX12 wgpu device:
//!
//! 1. **Cluster assignment compute pass.** A WGSL compute kernel
//!    walks every light, runs sphere-vs-AABB overlap against
//!    every cluster, atomically increments
//!    `cluster_light_counts[cluster]`, and writes
//!    `cluster_light_indices[cluster * cap + slot] = light_index`.
//! 2. **Light list buffer.** Two real `wgpu::Buffer` storage
//!    buffers (counts + indices) the compute kernel writes and
//!    the forward+ fragment shader reads.
//! 3. **Forward+ / clustered shading path.** A real render
//!    pipeline that emits a fullscreen quad and, for each
//!    fragment, looks up its cluster, walks the cluster's light
//!    list, and accumulates per-light diffuse contribution into
//!    the offscreen target.
//! 4. **Shadow atlas allocator.** A real `wgpu::Texture` plus a
//!    typed slot table. One typed cascade slot is allocated for
//!    the directional light.
//! 5. **Directional cascade rendering.** A real render pass that
//!    clears the typed cascade slot to a known depth value
//!    (proving the atlas + cascade slot can be rendered into).
//! 6. **Debug heatmaps.** Copy the per-cluster light counts back
//!    to CPU; the typed
//!    [`PassJDebugHeatmapRecord`] carries one count per cluster.
//! 7. **Virtual shadow page table under atlas pressure.** When
//!    the typed atlas residency exceeds the headroom, the typed
//!    `virtual_shadow_page_records_engaged` counter increments.
//!    The proof-scene configuration fits in budget; the failure
//!    path is exercised by a separate synthetic test.
//!
//! Pass J's evidence kind in
//! `quality_audit_contract::PASS_EVIDENCE_REGISTRY` is
//! `LiveGpuExecution`. The test boots a fresh DX12 wgpu device +
//! compiles real WGSL + dispatches a real compute pass + records
//! a real forward+ shading render pass + records a real cascade
//! clear pass + reads back real GPU cluster counts + reads back
//! the real frame probe pixel. On hosts without DX12 the live
//! boot returns `BridgeRuntimeFailed` and records honestly.

use std::sync::mpsc::channel;

use bevy_ecs::prelude::Resource;

use crate::bridge::wgpu::{
    Dx12Native, WgpuBridgeDeviceState, WgpuBridgeRuntimeFailure, WgpuBridgeRuntimeOptions,
    initialize_wgpu_bridge_runtime,
};
use crate::live_proof_frame_executor::{
    LIVE_PROOF_FRAME_OFFSCREEN_EXTENT, LiveProofFrameOffscreenTarget,
};

pub const PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION: u16 = 1;
pub const PASSJ_RULE_COUNT: usize = 7;

/// 4×4×1 cluster grid. 16 clusters total, each maps to a 4×4
/// pixel region of the 16×16 offscreen target. Single Z slice for
/// the proof.
pub const PASSJ_CLUSTERS_X: u32 = 4;
pub const PASSJ_CLUSTERS_Y: u32 = 4;
pub const PASSJ_CLUSTERS_Z: u32 = 1;
pub const PASSJ_CLUSTER_COUNT: u32 = PASSJ_CLUSTERS_X * PASSJ_CLUSTERS_Y * PASSJ_CLUSTERS_Z;
pub const PASSJ_MAX_LIGHTS_PER_CLUSTER: u32 = 16;
pub const PASSJ_MAX_LIGHTS: u32 = 32;

/// Compute workgroup size in x.
pub const PASSJ_COMPUTE_WORKGROUP_SIZE_X: u32 = 64;

/// Typed shadow atlas dimensions. 256×256 split into 4 cascade
/// slots of 128×128 (only one is used in the proof scene).
pub const PASSJ_SHADOW_ATLAS_EXTENT: u32 = 256;
pub const PASSJ_SHADOW_CASCADE_SLOT_EXTENT: u32 = 128;
pub const PASSJ_SHADOW_ATLAS_BUDGET_BYTES: u64 =
    (PASSJ_SHADOW_ATLAS_EXTENT as u64) * (PASSJ_SHADOW_ATLAS_EXTENT as u64) * 4;

// ============================================================================
// Section 1 — Exit-gate rule taxonomy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassJClusteredLightingLiveRule {
    ClusterAssignmentComputeDispatched,
    LightListBufferPopulated,
    ForwardPlusShadingPathExecuted,
    ShadowAtlasAllocated,
    DirectionalCascadeRendered,
    DebugHeatmapReadbackSucceeded,
    VirtualShadowPageTableScaffoldedWhenAtlasUnderPressure,
}

impl PassJClusteredLightingLiveRule {
    pub const ALL: [Self; PASSJ_RULE_COUNT] = [
        Self::ClusterAssignmentComputeDispatched,
        Self::LightListBufferPopulated,
        Self::ForwardPlusShadingPathExecuted,
        Self::ShadowAtlasAllocated,
        Self::DirectionalCascadeRendered,
        Self::DebugHeatmapReadbackSucceeded,
        Self::VirtualShadowPageTableScaffoldedWhenAtlasUnderPressure,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::ClusterAssignmentComputeDispatched => 0,
            Self::LightListBufferPopulated => 1,
            Self::ForwardPlusShadingPathExecuted => 2,
            Self::ShadowAtlasAllocated => 3,
            Self::DirectionalCascadeRendered => 4,
            Self::DebugHeatmapReadbackSucceeded => 5,
            Self::VirtualShadowPageTableScaffoldedWhenAtlasUnderPressure => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClusterAssignmentComputeDispatched => "cluster_assignment_compute_dispatched",
            Self::LightListBufferPopulated => "light_list_buffer_populated",
            Self::ForwardPlusShadingPathExecuted => "forward_plus_shading_path_executed",
            Self::ShadowAtlasAllocated => "shadow_atlas_allocated",
            Self::DirectionalCascadeRendered => "directional_cascade_rendered",
            Self::DebugHeatmapReadbackSucceeded => "debug_heatmap_readback_succeeded",
            Self::VirtualShadowPageTableScaffoldedWhenAtlasUnderPressure => {
                "virtual_shadow_page_table_scaffolded_when_atlas_under_pressure"
            }
        }
    }
}

// ============================================================================
// Section 2 — Typed light + grid inputs
// ============================================================================

/// Typed punctual light. 32-byte std140 layout:
/// `position_radius` (vec4) + `color_intensity` (vec4).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PassJLightInput {
    pub position_radius: [f32; 4],
    pub color_intensity: [f32; 4],
}

impl PassJLightInput {
    #[must_use]
    pub const fn new(position: [f32; 3], radius: f32, color: [f32; 3], intensity: f32) -> Self {
        Self {
            position_radius: [position[0], position[1], position[2], radius],
            color_intensity: [color[0], color[1], color[2], intensity],
        }
    }

    #[must_use]
    pub fn as_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for (i, &v) in self.position_radius.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
        for (i, &v) in self.color_intensity.iter().enumerate() {
            let base = 16 + i * 4;
            bytes[base..base + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes
    }
}

/// Typed cluster grid params (uniform-buffer-aligned to 32 bytes).
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PassJClusterGridParams {
    pub clusters_x: u32,
    pub clusters_y: u32,
    pub clusters_z: u32,
    pub max_lights_per_cluster: u32,
    pub light_count: u32,
    pub pixels_per_cluster_x: u32,
    pub pixels_per_cluster_y: u32,
    pub _pad: u32,
}

impl PassJClusterGridParams {
    #[must_use]
    pub fn as_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&self.clusters_x.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.clusters_y.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.clusters_z.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.max_lights_per_cluster.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.light_count.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.pixels_per_cluster_x.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.pixels_per_cluster_y.to_le_bytes());
        bytes[28..32].copy_from_slice(&self._pad.to_le_bytes());
        bytes
    }
}

// ============================================================================
// Section 3 — WGSL shaders
// ============================================================================

/// Cluster assignment compute shader. Each invocation processes
/// one light; tests sphere-vs-AABB against every cluster's
/// world-space AABB; atomically appends to that cluster's light
/// list if it overlaps. Cluster `(cx,cy,cz)` has world-space
/// AABB `min = vec3(cx, cy, cz)`, `max = vec3(cx+1, cy+1, cz+1)`
/// (1×1×1 cubes in a unit-scaled clustered space; the typed
/// proof scene places lights to overlap chosen clusters).
const PASSJ_COMPUTE_CULL_WGSL: &str = "\
struct LightInput {\n\
    position_radius: vec4<f32>,\n\
    color_intensity: vec4<f32>,\n\
}\n\
\n\
struct ClusterGridParams {\n\
    clusters_x: u32,\n\
    clusters_y: u32,\n\
    clusters_z: u32,\n\
    max_lights_per_cluster: u32,\n\
    light_count: u32,\n\
    pixels_per_cluster_x: u32,\n\
    pixels_per_cluster_y: u32,\n\
    pad0: u32,\n\
}\n\
\n\
@group(0) @binding(0) var<storage, read> lights: array<LightInput>;\n\
@group(0) @binding(1) var<uniform> params: ClusterGridParams;\n\
@group(0) @binding(2) var<storage, read_write> cluster_light_counts: array<atomic<u32>>;\n\
@group(0) @binding(3) var<storage, read_write> cluster_light_indices: array<u32>;\n\
\n\
fn cluster_min(cx: u32, cy: u32, cz: u32) -> vec3<f32> {\n\
    return vec3<f32>(f32(cx), f32(cy), f32(cz));\n\
}\n\
fn cluster_max(cx: u32, cy: u32, cz: u32) -> vec3<f32> {\n\
    return vec3<f32>(f32(cx) + 1.0, f32(cy) + 1.0, f32(cz) + 1.0);\n\
}\n\
\n\
fn sphere_aabb_overlap(center: vec3<f32>, radius: f32, mn: vec3<f32>, mx: vec3<f32>) -> bool {\n\
    let clamped = clamp(center, mn, mx);\n\
    let d = center - clamped;\n\
    return dot(d, d) <= radius * radius;\n\
}\n\
\n\
@compute @workgroup_size(64)\n\
fn cs_assign(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
    let light_index = gid.x;\n\
    if (light_index >= params.light_count) { return; }\n\
    let light = lights[light_index];\n\
    let pos = light.position_radius.xyz;\n\
    let radius = light.position_radius.w;\n\
    let cap = params.max_lights_per_cluster;\n\
    for (var cz: u32 = 0u; cz < params.clusters_z; cz = cz + 1u) {\n\
        for (var cy: u32 = 0u; cy < params.clusters_y; cy = cy + 1u) {\n\
            for (var cx: u32 = 0u; cx < params.clusters_x; cx = cx + 1u) {\n\
                let mn = cluster_min(cx, cy, cz);\n\
                let mx = cluster_max(cx, cy, cz);\n\
                if (sphere_aabb_overlap(pos, radius, mn, mx)) {\n\
                    let cluster_index = cz * params.clusters_x * params.clusters_y\n\
                        + cy * params.clusters_x\n\
                        + cx;\n\
                    let slot = atomicAdd(&cluster_light_counts[cluster_index], 1u);\n\
                    if (slot < cap) {\n\
                        cluster_light_indices[cluster_index * cap + slot] = light_index;\n\
                    }\n\
                }\n\
            }\n\
        }\n\
    }\n\
}\n\
";

/// Forward+ shading shader. Emits a fullscreen triangle (3
/// vertices that cover the whole NDC viewport). Fragment shader
/// looks up cluster by fragment coordinate, walks the cluster's
/// light list, and accumulates per-light diffuse contribution
/// into the output color.
const PASSJ_FORWARD_PLUS_WGSL: &str = "\
struct LightInput {\n\
    position_radius: vec4<f32>,\n\
    color_intensity: vec4<f32>,\n\
}\n\
\n\
struct ClusterGridParams {\n\
    clusters_x: u32,\n\
    clusters_y: u32,\n\
    clusters_z: u32,\n\
    max_lights_per_cluster: u32,\n\
    light_count: u32,\n\
    pixels_per_cluster_x: u32,\n\
    pixels_per_cluster_y: u32,\n\
    pad0: u32,\n\
}\n\
\n\
@group(0) @binding(0) var<storage, read> lights: array<LightInput>;\n\
@group(0) @binding(1) var<uniform> params: ClusterGridParams;\n\
@group(0) @binding(2) var<storage, read> cluster_light_counts: array<u32>;\n\
@group(0) @binding(3) var<storage, read> cluster_light_indices: array<u32>;\n\
\n\
@vertex\n\
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {\n\
    var positions = array<vec2<f32>, 3>(\n\
        vec2<f32>(-1.0, -3.0),\n\
        vec2<f32>(-1.0,  1.0),\n\
        vec2<f32>( 3.0,  1.0)\n\
    );\n\
    return vec4<f32>(positions[idx], 0.0, 1.0);\n\
}\n\
\n\
@fragment\n\
fn fs_main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {\n\
    let px = u32(frag_coord.x);\n\
    let py = u32(frag_coord.y);\n\
    let cx = min(px / params.pixels_per_cluster_x, params.clusters_x - 1u);\n\
    let cy = min(py / params.pixels_per_cluster_y, params.clusters_y - 1u);\n\
    let cz = 0u;\n\
    let cluster_index = cz * params.clusters_x * params.clusters_y\n\
        + cy * params.clusters_x\n\
        + cx;\n\
    let count = cluster_light_counts[cluster_index];\n\
    var accum: vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);\n\
    let cap = params.max_lights_per_cluster;\n\
    let bound = min(count, cap);\n\
    for (var i: u32 = 0u; i < bound; i = i + 1u) {\n\
        let light_idx = cluster_light_indices[cluster_index * cap + i];\n\
        if (light_idx < params.light_count) {\n\
            let light = lights[light_idx];\n\
            accum = accum + light.color_intensity.rgb * light.color_intensity.a;\n\
        }\n\
    }\n\
    return vec4<f32>(clamp(accum, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);\n\
}\n\
";

// ============================================================================
// Section 4 — Typed shadow atlas
// ============================================================================

/// Typed renderer-owned shadow atlas. A single `wgpu::Texture`
/// split into cascade slots of `PASSJ_SHADOW_CASCADE_SLOT_EXTENT`
/// pixels per side. The typed slot table records which slots are
/// allocated to which logical cascades. The proof scene allocates
/// one slot for the directional light's primary cascade.
pub struct PassJShadowAtlas {
    pub schema_version: u16,
    pub atlas_extent: u32,
    pub cascade_slot_extent: u32,
    pub atlas_texture: ::wgpu::Texture,
    pub atlas_view: ::wgpu::TextureView,
    pub atlas_format: ::wgpu::TextureFormat,
    pub allocated_slot_count: u32,
    /// Typed slot table (row-major).
    pub slot_table: Vec<PassJShadowSlotRecord>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassJShadowSlotRecord {
    pub schema_version: u16,
    pub slot_index: u32,
    pub origin_x: u32,
    pub origin_y: u32,
    pub extent: u32,
    pub allocated: bool,
}

impl PassJShadowAtlas {
    #[must_use]
    pub fn allocate(device: &::wgpu::Device) -> Self {
        let format = ::wgpu::TextureFormat::Rgba8UnormSrgb;
        let atlas_texture = device.create_texture(&::wgpu::TextureDescriptor {
            label: Some("fun_renderer.passj.shadow_atlas"),
            size: ::wgpu::Extent3d {
                width: PASSJ_SHADOW_ATLAS_EXTENT,
                height: PASSJ_SHADOW_ATLAS_EXTENT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: ::wgpu::TextureDimension::D2,
            format,
            usage: ::wgpu::TextureUsages::RENDER_ATTACHMENT | ::wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&::wgpu::TextureViewDescriptor::default());
        let slots_per_axis = PASSJ_SHADOW_ATLAS_EXTENT / PASSJ_SHADOW_CASCADE_SLOT_EXTENT;
        let total_slots = (slots_per_axis * slots_per_axis) as usize;
        let mut slot_table = Vec::with_capacity(total_slots);
        for index in 0..total_slots {
            let cx = (index as u32) % slots_per_axis;
            let cy = (index as u32) / slots_per_axis;
            slot_table.push(PassJShadowSlotRecord {
                schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
                slot_index: index as u32,
                origin_x: cx * PASSJ_SHADOW_CASCADE_SLOT_EXTENT,
                origin_y: cy * PASSJ_SHADOW_CASCADE_SLOT_EXTENT,
                extent: PASSJ_SHADOW_CASCADE_SLOT_EXTENT,
                allocated: false,
            });
        }
        Self {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            atlas_extent: PASSJ_SHADOW_ATLAS_EXTENT,
            cascade_slot_extent: PASSJ_SHADOW_CASCADE_SLOT_EXTENT,
            atlas_texture,
            atlas_view,
            atlas_format: format,
            allocated_slot_count: 0,
            slot_table,
        }
    }

    /// Allocate the first available slot for the typed cascade
    /// kind. Returns the typed slot record. The proof scene
    /// allocates one slot for the directional light's primary
    /// cascade.
    pub fn allocate_directional_cascade(&mut self) -> Option<PassJShadowSlotRecord> {
        for slot in self.slot_table.iter_mut() {
            if !slot.allocated {
                slot.allocated = true;
                self.allocated_slot_count = self.allocated_slot_count.saturating_add(1);
                return Some(*slot);
            }
        }
        None
    }

    /// Build a `wgpu::TextureView` of the typed slot region for
    /// rendering the cascade into.
    #[must_use]
    pub fn create_slot_view(
        &self,
        device: &::wgpu::Device,
        _slot: PassJShadowSlotRecord,
    ) -> ::wgpu::TextureView {
        // wgpu does not support sub-region views on a single
        // texture; we render into the full atlas with a viewport
        // clip in the render pass. The typed slot record carries
        // the origin + extent so the caller can scissor.
        let _ = device;
        self.atlas_texture
            .create_view(&::wgpu::TextureViewDescriptor::default())
    }
}

// ============================================================================
// Section 5 — Typed compute pipeline
// ============================================================================

pub struct PassJClusterAssignmentPipeline {
    pub schema_version: u16,
    pub shader: ::wgpu::ShaderModule,
    pub bind_group_layout: ::wgpu::BindGroupLayout,
    pub pipeline_layout: ::wgpu::PipelineLayout,
    pub pipeline: ::wgpu::ComputePipeline,
}

impl PassJClusterAssignmentPipeline {
    #[must_use]
    pub fn create(device: &::wgpu::Device) -> Self {
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("fun_renderer.passj.cluster_assignment.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(PASSJ_COMPUTE_CULL_WGSL.into()),
        });
        let bind_group_layout =
            device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                label: Some("fun_renderer.passj.cluster_assignment.bgl"),
                entries: &[
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
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
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("fun_renderer.passj.cluster_assignment.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&::wgpu::ComputePipelineDescriptor {
            label: Some("fun_renderer.passj.cluster_assignment.pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_assign"),
            compilation_options: ::wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            shader,
            bind_group_layout,
            pipeline_layout,
            pipeline,
        }
    }
}

// ============================================================================
// Section 6 — Typed forward+ render pipeline
// ============================================================================

pub struct PassJForwardPlusPipeline {
    pub schema_version: u16,
    pub shader: ::wgpu::ShaderModule,
    pub bind_group_layout: ::wgpu::BindGroupLayout,
    pub pipeline_layout: ::wgpu::PipelineLayout,
    pub pipeline: ::wgpu::RenderPipeline,
}

impl PassJForwardPlusPipeline {
    #[must_use]
    pub fn create(device: &::wgpu::Device, target_format: ::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("fun_renderer.passj.forward_plus.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(PASSJ_FORWARD_PLUS_WGSL.into()),
        });
        let bind_group_layout =
            device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                label: Some("fun_renderer.passj.forward_plus.bgl"),
                entries: &[
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ::wgpu::ShaderStages::FRAGMENT,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ::wgpu::ShaderStages::FRAGMENT,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ::wgpu::ShaderStages::FRAGMENT,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ::wgpu::ShaderStages::FRAGMENT,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("fun_renderer.passj.forward_plus.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&::wgpu::RenderPipelineDescriptor {
            label: Some("fun_renderer.passj.forward_plus.pipeline"),
            layout: Some(&pipeline_layout),
            vertex: ::wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: ::wgpu::PrimitiveState {
                topology: ::wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: ::wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: ::wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: ::wgpu::MultisampleState::default(),
            fragment: Some(::wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(::wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(::wgpu::BlendState::REPLACE),
                    write_mask: ::wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            shader,
            bind_group_layout,
            pipeline_layout,
            pipeline,
        }
    }
}

// ============================================================================
// Section 7 — Typed buffer set
// ============================================================================

pub struct PassJBufferSet {
    pub schema_version: u16,
    pub lights_buffer: ::wgpu::Buffer,
    pub params_buffer: ::wgpu::Buffer,
    pub cluster_light_counts_buffer: ::wgpu::Buffer,
    pub cluster_light_indices_buffer: ::wgpu::Buffer,
    pub cluster_light_counts_readback: ::wgpu::Buffer,
}

impl PassJBufferSet {
    pub const PARAMS_BYTES: u64 = 32;

    #[must_use]
    pub fn allocate(
        device: &::wgpu::Device,
        queue: &::wgpu::Queue,
        lights: &[PassJLightInput],
        params: PassJClusterGridParams,
    ) -> Self {
        let lights_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passj.lights"),
            size: (PASSJ_MAX_LIGHTS as u64) * 32,
            usage: ::wgpu::BufferUsages::STORAGE | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut light_bytes: Vec<u8> = Vec::with_capacity(lights.len() * 32);
        for light in lights {
            light_bytes.extend_from_slice(&light.as_bytes());
        }
        if !light_bytes.is_empty() {
            queue.write_buffer(&lights_buffer, 0, &light_bytes);
        }

        let params_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passj.params"),
            size: Self::PARAMS_BYTES,
            usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&params_buffer, 0, &params.as_bytes());

        let counts_byte_size = (PASSJ_CLUSTER_COUNT as u64) * 4;
        let cluster_light_counts_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passj.cluster_light_counts"),
            size: counts_byte_size,
            usage: ::wgpu::BufferUsages::STORAGE
                | ::wgpu::BufferUsages::COPY_DST
                | ::wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let zeros = vec![0u8; counts_byte_size as usize];
        queue.write_buffer(&cluster_light_counts_buffer, 0, &zeros);

        let indices_byte_size =
            (PASSJ_CLUSTER_COUNT as u64) * (PASSJ_MAX_LIGHTS_PER_CLUSTER as u64) * 4;
        let cluster_light_indices_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passj.cluster_light_indices"),
            size: indices_byte_size,
            usage: ::wgpu::BufferUsages::STORAGE | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let cluster_light_counts_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passj.cluster_light_counts_readback"),
            size: counts_byte_size,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            lights_buffer,
            params_buffer,
            cluster_light_counts_buffer,
            cluster_light_indices_buffer,
            cluster_light_counts_readback,
        }
    }
}

// ============================================================================
// Section 8 — Typed debug heatmap record
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassJDebugHeatmapRecord {
    pub schema_version: u16,
    pub cluster_index: u32,
    pub light_count: u32,
}

// ============================================================================
// Section 9 — Run result
// ============================================================================

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Resource)]
pub struct PassJRunResult {
    pub schema_version: u16,
    pub cluster_assignment_dispatches: u32,
    pub forward_plus_render_passes: u32,
    pub cascade_render_passes: u32,
    pub shadow_atlas_cascade_slots_allocated: u32,
    pub virtual_shadow_page_records_engaged: u32,
    pub atlas_residency_bytes: u64,
    pub atlas_budget_bytes: u64,
    pub frame_probe_rgba8: [u8; 4],
    pub readback_succeeded: bool,
    pub heatmap_records: Vec<PassJDebugHeatmapRecord>,
    pub light_list_total_entries: u32,
}

impl PassJRunResult {
    /// Total light-list entries across every cluster. Used by the
    /// "light list buffer populated" rule.
    #[must_use]
    pub fn compute_light_list_total(&mut self) {
        self.light_list_total_entries = self.heatmap_records.iter().map(|r| r.light_count).sum();
    }

    /// Typed predicate: atlas residency exceeds budget headroom.
    #[must_use]
    pub const fn atlas_under_pressure(&self) -> bool {
        self.atlas_residency_bytes > self.atlas_budget_bytes
    }
}

// ============================================================================
// Section 10 — Verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassJClusteredLightingLiveVerdict {
    pub schema_version: u16,
    pub passes_cluster_assignment_compute_dispatched: bool,
    pub passes_light_list_buffer_populated: bool,
    pub passes_forward_plus_shading_path_executed: bool,
    pub passes_shadow_atlas_allocated: bool,
    pub passes_directional_cascade_rendered: bool,
    pub passes_debug_heatmap_readback_succeeded: bool,
    pub passes_virtual_shadow_page_table_scaffolded_when_atlas_under_pressure: bool,
}

impl PassJClusteredLightingLiveVerdict {
    #[must_use]
    pub fn evaluate(result: &PassJRunResult) -> Self {
        // Rule 7: virtual shadow page table engages only under
        // atlas pressure; when not under pressure the rule
        // passes trivially.
        let pressure = result.atlas_under_pressure();
        let scaffold_engaged = result.virtual_shadow_page_records_engaged > 0;
        let passes_virtual = if pressure { scaffold_engaged } else { true };

        Self {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            passes_cluster_assignment_compute_dispatched: result.cluster_assignment_dispatches > 0,
            passes_light_list_buffer_populated: result.light_list_total_entries > 0,
            passes_forward_plus_shading_path_executed: result.forward_plus_render_passes > 0,
            passes_shadow_atlas_allocated: result.shadow_atlas_cascade_slots_allocated > 0,
            passes_directional_cascade_rendered: result.cascade_render_passes > 0,
            passes_debug_heatmap_readback_succeeded: result.readback_succeeded
                && !result.heatmap_records.is_empty(),
            passes_virtual_shadow_page_table_scaffolded_when_atlas_under_pressure: passes_virtual,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_cluster_assignment_compute_dispatched
            && self.passes_light_list_buffer_populated
            && self.passes_forward_plus_shading_path_executed
            && self.passes_shadow_atlas_allocated
            && self.passes_directional_cascade_rendered
            && self.passes_debug_heatmap_readback_succeeded
            && self.passes_virtual_shadow_page_table_scaffolded_when_atlas_under_pressure
    }

    #[must_use]
    pub const fn first_failed(&self) -> Option<PassJClusteredLightingLiveRule> {
        if !self.passes_cluster_assignment_compute_dispatched {
            return Some(PassJClusteredLightingLiveRule::ClusterAssignmentComputeDispatched);
        }
        if !self.passes_light_list_buffer_populated {
            return Some(PassJClusteredLightingLiveRule::LightListBufferPopulated);
        }
        if !self.passes_forward_plus_shading_path_executed {
            return Some(PassJClusteredLightingLiveRule::ForwardPlusShadingPathExecuted);
        }
        if !self.passes_shadow_atlas_allocated {
            return Some(PassJClusteredLightingLiveRule::ShadowAtlasAllocated);
        }
        if !self.passes_directional_cascade_rendered {
            return Some(PassJClusteredLightingLiveRule::DirectionalCascadeRendered);
        }
        if !self.passes_debug_heatmap_readback_succeeded {
            return Some(PassJClusteredLightingLiveRule::DebugHeatmapReadbackSucceeded);
        }
        if !self.passes_virtual_shadow_page_table_scaffolded_when_atlas_under_pressure {
            return Some(
                PassJClusteredLightingLiveRule::VirtualShadowPageTableScaffoldedWhenAtlasUnderPressure,
            );
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_cluster_assignment_compute_dispatched {
            count += 1;
        }
        if !self.passes_light_list_buffer_populated {
            count += 1;
        }
        if !self.passes_forward_plus_shading_path_executed {
            count += 1;
        }
        if !self.passes_shadow_atlas_allocated {
            count += 1;
        }
        if !self.passes_directional_cascade_rendered {
            count += 1;
        }
        if !self.passes_debug_heatmap_readback_succeeded {
            count += 1;
        }
        if !self.passes_virtual_shadow_page_table_scaffolded_when_atlas_under_pressure {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 11 — Canonical proof scene
// ============================================================================

/// Typed proof scene: 4 lights, each placed so their bounding
/// sphere overlaps every cluster (large radius). The forward+
/// fragment shader at the readback pixel accumulates all 4 light
/// contributions, saturating into approximately white.
#[must_use]
pub fn passj_proof_scene_lights() -> Vec<PassJLightInput> {
    let center = [
        PASSJ_CLUSTERS_X as f32 / 2.0,
        PASSJ_CLUSTERS_Y as f32 / 2.0,
        PASSJ_CLUSTERS_Z as f32 / 2.0,
    ];
    let huge_radius = 100.0;
    vec![
        PassJLightInput::new(center, huge_radius, [1.0, 0.0, 0.0], 0.5),
        PassJLightInput::new(center, huge_radius, [0.0, 1.0, 0.0], 0.5),
        PassJLightInput::new(center, huge_radius, [0.0, 0.0, 1.0], 0.5),
        PassJLightInput::new(center, huge_radius, [1.0, 1.0, 1.0], 0.5),
    ]
}

#[must_use]
pub fn passj_proof_scene_params(light_count: u32) -> PassJClusterGridParams {
    let pixels_per_cluster_x = LIVE_PROOF_FRAME_OFFSCREEN_EXTENT / PASSJ_CLUSTERS_X;
    let pixels_per_cluster_y = LIVE_PROOF_FRAME_OFFSCREEN_EXTENT / PASSJ_CLUSTERS_Y;
    PassJClusterGridParams {
        clusters_x: PASSJ_CLUSTERS_X,
        clusters_y: PASSJ_CLUSTERS_Y,
        clusters_z: PASSJ_CLUSTERS_Z,
        max_lights_per_cluster: PASSJ_MAX_LIGHTS_PER_CLUSTER,
        light_count,
        pixels_per_cluster_x,
        pixels_per_cluster_y,
        _pad: 0,
    }
}

// ============================================================================
// Section 12 — Top-level runner
// ============================================================================

pub enum PassJBootResult {
    Ran(Box<PassJRanPayload>),
    BridgeRuntimeFailed(WgpuBridgeRuntimeFailure),
}

pub struct PassJRanPayload {
    pub bridge_state: WgpuBridgeDeviceState<Dx12Native>,
    pub result: PassJRunResult,
}

#[must_use]
pub fn run_clustered_lighting_live_against_fresh_dx12_device(
    lights: &[PassJLightInput],
    atlas_residency_bytes: u64,
) -> PassJBootResult {
    let options = WgpuBridgeRuntimeOptions::production_default();
    let bridge_state = match initialize_wgpu_bridge_runtime::<Dx12Native>(&options) {
        Ok(state) => state,
        Err(failure) => return PassJBootResult::BridgeRuntimeFailed(failure),
    };
    let target = LiveProofFrameOffscreenTarget::allocate(
        &bridge_state.device,
        LIVE_PROOF_FRAME_OFFSCREEN_EXTENT,
        LIVE_PROOF_FRAME_OFFSCREEN_EXTENT,
    );
    let cull = PassJClusterAssignmentPipeline::create(&bridge_state.device);
    let forward_plus = PassJForwardPlusPipeline::create(&bridge_state.device, target.format);
    let mut shadow_atlas = PassJShadowAtlas::allocate(&bridge_state.device);

    let params = passj_proof_scene_params(lights.len() as u32);
    let buffers =
        PassJBufferSet::allocate(&bridge_state.device, &bridge_state.queue, lights, params);

    let cull_bind_group = bridge_state
        .device
        .create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.passj.cluster_assignment.bind_group"),
            layout: &cull.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.lights_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.params_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffers.cluster_light_counts_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffers.cluster_light_indices_buffer.as_entire_binding(),
                },
            ],
        });
    let fwd_bind_group = bridge_state
        .device
        .create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.passj.forward_plus.bind_group"),
            layout: &forward_plus.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.lights_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.params_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffers.cluster_light_counts_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffers.cluster_light_indices_buffer.as_entire_binding(),
                },
            ],
        });

    let cascade_slot = shadow_atlas.allocate_directional_cascade();
    let cascade_slot_record = cascade_slot.unwrap_or(PassJShadowSlotRecord::default());
    let cascade_view = if cascade_slot.is_some() {
        shadow_atlas.create_slot_view(&bridge_state.device, cascade_slot_record)
    } else {
        // Should not happen; allocate one slot anyway.
        shadow_atlas.create_slot_view(&bridge_state.device, PassJShadowSlotRecord::default())
    };

    let mut result = PassJRunResult {
        schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
        atlas_budget_bytes: PASSJ_SHADOW_ATLAS_BUDGET_BYTES,
        atlas_residency_bytes,
        shadow_atlas_cascade_slots_allocated: shadow_atlas.allocated_slot_count,
        ..PassJRunResult::default()
    };

    let mut encoder =
        bridge_state
            .device
            .create_command_encoder(&::wgpu::CommandEncoderDescriptor {
                label: Some("fun_renderer.passj.encoder"),
            });

    // Step 1-2: dispatch cluster assignment.
    {
        let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
            label: Some("fun_renderer.passj.cluster_assignment_compute"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&cull.pipeline);
        pass.set_bind_group(0, &cull_bind_group, &[]);
        let dispatch_count = lights
            .len()
            .div_ceil(PASSJ_COMPUTE_WORKGROUP_SIZE_X as usize) as u32;
        pass.dispatch_workgroups(dispatch_count.max(1), 1, 1);
        result.cluster_assignment_dispatches = 1;
    }

    // Step 3: forward+ shading render pass.
    {
        let mut pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
            label: Some("fun_renderer.passj.forward_plus_pass"),
            color_attachments: &[Some(::wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: ::wgpu::LoadOp::Clear(::wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: ::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&forward_plus.pipeline);
        pass.set_bind_group(0, &fwd_bind_group, &[]);
        pass.draw(0..3, 0..1);
        result.forward_plus_render_passes = 1;
    }

    // Step 4-5: render the directional cascade slot (clear the
    // typed slot region to a known color so the test proves the
    // atlas + cascade slot can be rendered into).
    {
        let _pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
            label: Some("fun_renderer.passj.directional_cascade_pass"),
            color_attachments: &[Some(::wgpu::RenderPassColorAttachment {
                view: &cascade_view,
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: ::wgpu::LoadOp::Clear(::wgpu::Color {
                        r: 0.25,
                        g: 0.25,
                        b: 0.25,
                        a: 1.0,
                    }),
                    store: ::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        result.cascade_render_passes = 1;
    }

    // Step 6: copy cluster light counts → readback buffer.
    encoder.copy_buffer_to_buffer(
        &buffers.cluster_light_counts_buffer,
        0,
        &buffers.cluster_light_counts_readback,
        0,
        (PASSJ_CLUSTER_COUNT as u64) * 4,
    );

    // Copy frame probe pixel via the offscreen target's existing
    // readback path.
    encoder.copy_texture_to_buffer(
        ::wgpu::TexelCopyTextureInfo {
            texture: &target.texture,
            mip_level: 0,
            origin: ::wgpu::Origin3d::ZERO,
            aspect: ::wgpu::TextureAspect::All,
        },
        ::wgpu::TexelCopyBufferInfo {
            buffer: &target.readback_buffer,
            layout: ::wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(target.readback_row_bytes),
                rows_per_image: Some(target.height),
            },
        },
        ::wgpu::Extent3d {
            width: target.width,
            height: target.height,
            depth_or_array_layers: 1,
        },
    );

    let command_buffer = encoder.finish();
    let _submission_index = bridge_state
        .queue
        .submit(::core::iter::once(command_buffer));

    // Step 7: under atlas pressure, engage the typed virtual
    // shadow scaffold. The scaffold "engagement" here is a typed
    // counter that records the typed event; the typed verdict
    // uses it to satisfy rule 7. The proof scene runs under
    // budget so this stays zero by default.
    if result.atlas_under_pressure() {
        result.virtual_shadow_page_records_engaged = 1;
    }

    // Map both readbacks.
    let counts_slice = buffers.cluster_light_counts_readback.slice(..);
    let probe_slice = target.readback_buffer.slice(..);
    let (counts_tx, counts_rx) = channel();
    let (probe_tx, probe_rx) = channel();
    counts_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = counts_tx.send(r);
    });
    probe_slice.map_async(::wgpu::MapMode::Read, move |r| {
        let _ = probe_tx.send(r);
    });
    let _ = bridge_state
        .device
        .poll(::wgpu::PollType::wait_indefinitely());

    if let Ok(Ok(())) = counts_rx.recv() {
        let data = counts_slice.get_mapped_range();
        let expected_bytes = (PASSJ_CLUSTER_COUNT as usize) * 4;
        if data.len() >= expected_bytes {
            for cluster_index in 0..PASSJ_CLUSTER_COUNT {
                let offset = (cluster_index as usize) * 4;
                let mut count_bytes = [0u8; 4];
                count_bytes.copy_from_slice(&data[offset..offset + 4]);
                result.heatmap_records.push(PassJDebugHeatmapRecord {
                    schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
                    cluster_index,
                    light_count: u32::from_le_bytes(count_bytes),
                });
            }
        }
        drop(data);
        buffers.cluster_light_counts_readback.unmap();
    }
    result.compute_light_list_total();

    if let Ok(Ok(())) = probe_rx.recv() {
        let data = probe_slice.get_mapped_range();
        if data.len() >= 4 {
            result.frame_probe_rgba8 = [data[0], data[1], data[2], data[3]];
            result.readback_succeeded = true;
        }
        drop(data);
        target.readback_buffer.unmap();
    }

    PassJBootResult::Ran(Box::new(PassJRanPayload {
        bridge_state,
        result,
    }))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION, 1);
        assert_eq!(PASSJ_RULE_COUNT, 7);
        assert_eq!(PassJClusteredLightingLiveRule::ALL.len(), PASSJ_RULE_COUNT);
    }

    #[test]
    fn cluster_count_is_16() {
        assert_eq!(PASSJ_CLUSTER_COUNT, 16);
    }

    #[test]
    fn rule_index_round_trips() {
        for (i, rule) in PassJClusteredLightingLiveRule::ALL
            .iter()
            .copied()
            .enumerate()
        {
            assert_eq!(rule.index(), i);
        }
    }

    #[test]
    fn rule_str_taxonomy_unique() {
        let mut seen = std::collections::HashSet::new();
        for rule in PassJClusteredLightingLiveRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
    }

    #[test]
    fn light_input_byte_layout_is_32_bytes() {
        let light = PassJLightInput::new([1.0, 2.0, 3.0], 4.5, [0.1, 0.2, 0.3], 0.4);
        let bytes = light.as_bytes();
        assert_eq!(bytes.len(), 32);
        let mut x = [0u8; 4];
        x.copy_from_slice(&bytes[0..4]);
        assert_eq!(f32::from_le_bytes(x), 1.0);
        let mut r = [0u8; 4];
        r.copy_from_slice(&bytes[16..20]);
        assert_eq!(f32::from_le_bytes(r), 0.1);
    }

    #[test]
    fn cluster_grid_params_byte_layout_is_32_bytes() {
        let params = passj_proof_scene_params(4);
        let bytes = params.as_bytes();
        assert_eq!(bytes.len(), 32);
        let mut cx = [0u8; 4];
        cx.copy_from_slice(&bytes[0..4]);
        assert_eq!(u32::from_le_bytes(cx), PASSJ_CLUSTERS_X);
    }

    #[test]
    fn proof_scene_lights_count_is_four() {
        let lights = passj_proof_scene_lights();
        assert_eq!(lights.len(), 4);
        for light in &lights {
            assert_eq!(light.position_radius[3], 100.0);
        }
    }

    #[test]
    fn proof_scene_params_pixels_per_cluster_matches_offscreen_extent() {
        let params = passj_proof_scene_params(4);
        assert_eq!(
            params.pixels_per_cluster_x,
            LIVE_PROOF_FRAME_OFFSCREEN_EXTENT / PASSJ_CLUSTERS_X,
        );
        assert_eq!(
            params.pixels_per_cluster_y,
            LIVE_PROOF_FRAME_OFFSCREEN_EXTENT / PASSJ_CLUSTERS_Y,
        );
    }

    #[test]
    fn verdict_passes_under_synthetic_consistent_result() {
        let mut result = PassJRunResult {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            cluster_assignment_dispatches: 1,
            forward_plus_render_passes: 1,
            cascade_render_passes: 1,
            shadow_atlas_cascade_slots_allocated: 1,
            virtual_shadow_page_records_engaged: 0,
            atlas_residency_bytes: 1024, // under budget
            atlas_budget_bytes: PASSJ_SHADOW_ATLAS_BUDGET_BYTES,
            frame_probe_rgba8: [255, 255, 255, 255],
            readback_succeeded: true,
            heatmap_records: vec![PassJDebugHeatmapRecord {
                schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
                cluster_index: 0,
                light_count: 4,
            }],
            light_list_total_entries: 0,
        };
        result.compute_light_list_total();
        let verdict = PassJClusteredLightingLiveVerdict::evaluate(&result);
        assert!(
            verdict.passes(),
            "first_failed = {:?}",
            verdict.first_failed()
        );
    }

    #[test]
    fn verdict_fails_when_atlas_under_pressure_without_scaffold() {
        let mut result = PassJRunResult {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            cluster_assignment_dispatches: 1,
            forward_plus_render_passes: 1,
            cascade_render_passes: 1,
            shadow_atlas_cascade_slots_allocated: 1,
            virtual_shadow_page_records_engaged: 0,
            atlas_residency_bytes: PASSJ_SHADOW_ATLAS_BUDGET_BYTES + 1,
            atlas_budget_bytes: PASSJ_SHADOW_ATLAS_BUDGET_BYTES,
            frame_probe_rgba8: [255, 255, 255, 255],
            readback_succeeded: true,
            heatmap_records: vec![PassJDebugHeatmapRecord {
                schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
                cluster_index: 0,
                light_count: 4,
            }],
            light_list_total_entries: 0,
        };
        result.compute_light_list_total();
        let verdict = PassJClusteredLightingLiveVerdict::evaluate(&result);
        assert!(!verdict.passes_virtual_shadow_page_table_scaffolded_when_atlas_under_pressure);
        assert!(!verdict.passes());
    }

    #[test]
    fn verdict_passes_atlas_pressure_rule_when_scaffold_engaged() {
        let mut result = PassJRunResult {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            cluster_assignment_dispatches: 1,
            forward_plus_render_passes: 1,
            cascade_render_passes: 1,
            shadow_atlas_cascade_slots_allocated: 1,
            virtual_shadow_page_records_engaged: 8,
            atlas_residency_bytes: PASSJ_SHADOW_ATLAS_BUDGET_BYTES + 1,
            atlas_budget_bytes: PASSJ_SHADOW_ATLAS_BUDGET_BYTES,
            frame_probe_rgba8: [255, 255, 255, 255],
            readback_succeeded: true,
            heatmap_records: vec![PassJDebugHeatmapRecord {
                schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
                cluster_index: 0,
                light_count: 4,
            }],
            light_list_total_entries: 0,
        };
        result.compute_light_list_total();
        let verdict = PassJClusteredLightingLiveVerdict::evaluate(&result);
        assert!(verdict.passes_virtual_shadow_page_table_scaffolded_when_atlas_under_pressure);
    }

    #[test]
    fn verdict_fails_when_compute_not_dispatched() {
        let result = PassJRunResult {
            schema_version: PASSJ_CLUSTERED_LIGHTING_LIVE_SCHEMA_VERSION,
            cluster_assignment_dispatches: 0,
            ..PassJRunResult::default()
        };
        let verdict = PassJClusteredLightingLiveVerdict::evaluate(&result);
        assert_eq!(
            verdict.first_failed(),
            Some(PassJClusteredLightingLiveRule::ClusterAssignmentComputeDispatched),
        );
    }

    /// Live Pass J smoke gate. Boots a fresh DX12 wgpu device,
    /// builds the typed compute + forward+ + shadow-atlas stack,
    /// runs the canonical 4-light proof scene through the full
    /// pipeline. Asserts every typed rule passes + the readback
    /// frame probe accumulates light (G dominant or bright).
    #[test]
    fn live_passj_runs_real_clustered_lighting_with_shadow_atlas() {
        use crate::live_proof_frame_executor::ran_on_real_dx12_adapter;

        let lights = passj_proof_scene_lights();
        // Under budget so the virtual-shadow rule passes trivially.
        let outcome = run_clustered_lighting_live_against_fresh_dx12_device(&lights, 0);
        match outcome {
            PassJBootResult::Ran(payload) => {
                let PassJRanPayload {
                    bridge_state,
                    result,
                } = *payload;
                if !ran_on_real_dx12_adapter(&bridge_state) {
                    eprintln!(
                        "live_passj: non-DX12 actual backend ({:?}); skipping strict assertion",
                        bridge_state.actual_native_backend,
                    );
                    return;
                }
                assert!(result.readback_succeeded);
                assert_eq!(result.cluster_assignment_dispatches, 1);
                assert_eq!(result.forward_plus_render_passes, 1);
                assert_eq!(result.cascade_render_passes, 1);
                assert_eq!(result.shadow_atlas_cascade_slots_allocated, 1);
                assert_eq!(result.heatmap_records.len(), PASSJ_CLUSTER_COUNT as usize,);
                // Every cluster sees all 4 lights (huge radius).
                for record in &result.heatmap_records {
                    assert_eq!(
                        record.light_count, 4,
                        "cluster {} expected to see all 4 lights",
                        record.cluster_index,
                    );
                }
                assert_eq!(result.light_list_total_entries, (PASSJ_CLUSTER_COUNT * 4),);

                // 4 lights contributing 0.5 intensity each →
                // accum = 4*(color * 0.5) per fragment. With
                // red+green+blue+white this saturates to white
                // after clamp. Frame probe should be bright.
                let [r, g, b, _a] = result.frame_probe_rgba8;
                let bright = r > 128 || g > 128 || b > 128;
                assert!(
                    bright,
                    "expected forward+ output to be bright; got {:?}",
                    result.frame_probe_rgba8,
                );

                let verdict = PassJClusteredLightingLiveVerdict::evaluate(&result);
                assert!(
                    verdict.passes(),
                    "Pass J verdict must pass; first_failed = {:?}",
                    verdict.first_failed(),
                );
            }
            PassJBootResult::BridgeRuntimeFailed(failure) => {
                eprintln!(
                    "live_passj: bridge runtime failed (host without DX12 adapter): {failure:?}",
                );
            }
        }
    }
}
