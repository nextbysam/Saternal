mod app;
mod tab;

use anyhow::Result;
use log::info;

fn main() -> Result<()> {
    // Load environment variables from .env file
    if let Err(e) = dotenvy::dotenv() {
        // .env file is optional - only log if it's a real error (not just missing)
        if !e.to_string().contains("not found") {
            log::warn!("Error loading .env file: {}", e);
        }
    }

    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Saternal - The blazing fast dropdown terminal");
    info!("Press Cmd+` to toggle the terminal");

    // Load configuration
    let config = saternal_core::Config::load(None)?;
    info!("Loaded configuration: {:?}", config);

    // Create tokio runtime for async operations
    // We keep the runtime alive for the entire application lifetime
    let runtime = tokio::runtime::Runtime::new()?;
    let handle = runtime.handle().clone();
    
    // Enter the runtime context so tokio operations work throughout the app
    let _guard = runtime.enter();

    // Create and run the application using pollster to block on async initialization
    let app = pollster::block_on(app::App::new(config, handle))?;
    app.run()?;

    Ok(())
}
