mod app;
mod tab;

use anyhow::Result;
use log::info;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting Saternal - The blazing fast dropdown terminal");
    info!("Press Cmd+` to toggle the terminal");

    // Load configuration
    let config = saternal_core::Config::load(None)?;
    info!("Loaded configuration: {:?}", config);

    // Create and run the application
    let app = app::App::new(config).await?;
    app.run().await?;

    Ok(())
}
