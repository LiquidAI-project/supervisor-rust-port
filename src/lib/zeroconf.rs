//! # zeroconf.rs
//!
//! This module handles zero-configuration networking for the Wasm supervisor service.
//!
//! It is responsible for:
//! - Determining the local host and port the supervisor is running on
//! - Building and managing a service identity (`WebthingZeroconf`)
//! - Registering that service with a remote orchestrator, if configured
//! - Advertising the service with mDNS
//!
//! This allows services to self-register into the orchestrator.


use serde::Serialize;
use std::env;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use reqwest::Client;
use log::{error, debug, info};
use local_ip_address;
use actix_web::rt::System;
use crate::lib::constants::{
    DEFAULT_URL_SCHEME,
    SUPERVISOR_DEFAULT_NAME,
    URL_BASE_PATH,
    DEFAULT_PORT
};
use zeroconf::prelude::*;
use zeroconf::{MdnsService, ServiceType, TxtRecord};


/// Represents a service that is advertised on the network.
///
/// Includes details such as:
/// - Name and service type (e.g. `_webthing._tcp`)
/// - Host IP and port
/// - Optional service metadata (`properties`) such as TLS info
#[derive(Debug, Serialize, Clone)]
pub struct WebthingZeroconf {
    pub service_name: String,
    pub service_type: String,
    pub service_protocol: String,
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

        // service name = supervisor._webthing._tcp.local.
        let service_type = "webthing".to_string();
        let service_protocol = "tcp".to_string();
        let service_name = env::var("SUPERVISOR_NAME")
            .unwrap_or_else(|_| SUPERVISOR_DEFAULT_NAME.to_string())
            + "._" + &service_type + "._" + &service_protocol + ".local.";

        let properties = vec![
            ("path".to_string(), "/".to_string()),
            ("tls".to_string(), tls_flag.to_string()),
        ];
        WebthingZeroconf {
            service_name,
            service_type,
            service_protocol,
            host,
            port,
            properties,
        }
    }
}

/// Payload structure used when sending service registration info to orchestrator.
#[derive(Debug, Serialize, Clone)]
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

/// Force registration of the supervisor to orchestrator.
/// Spawns a background thread that waits for the supervisor is ready
/// before sending the registration to orchestrator.
/// Requires the following env variables to be set in .env file:
///   - WASMIOT_ORCHESTRATOR_URL
pub fn force_supervisor_registration(zc: WebthingZeroconf) {
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

/// Sends a service registration POST request to the orchestrator.
/// This is the "manual" version of device discovery
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

    info!("Sending registration to: {}", orchestrator_url);
    info!("Payload: {:?}", data);

    let client = Client::new();
    let req = client
    .post(orchestrator_url)
    .json(&data)
    .timeout(Duration::from_secs(10));

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to send registration request: {}", e);
            return Err(anyhow::anyhow!("Request error: {:?}", e));
        }
    };

    if !resp.status().is_success() {
        let text = resp.text().await?;
        error!("Failed to register service: {}", text);
        anyhow::bail!("Orchestrator returned error: {}", text);
    }

    debug!("Service registered to orchestrator: {:?}", data);
    Ok(())
}

/// Waits for supervisor to be up, the starts listening to mdns requests
///
/// Spawns a background thread that:
/// - Repeatedly tries to connect to the supervisor
/// - Once supervisor is reachable, starts listening to mdns requests from the orchestrator
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

        // Advertise the service using mDNS
        // print the error if it fails
        if let Err(e) = register_service(zc) {
            error!("Failed to start mDNS listener: {}", e);
        } else {
            info!("Mdns listener started succesfully.");
        }


    });
}

/// Determines the IP address and port this supervisor instance should bind to.
/// Defaults to 127.0.0.1 and port 8080
/// 
/// Reads:
/// - `WASMIOT_SUPERVISOR_PORT` (falls back to default 8080)
pub fn get_listening_address() -> (String, u16) {
    let host = local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "127.0.0.1".to_string());

    let port_str = env::var("WASMIOT_SUPERVISOR_PORT")
        .unwrap_or_else(|_| DEFAULT_PORT.to_string());

    let port: u16 = port_str.parse().unwrap_or(DEFAULT_PORT);
    (host, port)
}

/// Spawn a separate thread that continously listens for mdns requests, and 
/// responds with supervisor data when requested.
pub fn register_service(zc: WebthingZeroconf) -> anyhow::Result<()> {
    std::thread::spawn(move || {
        let service_type = ServiceType::new(zc.service_type.as_str(), zc.service_protocol.as_str()).unwrap();
        let mut service = MdnsService::new(service_type, zc.port);
        let mut txt_record = TxtRecord::new();
        zc.properties
            .iter()
            .for_each(|(key, value)| {
                txt_record.insert(key, value).unwrap();
            });
        service.set_name(&zc.service_name);
        service.set_txt_record(txt_record);

        service.set_registered_callback(Box::new(|r, _| {
            if let Ok(svc) = r {
                info!("✅ Responded to mdns query with: {:?}", svc);
            }
        }));

        let event_loop = service.register().unwrap();
        loop {
            event_loop.poll(Duration::from_secs(0)).unwrap();
        }
    });
    Ok(())
}
