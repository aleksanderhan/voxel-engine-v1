// src/main.rs
mod app;
mod chunks;
mod render;
mod svo;

fn main() {
    let profile_enabled = std::env::args().any(|arg| arg == "--profile");
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop
        .run_app(&mut app::App::new(profile_enabled))
        .unwrap();
}
