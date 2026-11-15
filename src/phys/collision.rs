use super::hitbox::{Circle, Rect};

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
