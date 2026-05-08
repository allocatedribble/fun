use crate::{
    backend::{BackendCapabilityReport, NagaShaderTranslationStatus},
    ir::{BindingType, RootConstantDesc, ShaderModuleDesc, ShaderSourceLanguage, ShaderStageMask},
    shader::{
        RendererShaderSchema, RendererShaderSourceStrategy, ShaderDiagnosticArtifact,
        ShaderDiagnosticStatus, ShaderIoValueFormat, ShaderReflectedEntryPoint, ShaderReflection,
        ShaderReflectionOverflow, ShaderRenderTargetOutputDesc, ShaderResourceBindingSlot,
        ShaderTranslationCache, ShaderTranslationCacheDecision, ShaderTranslationCacheFailure,
        ShaderTranslationCacheKey, ShaderTranslationPhase, ShaderTranslationTarget,
        ShaderVertexInputDesc, ShaderWorkgroupShape, stable_label_hash,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuShaderTranslationPath {
    WgslThroughNaga,
    NagaIrPassthrough,
    SpirvPassthrough,
    BackendSpecificSource,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuNagaBridgeStatus {
    pub status: NagaShaderTranslationStatus,
    pub exposed_to_ecs: bool,
}

#[must_use]
pub const fn naga_bridge_status(report: BackendCapabilityReport) -> WgpuNagaBridgeStatus {
    WgpuNagaBridgeStatus {
        status: report.naga_shader_translation,
        exposed_to_ecs: false,
    }
}

#[must_use]
pub const fn shader_translation_path(desc: ShaderModuleDesc) -> WgpuShaderTranslationPath {
    match desc.language {
        ShaderSourceLanguage::Wgsl => WgpuShaderTranslationPath::WgslThroughNaga,
        ShaderSourceLanguage::NagaIr => WgpuShaderTranslationPath::NagaIrPassthrough,
        ShaderSourceLanguage::SpirV => WgpuShaderTranslationPath::SpirvPassthrough,
        ShaderSourceLanguage::Hlsl | ShaderSourceLanguage::Msl => {
            WgpuShaderTranslationPath::BackendSpecificSource
        }
    }
}

#[must_use]
pub const fn renderer_source_strategy(
    desc: ShaderModuleDesc,
    _target: ShaderTranslationTarget,
) -> RendererShaderSourceStrategy {
    match desc.language {
        ShaderSourceLanguage::Wgsl => RendererShaderSourceStrategy::WgslThroughNaga,
        ShaderSourceLanguage::NagaIr => RendererShaderSourceStrategy::NagaIrPassthrough,
        ShaderSourceLanguage::SpirV => RendererShaderSourceStrategy::SpirvVulkan,
        ShaderSourceLanguage::Hlsl => RendererShaderSourceStrategy::HlslDxcDx12,
        ShaderSourceLanguage::Msl => RendererShaderSourceStrategy::MslMetal,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NagaShaderBridgeFailure {
    Cache(ShaderTranslationCacheFailure),
    UnsupportedSourceStrategy {
        strategy: RendererShaderSourceStrategy,
        target: ShaderTranslationTarget,
    },
    WgslParseFailed {
        message_hash: u64,
        message_len: u32,
    },
    ValidationFailed {
        message_hash: u64,
        message_len: u32,
    },
    ReflectionFailed {
        overflow: ShaderReflectionOverflow,
    },
    MissingEntryPoint {
        entry_point_hash: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NagaShaderTranslation {
    pub path: WgpuShaderTranslationPath,
    pub cache_decision: ShaderTranslationCacheDecision,
    pub reflection: ShaderReflection,
    pub diagnostic: ShaderDiagnosticArtifact,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NagaShaderBridge {
    pub cache: ShaderTranslationCache,
}

impl NagaShaderBridge {
    pub fn translate_shader_source(
        &mut self,
        shader: RendererShaderSchema,
        desc: ShaderModuleDesc,
        source: &str,
        target: ShaderTranslationTarget,
        phase: ShaderTranslationPhase,
    ) -> Result<NagaShaderTranslation, NagaShaderBridgeFailure> {
        let strategy = renderer_source_strategy(desc, target);
        let key = ShaderTranslationCacheKey::new(shader, desc.source_digest, target);
        let path = shader_translation_path(desc);
        if let Some(entry) = self.cache.lookup(key) {
            return Ok(NagaShaderTranslation {
                path,
                cache_decision: ShaderTranslationCacheDecision {
                    key,
                    translated: false,
                    reused: true,
                },
                reflection: entry.reflection,
                diagnostic: entry.diagnostic,
            });
        }
        self.cache
            .ensure_can_translate(key, phase)
            .map_err(NagaShaderBridgeFailure::Cache)?;

        if strategy != RendererShaderSourceStrategy::WgslThroughNaga {
            return Err(NagaShaderBridgeFailure::UnsupportedSourceStrategy { strategy, target });
        }

        let module = ::naga::front::wgsl::parse_str(source).map_err(|error| {
            let stats = sanitized_error_stats(&format!("{error:?}"));
            NagaShaderBridgeFailure::WgslParseFailed {
                message_hash: stats.message_hash,
                message_len: stats.message_len,
            }
        })?;
        let mut validator = ::naga::valid::Validator::new(
            ::naga::valid::ValidationFlags::all(),
            ::naga::valid::Capabilities::default(),
        );
        validator.validate(&module).map_err(|error| {
            let stats = sanitized_error_stats(&format!("{error:?}"));
            NagaShaderBridgeFailure::ValidationFailed {
                message_hash: stats.message_hash,
                message_len: stats.message_len,
            }
        })?;

        let reflection = reflect_naga_module(&module, desc)?;
        let mut diagnostic = ShaderDiagnosticArtifact::new(
            shader,
            desc.source_digest,
            target,
            ShaderDiagnosticStatus::ParsedValidatedReflected,
        )
        .with_reflection(reflection);
        if shader.entry_point != desc.entry_point {
            diagnostic.reflection_error_count = diagnostic.reflection_error_count.saturating_add(1);
        }
        let cache_decision = self.cache.insert(key, reflection, diagnostic);

        Ok(NagaShaderTranslation {
            path,
            cache_decision,
            reflection,
            diagnostic,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SanitizedErrorStats {
    message_hash: u64,
    message_len: u32,
}

#[must_use]
fn sanitized_error_stats(message: &str) -> SanitizedErrorStats {
    SanitizedErrorStats {
        message_hash: stable_label_hash(message),
        message_len: message.len().min(u32::MAX as usize) as u32,
    }
}

fn reflect_naga_module(
    module: &::naga::Module,
    desc: ShaderModuleDesc,
) -> Result<ShaderReflection, NagaShaderBridgeFailure> {
    let mut reflection = ShaderReflection::new(desc.source_digest);
    for (_, variable) in module.global_variables.iter() {
        if let Some(binding) = variable.binding {
            reflection
                .push_resource_binding(ShaderResourceBindingSlot {
                    group: bounded_u8(binding.group),
                    binding: bounded_u8(binding.binding),
                    binding_type: binding_type_for_global(module, variable),
                    stages: desc.stages,
                })
                .map_err(|overflow| NagaShaderBridgeFailure::ReflectionFailed { overflow })?;
        }
        if matches!(variable.space, ::naga::AddressSpace::Immediate) {
            reflection
                .push_root_constant(RootConstantDesc {
                    slot: 0,
                    byte_offset: 0,
                    byte_count: 0,
                    stages: desc.stages,
                })
                .map_err(|overflow| NagaShaderBridgeFailure::ReflectionFailed { overflow })?;
        }
        if is_storage_or_uav_global(module, variable) {
            reflection.storage_or_uav_usage = true;
        }
    }

    let mut selected_entry_point_seen = false;
    for entry_point in &module.entry_points {
        let stages = stage_mask(entry_point.stage);
        let workgroup_size = ShaderWorkgroupShape::new(
            entry_point.workgroup_size[0],
            entry_point.workgroup_size[1],
            entry_point.workgroup_size[2],
        );
        reflection
            .push_entry_point(ShaderReflectedEntryPoint {
                name_hash: stable_label_hash(&entry_point.name),
                stages,
                workgroup_size,
            })
            .map_err(|overflow| NagaShaderBridgeFailure::ReflectionFailed { overflow })?;

        if entry_point.name == desc.entry_point {
            selected_entry_point_seen = true;
            reflection.workgroup_size = workgroup_size;
            collect_entry_point_io(module, entry_point, &mut reflection)?;
        }
    }

    if !selected_entry_point_seen {
        return Err(NagaShaderBridgeFailure::MissingEntryPoint {
            entry_point_hash: stable_label_hash(desc.entry_point),
        });
    }

    Ok(reflection)
}

fn collect_entry_point_io(
    module: &::naga::Module,
    entry_point: &::naga::EntryPoint,
    reflection: &mut ShaderReflection,
) -> Result<(), NagaShaderBridgeFailure> {
    for argument in &entry_point.function.arguments {
        collect_io_binding(
            module,
            argument.ty,
            argument.binding.as_ref(),
            entry_point.stage,
            ShaderIoDirection::Input,
            reflection,
        )?;
    }
    if let Some(result) = &entry_point.function.result {
        collect_io_binding(
            module,
            result.ty,
            result.binding.as_ref(),
            entry_point.stage,
            ShaderIoDirection::Output,
            reflection,
        )?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ShaderIoDirection {
    Input,
    Output,
}

fn collect_io_binding(
    module: &::naga::Module,
    ty: ::naga::Handle<::naga::Type>,
    binding: Option<&::naga::Binding>,
    stage: ::naga::ShaderStage,
    direction: ShaderIoDirection,
    reflection: &mut ShaderReflection,
) -> Result<(), NagaShaderBridgeFailure> {
    if let Some(binding) = binding {
        collect_bound_io(module, ty, binding, stage, direction, reflection)?;
        return Ok(());
    }

    if let ::naga::TypeInner::Struct { members, .. } = &module.types[ty].inner {
        for member in members {
            if let Some(binding) = member.binding.as_ref() {
                collect_bound_io(module, member.ty, binding, stage, direction, reflection)?;
            }
        }
    }
    Ok(())
}

fn collect_bound_io(
    module: &::naga::Module,
    ty: ::naga::Handle<::naga::Type>,
    binding: &::naga::Binding,
    stage: ::naga::ShaderStage,
    direction: ShaderIoDirection,
    reflection: &mut ShaderReflection,
) -> Result<(), NagaShaderBridgeFailure> {
    match binding {
        ::naga::Binding::Location { location, .. }
            if matches!(stage, ::naga::ShaderStage::Vertex)
                && matches!(direction, ShaderIoDirection::Input) =>
        {
            reflection
                .push_vertex_attribute(ShaderVertexInputDesc {
                    location: bounded_u8(*location),
                    format: shader_io_format(module, ty),
                })
                .map_err(|overflow| NagaShaderBridgeFailure::ReflectionFailed { overflow })?;
        }
        ::naga::Binding::Location { location, .. }
            if matches!(stage, ::naga::ShaderStage::Fragment)
                && matches!(direction, ShaderIoDirection::Output) =>
        {
            reflection
                .push_render_target_output(ShaderRenderTargetOutputDesc {
                    location: bounded_u8(*location),
                    shader_format: shader_io_format(module, ty),
                    target_format: crate::ir::TextureFormat::Undefined,
                })
                .map_err(|overflow| NagaShaderBridgeFailure::ReflectionFailed { overflow })?;
        }
        ::naga::Binding::BuiltIn(::naga::BuiltIn::FragDepth)
            if matches!(direction, ShaderIoDirection::Output) =>
        {
            reflection.depth_usage = true;
        }
        _ => {}
    }
    Ok(())
}

#[must_use]
fn binding_type_for_global(
    module: &::naga::Module,
    variable: &::naga::GlobalVariable,
) -> BindingType {
    match variable.space {
        ::naga::AddressSpace::Uniform => BindingType::UniformBuffer,
        ::naga::AddressSpace::Storage { .. } => BindingType::StorageBuffer,
        ::naga::AddressSpace::Immediate => BindingType::RootConstant,
        ::naga::AddressSpace::Handle => binding_type_for_handle_type(module, variable.ty),
        _ => BindingType::RootConstant,
    }
}

#[must_use]
fn binding_type_for_handle_type(
    module: &::naga::Module,
    ty: ::naga::Handle<::naga::Type>,
) -> BindingType {
    match &module.types[ty].inner {
        ::naga::TypeInner::Image { class, .. } => match class {
            ::naga::ImageClass::Storage { .. } => BindingType::StorageTexture,
            ::naga::ImageClass::Sampled { .. }
            | ::naga::ImageClass::Depth { .. }
            | ::naga::ImageClass::External => BindingType::SampledTexture,
        },
        ::naga::TypeInner::Sampler { .. } => BindingType::Sampler,
        ::naga::TypeInner::BindingArray { base, .. } => binding_type_for_handle_type(module, *base),
        _ => BindingType::RootConstant,
    }
}

#[must_use]
fn is_storage_or_uav_global(module: &::naga::Module, variable: &::naga::GlobalVariable) -> bool {
    match variable.space {
        ::naga::AddressSpace::Storage { access } => {
            access.intersects(::naga::StorageAccess::STORE | ::naga::StorageAccess::ATOMIC)
        }
        ::naga::AddressSpace::Handle => is_storage_image_type(module, variable.ty),
        _ => false,
    }
}

#[must_use]
fn is_storage_image_type(module: &::naga::Module, ty: ::naga::Handle<::naga::Type>) -> bool {
    match &module.types[ty].inner {
        ::naga::TypeInner::Image {
            class: ::naga::ImageClass::Storage { access, .. },
            ..
        } => access.intersects(::naga::StorageAccess::STORE | ::naga::StorageAccess::ATOMIC),
        ::naga::TypeInner::BindingArray { base, .. } => is_storage_image_type(module, *base),
        _ => false,
    }
}

#[must_use]
const fn stage_mask(stage: ::naga::ShaderStage) -> ShaderStageMask {
    match stage {
        ::naga::ShaderStage::Vertex => ShaderStageMask::VERTEX,
        ::naga::ShaderStage::Fragment => ShaderStageMask::FRAGMENT,
        ::naga::ShaderStage::Compute => ShaderStageMask::COMPUTE,
        ::naga::ShaderStage::Mesh | ::naga::ShaderStage::Task => ShaderStageMask::MESH,
        _ => ShaderStageMask::NONE,
    }
}

#[must_use]
fn shader_io_format(
    module: &::naga::Module,
    ty: ::naga::Handle<::naga::Type>,
) -> ShaderIoValueFormat {
    match &module.types[ty].inner {
        ::naga::TypeInner::Scalar(scalar) => scalar_format(scalar.kind, 1),
        ::naga::TypeInner::Vector { size, scalar } => scalar_format(scalar.kind, (*size).into()),
        _ => ShaderIoValueFormat::Unknown,
    }
}

#[must_use]
const fn scalar_format(kind: ::naga::ScalarKind, components: u8) -> ShaderIoValueFormat {
    match (kind, components) {
        (::naga::ScalarKind::Float, 1) => ShaderIoValueFormat::F32,
        (::naga::ScalarKind::Float, 2) => ShaderIoValueFormat::F32x2,
        (::naga::ScalarKind::Float, 3) => ShaderIoValueFormat::F32x3,
        (::naga::ScalarKind::Float, 4) => ShaderIoValueFormat::F32x4,
        (::naga::ScalarKind::Uint, 1) => ShaderIoValueFormat::U32,
        (::naga::ScalarKind::Uint, 2) => ShaderIoValueFormat::U32x2,
        (::naga::ScalarKind::Uint, 3) => ShaderIoValueFormat::U32x3,
        (::naga::ScalarKind::Uint, 4) => ShaderIoValueFormat::U32x4,
        (::naga::ScalarKind::Sint, 1) => ShaderIoValueFormat::I32,
        (::naga::ScalarKind::Sint, 2) => ShaderIoValueFormat::I32x2,
        (::naga::ScalarKind::Sint, 3) => ShaderIoValueFormat::I32x3,
        (::naga::ScalarKind::Sint, 4) => ShaderIoValueFormat::I32x4,
        (::naga::ScalarKind::Bool, 1) => ShaderIoValueFormat::Bool,
        _ => ShaderIoValueFormat::Unknown,
    }
}

#[must_use]
const fn bounded_u8(value: u32) -> u8 {
    if value > u8::MAX as u32 {
        u8::MAX
    } else {
        value as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ir::{
            BindLayoutDesc, BindSlotDesc, BindingType, IrBindLayoutId, IrPipelineLayoutId,
            ShaderModuleDesc, ShaderStageMask,
        },
        shader::{RendererShaderStage, validate_shader_reflection_layout},
    };

    const WGSL_GRAPHICS: &str = r#"
struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(1) @binding(0)
var scene_tex: texture_2d<f32>;

@group(1) @binding(1)
var scene_sampler: sampler;

struct VertexIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.position = camera.view_proj * vec4<f32>(input.pos, 1.0);
    out.uv = input.uv;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(scene_tex, scene_sampler, input.uv);
}
"#;

    const WGSL_COMPUTE: &str = r#"
struct Items {
    values: array<u32>,
};

@group(0) @binding(0)
var<storage, read_write> items: Items;

@compute @workgroup_size(8, 4, 1)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    items.values[id.x] = id.x;
}
"#;

    #[test]
    fn naga_bridge_parses_validates_and_reflects_wgsl_bindings_and_entry_points() {
        let desc = ShaderModuleDesc::wgsl(
            crate::ir::IrShaderModuleId::new(1),
            "scene.lit",
            "fs_main",
            ShaderStageMask::FRAGMENT,
            0xabc,
        );
        let shader = RendererShaderSchema::new(
            RendererShaderStage::Fragment,
            "fs_main",
            IrPipelineLayoutId::new(2),
            RendererShaderSourceStrategy::WgslThroughNaga,
            "scene.lit.fs",
        );
        let mut bridge = NagaShaderBridge::default();

        let translated = bridge
            .translate_shader_source(
                shader,
                desc,
                WGSL_GRAPHICS,
                ShaderTranslationTarget::WgpuDx12,
                ShaderTranslationPhase::Warmup,
            )
            .expect("WGSL should parse, validate, and reflect through naga");

        assert_eq!(translated.path, WgpuShaderTranslationPath::WgslThroughNaga);
        assert_eq!(translated.reflection.entry_point_count, 2);
        assert_eq!(translated.reflection.resource_binding_count, 3);
        assert_eq!(translated.reflection.render_target_output_count, 1);
        assert!(
            translated
                .reflection
                .resource_bindings
                .iter()
                .take(translated.reflection.resource_binding_count as usize)
                .any(|binding| binding.binding_type == BindingType::SampledTexture)
        );
        assert_eq!(
            translated.diagnostic.status,
            ShaderDiagnosticStatus::ParsedValidatedReflected
        );
    }

    #[test]
    fn naga_bridge_reflects_compute_workgroup_and_storage_uav_usage() {
        let desc = ShaderModuleDesc::wgsl(
            crate::ir::IrShaderModuleId::new(2),
            "compute.cull",
            "cs_main",
            ShaderStageMask::COMPUTE,
            0xdef,
        );
        let shader = RendererShaderSchema::new(
            RendererShaderStage::Compute,
            "cs_main",
            IrPipelineLayoutId::new(2),
            RendererShaderSourceStrategy::WgslThroughNaga,
            "compute.cull.cs",
        );
        let mut bridge = NagaShaderBridge::default();

        let translated = bridge
            .translate_shader_source(
                shader,
                desc,
                WGSL_COMPUTE,
                ShaderTranslationTarget::WgpuVulkan,
                ShaderTranslationPhase::Warmup,
            )
            .expect("compute WGSL should parse, validate, and reflect");

        assert_eq!(
            translated.reflection.workgroup_size,
            ShaderWorkgroupShape::new(8, 4, 1)
        );
        assert!(translated.reflection.storage_or_uav_usage);
        assert_eq!(
            translated.reflection.resource_bindings[0].binding_type,
            BindingType::StorageBuffer
        );
    }

    #[test]
    fn naga_bridge_cache_prevents_measured_runtime_translation() {
        let desc = ShaderModuleDesc::wgsl(
            crate::ir::IrShaderModuleId::new(3),
            "scene.lit",
            "fs_main",
            ShaderStageMask::FRAGMENT,
            0xabc,
        );
        let shader = RendererShaderSchema::new(
            RendererShaderStage::Fragment,
            "fs_main",
            IrPipelineLayoutId::new(2),
            RendererShaderSourceStrategy::WgslThroughNaga,
            "scene.lit.fs",
        );
        let mut bridge = NagaShaderBridge::default();

        bridge
            .translate_shader_source(
                shader,
                desc,
                WGSL_GRAPHICS,
                ShaderTranslationTarget::WgpuDx12,
                ShaderTranslationPhase::Warmup,
            )
            .expect("warmup translation should populate cache");
        let cached = bridge
            .translate_shader_source(
                shader,
                desc,
                WGSL_GRAPHICS,
                ShaderTranslationTarget::WgpuDx12,
                ShaderTranslationPhase::RuntimeMeasured,
            )
            .expect("measured runtime can reuse warmed shader metadata");

        assert!(!cached.cache_decision.translated);
        assert!(cached.cache_decision.reused);
        assert_eq!(cached.diagnostic.status, ShaderDiagnosticStatus::CacheHit);

        let cold_desc = ShaderModuleDesc::wgsl(
            crate::ir::IrShaderModuleId::new(4),
            "scene.other",
            "fs_main",
            ShaderStageMask::FRAGMENT,
            0x999,
        );
        let err = bridge
            .translate_shader_source(
                shader,
                cold_desc,
                WGSL_GRAPHICS,
                ShaderTranslationTarget::WgpuDx12,
                ShaderTranslationPhase::RuntimeMeasured,
            )
            .expect_err("cold shader translation is blocked during measured frames");

        assert!(matches!(
            err,
            NagaShaderBridgeFailure::Cache(
                ShaderTranslationCacheFailure::RuntimeShaderTranslation { .. }
            )
        ));
    }

    #[test]
    fn reflection_layout_mismatch_is_caught_before_pipeline_creation() {
        let desc = ShaderModuleDesc::wgsl(
            crate::ir::IrShaderModuleId::new(5),
            "scene.lit",
            "fs_main",
            ShaderStageMask::FRAGMENT,
            0xabc,
        );
        let shader = RendererShaderSchema::new(
            RendererShaderStage::Fragment,
            "fs_main",
            IrPipelineLayoutId::new(2),
            RendererShaderSourceStrategy::WgslThroughNaga,
            "scene.lit.fs",
        );
        let mut bridge = NagaShaderBridge::default();
        let translated = bridge
            .translate_shader_source(
                shader,
                desc,
                WGSL_GRAPHICS,
                ShaderTranslationTarget::WgpuDx12,
                ShaderTranslationPhase::Warmup,
            )
            .expect("WGSL reflection should succeed");
        let layout = crate::ir::PipelineLayoutDesc {
            id: IrPipelineLayoutId::new(2),
            bind_layouts: [
                IrBindLayoutId::new(1),
                IrBindLayoutId::new(2),
                IrBindLayoutId::INVALID,
                IrBindLayoutId::INVALID,
            ],
            bind_layout_count: 2,
            ..Default::default()
        };
        let mut group_zero_slots = [BindSlotDesc::default(); crate::ir::MAX_BINDINGS_PER_LAYOUT];
        group_zero_slots[0] = BindSlotDesc {
            binding: 0,
            binding_type: BindingType::StorageBuffer,
            stages: ShaderStageMask::VERTEX,
        };
        let group_zero = BindLayoutDesc {
            id: IrBindLayoutId::new(1),
            slots: group_zero_slots,
            slot_count: 1,
            ..Default::default()
        };

        let report =
            validate_shader_reflection_layout(translated.reflection, layout, &[group_zero]);

        assert!(!report.is_valid());
    }

    #[test]
    fn non_wgsl_source_strategies_are_scaffolded_below_renderer_schema() {
        let hlsl_desc = ShaderModuleDesc {
            id: crate::ir::IrShaderModuleId::new(6),
            schema_version: crate::ir::SHADER_IR_SCHEMA_VERSION,
            stable_name: "native.dlss",
            language: ShaderSourceLanguage::Hlsl,
            entry_point: "main",
            stages: ShaderStageMask::COMPUTE,
            source_digest: 7,
            requires_naga_translation: false,
        };

        assert_eq!(
            renderer_source_strategy(hlsl_desc, ShaderTranslationTarget::DirectDx12Dxc),
            RendererShaderSourceStrategy::HlslDxcDx12
        );
        assert_eq!(
            shader_translation_path(hlsl_desc),
            WgpuShaderTranslationPath::BackendSpecificSource
        );
    }
}
