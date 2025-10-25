// Two-pass Gaussian blur shader for terminal background
// Pass 1: Horizontal blur
// Pass 2: Vertical blur

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(input.position, 0.0, 1.0);
    output.tex_coords = input.tex_coords;
    return output;
}

// Bind group for blur shader
@group(0) @binding(0)
var t_texture: texture_2d<f32>;

@group(0) @binding(1)
var t_sampler: sampler;

// Blur uniforms
struct BlurUniforms {
    direction: vec2<f32>,  // (1, 0) for horizontal, (0, 1) for vertical
    strength: f32,          // Blur strength multiplier
    _padding: f32,
}

@group(0) @binding(2)
var<uniform> blur: BlurUniforms;

// 9-tap Gaussian blur kernel
const KERNEL_SIZE: i32 = 9;
const KERNEL_WEIGHTS: array<f32, 9> = array<f32, 9>(
    0.05, 0.09, 0.12, 0.15, 0.16, 0.15, 0.12, 0.09, 0.05
);

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_size = textureDimensions(t_texture);
    let texel_size = 1.0 / vec2<f32>(f32(tex_size.x), f32(tex_size.y));

    var result = vec4<f32>(0.0);
    let half_kernel = KERNEL_SIZE / 2;

    // Sample along blur direction
    for (var i = 0; i < KERNEL_SIZE; i++) {
        let offset = f32(i - half_kernel) * blur.strength;
        let sample_coords = input.tex_coords + blur.direction * texel_size * offset;
        let sample = textureSample(t_texture, t_sampler, sample_coords);
        result += sample * KERNEL_WEIGHTS[i];
    }

    return result;
}
