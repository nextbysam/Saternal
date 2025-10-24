// Selection highlight shader for GPU-accelerated text selection rendering

struct SelectionSpan {
    position: vec2<f32>,      // NDC position
    size: vec2<f32>,          // NDC size
}

struct SelectionUniform {
    spans: array<SelectionSpan, 64>,  // Support up to 64 highlight spans
    count: u32,                         // Number of active spans
    color: vec4<f32>,                   // RGBA highlight color
    _padding: vec3<u32>,                // Alignment
}

@group(0) @binding(0)
var<uniform> selection: SelectionUniform;

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
    
    // Skip if instance is beyond active span count
    if (instance_index >= selection.count) {
        output.position = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        output.color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        return output;
    }
    
    // Get the span for this instance
    let span = selection.spans[instance_index];
    
    // Generate quad vertices (6 vertices per quad)
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
    
    let final_pos = span.position + local * span.size;
    output.position = vec4<f32>(final_pos, 0.0, 1.0);
    output.color = selection.color;
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return input.color;
}
