mod borders;
mod color;
pub mod cursor;
mod glyph_atlas;
mod glyph_renderer;
mod gpu;
mod opacity;
mod pipeline;
mod text_rasterizer;
mod texture;
pub mod theme;
mod ui_box;
mod wallpaper;

use crate::font::FontManager;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Term, TermMode};
use anyhow::Result;
use log::info;
use parking_lot::Mutex;
use rayon::prelude::*;
use std::sync::Arc;
use wgpu;

use borders::BorderRenderer;
use cursor::{create_cursor_pipeline, CursorConfig, CursorState, CursorStyle};
use glyph_atlas::GlyphAtlas;
use glyph_renderer::GlyphRenderer;
use gpu::GpuContext;
use opacity::OpacityUniforms;
use pipeline::{create_render_pipeline, create_vertex_buffer};
use text_rasterizer::TextRasterizer;
use texture::TextureManager;
pub use theme::ColorPalette;
pub use ui_box::UIBox;
use wallpaper::WallpaperManager;
use crate::selection::{SelectionRange, SelectionRenderer, PaneViewport, calculate_pane_viewports};
use crate::pane::PaneNode;
use crate::ConfirmationLevel;

// Deleted: ScrollAnimation spring physics (Step 2 - Delete unnecessary complexity)
// Replaced with simple fractional scrolling for smooth, jitter-free scrolling

/// GPU-accelerated renderer using wgpu/Metal
/// 
/// Safety: The Surface has a 'static lifetime, but is actually tied to the Window's lifetime.
/// This is sound because:
/// 1. We store Arc<Window> to keep the window alive
/// 2. Rust drops struct fields in declaration order (top to bottom)
/// 3. Therefore, surface drops before _window, preventing use-after-free
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    font_manager: FontManager,
    texture_manager: TextureManager,
    glyph_atlas: GlyphAtlas,
    glyph_renderer: GlyphRenderer,
    text_rasterizer: TextRasterizer, // Keep for backward compatibility during transition
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    scroll_offset: f32,  // Fractional scroll position for smooth scrolling
    cursor_state: CursorState,
    cursor_pipeline: wgpu::RenderPipeline,
    color_palette: ColorPalette,
    selection_renderer: SelectionRenderer,
    border_renderer: BorderRenderer,
    wallpaper_manager: WallpaperManager,
    opacity_uniforms: OpacityUniforms,
    _window: std::sync::Arc<winit::window::Window>, // Keep window alive - must be last for drop order
}

impl Renderer {
    /// Create a new renderer
    ///
    /// Takes Arc<Window> to ensure proper lifetime management through drop order guarantees.
    pub async fn new(
        window: std::sync::Arc<winit::window::Window>,
        font_family: &str,
        font_size: f32,
        cursor_config: CursorConfig,
        color_palette: ColorPalette,
        wallpaper_path: Option<&str>,
        wallpaper_opacity: f32,
        background_opacity: f32,
    ) -> Result<Self> {
        // Initialize GPU context
        let gpu = GpuContext::new(window.clone()).await?;

        // Get current DPI scale factor
        let scale_factor = window.as_ref().scale_factor();
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

        // Create glyph atlas (2048x2048 texture)
        let glyph_atlas = GlyphAtlas::new(&gpu.device, &gpu.queue, &font_manager, 2048)?;
        
        // Create GPU glyph renderer
        let mut glyph_renderer = GlyphRenderer::new(
            &gpu.device,
            gpu.config.format,
            &glyph_atlas,
            cell_width,
            cell_height,
            baseline_offset,
            gpu.config.width,
            gpu.config.height,
        );
        
        // Upload initial screen dimensions
        glyph_renderer.update_screen_size(&gpu.queue, gpu.config.width, gpu.config.height);

        // Create text rasterizer (kept for backward compatibility)
        let text_rasterizer = TextRasterizer::new(cell_width, cell_height, baseline_offset);

        // Create texture manager
        let texture_manager = TextureManager::new(
            &gpu.device,
            gpu.config.width,
            gpu.config.height,
            gpu.config.format,
        );

        // Create wallpaper manager
        let mut wallpaper_manager = WallpaperManager::new(&gpu.device);

        // Load wallpaper if path provided
        if let Some(path) = wallpaper_path {
            log::info!("Attempting to load wallpaper from: {}", path);
            match wallpaper_manager.load(&gpu.device, &gpu.queue, path) {
                Ok(_) => log::info!("✓ Wallpaper loaded successfully: {}", path),
                Err(e) => log::error!("✗ WALLPAPER LOADING FAILED: {} - Error: {}", path, e),
            }
        } else {
            log::info!("No wallpaper path configured");
        }

        // Create opacity uniforms
        let has_wallpaper = wallpaper_manager.has_wallpaper();
        log::info!("Initializing opacity uniforms: wallpaper_opacity={}, background_opacity={}, has_wallpaper={}",
                   wallpaper_opacity, background_opacity, has_wallpaper);
        let opacity_uniforms = OpacityUniforms::new(
            &gpu.device,
            wallpaper_opacity,
            background_opacity,
            has_wallpaper,
        );

        // Create render pipeline with all bind group layouts
        let render_pipeline = create_render_pipeline(
            &gpu.device,
            &texture_manager.bind_group_layout,
            wallpaper_manager.bind_group_layout(),
            opacity_uniforms.bind_group_layout(),
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

        // Create border renderer
        let border_renderer = BorderRenderer::new(&gpu.device, gpu.config.format);

        Ok(Self {
            device: gpu.device,
            queue: gpu.queue,
            surface: gpu.surface,
            config: gpu.config,
            font_manager,
            texture_manager,
            glyph_atlas,
            glyph_renderer,
            text_rasterizer,
            render_pipeline,
            vertex_buffer,
            scroll_offset: 0.0,
            cursor_state,
            cursor_pipeline,
            color_palette,
            selection_renderer,
            border_renderer,
            wallpaper_manager,
            opacity_uniforms,
            _window: window, // Must be last to ensure correct drop order
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

        // Generate GPU instances for terminal text
        if let Some(term_arc) = &term {
            log::debug!("Attempting to lock terminal for rendering");
            if let Some(term_lock) = term_arc.try_lock() {
                log::debug!("Terminal locked, generating text instances");
                
                // Clamp scroll offset to available history
                let history_size = term_lock.grid().history_size();
                self.scroll_offset = self.scroll_offset.min(history_size as f32);
                
                self.generate_text_instances(&term_lock)?;
                
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

    /// Render a frame with pane tree (shows all panes in their viewports)
    /// Uses parallel rendering for improved performance with multiple panes
    pub fn render_with_panes(&mut self, pane_tree: &PaneNode) -> Result<()> {
        self.render_with_panes_and_ui(pane_tree, None)
    }

    /// Render with optional UI overlay
    pub fn render_with_panes_and_ui(&mut self, pane_tree: &PaneNode, ui_box: Option<&UIBox>) -> Result<()> {
        // Calculate pane viewports
        let viewports = calculate_pane_viewports(pane_tree, self.config.width, self.config.height);
        
        // Create a black buffer for the entire window
        let total_pixels = (self.config.width * self.config.height) as usize;
        let mut combined_buffer = vec![0u8; total_pixels * 4];

        // Collect pane data for parallel rendering (clone Arc<Mutex> to own it)
        let pane_data: Vec<_> = viewports.iter()
            .filter_map(|viewport| {
                pane_tree.find_pane(viewport.pane_id).map(|pane| {
                    let term_arc = pane.terminal.term();  // Clone Arc for ownership
                    (term_arc, viewport)
                })
            })
            .collect();

        // Extract immutable references for parallel access
        let text_rasterizer = &self.text_rasterizer;
        let font_manager = &self.font_manager;
        let surface_format = self.config.format;
        let color_palette = &self.color_palette;
        let scroll_offset = self.scroll_offset;

        // PARALLEL: Render all panes simultaneously on multiple CPU cores
        // Returns (viewport, buffer) pairs for successful renders
        let rendered_panes: Vec<(&PaneViewport, Vec<u8>)> = pane_data.par_iter()
            .filter_map(|(term_arc, viewport)| {
                // Try to lock terminal (non-blocking)
                let term_lock = term_arc.try_lock()?;
                
                log::debug!("Rendering pane {} to viewport ({}, {}) {}x{}", 
                    viewport.pane_id, viewport.x, viewport.y, viewport.width, viewport.height);
                
                // Clamp scroll offset to available history for focused pane
                let pane_scroll_offset = if viewport.focused {
                    let history_size = term_lock.grid().history_size();
                    scroll_offset.min(history_size as f32).round() as usize
                } else {
                    0 // Non-focused panes show live view
                };
                
                // Render this pane's terminal to a viewport-sized buffer (CPU-bound work)
                let pane_buffer = text_rasterizer.render_to_buffer(
                    &term_lock,
                    font_manager,
                    viewport.width,
                    viewport.height,
                    pane_scroll_offset,
                    surface_format,
                    color_palette,
                ).ok()?;
                
                Some((*viewport, pane_buffer))
            })
            .collect();

        // SEQUENTIAL: Copy buffers to combined buffer and update cursor
        for (viewport, pane_buffer) in rendered_panes {
            // Copy pane buffer to combined buffer at viewport position
            self.copy_buffer_to_region(
                &pane_buffer,
                &mut combined_buffer,
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                self.config.width,
            );
        }
        
        // Render UI overlay if present
        if let Some(ui_box) = ui_box {
            log::debug!("Rendering UI overlay");
            let box_cells = ui_box.render(&self.color_palette);
            
            // Calculate cell dimensions
            let effective_size = self.font_manager.effective_font_size();
            let line_metrics = self.font_manager.font().horizontal_line_metrics(effective_size).unwrap();
            let cell_width = self.font_manager.font().metrics('M', effective_size).advance_width;
            let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();
            
            // Get cursor position from focused pane and position UI in that viewport
            if let Some(focused_vp) = viewports.iter().find(|vp| vp.focused) {
                if let Some(pane) = pane_tree.find_pane(focused_vp.pane_id) {
                    if let Some(term_lock) = pane.terminal.term().try_lock() {
                        let cursor_pos = term_lock.grid().cursor.point;
                        
                        // Get scroll offset for this pane (only focused pane scrolls)
                        let history_size = term_lock.grid().history_size();
                        let pane_scroll_offset = if focused_vp.focused {
                            scroll_offset.min(history_size as f32).round() as usize
                        } else {
                            0
                        };
                        
                        // Calculate cursor position in viewport, accounting for scroll
                        // When scrolled up, cursor moves down visually
                        let cursor_line_in_viewport = (cursor_pos.line.0 as i32 + pane_scroll_offset as i32) as usize;
                        
                        // Position UI box 1 line below cursor, with 2 column indent
                        let ui_row = cursor_line_in_viewport.saturating_add(1);
                        let ui_col = 2; // 2 columns indent
                        
                        // Calculate viewport dimensions in cells
                        let vp_cols = ((focused_vp.width as f32 - crate::constants::PADDING_LEFT - crate::constants::PADDING_RIGHT) / cell_width) as usize;
                        let vp_rows = ((focused_vp.height as f32 - crate::constants::PADDING_TOP - crate::constants::PADDING_BOTTOM) / cell_height) as usize;
                        
                        // Clamp to viewport bounds (prevent overflow at bottom)
                        let box_height = ui_box.height();
                        let ui_row = ui_row.min(vp_rows.saturating_sub(box_height + 1));
                        
                        // Convert grid position to pixel position within viewport
                        let pixel_x_in_vp = ui_col as f32 * cell_width + crate::constants::PADDING_LEFT;
                        let pixel_y_in_vp = ui_row as f32 * cell_height + crate::constants::PADDING_TOP;
                        
                        // Add viewport offset to get window-relative pixel position
                        let pixel_x = focused_vp.x as f32 + pixel_x_in_vp;
                        let pixel_y = focused_vp.y as f32 + pixel_y_in_vp;
                        
                        log::debug!("UI overlay: cursor=({}, {}), scroll={}, viewport=({}, {}), pixel=({:.1}, {:.1})",
                                   cursor_pos.column.0, cursor_pos.line.0, pane_scroll_offset,
                                   focused_vp.x, focused_vp.y, pixel_x, pixel_y);
                        
                        self.text_rasterizer.overlay_cells(
                            &mut combined_buffer,
                            &box_cells,
                            pixel_x as u32,
                            pixel_y as u32,
                            self.config.width,
                            self.config.height,
                            &self.font_manager,
                            self.config.format,
                            &self.color_palette,
                        );
                    }
                }
            }
        }
        
        // Update cursor for focused pane (requires re-locking)
        if let Some(focused_vp) = viewports.iter().find(|vp| vp.focused) {
            if let Some(pane) = pane_tree.find_pane(focused_vp.pane_id) {
                if let Some(term_lock) = pane.terminal.term().try_lock() {
                    self.update_cursor_position_with_viewport(&term_lock, focused_vp);
                }
            }
        }

        // Upload combined buffer to GPU texture
        log::debug!("Uploading {}x{} combined texture to GPU", self.config.width, self.config.height);
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &self.texture_manager.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &combined_buffer,
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

        // Update cursor blink
        let blink_changed = self.cursor_state.update_blink();
        if blink_changed {
            self.cursor_state.upload_uniforms(&self.queue);
        }
        
        // Execute render pass with borders
        self.execute_render_pass_with_borders(&viewports)?;
        Ok(())
    }

    /// Copy a buffer to a specific region of the combined buffer
    fn copy_buffer_to_region(
        &self,
        src: &[u8],
        dst: &mut [u8],
        dst_x: u32,
        dst_y: u32,
        src_width: u32,
        src_height: u32,
        dst_width: u32,
    ) {
        let bytes_per_pixel = 4;
        for y in 0..src_height {
            let src_row_start = (y * src_width * bytes_per_pixel) as usize;
            let src_row_end = src_row_start + (src_width * bytes_per_pixel) as usize;
            
            let dst_row_start = ((dst_y + y) * dst_width * bytes_per_pixel + dst_x * bytes_per_pixel) as usize;
            let dst_row_end = dst_row_start + (src_width * bytes_per_pixel) as usize;
            
            if src_row_end <= src.len() && dst_row_end <= dst.len() {
                dst[dst_row_start..dst_row_end].copy_from_slice(&src[src_row_start..src_row_end]);
            }
        }
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

    /// Update cursor position with viewport offset
    fn update_cursor_position_with_viewport<T>(&mut self, term: &Term<T>, viewport: &PaneViewport) {
        let cursor_pos = term.grid().cursor.point;
        
        let hide_cursor = !term.mode().contains(TermMode::SHOW_CURSOR) 
                          || self.scroll_offset > 0.01;
        
        let effective_size = self.font_manager.effective_font_size();
        let line_metrics = self.font_manager.font()
            .horizontal_line_metrics(effective_size)
            .unwrap();
        let cell_width = self.font_manager.font()
            .metrics('M', effective_size)
            .advance_width;
        let cell_height = (line_metrics.ascent - line_metrics.descent + line_metrics.line_gap).ceil();

        // Calculate cursor position relative to viewport
        const PADDING_LEFT: f32 = 10.0;
        const PADDING_TOP: f32 = 5.0;
        let cursor_pixel_x = viewport.x as f32 + cursor_pos.column.0 as f32 * cell_width + PADDING_LEFT;
        let cursor_pixel_y = viewport.y as f32 + cursor_pos.line.0 as f32 * cell_height + PADDING_TOP;
        
        // Convert to NDC
        let ndc_x = (cursor_pixel_x / self.config.width as f32) * 2.0 - 1.0;
        let mut ndc_y = -((cursor_pixel_y / self.config.height as f32) * 2.0 - 1.0);
        
        // Calculate size based on cursor style
        let (width, height) = match self.cursor_state.config.style {
            CursorStyle::Block => (cell_width, cell_height),
            CursorStyle::Beam => (2.0, cell_height),
            CursorStyle::Underline => (cell_width, 2.0),
        };

        let ndc_width = (width / self.config.width as f32) * 2.0;
        let ndc_height = -((height / self.config.height as f32) * 2.0);
        
        // Adjust Y for underline style
        if matches!(self.cursor_state.config.style, CursorStyle::Underline) {
            ndc_y += (cell_height - 2.0) / self.config.height as f32 * 2.0;
        }
        
        log::debug!("Cursor at viewport offset: pixel=({:.1}, {:.1}), ndc=({:.3}, {:.3})", 
                   cursor_pixel_x, cursor_pixel_y, ndc_x, ndc_y);
        
        // Use pre-calculated NDC coordinates
        self.cursor_state.update_position_ndc(ndc_x, ndc_y, ndc_width, ndc_height, hide_cursor);
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
                            a: 0.0,  // Transparent clear for window transparency
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw background/wallpaper first
            log::trace!("Drawing background/wallpaper");
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.texture_manager.bind_group, &[]);
            render_pass.set_bind_group(1, self.wallpaper_manager.bind_group(), &[]);
            render_pass.set_bind_group(2, self.opacity_uniforms.bind_group(), &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);
            
            // Draw GPU-rasterized text using instanced rendering
            log::trace!("Drawing text glyphs with GPU instancing");
            self.glyph_renderer.render(&mut render_pass, &self.glyph_atlas);
            
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

    /// Execute the GPU render pass with pane borders
    fn execute_render_pass_with_borders(&mut self, viewports: &[PaneViewport]) -> Result<()> {
        // Update border renderer with current viewports
        if viewports.len() > 1 {
            self.border_renderer.update(viewports, self.config.width, self.config.height);
            self.border_renderer.upload_uniforms(&self.queue);
        }

        log::trace!("Getting surface texture for rendering...");
        let frame = self.surface.get_current_texture()?;
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
                            a: 0.0,  // Transparent clear for window transparency
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw terminal content
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.texture_manager.bind_group, &[]);
            render_pass.set_bind_group(1, self.wallpaper_manager.bind_group(), &[]);
            render_pass.set_bind_group(2, self.opacity_uniforms.bind_group(), &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.draw(0..6, 0..1);

            // Draw selection highlights
            if self.selection_renderer.has_selection() {
                self.selection_renderer.upload_uniforms(&self.queue);
                render_pass.set_pipeline(self.selection_renderer.pipeline());
                render_pass.set_bind_group(0, self.selection_renderer.bind_group(), &[]);
                let instance_count = self.selection_renderer.instance_count();
                render_pass.draw(0..6, 0..instance_count);
            }

            // Draw cursor overlay
            if self.cursor_state.is_visible() {
                render_pass.set_pipeline(&self.cursor_pipeline);
                render_pass.set_bind_group(0, &self.cursor_state.bind_group, &[]);
                render_pass.draw(0..6, 0..1);
            }

            // Draw pane borders if we have multiple panes
            if viewports.len() > 1 {
                log::trace!("Drawing {} pane borders with GPU shader", viewports.len());
                self.render_pane_borders(&mut render_pass, viewports);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        Ok(())
    }

    /// Render pane borders using GPU-accelerated shader
    fn render_pane_borders<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, viewports: &[PaneViewport]) {
        if !self.border_renderer.has_borders() {
            return;
        }

        log::trace!("Rendering {} pane borders with GPU shader", viewports.len());
        render_pass.set_pipeline(self.border_renderer.pipeline());
        render_pass.set_bind_group(0, self.border_renderer.bind_group(), &[]);
        let instance_count = self.border_renderer.instance_count();
        render_pass.draw(0..6, 0..instance_count);
    }

    /// Generate GPU instances for terminal content
    fn generate_text_instances<T>(&mut self, term: &Term<T>) -> Result<()> {
        self.glyph_renderer.generate_instances(
            &self.queue,
            term,
            &mut self.glyph_atlas,
            &self.font_manager,
            &self.device,
            self.scroll_offset.round() as usize,
            &self.color_palette,
            self.config.width,
            self.config.height,
        )
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
            
            // Update glyph renderer screen size
            self.glyph_renderer.update_screen_size(&self.queue, width, height);

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
        
        // Update glyph renderer
        self.glyph_renderer.update_dimensions(cell_width, cell_height, baseline_offset);
        
        // Update text rasterizer (kept for backward compatibility)
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
        
        // Update glyph renderer
        self.glyph_renderer.update_dimensions(cell_width, cell_height, baseline_offset);
        
        // Update text rasterizer (kept for backward compatibility)
        self.text_rasterizer.update_dimensions(cell_width, cell_height, baseline_offset);
        
        info!("DPI updated: effective font size={}, cell={}x{}",
              effective_size, cell_width, cell_height);

        Ok(())
    }

    /// Set or clear wallpaper
    pub fn set_wallpaper(&mut self, path: Option<&str>) -> Result<()> {
        match path {
            Some(p) => {
                info!("Setting wallpaper: {}", p);
                self.wallpaper_manager.load(&self.device, &self.queue, p)?;
            }
            None => {
                info!("Clearing wallpaper");
                self.wallpaper_manager.clear(&self.device);
            }
        }

        // Update opacity uniforms with new wallpaper status
        self.opacity_uniforms.update(
            &self.queue,
            self.opacity_uniforms.wallpaper_opacity(),
            self.opacity_uniforms.background_opacity(),
            self.wallpaper_manager.has_wallpaper(),
        );

        Ok(())
    }

    /// Set wallpaper opacity
    pub fn set_wallpaper_opacity(&mut self, opacity: f32) {
        info!("Setting wallpaper opacity: {}", opacity);
        self.opacity_uniforms.update(
            &self.queue,
            opacity,
            self.opacity_uniforms.background_opacity(),
            self.wallpaper_manager.has_wallpaper(),
        );
    }

    /// Set background opacity
    pub fn set_background_opacity(&mut self, opacity: f32) {
        info!("Setting background opacity: {}", opacity);
        self.opacity_uniforms.update(
            &self.queue,
            self.opacity_uniforms.wallpaper_opacity(),
            opacity,
            self.wallpaper_manager.has_wallpaper(),
        );
    }

    /// Set blur strength (0.0 = disabled, 2.0 = default, 10.0 = heavy)
    /// Applies CPU-based blur to the wallpaper image
    pub fn set_blur_strength(&mut self, strength: f32) {
        info!("Setting blur strength: {}", strength);
        if let Err(e) = self.wallpaper_manager.set_blur_strength(&self.device, &self.queue, strength) {
            log::error!("Failed to apply blur: {}", e);
        }
    }
}
