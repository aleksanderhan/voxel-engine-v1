use std::{sync::Arc, time::Instant};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{KeyEvent, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    window::{Fullscreen, Window, WindowId},
};

use crate::render::gpu::GpuState;

#[derive(Default)]
pub struct App {
    window: Option<Arc<Window>>,
    state: Option<GpuState>,
    start_time: Option<Instant>,
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

        self.window = Some(window);
        self.start_time = Some(Instant::now());

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

            WindowEvent::RedrawRequested => {
                if let Some(state) = &mut self.state {
                    let elapsed = self.start_time.map_or(0.0, |start| start.elapsed().as_secs_f32());
                    state.update(elapsed);
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


