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
    // 2.5px..61.5px content centers in a 64px cell.
    let cell_origin_px = vec2<f32>(tile_x, tile_y) * 64.0;
    let content_min = (cell_origin_px + vec2<f32>(2.5)) / 1024.0;
    let content_max = (cell_origin_px + vec2<f32>(61.5)) / 1024.0;
    return content_min + repeated * (content_max - content_min);
}

@fragment
fn fragment(vertex_output: VertexOutput) -> @location(0) vec4<f32> {
    var local_uv = vec2<f32>(0.5, 0.5);
#ifdef VERTEX_UVS_A
    local_uv = vertex_output.uv;
#endif
    let uv = tile_content_uv(local_uv, material.tile_index);
    // The base atlas is tagged sRGB by ImageLoaderSettings; Bevy's texture
    // format performs the sRGB -> linear decode. There is no manual sRGB
    // conversion here.
    let sampled = textureSample(color_atlas, color_sampler, uv).rgb;
    var rgb = sampled * material.tint.rgb;

    // Keep the fallback/not-ready state visible to diagnostics without a
    // shader variant.  The magenta tint is deliberately restrained so the
    // reference path remains the authoritative fallback.
    if ((material.flags & 32u) != 0u) {
        rgb = mix(vec3<f32>(0.8, 0.2, 0.8), rgb, 0.35);
    }
    // Base-only is the sampled atlas tile before profile tint. Final is the
    // composed base plus the profile emissive term; no surface alpha or
    // emissive is recomposed in the shader.
    if ((material.flags & 64u) != 0u) {
        rgb = sampled;
    }
    if ((material.flags & 128u) != 0u) {
        rgb = rgb + material.emissive_alpha.rgb;
    }
    return vec4<f32>(rgb, material.tint.a);
}
