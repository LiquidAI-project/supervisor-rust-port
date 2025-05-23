//! # Supervisor Entry Point
//!
//! This is the main executable entry point for the Wasmiot supervisor.
//!
//! This performs the following startup tasks:
//! - Initializes loggers and instance directories
//! - Starts the Actix-Web server for HTTP endpoints
//! - Registers the device with Zeroconf (mDNS/Bonjour)
//! - Spawns a background worker thread for executing WebAssembly tasks asynchronously

use actix_web::{App, HttpServer};
use actix_cors::Cors;
use log::{info, warn};
use supervisor::lib::{api, zeroconf, constants};

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
    // Load environment variables from .env if present
    match dotenv::dotenv() {
        Ok(path) => info!("Loaded .env from {:?}", path),
        Err(err) => warn!("Could not load .env file: {:?}", err),
    }

    // Ensure required folders like `params/` and `modules/` exist
    constants::ensure_required_folders();

    // Initialize logging with default level = info (unless overridden by env)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Start Zeroconf discovery and determine host/port
    let zc = zeroconf::WebthingZeroconf::new();
    let (host, port) = (zc.host.clone(), zc.port);
    info!("host:{}, port:{}", host, port);
    std::env::set_var("WASMIOT_SUPERVISOR_IP", &host);
    std::env::set_var("DEFAULT_URL_SCHEME", "http");

    // Wait for the server to be ready before advertising over Zeroconf
    zeroconf::wait_until_ready_and_register(zc);

    // Initialize the HTTP server.
    let server = HttpServer::new(move || {
        App::new()
        .wrap(
            Cors::default()
                .allow_any_origin() // Or .allowed_origin("http://localhost:3314")
                .allow_any_method()
                .allow_any_header()
                .max_age(3600)
        )
        .wrap(
            actix_web::middleware::Logger::default()
        )
        .configure(api::configure_routes)
    })
    .bind(("0.0.0.0", port))?;
    info!("Starting supervisor service at http://{}:{}/", host, port);
    server.run().await
}
