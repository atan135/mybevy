// Minimal Fangyuan atlas binding shader.
// Stage 4 only establishes a supported Material/AsBindGroup contract.  PBR
// lighting, normal reconstruction and noise modulation are intentionally left
// for the later stages.
#import bevy_pbr::forward_io::VertexOutput

struct FangyuanAtlasMaterialUniform {
    tint: vec4<f32>,
    surface: vec4<f32>,
    pbr: vec4<f32>,
    emissive_alpha: vec4<f32>,
    tile_index: u32,
    noise_tile_index: u32,
    flags: u32,
    padding: u32,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> material: FangyuanAtlasMaterialUniform;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var color_atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var color_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var normal_atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(5) var noise_atlas: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(6) var noise_sampler: sampler;

fn tile_content_uv(local_uv: vec2<f32>, tile_index: u32) -> vec2<f32> {
    let tile_x = f32(tile_index % 16u);
    let tile_y = f32(tile_index / 16u);
    let repeated = fract(local_uv * material.surface.xy);
    let cell_min = vec2<f32>(tile_x, tile_y) / 16.0;
    let cell_size = vec2<f32>(1.0 / 16.0);
    // 2.5px..61.5px content centers in a 64px cell.
    let content_min = cell_min + cell_size * (2.5 / 64.0);
    let content_size = cell_size * (59.0 / 64.0);
    return content_min + repeated * content_size;
}

@fragment
fn fragment(vertex_output: VertexOutput) -> @location(0) vec4<f32> {
    var local_uv = vec2<f32>(0.5, 0.5);
#ifdef VERTEX_UVS_A
    local_uv = vertex_output.uv;
#endif
    let uv = tile_content_uv(local_uv, material.tile_index);
    let sampled = textureSample(color_atlas, color_sampler, uv).rgb;
    var rgb = sampled * material.tint.rgb;

    // Keep the fallback/not-ready state visible to diagnostics without a
    // shader variant.  The magenta tint is deliberately restrained so the
    // reference path remains the authoritative fallback.
    if ((material.flags & 32u) != 0u) {
        rgb = mix(vec3<f32>(0.8, 0.2, 0.8), rgb, 0.35);
    }
    if ((material.flags & 64u) != 0u) {
        rgb = sampled;
    }
    return vec4<f32>(rgb, material.tint.a);
}
