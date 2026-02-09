use std::sync::Arc;

use glam::Vec3;
use winit::{dpi::PhysicalSize, window::Window};

use crate::chunks::{ChunkManager, VIEW_RADIUS_CHUNKS};
use crate::render::{
    bindgroups::SceneBindGroup,
    buffers::UniformBuffer,
    layouts::create_pipeline_layout,
    pipelines::{create_blit_pipeline, create_render_pipeline},
    shaders::{blit_wgsl, shader_wgsl},
    textures::create_view,
};
use crate::svo::world::World;

pub struct GpuState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pub pipeline: wgpu::RenderPipeline,
    pub blit_pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: UniformBuffer,
    pub scene_bind_group: SceneBindGroup,
    pub chunk_manager: ChunkManager,
    pub palette_buffer: wgpu::Buffer,
    pub chunk_origin: [f32; 4],
    last_chunk_coord: Option<glam::IVec3>,
}

impl GpuState {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an adapter");
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .expect("Failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let present_mode = surface_caps.present_modes[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Raymarch Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_wgsl().into()),
        });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(blit_wgsl().into()),
        });

        let uniform_buffer = UniformBuffer::new(&device, size);
        let chunk_manager = ChunkManager::new(&device);
        let palette_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Palette Buffer"),
            size: (256 * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene_bind_group = SceneBindGroup::new(
            &device,
            &uniform_buffer.buffer,
            &chunk_manager.brick_index_buffer,
            &chunk_manager.brick_materials_buffer,
            &palette_buffer,
        );

        let pipeline_layout = create_pipeline_layout(&device, &scene_bind_group.layout);
        let pipeline = create_render_pipeline(&device, &config, &pipeline_layout, &shader);
        let blit_pipeline = create_blit_pipeline(&device, &config, &pipeline_layout, &blit_shader);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            pipeline,
            blit_pipeline,
            uniform_buffer,
            scene_bind_group,
            chunk_manager,
            palette_buffer,
            chunk_origin: [0.0, 0.0, 0.0, 0.0],
            last_chunk_coord: None,
        }
    }

    pub fn update_chunk_data(&mut self, world: &World, camera_pos: Vec3) {
        let chunk_size = crate::svo::chunk::CHUNK_SIZE as f32;
        let chunk_coord = glam::IVec3::new(
            (camera_pos.x / chunk_size).floor() as i32,
            (camera_pos.y / chunk_size).floor() as i32,
            (camera_pos.z / chunk_size).floor() as i32,
        );
        if self.last_chunk_coord != Some(chunk_coord) {
            self.last_chunk_coord = Some(chunk_coord);
            let origin = (chunk_coord - glam::IVec3::splat(VIEW_RADIUS_CHUNKS))
                * crate::svo::chunk::CHUNK_SIZE;
            self.chunk_origin = [origin.x as f32, origin.y as f32, origin.z as f32, 0.0];
        }
        if self
            .chunk_manager
            .update_frame(&self.queue, world, chunk_coord)
        {
            self.scene_bind_group = SceneBindGroup::new(
                &self.device,
                &self.uniform_buffer.buffer,
                &self.chunk_manager.brick_index_buffer,
                &self.chunk_manager.brick_materials_buffer,
                &self.palette_buffer,
            );
        }
        self.queue.write_buffer(
            &self.palette_buffer,
            0,
            bytemuck::cast_slice(&world.palette),
        );
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn update(
        &mut self,
        time: f32,
        fps: f32,
        camera_pos: Vec3,
        camera_forward: Vec3,
        camera_right: Vec3,
        camera_up: Vec3,
    ) {
        self.uniform_buffer.update(
            &self.queue,
            self.size,
            time,
            fps,
            [camera_pos.x, camera_pos.y, camera_pos.z, 0.0],
            [camera_forward.x, camera_forward.y, camera_forward.z, 0.0],
            [camera_right.x, camera_right.y, camera_right.z, 0.0],
            [camera_up.x, camera_up.y, camera_up.z, 0.0],
            self.chunk_origin,
            [
                self.chunk_manager.chunk_wrap_offset().x,
                self.chunk_manager.chunk_wrap_offset().y,
                self.chunk_manager.chunk_wrap_offset().z,
                0,
            ],
        );
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = create_view(&output);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.scene_bind_group.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blit Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            render_pass.set_pipeline(&self.blit_pipeline);
            render_pass.set_bind_group(0, &self.scene_bind_group.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}
