use anyhow::Result;
use log::info;
use wgpu;

/// GPU context including device, queue, surface, and configuration
/// 
/// Safety: The Surface has a 'static lifetime, but is actually tied to the Window's lifetime.
/// This is sound because:
/// 1. We store Arc<Window> to keep the window alive
/// 2. Rust drops struct fields in declaration order (top to bottom)
/// 3. Therefore, surface drops before _window, preventing use-after-free
/// 4. The window cannot be dropped while the surface exists
pub(crate) struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    _window: std::sync::Arc<winit::window::Window>, // Keep window alive - must be last for drop order
}

impl GpuContext {
    /// Initialize GPU context with wgpu/Metal
    /// 
    /// Takes Arc<Window> to ensure proper lifetime management. The Window is kept alive
    /// via the stored Arc, ensuring the Surface remains valid through drop order guarantees.
    pub async fn new(window: std::sync::Arc<winit::window::Window>) -> Result<Self> {
        info!("Initializing GPU renderer");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        // Create surface from window reference, then extend lifetime to 'static
        // Safety: The window Arc is stored in the struct and drops after the surface,
        // ensuring the window outlives the surface through Rust's drop order guarantees
        let surface_temp = instance.create_surface(window.as_ref())?;
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface_temp) };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("Failed to find suitable GPU adapter"))?;

        info!("Using GPU adapter: {:?}", adapter.get_info());

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Saternal Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await?;

        let size = window.as_ref().inner_size();

        // Get the preferred surface format
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Choose the best supported alpha mode
        let alpha_mode =
            if surface_caps
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
            {
                wgpu::CompositeAlphaMode::PostMultiplied
            } else if surface_caps
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
            {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else {
                surface_caps.alpha_modes[0]
            };

        info!(
            "Using surface format: {:?}, alpha mode: {:?}",
            surface_format, alpha_mode
        );

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo, // VSync
            desired_maximum_frame_latency: 2,
            alpha_mode,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        Ok(Self {
            device,
            queue,
            surface,
            config,
            _window: window, // Must be last to ensure correct drop order
        })
    }
}
