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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WgpuNativeSurfaceDesc {
    pub stable_name: &'static str,
    pub width: u32,
    pub height: u32,
    pub format: ::wgpu::TextureFormat,
    pub usage: ::wgpu::TextureUsages,
    pub present_mode: ::wgpu::PresentMode,
    pub alpha_mode: ::wgpu::CompositeAlphaMode,
    pub desired_maximum_frame_latency: u32,
}

impl Default for WgpuNativeSurfaceDesc {
    fn default() -> Self {
        Self {
            stable_name: "fun.surface",
            width: 1,
            height: 1,
            format: ::wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: ::wgpu::TextureUsages::RENDER_ATTACHMENT,
            present_mode: ::wgpu::PresentMode::Fifo,
            alpha_mode: ::wgpu::CompositeAlphaMode::Auto,
            desired_maximum_frame_latency: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WgpuSurfaceClearDesc {
    pub stable_name: &'static str,
    pub color: [f64; 4],
}

impl Default for WgpuSurfaceClearDesc {
    fn default() -> Self {
        Self {
            stable_name: "fun_renderer.surface.clear",
            color: [0.015, 0.017, 0.021, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgpuSurfaceAcquireFailure {
    Timeout,
    Occluded,
    Outdated,
    Lost,
    Validation,
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

#[must_use]
pub fn native_surface_configuration(
    desc: WgpuNativeSurfaceDesc,
    view_formats: Vec<::wgpu::TextureFormat>,
) -> ::wgpu::SurfaceConfiguration {
    ::wgpu::SurfaceConfiguration {
        usage: desc.usage,
        format: desc.format,
        width: desc.width,
        height: desc.height,
        present_mode: desc.present_mode,
        desired_maximum_frame_latency: desc.desired_maximum_frame_latency,
        alpha_mode: desc.alpha_mode,
        view_formats,
    }
}

pub fn present_clear_surface_frame(
    device: &::wgpu::Device,
    queue: &::wgpu::Queue,
    surface: &::wgpu::Surface<'_>,
    desc: WgpuSurfaceClearDesc,
) -> Result<(), WgpuSurfaceAcquireFailure> {
    let surface_texture = match surface.get_current_texture() {
        ::wgpu::CurrentSurfaceTexture::Success(texture)
        | ::wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
        ::wgpu::CurrentSurfaceTexture::Timeout => return Err(WgpuSurfaceAcquireFailure::Timeout),
        ::wgpu::CurrentSurfaceTexture::Occluded => return Err(WgpuSurfaceAcquireFailure::Occluded),
        ::wgpu::CurrentSurfaceTexture::Outdated => return Err(WgpuSurfaceAcquireFailure::Outdated),
        ::wgpu::CurrentSurfaceTexture::Lost => return Err(WgpuSurfaceAcquireFailure::Lost),
        ::wgpu::CurrentSurfaceTexture::Validation => {
            return Err(WgpuSurfaceAcquireFailure::Validation);
        }
    };
    let surface_view = surface_texture
        .texture
        .create_view(&::wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&::wgpu::CommandEncoderDescriptor {
        label: Some(desc.stable_name),
    });
    {
        let _pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
            label: Some(desc.stable_name),
            color_attachments: &[Some(::wgpu::RenderPassColorAttachment {
                view: &surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: ::wgpu::LoadOp::Clear(::wgpu::Color {
                        r: desc.color[0],
                        g: desc.color[1],
                        b: desc.color[2],
                        a: desc.color[3],
                    }),
                    store: ::wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    queue.submit([encoder.finish()]);
    surface_texture.present();
    Ok(())
}
