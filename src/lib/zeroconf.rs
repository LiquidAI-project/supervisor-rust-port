use serde::Serialize;
use std::env;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use libmdns::{Responder as MdnsResponder, Service};
use reqwest::Client;
use log::{info, debug, error};

use crate::lib::constants::{
    DEFAULT_URL_SCHEME, 
    SUPERVISOR_DEFAULT_NAME, 
    URL_BASE_PATH, 
    DEFAULT_PORT
};


pub struct WebthingZeroconf {
    pub service_name: String,
    pub service_type: String,
    pub host: String,
    pub port: u16,
    pub properties: Vec<(String, String)>,
    pub registration: Option<Service>,
}

impl WebthingZeroconf {
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
        let service_name = env::var("SUPERVISOR_NAME").unwrap_or_else(|_| SUPERVISOR_DEFAULT_NAME.to_string());
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
            registration: None,
        }
    }

    /// Register supervisor using libmdns
    pub fn register_service(&mut self, responder: &MdnsResponder) {
        let txt_records: Vec<String> = self.properties
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        let service = responder.register(
            self.service_type.clone(),
            self.service_name.clone(),
            self.port,
            &txt_records.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        );

        self.registration = Some(service);
        info!("Supervisor with name '{}' registered succesfully.", &self.service_name);
    }
}

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

/// Spawns a background thread that tries to connect to the Actix server (host:port).
/// Once connected, we register supervisor.
pub fn wait_until_ready_and_register(mut zc: WebthingZeroconf) {
    thread::spawn(move || {
        let responder = MdnsResponder::new().expect("Failed to create mDNS responder");
        let addr = format!("{}:{}", zc.host, zc.port);
        loop {
            match TcpStream::connect(&addr) {
                Ok(_) => {
                    debug!("Server is ready at {}", addr);
                    zc.register_service(&responder);
                    break;
                }
                Err(err) => {
                    debug!("Waiting for server at {}: {:?}", addr, err);
                    thread::sleep(Duration::from_secs(1));
                }
            }
            break;
        }
        if let Ok(mut orchestrator_url) = env::var("WASMIOT_ORCHESTRATOR_URL") {
            orchestrator_url.push_str(URL_BASE_PATH);
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(async {
                register_services_to_orchestrator(&zc, &orchestrator_url).await
            });
            if let Err(e) = result {
                error!("Failed to register to orchestrator: {}", e);
            }
        } else {
            debug!("No ORCHESTRATOR_URL set, skipping orchestrator registration.");
        }
    });
}

/// Check env variables or use defaults.
pub fn get_listening_address() -> (String, u16) {
    let host = env::var("WASMIOT_SUPERVISOR_IP").expect("Error trying to read enviroment variable WASMIOT_SUPERVISOR_IP");
    let port_str = env::var("WASMIOT_SUPERVISOR_PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let port: u16 = port_str.parse().unwrap_or(DEFAULT_PORT);
    (host, port)
}