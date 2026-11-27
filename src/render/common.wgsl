// Shared by every shader: prepended to them in Rust (see `shader_source`).

struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

const LIGHT_DIR: u32 = 0;
const LIGHT_POINT: u32 = 1;
const LIGHT_SPOT: u32 = 2;
struct Light {
    // 16 bytes
    position: vec3<f32>,
    typ: u32,

    // 16 bytes
    direction: vec3<f32>,
    padding0: u32,

    // 16 bytes
    color: vec3<f32>,
    intensity: f32,

    // 16 bytes
    inner_angle: f32,
    outer_angle: f32,
    padding1: u32,
    padding2: u32,
}
const LIGHTS_MAX: u32 = 16; // uniform::LIGHTS_SIZE
struct LightsUniform {
    lights: array<Light, LIGHTS_MAX>,
    size: u32,
}

@group(0) @binding(1)
var<uniform> lights: LightsUniform;

// Direction towards the light and how much of it reaches `pos` (attenuation
// and spot cone), for a light of any type.
struct LightSample {
    dir: vec3<f32>,
    radiance: vec3<f32>,
}

fn sample_light(light: Light, pos: vec3<f32>) -> LightSample {
    var out: LightSample;
    if light.typ == LIGHT_DIR {
        out.dir = normalize(-light.direction);
        out.radiance = light.color * light.intensity;
        return out;
    }
    let to_light = light.position - pos;
    let dist2 = max(dot(to_light, to_light), 1e-4);
    out.dir = to_light * inverseSqrt(dist2);
    // inverse square falloff
    var attenuation = light.intensity / pow(dist2, 0.6);
    if light.typ == LIGHT_SPOT {
        // angles are half-angles of the cone, in radians
        let cos_angle = dot(-out.dir, normalize(light.direction));
        attenuation *= smoothstep(cos(light.outer_angle), cos(light.inner_angle), cos_angle);
    }
    out.radiance = light.color * attenuation;
    return out;
}

// 3am sky gradient, in linear color: deep navy with a faint glow at the
// horizon, so tree silhouettes stand out against it.
const SKY_HORIZON: vec3<f32> = vec3(0.009, 0.0078, 0.0066);
const SKY_ZENITH: vec3<f32> = vec3(0.0045, 0.0039, 0.0033);

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

// The fog is a ground layer this many meters thick: it swallows the sky at the
// horizon, but looking up the view ray leaves it quickly and the sky shows.
const FOG_HEIGHT: f32 = 20.0;

// Fog for the sky along view direction `dir`: the distance the ray travels
// inside the fog layer, with the same falloff as `apply_fog`.
fn apply_sky_fog(color: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let up = max(normalize(dir).y, 1e-4);
    let d = FOG_HEIGHT / up * FOG_DENSITY;
    let visibility = exp(-d * d);
    return mix(FOG_COLOR, color, visibility);
}
