use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use serde_json::json;

use std::env;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;

use libmdns::{Responder as MdnsResponder, Service};
use reqwest::Client;
use log::{info, debug, error, LevelFilter};

// ------------------ Constants / Defaults ------------------ //
static DEFAULT_PORT: u16 = 8080;
static DEFAULT_URL_SCHEME: &str = "http";
static URL_BASE_PATH: &str = "/file/device/discovery/register";
static SUPERVISOR_DEFAULT_NAME: &str = "supervisor";

// ------------------ get_listening_address (Rust version) ------------------ //

/// Equivalent to Python's `get_listening_address(app)`.
/// We check env variables or use defaults.
fn get_listening_address() -> (String, u16) {
    let host = env::var("WASMIOT_SUPERVISOR_IP").expect("Error trying to read enviroment variable WASMIOT_SUPERVISOR_IP");
    let port_str = env::var("WASMIOT_SUPERVISOR_PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let port: u16 = port_str.parse().unwrap_or(DEFAULT_PORT);
    (host, port)
}

// ------------------ WebthingZeroconf Struct ------------------ //

/// Omit `#[derive(Debug)]` because `libmdns::Service` does not implement `Debug`.
struct WebthingZeroconf {
    service_name: String,
    service_type: String,
    host: String,
    port: u16,
    properties: Vec<(String, String)>,
    registration: Option<Service>,
}

impl WebthingZeroconf {
    fn new() -> Self {
        // 1) Decide on host/port from environment or defaults
        let (host, port) = get_listening_address();

        // Check TLS via PREFERRED_URL_SCHEME
        let preferred_url_scheme = env::var("PREFERRED_URL_SCHEME")
            .unwrap_or_else(|_| DEFAULT_URL_SCHEME.to_string());
        let tls_flag = if preferred_url_scheme.to_lowercase() == "https" {
            "1"
        } else {
            "0"
        };

        // Typically for mDNS, a service type looks like "_webthing._tcp"
        // (no ".local." suffix).
        let service_type = "_webthing._tcp".to_string();

        // For the service name, Python used <app.name>._webthing._tcp.local.
        // We can just do something like "<appname>.local." to mimic that:
        let app_name = env::var("SUPERVISOR_NAME").unwrap_or_else(|_| SUPERVISOR_DEFAULT_NAME.to_string());
        let service_name = format!("{}.local.", app_name);

        // Some arbitrary key-value properties
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

    /// Register (announce) our service via libmdns.
    fn register_service(&mut self, responder: &MdnsResponder) {
        // Convert properties into a `&[&str]` for the TXT records
        let txt_records: Vec<String> = self.properties
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();

        // For libmdns 0.7.5, the signature is:
        //   register(svc_type: String, svc_name: String, port: u16, txt: &[&str]) -> Service
        let service = responder.register(
            self.service_type.clone(), // must be owned, not &String
            self.service_name.clone(), // must be owned, not &String
            self.port,
            &txt_records.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        );

        self.registration = Some(service);
        info!("Zeroconf service registered: {}", &self.service_name);
    }
}

// ------------------ Orchestrator Registration Logic ------------------ //

#[derive(Debug, Serialize)]
struct ZeroconfRegistrationData<'a> {
    #[serde(rename = "name")]
    name: &'a str,
    #[serde(rename = "type")]
    kind: &'a str,
    port: u16,
    properties: serde_json::Value,
    addresses: Vec<String>,
    host: String,
}

async fn register_services_to_orchestrator(
    zc: &WebthingZeroconf,
    orchestrator_url: &str,
) -> anyhow::Result<()> {
    // Convert our `(String, String)` properties into JSON
    let mut props_map = serde_json::Map::new();
    for (k, v) in &zc.properties {
        props_map.insert(k.clone(), serde_json::json!(v));
    }

    // Replicate the Python structure
    let data = ZeroconfRegistrationData {
        name: &zc.service_name,
        kind: &zc.service_type,
        port: zc.port,
        properties: serde_json::Value::Object(props_map),
        addresses: vec![zc.host.clone()],
        host: format!("{}.local.", zc.host),
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

// ------------------ "Wait until ready" ------------------ //

/// Spawns a background thread that tries to connect to the Actix server (host:port).
/// Once connected, we register Zeroconf and optionally do orchestrator POST.
fn wait_until_ready_and_register(mut zc: WebthingZeroconf) {
    thread::spawn(move || {
        // Create the mDNS responder
        let responder = MdnsResponder::new().expect("Failed to create mDNS responder");

        let addr = format!("{}:{}", zc.host, zc.port);
        loop {
            match TcpStream::connect(&addr) {
                Ok(_) => {
                    debug!("Server is ready at {}", addr);
                    // Register mDNS
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

        // If orchestrator is set, do the same POST
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

// ------------------ Actix Handlers ------------------ //

// TODO: Remove this later when no longer needed
async fn get_test() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "message": "This is a GET response!"
    }))
}

// TODO: Remove this later when no longer needed
#[derive(Deserialize)]
struct PostInput {
    data: String,
}

// TODO: Remove this later when no longer needed
async fn post_test(input: web::Json<PostInput>) -> impl Responder {
    HttpResponse::Ok().json(json!({
        "received": input.data.clone()
    }))
}

// ------------------ Main ------------------ //

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    
    // Initialize logging
    env_logger::builder().filter(None, LevelFilter::Debug).init();
    // Create the Zeroconf struct
    let zc = WebthingZeroconf::new();
    let (host, port) = (zc.host.clone(), zc.port);
    println!("host:{}, port:{}", host, port);
    // Start the Actix server
    let server = HttpServer::new(|| {
        App::new()
            .route("/get_test", web::get().to(get_test))
            .route("/post_test", web::post().to(post_test))
    })
    .bind(("0.0.0.0", port))?;

    info!("Starting Actix web server at http://{}:{}/", host, port);

    // Start a background thread that waits for readiness and announces Zeroconf
    wait_until_ready_and_register(zc);

    // Finally, run Actix
    server.run().await
}
