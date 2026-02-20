use std::{collections::HashMap, sync::Arc};

use glam::Vec3;
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings};
use winit::{dpi::PhysicalSize, window::Window};

use crate::chunks::{ChunkManager, VIEW_RADIUS_CHUNKS};
use crate::render::{
    bindgroups::{BlitBindGroup, SceneBindGroup},
    buffers::UniformBuffer,
    layouts::{create_blit_pipeline_layout, create_compute_pipeline_layout},
    pipelines::{create_blit_pipeline, create_compute_pipeline},
    shaders::{blit_wgsl, raymarch_compute_wgsl},
    textures::{create_output_texture, create_view},
};
use crate::svo::world::World;

pub struct GpuState {
    pub window: Arc<Window>,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pub compute_pipeline: wgpu::ComputePipeline,
    pub blit_pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: UniformBuffer,
    pub scene_bind_group: SceneBindGroup,
    pub blit_bind_group: BlitBindGroup,
    pub chunk_manager: ChunkManager,
    pub palette_buffer: wgpu::Buffer,
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,
    pub output_sampler: wgpu::Sampler,
    pub chunk_origin: [f32; 4],
    last_chunk_coord: Option<glam::IVec3>,
    profiler: GpuProfiler,
    pass_stats: HashMap<String, PassTimingStats>,
    profile_enabled: bool,
}

#[derive(Debug, Default, Clone)]
struct PassTimingStats {
    samples: u64,
    total_ms: f64,
    max_ms: f64,
}

impl PassTimingStats {
    fn record(&mut self, duration_ms: f64) {
        self.samples += 1;
        self.total_ms += duration_ms;
        self.max_ms = self.max_ms.max(duration_ms);
    }

    fn average_ms(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.total_ms / self.samples as f64
        }
    }
}

impl GpuState {
    pub async fn new(window: Arc<Window>, profile_enabled: bool) -> Result<Self, String> {
        let size = window.inner_size();
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
             .map_err(|e| format!("Failed to create surface: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("Failed to find a compatible GPU adapter: {e}"))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Device"),
                required_features: adapter.features() & GpuProfiler::ALL_WGPU_TIMER_FEATURES,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("Failed to create device: {e}"))?;

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
            label: Some("Raymarch Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(raymarch_compute_wgsl().into()),
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
        let (output_texture, output_view) = create_output_texture(&device, size);
        let output_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Output Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let scene_bind_group = SceneBindGroup::new(
            &device,
            &uniform_buffer.buffer,
            &chunk_manager.brick_index_buffer,
            &chunk_manager.brick_materials_buffer,
            &palette_buffer,
            &chunk_manager.chunk_occupancy_buffer,
            &chunk_manager.region_occupancy_buffer,
            &output_view,
        );
        let blit_bind_group = BlitBindGroup::new(
            &device,
            &uniform_buffer.buffer,
            &output_view,
            &output_sampler,
        );

        let compute_layout = create_compute_pipeline_layout(&device, &scene_bind_group.layout);
        let blit_layout = create_blit_pipeline_layout(&device, &blit_bind_group.layout);
        let compute_pipeline = create_compute_pipeline(&device, &compute_layout, &shader);
        let blit_pipeline = create_blit_pipeline(&device, &config, &blit_layout, &blit_shader);
        let profiler = GpuProfiler::new(&device, GpuProfilerSettings::default())
            .map_err(|e| format!("Failed to create GPU profiler: {e}"))?;

        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            size,
            compute_pipeline,
            blit_pipeline,
            uniform_buffer,
            scene_bind_group,
            blit_bind_group,
            chunk_manager,
            palette_buffer,
            output_texture,
            output_view,
            output_sampler,
            chunk_origin: [0.0, 0.0, 0.0, 0.0],
            last_chunk_coord: None,
            profiler,
            pass_stats: HashMap::new(),
            profile_enabled,
        })
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
                &self.chunk_manager.chunk_occupancy_buffer,
                &self.chunk_manager.region_occupancy_buffer,
                &self.output_view,
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
        let (output_texture, output_view) = create_output_texture(&self.device, new_size);
        self.output_texture = output_texture;
        self.output_view = output_view;
        self.scene_bind_group = SceneBindGroup::new(
            &self.device,
            &self.uniform_buffer.buffer,
            &self.chunk_manager.brick_index_buffer,
            &self.chunk_manager.brick_materials_buffer,
            &self.palette_buffer,
            &self.chunk_manager.chunk_occupancy_buffer,
            &self.chunk_manager.region_occupancy_buffer,
            &self.output_view,
        );
        self.blit_bind_group = BlitBindGroup::new(
            &self.device,
            &self.uniform_buffer.buffer,
            &self.output_view,
            &self.output_sampler,
        );
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
            [
                self.chunk_manager.region_wrap_offset().x,
                self.chunk_manager.region_wrap_offset().y,
                self.chunk_manager.region_wrap_offset().z,
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
            let mut frame_scope = self.profiler.scope("frame", &mut encoder);

            {
                let mut compute_pass = frame_scope.scoped_compute_pass("raymarch compute");
                compute_pass.set_pipeline(&self.compute_pipeline);
                compute_pass.set_bind_group(0, &self.scene_bind_group.bind_group, &[]);
                let x_groups = (self.size.width + 7) / 8;
                let y_groups = (self.size.height + 7) / 8;
                compute_pass.dispatch_workgroups(x_groups, y_groups, 1);
            }

            {
                let mut render_pass = frame_scope.scoped_render_pass(
                    "blit pass",
                    wgpu::RenderPassDescriptor {
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
                    },
                );
                render_pass.set_pipeline(&self.blit_pipeline);
                render_pass.set_bind_group(0, &self.blit_bind_group.bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
        }

        self.profiler.resolve_queries(&mut encoder);
        self.queue.submit(Some(encoder.finish()));
        output.present();
        self.profiler.end_frame().expect("Failed to end GPU frame");
        if let Some(profiling_data) =
            self.profiler
                .process_finished_frame(self.queue.get_timestamp_period())
        {
            let _ = wgpu_profiler::chrometrace::write_chrometrace(
                std::path::Path::new("wgpu-profile.json"),
                &profiling_data,
            );
            if self.profile_enabled {
                self.update_pass_stats(&profiling_data);
            }
        }
        Ok(())
    }

    fn update_pass_stats(&mut self, results: &[wgpu_profiler::GpuTimerQueryResult]) {
        let mut stack = results.to_vec();
        while let Some(result) = stack.pop() {
            if let Some(range) = result.time {
                let duration_ms = (range.end - range.start) * 1000.0;
                self.pass_stats
                    .entry(result.label.clone())
                    .or_default()
                    .record(duration_ms);
            }
            stack.extend(result.nested_queries);
        }

        for label in ["raymarch compute", "blit pass"] {
            if let Some(stats) = self.pass_stats.get(label) {
                println!(
                    "[profiler] {label}: avg {:.3} ms, max {:.3} ms over {} samples",
                    stats.average_ms(),
                    stats.max_ms,
                    stats.samples
                );
            }
        }
    }
}
