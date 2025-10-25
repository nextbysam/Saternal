use wgpu;
use wgpu::util::DeviceExt;

/// GPU-based Gaussian blur renderer
///
/// Implements a two-pass blur (horizontal + vertical) for terminal backgrounds
/// when no wallpaper is set. This provides a nice frosted-glass effect.
pub struct BlurRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group_horizontal: Option<wgpu::BindGroup>,
    bind_group_vertical: Option<wgpu::BindGroup>,
    uniforms_buffer: wgpu::Buffer,
    temp_texture: Option<wgpu::Texture>,
    temp_texture_view: Option<wgpu::TextureView>,
    vertex_buffer: wgpu::Buffer,
    blur_strength: f32,
    enabled: bool,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct BlurUniforms {
    direction: [f32; 2],  // (1, 0) for horizontal, (0, 1) for vertical
    strength: f32,
    _padding: f32,
}

unsafe impl bytemuck::Pod for BlurUniforms {}
unsafe impl bytemuck::Zeroable for BlurUniforms {}

impl BlurRenderer {
    /// Create a new blur renderer
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                // Source texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Blur Bind Group Layout"),
        });

        // Create render pipeline
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/blur.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blur Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blur Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 4 * 4, // 4 floats per vertex
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // Position
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        // Tex coords
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
        });

        // Create uniforms buffer with default horizontal blur
        let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blur Uniforms Buffer"),
            contents: bytemuck::cast_slice(&[BlurUniforms {
                direction: [1.0, 0.0], // Horizontal
                strength: 2.0,
                _padding: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create vertex buffer (fullscreen quad)
        #[rustfmt::skip]
        let vertices: &[f32] = &[
            // positions      // tex coords
            -1.0, -1.0,       0.0, 1.0,
             1.0, -1.0,       1.0, 1.0,
             1.0,  1.0,       1.0, 0.0,
            -1.0, -1.0,       0.0, 1.0,
             1.0,  1.0,       1.0, 0.0,
            -1.0,  1.0,       0.0, 0.0,
        ];

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Blur Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            pipeline,
            bind_group_layout,
            bind_group_horizontal: None,
            bind_group_vertical: None,
            uniforms_buffer,
            temp_texture: None,
            temp_texture_view: None,
            vertex_buffer,
            blur_strength: 2.0,
            enabled: false,
        }
    }

    /// Enable or disable blur rendering
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if blur is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set blur strength (default: 2.0)
    pub fn set_strength(&mut self, strength: f32, queue: &wgpu::Queue) {
        self.blur_strength = strength;
        self.update_uniforms(queue);
    }

    /// Update uniforms for both passes
    fn update_uniforms(&self, _queue: &wgpu::Queue) {
        // We'll update before each pass in render()
    }

    /// Initialize or resize blur textures
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32, format: wgpu::TextureFormat) {
        if width == 0 || height == 0 {
            return;
        }

        // Create temporary texture for two-pass blur
        let temp_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Blur Temp Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let temp_texture_view = temp_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.temp_texture = Some(temp_texture);
        self.temp_texture_view = Some(temp_texture_view);
    }

    /// Prepare bind groups for blur passes
    pub fn prepare_bind_groups(
        &mut self,
        device: &wgpu::Device,
        source_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) {
        if self.temp_texture_view.is_none() {
            return;
        }

        // Horizontal pass: source -> temp
        self.bind_group_horizontal = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniforms_buffer.as_entire_binding(),
                },
            ],
            label: Some("Blur Bind Group Horizontal"),
        }));

        // Vertical pass: temp -> output
        self.bind_group_vertical = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        self.temp_texture_view.as_ref().unwrap(),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniforms_buffer.as_entire_binding(),
                },
            ],
            label: Some("Blur Bind Group Vertical"),
        }));
    }

    /// Render two-pass blur
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        output_view: &wgpu::TextureView,
    ) {
        if !self.enabled || self.temp_texture_view.is_none() {
            return;
        }

        // Pass 1: Horizontal blur (source -> temp)
        {
            queue.write_buffer(
                &self.uniforms_buffer,
                0,
                bytemuck::cast_slice(&[BlurUniforms {
                    direction: [1.0, 0.0],
                    strength: self.blur_strength,
                    _padding: 0.0,
                }]),
            );

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blur Pass Horizontal"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.temp_texture_view.as_ref().unwrap(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, self.bind_group_horizontal.as_ref().unwrap(), &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }

        // Pass 2: Vertical blur (temp -> output)
        {
            queue.write_buffer(
                &self.uniforms_buffer,
                0,
                bytemuck::cast_slice(&[BlurUniforms {
                    direction: [0.0, 1.0],
                    strength: self.blur_strength,
                    _padding: 0.0,
                }]),
            );

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blur Pass Vertical"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve existing content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, self.bind_group_vertical.as_ref().unwrap(), &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
        }
    }

    /// Get the render pipeline
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}
