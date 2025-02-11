use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::env;
use std::net::TcpStream;
use std::thread;
use std::time::Duration;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use libmdns::{Responder as MdnsResponder, Service};
use reqwest::Client;
use log::{info, debug, error, LevelFilter};
use rand::{Rng, thread_rng};

// TODO: Change healthcheck processor usage to not be random anymore

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



// ------------------ configuration.py translation ----//

/// Returns the absolute path to the instance directory.
/// By default, uses the current working directory + "/instance".
fn get_instance_path() -> PathBuf {
    // Read INSTANCE_PATH from environment.
    // If not set, fallback to: current_dir()/instance
    let instance_str = env::var("INSTANCE_PATH")
        .unwrap_or_else(|_| {
            let cwd = env::current_dir().expect("Failed to get current working directory");
            cwd.join("instance").to_string_lossy().to_string()
        });

    // Convert to an absolute (canonical) path if possible.
    Path::new(&instance_str)
        .canonicalize()
        .unwrap_or_else(|_| {
            // If canonicalize fails, we just return the raw path as fallback
            PathBuf::from(&instance_str)
        })
}

/// Returns the absolute path to the config directory = <INSTANCE_PATH>/configs.
fn get_config_dir() -> PathBuf {
    let instance_path = get_instance_path();
    instance_path.join("configs")
}

/// Creates the file if it does not exist, writing `default_obj` as JSON.
/// Returns the file content as a `String` for further processing.
fn check_open(path: &Path, default_obj: &Value) -> io::Result<String> {
    // If the file is missing, create it with default JSON content.
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // "x" means create and fail if it already exists (so we don't overwrite).
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
        let content = serde_json::to_string_pretty(default_obj)
            .unwrap_or_else(|_| "{}".to_string());
        file.write_all(content.as_bytes())?;
    }
    // Now read the file to string
    let file_str = fs::read_to_string(path)?;
    Ok(file_str)
}

/// Load `remote_functions.json` from config directory.
/// If missing, create with an empty object {}.
pub fn get_remote_functions() -> Value {
    let config_dir = get_config_dir();
    let path = config_dir.join("remote_functions.json");

    // If missing, create with empty JSON object
    let default_json = json!({});
    let content = check_open(&path, &default_json)
        .expect("Failed to open or create remote_functions.json");
    
    serde_json::from_str(&content)
        .expect("Failed to parse JSON in remote_functions.json")
}

/// Load `modules.json` from config directory.
/// If missing, create with an empty object {}.
pub fn get_modules() -> Value {
    let config_dir = get_config_dir();
    let path = config_dir.join("modules.json");

    let default_json = json!({});
    let content = check_open(&path, &default_json)
        .expect("Failed to open or create modules.json");
    
    serde_json::from_str(&content)
        .expect("Failed to parse JSON in modules.json")
}

/// Must exist or fail by design. Merge random "platform" info into JSON.
pub fn get_device_description() -> Value {
    let config_dir = get_config_dir();
    let path = config_dir.join("wasmiot-device-description.json");

    // Read or fail if missing. 
    // "Fails by design" means we do NOT create it automatically here.
    let file_str = fs::read_to_string(&path)
        .unwrap_or_else(|_| {
            panic!("Could not open or read {}", path.display())
        });

    let mut description: Value = serde_json::from_str(&file_str)
        .unwrap_or_else(|e| {
            panic!("Error parsing JSON in {}: {}", path.display(), e)
        });

    // Insert platform info
    description["platform"] = get_device_platform_info();
    description
}

/// Not yet implemented. In Python: `raise NotImplementedError`.
/// In Rust, you can simply `unimplemented!()` or `todo!()`.
pub fn get_wot_td() -> Value {
    unimplemented!("get_wot_td() is not implemented yet")
}

/// Return a random "platform" info object, mimicking `_get_device_platform_info()`.
fn get_device_platform_info() -> Value {

    // A simple random approach to emulate memory and CPU info
    let mut rng = thread_rng();

    // Generate random memory in 4GB increments between 4 and 64
    let memory_gb_options = [4, 8, 12, 16, 32, 64];
    let mem_index = rng.gen_range(0..memory_gb_options.len());
    let memory_gb = memory_gb_options[mem_index];
    // Convert GB to bytes
    let memory_bytes = memory_gb as u64 * 1_000_000_000;

    // Random CPU name
    let cpus = [
        "12th Gen Intel(R) Core(TM) i7-12700H",
        "AMD EPYC™ Embedded 7551",
        "Dual-core Arm Cortex-M0+"
    ];
    let cpu_name = cpus[rng.gen_range(0..cpus.len())];

    // Random clock speed up to ~3 GHz
    let clock_speed_hz = rng.gen_range(1_000_000_000_u64..3_000_000_001_u64);

    // Build the JSON
    json!({
        "memory": {
            "bytes": memory_bytes
        },
        "cpu": {
            "humanReadableName": cpu_name,
            "clockSpeed": {
                "Hz": clock_speed_hz
            }
        }
    })
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

async fn wasmiot_device_description() -> impl Responder {
    info!("Device description request served");
    // Return JSON data
    HttpResponse::Ok().json(get_device_description())
}

async fn thingi_description() -> impl Responder {
    // TODO: get_wot_td returns an error as its not implemented
    HttpResponse::Ok().json(get_wot_td())
}

///
/// 
async fn thingi_health() -> impl Responder {
    info!("Health check done");
    // Return random CPU usage in JSON
    // TODO: Should actual cpu usage be returned??
    let cpu_usage = rand::thread_rng().gen_range(0.0..1.0);
    HttpResponse::Ok().json(json!({
        "cpuUsage": cpu_usage
    }))
}

/// Catch-all handler for debug purposes
async fn default(req: HttpRequest) -> impl Responder {
    let path = req.path();
    println!("Handling request for: {}", path);
    HttpResponse::NotFound()
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
            .route("/.well-known/wasmiot-device-description", web::get().to(wasmiot_device_description))
            .route("/.well-known/wot-thing-description", web::get().to(thingi_description))
            .route("/health", web::get().to(thingi_health))
            .route("//health", web::get().to(thingi_health)) // TODO: For some reason the orchestrator has two slashes in the requested address
            .route("/get_test", web::get().to(get_test))
            .route("/post_test", web::post().to(post_test))
            .default_service(web::route().to(default))
    })
    .bind(("0.0.0.0", port))?;

    info!("Starting Actix web server at http://{}:{}/", host, port);

    // Start a background thread that waits for readiness and announces Zeroconf
    wait_until_ready_and_register(zc);

    // Finally, run Actix
    server.run().await
}
