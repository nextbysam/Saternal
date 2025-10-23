mod app;
mod tab;

use anyhow::Result;
use log::info;

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Saternal - The blazing fast dropdown terminal");
    info!("Press Cmd+` to toggle the terminal");

    // Load configuration
    let config = saternal_core::Config::load(None)?;
    info!("Loaded configuration: {:?}", config);

    // Create and run the application using pollster to block on async initialization
    let app = pollster::block_on(app::App::new(config))?;
    app.run()?;

    Ok(())
}
