use crate::{
    backend::{BackendCapabilityReport, NagaShaderTranslationStatus},
    ir::{ShaderModuleDesc, ShaderSourceLanguage},
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
