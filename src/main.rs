//! # Supervisor Entry Point
//!
//! This is the main executable entry point for the Wasmiot supervisor.
//!
//! This performs the following startup tasks:
//! - Initializes loggers and instance directories
//! - Starts the Actix-Web server for HTTP endpoints
//! - Registers the device with Zeroconf (mDNS/Bonjour)
//! - Spawns a background worker thread for executing WebAssembly tasks asynchronously

use actix_web::web;
use actix_web::{App, HttpServer, web::Data};
use actix_cors::Cors;
use log::info;
use parking_lot::Mutex;
use supervisor::api::deployment::{deployment_create, deployment_delete, deployment_get};
use supervisor::api::device::{register_orchestrator, thingi_description, thingi_health, wasmiot_device_description};
use supervisor::api::results::{get_module_result, request_history_list, request_history_list_1};
use supervisor::api::run::{run_module_function, run_module_function_3};
use supervisor::structs::deployment_supervisor::Deployment;
use std::sync::Arc;
use supervisor::lib::{zeroconf, constants};
use supervisor::lib::constants::{DEPLOYMENTS, DEPLOYMENTS_FOLDER};

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
        Ok(path) => println!("Loaded .env from {:?}", path),
        Err(err) => println!("Could not load .env file: {:?}", err),
    }

    // Ensure required folders like `params/` and `modules/` exist
    constants::ensure_required_folders();

    // Initialize logging with default level = info (unless overridden by env)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Ensure that the environment variable for supervisor name is set
    // - order: SUPERVISOR_NAME, WASMIOT_SUPERVISOR_NAME, or the default name
    if std::env::var("SUPERVISOR_NAME").is_err() {
        unsafe {
            std::env::set_var(
                "SUPERVISOR_NAME",
                match std::env::var("WASMIOT_SUPERVISOR_NAME") {
                    Ok(name) => name,
                    Err(_) => constants::SUPERVISOR_DEFAULT_NAME.to_string(),
                }
            );
        }

    }
    info!("Supervisor name: {}", std::env::var("SUPERVISOR_NAME").unwrap());

    // Start Zeroconf discovery and determine host/port
    let zc = zeroconf::WebthingZeroconf::new();
    let (host, port) = (zc.host.clone(), zc.port);
    info!("host:{}, port:{}", host, port);
    unsafe {
        std::env::set_var("WASMIOT_SUPERVISOR_IP", &host);
        std::env::set_var("DEFAULT_URL_SCHEME", "http");
    }

    let zc_arc = Arc::new(Mutex::new(zc.clone()));
    // Wait for the server to be ready before advertising over Zeroconf
    zeroconf::wait_until_ready_and_register(zc_arc.clone());
    // Force registration of the supervisor with the orchestrator with HTTP
    // if the WASMIOT_ORCHESTRATOR_URL environment variable is set.
    zeroconf::force_supervisor_registration(zc_arc.clone());

    // Before initializing the server, load the currently existing deployments into memory
    if let Err(e) = std::fs::create_dir_all(&*DEPLOYMENTS_FOLDER) {
        log::error!(
            "Failed to create deployments folder {}: {}",
            DEPLOYMENTS_FOLDER.display(),
            e
        );
    } else if let Ok(entries) = std::fs::read_dir(&*DEPLOYMENTS_FOLDER) {
        for entry_res in entries {
            let Ok(entry) = entry_res else { continue };
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("<unknown>");
            let contents = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    log::error!(
                        "Failed to read deployment JSON file {}: {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };
            let mut deployment: Deployment = match serde_json::from_str(&contents) {
                Ok(d) => d,
                Err(e) => {
                    log::error!(
                        "Failed to deserialize deployment JSON from {}: {}",
                        path.display(),
                        e
                    );
                    continue;
                }
            };
            deployment.init();
            let id = deployment.id.clone();
            {
                let mut deps = DEPLOYMENTS.lock();
                deps.insert(id.clone(), deployment);
            }
            log::info!("Loaded saved deployment '{}' from {}", id, file_name);
        }
    }

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
        .app_data(Data::new(zc_arc.clone()))  // Pass the Zeroconf instance to the app
        .service(
                web::resource("/.well-known/wasmiot-device-description")
                    .name("/.well-known/wasmiot-device-description")
                    .route(web::get().to(wasmiot_device_description))
            )

            // Returns W3C WoT Thing Description for this device
            .service(
                web::resource("/.well-known/wot-thing-description")
                    .name("/.well-known/wot-thing-description")
                    .route(web::get().to(thingi_description))
            )

            // Health check for device (CPU, memory, network)
            .service(
                web::resource("/health")
                    .name("/health")
                    .route(web::get().to(thingi_health))
            )
            // Duplicate health route for compatibility
            .service(
                web::resource("//health")
                    .name("//health")
                    .route(web::get().to(thingi_health))
            )

            // Registers the active orchestrator URL to the device
            .service(
                web::resource("/register")
                    .name("/register")
                    .route(web::post().to(register_orchestrator))
            )

            // Fetch result files generated by module execution
            .service(
                web::resource("/module_results/{deployment_id}/{module_name}/{filename}")
                    .name("/module_results/{deployment_id}/{module_name}/{filename}")
                    .route(web::get().to(get_module_result))
            )

            // Fetch execution history (entire list or single entry by ID)
            .service(
                web::resource("/request-history/{request_id}")
                    .name("/request-history/{request_id}")
                    .route(web::get().to(request_history_list))
            )
            .service(
                web::resource("/request-history")
                    .name("/request-history")
                    .route(web::get().to(request_history_list_1))
            )

            // Serve result file produced by specific function execution
            .service(
                web::resource("/{deployment_id}/modules/{module_name}/{function_name}/{filename}")
                    .name("/{deployment_id}/modules/{module_name}/{function_name}/{filename}")
                    .route(web::get().to(run_module_function))
            )

            // Run a module function (GET: immediate execution, no input files)
            // Run a module function (POST: allows input files via multipart)
            .service(
                web::resource("/{deployment_id}/modules/{module_name}/{function_name}")
                    .name("/{deployment_id}/modules/{module_name}/{function_name}")
                    .route(web::get().to(run_module_function_3))
                    .route(web::post().to(run_module_function_3))
            )

            // Delete an existing deployment by ID
            .service(
                web::resource("/deploy/{deployment_id}")
                    .name("/deploy/{deployment_id}")
                    .route(web::delete().to(deployment_delete))
            )

            // Get a list of all deployments currently active on this device
            // Create a new deployment with modules and optional mount/config data
            .service(
                web::resource("/deploy")
                    .name("/deploy")
                    .route(web::get().to(deployment_get))
                    .route(web::post().to(deployment_create))
            )
            // This is needed for current version of orchestrator for some reason
            .service(
                web::resource("//deploy")
                    .name("//deploy")
                    .route(web::post().to(deployment_create))
            )
    })
    .bind(("0.0.0.0", port))?;
    info!("Starting supervisor service at http://{}:{}/", host, port);
    server.run().await
}
