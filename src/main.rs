// src/main.rs
mod app;
mod chunk_manager;
mod render;
mod svo;

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.run_app(&mut app::App::default()).unwrap();
}
