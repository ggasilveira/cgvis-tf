// Grass blades built entirely in the vertex shader: no vertex buffer, each
// instance is one blade and each vertex a point on it.
// `common.wgsl` is prepended: camera, lights, sample_light, apply_fog.
//
// Instance `i` is blade `i % blades_per_cell` of cell `i / blades_per_cell`.
// A blade's position comes from a hash of its cell's grid coordinates and its
// index, so blades stay put as cells come and go around the camera.

struct GrassUniform {
    // world xz of heightmap texel (0, 0), and meters per texel
    height_origin: vec2<f32>,
    height_texel: f32,
    time: f32,
    wind_dir: vec2<f32>,
    cell_size: f32,
    // blades thin out with distance and are gone at this radius
    radius: f32,
    blades_per_cell: u32,
    _pad0: u32,
    // world xz size of the area the mask covers, starting at height_origin
    mask_size: vec2<f32>,
}

@group(1) @binding(0)
var<uniform> grass: GrassUniform;
// world xz of the min corner of each cell drawn this frame
@group(1) @binding(1)
var<storage, read> cells: array<vec2<f32>>;
// ground height per texel; HEIGHT_NONE where there's no ground
@group(1) @binding(2)
var heightmap: texture_2d<f32>;
// grass density painted from above: 1 full, 0 none
@group(1) @binding(3)
var mask: texture_2d<f32>;
@group(1) @binding(4)
var mask_sampler: sampler;

const SEGMENTS: u32 = 4u; // grass::SEGMENTS: a strip of 2 * SEGMENTS + 1 vertices
const HEIGHT_NONE: f32 = -1e30; // grass::HEIGHT_NONE

const BLADE_MIN_HEIGHT: f32 = 0.5;
const BLADE_MAX_HEIGHT: f32 = 1.1; // grass::BLADE_MAX_HEIGHT
const BLADE_WIDTH: f32 = 0.07;
// tip displacement as a fraction of the blade's height, at full gust
const WIND_STRENGTH: f32 = 0.35;

const BASE_COLOR: vec3<f32> = vec3(0.02, 0.05, 0.01);
const TIP_COLOR: vec3<f32> = vec3(0.12, 0.17, 0.04);
const DRY_COLOR: vec3<f32> = vec3(0.16, 0.14, 0.05);

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
}

// pcg hash: a well mixed u32 per input
fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Uniform float in [0, 1); advances the seed.
fn rand(seed: ptr<function, u32>) -> f32 {
    *seed = pcg(*seed);
    return f32(*seed >> 8u) / 16777216.0;
}

// Fraction of blades kept at `dist` from the camera (grass::density).
fn density(dist: f32) -> f32 {
    return 1.0 - smoothstep(0.2 * grass.radius, grass.radius, dist);
}

// Bilinear ground height at `xz`; HEIGHT_NONE off the heightmap or where a
// sample has no ground. R32Float isn't filterable, so this filters by hand.
fn ground_height(xz: vec2<f32>) -> f32 {
    let size = vec2<i32>(textureDimensions(heightmap));
    let p = (xz - grass.height_origin) / grass.height_texel;
    let i = vec2<i32>(floor(p));
    if any(i < vec2(0)) || any(i + 1 >= size) {
        return HEIGHT_NONE;
    }
    let f = p - floor(p);
    let h00 = textureLoad(heightmap, i, 0).r;
    let h10 = textureLoad(heightmap, i + vec2(1, 0), 0).r;
    let h01 = textureLoad(heightmap, i + vec2(0, 1), 0).r;
    let h11 = textureLoad(heightmap, i + vec2(1, 1), 0).r;
    if min(min(h00, h10), min(h01, h11)) <= HEIGHT_NONE {
        return HEIGHT_NONE;
    }
    return mix(mix(h00, h10, f.x), mix(h01, h11, f.x), f.y);
}

@vertex
fn vs_main(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> VertexOutput {
    var out: VertexOutput;
    let cell = cells[i / grass.blades_per_cell];
    let blade = i % grass.blades_per_cell;
    let cell_id = vec2<i32>(round(cell / grass.cell_size));
    var seed = pcg(bitcast<u32>(cell_id.x) ^ pcg(bitcast<u32>(cell_id.y) ^ pcg(blade)));

    let root = cell + vec2(rand(&seed), rand(&seed)) * grass.cell_size;
    let ground = ground_height(root);
    // Blades keep their index order as the "rank" for thinning: a cell drawn
    // with fewer instances drops exactly the blades this test would drop.
    let rank = (f32(blade) + 0.5) / f32(grass.blades_per_cell);
    // vertex shaders have no derivatives to pick a mip: sample level 0
    let painted = textureSampleLevel(mask, mask_sampler, (root - grass.height_origin) / grass.mask_size, 0.0).r;
    let keep = density(distance(root, camera.view_pos.xz)) * painted - rank;
    if ground <= HEIGHT_NONE || keep <= 0.0 {
        // every vertex at the same point: zero-area triangles, dropped
        // before rasterization
        out.clip_position = vec4(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    // Shrink blades close to being thinned out, instead of popping. Along a
    // mask's edge this also makes the grass get shorter towards the path.
    let height = mix(BLADE_MIN_HEIGHT, BLADE_MAX_HEIGHT, rand(&seed)) * saturate(keep * 20.0);

    let yaw = rand(&seed) * 6.2831853;
    // across the blade, and the direction it leans to
    let across = vec2(cos(yaw), sin(yaw));
    let facing = vec2(-across.y, across.x);
    let lean = mix(0.1, 0.45, rand(&seed));

    // Gusts travel along the wind direction; each blade is slightly out of phase.
    let phase = dot(root, grass.wind_dir) * 0.2 - grass.time * 1.6 + rand(&seed) * 0.8;
    let gust = 0.55 + 0.3 * sin(phase) + 0.15 * sin(phase * 2.7 + 1.3);

    // Strip: vertex pairs up the blade, then the tip (width 0, so no special case).
    let t = f32(v / 2u) / f32(SEGMENTS);
    let side = select(-0.5, 0.5, (v & 1u) == 1u);
    let width = BLADE_WIDTH * (1.0 - t);
    // quadratic bend: the base stays planted, the tip moves the most
    let bend = (facing * lean + grass.wind_dir * gust * WIND_STRENGTH) * t * t * height;
    let xz = root + across * side * width + bend;
    let world_position = vec3(xz.x, ground + t * height, xz.y);

    out.clip_position = camera.view_proj * vec4(world_position, 1.0);
    out.world_position = world_position;
    // Facing normal tilted up: blades read as a soft surface instead of
    // flashing as each one turns edge-on to the light.
    out.world_normal = vec3(facing.x, 1.2, facing.y);
    let tip_color = mix(TIP_COLOR, DRY_COLOR, rand(&seed) * 0.6);
    // darker at the base: the blades shade each other there
    out.color = mix(BASE_COLOR, tip_color, t) * mix(0.4, 1.0, t);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);
    var n = normalize(in.world_normal);
    // double-sided: light the side facing the camera
    n = select(n, -n, dot(n.xz, view_dir.xz) < 0.0);

    var color = vec3(0.0);
    for(var i = 0u; i < min(lights.size, LIGHTS_MAX); i++) {
        let light = sample_light(lights.lights[i], in.world_position);
        // wrapped diffuse: thin blades pass some light through
        let diffuse = max((dot(n, light.dir) + 0.5) / 1.5, 0.0);
        color += in.color * light.radiance * diffuse;
    }
    return vec4(apply_fog(color, in.world_position), 1.0);
}
