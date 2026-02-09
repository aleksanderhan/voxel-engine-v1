pub fn create_view(output: &wgpu::SurfaceTexture) -> wgpu::TextureView {
    output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default())
}
