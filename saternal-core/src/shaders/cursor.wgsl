// Cursor shader for GPU-accelerated cursor rendering

struct CursorUniform {
    position: vec2<f32>,      // NDC position (-1 to 1)
    size: vec2<f32>,          // NDC size
    color: vec4<f32>,         // RGBA color
    visible: u32,             // 0 = hidden, 1 = visible
    style: u32,               // 0 = block, 1 = beam, 2 = underline
    _padding: vec2<u32>,      // Alignment
}

@group(0) @binding(0)
var<uniform> cursor: CursorUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,  // Position within cursor quad (0-1)
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    
    // Generate quad vertices using switch for WebGPU compatibility
    var local: vec2<f32>;
    switch vertex_index {
        case 0u: { local = vec2<f32>(0.0, 0.0); }  // Top-left
        case 1u: { local = vec2<f32>(1.0, 0.0); }  // Top-right
        case 2u: { local = vec2<f32>(1.0, 1.0); }  // Bottom-right
        case 3u: { local = vec2<f32>(0.0, 0.0); }  // Top-left
        case 4u: { local = vec2<f32>(1.0, 1.0); }  // Bottom-right
        default: { local = vec2<f32>(0.0, 1.0); }  // Bottom-left
    }
    
    // Size and position are pre-calculated in Rust based on cursor style
    // No need for shader-side adjustments - keeps the GPU work minimal
    let final_pos = cursor.position + local * cursor.size;
    output.position = vec4<f32>(final_pos, 0.0, 1.0);
    output.local_pos = local;
    
    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Only render if visible
    if (cursor.visible == 0u) {
        discard;
    }
    
    return cursor.color;
}
