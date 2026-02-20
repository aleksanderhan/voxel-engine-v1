// src/main.rs
mod app;
mod chunks;
mod render;
mod svo;

#[cfg(target_arch = "wasm32")]
use winit::platform::web::EventLoopExtWebSys;

fn main() {
    let profile_enabled = std::env::args().any(|arg| arg == "--profile");
    let event_loop = winit::event_loop::EventLoop::new().unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        event_loop.spawn_app(app::App::new(profile_enabled));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        event_loop
            .run_app(&mut app::App::new(profile_enabled))
            .unwrap();
    }
}
