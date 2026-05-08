use crate::{
    backend::{
        BackendCapabilityReport, NagaShaderTranslationStatus, NativeBackend,
        NativeCommandEncoderAvailability, WgpuCoreValidationStatus, WgpuHalNativeHandleSupport,
    },
    ir::{
        BridgeTranslationDecision, BridgeTranslationReason, DescriptorFingerprint, IrDescriptorKey,
        IrDescriptorKind, RendererIrBridgeTranslator,
    },
};

use super::{
    WgpuNativeBackend,
    device::{WgpuAdapterInfo, WgpuFeatureSummary, WgpuLimitSummary},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuCachedDescriptorKind {
    Buffer,
    Texture,
    Sampler,
    BindGroupLayout,
    PipelineLayout,
    RenderPipeline,
    ComputePipeline,
    StaticBindGroup,
    GraphPass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCacheKey {
    pub kind: WgpuCachedDescriptorKind,
    pub stable_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuCreationPhase {
    Warmup,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuCacheDecision {
    pub key: WgpuCacheKey,
    pub created: bool,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuBridgeFailure {
    RuntimeCreationAfterWarmup {
        kind: WgpuCachedDescriptorKind,
        stable_id: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct WgpuCacheEntry {
    key: WgpuCacheKey,
    fingerprint: DescriptorFingerprint,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WgpuDescriptorCache {
    entries: Vec<WgpuCacheEntry>,
    pub created_count: u32,
    pub changed_count: u32,
    pub reused_count: u32,
    pub skipped_unchanged: u32,
    pub runtime_creation_failures: u32,
}

impl WgpuDescriptorCache {
    pub fn ensure_cached(
        &mut self,
        key: WgpuCacheKey,
        fingerprint: DescriptorFingerprint,
        phase: WgpuCreationPhase,
        measured_build: bool,
    ) -> Result<WgpuCacheDecision, WgpuBridgeFailure> {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            if entry.fingerprint == fingerprint {
                self.reused_count = self.reused_count.saturating_add(1);
                return Ok(WgpuCacheDecision {
                    key,
                    created: false,
                    changed: false,
                });
            }
            if measured_build && matches!(phase, WgpuCreationPhase::Runtime) {
                self.runtime_creation_failures = self.runtime_creation_failures.saturating_add(1);
                return Err(WgpuBridgeFailure::RuntimeCreationAfterWarmup {
                    kind: key.kind,
                    stable_id: key.stable_id,
                });
            }
            entry.fingerprint = fingerprint;
            self.changed_count = self.changed_count.saturating_add(1);
            return Ok(WgpuCacheDecision {
                key,
                created: false,
                changed: true,
            });
        }

        if measured_build && matches!(phase, WgpuCreationPhase::Runtime) {
            self.runtime_creation_failures = self.runtime_creation_failures.saturating_add(1);
            return Err(WgpuBridgeFailure::RuntimeCreationAfterWarmup {
                kind: key.kind,
                stable_id: key.stable_id,
            });
        }

        self.entries.push(WgpuCacheEntry { key, fingerprint });
        self.entries
            .sort_by_key(|entry| (entry.key.kind as u8, entry.key.stable_id));
        self.created_count = self.created_count.saturating_add(1);
        Ok(WgpuCacheDecision {
            key,
            created: true,
            changed: false,
        })
    }

    #[must_use]
    pub fn status(&self) -> WgpuPipelineCacheStatus {
        let count = |kind| {
            self.entries
                .iter()
                .filter(|entry| entry.key.kind == kind)
                .count() as u32
        };
        WgpuPipelineCacheStatus {
            cached_bind_group_layouts: count(WgpuCachedDescriptorKind::BindGroupLayout),
            cached_pipeline_layouts: count(WgpuCachedDescriptorKind::PipelineLayout),
            cached_render_pipelines: count(WgpuCachedDescriptorKind::RenderPipeline),
            cached_compute_pipelines: count(WgpuCachedDescriptorKind::ComputePipeline),
            cached_samplers: count(WgpuCachedDescriptorKind::Sampler),
            cached_static_bind_groups: count(WgpuCachedDescriptorKind::StaticBindGroup),
            runtime_creation_failures: self.runtime_creation_failures,
        }
    }

    fn translate(
        &mut self,
        mut key: IrDescriptorKey,
        kind: IrDescriptorKind,
        cache_kind: WgpuCachedDescriptorKind,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        key.kind = kind;
        match self.ensure_cached(
            WgpuCacheKey {
                kind: cache_kind,
                stable_id: key.index,
            },
            fingerprint,
            WgpuCreationPhase::Warmup,
            false,
        ) {
            Ok(decision) if decision.created => BridgeTranslationDecision {
                key,
                translated: true,
                reason: BridgeTranslationReason::FirstTranslation,
            },
            Ok(decision) if decision.changed => BridgeTranslationDecision {
                key,
                translated: true,
                reason: BridgeTranslationReason::DescriptorChanged,
            },
            Ok(_) => {
                self.skipped_unchanged = self.skipped_unchanged.saturating_add(1);
                BridgeTranslationDecision {
                    key,
                    translated: false,
                    reason: BridgeTranslationReason::DescriptorUnchanged,
                }
            }
            Err(_) => BridgeTranslationDecision {
                key,
                translated: true,
                reason: BridgeTranslationReason::DescriptorChanged,
            },
        }
    }
}

impl RendererIrBridgeTranslator for WgpuDescriptorCache {
    fn translate_resource_creation(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(
            key,
            IrDescriptorKind::ResourceCreation,
            WgpuCachedDescriptorKind::Buffer,
            fingerprint,
        )
    }

    fn translate_pipeline_creation(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(
            key,
            IrDescriptorKind::PipelineCreation,
            WgpuCachedDescriptorKind::RenderPipeline,
            fingerprint,
        )
    }

    fn translate_pass_execution(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(
            key,
            IrDescriptorKind::PassExecution,
            WgpuCachedDescriptorKind::GraphPass,
            fingerprint,
        )
    }

    fn translate_graph_compile(
        &mut self,
        key: IrDescriptorKey,
        fingerprint: DescriptorFingerprint,
    ) -> BridgeTranslationDecision {
        self.translate(
            key,
            IrDescriptorKind::GraphCompile,
            WgpuCachedDescriptorKind::GraphPass,
            fingerprint,
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuPipelineCacheStatus {
    pub cached_bind_group_layouts: u32,
    pub cached_pipeline_layouts: u32,
    pub cached_render_pipelines: u32,
    pub cached_compute_pipelines: u32,
    pub cached_samplers: u32,
    pub cached_static_bind_groups: u32,
    pub runtime_creation_failures: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuBridgeHealthReport<'a> {
    pub schema_version: u16,
    pub selected_wgpu_backend: ::wgpu::Backends,
    pub actual_native_backend: NativeBackend,
    pub adapter_info: WgpuAdapterInfo<'a>,
    pub validation_status: WgpuCoreValidationStatus,
    pub limits: WgpuLimitSummary,
    pub features: WgpuFeatureSummary,
    pub hal_access: WgpuHalNativeHandleSupport,
    pub command_encoder_availability: NativeCommandEncoderAvailability,
    pub shader_translation_path: NagaShaderTranslationStatus,
    pub pipeline_cache_status: WgpuPipelineCacheStatus,
}

impl<'a> WgpuBridgeHealthReport<'a> {
    #[must_use]
    pub fn from_parts<B: WgpuNativeBackend>(
        adapter_info: WgpuAdapterInfo<'a>,
        limits: WgpuLimitSummary,
        features: WgpuFeatureSummary,
        pipeline_cache_status: WgpuPipelineCacheStatus,
    ) -> Self {
        Self::from_report(
            B::WGPU_BACKENDS,
            B::CAPABILITY_REPORT,
            adapter_info,
            limits,
            features,
            pipeline_cache_status,
        )
    }

    #[must_use]
    pub const fn from_report(
        selected_wgpu_backend: ::wgpu::Backends,
        report: BackendCapabilityReport,
        adapter_info: WgpuAdapterInfo<'a>,
        limits: WgpuLimitSummary,
        features: WgpuFeatureSummary,
        pipeline_cache_status: WgpuPipelineCacheStatus,
    ) -> Self {
        Self {
            schema_version: super::WGPU_BRIDGE_SCHEMA_VERSION,
            selected_wgpu_backend,
            actual_native_backend: report.actual_native_backend,
            adapter_info,
            validation_status: report.wgpu_core_validation,
            limits,
            features,
            hal_access: report.wgpu_hal_native_handle_support,
            command_encoder_availability: report.command_encoder_availability,
            shader_translation_path: report.naga_shader_translation,
            pipeline_cache_status,
        }
    }
}
