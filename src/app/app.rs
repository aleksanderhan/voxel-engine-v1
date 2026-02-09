use std::{sync::Arc, time::Instant};

use glam::Vec3;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{CursorGrabMode, Fullscreen, Window, WindowId},
};

use crate::app::{CameraController, InputState};
use crate::render::gpu::GpuState;

pub struct App {
    window: Option<Arc<Window>>,
    state: Option<GpuState>,
    input: InputState,
    camera: CameraController,
    start_time: Option<Instant>,
    last_frame: Option<Instant>,
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

        window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
        }
        window.set_cursor_visible(false);

        self.window = Some(window);
        let now = Instant::now();
        self.start_time = Some(now);
        self.last_frame = Some(now);

        if let Some(window) = &self.window {
            self.state = Some(pollster::block_on(GpuState::new(window.clone())));
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

            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, .. }, .. } => {
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
            }

            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let elapsed = self.start_time.map_or(0.0, |start| start.elapsed().as_secs_f32());
                    let now = Instant::now();
                    let dt = self
                        .last_frame
                        .map_or(0.0, |last| (now - last).as_secs_f32());
                    self.last_frame = Some(now);
                    self.camera.update(&mut self.input, dt);
                    let (forward, right, up) = self.camera.basis();
                    state.update(elapsed, self.camera.position, forward, right, up);
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Drive continuous redraw (optional).
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
