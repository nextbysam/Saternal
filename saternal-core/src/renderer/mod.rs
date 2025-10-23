mod color;
pub mod cursor;
mod gpu;
mod pipeline;
mod text_rasterizer;
mod texture;

use crate::font::FontManager;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Term, TermMode};
use anyhow::Result;
use log::info;
use parking_lot::Mutex;
use std::sync::Arc;
use wgpu;

use cursor::{create_cursor_pipeline, CursorConfig, CursorState};
use gpu::GpuContext;
use pipeline::{create_render_pipeline, create_vertex_buffer};
use text_rasterizer::TextRasterizer;
use texture::TextureManager;

/// GPU-accelerated renderer using wgpu/Metal
pub struct Renderer<'a> {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'a>,
    config: wgpu::SurfaceConfiguration,
    font_manager: FontManager,
    texture_manager: TextureManager,
    text_rasterizer: TextRasterizer,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    scroll_offset: usize,
    cursor_state: CursorState,
    cursor_pipeline: wgpu::RenderPipeline,
}

impl<'a> Renderer<'a> {
    /// Create a new renderer
    pub async fn new(
        window: &'a winit::window::Window,
        font_family: &str,
        font_size: f32,
        cursor_config: CursorConfig,
    ) -> Result<Self> {
        // Initialize GPU context
        let gpu = GpuContext::new(window).await?;

        let font_manager = FontManager::new(font_family, font_size)?;

        // Calculate cell dimensions and baseline
        let (cell_width, cell_height, baseline_offset) = {
            let line_metrics = font_manager.font().horizontal_line_metrics(font_size).unwrap();
            let cell_width = font_manager.font().metrics('M', font_size).advance_width;
            let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
            let baseline_offset = line_metrics.ascent.ceil();
            (cell_width, cell_height, baseline_offset)
        };

        // Create text rasterizer
        let text_rasterizer = TextRasterizer::new(cell_width, cell_height, baseline_offset);

        // Create texture manager
        let texture_manager = TextureManager::new(
            &gpu.device,
            gpu.config.width,
            gpu.config.height,
            gpu.config.format,
        );

        // Create render pipeline
        let render_pipeline = create_render_pipeline(
            &gpu.device,
            &texture_manager.bind_group_layout,
            gpu.config.format,
        );

        // Create vertex buffer
        let vertex_buffer = create_vertex_buffer(&gpu.device);

        // Create cursor state and pipeline
        let cursor_state = CursorState::new(&gpu.device, cursor_config);
        let cursor_pipeline = create_cursor_pipeline(
            &gpu.device,
            &cursor_state.bind_group_layout,
            gpu.config.format,
        );

        Ok(Self {
            device: gpu.device,
            queue: gpu.queue,
            surface: gpu.surface,
            config: gpu.config,
            font_manager,
            texture_manager,
            text_rasterizer,
            render_pipeline,
            vertex_buffer,
            scroll_offset: 0,
            cursor_state,
            cursor_pipeline,
        })
    }

    /// Scroll viewport by delta lines
    /// Positive delta = scroll up (into history), Negative delta = scroll down (toward present)
    /// 
    /// Note: The actual bounds checking happens in render_to_buffer() where we have access
    /// to the terminal's history_size(). This allows us to clamp to the available scrollback.
    /// Using saturating arithmetic here prevents overflow but the real limit is enforced at render time.
    pub fn scroll(&mut self, delta: i32) {
        if delta > 0 {
            // Scroll up into history
            self.scroll_offset = self.scroll_offset.saturating_add(delta as usize);
            log::debug!("Scrolled up, offset now: {}", self.scroll_offset);
        } else if delta < 0 {
            // Scroll down toward present (offset 0 = live view)
            self.scroll_offset = self.scroll_offset.saturating_sub((-delta) as usize);
            log::debug!("Scrolled down, offset now: {}", self.scroll_offset);
        }
    }

    /// Reset scroll to bottom (live view)
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
        log::debug!("Reset scroll to bottom");
    }

    /// Render a frame with terminal content
    pub fn render<T>(&mut self, term: Option<Arc<Mutex<Term<T>>>>) -> Result<()> {
        // Update cursor blink state
        let blink_changed = self.cursor_state.update_blink();

        // Render terminal text to texture
        if let Some(term_arc) = &term {
            log::debug!("Attempting to lock terminal for rendering");
            if let Some(term_lock) = term_arc.try_lock() {
                log::debug!("Terminal locked, rendering to texture");
                self.render_terminal_to_texture(&term_lock)?;
                
                // Update cursor position
                self.update_cursor_position(&term_lock);
            } else {
                log::warn!("Could not lock terminal for rendering");
            }
        } else {
            log::warn!("No terminal provided to render");
        }

        // Upload cursor uniforms if blink changed
        if blink_changed {
            self.cursor_state.upload_uniforms(&self.queue);
        }

        self.execute_render_pass()?;
        Ok(())
    }

    /// Update cursor position based on terminal state
    fn update_cursor_position<T>(&mut self, term: &Term<T>) {
        let cursor_pos = term.grid().cursor.point;
        
        // Cursor visibility is managed by the terminal's DECTCEM mode (CSI ? 25 h/l)
        // SHOW_CURSOR flag present = visible, absent = hidden
        let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR);
        
        let line_metrics = self.font_manager.font()
            .horizontal_line_metrics(self.font_manager.font_size())
            .unwrap();
        let cell_width = self.font_manager.font()
            .metrics('M', self.font_manager.font_size())
            .advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();

        self.cursor_state.update_position(
            cursor_pos,
            cell_width,
            cell_height,
            self.config.width,
            self.config.height,
            self.scroll_offset,
            hide_cursor,
        );
        
        // Upload uniforms to GPU
        self.cursor_state.upload_uniforms(&self.queue);
    }

    /// Execute the GPU render pass to draw the frame
    fn execute_render_pass(&mut self) -> Result<()> {
        log::trace!("Getting surface texture for rendering...");
        let frame = self.surface.get_current_texture()?;
        log::trace!("Got surface texture, creating view...");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            log::trace!("Setting pipeline and drawing quad...");
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.texture_manager.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            log::trace!("Drawing 6 vertices for fullscreen quad");
            render_pass.draw(0..6, 0..1);
            
            // Draw cursor overlay
            if self.cursor_state.is_visible() {
                log::trace!("Drawing cursor overlay");
                render_pass.set_pipeline(&self.cursor_pipeline);
                render_pass.set_bind_group(0, &self.cursor_state.bind_group, &[]);
                render_pass.draw(0..6, 0..1);
            }
        }
        
        log::trace!("Submitting command buffer and presenting frame...");
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    /// Render terminal content to texture (CPU-based for simplicity)
    fn render_terminal_to_texture<T>(&mut self, term: &Term<T>) -> Result<()> {
        // Use text rasterizer to generate buffer
        let buffer = self.text_rasterizer.render_to_buffer(
            term,
            &self.font_manager,
            self.config.width,
            self.config.height,
            self.scroll_offset,
            self.config.format,
        )?;

        // Upload to GPU texture
        log::debug!("Uploading {}x{} texture to GPU", self.config.width, self.config.height);
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture_manager.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &buffer,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(self.config.width * 4),
                rows_per_image: Some(self.config.height),
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );

        Ok(())
    }

    /// Resize the renderer
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            info!("Resizing renderer to {}x{}", width, height);
            
            // Update surface configuration
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            
            // Resize texture manager
            self.texture_manager.resize(&self.device, width, height, self.config.format);
            
            info!("Renderer resized successfully");
        }
    }

    /// Get font manager
    pub fn font_manager(&mut self) -> &mut FontManager {
        &mut self.font_manager
    }

    /// Update font size and recalculate cell dimensions
    pub fn set_font_size(&mut self, font_size: f32) -> Result<()> {
        // Update font manager
        self.font_manager.set_font_size(font_size);
        
        // Recalculate cell dimensions
        let line_metrics = self.font_manager.font().horizontal_line_metrics(font_size).unwrap();
        let cell_width = self.font_manager.font().metrics('M', font_size).advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        let baseline_offset = line_metrics.ascent.ceil();
        
        // Update text rasterizer
        self.text_rasterizer.update_dimensions(cell_width, cell_height, baseline_offset);
        
        info!("Font size updated to {}: cell={}x{}, baseline={}", 
              font_size, cell_width, cell_height, baseline_offset);
        
        Ok(())
    }
}
