use ::core::marker::PhantomData;

use crate::backend::{BackendCapabilityReport, NativeBackend};

pub mod binding;
pub mod command;
pub mod core;
pub mod device;
pub mod diagnostics;
pub mod dx12_diagnostics;
pub mod hal;
pub mod naga;
pub mod pipeline;
pub mod resource;
pub mod runtime;
pub mod surface;

pub use binding::*;
pub use command::*;
pub use core::*;
pub use device::*;
pub use diagnostics::*;
pub use dx12_diagnostics::*;
pub use hal::*;
pub use naga::*;
pub use pipeline::*;
pub use resource::*;
pub use runtime::*;
pub use surface::*;

pub const WGPU_BRIDGE_SCHEMA_VERSION: u16 = 1;

pub trait WgpuNativeBackend: Sized + 'static {
    const NAME: &'static str;
    const NATIVE_BACKEND: NativeBackend;
    const WGPU_BACKENDS: ::wgpu::Backends;
    const CAPABILITY_REPORT: BackendCapabilityReport;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dx12Native;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VulkanNative;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MetalNative;

impl WgpuNativeBackend for Dx12Native {
    const NAME: &'static str = "dx12";
    const NATIVE_BACKEND: NativeBackend = NativeBackend::Dx12;
    const WGPU_BACKENDS: ::wgpu::Backends = ::wgpu::Backends::DX12;
    const CAPABILITY_REPORT: BackendCapabilityReport = BackendCapabilityReport::WGPU_DX12;
}

impl WgpuNativeBackend for VulkanNative {
    const NAME: &'static str = "vulkan";
    const NATIVE_BACKEND: NativeBackend = NativeBackend::Vulkan;
    const WGPU_BACKENDS: ::wgpu::Backends = ::wgpu::Backends::VULKAN;
    const CAPABILITY_REPORT: BackendCapabilityReport = BackendCapabilityReport::WGPU_VULKAN;
}

impl WgpuNativeBackend for MetalNative {
    const NAME: &'static str = "metal";
    const NATIVE_BACKEND: NativeBackend = NativeBackend::Metal;
    const WGPU_BACKENDS: ::wgpu::Backends = ::wgpu::Backends::METAL;
    const CAPABILITY_REPORT: BackendCapabilityReport = BackendCapabilityReport::WGPU_METAL;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuBridge<B: WgpuNativeBackend> {
    marker: PhantomData<fn() -> B>,
}

impl<B: WgpuNativeBackend> WgpuBridge<B> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marker: PhantomData,
        }
    }

    #[must_use]
    pub fn native_backend(self) -> NativeBackend {
        B::NATIVE_BACKEND
    }

    #[must_use]
    pub fn wgpu_backends(self) -> ::wgpu::Backends {
        B::WGPU_BACKENDS
    }

    #[must_use]
    pub fn capability_report(self) -> BackendCapabilityReport {
        B::CAPABILITY_REPORT
    }
}

impl<B: WgpuNativeBackend> Default for WgpuBridge<B> {
    fn default() -> Self {
        Self::new()
    }
}

pub type WgpuDx12NativeBridge = WgpuBridge<Dx12Native>;
pub type WgpuVulkanNativeBridge = WgpuBridge<VulkanNative>;
pub type WgpuMetalNativeBridge = WgpuBridge<MetalNative>;

pub trait WgpuRendererBackend: crate::backend::RendererBackend {
    type Native: WgpuNativeBackend;
}

impl<B: WgpuNativeBackend> WgpuRendererBackend for WgpuBridge<B>
where
    WgpuBridge<B>: crate::backend::RendererBackend,
{
    type Native = B;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backend::{BackendBridgeType, Renderer, WgpuDx12Bridge},
        ir::{
            BufferDesc, BufferMemoryClass, BufferUsageFlags, DescriptorFingerprint, DrawPacket,
            GraphPassKind, IrBufferId, IrDescriptorKey, IrDescriptorKind, IrGraphPassId,
            IrRenderPipelineId, RenderPassCommand, RendererIrBridgeTranslator, TextureDesc,
            TextureFormat, TextureUsageFlags,
        },
    };

    #[test]
    fn static_wgpu_bridge_types_are_renderer_backend_types() {
        let renderer = Renderer::<WgpuBridge<Dx12Native>>::new();
        let report = renderer.capability_report();

        assert_eq!(
            Renderer::<WgpuBridge<Dx12Native>>::new().backend_name(),
            "wgpu_dx12"
        );
        assert_eq!(report.bridge_type, BackendBridgeType::Wgpu);
        assert_eq!(report.actual_native_backend, NativeBackend::Dx12);
        assert_eq!(
            WgpuBridge::<Dx12Native>::new().wgpu_backends(),
            ::wgpu::Backends::DX12
        );
        assert_eq!(
            ::core::mem::size_of::<Renderer<WgpuBridge<Dx12Native>>>(),
            0
        );
        assert_eq!(::core::mem::size_of::<Renderer<WgpuDx12Bridge>>(), 0);
    }

    #[test]
    fn resource_ir_maps_to_wgpu_descriptors_without_backend_handles() {
        let buffer = BufferDesc::new(
            IrBufferId(7),
            "scene.object_buffer",
            4096,
            BufferUsageFlags::STORAGE.union(BufferUsageFlags::COPY_DST),
            BufferMemoryClass::DeviceLocal,
        );
        let texture = TextureDesc::new_2d(
            crate::ir::IrTextureId(3),
            "scene.color",
            1920,
            1080,
            TextureFormat::Rgba16Float,
            TextureUsageFlags::RENDER_TARGET.union(TextureUsageFlags::SAMPLED),
        );

        let wgpu_buffer = resource::buffer_descriptor(buffer);
        let wgpu_texture = resource::texture_descriptor(texture);

        assert_eq!(wgpu_buffer.label, Some("scene.object_buffer"));
        assert_eq!(wgpu_buffer.size, 4096);
        assert!(wgpu_buffer.usage.contains(::wgpu::BufferUsages::STORAGE));
        assert!(wgpu_buffer.usage.contains(::wgpu::BufferUsages::COPY_DST));
        assert_eq!(wgpu_texture.format, ::wgpu::TextureFormat::Rgba16Float);
        assert!(
            wgpu_texture
                .usage
                .contains(::wgpu::TextureUsages::RENDER_ATTACHMENT)
        );
        assert!(
            wgpu_texture
                .usage
                .contains(::wgpu::TextureUsages::TEXTURE_BINDING)
        );
    }

    #[test]
    fn descriptor_cache_blocks_runtime_creation_after_warmup_in_measured_builds() {
        let mut cache = diagnostics::WgpuDescriptorCache::default();
        let key = diagnostics::WgpuCacheKey {
            kind: diagnostics::WgpuCachedDescriptorKind::RenderPipeline,
            stable_id: 11,
        };
        let first = cache.ensure_cached(
            key,
            DescriptorFingerprint::from_u64(10),
            diagnostics::WgpuCreationPhase::Warmup,
            true,
        );
        let second = cache.ensure_cached(
            key,
            DescriptorFingerprint::from_u64(20),
            diagnostics::WgpuCreationPhase::Runtime,
            true,
        );

        assert_eq!(first.expect("warmup creation is allowed").created, true);
        assert_eq!(
            second,
            Err(diagnostics::WgpuBridgeFailure::RuntimeCreationAfterWarmup {
                kind: diagnostics::WgpuCachedDescriptorKind::RenderPipeline,
                stable_id: 11,
            })
        );
    }

    #[test]
    fn pass_execution_plans_compile_to_dense_batches() {
        let command = RenderPassCommand {
            pass: IrGraphPassId(1),
            pipeline: IrRenderPipelineId(2),
            material_bindings: Default::default(),
            view_bindings: Default::default(),
            scene_bindings: Default::default(),
            draw_packets: crate::ir::PacketRange { start: 4, count: 9 },
            indirect_draw_packets: crate::ir::PacketRange::empty(),
        };
        let graph = crate::ir::CompiledGraph {
            schema_version: crate::ir::GRAPH_IR_SCHEMA_VERSION,
            passes: vec![crate::ir::PassExecutionPlan {
                pass: IrGraphPassId(1),
                order: 0,
                command_kind: GraphPassKind::Render,
                render_command: Some(command),
                compute_command: None,
                copy_command: None,
            }],
            edges: Vec::new(),
            graph_fingerprint: DescriptorFingerprint::from_u64(1),
        };

        let batches = command::WgpuPassExecutionArrays::from_compiled_graph(&graph);

        assert_eq!(batches.render_batches.len(), 1);
        assert_eq!(batches.render_batches[0].command.draw_packets.count, 9);
        assert_eq!(::core::mem::size_of::<DrawPacket>(), 32);
    }

    #[test]
    fn wgpu_bridge_translation_cache_uses_ir_hashes() {
        let mut cache = diagnostics::WgpuDescriptorCache::default();
        let decision = cache.translate_resource_creation(
            IrDescriptorKey {
                kind: IrDescriptorKind::ResourceCreation,
                index: 4,
            },
            DescriptorFingerprint::from_u64(44),
        );
        let repeated = cache.translate_resource_creation(
            IrDescriptorKey {
                kind: IrDescriptorKind::ResourceCreation,
                index: 4,
            },
            DescriptorFingerprint::from_u64(44),
        );

        assert!(decision.translated);
        assert!(!repeated.translated);
        assert_eq!(cache.skipped_unchanged, 1);
    }

    #[test]
    fn health_report_names_bridge_truth_and_cache_status() {
        let mut cache = diagnostics::WgpuDescriptorCache::default();
        cache
            .ensure_cached(
                diagnostics::WgpuCacheKey {
                    kind: diagnostics::WgpuCachedDescriptorKind::Sampler,
                    stable_id: 1,
                },
                DescriptorFingerprint::from_u64(1),
                diagnostics::WgpuCreationPhase::Warmup,
                true,
            )
            .expect("sampler warmup should be cacheable");

        let health = diagnostics::WgpuBridgeHealthReport::from_parts::<Dx12Native>(
            device::WgpuAdapterInfo::unknown_for(NativeBackend::Dx12),
            device::WgpuLimitSummary::default(),
            device::WgpuFeatureSummary::default(),
            cache.status(),
        );

        assert_eq!(health.selected_wgpu_backend, ::wgpu::Backends::DX12);
        assert_eq!(health.actual_native_backend, NativeBackend::Dx12);
        assert_eq!(
            health.command_encoder_availability,
            crate::backend::NativeCommandEncoderAvailability::BridgeDoesNotExpose
        );
        assert!(health.native_interop_capabilities.native_device_available);
        assert!(
            !health
                .native_interop_capabilities
                .native_command_list_available_dx12
        );
        assert_eq!(
            health.core_bridge_status.backend_truth.status,
            crate::backend::WgpuBackendTruthStatus::MatchesRequestedBackend
        );
        assert!(!health.core_compatibility.private_internals_used);
        assert_eq!(health.pipeline_cache_status.cached_samplers, 1);
    }
}
