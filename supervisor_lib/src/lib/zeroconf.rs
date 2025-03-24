//! # zeroconf.rs
//!
//! This module handles zero-configuration networking for the Wasm supervisor service.
//!
//! It is responsible for:
//! - Determining the local host and port the supervisor is running on
//! - Building and managing a service identity (`WebthingZeroconf`)
//! - Registering that service with a remote orchestrator, if configured
//!
//! This allows services to self-register into the orchestrator.


use serde::Serialize;
use std::env;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use reqwest::Client;
use log::{error, debug};
use local_ip_address;
use actix_web::rt::System;
use crate::lib::constants::{
    DEFAULT_URL_SCHEME, 
    SUPERVISOR_DEFAULT_NAME, 
    URL_BASE_PATH, 
    DEFAULT_PORT
};


/// Represents a service that is advertised on the network.
///
/// Includes details such as:
/// - Name and service type (e.g. `_webthing._tcp`)
/// - Host IP and port
/// - Optional service metadata (`properties`) such as TLS info
pub struct WebthingZeroconf {
    pub service_name: String,
    pub service_type: String,
    pub host: String,
    pub port: u16,
    pub properties: Vec<(String, String)>,
}

impl WebthingZeroconf {
    /// Constructs a new service representation using env vars or defaults.
    ///
    /// Populates host and port using `get_listening_address()`, reads environment variables
    /// like `PREFERRED_URL_SCHEME` and `SUPERVISOR_NAME`, and sets standard `_webthing._tcp`
    /// service type.
    pub fn new() -> Self {
        let (host, port) = get_listening_address();

        let preferred_url_scheme = env::var("PREFERRED_URL_SCHEME")
            .unwrap_or_else(|_| DEFAULT_URL_SCHEME.to_string());
        let tls_flag = if preferred_url_scheme.to_lowercase() == "https" {
            "1"
        } else {
            "0"
        };

        let service_type = "_webthing._tcp".to_string();
        let service_name = env::var("SUPERVISOR_NAME")
            .unwrap_or_else(|_| SUPERVISOR_DEFAULT_NAME.to_string());

        let properties = vec![
            ("path".to_string(), "/".to_string()),
            ("tls".to_string(), tls_flag.to_string()),
        ];

        WebthingZeroconf {
            service_name,
            service_type,
            host,
            port,
            properties,
        }
    }
}

/// Payload structure used when sending service registration info to orchestrator.
#[derive(Debug, Serialize)]
pub struct ZeroconfRegistrationData<'a> {
    #[serde(rename = "name")]
    name: &'a str,
    #[serde(rename = "type")]
    kind: &'a str,
    port: u16,
    properties: serde_json::Value,
    addresses: Vec<String>,
    host: String,
}

/// Sends a service registration POST request to the orchestrator.
///
/// Converts the `WebthingZeroconf` instance into the proper payload and sends it
/// to the configured `orchestrator_url`. Logs and returns errors if any occur.
pub async fn register_services_to_orchestrator(
    zc: &WebthingZeroconf,
    orchestrator_url: &str,
) -> anyhow::Result<()> {
    let mut props_map = serde_json::Map::new();
    for (k, v) in &zc.properties {
        props_map.insert(k.clone(), serde_json::json!(v));
    }

    let data = ZeroconfRegistrationData {
        name: &zc.service_name,
        kind: &zc.service_type,
        port: zc.port,
        properties: serde_json::Value::Object(props_map),
        addresses: vec![zc.host.clone()],
        host: zc.host.clone(),
    };

    let client = Client::new();
    let resp = client
        .post(orchestrator_url)
        .json(&data)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await?;
        error!("Failed to register service: {}", text);
        anyhow::bail!("Orchestrator returned error: {}", text);
    }

    debug!("Service registered to orchestrator: {:?}", data);
    Ok(())
}

/// Waits for the local supervisor server to be up, then registers the service.
///
/// Spawns a background thread that:
/// - Repeatedly tries to connect to the server address
/// - Once reachable, attempts orchestrator registration (if env var is set)
pub fn wait_until_ready_and_register(zc: WebthingZeroconf) {
    thread::spawn(move || {
        let addr = format!("{}:{}", zc.host, zc.port);

        loop {
            match TcpStream::connect(&addr) {
                Ok(_) => {
                    debug!("Server is ready at {}", addr);
                    break;
                }
                Err(err) => {
                    debug!("Waiting for server at {}: {:?}", addr, err);
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        if let Ok(mut orchestrator_url) = env::var("WASMIOT_ORCHESTRATOR_URL") {
            orchestrator_url.push_str(URL_BASE_PATH);

            let result = System::new().block_on(async {
                register_services_to_orchestrator(&zc, &orchestrator_url).await
            });

            if let Err(e) = result {
                error!("Failed to register to orchestrator: {}", e);
            }
        } else {
            debug!("No WASMIOT_ORCHESTRATOR_URL set, skipping orchestrator registration.");
        }
    });
}

/// Determines the IP address and port this supervisor instance should bind to.
///
/// Reads:
/// - `WASMIOT_SUPERVISOR_IP` (falls back to local IP or `127.0.0.1`)
/// - `WASMIOT_SUPERVISOR_PORT` (falls back to default 8080)
pub fn get_listening_address() -> (String, u16) {
    let host = env::var("WASMIOT_SUPERVISOR_IP").unwrap_or_else(|_| {
        local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "127.0.0.1".to_string())
    });

    let port_str = env::var("WASMIOT_SUPERVISOR_PORT")
        .unwrap_or_else(|_| DEFAULT_PORT.to_string());

    let port: u16 = port_str.parse().unwrap_or(DEFAULT_PORT);
    (host, port)
}