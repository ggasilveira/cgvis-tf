use super::hitbox::{Circle, Hitbox, Rect, Segment};
use glam::{Mat4, Quat, Vec2, Vec3};
use std::sync::Arc;

/// A collider attached to a game object, in the object's local space.
/// Collision is 2D, on the xz plane: heights are ignored.
#[derive(Debug, Clone)]
pub enum Collider {
    Shape {
        shape: Shape,
        offset: Vec3,
        rotation: Quat,
    },
    /// Wall segments taken from a model's `COL_` nodes, shared between instances.
    Walls(Arc<[[Vec3; 2]]>),
}

#[derive(Debug, Clone, Copy)]
pub enum Shape {
    Box { half_extents: Vec3 },
    Sphere { radius: f32 },
    Capsule { radius: f32, height: f32 },
    Cylinder { radius: f32, height: f32 },
}

impl Collider {
    pub fn new(shape: Shape) -> Self {
        Self::Shape {
            shape,
            offset: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }

    /// Appends this collider's world-space hitboxes for an object at `world`.
    pub fn hitboxes(&self, world: Mat4, out: &mut Vec<Hitbox>) {
        let flat = |p: Vec3| Vec2::new(p.x, p.z);
        match self {
            Collider::Shape {
                shape,
                offset,
                rotation,
            } => {
                let local = world * Mat4::from_rotation_translation(*rotation, *offset);
                match *shape {
                    Shape::Box { half_extents } => {
                        // Rects are axis aligned: use the bounds of the rotated box.
                        let mut min = Vec2::INFINITY;
                        let mut max = Vec2::NEG_INFINITY;
                        for i in 0..8 {
                            let sign = Vec3::new(
                                if i & 1 == 0 { -1.0 } else { 1.0 },
                                if i & 2 == 0 { -1.0 } else { 1.0 },
                                if i & 4 == 0 { -1.0 } else { 1.0 },
                            );
                            let p = flat(local.transform_point3(sign * half_extents));
                            min = min.min(p);
                            max = max.max(p);
                        }
                        out.push(Hitbox::Rect(Rect {
                            center: (min + max) / 2.0,
                            half_extents: (max - min) / 2.0,
                        }));
                    }
                    Shape::Sphere { radius }
                    | Shape::Capsule { radius, .. }
                    | Shape::Cylinder { radius, .. } => {
                        let scale = local.x_axis.length().max(local.z_axis.length());
                        out.push(Hitbox::Circle(Circle {
                            center: flat(local.w_axis.truncate()),
                            radius: radius * scale,
                        }));
                    }
                }
            }
            Collider::Walls(walls) => {
                out.extend(walls.iter().map(|[a, b]| {
                    Hitbox::Segment(Segment {
                        a: flat(world.transform_point3(*a)),
                        b: flat(world.transform_point3(*b)),
                    })
                }));
            }
        }
    }
}
