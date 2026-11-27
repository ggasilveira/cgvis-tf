use crate::engine::prelude::*;

pub struct Door {
    pub open_angle: f32, // radians
    pub speed: f32,      // radians per second
    open: bool,
    angle: f32,
    closed_rotation: Quat,
    was_pressed: bool,
}
impl Door {
    pub fn new(open_angle_deg: f32) -> Self {
        Self {
            open_angle: open_angle_deg.to_radians(),
            speed: std::f32::consts::PI / 1.5,
            open: false,
            angle: 0.0,
            closed_rotation: Quat::IDENTITY,
            was_pressed: false,
        }
    }
}

impl Behaviour for Door {
    fn start(&mut self, ctx: &mut Context) {
        self.closed_rotation = ctx.transform().rotation;
    }
    fn update(&mut self, ctx: &mut Context) {
        let pressed = ctx.input().is_key_down(KeyCode::KeyE);
        if pressed && !self.was_pressed {
            let hinge = ctx.transform().position;
            let near = ctx
                .find("player")
                .and_then(|p| ctx.object(p))
                .is_some_and(|p| p.transform.position.distance(hinge) < 6.0);
            if near {
                self.open = !self.open;
            }
        }
        self.was_pressed = pressed;

        let target = if self.open { self.open_angle } else { 0.0 };
        let step = self.speed * ctx.time().delta;
        self.angle += (target - self.angle).clamp(-step, step);
        ctx.transform_mut().rotation = self.closed_rotation * Quat::from_rotation_y(self.angle);
    }
}
