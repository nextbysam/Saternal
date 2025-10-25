use anyhow::Result;
use log::info;
use wgpu;

/// GPU context including device, queue, surface, and configuration
/// 
/// Safety: Uses 'static lifetime for Surface. The caller must ensure
/// the Window remains valid for the Surface's lifetime.
pub(crate) struct GpuContext<'static> {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
}

impl GpuContext<'static> {
    /// Initialize GPU context with wgpu/Metal
    pub async fn new(window: &winit::window::Window) -> Result<Self> {
        info!("Initializing GPU renderer with Metal backend");

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL, // Force Metal on macOS
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

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
