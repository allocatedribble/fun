use crate::{
    backend::NativeBackend,
    ir::{
        BindLayoutDesc, BindingType, GraphPassDesc, IrPipelineLayoutId, MaterialBindingDesc,
        PipelineLayoutDesc, ResourceAccess, RootConstantDesc, ShaderStageMask, TextureFormat,
    },
};

pub const RENDERER_SHADER_SCHEMA_VERSION: u16 = 1;
pub const SHADER_REFLECTION_SCHEMA_VERSION: u16 = 1;
pub const SHADER_DIAGNOSTIC_SCHEMA_VERSION: u16 = 1;
pub const SHADER_TRANSLATION_CACHE_SCHEMA_VERSION: u16 = 1;
pub const MAX_SHADER_FEATURE_DEFINES: usize = 16;
pub const MAX_SHADER_VERTEX_INPUTS: usize = 16;
pub const MAX_SHADER_RENDER_TARGET_OUTPUTS: usize = 8;
pub const MAX_SHADER_REFLECTED_BINDINGS: usize = 32;
pub const MAX_SHADER_REFLECTED_ENTRY_POINTS: usize = 8;
pub const MAX_SHADER_ROOT_CONSTANTS: usize = 8;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererShaderStage {
    #[default]
    Vertex,
    Fragment,
    Compute,
    Mesh,
}

impl RendererShaderStage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vertex => "vertex",
            Self::Fragment => "fragment",
            Self::Compute => "compute",
            Self::Mesh => "mesh",
        }
    }

    #[must_use]
    pub const fn mask(self) -> ShaderStageMask {
        match self {
            Self::Vertex => ShaderStageMask::VERTEX,
            Self::Fragment => ShaderStageMask::FRAGMENT,
            Self::Compute => ShaderStageMask::COMPUTE,
            Self::Mesh => ShaderStageMask::MESH,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RendererShaderSourceStrategy {
    #[default]
    WgslThroughNaga,
    NagaIrPassthrough,
    HlslDxcDx12,
    SpirvVulkan,
    MslMetal,
}

impl RendererShaderSourceStrategy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WgslThroughNaga => "wgsl_through_naga",
            Self::NagaIrPassthrough => "naga_ir_passthrough",
            Self::HlslDxcDx12 => "hlsl_dxc_dx12",
            Self::SpirvVulkan => "spirv_vulkan",
            Self::MslMetal => "msl_metal",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderTranslationTarget {
    #[default]
    WgpuDx12,
    WgpuVulkan,
    WgpuMetal,
    DirectDx12Dxc,
    DirectVulkanSpirv,
    DirectMetalMsl,
}

impl ShaderTranslationTarget {
    #[must_use]
    pub const fn backend(self) -> NativeBackend {
        match self {
            Self::WgpuDx12 | Self::DirectDx12Dxc => NativeBackend::Dx12,
            Self::WgpuVulkan | Self::DirectVulkanSpirv => NativeBackend::Vulkan,
            Self::WgpuMetal | Self::DirectMetalMsl => NativeBackend::Metal,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WgpuDx12 => "wgpu_dx12",
            Self::WgpuVulkan => "wgpu_vulkan",
            Self::WgpuMetal => "wgpu_metal",
            Self::DirectDx12Dxc => "direct_dx12_dxc",
            Self::DirectVulkanSpirv => "direct_vulkan_spirv",
            Self::DirectMetalMsl => "direct_metal_msl",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderIoValueFormat {
    #[default]
    Unknown,
    F32,
    F32x2,
    F32x3,
    F32x4,
    U32,
    U32x2,
    U32x3,
    U32x4,
    I32,
    I32x2,
    I32x3,
    I32x4,
    Bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderFeatureDefine {
    pub name_hash: u64,
    pub value: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderWorkgroupShape {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl ShaderWorkgroupShape {
    pub const ONE: Self = Self { x: 1, y: 1, z: 1 };

    #[must_use]
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderVertexInputDesc {
    pub location: u8,
    pub format: ShaderIoValueFormat,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderRenderTargetOutputDesc {
    pub location: u8,
    pub shader_format: ShaderIoValueFormat,
    pub target_format: TextureFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RendererShaderSchema {
    pub schema_version: u16,
    pub stage: RendererShaderStage,
    pub entry_point: &'static str,
    pub pipeline_layout: IrPipelineLayoutId,
    pub vertex_inputs: [ShaderVertexInputDesc; MAX_SHADER_VERTEX_INPUTS],
    pub vertex_input_count: u8,
    pub fragment_outputs: [ShaderRenderTargetOutputDesc; MAX_SHADER_RENDER_TARGET_OUTPUTS],
    pub fragment_output_count: u8,
    pub compute_workgroup_shape: ShaderWorkgroupShape,
    pub feature_defines: [ShaderFeatureDefine; MAX_SHADER_FEATURE_DEFINES],
    pub feature_define_count: u8,
    pub material_schema_version: u16,
    pub source_strategy: RendererShaderSourceStrategy,
    pub debug_name: &'static str,
}

impl RendererShaderSchema {
    #[must_use]
    pub const fn new(
        stage: RendererShaderStage,
        entry_point: &'static str,
        pipeline_layout: IrPipelineLayoutId,
        source_strategy: RendererShaderSourceStrategy,
        debug_name: &'static str,
    ) -> Self {
        Self {
            schema_version: RENDERER_SHADER_SCHEMA_VERSION,
            stage,
            entry_point,
            pipeline_layout,
            vertex_inputs: [ShaderVertexInputDesc {
                location: 0,
                format: ShaderIoValueFormat::Unknown,
            }; MAX_SHADER_VERTEX_INPUTS],
            vertex_input_count: 0,
            fragment_outputs: [ShaderRenderTargetOutputDesc {
                location: 0,
                shader_format: ShaderIoValueFormat::Unknown,
                target_format: TextureFormat::Undefined,
            }; MAX_SHADER_RENDER_TARGET_OUTPUTS],
            fragment_output_count: 0,
            compute_workgroup_shape: ShaderWorkgroupShape::ONE,
            feature_defines: [ShaderFeatureDefine {
                name_hash: 0,
                value: 0,
            }; MAX_SHADER_FEATURE_DEFINES],
            feature_define_count: 0,
            material_schema_version: 0,
            source_strategy,
            debug_name,
        }
    }

    #[must_use]
    pub fn define_fingerprint(self) -> u64 {
        let mut hash = FNV_OFFSET_BASIS;
        for define in self
            .feature_defines
            .iter()
            .take(self.feature_define_count as usize)
        {
            hash = fnv1a_u64(hash, define.name_hash);
            hash = fnv1a_u64(hash, u64::from(define.value));
        }
        hash
    }

    #[must_use]
    pub fn schema_fingerprint(self) -> u64 {
        let mut hash = stable_label_hash(self.debug_name);
        hash = fnv1a_u64(hash, u64::from(self.schema_version));
        hash = fnv1a_u64(hash, self.stage as u64);
        hash = fnv1a_u64(hash, stable_label_hash(self.entry_point));
        hash = fnv1a_u64(hash, u64::from(self.material_schema_version));
        hash = fnv1a_u64(hash, self.source_strategy as u64);
        hash
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderResourceBindingSlot {
    pub group: u8,
    pub binding: u8,
    pub binding_type: BindingType,
    pub stages: ShaderStageMask,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderReflectedEntryPoint {
    pub name_hash: u64,
    pub stages: ShaderStageMask,
    pub workgroup_size: ShaderWorkgroupShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderReflection {
    pub schema_version: u16,
    pub source_hash: u64,
    pub entry_points: [ShaderReflectedEntryPoint; MAX_SHADER_REFLECTED_ENTRY_POINTS],
    pub entry_point_count: u8,
    pub resource_bindings: [ShaderResourceBindingSlot; MAX_SHADER_REFLECTED_BINDINGS],
    pub resource_binding_count: u8,
    pub root_constants: [RootConstantDesc; MAX_SHADER_ROOT_CONSTANTS],
    pub root_constant_count: u8,
    pub vertex_attributes: [ShaderVertexInputDesc; MAX_SHADER_VERTEX_INPUTS],
    pub vertex_attribute_count: u8,
    pub render_target_outputs: [ShaderRenderTargetOutputDesc; MAX_SHADER_RENDER_TARGET_OUTPUTS],
    pub render_target_output_count: u8,
    pub depth_usage: bool,
    pub storage_or_uav_usage: bool,
    pub workgroup_size: ShaderWorkgroupShape,
}

impl ShaderReflection {
    #[must_use]
    pub const fn new(source_hash: u64) -> Self {
        Self {
            schema_version: SHADER_REFLECTION_SCHEMA_VERSION,
            source_hash,
            entry_points: [ShaderReflectedEntryPoint {
                name_hash: 0,
                stages: ShaderStageMask::NONE,
                workgroup_size: ShaderWorkgroupShape::ONE,
            }; MAX_SHADER_REFLECTED_ENTRY_POINTS],
            entry_point_count: 0,
            resource_bindings: [ShaderResourceBindingSlot {
                group: 0,
                binding: 0,
                binding_type: BindingType::RootConstant,
                stages: ShaderStageMask::NONE,
            }; MAX_SHADER_REFLECTED_BINDINGS],
            resource_binding_count: 0,
            root_constants: [RootConstantDesc {
                slot: 0,
                byte_offset: 0,
                byte_count: 0,
                stages: ShaderStageMask::NONE,
            }; MAX_SHADER_ROOT_CONSTANTS],
            root_constant_count: 0,
            vertex_attributes: [ShaderVertexInputDesc {
                location: 0,
                format: ShaderIoValueFormat::Unknown,
            }; MAX_SHADER_VERTEX_INPUTS],
            vertex_attribute_count: 0,
            render_target_outputs: [ShaderRenderTargetOutputDesc {
                location: 0,
                shader_format: ShaderIoValueFormat::Unknown,
                target_format: TextureFormat::Undefined,
            }; MAX_SHADER_RENDER_TARGET_OUTPUTS],
            render_target_output_count: 0,
            depth_usage: false,
            storage_or_uav_usage: false,
            workgroup_size: ShaderWorkgroupShape::ONE,
        }
    }

    pub fn push_entry_point(
        &mut self,
        entry_point: ShaderReflectedEntryPoint,
    ) -> Result<(), ShaderReflectionOverflow> {
        push_copy(
            &mut self.entry_points,
            &mut self.entry_point_count,
            entry_point,
            ShaderReflectionOverflow::EntryPoints,
        )
    }

    pub fn push_resource_binding(
        &mut self,
        binding: ShaderResourceBindingSlot,
    ) -> Result<(), ShaderReflectionOverflow> {
        push_copy(
            &mut self.resource_bindings,
            &mut self.resource_binding_count,
            binding,
            ShaderReflectionOverflow::ResourceBindings,
        )
    }

    pub fn push_root_constant(
        &mut self,
        root_constant: RootConstantDesc,
    ) -> Result<(), ShaderReflectionOverflow> {
        push_copy(
            &mut self.root_constants,
            &mut self.root_constant_count,
            root_constant,
            ShaderReflectionOverflow::RootConstants,
        )
    }

    pub fn push_vertex_attribute(
        &mut self,
        attribute: ShaderVertexInputDesc,
    ) -> Result<(), ShaderReflectionOverflow> {
        push_copy(
            &mut self.vertex_attributes,
            &mut self.vertex_attribute_count,
            attribute,
            ShaderReflectionOverflow::VertexAttributes,
        )
    }

    pub fn push_render_target_output(
        &mut self,
        output: ShaderRenderTargetOutputDesc,
    ) -> Result<(), ShaderReflectionOverflow> {
        push_copy(
            &mut self.render_target_outputs,
            &mut self.render_target_output_count,
            output,
            ShaderReflectionOverflow::RenderTargetOutputs,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderReflectionOverflow {
    EntryPoints,
    ResourceBindings,
    RootConstants,
    VertexAttributes,
    RenderTargetOutputs,
}

fn push_copy<T: Copy, const N: usize>(
    slots: &mut [T; N],
    count: &mut u8,
    value: T,
    error: ShaderReflectionOverflow,
) -> Result<(), ShaderReflectionOverflow> {
    let index = usize::from(*count);
    if index >= N {
        return Err(error);
    }
    slots[index] = value;
    *count = count.saturating_add(1);
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderDiagnosticStatus {
    ParsedValidatedReflected,
    CacheHit,
    UnsupportedSourceStrategy,
    ParseFailed,
    ValidationFailed,
    ReflectionFailed,
    LayoutMismatch,
    RuntimeTranslationBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderDiagnosticArtifact {
    pub schema_version: u16,
    pub debug_name_hash: u64,
    pub source_hash: u64,
    pub defines_hash: u64,
    pub entry_point_hash: u64,
    pub target: ShaderTranslationTarget,
    pub source_strategy: RendererShaderSourceStrategy,
    pub status: ShaderDiagnosticStatus,
    pub parse_error_count: u16,
    pub validation_error_count: u16,
    pub reflection_error_count: u16,
    pub layout_error_count: u16,
    pub reflected_entry_points: u8,
    pub reflected_resource_bindings: u8,
}

impl ShaderDiagnosticArtifact {
    #[must_use]
    pub fn new(
        shader: RendererShaderSchema,
        source_hash: u64,
        target: ShaderTranslationTarget,
        status: ShaderDiagnosticStatus,
    ) -> Self {
        Self {
            schema_version: SHADER_DIAGNOSTIC_SCHEMA_VERSION,
            debug_name_hash: stable_label_hash(shader.debug_name),
            source_hash,
            defines_hash: shader.define_fingerprint(),
            entry_point_hash: stable_label_hash(shader.entry_point),
            target,
            source_strategy: shader.source_strategy,
            status,
            parse_error_count: 0,
            validation_error_count: 0,
            reflection_error_count: 0,
            layout_error_count: 0,
            reflected_entry_points: 0,
            reflected_resource_bindings: 0,
        }
    }

    #[must_use]
    pub const fn with_status(mut self, status: ShaderDiagnosticStatus) -> Self {
        self.status = status;
        self
    }

    #[must_use]
    pub const fn with_reflection(mut self, reflection: ShaderReflection) -> Self {
        self.reflected_entry_points = reflection.entry_point_count;
        self.reflected_resource_bindings = reflection.resource_binding_count;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderTranslationCacheKey {
    pub schema_version: u16,
    pub source_hash: u64,
    pub defines_hash: u64,
    pub backend: NativeBackend,
    pub target: ShaderTranslationTarget,
    pub shader_model_hash: u64,
    pub entry_point_hash: u64,
}

impl ShaderTranslationCacheKey {
    #[must_use]
    pub fn new(
        shader: RendererShaderSchema,
        source_hash: u64,
        target: ShaderTranslationTarget,
    ) -> Self {
        Self {
            schema_version: SHADER_TRANSLATION_CACHE_SCHEMA_VERSION,
            source_hash,
            defines_hash: shader.define_fingerprint(),
            backend: target.backend(),
            target,
            shader_model_hash: shader.schema_fingerprint(),
            entry_point_hash: stable_label_hash(shader.entry_point),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderTranslationPhase {
    Warmup,
    RuntimeMeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderTranslationCacheFailure {
    RuntimeShaderTranslation { key: ShaderTranslationCacheKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderTranslationCacheEntry {
    pub key: ShaderTranslationCacheKey,
    pub reflection: ShaderReflection,
    pub diagnostic: ShaderDiagnosticArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderTranslationCacheDecision {
    pub key: ShaderTranslationCacheKey,
    pub translated: bool,
    pub reused: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShaderTranslationCache {
    entries: Vec<ShaderTranslationCacheEntry>,
    pub translated_count: u32,
    pub reused_count: u32,
    pub runtime_translation_failures: u32,
}

impl ShaderTranslationCache {
    #[must_use]
    pub fn lookup(
        &mut self,
        key: ShaderTranslationCacheKey,
    ) -> Option<ShaderTranslationCacheEntry> {
        let entry = self
            .entries
            .iter()
            .copied()
            .find(|entry| entry.key == key)?;
        self.reused_count = self.reused_count.saturating_add(1);
        Some(ShaderTranslationCacheEntry {
            diagnostic: entry
                .diagnostic
                .with_status(ShaderDiagnosticStatus::CacheHit),
            ..entry
        })
    }

    pub fn ensure_can_translate(
        &mut self,
        key: ShaderTranslationCacheKey,
        phase: ShaderTranslationPhase,
    ) -> Result<(), ShaderTranslationCacheFailure> {
        if matches!(phase, ShaderTranslationPhase::RuntimeMeasured) {
            self.runtime_translation_failures = self.runtime_translation_failures.saturating_add(1);
            return Err(ShaderTranslationCacheFailure::RuntimeShaderTranslation { key });
        }
        Ok(())
    }

    pub fn insert(
        &mut self,
        key: ShaderTranslationCacheKey,
        reflection: ShaderReflection,
        diagnostic: ShaderDiagnosticArtifact,
    ) -> ShaderTranslationCacheDecision {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.reflection = reflection;
            entry.diagnostic = diagnostic;
        } else {
            self.entries.push(ShaderTranslationCacheEntry {
                key,
                reflection,
                diagnostic,
            });
            self.entries.sort_by_key(|entry| {
                (
                    entry.key.backend as u8,
                    entry.key.target as u8,
                    entry.key.source_hash,
                    entry.key.defines_hash,
                    entry.key.entry_point_hash,
                )
            });
        }
        self.translated_count = self.translated_count.saturating_add(1);
        ShaderTranslationCacheDecision {
            key,
            translated: true,
            reused: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderLayoutValidationCode {
    MissingBindGroup,
    MissingBindingSlot,
    BindingTypeMismatch,
    StageVisibilityMismatch,
    MaterialSchemaMismatch,
    GraphPassReadMissing,
    GraphPassWriteMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderLayoutValidationIssue {
    pub code: ShaderLayoutValidationCode,
    pub group: u8,
    pub binding: u8,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShaderLayoutValidationReport {
    pub issues: Vec<ShaderLayoutValidationIssue>,
}

impl ShaderLayoutValidationReport {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn push(&mut self, code: ShaderLayoutValidationCode, group: u8, binding: u8) {
        self.issues.push(ShaderLayoutValidationIssue {
            code,
            group,
            binding,
        });
    }
}

#[must_use]
pub fn validate_shader_reflection_layout(
    reflection: ShaderReflection,
    pipeline_layout: PipelineLayoutDesc,
    bind_layouts: &[BindLayoutDesc],
) -> ShaderLayoutValidationReport {
    let mut report = ShaderLayoutValidationReport::default();
    for binding in reflection
        .resource_bindings
        .iter()
        .copied()
        .take(reflection.resource_binding_count as usize)
    {
        let Some(layout_id) = pipeline_layout
            .bind_layouts
            .get(binding.group as usize)
            .copied()
        else {
            report.push(
                ShaderLayoutValidationCode::MissingBindGroup,
                binding.group,
                binding.binding,
            );
            continue;
        };
        if !layout_id.is_valid() || binding.group >= pipeline_layout.bind_layout_count {
            report.push(
                ShaderLayoutValidationCode::MissingBindGroup,
                binding.group,
                binding.binding,
            );
            continue;
        }
        let Some(bind_layout) = bind_layouts.iter().find(|layout| layout.id == layout_id) else {
            report.push(
                ShaderLayoutValidationCode::MissingBindGroup,
                binding.group,
                binding.binding,
            );
            continue;
        };
        let Some(slot) = bind_layout
            .slots
            .iter()
            .copied()
            .take(bind_layout.slot_count as usize)
            .find(|slot| slot.binding == binding.binding)
        else {
            report.push(
                ShaderLayoutValidationCode::MissingBindingSlot,
                binding.group,
                binding.binding,
            );
            continue;
        };
        if slot.binding_type != binding.binding_type {
            report.push(
                ShaderLayoutValidationCode::BindingTypeMismatch,
                binding.group,
                binding.binding,
            );
        }
        if !slot.stages.contains(binding.stages) {
            report.push(
                ShaderLayoutValidationCode::StageVisibilityMismatch,
                binding.group,
                binding.binding,
            );
        }
    }
    report
}

#[must_use]
pub fn validate_material_schema(
    shader: RendererShaderSchema,
    material: MaterialBindingDesc,
) -> ShaderLayoutValidationReport {
    let mut report = ShaderLayoutValidationReport::default();
    if shader.material_schema_version != material.material_schema_version {
        report.push(ShaderLayoutValidationCode::MaterialSchemaMismatch, 0, 0);
    }
    report
}

#[must_use]
pub fn validate_graph_pass_resource_usage(
    reflection: ShaderReflection,
    pass: GraphPassDesc,
) -> ShaderLayoutValidationReport {
    let mut report = ShaderLayoutValidationReport::default();
    let reads_any = pass
        .uses
        .iter()
        .take(pass.use_count as usize)
        .any(|resource| resource.access.reads());
    let writes_any = pass
        .uses
        .iter()
        .take(pass.use_count as usize)
        .any(|resource| {
            matches!(
                resource.access,
                ResourceAccess::Write | ResourceAccess::ReadWrite
            )
        });
    if reflection.resource_binding_count > 0 && !reads_any {
        report.push(ShaderLayoutValidationCode::GraphPassReadMissing, 0, 0);
    }
    if reflection.storage_or_uav_usage && !writes_any {
        report.push(ShaderLayoutValidationCode::GraphPassWriteMissing, 0, 0);
    }
    report
}

pub const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
pub const FNV_PRIME: u64 = 0x100000001b3;

#[must_use]
pub fn stable_label_hash(label: &str) -> u64 {
    label
        .as_bytes()
        .iter()
        .fold(FNV_OFFSET_BASIS, |hash, byte| fnv1a_byte(hash, *byte))
}

#[must_use]
pub const fn fnv1a_byte(hash: u64, byte: u8) -> u64 {
    (hash ^ byte as u64).wrapping_mul(FNV_PRIME)
}

#[must_use]
pub const fn fnv1a_u64(mut hash: u64, value: u64) -> u64 {
    let bytes = value.to_le_bytes();
    let mut index = 0;
    while index < bytes.len() {
        hash = fnv1a_byte(hash, bytes[index]);
        index += 1;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{
        BindSlotDesc, GraphPassKind, GraphResourceUse, IrBindLayoutId, IrGraphPassId,
        IrGraphResourceId, IrResourceRef,
    };

    #[test]
    fn renderer_shader_schema_is_source_strategy_independent() {
        let wgsl = RendererShaderSchema::new(
            RendererShaderStage::Fragment,
            "fs_main",
            IrPipelineLayoutId::new(3),
            RendererShaderSourceStrategy::WgslThroughNaga,
            "scene.lit.fs",
        );
        let hlsl = RendererShaderSchema {
            source_strategy: RendererShaderSourceStrategy::HlslDxcDx12,
            ..wgsl
        };

        assert_eq!(wgsl.stage, hlsl.stage);
        assert_eq!(wgsl.entry_point, hlsl.entry_point);
        assert_eq!(wgsl.pipeline_layout, hlsl.pipeline_layout);
        assert_ne!(wgsl.source_strategy, hlsl.source_strategy);
    }

    #[test]
    fn shader_translation_cache_blocks_measured_runtime_misses() {
        let shader = RendererShaderSchema::new(
            RendererShaderStage::Compute,
            "cs_main",
            IrPipelineLayoutId::new(1),
            RendererShaderSourceStrategy::WgslThroughNaga,
            "compute.cull",
        );
        let key = ShaderTranslationCacheKey::new(shader, 44, ShaderTranslationTarget::WgpuDx12);
        let mut cache = ShaderTranslationCache::default();

        let err = cache
            .ensure_can_translate(key, ShaderTranslationPhase::RuntimeMeasured)
            .expect_err("runtime shader translation must be rejected when the key is cold");

        assert_eq!(
            err,
            ShaderTranslationCacheFailure::RuntimeShaderTranslation { key }
        );
        assert_eq!(cache.runtime_translation_failures, 1);
    }

    #[test]
    fn reflection_layout_validation_catches_missing_or_wrong_slots() {
        let bind_layout_id = IrBindLayoutId::new(1);
        let pipeline_layout = PipelineLayoutDesc {
            id: IrPipelineLayoutId::new(2),
            bind_layouts: [
                bind_layout_id,
                IrBindLayoutId::INVALID,
                IrBindLayoutId::INVALID,
                IrBindLayoutId::INVALID,
            ],
            bind_layout_count: 1,
            ..PipelineLayoutDesc::default()
        };
        let mut slots = [BindSlotDesc::default(); crate::ir::MAX_BINDINGS_PER_LAYOUT];
        slots[0] = BindSlotDesc {
            binding: 0,
            binding_type: BindingType::UniformBuffer,
            stages: ShaderStageMask::VERTEX,
        };
        let bind_layout = BindLayoutDesc {
            id: bind_layout_id,
            slots,
            slot_count: 1,
            ..BindLayoutDesc::default()
        };
        let mut reflection = ShaderReflection::new(9);
        reflection
            .push_resource_binding(ShaderResourceBindingSlot {
                group: 0,
                binding: 0,
                binding_type: BindingType::StorageBuffer,
                stages: ShaderStageMask::FRAGMENT,
            })
            .expect("test reflection stays within limits");

        let report = validate_shader_reflection_layout(reflection, pipeline_layout, &[bind_layout]);

        assert!(!report.is_valid());
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == ShaderLayoutValidationCode::BindingTypeMismatch)
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == ShaderLayoutValidationCode::StageVisibilityMismatch)
        );
    }

    #[test]
    fn graph_usage_validation_requires_writes_for_storage_uav_reflection() {
        let mut reflection = ShaderReflection::new(9);
        reflection.storage_or_uav_usage = true;
        reflection
            .push_resource_binding(ShaderResourceBindingSlot {
                group: 0,
                binding: 0,
                binding_type: BindingType::StorageBuffer,
                stages: ShaderStageMask::COMPUTE,
            })
            .expect("test reflection stays within limits");
        let pass = GraphPassDesc {
            id: IrGraphPassId::new(1),
            kind: GraphPassKind::Compute,
            uses: [GraphResourceUse {
                resource: IrGraphResourceId::new(1),
                resource_ref: IrResourceRef::buffer(crate::ir::IrBufferId::new(1)),
                access: ResourceAccess::Read,
            }; crate::ir::MAX_GRAPH_RESOURCE_USES_PER_PASS],
            use_count: 1,
            ..GraphPassDesc::default()
        };

        let report = validate_graph_pass_resource_usage(reflection, pass);

        assert_eq!(
            report.issues[0].code,
            ShaderLayoutValidationCode::GraphPassWriteMissing
        );
    }
}
