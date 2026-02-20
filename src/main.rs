// src/main.rs
mod app;
mod chunks;
mod render;
mod svo;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    let profile_enabled = std::env::args().any(|arg| arg == "--profile");
    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
    }
    #[cfg(target_arch = "wasm32")]
    let profile_enabled = false;

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop
        .run_app(&mut app::App::new(profile_enabled))
        .unwrap();
}
