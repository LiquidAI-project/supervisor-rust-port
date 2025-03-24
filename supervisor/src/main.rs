//! # Supervisor Entry Point (Non-ARM32)
//!
//! This is the main executable entry point for the full version Wasm supervisor.
//!
//! It performs the following startup tasks:
//! - Initializes loggers and instance directories
//! - Starts the Actix-Web server for HTTP endpoints
//! - Registers the device with Zeroconf (mDNS/Bonjour)
//! - Spawns a background worker thread for executing WebAssembly tasks asynchronously

use actix_web::{App, HttpServer};
use log::info;
use supervisor_lib::lib::{api, zeroconf, constants};

/// Main entry point for the supervisor service.
///
/// Initializes required folders, logging, networking, HTTP server, and the background
/// WebAssembly execution worker.
///
/// # Returns
/// - `Ok(())` if the supervisor runs successfully
/// - Any `std::io::Error` that occurs during HTTP server setup
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Ensure required folders like `params/` and `modules/` exist
    constants::ensure_required_folders();

    // Initialize logging with default level = info (unless overridden by env)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Start Zeroconf discovery and determine host/port
    let zc = zeroconf::WebthingZeroconf::new();
    let (host, port) = (zc.host.clone(), zc.port);
    info!("host:{}, port:{}", host, port);

    // Initialize and bind the HTTP server
    let server = HttpServer::new(move || {
        App::new().configure(api::configure_routes)
    })
    .bind(("0.0.0.0", port))?;

    info!("Starting supervisor service at http://{}:{}/", host, port);

    // Wait for the server to be ready before advertising over Zeroconf
    zeroconf::wait_until_ready_and_register(zc);

    // Launch the background WebAssembly execution thread
    std::thread::spawn(|| {
        api::wasm_worker();
    });

    // Run the Actix-Web server
    server.run().await
}
