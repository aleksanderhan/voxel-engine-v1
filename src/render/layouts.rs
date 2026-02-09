pub fn create_compute_pipeline_layout(
    device: &wgpu::Device,
    scene_layout: &wgpu::BindGroupLayout,
) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Compute Pipeline Layout"),
        bind_group_layouts: &[scene_layout],
        push_constant_ranges: &[],
    })
}

pub fn create_blit_pipeline_layout(
    device: &wgpu::Device,
    blit_layout: &wgpu::BindGroupLayout,
) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Blit Pipeline Layout"),
        bind_group_layouts: &[blit_layout],
        push_constant_ranges: &[],
    })
}
