use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[cfg(not(target_arch = "wasm32"))]
type AppInstant = std::time::Instant;
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
struct AppInstant(f64);

#[cfg(not(target_arch = "wasm32"))]
fn app_now() -> AppInstant {
    AppInstant::now()
}

#[cfg(target_arch = "wasm32")]
fn app_now() -> AppInstant {
    AppInstant(js_sys::Date::now() / 1000.0)
}

#[cfg(not(target_arch = "wasm32"))]
fn seconds_since(start: AppInstant, now: AppInstant) -> f32 {
    (now - start).as_secs_f32()
}

#[cfg(target_arch = "wasm32")]
fn seconds_since(start: AppInstant, now: AppInstant) -> f32 {
    (now.0 - start.0).max(0.0) as f32
}

use glam::{IVec3, Vec3};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{CursorGrabMode, Window, WindowId},
};

use crate::app::{CameraController, InputState};
use crate::chunks::VIEW_SIZE;
use crate::render::gpu::GpuState;
use crate::svo::{VoxFile, World};

pub struct App {
    window: Option<Arc<Window>>,
    state: Option<GpuState>,
    input: InputState,
    camera: CameraController,
    start_time: Option<AppInstant>,
    last_frame: Option<AppInstant>,
    fps: f32,
    world: World,
    profile_enabled: bool,
    #[cfg(target_arch = "wasm32")]
    pending_state: Option<Rc<RefCell<Option<GpuState>>>>,
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
            #[cfg(target_arch = "wasm32")]
            pending_state: None,
        }
    }
}

impl App {
    pub fn new(profile_enabled: bool) -> Self {
        Self {
            profile_enabled,
            ..Self::default()
        }
    }

    fn load_initial_world(&mut self) {
        if self.world.chunks.is_empty() {
            #[cfg(target_arch = "wasm32")]
            let vox_result = VoxFile::from_bytes(include_bytes!("../../assets/models/house.vox"));
            #[cfg(not(target_arch = "wasm32"))]
            let vox_result = VoxFile::load("assets/models/house.vox");

            match vox_result {
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
    }

    fn lock_cursor(window: &Window) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                let _ = window.set_cursor_grab(CursorGrabMode::Confined);
            }
            window.set_cursor_visible(false);
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = window;
        }
    }

    fn unlock_cursor(window: &Window) {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
    }

    #[cfg(target_arch = "wasm32")]
    fn finalize_wasm_state_if_ready(&mut self) {
        if self.state.is_some() {
            return;
        }
        if let Some(pending) = self.pending_state.clone() {
            let state = pending.borrow_mut().take();
            if let Some(mut state) = state {
                state.update_chunk_data(&self.world, self.camera.position);
                self.state = Some(state);
                self.pending_state = None;
            }
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the window on resume (this is the intended place in the new API).
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("SVO Engine")
                        .with_inner_size(PhysicalSize::new(1280, 720)),
                )
                .unwrap(),
        );

        Self::lock_cursor(&window);

        self.window = Some(window);
        let now = app_now();
        self.start_time = Some(now);
        self.last_frame = Some(now);

        self.load_initial_world();

        if let Some(window) = &self.window {
            #[cfg(not(target_arch = "wasm32"))]
            {
                match pollster::block_on(GpuState::new(window.clone(), self.profile_enabled)) {
                    Ok(mut state) => {
                        state.update_chunk_data(&self.world, self.camera.position);
                        self.state = Some(state);
                    }
                    Err(error) => {
                        eprintln!("GPU initialization failed: {error}");
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                let pending = Rc::new(RefCell::new(None));
                let pending_for_task = pending.clone();
                let window = window.clone();
                let profile_enabled = self.profile_enabled;
                spawn_local(async move {
                    match GpuState::new(window, profile_enabled).await {
                        Ok(state) => {
                            *pending_for_task.borrow_mut() = Some(state);
                        }
                        Err(error) => {
                            eprintln!("GPU initialization failed: {error}");
                        }
                    }
                });
                self.pending_state = Some(pending);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        #[cfg(target_arch = "wasm32")]
        self.finalize_wasm_state_if_ready();
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
                if let Some(window) = &self.window {
                    if is_focused {
                        Self::lock_cursor(window);
                    } else {
                        Self::unlock_cursor(window);
                        self.input.clear_cursor();
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let now = app_now();
                    let elapsed = self
                        .start_time
                        .map_or(0.0, |start| seconds_since(start, now));
                    let dt = self.last_frame.map_or(0.0, |last| seconds_since(last, now));
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
        #[cfg(target_arch = "wasm32")]
        self.finalize_wasm_state_if_ready();
        // Drive continuous redraw (optional).
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
