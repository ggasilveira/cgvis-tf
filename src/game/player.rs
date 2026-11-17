use crate::engine::prelude::*;

pub struct Player {
    pitch: f32,
    yaw: f32,
    speed: f32,
    /// Size of the player's body for collisions.
    radius: f32,
    /// Radians per pixel of mouse movement.
    sensitivity: f32,
}
impl Player {
    pub fn new() -> Self {
        Self {
            pitch: 0.0,
            yaw: 0.0,
            speed: 4.0,
            radius: 0.3,
            sensitivity: 0.005,
        }
    }
    pub fn look_direction(&self) -> Vec3 {
        Mat3::from_rotation_y(self.yaw) * Mat3::from_rotation_x(self.pitch) * Vec3::Z
    }
}
impl Behaviour for Player {
    fn start(&mut self, ctx: &mut Context) {
        let t = ctx.transform().clone();
        ctx.lights_mut().push(Light::Spot(SpotLight {
            position: t.position,
            direction: self.look_direction(),
            color: Vec3::ONE,
            intensity: 5.0,
            inner_angle: 5f32.to_radians(),
            outer_angle: 20f32.to_radians(),
        }))
    }
    fn update(&mut self, ctx: &mut Context) {
        let input = ctx.input();
        let delta = ctx.time().delta;

        // Mouse deltas are already per frame, so they aren't scaled by delta.
        self.yaw -= input.mouse_delta().x * self.sensitivity;
        self.yaw = self.yaw.rem_euclid(std::f32::consts::TAU);
        self.pitch += input.mouse_delta().y * self.sensitivity;
        self.pitch = self.pitch.clamp(-80f32.to_radians(), 80f32.to_radians());

        let forward = Vec3::new(self.yaw.sin(), 0.0, self.yaw.cos());
        let right = forward.cross(Vec3::Y);
        let axis =
            |pos, neg| input.is_key_down(pos) as i8 as f32 - input.is_key_down(neg) as i8 as f32;
        let dir = forward * axis(KeyCode::KeyW, KeyCode::KeyS)
            + right * axis(KeyCode::KeyD, KeyCode::KeyA)
            + Vec3::Y * axis(KeyCode::Space, KeyCode::ControlLeft);
        let speed = if input.is_key_down(KeyCode::ShiftLeft) {
            self.speed * 1.5
        } else {
            self.speed
        };

        let mut position = ctx.transform().position + dir.normalize_or_zero() * speed * delta;
        let center = ctx.resolve_circle(Circle {
            center: Vec2::new(position.x, position.z),
            radius: self.radius,
        });
        position.x = center.x;
        position.z = center.y;

        let transform = ctx.transform_mut();
        transform.position = position;
        transform.rotation = Quat::from_rotation_y(self.yaw);

        let look = self.look_direction();
        let camera = ctx.camera_mut();
        camera.eye = position;
        camera.target = position + look;
        if let Light::Spot(l) = &mut ctx.lights_mut()[0] {
            l.direction = look;
            l.position = position;
        }
    }
}
