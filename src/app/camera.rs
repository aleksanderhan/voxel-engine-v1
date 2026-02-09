use glam::Vec3;

use crate::app::InputState;

#[derive(Debug)]
pub struct CameraController {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub sensitivity: f32,
    pub sprint_multiplier: f32,
}

impl CameraController {
    pub fn new(
        position: Vec3,
        yaw: f32,
        pitch: f32,
        speed: f32,
        sensitivity: f32,
        sprint_multiplier: f32,
    ) -> Self {
        Self {
            position,
            yaw,
            pitch,
            speed,
            sensitivity,
            sprint_multiplier,
        }
    }

    pub fn update(&mut self, input: &mut InputState, dt_seconds: f32) {
        let movement = input.movement_axis();
        if movement.length_squared() > 0.0 {
            let speed_multiplier = if input.is_sprinting() {
                self.sprint_multiplier
            } else {
                1.0
            };
            let (forward, right, _) = self.basis();
            let planar_forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let direction =
                (planar_forward * movement.z + right * movement.x + Vec3::Y * movement.y)
                    .normalize_or_zero();
            self.position += direction * self.speed * speed_multiplier * dt_seconds;
        }

        let look_delta = input.take_look_delta();
        self.yaw += look_delta.x * self.sensitivity;
        self.pitch = (self.pitch - look_delta.y * self.sensitivity).clamp(-1.54, 1.54);
    }

    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = Vec3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize();
        let right = forward.cross(Vec3::Y).normalize();
        let up = right.cross(forward).normalize();
        (forward, right, up)
    }
}
