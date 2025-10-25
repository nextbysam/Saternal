// Vertex shader

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

// Fragment shader

// Group 0: Terminal content texture
@group(0) @binding(0)
var t_texture: texture_2d<f32>;

@group(0) @binding(1)
var t_sampler: sampler;

// Group 1: Wallpaper texture
@group(1) @binding(0)
var wallpaper_texture: texture_2d<f32>;

@group(1) @binding(1)
var wallpaper_sampler: sampler;

// Group 2: Opacity uniforms
struct OpacityUniforms {
    wallpaper_opacity: f32,
    background_opacity: f32,
    has_wallpaper: u32,
    _padding: f32,
}

@group(2) @binding(0)
var<uniform> opacity: OpacityUniforms;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Sample terminal content (text + background)
    let terminal_color = textureSample(t_texture, t_sampler, input.tex_coords);

    // If no wallpaper, apply background opacity and return
    if (opacity.has_wallpaper == 0u) {
        return vec4<f32>(terminal_color.rgb, terminal_color.a * opacity.background_opacity);
    }

    // Sample wallpaper texture
    let wallpaper_color = textureSample(wallpaper_texture, wallpaper_sampler, input.tex_coords);

    // Apply wallpaper opacity (dim the wallpaper)
    let wallpaper_dimmed = vec4<f32>(
        wallpaper_color.rgb * opacity.wallpaper_opacity,
        opacity.wallpaper_opacity
    );

    // Blend layers using premultiplied alpha:
    // wallpaper (bottom) → terminal content (top)
    let blended = wallpaper_dimmed * (1.0 - terminal_color.a) + terminal_color;

    // Apply overall background opacity
    return vec4<f32>(blended.rgb, blended.a * opacity.background_opacity);
}
