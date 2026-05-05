use bevy::render::settings::{Backends, PowerPreference};

fn main() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: Backends::DX12,
        flags: wgpu::InstanceFlags::empty(),
        memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
        display: None,
        backend_options: wgpu::BackendOptions::default(),
    });
    let adapter = bevy::tasks::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("DX12 adapter unavailable");
    println!("{:?}", adapter.get_info());
}
