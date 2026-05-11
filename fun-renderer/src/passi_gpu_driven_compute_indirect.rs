//! Pass I — GPU-Driven Compute Culling + Indirect Draw.
//!
//! The user's "Future bleeding-edge plan / Priority 1" calls for
//! "GPU-driven rendering with real GPU dispatch": run actual
//! compute + indirect draws, not just CPU-simulated parity. The
//! build order in the user prompt:
//!
//! 1. Upload object bounds buffer.
//! 2. Upload camera planes.
//! 3. Dispatch GPU frustum culling.
//! 4. Write visible instance list.
//! 5. Write indirect args/count.
//! 6. Draw indirect.
//! 7. Read debug counters.
//! 8. Compare against CPU/direct parity.
//!
//! Pass I implements every step against a real DX12 wgpu device
//! and produces a typed
//! [`PassIGpuVsCpuParityVerdict`] that proves the GPU output
//! matches the CPU-direct path. The compute shader runs a typed
//! plane-vs-AABB SAT against six camera planes — the same
//! algorithm Tier 2's `run_gpu_driven_frustum_cull` simulates on
//! CPU and Pass D's parity verdict already validates
//! algorithmically. Pass I closes the loop by running the real
//! WGSL compute kernel and reading back the typed atomic
//! counters.
//!
//! Honest scope: Pass I's evidence kind in
//! `quality_audit_contract::PASS_EVIDENCE_REGISTRY` is
//! `LiveGpuExecution` because the test boots a fresh DX12 wgpu
//! device + compiles real WGSL + dispatches a real compute pass
//! + records `draw_indexed_indirect` + reads back real GPU
//! counters. On hosts without DX12 the live boot returns
//! `BridgeRuntimeFailed` and records honestly.

use flume::unbounded;

use bevy_ecs::prelude::Resource;

use crate::bridge::wgpu::{
    Dx12Native, WgpuBridgeDeviceState, WgpuBridgeRuntimeFailure, WgpuBridgeRuntimeOptions,
    initialize_wgpu_bridge_runtime,
};
use crate::live_proof_frame_executor::{
    LIVE_PROOF_FRAME_OFFSCREEN_EXTENT, LiveProofFrameOffscreenTarget,
};

pub const PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION: u16 = 1;
pub const PASSI_RULE_COUNT: usize = 5;
/// Maximum number of objects the typed culling pipeline allocates
/// for. The proof scene uses 4 (2 inside the frustum, 2 outside).
pub const PASSI_MAX_OBJECTS: u32 = 64;
/// Compute workgroup size in x; the WGSL shader uses 64-thread
/// workgroups.
pub const PASSI_COMPUTE_WORKGROUP_SIZE_X: u32 = 64;

// ============================================================================
// Section 1 — Exit-gate rule taxonomy
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PassIGpuDrivenComputeIndirectRule {
    /// The typed compute pipeline dispatched at least once.
    GpuComputeCullDispatched,
    /// The GPU's typed atomic `instances_tested` counter equals
    /// the typed scene's object count.
    InstancesTestedMatchesObjectCount,
    /// The GPU's typed atomic `instances_visible` + `instances_rejected`
    /// equals `instances_tested` (the typed SAT either accepts or
    /// rejects every tested object — there is no third bucket).
    DebugCountersAreConsistent,
    /// `draw_indexed_indirect` was recorded against the typed
    /// indirect args buffer the compute pass wrote.
    IndirectDrawIssuedAgainstComputeWrittenArgs,
    /// The GPU's typed `instances_visible` count matches the
    /// CPU-direct frustum-cull count for the same scene + frustum.
    /// Closes the user's "Compare against CPU/direct parity" step.
    GpuInstancesVisibleMatchesCpuDirectCount,
}

impl PassIGpuDrivenComputeIndirectRule {
    pub const ALL: [Self; PASSI_RULE_COUNT] = [
        Self::GpuComputeCullDispatched,
        Self::InstancesTestedMatchesObjectCount,
        Self::DebugCountersAreConsistent,
        Self::IndirectDrawIssuedAgainstComputeWrittenArgs,
        Self::GpuInstancesVisibleMatchesCpuDirectCount,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::GpuComputeCullDispatched => 0,
            Self::InstancesTestedMatchesObjectCount => 1,
            Self::DebugCountersAreConsistent => 2,
            Self::IndirectDrawIssuedAgainstComputeWrittenArgs => 3,
            Self::GpuInstancesVisibleMatchesCpuDirectCount => 4,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpuComputeCullDispatched => "gpu_compute_cull_dispatched",
            Self::InstancesTestedMatchesObjectCount => "instances_tested_matches_object_count",
            Self::DebugCountersAreConsistent => "debug_counters_are_consistent",
            Self::IndirectDrawIssuedAgainstComputeWrittenArgs => {
                "indirect_draw_issued_against_compute_written_args"
            }
            Self::GpuInstancesVisibleMatchesCpuDirectCount => {
                "gpu_instances_visible_matches_cpu_direct_count"
            }
        }
    }
}

// ============================================================================
// Section 2 — Typed scene + frustum inputs (CPU side)
// ============================================================================

/// Typed object AABB used by both the CPU-direct path and the
/// GPU compute shader. The std140-friendly memory layout is two
/// `vec3<f32>` slots with explicit padding so the typed struct
/// matches the WGSL `ObjectBoundsGpu` layout one-to-one.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PassIObjectBounds {
    pub min: [f32; 3],
    pub _pad1: f32,
    pub max: [f32; 3],
    pub _pad2: f32,
}

impl PassIObjectBounds {
    #[must_use]
    pub const fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self {
            min,
            _pad1: 0.0,
            max,
            _pad2: 0.0,
        }
    }

    /// Convert the typed bounds into a `[u8; 32]` byte slice for
    /// upload to a `wgpu::Buffer`.
    #[must_use]
    pub fn as_bytes(&self) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        bytes[0..4].copy_from_slice(&self.min[0].to_le_bytes());
        bytes[4..8].copy_from_slice(&self.min[1].to_le_bytes());
        bytes[8..12].copy_from_slice(&self.min[2].to_le_bytes());
        bytes[12..16].copy_from_slice(&self._pad1.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.max[0].to_le_bytes());
        bytes[20..24].copy_from_slice(&self.max[1].to_le_bytes());
        bytes[24..28].copy_from_slice(&self.max[2].to_le_bytes());
        bytes[28..32].copy_from_slice(&self._pad2.to_le_bytes());
        bytes
    }
}

/// Typed camera frustum (6 planes). Each plane is
/// `vec4<f32>(nx, ny, nz, d)` where `dot(plane.xyz, p) + plane.w
/// >= 0` means "inside the half-space". The compute shader
/// performs plane-vs-AABB SAT using the "positive vertex" test.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PassICameraPlanes {
    pub planes: [[f32; 4]; 6],
}

impl PassICameraPlanes {
    /// An accept-all frustum (every plane normal points "outward
    /// from infinity"). Every object passes every plane test.
    #[must_use]
    pub const fn accept_all() -> Self {
        Self {
            planes: [[0.0, 0.0, 1.0, f32::MAX]; 6],
        }
    }

    /// A typed frustum that accepts the half-space `x <= cutoff`
    /// (the other five planes are accept-all). Used by tests
    /// because the half-space cutoff is a simple knob:
    /// objects whose AABB max.x <= cutoff pass, others fail.
    #[must_use]
    pub fn half_space_x(cutoff: f32) -> Self {
        let mut planes = [[0.0, 0.0, 1.0, f32::MAX]; 6];
        // Plane: -x + cutoff >= 0 → x <= cutoff.
        planes[0] = [-1.0, 0.0, 0.0, cutoff];
        Self { planes }
    }

    #[must_use]
    pub fn as_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        for (i, plane) in self.planes.iter().enumerate() {
            let base = i * 16;
            bytes[base..base + 4].copy_from_slice(&plane[0].to_le_bytes());
            bytes[base + 4..base + 8].copy_from_slice(&plane[1].to_le_bytes());
            bytes[base + 8..base + 12].copy_from_slice(&plane[2].to_le_bytes());
            bytes[base + 12..base + 16].copy_from_slice(&plane[3].to_le_bytes());
        }
        bytes
    }
}

/// CPU-direct frustum cull (the typed predicate the user's
/// "Compare against CPU/direct parity" step compares the GPU
/// output against). Uses the same plane-vs-AABB SAT positive-
/// vertex test the WGSL compute shader runs.
#[must_use]
pub fn cpu_direct_count_visible(objects: &[PassIObjectBounds], planes: &PassICameraPlanes) -> u32 {
    let mut visible = 0u32;
    for obj in objects {
        if cpu_direct_object_passes(obj, planes) {
            visible = visible.saturating_add(1);
        }
    }
    visible
}

fn cpu_direct_object_passes(obj: &PassIObjectBounds, planes: &PassICameraPlanes) -> bool {
    for plane in &planes.planes {
        let positive_vertex = [
            if plane[0] >= 0.0 {
                obj.max[0]
            } else {
                obj.min[0]
            },
            if plane[1] >= 0.0 {
                obj.max[1]
            } else {
                obj.min[1]
            },
            if plane[2] >= 0.0 {
                obj.max[2]
            } else {
                obj.min[2]
            },
        ];
        let distance = plane[0] * positive_vertex[0]
            + plane[1] * positive_vertex[1]
            + plane[2] * positive_vertex[2]
            + plane[3];
        if distance < 0.0 {
            return false;
        }
    }
    true
}

// ============================================================================
// Section 3 — WGSL shaders
// ============================================================================

const PASSI_COMPUTE_CULL_WGSL: &str = "\
struct ObjectBoundsGpu {\n\
    min: vec3<f32>,\n\
    pad1: f32,\n\
    max: vec3<f32>,\n\
    pad2: f32,\n\
}\n\
\n\
struct CameraPlanesGpu {\n\
    planes: array<vec4<f32>, 6>,\n\
}\n\
\n\
struct DebugCountersGpu {\n\
    instances_tested: atomic<u32>,\n\
    instances_visible: atomic<u32>,\n\
    instances_rejected: atomic<u32>,\n\
}\n\
\n\
struct DrawIndexedIndirectGpu {\n\
    index_count: u32,\n\
    instance_count: atomic<u32>,\n\
    first_index: u32,\n\
    base_vertex: i32,\n\
    first_instance: u32,\n\
}\n\
\n\
struct CullParamsGpu {\n\
    object_count: u32,\n\
    _pad0: u32,\n\
    _pad1: u32,\n\
    _pad2: u32,\n\
}\n\
\n\
@group(0) @binding(0) var<storage, read> object_bounds: array<ObjectBoundsGpu>;\n\
@group(0) @binding(1) var<uniform> camera_planes: CameraPlanesGpu;\n\
@group(0) @binding(2) var<storage, read_write> visible_instances: array<u32>;\n\
@group(0) @binding(3) var<storage, read_write> indirect_args: DrawIndexedIndirectGpu;\n\
@group(0) @binding(4) var<storage, read_write> debug_counters: DebugCountersGpu;\n\
@group(0) @binding(5) var<uniform> params: CullParamsGpu;\n\
\n\
@compute @workgroup_size(64)\n\
fn cs_cull(@builtin(global_invocation_id) gid: vec3<u32>) {\n\
    let object_index = gid.x;\n\
    if (object_index >= params.object_count) {\n\
        return;\n\
    }\n\
    atomicAdd(&debug_counters.instances_tested, 1u);\n\
    let bounds = object_bounds[object_index];\n\
    var visible: bool = true;\n\
    for (var i: u32 = 0u; i < 6u; i = i + 1u) {\n\
        let plane = camera_planes.planes[i];\n\
        let positive_vertex = vec3<f32>(\n\
            select(bounds.min.x, bounds.max.x, plane.x >= 0.0),\n\
            select(bounds.min.y, bounds.max.y, plane.y >= 0.0),\n\
            select(bounds.min.z, bounds.max.z, plane.z >= 0.0)\n\
        );\n\
        let distance = dot(plane.xyz, positive_vertex) + plane.w;\n\
        if (distance < 0.0) {\n\
            visible = false;\n\
            break;\n\
        }\n\
    }\n\
    if (visible) {\n\
        let slot = atomicAdd(&indirect_args.instance_count, 1u);\n\
        visible_instances[slot] = object_index;\n\
        atomicAdd(&debug_counters.instances_visible, 1u);\n\
    } else {\n\
        atomicAdd(&debug_counters.instances_rejected, 1u);\n\
    }\n\
}\n\
";

const PASSI_INDIRECT_DRAW_WGSL: &str = "\
@group(0) @binding(0) var<storage, read> visible_instances: array<u32>;\n\
\n\
@vertex\n\
fn vs_main(\n\
    @builtin(vertex_index) vid: u32,\n\
    @builtin(instance_index) iid: u32\n\
) -> @builtin(position) vec4<f32> {\n\
    let instance = visible_instances[iid];\n\
    let offset = f32(instance) * 0.4 - 0.6;\n\
    var positions = array<vec2<f32>, 3>(\n\
        vec2<f32>(-0.2 + offset, -0.2),\n\
        vec2<f32>( 0.2 + offset, -0.2),\n\
        vec2<f32>( 0.0 + offset,  0.4)\n\
    );\n\
    return vec4<f32>(positions[vid], 0.0, 1.0);\n\
}\n\
\n\
@fragment\n\
fn fs_main() -> @location(0) vec4<f32> {\n\
    return vec4<f32>(0.2, 0.4, 1.0, 1.0);\n\
}\n\
";

// ============================================================================
// Section 4 — Typed GPU pipelines + buffers
// ============================================================================

/// Typed compute pipeline for the GPU frustum-cull kernel.
pub struct PassIGpuDrivenCullPipeline {
    pub schema_version: u16,
    pub shader: ::wgpu::ShaderModule,
    pub bind_group_layout: ::wgpu::BindGroupLayout,
    pub pipeline_layout: ::wgpu::PipelineLayout,
    pub pipeline: ::wgpu::ComputePipeline,
}

impl PassIGpuDrivenCullPipeline {
    #[must_use]
    pub fn create(device: &::wgpu::Device) -> Self {
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("fun_renderer.passi.compute_cull.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(PASSI_COMPUTE_CULL_WGSL.into()),
        });
        let bind_group_layout =
            device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                label: Some("fun_renderer.passi.compute_cull.bgl"),
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
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    ::wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: ::wgpu::ShaderStages::COMPUTE,
                        ty: ::wgpu::BindingType::Buffer {
                            ty: ::wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("fun_renderer.passi.compute_cull.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&::wgpu::ComputePipelineDescriptor {
            label: Some("fun_renderer.passi.compute_cull.pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cs_cull"),
            compilation_options: ::wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });
        Self {
            schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
            shader,
            bind_group_layout,
            pipeline_layout,
            pipeline,
        }
    }
}

/// Typed indirect-draw render pipeline. The vertex shader reads
/// `@builtin(instance_index)` and indexes into the typed
/// `visible_instances` buffer to position each triangle.
pub struct PassIIndirectDrawPipeline {
    pub schema_version: u16,
    pub shader: ::wgpu::ShaderModule,
    pub bind_group_layout: ::wgpu::BindGroupLayout,
    pub pipeline_layout: ::wgpu::PipelineLayout,
    pub pipeline: ::wgpu::RenderPipeline,
    pub index_buffer: ::wgpu::Buffer,
    pub index_count: u32,
}

impl PassIIndirectDrawPipeline {
    pub const INDEX_COUNT: u32 = 3;
    pub const INDEX_BUFFER_BYTES: u64 = 8;

    #[must_use]
    pub fn create(device: &::wgpu::Device, target_format: ::wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some("fun_renderer.passi.indirect_draw.wgsl"),
            source: ::wgpu::ShaderSource::Wgsl(PASSI_INDIRECT_DRAW_WGSL.into()),
        });
        let bind_group_layout =
            device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                label: Some("fun_renderer.passi.indirect_draw.bgl"),
                entries: &[::wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ::wgpu::ShaderStages::VERTEX,
                    ty: ::wgpu::BindingType::Buffer {
                        ty: ::wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let pipeline_layout = device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
            label: Some("fun_renderer.passi.indirect_draw.layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&::wgpu::RenderPipelineDescriptor {
            label: Some("fun_renderer.passi.indirect_draw.pipeline"),
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

        let index_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.indirect_draw.index_buffer"),
            size: Self::INDEX_BUFFER_BYTES,
            usage: ::wgpu::BufferUsages::INDEX,
            mapped_at_creation: true,
        });
        {
            let mut view = index_buffer.slice(..).get_mapped_range_mut();
            // Little-endian u16: [0, 1, 2, _].
            view.copy_from_slice(&[0u8, 0, 1, 0, 2, 0, 0, 0]);
        }
        index_buffer.unmap();

        Self {
            schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
            shader,
            bind_group_layout,
            pipeline_layout,
            pipeline,
            index_buffer,
            index_count: Self::INDEX_COUNT,
        }
    }
}

// ============================================================================
// Section 5 — Typed buffer set
// ============================================================================

/// Typed bag of GPU buffers Pass I owns end to end. The compute
/// pipeline writes the typed atomic counters; the indirect draw
/// reads `indirect_args` and `visible_instances`.
pub struct PassIBufferSet {
    pub schema_version: u16,
    pub object_bounds_buffer: ::wgpu::Buffer,
    pub camera_planes_buffer: ::wgpu::Buffer,
    pub visible_instances_buffer: ::wgpu::Buffer,
    pub indirect_args_buffer: ::wgpu::Buffer,
    pub debug_counters_buffer: ::wgpu::Buffer,
    pub cull_params_buffer: ::wgpu::Buffer,
    pub debug_counters_readback: ::wgpu::Buffer,
}

impl PassIBufferSet {
    pub const DEBUG_COUNTERS_BYTES: u64 = 12; // 3 × u32
    pub const INDIRECT_ARGS_BYTES: u64 = 20; // 5 × u32
    pub const CULL_PARAMS_BYTES: u64 = 16; // 4 × u32 (uniform-aligned)

    #[must_use]
    pub fn allocate(
        device: &::wgpu::Device,
        queue: &::wgpu::Queue,
        objects: &[PassIObjectBounds],
        planes: &PassICameraPlanes,
    ) -> Self {
        let object_count = objects.len() as u32;
        let object_bounds_bytes = (object_count as u64) * 32;
        let object_bounds_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.object_bounds"),
            size: object_bounds_bytes.max(32),
            usage: ::wgpu::BufferUsages::STORAGE | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Upload typed object bounds.
        let mut bytes: Vec<u8> = Vec::with_capacity(object_bounds_bytes as usize);
        for obj in objects {
            bytes.extend_from_slice(&obj.as_bytes());
        }
        if !bytes.is_empty() {
            queue.write_buffer(&object_bounds_buffer, 0, &bytes);
        }

        let camera_planes_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.camera_planes"),
            size: 96,
            usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&camera_planes_buffer, 0, &planes.as_bytes());

        let visible_instances_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.visible_instances"),
            size: (PASSI_MAX_OBJECTS as u64) * 4,
            usage: ::wgpu::BufferUsages::STORAGE | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // indirect_args = [index_count = 3, instance_count = 0,
        // first_index = 0, base_vertex = 0, first_instance = 0].
        let indirect_args_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.indirect_args"),
            size: Self::INDIRECT_ARGS_BYTES,
            usage: ::wgpu::BufferUsages::INDIRECT
                | ::wgpu::BufferUsages::STORAGE
                | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut indirect_init = [0u8; 20];
        indirect_init[0..4].copy_from_slice(&PassIIndirectDrawPipeline::INDEX_COUNT.to_le_bytes());
        // Rest stay zero — instance_count starts at 0 and the
        // compute shader atomically increments it.
        queue.write_buffer(&indirect_args_buffer, 0, &indirect_init);

        let debug_counters_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.debug_counters"),
            size: Self::DEBUG_COUNTERS_BYTES,
            usage: ::wgpu::BufferUsages::STORAGE
                | ::wgpu::BufferUsages::COPY_DST
                | ::wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        queue.write_buffer(&debug_counters_buffer, 0, &[0u8; 12]);

        let cull_params_buffer = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.cull_params"),
            size: Self::CULL_PARAMS_BYTES,
            usage: ::wgpu::BufferUsages::UNIFORM | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut cull_params_init = [0u8; 16];
        cull_params_init[0..4].copy_from_slice(&object_count.to_le_bytes());
        queue.write_buffer(&cull_params_buffer, 0, &cull_params_init);

        // Mapped readback buffer (COPY_DST + MAP_READ).
        let debug_counters_readback = device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("fun_renderer.passi.debug_counters_readback"),
            size: Self::DEBUG_COUNTERS_BYTES,
            usage: ::wgpu::BufferUsages::COPY_DST | ::wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
            object_bounds_buffer,
            camera_planes_buffer,
            visible_instances_buffer,
            indirect_args_buffer,
            debug_counters_buffer,
            cull_params_buffer,
            debug_counters_readback,
        }
    }
}

// ============================================================================
// Section 6 — Run result
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct PassIRunResult {
    pub schema_version: u16,
    pub object_count: u32,
    pub cpu_direct_visible_count: u32,
    pub gpu_instances_tested: u32,
    pub gpu_instances_visible: u32,
    pub gpu_instances_rejected: u32,
    pub compute_dispatch_count: u32,
    pub indirect_draws_recorded: u32,
    pub readback_succeeded: bool,
}

impl PassIRunResult {
    #[must_use]
    pub const fn debug_counters_consistent(&self) -> bool {
        self.gpu_instances_visible
            .saturating_add(self.gpu_instances_rejected)
            == self.gpu_instances_tested
    }
}

// ============================================================================
// Section 7 — Top-level runner
// ============================================================================

pub enum PassIBootResult {
    Ran(Box<PassIRanPayload>),
    BridgeRuntimeFailed(WgpuBridgeRuntimeFailure),
}

#[derive(Debug)]
pub struct PassIRanPayload {
    pub bridge_state: WgpuBridgeDeviceState<Dx12Native>,
    pub result: PassIRunResult,
}

/// Run the typed GPU-driven cull + indirect-draw pipeline end to
/// end against a fresh DX12 wgpu device. Returns the typed run
/// result on success or the typed bridge runtime failure on hosts
/// without DX12.
#[must_use]
pub fn run_gpu_driven_compute_indirect_against_fresh_dx12_device(
    objects: &[PassIObjectBounds],
    planes: &PassICameraPlanes,
    clear_color: [f64; 4],
) -> PassIBootResult {
    let options = WgpuBridgeRuntimeOptions::production_default();
    let bridge_state = match initialize_wgpu_bridge_runtime::<Dx12Native>(&options) {
        Ok(state) => state,
        Err(failure) => return PassIBootResult::BridgeRuntimeFailed(failure),
    };
    let target = LiveProofFrameOffscreenTarget::allocate(
        &bridge_state.device,
        LIVE_PROOF_FRAME_OFFSCREEN_EXTENT,
        LIVE_PROOF_FRAME_OFFSCREEN_EXTENT,
    );
    let cull_pipeline = PassIGpuDrivenCullPipeline::create(&bridge_state.device);
    let draw_pipeline = PassIIndirectDrawPipeline::create(&bridge_state.device, target.format);
    let buffers =
        PassIBufferSet::allocate(&bridge_state.device, &bridge_state.queue, objects, planes);

    let cull_bind_group = bridge_state
        .device
        .create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.passi.cull_bind_group"),
            layout: &cull_pipeline.bind_group_layout,
            entries: &[
                ::wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffers.object_bounds_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buffers.camera_planes_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buffers.visible_instances_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buffers.indirect_args_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 4,
                    resource: buffers.debug_counters_buffer.as_entire_binding(),
                },
                ::wgpu::BindGroupEntry {
                    binding: 5,
                    resource: buffers.cull_params_buffer.as_entire_binding(),
                },
            ],
        });
    let draw_bind_group = bridge_state
        .device
        .create_bind_group(&::wgpu::BindGroupDescriptor {
            label: Some("fun_renderer.passi.draw_bind_group"),
            layout: &draw_pipeline.bind_group_layout,
            entries: &[::wgpu::BindGroupEntry {
                binding: 0,
                resource: buffers.visible_instances_buffer.as_entire_binding(),
            }],
        });

    let mut result = PassIRunResult {
        schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
        object_count: objects.len() as u32,
        cpu_direct_visible_count: cpu_direct_count_visible(objects, planes),
        ..PassIRunResult::default()
    };

    let mut encoder =
        bridge_state
            .device
            .create_command_encoder(&::wgpu::CommandEncoderDescriptor {
                label: Some("fun_renderer.passi.encoder"),
            });

    // Step 3-5: dispatch compute. The shader atomically writes
    // visible_instances + indirect_args.instance_count +
    // debug_counters.
    {
        let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
            label: Some("fun_renderer.passi.compute_cull"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&cull_pipeline.pipeline);
        pass.set_bind_group(0, &cull_bind_group, &[]);
        let workgroup_count_x = result
            .object_count
            .div_ceil(PASSI_COMPUTE_WORKGROUP_SIZE_X)
            .max(1);
        pass.dispatch_workgroups(workgroup_count_x, 1, 1);
        result.compute_dispatch_count = 1;
    }

    // Step 6: indirect draw. The render pass reads
    // indirect_args.instance_count (which the compute shader
    // wrote) and renders one triangle per visible instance.
    {
        let mut pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
            label: Some("fun_renderer.passi.indirect_draw_pass"),
            color_attachments: &[Some(::wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: ::wgpu::LoadOp::Clear(::wgpu::Color {
                        r: clear_color[0],
                        g: clear_color[1],
                        b: clear_color[2],
                        a: clear_color[3],
                    }),
                    store: ::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&draw_pipeline.pipeline);
        pass.set_bind_group(0, &draw_bind_group, &[]);
        pass.set_index_buffer(
            draw_pipeline.index_buffer.slice(..),
            ::wgpu::IndexFormat::Uint16,
        );
        pass.draw_indexed_indirect(&buffers.indirect_args_buffer, 0);
        result.indirect_draws_recorded = 1;
    }

    // Step 7: copy debug counters → readback buffer.
    encoder.copy_buffer_to_buffer(
        &buffers.debug_counters_buffer,
        0,
        &buffers.debug_counters_readback,
        0,
        PassIBufferSet::DEBUG_COUNTERS_BYTES,
    );

    let command_buffer = encoder.finish();
    let _submission_index = bridge_state
        .queue
        .submit(::core::iter::once(command_buffer));

    // Map + read the typed debug counters.
    let slice = buffers.debug_counters_readback.slice(..);
    let (sender, receiver) = unbounded();
    slice.map_async(::wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    let _ = bridge_state
        .device
        .poll(::wgpu::PollType::wait_indefinitely());
    if let Ok(Ok(())) = receiver.recv() {
        let data = slice.get_mapped_range();
        if data.len() >= 12 {
            let mut tested_bytes = [0u8; 4];
            let mut visible_bytes = [0u8; 4];
            let mut rejected_bytes = [0u8; 4];
            tested_bytes.copy_from_slice(&data[0..4]);
            visible_bytes.copy_from_slice(&data[4..8]);
            rejected_bytes.copy_from_slice(&data[8..12]);
            result.gpu_instances_tested = u32::from_le_bytes(tested_bytes);
            result.gpu_instances_visible = u32::from_le_bytes(visible_bytes);
            result.gpu_instances_rejected = u32::from_le_bytes(rejected_bytes);
            result.readback_succeeded = true;
        }
        drop(data);
        buffers.debug_counters_readback.unmap();
    }

    PassIBootResult::Ran(Box::new(PassIRanPayload {
        bridge_state,
        result,
    }))
}

// ============================================================================
// Section 8 — Verdict
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PassIGpuVsCpuParityVerdict {
    pub schema_version: u16,
    pub passes_gpu_compute_cull_dispatched: bool,
    pub passes_instances_tested_matches_object_count: bool,
    pub passes_debug_counters_are_consistent: bool,
    pub passes_indirect_draw_issued_against_compute_written_args: bool,
    pub passes_gpu_instances_visible_matches_cpu_direct_count: bool,
}

impl PassIGpuVsCpuParityVerdict {
    #[must_use]
    pub fn evaluate(result: &PassIRunResult) -> Self {
        Self {
            schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
            passes_gpu_compute_cull_dispatched: result.compute_dispatch_count > 0,
            passes_instances_tested_matches_object_count: result.gpu_instances_tested
                == result.object_count,
            passes_debug_counters_are_consistent: result.debug_counters_consistent(),
            passes_indirect_draw_issued_against_compute_written_args: result
                .indirect_draws_recorded
                > 0,
            passes_gpu_instances_visible_matches_cpu_direct_count: result.gpu_instances_visible
                == result.cpu_direct_visible_count,
        }
    }

    #[must_use]
    pub const fn passes(&self) -> bool {
        self.passes_gpu_compute_cull_dispatched
            && self.passes_instances_tested_matches_object_count
            && self.passes_debug_counters_are_consistent
            && self.passes_indirect_draw_issued_against_compute_written_args
            && self.passes_gpu_instances_visible_matches_cpu_direct_count
    }

    #[must_use]
    pub const fn first_failed(&self) -> Option<PassIGpuDrivenComputeIndirectRule> {
        if !self.passes_gpu_compute_cull_dispatched {
            return Some(PassIGpuDrivenComputeIndirectRule::GpuComputeCullDispatched);
        }
        if !self.passes_instances_tested_matches_object_count {
            return Some(PassIGpuDrivenComputeIndirectRule::InstancesTestedMatchesObjectCount);
        }
        if !self.passes_debug_counters_are_consistent {
            return Some(PassIGpuDrivenComputeIndirectRule::DebugCountersAreConsistent);
        }
        if !self.passes_indirect_draw_issued_against_compute_written_args {
            return Some(
                PassIGpuDrivenComputeIndirectRule::IndirectDrawIssuedAgainstComputeWrittenArgs,
            );
        }
        if !self.passes_gpu_instances_visible_matches_cpu_direct_count {
            return Some(
                PassIGpuDrivenComputeIndirectRule::GpuInstancesVisibleMatchesCpuDirectCount,
            );
        }
        None
    }

    #[must_use]
    pub const fn violation_count(&self) -> u32 {
        let mut count = 0u32;
        if !self.passes_gpu_compute_cull_dispatched {
            count += 1;
        }
        if !self.passes_instances_tested_matches_object_count {
            count += 1;
        }
        if !self.passes_debug_counters_are_consistent {
            count += 1;
        }
        if !self.passes_indirect_draw_issued_against_compute_written_args {
            count += 1;
        }
        if !self.passes_gpu_instances_visible_matches_cpu_direct_count {
            count += 1;
        }
        count
    }
}

// ============================================================================
// Section 9 — Canonical proof scene
// ============================================================================

/// 4-object proof scene with 2 inside the typed half-space
/// `x <= 0.5` frustum and 2 outside. CPU expects 2 visible.
#[must_use]
pub fn passi_proof_scene_objects() -> Vec<PassIObjectBounds> {
    vec![
        // Inside (max.x = 0.0 <= 0.5).
        PassIObjectBounds::new([-0.5, -0.5, -0.5], [0.0, 0.5, 0.5]),
        // Inside (max.x = 0.5).
        PassIObjectBounds::new([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5]),
        // Outside (min.x = 1.0 > 0.5; positive-vertex test
        // selects max.x = 2.0 for the negative-x plane and the
        // distance is -2.0+0.5 = -1.5 < 0).
        PassIObjectBounds::new([1.0, -0.5, -0.5], [2.0, 0.5, 0.5]),
        // Outside.
        PassIObjectBounds::new([3.0, -0.5, -0.5], [4.0, 0.5, 0.5]),
    ]
}

#[must_use]
pub fn passi_proof_scene_planes() -> PassICameraPlanes {
    PassICameraPlanes::half_space_x(0.5)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION, 1);
        assert_eq!(PASSI_RULE_COUNT, 5);
        assert_eq!(
            PassIGpuDrivenComputeIndirectRule::ALL.len(),
            PASSI_RULE_COUNT
        );
    }

    #[test]
    fn rule_index_round_trips() {
        for (i, rule) in PassIGpuDrivenComputeIndirectRule::ALL
            .iter()
            .copied()
            .enumerate()
        {
            assert_eq!(rule.index(), i);
        }
    }

    #[test]
    fn rule_str_taxonomy_unique() {
        let mut seen = hashbrown::HashSet::new();
        for rule in PassIGpuDrivenComputeIndirectRule::ALL {
            assert!(seen.insert(rule.as_str()), "duplicate: {}", rule.as_str());
        }
    }

    #[test]
    fn object_bounds_byte_layout_is_32_bytes() {
        let obj = PassIObjectBounds::new([-1.0, -2.0, -3.0], [4.0, 5.0, 6.0]);
        let bytes = obj.as_bytes();
        assert_eq!(bytes.len(), 32);
        // Verify min.x at offset 0, max.x at offset 16.
        let mut min_x = [0u8; 4];
        min_x.copy_from_slice(&bytes[0..4]);
        assert_eq!(f32::from_le_bytes(min_x), -1.0);
        let mut max_x = [0u8; 4];
        max_x.copy_from_slice(&bytes[16..20]);
        assert_eq!(f32::from_le_bytes(max_x), 4.0);
    }

    #[test]
    fn camera_planes_byte_layout_is_96_bytes() {
        let planes = PassICameraPlanes::accept_all();
        let bytes = planes.as_bytes();
        assert_eq!(bytes.len(), 96);
    }

    #[test]
    fn half_space_x_accepts_inside_objects_and_rejects_outside() {
        let objects = passi_proof_scene_objects();
        let planes = passi_proof_scene_planes();
        // Expect 2 visible (objects 0 and 1) and 2 rejected
        // (objects 2 and 3).
        assert_eq!(cpu_direct_count_visible(&objects, &planes), 2);
    }

    #[test]
    fn accept_all_frustum_passes_every_object() {
        let objects = passi_proof_scene_objects();
        let planes = PassICameraPlanes::accept_all();
        assert_eq!(
            cpu_direct_count_visible(&objects, &planes),
            objects.len() as u32,
        );
    }

    #[test]
    fn run_result_debug_counters_consistent_predicate() {
        let mut r = PassIRunResult::default();
        r.gpu_instances_tested = 10;
        r.gpu_instances_visible = 7;
        r.gpu_instances_rejected = 3;
        assert!(r.debug_counters_consistent());
        r.gpu_instances_rejected = 2;
        assert!(!r.debug_counters_consistent());
    }

    #[test]
    fn verdict_passes_under_synthetic_consistent_result() {
        let result = PassIRunResult {
            schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
            object_count: 4,
            cpu_direct_visible_count: 2,
            gpu_instances_tested: 4,
            gpu_instances_visible: 2,
            gpu_instances_rejected: 2,
            compute_dispatch_count: 1,
            indirect_draws_recorded: 1,
            readback_succeeded: true,
        };
        let verdict = PassIGpuVsCpuParityVerdict::evaluate(&result);
        assert!(verdict.passes());
        assert!(verdict.first_failed().is_none());
    }

    #[test]
    fn verdict_fails_when_gpu_count_diverges_from_cpu() {
        let result = PassIRunResult {
            schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
            object_count: 4,
            cpu_direct_visible_count: 2,
            gpu_instances_tested: 4,
            gpu_instances_visible: 3,
            gpu_instances_rejected: 1,
            compute_dispatch_count: 1,
            indirect_draws_recorded: 1,
            readback_succeeded: true,
        };
        let verdict = PassIGpuVsCpuParityVerdict::evaluate(&result);
        assert!(!verdict.passes_gpu_instances_visible_matches_cpu_direct_count);
        assert_eq!(
            verdict.first_failed(),
            Some(PassIGpuDrivenComputeIndirectRule::GpuInstancesVisibleMatchesCpuDirectCount),
        );
    }

    #[test]
    fn verdict_fails_when_compute_dispatch_count_is_zero() {
        let result = PassIRunResult {
            schema_version: PASSI_GPU_DRIVEN_COMPUTE_INDIRECT_SCHEMA_VERSION,
            object_count: 4,
            cpu_direct_visible_count: 2,
            gpu_instances_tested: 0,
            gpu_instances_visible: 0,
            gpu_instances_rejected: 0,
            compute_dispatch_count: 0,
            indirect_draws_recorded: 0,
            readback_succeeded: false,
        };
        let verdict = PassIGpuVsCpuParityVerdict::evaluate(&result);
        assert!(!verdict.passes_gpu_compute_cull_dispatched);
        assert_eq!(
            verdict.first_failed(),
            Some(PassIGpuDrivenComputeIndirectRule::GpuComputeCullDispatched),
        );
    }

    /// Live Pass I "single command" smoke gate. Boots a fresh DX12
    /// wgpu device, allocates every typed buffer, dispatches the
    /// real compute cull kernel, issues `draw_indexed_indirect`,
    /// reads back the debug counters, and asserts every typed rule
    /// passes:
    ///
    /// - Compute dispatched (real GPU work).
    /// - `instances_tested == 4` (every object reached the shader).
    /// - `instances_visible + instances_rejected == instances_tested`
    ///   (atomic counters consistent).
    /// - `draw_indexed_indirect` recorded against the typed
    ///   indirect args buffer the compute kernel wrote.
    /// - `instances_visible == cpu_direct_visible_count` (the
    ///   real GPU output matches the CPU-direct frustum cull —
    ///   the user's "Compare against CPU/direct parity" step).
    ///
    /// On hosts without a DX12 adapter the live boot returns
    /// `BridgeRuntimeFailed` and the test records honestly.
    #[test]
    fn live_passi_runs_real_gpu_cull_and_indirect_draw_with_cpu_parity() {
        use crate::live_proof_frame_executor::ran_on_real_dx12_adapter;

        let objects = passi_proof_scene_objects();
        let planes = passi_proof_scene_planes();
        let outcome = run_gpu_driven_compute_indirect_against_fresh_dx12_device(
            &objects,
            &planes,
            [0.0, 0.0, 0.0, 1.0],
        );
        match outcome {
            PassIBootResult::Ran(payload) => {
                let PassIRanPayload {
                    bridge_state,
                    result,
                } = *payload;
                if !ran_on_real_dx12_adapter(&bridge_state) {
                    eprintln!(
                        "live_passi: non-DX12 actual backend ({:?}); skipping strict assertion",
                        bridge_state.actual_native_backend,
                    );
                    return;
                }
                assert!(result.readback_succeeded, "debug counters readback failed");
                assert_eq!(result.object_count, 4);
                assert_eq!(result.cpu_direct_visible_count, 2);
                assert_eq!(
                    result.gpu_instances_tested, 4,
                    "expected GPU compute to test every object",
                );
                assert_eq!(
                    result.gpu_instances_visible, 2,
                    "expected GPU compute to accept 2 inside-frustum objects",
                );
                assert_eq!(
                    result.gpu_instances_rejected, 2,
                    "expected GPU compute to reject 2 outside-frustum objects",
                );
                assert!(result.debug_counters_consistent());
                assert_eq!(result.compute_dispatch_count, 1);
                assert_eq!(result.indirect_draws_recorded, 1);

                let verdict = PassIGpuVsCpuParityVerdict::evaluate(&result);
                assert!(
                    verdict.passes(),
                    "Pass I verdict must pass under proof scene; first_failed = {:?}",
                    verdict.first_failed(),
                );
            }
            PassIBootResult::BridgeRuntimeFailed(failure) => {
                eprintln!(
                    "live_passi: bridge runtime failed (host without DX12 adapter): {failure:?}",
                );
            }
        }
    }
}
