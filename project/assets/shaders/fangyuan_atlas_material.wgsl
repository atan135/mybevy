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
    noise_uv_scale: f32,
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

fn noise_tile_is_valid(tile_index: u32) -> bool {
    if (tile_index == 0xffffffffu || tile_index >= 256u) {
        return false;
    }
    let x = tile_index % 16u;
    let y = tile_index / 16u;
    // 122 catalog cells: rows 0..5 x 0..11, rows 6..10 x 0..9.
    return (y <= 5u && x <= 11u) || (y >= 6u && y <= 10u && x <= 9u);
}

fn decode_normal_sample(sample: vec3<f32>, strength: f32, flip_y: bool) -> vec3<f32> {
    var tangent_normal = sample * 2.0 - vec3<f32>(1.0);
    if (flip_y) {
        tangent_normal.y = -tangent_normal.y;
    }
    let controlled_strength = clamp(strength, 0.0, 1.0);
    if (controlled_strength <= 0.0) {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let blended = mix(vec3<f32>(0.0, 0.0, 1.0), tangent_normal, controlled_strength);
    if (dot(blended, blended) <= 1e-8) {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    return normalize(blended);
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
    var controlled_roughness = clamp(material.surface.z, 0.0, 1.0);
    var tangent_normal = vec3<f32>(0.0, 0.0, 1.0);

    if ((material.flags & 1u) != 0u) {
        // Normal atlas data is linear and uses the same surface tile/local UV.
        let normal_sample = textureSample(normal_atlas, normal_sampler, uv).rgb;
        tangent_normal = decode_normal_sample(
            normal_sample,
            material.pbr.z,
            (material.flags & 8u) != 0u,
        );
        // This fragment skeleton does not yet have the full Bevy PBR TBN
        // integration. Keep the decoded normal in shader state and expose it
        // only through the explicit normal-only debug mode below.
    }

    var noise_value = 0.5;
    if ((material.flags & 2u) != 0u && noise_tile_is_valid(material.noise_tile_index)) {
        let noise_uv = tile_content_uv(local_uv * material.noise_uv_scale, material.noise_tile_index);
        // Noise atlas data is linear. It is visual-only and only modulates the
        // already-resolved roughness constant.
        noise_value = textureSample(noise_atlas, noise_sampler, noise_uv).r;
        let bipolar_noise = noise_value * 2.0 - 1.0;
        controlled_roughness = clamp(
            controlled_roughness + bipolar_noise * clamp(material.pbr.w, 0.0, 1.0),
            0.0,
            1.0,
        );
        // The resolved roughness value is ready for the future PBR input. No
        // default base-color change is made by noise in this skeleton.
    }

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
    if ((material.flags & 256u) != 0u) {
        rgb = tangent_normal * 0.5 + vec3<f32>(0.5);
    }
    if ((material.flags & 512u) != 0u) {
        rgb = vec3<f32>(noise_value);
    }
    return vec4<f32>(rgb, material.tint.a);
}
