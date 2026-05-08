use crate::{
    bridge::wgpu::resource::{texture_format, texture_usages},
    ir::{TextureFormat, TextureUsageFlags},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuSurfaceDesc {
    pub stable_name: &'static str,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    pub usage: TextureUsageFlags,
    pub present_mode: ::wgpu::PresentMode,
    pub alpha_mode: ::wgpu::CompositeAlphaMode,
    pub desired_maximum_frame_latency: u32,
}

impl Default for WgpuSurfaceDesc {
    fn default() -> Self {
        Self {
            stable_name: "fun.surface",
            width: 1,
            height: 1,
            format: TextureFormat::Rgba8Srgb,
            usage: TextureUsageFlags::RENDER_TARGET,
            present_mode: ::wgpu::PresentMode::Fifo,
            alpha_mode: ::wgpu::CompositeAlphaMode::Auto,
            desired_maximum_frame_latency: 2,
        }
    }
}

#[must_use]
pub fn surface_configuration(
    desc: WgpuSurfaceDesc,
    view_formats: &[::wgpu::TextureFormat],
) -> ::wgpu::SurfaceConfiguration {
    ::wgpu::SurfaceConfiguration {
        usage: texture_usages(desc.usage),
        format: texture_format(desc.format),
        width: desc.width,
        height: desc.height,
        present_mode: desc.present_mode,
        desired_maximum_frame_latency: desc.desired_maximum_frame_latency,
        alpha_mode: desc.alpha_mode,
        view_formats: view_formats.to_vec(),
    }
}
