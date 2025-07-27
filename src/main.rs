use anyhow::Result;
use clap::Parser;
use log::{info, warn};
use std::sync::Arc;
use tokio::runtime::Runtime;
use eframe::egui;

mod terminal;
mod ai;
mod security;
mod file_explorer;
mod autocomplete;
mod ui;

use ui::AnTraftApp;

#[derive(Parser, Debug)]
#[command(name = "antraft")]
#[command(about = "Next-gen AI-powered terminal application", long_about = None)]
struct Args {
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
    
    /// Configuration file path
    #[arg(short, long)]
    config: Option<String>,
    
    /// Working directory
    #[arg(short = 'w', long)]
    directory: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    if args.debug {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }
    
    info!("🚀 Starting ANTRAFT - Next-gen AI Terminal");
    
    // Set working directory if specified
    if let Some(dir) = args.directory {
        std::env::set_current_dir(&dir)?;
        info!("Changed working directory to: {}", dir);
    }
    
    // Launch the GUI application
    info!("🚀 Launching ANTRAFT GUI...");
    
    let config = ui::Config::default();
    let app = AnTraftApp::new(config).await?;
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("ANTRAFT - AI Terminal"),
        ..Default::default()
    };
    
    eframe::run_native(
        "ANTRAFT - AI Terminal", 
        options, 
        Box::new(|_cc| Box::new(app))
    ).map_err(|e| {
        log::error!("Failed to run GUI: {}", e);
        anyhow::anyhow!("GUI launch failed: {}", e)
    })?;
    
    Ok(())
}
