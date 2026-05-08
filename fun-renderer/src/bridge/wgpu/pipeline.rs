use std::borrow::Cow;

use crate::{
    bridge::wgpu::{binding::shader_stages, resource::texture_format},
    ir::{
        ComputePipelineDesc, CullMode, PipelineLayoutDesc, PrimitiveTopology, RenderPipelineDesc,
        ShaderModuleDesc,
    },
};

#[must_use]
pub fn pipeline_layout_descriptor<'a>(
    desc: PipelineLayoutDesc,
    bind_group_layouts: &'a [Option<&'a ::wgpu::BindGroupLayout>],
) -> ::wgpu::PipelineLayoutDescriptor<'a> {
    ::wgpu::PipelineLayoutDescriptor {
        label: Some(desc.stable_name),
        bind_group_layouts,
        immediate_size: desc
            .root_constants
            .iter()
            .take(desc.root_constant_count as usize)
            .map(|constant| u32::from(constant.byte_offset) + u32::from(constant.byte_count))
            .max()
            .unwrap_or(0),
    }
}

#[must_use]
pub fn shader_module_descriptor_wgsl<'a>(
    desc: ShaderModuleDesc,
    source: Cow<'a, str>,
) -> ::wgpu::ShaderModuleDescriptor<'a> {
    let _stages = shader_stages(desc.stages);
    ::wgpu::ShaderModuleDescriptor {
        label: Some(desc.stable_name),
        source: ::wgpu::ShaderSource::Wgsl(source),
    }
}

#[must_use]
pub fn render_pipeline_descriptor<'a>(
    desc: RenderPipelineDesc,
    layout: &'a ::wgpu::PipelineLayout,
    vertex_shader: &'a ::wgpu::ShaderModule,
    fragment_shader: &'a ::wgpu::ShaderModule,
    vertex_buffers: &'a [::wgpu::VertexBufferLayout<'a>],
    color_targets: &'a [Option<::wgpu::ColorTargetState>],
) -> ::wgpu::RenderPipelineDescriptor<'a> {
    ::wgpu::RenderPipelineDescriptor {
        label: Some(desc.stable_name),
        layout: Some(layout),
        vertex: ::wgpu::VertexState {
            module: vertex_shader,
            entry_point: None,
            compilation_options: Default::default(),
            buffers: vertex_buffers,
        },
        primitive: primitive_state(desc.topology, desc.cull_mode),
        depth_stencil: depth_stencil_state(desc),
        multisample: multisample_state(desc.sample_count),
        fragment: Some(::wgpu::FragmentState {
            module: fragment_shader,
            entry_point: None,
            compilation_options: Default::default(),
            targets: color_targets,
        }),
        multiview_mask: None,
        cache: None,
    }
}

#[must_use]
pub fn compute_pipeline_descriptor<'a>(
    desc: ComputePipelineDesc,
    layout: &'a ::wgpu::PipelineLayout,
    compute_shader: &'a ::wgpu::ShaderModule,
) -> ::wgpu::ComputePipelineDescriptor<'a> {
    ::wgpu::ComputePipelineDescriptor {
        label: Some(desc.stable_name),
        layout: Some(layout),
        module: compute_shader,
        entry_point: None,
        compilation_options: Default::default(),
        cache: None,
    }
}

#[must_use]
pub fn color_target_states(
    desc: RenderPipelineDesc,
) -> [Option<::wgpu::ColorTargetState>; crate::ir::MAX_RENDER_TARGETS_PER_PASS] {
    let mut targets: [Option<::wgpu::ColorTargetState>; crate::ir::MAX_RENDER_TARGETS_PER_PASS] =
        Default::default();
    for (target, format) in targets.iter_mut().zip(
        desc.color_formats
            .iter()
            .take(desc.color_target_count as usize),
    ) {
        *target = Some(::wgpu::ColorTargetState {
            format: texture_format(*format),
            blend: None,
            write_mask: ::wgpu::ColorWrites::ALL,
        });
    }
    targets
}

#[must_use]
pub const fn primitive_state(
    topology: PrimitiveTopology,
    cull_mode: CullMode,
) -> ::wgpu::PrimitiveState {
    ::wgpu::PrimitiveState {
        topology: primitive_topology(topology),
        strip_index_format: None,
        front_face: ::wgpu::FrontFace::Ccw,
        cull_mode: cull_mode_to_wgpu(cull_mode),
        polygon_mode: ::wgpu::PolygonMode::Fill,
        unclipped_depth: false,
        conservative: false,
    }
}

#[must_use]
pub const fn primitive_topology(topology: PrimitiveTopology) -> ::wgpu::PrimitiveTopology {
    match topology {
        PrimitiveTopology::TriangleList => ::wgpu::PrimitiveTopology::TriangleList,
        PrimitiveTopology::TriangleStrip => ::wgpu::PrimitiveTopology::TriangleStrip,
        PrimitiveTopology::LineList => ::wgpu::PrimitiveTopology::LineList,
    }
}

#[must_use]
pub const fn cull_mode_to_wgpu(cull_mode: CullMode) -> Option<::wgpu::Face> {
    match cull_mode {
        CullMode::None => None,
        CullMode::Front => Some(::wgpu::Face::Front),
        CullMode::Back => Some(::wgpu::Face::Back),
    }
}

#[must_use]
pub fn depth_stencil_state(desc: RenderPipelineDesc) -> Option<::wgpu::DepthStencilState> {
    if desc.depth_format.is_depth() {
        Some(::wgpu::DepthStencilState {
            format: texture_format(desc.depth_format),
            depth_write_enabled: Some(true),
            depth_compare: Some(::wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        })
    } else {
        None
    }
}

#[must_use]
pub const fn multisample_state(sample_count: u8) -> ::wgpu::MultisampleState {
    ::wgpu::MultisampleState {
        count: sample_count as u32,
        mask: !0,
        alpha_to_coverage_enabled: false,
    }
}
