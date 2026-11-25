// Procedural sky: one triangle covering the screen at the far plane. Drawn
// after opaque geometry, so it only shades pixels nothing else covered.
// `common.wgsl` is prepended (camera, sky_color, apply_sky_fog).

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) ndc: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VertexOutput {
    // (-1,-1), (3,-1), (-1,3): covers the [-1,1] square
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    let ndc = uv * 2.0 - 1.0;
    return VertexOutput(vec4(ndc, 1.0, 1.0), ndc);
}

// Stars live on a sphere of radius STAR_GRID, cut into unit cells; each cell
// holds at most one star.
const STAR_GRID: f32 = 250.0;
// Fraction of cells with a star.
const STAR_DENSITY: f32 = 0.01;
// Star radius, in cells. Smaller than a pixel stars are widened to a pixel
// and dimmed to match, so they don't flicker when the camera turns.
const STAR_RADIUS: f32 = 0.06;
const STAR_BRIGHTNESS: f32 = 0.3;

const MOON_DIR: vec3<f32> = vec3(-0.45, 0.35, -0.82);
// Angular radius, in radians (the real moon is ~0.0045; this reads better).
const MOON_RADIUS: f32 = 0.025;
const MOON_COLOR: vec3<f32> = vec3(0.55, 0.57, 0.6);
const MOON_GLOW: vec3<f32> = vec3(0.012, 0.014, 0.02);

// pcg3d: three uniform floats in [0, 1) per integer cell.
fn hash3(c: vec3<i32>) -> vec3<f32> {
    var v = bitcast<vec3<u32>>(c) * 1664525u + 1013904223u;
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    v ^= v >> vec3(16u);
    v.x += v.y * v.z;
    v.y += v.z * v.x;
    v.z += v.x * v.y;
    return vec3<f32>(v) / 4294967296.0;
}

fn stars(dir: vec3<f32>) -> vec3<f32> {
    let p = dir * STAR_GRID;
    // how many cells one pixel covers
    let pixel = length(fwidth(p));
    let radius = max(STAR_RADIUS, pixel);
    let dim = (STAR_RADIUS * STAR_RADIUS) / (radius * radius);

    let cell = vec3<i32>(floor(p));
    var color = vec3(0.0);
    // A star projected onto the sphere can leave its cell, so check neighbours.
    for(var x = -1; x <= 1; x++) {
        for(var y = -1; y <= 1; y++) {
            for(var z = -1; z <= 1; z++) {
                let c = cell + vec3(x, y, z);
                let h = hash3(c);
                if h.x > STAR_DENSITY {
                    continue;
                }
                let star = normalize(vec3<f32>(c) + hash3(c + 7919)) * STAR_GRID;
                let d = distance(p, star);
                // few bright stars, many faint ones
                let brightness = pow(h.y, 6.0) * 4.0 + 0.15;
                // blue-white to warm
                let tint = mix(vec3(0.75, 0.85, 1.0), vec3(1.0, 0.85, 0.7), h.z);
                color += tint * brightness * (1.0 - smoothstep(0.0, radius, d));
            }
        }
    }
    // haze near the horizon hides the faint ones
    let altitude = smoothstep(0.0, 0.25, dir.y);
    return color * dim * altitude * STAR_BRIGHTNESS;
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = vec2<i32>(floor(p));
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash3(vec3(i, 0)).x;
    let b = hash3(vec3(i + vec2(1, 0), 0)).x;
    let c = hash3(vec3(i + vec2(0, 1), 0)).x;
    let d = hash3(vec3(i + vec2(1, 1), 0)).x;
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Returns (color, coverage); coverage hides the stars behind the disc.
fn moon(dir: vec3<f32>) -> vec4<f32> {
    let center = normalize(MOON_DIR);
    // chord length ~= angle for small angles, without acos precision issues
    let d = distance(dir, center);
    let aa = fwidth(d);
    let disc = 1.0 - smoothstep(MOON_RADIUS - aa, MOON_RADIUS + aa, d);

    // position on the disc, in moon radii, for the surface pattern
    let right = normalize(cross(center, vec3(0.0, 1.0, 0.0)));
    let up = cross(right, center);
    let uv = vec2(dot(dir, right), dot(dir, up)) / MOON_RADIUS;
    let maria = value_noise(uv * 2.5) * 0.6 + value_noise(uv * 6.0) * 0.4;
    // darker toward the limb
    let limb = sqrt(max(1.0 - dot(uv, uv), 0.0));
    let surface = MOON_COLOR * mix(0.55, 1.0, maria) * mix(0.7, 1.0, limb);

    let glow = MOON_GLOW * exp(-d / (MOON_RADIUS * 3.0));
    return vec4(surface * disc + glow, disc);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // un-project the pixel onto the far plane to get its view direction
    let far = camera.inv_view_proj * vec4(in.ndc, 1.0, 1.0);
    let dir = normalize(far.xyz / far.w - camera.view_pos.xyz);
    let m = moon(dir);
    let color = sky_color(dir) + stars(dir) * (1.0 - m.a) + m.rgb;
    return vec4(apply_sky_fog(color, dir), 1.0);
}
