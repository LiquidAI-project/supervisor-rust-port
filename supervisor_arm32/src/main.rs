//! # Supervisor Entry Point (ARM32)
//!
//! This is the entry point for running the Wasm supervisor on ARM32 devices (e.g., Raspberry Pi).
//!
//! Due to possible hardware constraints, this version does **not** spawn a separate background thread
//! for WebAssembly execution, and limits Actix-Web to a single worker thread.
//!
//! It performs the following startup tasks:
//! - Creates required instance folders (e.g. `params/`, `modules/`)
//! - Initializes the logger
//! - Registers the device using Zeroconf (Bonjour/mDNS)
//! - Starts the Actix-Web server with all available API endpoints

use actix_web::{App, HttpServer};
use log::info;
use supervisor_lib::lib::{api, zeroconf, constants};

/// Main entry point for the ARM32 variant of the supervisor.
///
/// This version avoids multithreading due to hardware limitations.
/// WebAssembly function execution is performed synchronously inside request handlers.
///
/// # Returns
/// - `Ok(())` if the server starts and runs successfully
/// - `std::io::Error` if the HTTP server fails to bind or run
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Ensure required folders like `params/` and `modules/` exist
    constants::ensure_required_folders();

    // Initialize logging (defaults to INFO level unless overridden via env)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Small delay to allow the network to initialize (especially on slower Pi boots)
    std::thread::sleep(std::time::Duration::from_millis(1000));

    // Start Zeroconf/mDNS service and obtain host/port
    let zc = zeroconf::WebthingZeroconf::new();
    let (host, port) = (zc.host.clone(), zc.port);
    info!("host:{}, port:{}", host, port);

    // Start the Actix-Web server with a single worker thread (low resource mode)
    let server = HttpServer::new(move || {
        App::new().configure(api::configure_routes)
    })
    .workers(1)
    .bind(("0.0.0.0", port))?;

    info!("Starting supervisor service at http://{}:{}/", host, port);

    // Wait for server readiness before advertising it on the local network
    zeroconf::wait_until_ready_and_register(zc);

    // Run the Actix-Web server
    server.run().await
}
