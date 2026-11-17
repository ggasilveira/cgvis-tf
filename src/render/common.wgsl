// Shared by every shader: prepended to them in Rust (see `shader_source`).

struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

// 3am sky gradient, in linear color: deep navy with a faint glow at the
// horizon, so tree silhouettes stand out against it.
const SKY_HORIZON: vec3<f32> = vec3(0.012, 0.016, 0.03);
const SKY_ZENITH: vec3<f32> = vec3(0.002, 0.003, 0.008);

// Fog is darkness, not haze: only the flashlight lights the scene.
const FOG_COLOR: vec3<f32> = vec3(0.0);

// Exponential-squared fog: ~45% visibility at 90m.
const FOG_DENSITY: f32 = 0.01;

fn sky_color(dir: vec3<f32>) -> vec3<f32> {
    let up = clamp(normalize(dir).y, 0.0, 1.0);
    // sqrt keeps the horizon glow in a thin band near the ground
    return mix(SKY_HORIZON, SKY_ZENITH, sqrt(up));
}

// Fades a lit surface into darkness with distance, which also hides the far
// plane cutoff (distant geometry is a black silhouette against the sky).
fn apply_fog(color: vec3<f32>, world_position: vec3<f32>) -> vec3<f32> {
    let to_surface = world_position - camera.view_pos.xyz;
    let d = length(to_surface) * FOG_DENSITY;
    let visibility = exp(-d * d);
    return mix(FOG_COLOR, color, visibility);
}
