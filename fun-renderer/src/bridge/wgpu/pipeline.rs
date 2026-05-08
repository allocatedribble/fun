use std::borrow::Cow;

use crate::{
    FunRendererBackend,
    bridge::wgpu::{binding::shader_stages, resource::texture_format},
    ir::{
        ComputePipelineDesc, CullMode, DescriptorFingerprint, PipelineLayoutDesc,
        PrimitiveTopology, RenderPipelineDesc, ShaderModuleDesc,
    },
    pipeline::{
        BlendState, PipelineCache, PipelineCacheDecision, PipelineCacheError, PipelineCacheKey,
        PipelineCreationPhase, PipelineFeatureMask, QualityTier, RenderPipelineKeyDesc,
        ShaderEntrySet, VertexLayout,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuRenderPipelinePrepareDesc {
    pub desc: RenderPipelineDesc,
    pub shader_entries: ShaderEntrySet,
    pub reflection_signature: DescriptorFingerprint,
    pub bind_layout_hash: DescriptorFingerprint,
    pub blend_state: BlendState,
    pub vertex_layout: VertexLayout,
    pub quality_tier: QualityTier,
    pub feature_mask: PipelineFeatureMask,
    pub backend: FunRendererBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WgpuComputePipelinePrepareDesc {
    pub desc: ComputePipelineDesc,
    pub shader_entries: ShaderEntrySet,
    pub reflection_signature: DescriptorFingerprint,
    pub bind_layout_hash: DescriptorFingerprint,
    pub quality_tier: QualityTier,
    pub feature_mask: PipelineFeatureMask,
    pub backend: FunRendererBackend,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WgpuPipelineBridgeCache {
    pub pipeline_cache: PipelineCache,
}

impl WgpuPipelineBridgeCache {
    pub fn prepare_shader_module(
        &mut self,
        desc: ShaderModuleDesc,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.pipeline_cache.ensure_shader_module(
            desc.stable_name,
            shader_module_fingerprint(desc),
            phase,
        )
    }

    pub fn prepare_pipeline_layout(
        &mut self,
        desc: PipelineLayoutDesc,
        bind_layout_hash: DescriptorFingerprint,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.pipeline_cache.ensure_pipeline_layout(
            desc.stable_name,
            pipeline_layout_fingerprint(desc, bind_layout_hash),
            phase,
        )
    }

    pub fn prepare_render_pipeline(
        &mut self,
        desc: WgpuRenderPipelinePrepareDesc,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.pipeline_cache
            .ensure_render_pipeline(render_pipeline_cache_key_from_desc(desc), phase)
    }

    pub fn prepare_compute_pipeline(
        &mut self,
        desc: WgpuComputePipelinePrepareDesc,
        phase: PipelineCreationPhase,
    ) -> Result<PipelineCacheDecision, PipelineCacheError> {
        self.pipeline_cache
            .ensure_compute_pipeline(compute_pipeline_cache_key_from_desc(desc), phase)
    }
}

#[must_use]
pub fn render_pipeline_cache_key_from_desc(
    desc: WgpuRenderPipelinePrepareDesc,
) -> PipelineCacheKey {
    PipelineCacheKey::from_render_desc(RenderPipelineKeyDesc {
        pipeline: desc.desc,
        shader_entries: desc.shader_entries,
        reflection_signature: desc.reflection_signature,
        bind_layout_hash: desc.bind_layout_hash,
        blend_state: desc.blend_state,
        vertex_layout: desc.vertex_layout,
        quality_tier: desc.quality_tier,
        feature_mask: desc.feature_mask,
        backend: desc.backend,
    })
}

#[must_use]
pub fn compute_pipeline_cache_key_from_desc(
    desc: WgpuComputePipelinePrepareDesc,
) -> PipelineCacheKey {
    PipelineCacheKey::from_compute_desc(
        desc.desc,
        desc.shader_entries,
        desc.reflection_signature,
        desc.bind_layout_hash,
        desc.quality_tier,
        desc.feature_mask,
        desc.backend,
    )
}

#[must_use]
pub fn shader_module_fingerprint(desc: ShaderModuleDesc) -> DescriptorFingerprint {
    DescriptorFingerprint::from_label_and_words(
        desc.stable_name,
        &[
            u64::from(desc.id.0),
            u64::from(desc.schema_version),
            desc.language as u64,
            label_fingerprint(desc.entry_point),
            u64::from(desc.stages.0),
            desc.source_digest,
            desc.requires_naga_translation as u64,
        ],
    )
}

#[must_use]
pub fn pipeline_layout_fingerprint(
    desc: PipelineLayoutDesc,
    bind_layout_hash: DescriptorFingerprint,
) -> DescriptorFingerprint {
    DescriptorFingerprint::from_label_and_words(
        desc.stable_name,
        &[
            u64::from(desc.id.0),
            u64::from(desc.schema_version),
            u64::from(desc.bind_layout_count),
            u64::from(desc.bind_layouts[0].0),
            u64::from(desc.bind_layouts[1].0),
            u64::from(desc.bind_layouts[2].0),
            u64::from(desc.bind_layouts[3].0),
            u64::from(desc.root_constant_count),
            bind_layout_hash.0,
        ],
    )
}

fn label_fingerprint(label: &'static str) -> u64 {
    DescriptorFingerprint::from_label_and_words(label, &[]).0
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ir::{
            IrBindLayoutId, IrComputePipelineId, IrPipelineLayoutId, IrRenderPipelineId,
            IrShaderModuleId, PIPELINE_IR_SCHEMA_VERSION, ShaderStageMask, TextureFormat,
        },
        pipeline::{PipelineCreationKind, ShaderEntry},
    };

    #[test]
    fn wgpu_pipeline_bridge_prepares_handles_during_warmup_and_reuses() {
        let mut cache = WgpuPipelineBridgeCache::default();
        let shader = ShaderModuleDesc::wgsl(
            IrShaderModuleId::new(1),
            "test.wgpu.shader",
            "main",
            ShaderStageMask::VERTEX.union(ShaderStageMask::FRAGMENT),
            100,
        );
        let layout = pipeline_layout_desc("test.wgpu.layout");
        let render = render_prepare_desc("test.wgpu.render");

        let shader_decision = cache
            .prepare_shader_module(shader, PipelineCreationPhase::Warmup)
            .expect("shader module warms up");
        let layout_decision = cache
            .prepare_pipeline_layout(
                layout,
                DescriptorFingerprint::from_u64(101),
                PipelineCreationPhase::Warmup,
            )
            .expect("pipeline layout warms up");
        let render_warmup = cache
            .prepare_render_pipeline(render, PipelineCreationPhase::Warmup)
            .expect("render pipeline warms up");
        let render_runtime = cache
            .prepare_render_pipeline(render, PipelineCreationPhase::RuntimeMeasured)
            .expect("prepared render pipeline is reused");

        assert!(shader_decision.created);
        assert!(layout_decision.created);
        assert!(render_warmup.created);
        assert!(!render_runtime.created);
        assert_eq!(
            render_warmup.prepared_pipeline,
            render_runtime.prepared_pipeline
        );
        assert_eq!(cache.pipeline_cache.telemetry.shader_modules_created, 1);
        assert_eq!(cache.pipeline_cache.telemetry.pipeline_layouts_created, 1);
        assert_eq!(cache.pipeline_cache.telemetry.render_pipelines_created, 1);
    }

    #[test]
    fn wgpu_pipeline_bridge_blocks_measured_runtime_creation() {
        let mut cache = WgpuPipelineBridgeCache::default();
        let render = render_prepare_desc("test.wgpu.runtime.render");
        let compute = compute_prepare_desc("test.wgpu.runtime.compute");

        assert_eq!(
            cache.prepare_render_pipeline(render, PipelineCreationPhase::RuntimeMeasured),
            Err(PipelineCacheError::RuntimeCreationAfterWarmup {
                kind: PipelineCreationKind::RenderPipeline,
                stable_name: "test.wgpu.runtime.render",
            })
        );
        assert_eq!(
            cache.prepare_compute_pipeline(compute, PipelineCreationPhase::RuntimeMeasured),
            Err(PipelineCacheError::RuntimeCreationAfterWarmup {
                kind: PipelineCreationKind::ComputePipeline,
                stable_name: "test.wgpu.runtime.compute",
            })
        );
        assert_eq!(cache.pipeline_cache.telemetry.runtime_creation_failures, 2);
    }

    fn render_prepare_desc(stable_name: &'static str) -> WgpuRenderPipelinePrepareDesc {
        WgpuRenderPipelinePrepareDesc {
            desc: RenderPipelineDesc {
                id: IrRenderPipelineId::new(1),
                stable_name,
                color_target_count: 1,
                color_formats: [
                    TextureFormat::Rgba16Float,
                    TextureFormat::Undefined,
                    TextureFormat::Undefined,
                    TextureFormat::Undefined,
                ],
                depth_format: TextureFormat::Depth32Float,
                ..RenderPipelineDesc::default()
            },
            shader_entries: ShaderEntrySet::render(
                ShaderEntry {
                    shader_hash: 100,
                    module_label: "test.wgpu.vertex",
                    entry_point: "vs_main",
                },
                ShaderEntry {
                    shader_hash: 101,
                    module_label: "test.wgpu.fragment",
                    entry_point: "fs_main",
                },
            ),
            reflection_signature: DescriptorFingerprint::from_u64(102),
            bind_layout_hash: DescriptorFingerprint::from_u64(103),
            blend_state: BlendState::OPAQUE,
            vertex_layout: VertexLayout::EMPTY,
            quality_tier: QualityTier::Balanced,
            feature_mask: PipelineFeatureMask::NONE,
            backend: FunRendererBackend::Dx12,
        }
    }

    fn compute_prepare_desc(stable_name: &'static str) -> WgpuComputePipelinePrepareDesc {
        WgpuComputePipelinePrepareDesc {
            desc: ComputePipelineDesc {
                id: IrComputePipelineId::new(1),
                schema_version: PIPELINE_IR_SCHEMA_VERSION,
                stable_name,
                layout: IrPipelineLayoutId::new(1),
                compute_shader: IrShaderModuleId::new(1),
                requires_work_graphs: false,
            },
            shader_entries: ShaderEntrySet::compute(ShaderEntry {
                shader_hash: 104,
                module_label: "test.wgpu.compute",
                entry_point: "main",
            }),
            reflection_signature: DescriptorFingerprint::from_u64(105),
            bind_layout_hash: DescriptorFingerprint::from_u64(106),
            quality_tier: QualityTier::Balanced,
            feature_mask: PipelineFeatureMask::COMPUTE_CULLING,
            backend: FunRendererBackend::Dx12,
        }
    }

    fn pipeline_layout_desc(stable_name: &'static str) -> PipelineLayoutDesc {
        PipelineLayoutDesc {
            id: IrPipelineLayoutId::new(1),
            stable_name,
            bind_layouts: [
                IrBindLayoutId::new(1),
                IrBindLayoutId::INVALID,
                IrBindLayoutId::INVALID,
                IrBindLayoutId::INVALID,
            ],
            bind_layout_count: 1,
            ..PipelineLayoutDesc::default()
        }
    }
}
