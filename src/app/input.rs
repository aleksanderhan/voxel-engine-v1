use glam::{Vec2, Vec3};
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Debug, Default)]
pub struct InputState {
    move_forward: bool,
    move_backward: bool,
    move_left: bool,
    move_right: bool,
    move_up: bool,
    move_down: bool,
    sprint: bool,
    last_cursor: Option<Vec2>,
    look_delta: Vec2,
}

impl InputState {
    pub fn process_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => {
                let pressed = *state == ElementState::Pressed;
                match code {
                    KeyCode::KeyW => self.move_forward = pressed,
                    KeyCode::KeyS => self.move_backward = pressed,
                    KeyCode::KeyA => self.move_left = pressed,
                    KeyCode::KeyD => self.move_right = pressed,
                    KeyCode::Space => self.move_up = pressed,
                    KeyCode::AltLeft => self.move_down = pressed,
                    KeyCode::ShiftLeft => self.sprint = pressed,
                    _ => return false,
                }
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                let position = Vec2::new(position.x as f32, position.y as f32);
                if let Some(last) = self.last_cursor {
                    self.look_delta += position - last;
                }
                self.last_cursor = Some(position);
                true
            }
            _ => false,
        }
    }

    pub fn movement_axis(&self) -> Vec3 {
        let x = self.move_right as i8 - self.move_left as i8;
        let y = self.move_up as i8 - self.move_down as i8;
        let z = self.move_forward as i8 - self.move_backward as i8;
        Vec3::new(x as f32, y as f32, z as f32)
    }

    pub fn take_look_delta(&mut self) -> Vec2 {
        let delta = self.look_delta;
        self.look_delta = Vec2::ZERO;
        delta
    }

    pub fn is_sprinting(&self) -> bool {
        self.sprint
    }

    pub fn clear_cursor(&mut self) {
        self.last_cursor = None;
        self.look_delta = Vec2::ZERO;
    }
}
