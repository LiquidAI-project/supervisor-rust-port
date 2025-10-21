//! # API Module
//!
//! This module provides the HTTP interface for interacting with the WebAssembly supervisor runtime.
//!
//! It exposes endpoints to:
//!
//! - Query device metadata (WasmIoT & WoT descriptions)
//! - Check system health (CPU, memory, network)
//! - Create and delete WebAssembly deployments
//! - Trigger function execution in deployed modules (GET/POST with optional input files)
//! - Fetch module-generated result files
//! - Inspect execution history of Wasm calls
//!
//! ## Key Concepts
//!
//! - **Deployment**: A collection of Wasm modules, each with a runtime and configuration,
//!   plus metadata like function chaining (instructions), input/output files (mounts),
//!   and HTTP endpoint mappings.
//!
//! - **Execution Queue**: Requests are queued for background execution to here
//!   (e.g. POST requests with large input files). A background thread (`wasm_worker`) continuously
//!   pulls from this queue.
//!
//! - **RequestEntry**: Represents a single invocation of a Wasm function, including timestamp,
//!   success/failure, input arguments, uploaded files, and result.
//!
//! - **Logging**: Most actions are logged to an external service using `send_log`, including debug,
//!   error, and info events around function execution.
//!
//! ## Key Dependencies
//!
//! - `actix-web`: HTTP server and routing
//! - `wasmtime`: WebAssembly execution engine
//! - `sysinfo`: System health stats
//! - `serde_json`: Flexible JSON handling for requests and responses
//!
//! This module is the central point of interaction between external users/devices and
//! the Wasm runtime system, providing observability, control, and lifecycle management
//! for WebAssembly-based pipelines.


use actix_web::web::Data;
use parking_lot::Mutex;
use tokio::task;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_files::NamedFile;
use sysinfo::System;
use serde_json::{json, Value};
use chrono::{Utc, DateTime};
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use hex;
use log::error;
use std::sync::Arc;
use once_cell::sync::Lazy;
use std::path::PathBuf;
use wasmtime::Val;
use sanitize_filename;
use futures_util::StreamExt;
use std::fs::File;
use std::env;
use std::io::Write;
use crate::lib::configuration::{get_wot_td, get_device_description};
use crate::lib::logging::{send_log, get_device_ip};
use crate::function_name;
use crate::lib::deployment::{Deployment, EndpointArgs, ModuleEndpointMap, EndpointData, Endpoint};
use crate::lib::wasmtime::{WasmtimeRuntime, ModuleConfig};
use crate::lib::constants::{MODULE_FOLDER, PARAMS_FOLDER};
use crate::lib::zeroconf::{register_health_check, WebthingZeroconf};
use indexmap::IndexMap;
use crate::structs::device::{
    HealthReport, 
    NetworkInterfaceUsage, 
};
use crate::lib::constants::{SYSTEM, NETWORKS, DISKS};

/// Represents a failure to fetch one or more module binaries or data files.
///
/// This struct is returned from deployment logic when HTTP fetches fail.
pub struct FetchFailures {
    pub errors: Vec<Value>,
}

/// Global in-memory storage of active deployments.
///
/// Maps a deployment ID to its corresponding `Deployment` struct,
/// including runtime environments, modules, instructions and mounts.
static DEPLOYMENTS: Lazy<Mutex<HashMap<String, Deployment>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// History of request executions, including success/failure and output data.
///
/// This mirrors `request_history` in the original Python code.
static REQUEST_HISTORY: Lazy<Mutex<Vec<RequestEntry>>> = Lazy::new(|| Mutex::new(Vec::new()));

/// Constructs and returns the filesystem path to the given module's `.wasm` file.
///
/// # Arguments
/// * `module_name` - The name of the module
///
/// # Returns
/// PathBuf pointing to the module inside the configured `MODULE_FOLDER`.
pub fn get_module_path(module_name: &str) -> PathBuf {
    MODULE_FOLDER.join(module_name)
}

/// Constructs the path to a file mounted to a specific module.
///
/// # Arguments
/// * `module_name` - The name of the module.
/// * `filename` - Optional file name. If `None`, returns the mount folder path.
///
/// # Returns
/// PathBuf to the mount directory or the specific file inside it.
pub fn get_params_path(module_name: &str, filename: Option<&str>) -> PathBuf {
    match filename {
        Some(file) => PARAMS_FOLDER.join(module_name).join(file),
        None => PARAMS_FOLDER.join(module_name),
    }
}


/// Represents a single request made to the supervisor for executing a WebAssembly function.
///
/// Tracks metadata like request time, parameters, function name, execution status, and result.
/// The `request_id` is a hash based on module/function identifiers and time for uniqueness.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestEntry {
    /// Unique identifier for this request.
    pub request_id: String,
    /// Deployment ID this request belongs to.
    pub deployment_id: String,
    /// The module name whose function is being executed.
    pub module_name: String,
    /// The function name within the module to run.
    pub function_name: String,
    /// The HTTP method used for this request.
    pub method: String,
    /// Query or JSON arguments passed to the function.
    pub request_args: Value,
    /// Mapping from mount path -> local file path for input files.
    pub request_files: HashMap<String, String>,
    /// Timestamp when this request was queued for execution.
    pub work_queued_at: DateTime<Utc>,
    /// Optional result value (primitive output or result path).
    pub result: Option<Value>,
    /// Indicates whether the execution succeeded.
    pub success: bool,
}

impl RequestEntry {
    /// Construct a new request entry and auto-generate a unique request ID.
    pub fn new(
        deployment_id: String,
        module_name: String,
        function_name: String,
        method: String,
        request_args: Value,
        request_files: HashMap<String, String>,
        work_queued_at: DateTime<Utc>,
    ) -> Self {
        let mut entry = RequestEntry {
            request_id: String::new(),
            deployment_id,
            module_name,
            function_name,
            method,
            request_args,
            request_files,
            work_queued_at,
            result: None,
            success: false,
        };
        entry.init_request_id();
        entry
    }

    /// Initializes `request_id` by hashing the module/function and timestamp.
    fn init_request_id(&mut self) {
        let key = format!("{}:{}:{}", self.deployment_id, self.module_name, self.function_name);
        let time = Utc::now().to_rfc3339();
        let input_string = format!("{}:{}", key, time);
        let mut hasher = Sha256::new();
        hasher.update(input_string.as_bytes());
        let hash_bytes = hasher.finalize();
        self.request_id = hex::encode(hash_bytes);
    }
}

/// Executes the WebAssembly function for the given request and performs any chained subcalls.
///
/// This is the core logic that:
/// 1. Prepares and runs the function,
/// 2. Interprets its result,
/// 3. Initiates a next call if the deployment specifies one,
/// 4. Returns the result or sub-response.
pub async fn do_wasm_work(entry: &mut RequestEntry) -> Result<Value, String> {
    let mut deployments = DEPLOYMENTS.lock();
    let deployment = deployments.get_mut(&entry.deployment_id)
        .ok_or_else(|| format!("Deployment '{}' not found", entry.deployment_id))?;

    let func_name = function_name!().to_string();
    let module_name_clone = entry.module_name.clone();
    let entry_clone = entry.clone();
    task::spawn(async move {
        send_log(
            "DEBUG",
            &format!("Preparing Wasm module '{}'", &module_name_clone),
            &func_name,
            Some(&entry_clone)
        ).await;
    });

    let request_args: IndexMap<String, Value> = entry.request_args
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_else(IndexMap::new);
    let (_module, wasm_args) = deployment.prepare_for_running(
        &entry.module_name,
        &entry.function_name,
        &request_args,
        &entry.request_files,
    ).await?;
    let func_name = function_name!().to_string();
    let entry_function_name = entry.function_name.clone();
    let entry_clone = entry.clone();
    task::spawn(async move {
        send_log(
            "DEBUG",
            &format!("Running Wasm function '{}'", &entry_function_name),
            &func_name,
            Some(&entry_clone)
        ).await;
    });

    let runtime = deployment.runtimes.get_mut(&entry.module_name)
        .ok_or_else(|| format!("Runtime not found for module '{}'", entry.module_name))?;

    let return_count = runtime.get_return_types(&entry.module_name, &entry.function_name).await.len();
    let output_vals = runtime.run_function(
        &entry.module_name,
        &entry.function_name,
        wasm_args,
        return_count,
    ).await;

    let raw_output = output_vals.first().map(|v| match v {
        Val::I32(i) => json!(i),
        Val::I64(i) => json!(i),
        Val::F32(f) => json!(f32::from_bits(*f)),
        Val::F64(f) => json!(f64::from_bits(*f)),
        _ => Value::Null,
    }).unwrap_or(Value::Null);

    let raw_output_clone = raw_output.clone();
    let entry_clone = entry.clone();
    let func_name = function_name!().to_string();
    tokio::spawn(async move {
        send_log(
            "DEBUG",
            &format!("... Result: {}", raw_output_clone),
            &func_name,
            Some(&entry_clone),
        )
        .await;
    });

    let (this_result, next_call) = deployment.interpret_call_from(
        &entry.module_name,
        &entry.function_name,
        raw_output.clone()
    );

    if let Some(val) = &this_result.0 {
        let val_clone = val.clone();
        let func_name = function_name!().to_string();
        let entry_clone = entry.clone();
        task::spawn( async move {
            send_log(
                "DEBUG",
                &format!("Execution result: {:?}", &val_clone),
                &func_name,
                Some(&entry_clone)
            ).await;
        });
    }
    if let Some(EndpointData::StrList(filenames)) = &this_result.1 {
        if let Some(filename) = filenames.first() {
            let result_url = format!(
                "http://{}/module_results/{}/{}",
                get_device_ip(),
                entry.module_name,
                filename
            );
            let result_url_clone = result_url.clone();
            let func_name = function_name!().to_string();
            let entry_clone = entry.clone();
            task::spawn(async move {
                send_log(
                    "DEBUG",
                    &format!("Result URL: {}", &result_url_clone),
                    &func_name,
                    Some(&entry_clone),
                ).await;
            });
        }
    }

    entry.result = this_result.0.clone().map(|arg| match arg {
        EndpointArgs::Str(s) => Value::String(s),
        EndpointArgs::StrList(vs) => Value::Array(vs.into_iter().map(Value::String).collect()),
        EndpointArgs::Dict(map) => Value::Object(map.into_iter().collect()),
    });

    if let Some(call_data) = next_call {
        let mut files = HashMap::new();
        let EndpointData::StrList(ref file_names) = call_data.files;
        for name in file_names {
            let full_path = get_params_path(&entry.module_name, Some(name));
            let file = std::fs::File::open(&full_path)
                .map_err(|e| format!("Failed to open file for subcall: {}", e))?;
            files.insert(name.clone(), file);
        }

        let mut headers = reqwest::header::HeaderMap::new();
        for (k, v) in &call_data.headers {
            if let (Ok(key), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                headers.insert(key, val);
            }
        }

        let module_name_clone = entry.module_name.clone();
        let call_data_url_clone = call_data.url.clone();
        let func_name = function_name!().to_string();
        let entry_clone = entry.clone();
        task::spawn(async move {
            send_log(
                "DEBUG",
                &format!("Making sub-call from '{}' to '{}'", &module_name_clone, &call_data_url_clone),
                &func_name,
                Some(&entry_clone),
            ).await;
        });

        drop(deployments);

        let mut form = reqwest::multipart::Form::new();

        for (name, mut file) in files {
            let mut buf = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buf).map_err(|e| format!("Failed to read file for multipart: {}", e))?;

            form = form.part(name.clone(), reqwest::multipart::Part::bytes(buf).file_name(name));
        }

        let client = reqwest::Client::new();
        let response = client
            .request(
                call_data.method.to_string().to_uppercase().parse().unwrap_or(reqwest::Method::POST),
                &call_data.url
            )
            .headers(headers)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Failed to send chained request: {}", e))?;

        // Assume JSON response from the chained call
        let chained_json: Value = response
            .json()
            .await
            .map_err(|e| format!("Invalid response JSON from {}: {}", call_data.url, e))?;

        // If there's a resultUrl, fetch it (also expected to be JSON)
        if let Some(url) = chained_json.get("resultUrl").and_then(|v| v.as_str()) {
            let fetched_json: Value = client
                .get(url)
                .send()
                .await
                .map_err(|e| format!("Failed to fetch resultUrl {}: {}", url, e))?
                .json()
                .await
                .map_err(|e| format!("Invalid JSON from resultUrl {}: {}", url, e))?;

            // If the fetched JSON contains a "result" key, return that; else return the fetched JSON.
            let final_json: Value = fetched_json
                .get("result")
                .cloned()
                .unwrap_or_else(|| fetched_json.clone());

            // Return the final JSON, but dont overwrite own results in history with it
            entry.success = true;
            return Ok(final_json);
        }

        // No resultUrl -> record and return the original chained JSON
        entry.success = true;
        return Ok(chained_json);

    }

    Ok(json!({ "result": entry.result }))
}

/// Executes a WebAssembly function call and records its result in history.
///
/// This function performs the full execution lifecycle of a `RequestEntry`, including:
/// - Calling the Wasm function via `do_wasm_work()`
/// - Setting the result and success state
/// - Logging the outcome (both to stdout and external log sink)
/// - Appending the result to global `REQUEST_HISTORY`
///
/// This is the main entry point for any completed function execution (GET or POST).
///
/// # Arguments
/// - `entry`: The request entry to execute
///
/// # Returns
/// - The updated `RequestEntry` with result and success set
/// - An optional `Value` containing the final result from the execution
pub async fn make_history(mut entry: RequestEntry) -> (RequestEntry, Option<Value>) {
    let mut final_opt: Option<Value> = None;

    match do_wasm_work(&mut entry).await {
        Ok(final_json) => {
            entry.success = true;
            final_opt = Some(final_json);
        }
        Err(err) => {
            entry.result = Some(Value::String(err.clone()));
            entry.success = false;
            log::error!("Error during Wasm execution: {}", err);
            let func_name = function_name!().to_string();
            let entry_clone = entry.clone();
            task::spawn(async move {
                send_log(
                    "ERROR",
                    &format!("Error during Wasm execution: {}", err),
                    &func_name,
                    Some(&entry_clone)
                ).await;
            });
        }
    }

    REQUEST_HISTORY.lock().push(entry.clone());
    (entry, final_opt)
}


/// Returns a machine-readable description of the device and supported Wasm host functions.
///
/// This follows the WasmIoT specification's device discovery protocol, enabling clients
/// to understand what built-in functions (host APIs) are available to Wasm modules.
///
/// This is served at the special `.well-known` path.
pub async fn wasmiot_device_description() -> impl Responder {
    let func_name = function_name!().to_string();
    tokio::spawn(async move {
        send_log("INFO", "Device description request served", &func_name, None).await;
    });

    HttpResponse::Ok().json(get_device_description())
}

/// Returns the W3C Web of Things (WoT) Thing Description for this device.
///
/// This describes the exposed capabilities and HTTP API surface of the device
/// in a standard semantic format that can be consumed by WoT-compatible tools.
///
/// Served at a standard `.well-known` endpoint.
pub async fn thingi_description() -> impl Responder {
    let func_name = function_name!().to_string();
    tokio::spawn(async move {
        send_log("INFO", "Web of Things description request served", &func_name, None).await;
    });

    HttpResponse::Ok().json(get_wot_td())
}

/// Returns a system-level health report for the device.
///
/// This endpoint provides diagnostics about:
/// - CPU usage
/// - Memory usage
/// - Per-interface network traffic (bytes up/down)
///
/// Useful for monitoring the host system and debugging Wasm workload issues.
pub async fn thingi_health(request: HttpRequest) -> impl Responder {
    // Get system info
    let (cpu_usage, memory_usage, uptime) = {
        let uptime = System::uptime();
        let mut sys =  SYSTEM.lock();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let cpu = sys.global_cpu_usage() / 100.0; // Divide by hundred to convert % to 0..1
        let used = sys.used_memory() as f32;
        let total = sys.total_memory() as f32;
        let mem = if total > 0.0 { used / total } else { 0.0 };
        (cpu, mem, uptime)
    };

    // Get network info, and handle possible poisoned mutex by reinitializing
    let network_usage = {
        let mut networks =  NETWORKS.lock();
        networks.refresh(true);
        let mut network_usage = std::collections::HashMap::new();
        for (if_name, data) in networks.iter() {
            network_usage.insert(
                if_name.clone(),
                NetworkInterfaceUsage {
                    down_bytes: data.total_received(),
                    up_bytes: data.total_transmitted(),
                },
            );
        }
        network_usage
    };

    // Get disk info
    let storage_usage = {
        let mut disks =  DISKS.lock();
        disks.refresh(true);
        let disk_list = disks.list();
        let mut storage_usage = std::collections::HashMap::new();
        for disk in disk_list.iter() {
            let disk_name = disk.name();
            let disk_total_bytes = disk.total_space();
            let disk_available_bytes = disk.available_space();
            let used_percentage = if disk_total_bytes > 0 {
                (disk_total_bytes - disk_available_bytes) as f32 / disk_total_bytes as f32
            } else {
                0.0
            };
            storage_usage.insert(
                disk_name.to_string_lossy().to_string(),
                used_percentage
            );
        }
        storage_usage
    };

    let report = HealthReport {
        cpu_usage,
        memory_usage,
        network_usage,
        uptime,
        storage_usage
    };

    let orchestrator_url = env::var("WASMIOT_ORCHESTRATOR_URL").unwrap_or(String::new());
    let orchestrator_ip = match reqwest::Url::parse(&orchestrator_url) {
        Ok(url) => url.host().map(|s| s.to_string()).unwrap_or(String::new()),
        Err(_) => String::new(),
    };
    // orchestrator should set X-Forwarded-For header with the public IP of the orchestrator
    let url_from_request = match request.headers().get("X-Forwarded-For") {
        Some(value) => value.to_str().map(|s| s.to_string()).unwrap_or(String::new()),
        // if X-Forwarded-For is not set, use the IP of the request sender
        None => match request.peer_addr() {
            Some(addr) => addr.ip().to_string(),
            None => String::new(),
        }
    };

    if orchestrator_url != String::new() && url_from_request == orchestrator_ip {
        tokio::spawn(async move {
            send_log(
                "DEBUG",
                "Reporting health check done by the orchestrator",
                &function_name!().to_string(),
                None
            ).await;
        });

        // Report the health check to the mDNS service to restart renewal timer
        match request.app_data::<Data<Arc<Mutex<WebthingZeroconf>>>>() {
            Some(data) => {
                let zc_arc = data.get_ref().clone();
                register_health_check(zc_arc.clone());
            }
            None => {
                error!("Failed to get WebthingZeroconf from app data");
            }
        }
    }
    else {
        tokio::spawn(async move {
            send_log(
                "DEBUG",
                &format!("Not reporting health check since IP does not match orchestrator host ({url_from_request} vs {orchestrator_ip})"),
                &function_name!().to_string(),
                None
            ).await;
        });
    }

    tokio::spawn(async move {
        send_log("INFO", "Health check done", &function_name!().to_string(), None).await;
    });

    HttpResponse::Ok().json(report)
        .customize()
        .insert_header(("Custom-Orchestrator-Set", env::var("WASMIOT_ORCHESTRATOR_URL").is_ok().to_string()))
}

/// Registers the active orchestrator URL to the device.
pub async fn register_orchestrator(payload: web::Json<Value>) -> impl Responder {
    let func_name = function_name!().to_string();
    let data: Value = payload.into_inner();

    if !data.is_object() || !data.get("url").is_some() {
        tokio::spawn(async move {send_log("ERROR", "No url found", &func_name, None).await;});
        return HttpResponse::BadRequest().json(json!({"error": "No url found"}));
    }

    let orchestrator_url = data["url"].as_str().unwrap_or("");
    if !reqwest::Url::parse(&orchestrator_url).is_ok() {
        tokio::spawn(async move {send_log("ERROR", "Invalid url", &func_name, None).await;});
        return HttpResponse::BadRequest().json(json!({"error": "Invalid url"}));
    }

    // Note: on non-Windows platforms setting the environment variables should only be done
    // if no other process is writing or reading them at the same time
    let logging_endpoint = format!("{}/device/logs", orchestrator_url);
    unsafe {
        env::set_var("WASMIOT_ORCHESTRATOR_URL", orchestrator_url);
        env::set_var("WASMIOT_LOGGING_ENDPOINT", &logging_endpoint);
    }
    assert_eq!(env::var("WASMIOT_ORCHESTRATOR_URL"), Ok(orchestrator_url.to_string()));
    assert_eq!(env::var("WASMIOT_LOGGING_ENDPOINT"), Ok(logging_endpoint));

    let orchestrator_url_string = orchestrator_url.to_string();

    tokio::spawn(async move {
        send_log("INFO", &format!("Orchestrator registered at url {orchestrator_url_string}"), &func_name, None).await;
    });
    HttpResponse::Ok().json(json!({"status": "success"}))
}

/// Serves a file produced as output by a WebAssembly module.
///
/// This handles URLs like `/module_results/{module_name}/{filename}` and returns
/// the corresponding file from the module’s parameter/output folder.
///
/// # Path Parameters
/// - `module_name`: The module that created the file
/// - `filename`: The output file name
pub async fn get_module_result(req: HttpRequest, path: web::Path<(String, String)>) -> impl Responder {
    let (module_name, filename) = path.into_inner();
    let file_path = get_params_path(&module_name, Some(&filename));

    let func_name = function_name!().to_string();
    let log_msg = format!("Request for module execution result: {}/{}", module_name, filename);
    tokio::spawn(async move {
        send_log("INFO", &log_msg, &func_name, None).await;
    });

    match NamedFile::open(&file_path) {
        Ok(file) => file.into_response(&req),
        Err(_) => HttpResponse::NotFound().json(json!({
            "error": "Module result file not found",
            "module": module_name,
            "filename": filename
        })),
    }
}

/// Handler for getting request history list
///
/// This is here to match a path that has no parameters vs the default 1 parameter
pub async fn request_history_list_1() -> impl Responder {
    let new_path = web::Path::from("".to_string());
    request_history_list(new_path).await
}

/// Returns previous WebAssembly execution entries or one specific request if ID is given.
///
/// Supports:
/// - `/request-history` to get all requests
/// - `/request-history/{request_id}` to get a specific request entry
///
/// The response includes the success state and result of each request.
/// If the matched request failed, it returns HTTP 500 instead of 200.
pub async fn request_history_list(path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    let history = REQUEST_HISTORY.lock();
    if id != "" {
        let func_name = function_name!().to_string();
        let log_msg = format!("Requested history for request ID: {}", id);
        tokio::spawn(async move {
            send_log("INFO", &log_msg, &func_name, None).await;
        });
        if let Some(req) = history.iter().find(|r| r.request_id == id) {
            let status_code = if req.success { 200 } else { 500 };
            return HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap())
                .json(req);
        }
        HttpResponse::NotFound().json(json!({
            "error": "No request with that ID",
            "request_id": id
        }))
    } else {
        let func_name = function_name!().to_string();
        let log_msg = "Requested history for all requests".to_string();
        tokio::spawn(async move {
            send_log("INFO", &log_msg, &func_name, None).await;
        });

        HttpResponse::Ok().json(&*history)
    }
}

/// Handler for running a module function
///
/// This is here to match a path that has only 3 parameters vs the default 4 parameters
pub async fn run_module_function_3(
    path: web::Path<(String, String, String)>,
    req: HttpRequest,
    payload: web::Payload,
) -> impl Responder {
    let (deployment_id, module_name, function_name) = path.into_inner();
    let new_path = web::Path::from((deployment_id, module_name, function_name, None));
    run_module_function(new_path, req, payload).await
}


/// Executes a function in a given module in a given deployment.
///
/// If a filename is provided, this acts as a file-serving route.
/// Otherwise, this will:
/// - Save incoming multipart files (if any)
/// - Construct a `RequestEntry`
/// - Either push to the async queue (POST) or execute immediately (GET)
/// - Return a link to the result in request history
pub async fn run_module_function(
    path: web::Path<(String, String, String, Option<String>)>,
    req: HttpRequest,
    payload: web::Payload,
) -> impl Responder {
    let (deployment_id, module_name, function_name, maybe_filename) = path.into_inner();

    // Serve static file if filename is provided
    if let Some(filename) = maybe_filename {
        let file_path = PARAMS_FOLDER.join(&module_name).join(&filename);
        let log_msg = format!(
            "Serving file: {}/{}/{}/{}",
            deployment_id.clone(),
            module_name.clone(),
            function_name.clone(),
            filename.clone()
        );
        let func_name = function_name!().to_string();
        tokio::spawn(async move {
            send_log(
                "INFO",
                &log_msg,
                &func_name,
                None
            ).await;
        });
        return match NamedFile::open(&file_path) {
            Ok(file) => file.into_response(&req),
            Err(_) => HttpResponse::NotFound().json(json!({
                "error": "File not found",
                "filename": filename,
            })),
        };
    }

    // Check if deployment and module exist
    let deployments_map = DEPLOYMENTS.lock();
    let deployment = match deployments_map.get(&deployment_id) {
        Some(dep) => dep,
        None => {
            return HttpResponse::NotFound().json(json!({
                "error": "Deployment not found",
                "deployment_id": deployment_id
            }));
        }
    };

    if !deployment.modules.contains_key(&module_name) {
        return HttpResponse::NotFound().json(json!({
            "error": "Module not found in deployment",
            "module_name": module_name
        }));
    }
    drop(deployments_map); // Free the lock early

    // Parse query parameters into JSON
    let query_str = req.uri().query().unwrap_or("");
    let query_map: HashMap<String, String> =
        serde_urlencoded::from_str(query_str).unwrap_or_default();
    let request_args = json!(query_map);

    // Handle multipart file uploads (for POST only)
    let mut request_files: HashMap<String, String> = HashMap::new();
    let is_post = req.method() == "POST";
    if is_post {
        let mut multipart = Multipart::new(&req.headers(), payload);
        while let Some(Ok(mut field)) = multipart.next().await {
            let content_disposition = field.content_disposition();
            let param_name = content_disposition.get_name().unwrap_or("file").to_string();
            let filename = content_disposition
                .get_filename()
                .map(sanitize_filename::sanitize)
                .unwrap_or_else(|| format!("{}_input.dat", param_name));

            let save_path = PARAMS_FOLDER.join(&module_name).join(&filename);
            if let Some(parent) = save_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let mut f = match File::create(&save_path) {
                Ok(f) => f,
                Err(e) => {
                    return HttpResponse::InternalServerError().json(json!({
                        "error": format!("Failed to save file: {}", e)
                    }));
                }
            };

            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                if let Err(e) = f.write_all(&data) {
                    return HttpResponse::InternalServerError().json(json!({
                        "error": format!("File write error: {}", e)
                    }));
                }
            }

            request_files.insert(param_name, save_path.to_string_lossy().to_string());
        }
    }

    // Create RequestEntry
    let entry = RequestEntry::new(
        deployment_id.clone(),
        module_name.clone(),
        function_name.clone(),
        req.method().to_string(),
        request_args,
        request_files,
        Utc::now(),
    );

    let log_msg = format!(
        "Executing module function: {}/{}/{}",
        deployment_id.clone(),
        module_name.clone(),
        function_name.clone()
    );
    let func_name = function_name!().to_string();
    let entry_clone = entry.clone();
    tokio::spawn(async move {
        send_log(
            "INFO",
            &log_msg,
            &func_name,
            Some(&entry_clone)
        ).await;
    });

    let (entry, final_opt) = make_history(entry).await;
    let http_scheme = env::var("DEFAULT_URL_SCHEME").unwrap_or_else(|_| {
        error!("Failed to read DEFAULT_URL_SCHEME from enviroment variables, defaulting to 'http'.");
        "http".to_string()
    });
    let host = env::var("WASMIOT_SUPERVISOR_IP").unwrap_or_else(|_| {
        error!("Failed to read WASMIOT_SUPERVISOR_IP from enviroment variables, defaulting to 'localhost'.");
        "localhost".to_string()
    });
    let port = env::var("WASMIOT_SUPERVISOR_PORT").unwrap_or_else(|_| {
        error!("Failed to read WASMIOT_SUPERVISOR_PORT from enviroment variables, defaulting to '8080'.");
        "8080".to_string()
    });
    let result_url = format!("{}://{}:{}/request-history/{}", http_scheme, host, port, entry.request_id);
    let mut resp = json!({ "resultUrl": result_url });
    if let Some(final_json) = final_opt {
        resp["result"] = final_json;
    }
    HttpResponse::Ok().json(resp)
}

/// Deletes (removes) an active deployment from memory by its ID.
///
/// This endpoint is typically used when a pipeline or WebAssembly workload
/// is no longer needed and should be unloaded.
///
/// # Path Parameters
/// - `deployment_id`: ID of the deployment to delete (string)
///
/// # Behavior
/// - If the deployment exists in memory, it is removed and a success message is returned.
/// - If not found, returns a 404 with an error message.
///
/// # Example
/// DELETE /deploy/my-deployment-id
pub async fn deployment_delete(path: web::Path<String>) -> impl Responder {
    let deployment_id = path.into_inner();
    let func_name = function_name!().to_string();

    let log_msg = format!("Delete request for deployment: {}", deployment_id);
    tokio::spawn(async move {
            send_log("INFO", &log_msg, &func_name, None
        ).await;
    });

    let mut deps = DEPLOYMENTS.lock();

    if deps.remove(&deployment_id).is_some() {
        HttpResponse::Ok().json(json!({ "status": "success" }))
    } else {
        HttpResponse::NotFound().json(json!({
            "error": "Deployment does not exist",
            "deployment_id": deployment_id
        }))
    }
}

/// Creates a new WebAssembly deployment with modules and optional data files.
///
/// Expects a JSON payload with fields:
/// - `deploymentId` (string)
/// - `modules` (list of modules, each with `id`, `name`, and `urls`)
/// - Optional: `endpoints`, `instructions`, `mounts`
///
/// Downloads all binaries and additional data files, sets up execution environments,
/// and stores the deployment in memory.
///
/// Returns:
/// - 200 OK if deployment succeeds
/// - 400/500 with JSON error otherwise
pub async fn deployment_create(payload: web::Json<Value>) -> impl Responder {
    let func_name = function_name!().to_string();
    send_log("INFO", "Deployment creation request received", &func_name, None).await;

    let data = payload.into_inner();

    let deployment_id = match data["deploymentId"].as_str() {
        Some(s) => s.to_string(),
        None => {
            send_log("ERROR", "Missing deploymentId", &func_name, None).await;
            return HttpResponse::BadRequest().json(json!({ "error": "Missing deploymentId" }));
        }
    };

    let modules = match data["modules"].as_array() {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            send_log("ERROR", "No modules provided", &func_name, None).await;
            return HttpResponse::BadRequest().json(json!({ "error": "No modules provided in deployment request" }));
        }
    };

    let mut module_configs = Vec::new();
    let mut errors = Vec::new();

    for module in modules {
        let id = module.get("id").and_then(Value::as_str).unwrap_or("unknown").to_string();
        let name = match module.get("name").and_then(Value::as_str) {
            Some(n) => n.to_string(),
            None => {
                let err = json!({ "error": "Module missing name", "module": module });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        // Fetch binary
        let binary_url = match module.get("urls").and_then(|urls| urls.get("binary")).and_then(Value::as_str) {
            Some(url) => url.to_string(),
            None => {
                let err = json!({ "error": "Module missing binary URL", "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let bin_response = match reqwest::get(&binary_url).await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                let err = json!({ "error": format!("Binary URL returned {}", resp.status()), "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
            Err(e) => {
                let err = json!({ "error": format!("Failed to fetch binary: {}", e), "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let bin_bytes = match bin_response.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                let err = json!({ "error": format!("Failed to read binary response: {}", e), "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let binary_path = get_module_path(&name);
        if let Some(parent) = binary_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }

        if let Err(e) = std::fs::write(&binary_path, &bin_bytes) {
            let err = json!({ "error": format!("Failed to write binary: {}", e), "path": binary_path });
            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
            errors.push(err);
            continue;
        }

        // Create the params folder by default
        let path = get_params_path(&name, None);
        std::fs::create_dir_all(path).ok();

        let mut data_files = HashMap::new();
        if let Some(other_map) = module.get("urls")
            .and_then(|urls| urls.get("other"))
            .and_then(Value::as_object)
        {
            for (filename, url_val) in other_map {
                if let Some(url) = url_val.as_str() {
                    match reqwest::get(url).await {
                        Ok(resp) if resp.status().is_success() => {
                            match resp.bytes().await {
                                Ok(file_bytes) => {
                                    let path = get_params_path(&name, Some(filename));
                                    if let Some(parent) = path.parent() {
                                        std::fs::create_dir_all(parent).ok();
                                    }
                                    match std::fs::write(&path, &file_bytes) {
                                        Ok(_) => {
                                            data_files.insert(filename.clone(), path.to_string_lossy().to_string());
                                        }
                                        Err(e) => {
                                            let err = json!({
                                                "error": format!("Failed to save extra file: {}", e),
                                                "file": filename,
                                                "module": name
                                            });
                                            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                                            errors.push(err);
                                        }
                                    }
                                }
                                Err(e) => {
                                    let err = json!({
                                        "error": format!("Failed to read extra file bytes: {}", e),
                                        "file": filename,
                                        "module": name
                                    });
                                    send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                                    errors.push(err);
                                }
                            }
                        }
                        Ok(resp) => {
                            let err = json!({
                                "error": format!("Non-200 response for extra file: {}", resp.status()),
                                "file": filename,
                                "module": name
                            });
                            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                            errors.push(err);
                        }
                        Err(e) => {
                            let err = json!({
                                "error": format!("Failed to fetch extra file: {}", e),
                                "file": filename,
                                "module": name
                            });
                            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                            errors.push(err);
                        }
                    }
                }
            }
        }

        // Construct module config and apply model detection
        let mut config = ModuleConfig {
            id,
            name: name.clone(),
            path: binary_path,
            data_files,
            ml_model: None,
            data_ptr_function_name: "get_image_ptr".to_string(),
        };
        config.set_model_from_data_files(None);

        module_configs.push(config);
    }

    if !errors.is_empty() {
        return HttpResponse::InternalServerError().json(json!({
            "error": "One or more modules failed to load",
            "details": errors
        }));
    }

    // Initialize Wasmtime runtimes for each module with their param folders mounted
    let mut runtimes = HashMap::new();
    for config in &module_configs {
        match WasmtimeRuntime::new(vec![
            (get_params_path(&config.name, None).to_string_lossy().to_string(), ".".to_string())
        ]).await {
            Ok(runtime) => {
                runtimes.insert(config.name.clone(), runtime);
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(json!({
                    "error": format!("Failed to initialize runtime: {}", e),
                    "module": config.name
                }));
            }
        }
    }

    // Convert endpoints (nested map) to expected type
    let endpoints: ModuleEndpointMap = match data.get("endpoints") {
        Some(Value::Object(mod_map)) => {
            let mut result = HashMap::new();
            for (mod_name, fn_map_val) in mod_map {
                if let Some(fn_map) = fn_map_val.as_object() {
                    let mut inner = HashMap::new();
                    for (fn_name, endpoint_val) in fn_map {
                        match serde_json::from_value::<Endpoint>(endpoint_val.clone()) {
                            Ok(endpoint) => {
                                inner.insert(fn_name.clone(), endpoint);
                            }
                            Err(e) => {
                                return HttpResponse::BadRequest().json(json!({
                                    "error": format!("Invalid endpoint for '{}::{}': {}", mod_name, fn_name, e)
                                }));
                            }
                        }
                    }
                    result.insert(mod_name.clone(), inner);
                }
            }
            result
        }
        _ => HashMap::new(),
    };

    // Convert instructions and mounts from Map<String, Value> → HashMap<String, Value>
    let instructions = data.get("instructions")
        .and_then(|v| v.as_object())
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_else(HashMap::new);

    let mounts = data.get("mounts")
        .and_then(|v| v.as_object())
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_else(HashMap::new);

    let deployment = Deployment::new(
        deployment_id.clone(),
        runtimes,
        module_configs,
        endpoints,
        instructions,
        mounts,
    );

    DEPLOYMENTS.lock().insert(deployment_id.clone(), deployment); // TODO: The lock can become poisoned if something panics and DEPLOYMENTS is locked

    send_log("INFO", &format!("Deployment created: {}", deployment_id), &func_name, None).await;

    HttpResponse::Ok().json(json!({
        "status": "success",
        "deploymentId": deployment_id
    }))
}

/// Configures the HTTP routes for the Wasm supervisor API.
///
/// This function sets up all the endpoints used for:
/// - Device metadata and health checks
/// - Deployment management (create/delete)
/// - Module execution (GET and POST)
/// - Result file access
/// - Execution history tracking
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Returns metadata about the device and supported host functions (WasmIoT spec)
        .route("/.well-known/wasmiot-device-description", web::get().to(wasmiot_device_description))

        // Returns W3C WoT Thing Description for this device
        .route("/.well-known/wot-thing-description", web::get().to(thingi_description))

        // Health check for device (CPU, memory, network)
        .route("/health", web::get().to(thingi_health))

        // Duplicate health route for compatibility (was required at some point)
        .route("//health", web::get().to(thingi_health))

        // Registers the active orchestrator URL to the device
        .route("/register", web::post().to(register_orchestrator))

        // Fetch result files generated by module execution
        .route("/module_results/{module_name}/{filename}", web::get().to(get_module_result))

        // Fetch execution history (entire list or single entry by ID)
        .route("/request-history/{request_id}", web::get().to(request_history_list))
        .route("/request-history", web::get().to(request_history_list_1))

        // Serve result file produced by specific function execution
        .route("/{deployment_id}/modules/{module_name}/{function_name}/{filename}", web::get().to(run_module_function))

        // Run a module function (GET: immediate execution, no input files)
        .route("/{deployment_id}/modules/{module_name}/{function_name}", web::get().to(run_module_function_3))
        // Supposedly this doesn't match:   /67ebca4c67d46c4d7f551bea/modules/fibo/fibo?param0=7

        // Run a module function (POST: allows input files via multipart)
        .route("/{deployment_id}/modules/{module_name}/{function_name}", web::post().to(run_module_function_3))

        // Delete an existing deployment by ID
        .route("/deploy/{deployment_id}", web::delete().to(deployment_delete))

        // Create a new deployment with modules and optional mount/config data
        .route("/deploy", web::post().to(deployment_create))
        .route("//deploy", web::post().to(deployment_create)); // This is needed for current version of orchestrator for some reason
}

