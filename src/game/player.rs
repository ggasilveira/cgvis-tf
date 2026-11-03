use crate::engine::Context;
use glam::prelude::*;
use winit::keyboard::KeyCode;

pub struct Player {
    position: Vec3,
    pitch: f32,
    yaw: f32,
    speed: f32,
}
impl Player {
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            pitch: 0.0,
            yaw: 0.0,
            speed: 1.0,
        }
    }
    pub fn look_direction(&self) -> Vec3 {
        Mat3::from_rotation_y(self.yaw) * Mat3::from_rotation_x(self.pitch) * Vec3::Z
    }
    pub fn update(&mut self, ctx: &Context, delta: f32) {
        let input = ctx.input;
        let mut dir = Vec2::ZERO;
        if input.is_key_down(KeyCode::KeyW) {
            dir.y += 1.;
        }
        if input.is_key_down(KeyCode::KeyA) {
            dir.x -= 1.;
        }
        if input.is_key_down(KeyCode::KeyS) {
            dir.y -= 1.;
        }
        if input.is_key_down(KeyCode::KeyD) {
            dir.x += 1.;
        }
        if input.is_key_down(KeyCode::Space) {
            self.position.y += delta;
        }
        if input.is_key_down(KeyCode::ControlLeft) {
            self.position.y -= delta;
        }
        if input.is_key_down(KeyCode::ShiftLeft) {
            self.speed = 1.5;
        } else {
            self.speed = 1.0;
        }
        let forward2d = Vec2::Y.rotate_angle(self.yaw);
        let right2d = forward2d.rotate_angle(-90.0f32.to_radians());
        let dir = (forward2d * dir.y + right2d * dir.x).normalize();
        let mut pos2d = self.position.xy();
        pos2d += dir * delta * self.speed;

        self.position.x = pos2d.x;
        self.position.z = pos2d.y;
        self.yaw += input.mouse_delta().x * delta;
        while self.yaw > std::f32::consts::PI * 2.0 {
            self.yaw -= std::f32::consts::PI * 2.0;
        }
        self.pitch += input.mouse_delta().y * delta;
        self.pitch = self.pitch.clamp(-80f32.to_radians(), 80f32.to_radians());
    }
}
