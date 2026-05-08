use crate::backend::NativeBackend;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuAdapterDeviceType {
    Other,
    IntegratedGpu,
    DiscreteGpu,
    VirtualGpu,
    Cpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuAdapterInfo<'a> {
    pub name: &'a str,
    pub vendor: u32,
    pub device: u32,
    pub device_type: WgpuAdapterDeviceType,
    pub backend: NativeBackend,
}

impl<'a> WgpuAdapterInfo<'a> {
    #[must_use]
    pub const fn unknown_for(backend: NativeBackend) -> Self {
        Self {
            name: "unknown",
            vendor: 0,
            device: 0,
            device_type: WgpuAdapterDeviceType::Other,
            backend,
        }
    }

    #[must_use]
    pub fn from_wgpu(info: &'a ::wgpu::AdapterInfo) -> Self {
        Self {
            name: info.name.as_str(),
            vendor: info.vendor,
            device: info.device,
            device_type: device_type(info.device_type),
            backend: native_backend(info.backend),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuLimitSummary {
    pub max_texture_dimension_2d: u32,
    pub max_bind_groups: u32,
    pub max_bindings_per_bind_group: u32,
    pub max_buffer_size: u64,
    pub max_color_attachments: u32,
    pub max_compute_workgroups_per_dimension: u32,
}

impl WgpuLimitSummary {
    #[must_use]
    pub fn from_wgpu(limits: &::wgpu::Limits) -> Self {
        Self {
            max_texture_dimension_2d: limits.max_texture_dimension_2d,
            max_bind_groups: limits.max_bind_groups,
            max_bindings_per_bind_group: limits.max_bindings_per_bind_group,
            max_buffer_size: limits.max_buffer_size,
            max_color_attachments: limits.max_color_attachments,
            max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuFeatureSummary {
    pub bits_low: u64,
    pub bits_high: u64,
    pub timestamp_query: bool,
    pub pipeline_cache: bool,
    pub shader_f16: bool,
    pub ray_tracing_acceleration_structure: bool,
    pub mesh_shader: bool,
}

impl WgpuFeatureSummary {
    #[must_use]
    pub fn from_wgpu(features: ::wgpu::Features) -> Self {
        let bits = features.bits();
        Self {
            bits_low: bits.0[0],
            bits_high: bits.0[1],
            timestamp_query: features.contains(::wgpu::Features::TIMESTAMP_QUERY),
            pipeline_cache: features.contains(::wgpu::Features::PIPELINE_CACHE),
            shader_f16: features.contains(::wgpu::Features::SHADER_F16),
            ray_tracing_acceleration_structure: features
                .contains(::wgpu::Features::EXPERIMENTAL_RAY_QUERY),
            mesh_shader: features.contains(::wgpu::Features::EXPERIMENTAL_MESH_SHADER),
        }
    }
}

#[must_use]
pub const fn native_backend(backend: ::wgpu::Backend) -> NativeBackend {
    match backend {
        ::wgpu::Backend::Dx12 => NativeBackend::Dx12,
        ::wgpu::Backend::Vulkan => NativeBackend::Vulkan,
        ::wgpu::Backend::Metal => NativeBackend::Metal,
        _ => NativeBackend::Unknown,
    }
}

#[must_use]
pub const fn device_type(device_type: ::wgpu::DeviceType) -> WgpuAdapterDeviceType {
    match device_type {
        ::wgpu::DeviceType::Other => WgpuAdapterDeviceType::Other,
        ::wgpu::DeviceType::IntegratedGpu => WgpuAdapterDeviceType::IntegratedGpu,
        ::wgpu::DeviceType::DiscreteGpu => WgpuAdapterDeviceType::DiscreteGpu,
        ::wgpu::DeviceType::VirtualGpu => WgpuAdapterDeviceType::VirtualGpu,
        ::wgpu::DeviceType::Cpu => WgpuAdapterDeviceType::Cpu,
    }
}
