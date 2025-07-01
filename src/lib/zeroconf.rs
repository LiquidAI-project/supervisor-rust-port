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


use parking_lot::Mutex;
use serde::Serialize;
use tokio::runtime::Runtime;
use std::env;
use std::net::TcpStream;
use std::sync::Arc;
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
    DEFAULT_SERVICE_RENEWAL_TIME,
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
    pub register_renewal_time: i64,
    pub last_register_time: i64,
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
            .unwrap_or_else(|_| SUPERVISOR_DEFAULT_NAME.to_string());

        let properties = vec![
            ("path".to_string(), "/".to_string()),
            ("tls".to_string(), tls_flag.to_string()),
            ("address".to_string(), host.clone()),
        ];
        let register_renewal_time = match env::var("WASMIOT_REGISTER_RENEWAL_TIME") {
            Ok(val) => val.parse().unwrap_or(DEFAULT_SERVICE_RENEWAL_TIME),
            Err(_) => DEFAULT_SERVICE_RENEWAL_TIME,
        };
        WebthingZeroconf {
            service_name,
            service_type,
            service_protocol,
            host,
            port,
            properties,
            register_renewal_time,
            last_register_time: chrono::Utc::now().timestamp(),
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
pub fn force_supervisor_registration(zc: Arc<Mutex<WebthingZeroconf>>) {
    thread::spawn(move || {
        if let Ok(mut orchestrator_url) = env::var("WASMIOT_ORCHESTRATOR_URL") {
            let zc_lock = zc.lock();
            let addr = format!("{}:{}", zc_lock.host, zc_lock.port);
            drop(zc_lock);

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

            orchestrator_url.push_str(URL_BASE_PATH);
            let result = System::new().block_on(async {
                register_services_to_orchestrator(zc, &orchestrator_url).await
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
    zc: Arc<Mutex<WebthingZeroconf>>,
    orchestrator_url: &str,
) -> anyhow::Result<()> {
    let mut props_map = serde_json::Map::new();
    let zc_lock = zc.lock();
    for (k, v) in &zc_lock.properties {
        props_map.insert(k.clone(), serde_json::json!(v));
    }

    let data = ZeroconfRegistrationData {
        name: &zc_lock.clone().service_name,
        kind: &zc_lock.clone().service_type,
        port: zc_lock.port,
        properties: serde_json::Value::Object(props_map),
        addresses: vec![zc_lock.host.clone()],
        host: zc_lock.host.clone(),
    };
    drop(zc_lock);

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
pub fn wait_until_ready_and_register(zc: Arc<Mutex<WebthingZeroconf>>) {
    thread::spawn(move || {
        {
            let zc_lock = zc.lock();
            let addr = format!("{}:{}", zc_lock.host, zc_lock.port);
            drop(zc_lock);

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
        }

        // Advertise the service using mDNS
        // print the error if it fails
        if let Err(e) = register_service(zc.clone()) {
            error!("Failed to start mDNS listener: {}", e);
        } else {
            info!("mDNS listener started successfully.");
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

/// Spawn a separate thread that continuously listens for mdns requests, and
/// responds with supervisor data when requested.
pub fn register_service(zc: Arc<Mutex<WebthingZeroconf>>) -> anyhow::Result<()> {
    let zc_clone = zc.clone();

    std::thread::spawn(move || {
        let zc_clone = zc_clone.clone();
        let zc_lock = zc_clone.lock();
        let service_type = ServiceType::new(zc_lock.service_type.as_str(), zc_lock.service_protocol.as_str()).unwrap();
        let mut service = MdnsService::new(service_type, zc_lock.port);
        let mut txt_record = TxtRecord::new();
        zc_lock.properties
            .iter()
            .for_each(|(key, value)| {
                txt_record.insert(key, value).unwrap();
            });
        service.set_name(&zc_lock.service_name);
        drop(zc_lock);
        service.set_txt_record(txt_record);

        service.set_registered_callback(Box::new(|r, _| {
            if let Ok(svc) = r {
                info!("✅ Responded to mDNS query with: {:?}", svc);
            }
        }));

        let event_loop = service.register().unwrap();
        loop {
            event_loop.poll(Duration::from_secs(1)).unwrap();

            let zc_lock = zc_clone.lock();
            let time_since_last_register = chrono::Utc::now().timestamp() - zc_lock.last_register_time;
            let time_check = time_since_last_register > zc_lock.register_renewal_time;
            drop(zc_lock);

            if time_check {
                info!("Health check timeout exceeded, re-registering service");
                break;
            }
        }

        match Runtime::new() {
            Ok(rt) => rt.block_on(async move {
                update_service_registration(zc.clone()).await;
            }),
            Err(e) => {
                error!("Failed to create Tokio runtime: {}", e);
            }
        }
    });
    Ok(())
}

pub fn register_health_check(zc: Arc<Mutex<WebthingZeroconf>>) {
    let mut zc_lock = zc.lock();
    zc_lock.last_register_time = chrono::Utc::now().timestamp();
}

pub async fn update_service_registration(zc: Arc<Mutex<WebthingZeroconf>>) {
    tokio::time::sleep(Duration::from_secs(5)).await;
    register_health_check(zc.clone());
    wait_until_ready_and_register(zc.clone());
    force_supervisor_registration(zc);
}
