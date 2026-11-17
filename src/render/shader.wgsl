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
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_position: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
    @location(3) world_tangent: vec3<f32>,
    @location(4) world_bitangent: vec3<f32>,
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
const LIGHTS_MAX: u32 = 16; // uniform::LIGHTS_SIZE
struct LightsUniform {
    lights: array<Light, LIGHTS_MAX>,
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

@group(1) @binding(0)
var<storage, read> instances: array<InstanceData>;

@vertex
fn vs_main(v: VertexInput, @builtin(instance_index) i: u32) -> VertexOutput {
    var out: VertexOutput;
    let inst = instances[i];
    let world_position = inst.model * vec4<f32>(v.position, 1.0);

    // Lighting happens in world space in the fragment shader, so every light
    // can be evaluated there (a tangent-space light only fits one light).
    out.clip_position = camera.view_proj * world_position;
    out.tex_coords = v.tex_coords;
    out.world_position = world_position.xyz;
    out.world_normal = inst.normal * v.normal;
    out.world_tangent = inst.normal * v.tangent;
    out.world_bitangent = inst.normal * v.bitangent;
    return out;
}

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;
@group(2) @binding(2)
var t_normal: texture_2d<f32>;
@group(2) @binding(3)
var s_normal: sampler;
@group(2) @binding(4)
var<uniform> mat_settings: MaterialUniform;

const AMBIENT: vec3<f32> = vec3(0.5, 0.5, 0.5);

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
    var attenuation = light.intensity / dist2;
    if light.typ == LIGHT_SPOT {
        // angles are half-angles of the cone, in radians
        let cos_angle = dot(-out.dir, normalize(light.direction));
        attenuation *= smoothstep(cos(light.outer_angle), cos(light.inner_angle), cos_angle);
    }
    out.radiance = light.color * attenuation;
    return out;
}

// World-space normal from the vertex frame and the normal map.
fn surface_normal(in: VertexOutput, front_facing: bool) -> vec3<f32> {
    var tangent_normal = textureSample(t_normal, s_normal, in.tex_coords).xyz * 2.0 - 1.0;
    tangent_normal = vec3(tangent_normal.xy * mat_settings.normal_scale, tangent_normal.z);
    let tbn = mat3x3<f32>(
        normalize(in.world_tangent),
        normalize(in.world_bitangent),
        normalize(in.world_normal),
    );
    let n = normalize(tbn * tangent_normal);
    // back faces of double-sided materials (leaves) face the other way
    return select(-n, n, front_facing);
}

// Blinn-Phong driven by the glTF metallic/roughness parameters.
fn shade(in: VertexOutput, front_facing: bool) -> vec4<f32> {
    let albedo = textureSample(t_diffuse, s_diffuse, in.tex_coords) * mat_settings.base_color;
    let n = surface_normal(in, front_facing);
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);

    let metallic = clamp(mat_settings.metallic, 0.0, 1.0);
    let roughness = clamp(mat_settings.roughness, 0.05, 1.0);
    // Metals have no diffuse and tint their reflections; dielectrics reflect ~4%.
    let diffuse_color = albedo.rgb * (1.0 - metallic);
    let specular_color = mix(vec3(0.04), albedo.rgb, metallic);
    // Rough surfaces get a wide, dim highlight; smooth ones a tight, bright one.
    let shininess = 2.0 / (roughness * roughness * roughness * roughness) - 2.0;
    let specular_norm = (shininess + 8.0) / 8.0;

    var color = AMBIENT * albedo.rgb;
    for (var i = 0u; i < min(lights.size, LIGHTS_MAX); i++) {
        let light = sample_light(lights.lights[i], in.world_position);
        let n_dot_l = max(dot(n, light.dir), 0.0);
        let half_dir = normalize(view_dir + light.dir);
        let specular = specular_color * specular_norm * pow(max(dot(n, half_dir), 0.0), shininess);
        color += (diffuse_color + specular) * light.radiance * n_dot_l;
    }
    return vec4<f32>(color, albedo.a);
}

@fragment
fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    return shade(in, front_facing);
}

@fragment
fn fs_mask(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    let color = shade(in, front_facing);
    if color.a < mat_settings.alpha_cutoff {
        discard;
    }
    return vec4<f32>(color.rgb, 1.0);
}
