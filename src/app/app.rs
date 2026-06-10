use std::sync::Arc;

use web_time::Instant;

use glam::{IVec3, Vec3};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
};

#[cfg(not(target_arch = "wasm32"))]
use winit::window::{CursorGrabMode, Fullscreen};

#[cfg(target_arch = "wasm32")]
use winit::platform::web::WindowAttributesExtWebSys;

use crate::app::{CameraController, InputState};
use crate::chunks::VIEW_SIZE;
use crate::render::gpu::GpuState;
use crate::svo::{VoxFile, World};

pub enum AppEvent {
    GpuReady(GpuState),
}

pub struct App {
    window: Option<Arc<Window>>,
    state: Option<GpuState>,
    input: InputState,
    camera: CameraController,
    start_time: Option<Instant>,
    last_frame: Option<Instant>,
    fps: f32,
    world: World,
    profile_enabled: bool,
    event_proxy: Option<EventLoopProxy<AppEvent>>,
    initializing_gpu: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            state: None,
            input: InputState::default(),
            camera: CameraController::new(
                Vec3::new(0.0, 2.5, 6.0),
                -std::f32::consts::FRAC_PI_2,
                -0.245,
                6.0,
                0.002,
                2.0,
            ),
            start_time: None,
            last_frame: None,
            fps: 0.0,
            world: World::new(),
            profile_enabled: false,
            event_proxy: None,
            initializing_gpu: false,
        }
    }
}

impl App {
    pub fn new(profile_enabled: bool, event_proxy: EventLoopProxy<AppEvent>) -> Self {
        Self {
            profile_enabled,
            event_proxy: Some(event_proxy),
            ..Self::default()
        }
    }

    fn start_gpu(&mut self) {
        if self.state.is_some() || self.initializing_gpu {
            return;
        }
        let Some(window) = &self.window else {
            return;
        };

        self.initializing_gpu = true;
        let window = window.clone();
        let profile_enabled = self.profile_enabled;

        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self
                .event_proxy
                .as_ref()
                .expect("web builds need an event loop proxy")
                .clone();
            wasm_bindgen_futures::spawn_local(async move {
                let state = GpuState::new(window, profile_enabled).await;
                let _ = proxy.send_event(AppEvent::GpuReady(state));
            });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut state = pollster::block_on(GpuState::new(window, profile_enabled));
            state.update_chunk_data(&self.world, self.camera.position);
            self.state = Some(state);
            self.initializing_gpu = false;
        }
    }
}

fn load_starting_vox() -> Result<VoxFile, crate::svo::VoxError> {
    #[cfg(target_arch = "wasm32")]
    {
        VoxFile::from_bytes(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/models/house.vox"
        )))
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        VoxFile::load("assets/models/house.vox")
    }
}

#[cfg(target_arch = "wasm32")]
fn window_attributes() -> winit::window::WindowAttributes {
    Window::default_attributes()
        .with_title("SVO Engine")
        .with_inner_size(PhysicalSize::new(1280, 720))
        .with_append(true)
}

#[cfg(not(target_arch = "wasm32"))]
fn window_attributes() -> winit::window::WindowAttributes {
    Window::default_attributes()
        .with_title("SVO Engine")
        .with_inner_size(PhysicalSize::new(1280, 720))
}

#[cfg(target_arch = "wasm32")]
fn configure_window(window: &Window) {
    window.set_cursor_visible(true);
}

#[cfg(not(target_arch = "wasm32"))]
fn configure_window(window: &Window) {
    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
    if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
    }
    window.set_cursor_visible(false);
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.start_gpu();
            return;
        }

        // Create the window on resume (this is the intended place in the new API).
        let window = Arc::new(event_loop.create_window(window_attributes()).unwrap());

        configure_window(&window);

        self.window = Some(window);
        let now = Instant::now();
        self.start_time = Some(now);
        self.last_frame = Some(now);

        if self.world.chunks.is_empty() {
            match load_starting_vox() {
                Ok(vox) => {
                    if let Some(model) = vox.models.first() {
                        let world_size = IVec3::new(
                            model.size[0] as i32,
                            model.size[2] as i32,
                            model.size[1] as i32,
                        );
                        let map_center = IVec3::splat(VIEW_SIZE / 2);
                        let origin = map_center - world_size / 2;
                        let center = map_center;
                        self.world.import_vox_file(&vox, origin);
                        let max_y = origin.y + world_size.y - 1;
                        let surface_y = self
                            .world
                            .surface_height_at(center.x, center.z, origin.y, max_y);
                        let camera_y = surface_y
                            .map(|height| height as f32 + 6.0)
                            .unwrap_or((max_y + 6) as f32);
                        self.camera.position =
                            Vec3::new(center.x as f32, camera_y, center.z as f32);
                    } else {
                        self.world.import_vox_file(&vox, glam::IVec3::ZERO);
                    }
                }
                Err(error) => {
                    eprintln!("Failed to load model: {:?}", error);
                }
            }
        }

        self.start_gpu();
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::GpuReady(mut state) => {
                state.update_chunk_data(&self.world, self.camera.position);
                self.state = Some(state);
                self.initializing_gpu = false;
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        self.input.process_window_event(&event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key, .. },
                ..
            } => {
                if logical_key == Key::Named(NamedKey::Escape) {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.resize(size);
                }
            }
            WindowEvent::Focused(is_focused) => {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(window) = &self.window {
                    if is_focused {
                        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                        }
                        window.set_cursor_visible(false);
                    } else {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                        self.input.clear_cursor();
                    }
                }

                #[cfg(target_arch = "wasm32")]
                if !is_focused {
                    self.input.clear_cursor();
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let elapsed = self
                        .start_time
                        .map_or(0.0, |start| start.elapsed().as_secs_f32());
                    let now = Instant::now();
                    let dt = self
                        .last_frame
                        .map_or(0.0, |last| (now - last).as_secs_f32());
                    self.last_frame = Some(now);
                    if dt > 0.0 {
                        let instant_fps = 1.0 / dt;
                        self.fps = if self.fps == 0.0 {
                            instant_fps
                        } else {
                            self.fps * 0.9 + instant_fps * 0.1
                        };
                    }
                    self.camera.update(&mut self.input, dt);
                    let (forward, right, up) = self.camera.basis();
                    state.update_chunk_data(&self.world, self.camera.position);
                    state.update(elapsed, self.fps, self.camera.position, forward, right, up);
                    if let Err(error) = state.render() {
                        match error {
                            wgpu::SurfaceError::Lost => {
                                state.resize(state.size);
                            }
                            wgpu::SurfaceError::OutOfMemory => {
                                event_loop.exit();
                            }
                            _ => {} // Outdated, Timeout, Other, and any future variants
                        }
                    }
                }
            }

            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        self.input.process_device_event(&event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Drive continuous redraw (optional).
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
