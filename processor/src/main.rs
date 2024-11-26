mod config;
mod processor;

use crate::processor::*;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use crate::config::config::load_config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load config at startup
    let config = match load_config() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    let processor = BatchProcessor::new(config)?;
    processor.run().await?;

    Ok(())
}