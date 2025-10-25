mod color;
pub mod cursor;
mod gpu;
mod pipeline;
mod text_rasterizer;
mod texture;
pub mod theme;

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
pub use theme::ColorPalette;
use crate::selection::{SelectionRange, SelectionRenderer, PaneViewport, calculate_pane_viewports};
use crate::pane::PaneNode;

// Deleted: ScrollAnimation spring physics (Step 2 - Delete unnecessary complexity)
// Replaced with simple fractional scrolling for smooth, jitter-free scrolling

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
    scroll_offset: f32,  // Fractional scroll position for smooth scrolling
    cursor_state: CursorState,
    cursor_pipeline: wgpu::RenderPipeline,
    color_palette: ColorPalette,
    selection_renderer: SelectionRenderer,
}

impl<'a> Renderer<'a> {
    /// Create a new renderer
    pub async fn new(
        window: &'a winit::window::Window,
        font_family: &str,
        font_size: f32,
        cursor_config: CursorConfig,
        color_palette: ColorPalette,
    ) -> Result<Self> {
        // Initialize GPU context
        let gpu = GpuContext::new(window).await?;

        // Get current DPI scale factor
        let scale_factor = window.scale_factor();
        let font_manager = FontManager::new_with_scale(font_family, font_size, scale_factor)?;

        // Calculate cell dimensions and baseline using effective font size
        let (cell_width, cell_height, baseline_offset) = {
            let effective_size = font_manager.effective_font_size();
            let line_metrics = font_manager.font().horizontal_line_metrics(effective_size).unwrap();
            let cell_width = font_manager.font().metrics('M', effective_size).advance_width;
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

        // Create selection renderer
        let selection_renderer = SelectionRenderer::new(&gpu.device, gpu.config.format);

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
            scroll_offset: 0.0,
            cursor_state,
            cursor_pipeline,
            color_palette,
            selection_renderer,
        })
    }

    /// Scroll viewport by fractional delta (direct, smooth scrolling)
    /// Positive delta = scroll up (into history), Negative delta = scroll down (toward present)
    pub fn scroll(&mut self, delta: f32) {
        // Directly apply the delta for smooth scrolling
        self.scroll_offset = (self.scroll_offset + delta).max(0.0);
        // Bounds checking happens in render() where we clamp to history_size
    }

    /// Reset scroll to bottom (live view)
    pub fn reset_scroll(&mut self) {
        self.scroll_offset = 0.0;
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
                
                // Clamp scroll offset to available history
                let history_size = term_lock.grid().history_size();
                self.scroll_offset = self.scroll_offset.min(history_size as f32);
                
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

    /// Render a frame with pane tree (shows focused pane + borders)
    pub fn render_with_panes<T>(&mut self, pane_tree: &PaneNode) -> Result<()> {
        // Get focused pane
        if let Some(focused_pane) = pane_tree.focused_pane() {
            // Render focused pane's terminal (reuse existing render logic)
            if let Some(term_lock) = focused_pane.terminal.term().try_lock() {
                log::debug!("Rendering focused pane {}", focused_pane.id);
                
                // Clamp scroll offset to available history
                let history_size = term_lock.grid().history_size();
                self.scroll_offset = self.scroll_offset.min(history_size as f32);
                
                self.render_terminal_to_texture(&term_lock)?;
                self.update_cursor_position(&term_lock);
            } else {
                log::warn!("Could not lock focused pane terminal for rendering");
            }
        } else {
            log::warn!("No focused pane found");
        }

        // Update cursor blink
        let blink_changed = self.cursor_state.update_blink();
        if blink_changed {
            self.cursor_state.upload_uniforms(&self.queue);
        }

        // Calculate pane viewports for border rendering
        let viewports = calculate_pane_viewports(pane_tree, self.config.width, self.config.height);
        
        // Execute render pass with borders
        self.execute_render_pass_with_borders(&viewports)?;
        Ok(())
    }

    /// Update cursor position based on terminal state
    fn update_cursor_position<T>(&mut self, term: &Term<T>) {
        let cursor_pos = term.grid().cursor.point;
        
        // Cursor visibility is managed by the terminal's DECTCEM mode (CSI ? 25 h/l)
        // SHOW_CURSOR flag present = visible, absent = hidden
        // Also hide cursor when scrolled in history
        let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR) 
                          || self.scroll_offset > 0.01;
        
        log::debug!("Cursor: pos=({}, {}), SHOW_CURSOR={}, hide={}", 
                   cursor_pos.column.0, cursor_pos.line.0, 
                   term.mode().contains(TermMode::SHOW_CURSOR), hide_cursor);
        
        // Use effective_font_size() to account for DPI scaling across monitors₹
        let effective_size = self.font_manager.effective_font_size();
        let line_metrics = self.font_manager.font()
            .horizontal_line_metrics(effective_size)
            .unwrap();
        let cell_width = self.font_manager.font()
            .metrics('M', effective_size)
            .advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();

        self.cursor_state.update_position(
            cursor_pos,
            cell_width,
            cell_height,
            self.config.width,
            self.config.height,
            self.scroll_offset.round() as usize,  // Convert to usize for cursor position
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
            
            // Draw selection highlights
            if self.selection_renderer.has_selection() {
                log::trace!("Drawing selection highlights");
                self.selection_renderer.upload_uniforms(&self.queue);
                render_pass.set_pipeline(self.selection_renderer.pipeline());
                render_pass.set_bind_group(0, self.selection_renderer.bind_group(), &[]);
                let instance_count = self.selection_renderer.instance_count();
                render_pass.draw(0..6, 0..instance_count);
            }
            
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
            self.scroll_offset.round() as usize,  // Convert to usize for grid access
            self.config.format,
            &self.color_palette,
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

    /// Get current scroll offset
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset.round() as usize
    }

    /// Update selection rendering
    pub fn update_selection(&mut self, range: Option<SelectionRange>, grid_cols: usize, grid_lines: usize) {
        let line_metrics = self.font_manager.font()
            .horizontal_line_metrics(self.font_manager.font_size())
            .unwrap();
        let cell_width = self.font_manager.font()
            .metrics('M', self.font_manager.font_size())
            .advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();

        self.selection_renderer.update(
            range,
            cell_width,
            cell_height,
            self.config.width,
            self.config.height,
            grid_cols,
            grid_lines,
        );
    }

    /// Update font size and recalculate cell dimensions
    pub fn set_font_size(&mut self, font_size: f32) -> Result<()> {
        // Update font manager
        self.font_manager.set_font_size(font_size);
        
        // Recalculate cell dimensions
        let effective_size = self.font_manager.effective_font_size();
        let line_metrics = self.font_manager.font().horizontal_line_metrics(effective_size).unwrap();
        let cell_width = self.font_manager.font().metrics('M', effective_size).advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        let baseline_offset = line_metrics.ascent.ceil();
        
        // Update text rasterizer
        self.text_rasterizer.update_dimensions(cell_width, cell_height, baseline_offset);
        
        info!("Font size updated to {} (effective: {}): cell={}x{}, baseline={}", 
              font_size, effective_size, cell_width, cell_height, baseline_offset);
        
        Ok(())
    }

    /// Handle DPI scale factor change (monitor change, etc.)
    pub fn handle_scale_factor_changed(&mut self, scale_factor: f64) -> Result<()> {
        info!("Scale factor changed to: {:.2}x", scale_factor);
        
        // Update font manager with new scale
        self.font_manager.update_scale_factor(scale_factor);
        
        // Recalculate cell dimensions with new effective font size
        let effective_size = self.font_manager.effective_font_size();
        let line_metrics = self.font_manager.font().horizontal_line_metrics(effective_size).unwrap();
        let cell_width = self.font_manager.font().metrics('M', effective_size).advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
        let baseline_offset = line_metrics.ascent.ceil();
        
        // Update text rasterizer
        self.text_rasterizer.update_dimensions(cell_width, cell_height, baseline_offset);
        
        info!("DPI updated: effective font size={}, cell={}x{}", 
              effective_size, cell_width, cell_height);
        
        Ok(())
    }
}
