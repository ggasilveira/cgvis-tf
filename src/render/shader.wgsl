struct Camera {
    view_pos: vec4<f32>,
    view_proj: mat4x4<f32>,
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
}
struct InstanceData {
    model: mat4x4<f32>,
    normal: mat3x3<f32>,
}
struct PointLight {
    position: vec3<f32>,
    color: vec3<f32>,
}
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) tangent_position: vec3<f32>,
    @location(2) tangent_light_position: vec3<f32>,
    @location(3) tangent_view_position: vec3<f32>,
}

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
struct LightsUniform {
    lights: array<Light, 16>,
    size: u32,
}
struct MaterialUniform {
    base_color: vec4<f32>,
    metallic: f32,
    roughness: f32,
    normal_scale: f32,
    alpha_cutoff: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

@group(0) @binding(1)
var<uniform> lights: LightsUniform;

@group(2) @binding(0)
var<storage, read> instances: array<InstanceData>;

@vertex
fn vs_main(v: VertexInput, @builtin(instance_index) i: u32) -> VertexOutput {
    var out: VertexOutput;
    let inst = instances[i];
    let world_normal = normalize(inst.normal * v.normal);
    let world_tangent = normalize(inst.normal * v.tangent);
    let world_bitangent = normalize(inst.normal * v.bitangent);
    let tangent_matrix = transpose(
            mat3x3<f32>(
                world_tangent,
                world_bitangent,
                world_normal,
            ),
        );

    let world_position = inst.model * vec4<f32>(v.position, 1.0);

    out.clip_position = camera.view_proj * world_position;
    out.tex_coords = v.tex_coords;
    out.tangent_position = tangent_matrix * world_position.xyz;
    out.tangent_view_position = tangent_matrix * camera.view_pos.xyz;
    out.tangent_light_position = tangent_matrix * lights.lights[0].position;
    return out;
}

@group(1) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(1) @binding(1)
var s_diffuse: sampler;
@group(1) @binding(2)
var t_normal: texture_2d<f32>;
@group(1) @binding(3)
var s_normal: sampler;
@group(1) @binding(4)
var<uniform> mat_settings: MaterialUniform;

fn shade(in: VertexOutput) -> vec4<f32> {
    let object_color: vec4<f32> = textureSample(t_diffuse, s_diffuse, in.tex_coords);
    let object_normal: vec4<f32> = textureSample(t_normal, s_normal, in.tex_coords);

    let ambient_strength = 0.5;
    let light = lights.lights[0];
    let ambient_color = vec3(1, 1, 1) * ambient_strength;

    // create the lightning vectors
    let tangent_normal = object_normal.xyz * 2.0 - 1.0;
    let light_dir = normalize(in.tangent_light_position - in.tangent_position);
    let view_dir = normalize(in.tangent_view_position - in.tangent_position);
    let half_dir = normalize(view_dir + light_dir);

    let specular_strength = pow(max(dot(tangent_normal, half_dir), 0.0), 32.0);
    let specular_color = light.color * specular_strength;

    let diffuse_strength = max(dot(tangent_normal, light_dir), 0.0);
    let diffuse_color = light.color * diffuse_strength;

    let result = (ambient_color + diffuse_color + specular_color) * object_color.xyz;
    return vec4<f32>(result, object_color.a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade(in);
}

@fragment
fn fs_mask(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = shade(in);
    if color.a < mat_settings.alpha_cutoff {
        discard;
    }
    return vec4<f32>(color.rgb, 1.0);
}
