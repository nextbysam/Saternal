use anyhow::Result;
use log::info;
use wgpu;

/// GPU context including device, queue, surface, and configuration
/// 
/// Safety: Uses 'static lifetime for Surface. The caller must ensure
/// the Window remains valid for the Surface's lifetime.
pub(crate) struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

impl GpuContext {
    /// Initialize GPU context with wgpu/Metal
    /// 
    /// # Safety
    /// 
    /// The returned Surface has a 'static lifetime, but it's created from a borrowed Window.
    /// The caller MUST ensure that:
    /// - The Window remains valid for the entire lifetime of the returned GpuContext
    /// - The Window is not dropped while the Surface is still in use
    /// 
    /// In our case, this is guaranteed because:
    /// - Window is owned by App wrapped in Arc<Window>
    /// - GpuContext (and thus Renderer) is also wrapped in Arc
    /// - Both are kept alive for the application's entire lifetime
    /// - The event loop consumes and holds these Arc references
    pub async fn new(window: &winit::window::Window) -> Result<Self> {
        info!("Initializing GPU renderer with Metal backend");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL, // Force Metal on macOS
            ..Default::default()
        });

        // Create surface with temporary lifetime
        let surface_temp = instance.create_surface(window)?;
        
        // SAFETY: We transmute the surface lifetime from the borrowed window lifetime to 'static.
        // This is sound because:
        // 1. The Window is owned by App and wrapped in Arc<Window>
        // 2. The GpuContext/Renderer is also wrapped in Arc<Mutex<Renderer<'static>>>
        // 3. Both Arcs are kept alive for the application's entire lifetime
        // 4. The Window will not be dropped while the Surface is in use
        // 5. The event loop holds Arc clones of both, ensuring they live until app exit
        let surface: wgpu::Surface<'static> = unsafe {
            std::mem::transmute(surface_temp)
        };

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

        let size = window.inner_size();

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
        })
    }
}
