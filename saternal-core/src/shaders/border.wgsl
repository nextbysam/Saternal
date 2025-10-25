// Pane border shader for GPU-accelerated border rendering
// Renders colored borders around terminal panes with per-pane focus state

struct BorderRect {
    position: vec2<f32>,      // NDC position (top-left corner)
    size: vec2<f32>,          // NDC size (width, height)
}

struct BorderUniform {
    rects: array<BorderRect, 32>,      // Support up to 32 border rectangles (512 bytes)
    count: u32,                         // Number of active borders (4 bytes)
    thickness: f32,                     // Border thickness in pixels (4 bytes)
    _padding1: vec2<u32>,               // Padding to 16-byte boundary (8 bytes)
    active_color: vec4<f32>,            // RGBA color for focused pane (16 bytes)
    inactive_color: vec4<f32>,          // RGBA color for unfocused panes (16 bytes)
    viewport_ids: array<u32, 32>,       // Pane IDs for each rect (128 bytes)
    focused_id: u32,                    // ID of focused pane (4 bytes)
    _padding2: vec3<u32>,               // Final padding (12 bytes)
}

@group(0) @binding(0)
var<uniform> borders: BorderUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32
) -> VertexOutput {
    var output: VertexOutput;

    // Skip if instance is beyond active border count
    if (instance_index >= borders.count) {
        output.position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        output.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return output;
    }

    // Get the border rect for this instance
    let rect = borders.rects[instance_index];
    let pane_id = borders.viewport_ids[instance_index];

    // Determine color based on focus state
    let is_focused = (pane_id == borders.focused_id);
    let border_color = select(borders.inactive_color, borders.active_color, is_focused);

    // Generate quad vertices (6 vertices per quad for 4 border segments)
    // Each border is drawn as 4 separate rectangles (top, bottom, left, right)
    var local: vec2<f32>;
    let vertex_in_quad = vertex_index % 6u;
    switch vertex_in_quad {
        case 0u: { local = vec2<f32>(0.0, 0.0); }  // Top-left
        case 1u: { local = vec2<f32>(1.0, 0.0); }  // Top-right
        case 2u: { local = vec2<f32>(1.0, 1.0); }  // Bottom-right
        case 3u: { local = vec2<f32>(0.0, 0.0); }  // Top-left
        case 4u: { local = vec2<f32>(1.0, 1.0); }  // Bottom-right
        default: { local = vec2<f32>(0.0, 1.0); }  // Bottom-left
    }

    let final_pos = rect.position + local * rect.size;
    output.position = vec4<f32>(final_pos, 0.0, 1.0);
    output.color = border_color;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
