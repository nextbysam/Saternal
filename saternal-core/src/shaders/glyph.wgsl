// GPU-based glyph rendering shader using instanced rendering

// Group 0: Glyph atlas texture
@group(0) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(0) @binding(1)
var atlas_sampler: sampler;

// Group 1: Screen uniforms
struct ScreenUniforms {
    width: f32,
    height: f32,
}

@group(1) @binding(0)
var<uniform> screen: ScreenUniforms;

// Instance data (per-glyph)
struct InstanceInput {
    @location(0) position: vec2<f32>,      // Position in NDC
    @location(1) size: vec2<f32>,          // Size in NDC
    @location(2) uv_min: vec2<f32>,        // Atlas UV min
    @location(3) uv_max: vec2<f32>,        // Atlas UV max
    @location(4) color: vec4<f32>,         // RGBA color
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

// Vertex shader - Generate quad vertices procedurally
@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var output: VertexOutput;
    
    // Generate quad vertices (two triangles)
    var local_pos: vec2<f32>;
    var local_uv: vec2<f32>;
    
    switch vertex_index {
        case 0u: {
            local_pos = vec2<f32>(0.0, 0.0);
            local_uv = vec2<f32>(0.0, 0.0);
        }
        case 1u: {
            local_pos = vec2<f32>(1.0, 0.0);
            local_uv = vec2<f32>(1.0, 0.0);
        }
        case 2u: {
            local_pos = vec2<f32>(1.0, 1.0);
            local_uv = vec2<f32>(1.0, 1.0);
        }
        case 3u: {
            local_pos = vec2<f32>(0.0, 0.0);
            local_uv = vec2<f32>(0.0, 0.0);
        }
        case 4u: {
            local_pos = vec2<f32>(1.0, 1.0);
            local_uv = vec2<f32>(1.0, 1.0);
        }
        default: {
            local_pos = vec2<f32>(0.0, 1.0);
            local_uv = vec2<f32>(0.0, 1.0);
        }
    }
    
    // Transform to instance position and size
    let world_pos = instance.position + local_pos * instance.size;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    
    // Interpolate UV coordinates in atlas
    output.uv = mix(instance.uv_min, instance.uv_max, local_uv);
    
    // Pass through color
    output.color = instance.color;
    
    return output;
}

// Fragment shader - Sample atlas and apply color
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample glyph coverage from atlas (grayscale)
    let coverage = textureSample(atlas_texture, atlas_sampler, input.uv).r;
    
    // Premultiply alpha for correct blending
    let rgb_pre = input.color.rgb * coverage;
    
    return vec4<f32>(rgb_pre, coverage);
}
