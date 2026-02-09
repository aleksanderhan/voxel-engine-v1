pub fn create_pipeline_layout(
    device: &wgpu::Device,
    scene_layout: &wgpu::BindGroupLayout,
) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[scene_layout],
        push_constant_ranges: &[],
    })
}
