// Procedural sky: one triangle covering the screen at the far plane. Drawn
// after opaque geometry, so it only shades pixels nothing else covered.
// `common.wgsl` is prepended (camera, sky_color).

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // un-project the pixel onto the far plane to get its view direction
    let far = camera.inv_view_proj * vec4(in.ndc, 1.0, 1.0);
    let dir = far.xyz / far.w - camera.view_pos.xyz;
    return vec4(sky_color(dir), 1.0);
}
