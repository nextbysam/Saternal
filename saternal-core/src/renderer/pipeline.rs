use wgpu;
use wgpu::util::DeviceExt;

/// Create the render pipeline for text rendering with wallpaper support
pub(crate) fn create_render_pipeline(
    device: &wgpu::Device,
    terminal_bind_group_layout: &wgpu::BindGroupLayout,
    wallpaper_bind_group_layout: &wgpu::BindGroupLayout,
    opacity_bind_group_layout: &wgpu::BindGroupLayout,
    surface_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Text Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/text.wgsl").into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Render Pipeline Layout"),
        bind_group_layouts: &[
            terminal_bind_group_layout,  // @group(0) - Terminal texture
            wallpaper_bind_group_layout, // @group(1) - Wallpaper texture
            opacity_bind_group_layout,   // @group(2) - Opacity uniforms
        ],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 16, // 2 floats (pos) + 2 floats (tex) = 16 bytes
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                    wgpu::VertexAttribute {
                        offset: 8,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x2,
                    },
                ],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
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

/// Vertex structure for fullscreen quad
#[repr(C)]
#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

/// Create vertex buffer with fullscreen quad
pub(crate) fn create_vertex_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    let vertices = [
        // Top-left triangle
        Vertex { position: [-1.0, 1.0], tex_coords: [0.0, 0.0] },
        Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
        Vertex { position: [1.0, -1.0], tex_coords: [1.0, 1.0] },
        // Bottom-right triangle
        Vertex { position: [-1.0, 1.0], tex_coords: [0.0, 0.0] },
        Vertex { position: [1.0, -1.0], tex_coords: [1.0, 1.0] },
        Vertex { position: [1.0, 1.0], tex_coords: [1.0, 0.0] },
    ];

    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Vertex Buffer"),
        contents: unsafe {
            std::slice::from_raw_parts(
                vertices.as_ptr() as *const u8,
                std::mem::size_of_val(&vertices),
            )
        },
        usage: wgpu::BufferUsages::VERTEX,
    });

    log::info!("Created fullscreen quad vertex buffer with 6 vertices");
    buffer
}
