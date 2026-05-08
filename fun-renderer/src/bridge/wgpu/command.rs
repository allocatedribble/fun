use crate::ir::{
    CompiledGraph, ComputePassCommand, CopyCommand, GraphPassDesc, GraphPassKind, RenderPassCommand,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuRenderBatch {
    pub command: RenderPassCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuComputeBatch {
    pub command: ComputePassCommand,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WgpuPassExecutionArrays {
    pub render_batches: Vec<WgpuRenderBatch>,
    pub compute_batches: Vec<WgpuComputeBatch>,
    pub copy_ops: Vec<CopyCommand>,
}

impl WgpuPassExecutionArrays {
    #[must_use]
    pub fn from_compiled_graph(graph: &CompiledGraph) -> Self {
        let mut arrays = Self::default();
        for plan in &graph.passes {
            match plan.command_kind {
                GraphPassKind::Render => {
                    if let Some(command) = plan.render_command {
                        arrays.render_batches.push(WgpuRenderBatch { command });
                    }
                }
                GraphPassKind::Compute => {
                    if let Some(command) = plan.compute_command {
                        arrays.compute_batches.push(WgpuComputeBatch { command });
                    }
                }
                GraphPassKind::Copy => {
                    if let Some(command) = plan.copy_command {
                        arrays.copy_ops.push(command);
                    }
                }
                GraphPassKind::Present => {}
            }
        }
        arrays
    }
}

#[derive(Debug)]
pub enum WgpuPassDescriptor<'a> {
    Render(::wgpu::RenderPassDescriptor<'a>),
    Compute(::wgpu::ComputePassDescriptor<'a>),
    Copy(CopyCommand),
    Present,
}

#[must_use]
pub fn pass_descriptor<'a>(
    pass: GraphPassDesc,
    color_attachments: &'a [Option<::wgpu::RenderPassColorAttachment<'a>>],
    depth_stencil_attachment: Option<::wgpu::RenderPassDepthStencilAttachment<'a>>,
) -> WgpuPassDescriptor<'a> {
    match pass.kind {
        GraphPassKind::Render => WgpuPassDescriptor::Render(::wgpu::RenderPassDescriptor {
            label: Some(pass.stable_name),
            color_attachments,
            depth_stencil_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        }),
        GraphPassKind::Compute => WgpuPassDescriptor::Compute(::wgpu::ComputePassDescriptor {
            label: Some(pass.stable_name),
            timestamp_writes: None,
        }),
        GraphPassKind::Copy => WgpuPassDescriptor::Copy(pass.copy_command.unwrap_or_default()),
        GraphPassKind::Present => WgpuPassDescriptor::Present,
    }
}
