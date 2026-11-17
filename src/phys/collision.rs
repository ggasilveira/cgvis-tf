use super::hitbox::{Circle, Hitbox, Rect, Segment};
use glam::Vec2;

pub fn circles_overlap(a: &Circle, b: &Circle) -> bool {
    let radius_sum = a.radius + b.radius;
    a.center.distance_squared(b.center) <= radius_sum * radius_sum
}

pub fn circle_rect_overlap(circle: &Circle, rect: &Rect) -> bool {
    let rect_min = rect.center - rect.half_extents;
    let rect_max = rect.center + rect.half_extents;
    let closest_point = circle.center.clamp(rect_min, rect_max);

    circle.center.distance_squared(closest_point) <= circle.radius * circle.radius
}

pub fn rects_overlap(a: &Rect, b: &Rect) -> bool {
    let distance = (a.center - b.center).abs();
    let combined_half_extents = a.half_extents + b.half_extents;

    distance.x <= combined_half_extents.x && distance.y <= combined_half_extents.y
}

/// Smallest translation that moves `circle` out of `hitbox`, or None if they
/// don't overlap.
pub fn circle_push(circle: &Circle, hitbox: &Hitbox) -> Option<Vec2> {
    match hitbox {
        Hitbox::Circle(other) => circle_circle_push(circle, other),
        Hitbox::Rect(rect) => circle_rect_push(circle, rect),
        Hitbox::Segment(segment) => circle_segment_push(circle, segment),
    }
}

fn circle_circle_push(circle: &Circle, other: &Circle) -> Option<Vec2> {
    let radius_sum = circle.radius + other.radius;
    let offset = circle.center - other.center;
    let distance = offset.length();
    if distance >= radius_sum {
        return None;
    }
    let normal = offset.try_normalize().unwrap_or(Vec2::X);
    Some(normal * (radius_sum - distance))
}

fn circle_rect_push(circle: &Circle, rect: &Rect) -> Option<Vec2> {
    let local = circle.center - rect.center;
    let closest = local.clamp(-rect.half_extents, rect.half_extents);
    if closest != local {
        // center outside the rectangle: push away from the closest point
        let offset = local - closest;
        let distance = offset.length();
        return (distance < circle.radius).then(|| offset / distance * (circle.radius - distance));
    }
    // center inside: leave through the nearest side
    let depth = rect.half_extents - local.abs();
    let sign = Vec2::new(local.x.signum(), local.y.signum());
    Some(if depth.x < depth.y {
        Vec2::new(sign.x * (depth.x + circle.radius), 0.0)
    } else {
        Vec2::new(0.0, sign.y * (depth.y + circle.radius))
    })
}

fn circle_segment_push(circle: &Circle, segment: &Segment) -> Option<Vec2> {
    let ab = segment.b - segment.a;
    let t = ((circle.center - segment.a).dot(ab) / ab.length_squared().max(f32::EPSILON))
        .clamp(0.0, 1.0);
    let offset = circle.center - (segment.a + ab * t);
    let distance = offset.length();
    if distance >= circle.radius {
        return None;
    }
    let normal = offset.try_normalize().unwrap_or(ab.perp().normalize_or(Vec2::X));
    Some(normal * (circle.radius - distance))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(x: f32, y: f32, radius: f32) -> Circle {
        Circle {
            center: Vec2::new(x, y),
            radius,
        }
    }

    #[test]
    fn circle_pushed_out_of_circle() {
        let push = circle_push(&circle(1.5, 0.0, 1.0), &Hitbox::Circle(circle(0.0, 0.0, 1.0)));
        assert!(push.unwrap().abs_diff_eq(Vec2::new(0.5, 0.0), 1e-5));
        assert!(circle_push(&circle(3.0, 0.0, 1.0), &Hitbox::Circle(circle(0.0, 0.0, 1.0))).is_none());
    }

    #[test]
    fn circle_pushed_out_of_rect() {
        let rect = Hitbox::Rect(Rect {
            center: Vec2::ZERO,
            half_extents: Vec2::new(2.0, 1.0),
        });
        // touching the right side from outside
        let push = circle_push(&circle(2.25, 0.0, 0.5), &rect).unwrap();
        assert!(push.abs_diff_eq(Vec2::new(0.25, 0.0), 1e-5));
        // center inside, nearest side is the top one
        let push = circle_push(&circle(0.0, 0.75, 0.5), &rect).unwrap();
        assert!(push.abs_diff_eq(Vec2::new(0.0, 0.75), 1e-5));
        assert!(circle_push(&circle(3.0, 0.0, 0.5), &rect).is_none());
    }

    #[test]
    fn circle_pushed_out_of_segment() {
        let wall = Hitbox::Segment(Segment {
            a: Vec2::new(-1.0, 0.0),
            b: Vec2::new(1.0, 0.0),
        });
        let push = circle_push(&circle(0.0, -0.2, 0.5), &wall).unwrap();
        assert!(push.abs_diff_eq(Vec2::new(0.0, -0.3), 1e-5));
        // past the end of the wall
        assert!(circle_push(&circle(1.6, 0.0, 0.5), &wall).is_none());
    }
}
