use wgpu;

/// Create cursor render pipeline
pub fn create_cursor_pipeline(
    device: &wgpu::Device,
    bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Cursor Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/cursor.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Cursor Pipeline Layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Cursor Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
    })
}

/// Create vertex buffer for cursor (not used with instance rendering)
pub fn create_cursor_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    // Empty buffer - we generate vertices in shader
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Cursor Vertex Buffer"),
        size: 0,
        usage: wgpu::BufferUsages::VERTEX,
        mapped_at_creation: false,
    })
}
