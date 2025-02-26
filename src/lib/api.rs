
use actix_web::{web, HttpResponse, Responder};
use sysinfo::{System, Networks};
use serde_json::{json, Value};
use log::info;
use chrono::Utc;
use std::collections::HashMap;
use sha2::{Sha256, Digest};
use hex;
use crate::lib::configuration::{get_wot_td, get_device_description};
use crate::lib::logging::send_log;
use crate::function_name;


// TODO: Check the configure_routes function so that endpoint methods are correct
// TODO: Implement missing functionality
// TODO: Figure out what the FetchFailures should actually contain


// Struct for containing failed attempts to fetch modules or their related files
pub struct FetchFailures {
    pub errors: Vec<Value>,
}

/// Struct for containing requests made to supervisor
#[derive(Debug)]
pub struct RequestEntry {
    pub request_id: String,
    pub deployment_id: String,
    pub module_name: String,
    pub function_name: String,
    pub method: String,
    pub request_args: Value,
    pub request_files: HashMap<String, String>,
    pub work_queued_at: Utc,
    pub result: Option<Value>,
    pub success: bool,
}

impl RequestEntry {

    pub fn new(
        deployment_id: String,
        module_name: String,
        function_name: String,
        method: String,
        request_args: Value,
        request_files: HashMap<String, String>,
        work_queued_at: Utc,
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

    /// Gives a hashed id for a request based on its details and the exact current time (down to milliseconds)
    fn init_request_id(&mut self) {
        let key = format!("{}:{}:{}", self.deployment_id, self.module_name, self.function_name);
        let time = Utc::now().to_rfc3339();
        let input_string = format!("{}:{}", key, time);
        let mut hasher = Sha256::new();
        hasher.update(input_string.as_bytes());
        let hash_bytes = hasher.finalize();
        let hash_hex = hex::encode(hash_bytes);
        self.request_id = hash_hex;
    }

}


/// Returns the device description
pub async fn wasmiot_device_description() -> impl Responder {
    info!("Device description request served");
    // Return JSON data
    info!("Wasmiot device description served");
    send_log("info", "Device description request served", function_name!()).await;
    HttpResponse::Ok().json(get_device_description())
}

/// Returns the device web-of-things description
pub async fn thingi_description() -> impl Responder {
    info!("Web of things description served.");
    send_log("info", "Web of Things description request served", function_name!()).await;
    HttpResponse::Ok().json(get_wot_td())
}

/// Return health status, contains global cpu usage and memory usage at the moment.
pub async fn thingi_health() -> impl Responder {
    info!("Health check done");
    send_log("info", "Health check done", function_name!()).await;
    let mut sys = System::new_all();
    sys.refresh_all();
    let cpu_usage = sys.global_cpu_usage();
    let memory_usage = sys.used_memory() / sys.total_memory();
    let networks = Networks::new_with_refreshed_list();
    let network_usage: Value = networks.iter()
    .filter_map(|(interface_name, data)| {
        let down_bytes = data.total_received();
        let up_bytes = data.total_transmitted();
        if down_bytes > 0 || up_bytes > 0 {
            Some((
                interface_name.clone(),
                json!({
                    "downBytes": down_bytes,
                    "upBytes": up_bytes,
                }),
            ))
        } else {
            None
        }
    })
    .collect();

    HttpResponse::Ok().json(json!({
        "cpuUsage": cpu_usage,
        "memoryUsage": memory_usage,
        "networkUsage": network_usage
    }))
}

/// Return result of executing a module
pub async fn get_module_result(path: web::Path<(String, String)>) -> impl Responder {
    let (module_name, filename) = path.into_inner();
    info!("Request for module execution result: {}/{}", module_name, filename);
    HttpResponse::NotImplemented().json(json!({
        "error": "Module result retrieval not implemented",
        "module": module_name,
        "filename": filename
    }))
}

/// Return one or more items from request history
pub async fn request_history_list(path: web::Path<Option<String>>) -> impl Responder {
    let request_id = path.into_inner();
    if let Some(id) = request_id {
        info!("Requested history for request ID: {}", id);
        HttpResponse::NotImplemented().json(json!({
            "error": "Request history retrieval not implemented",
            "request_id": id
        }))
    } else {
        info!("Requested history for all requests");
        HttpResponse::NotImplemented().json(json!({
            "error": "Request history retrieval not implemented",
            "all_requests": true
        }))
    }
}

/// Executes a function in a given module in a given deployment
/// Alternatively, does file servicing if a filename is given
pub async fn run_module_function(path: web::Path<(String, String, String, Option<String>)>) -> impl Responder {
    let (deployment_id, module_name, function_name, filename) = path.into_inner();
    if let Some(file) = filename {
        info!("File servicing requested: {}/{}/{}/{}", deployment_id, module_name, function_name, file);
        HttpResponse::NotImplemented().json(json!({
            "error": "Deployment file servicing not implemented",
            "deployment_id": deployment_id,
            "module_name": module_name,
            "function_name": function_name,
            "filename": file
        }))
    } else {
        info!("Executing module function: {}/{}/{}", deployment_id, module_name, function_name);
        HttpResponse::NotImplemented().json(json!({
            "error": "Deployment execution not implemented",
            "deployment_id": deployment_id,
            "module_name": module_name,
            "function_name": function_name
        }))
    }
}

/// Deletes a given deployment
pub async fn deployment_delete(path: web::Path<String>) -> impl Responder {
    let deployment_id = path.into_inner();
    info!("Delete request for deployment: {}", deployment_id);
    HttpResponse::NotImplemented().json(json!({
        "error": "Deployment deletion not implemented",
        "deployment_id": deployment_id
    }))
}

/// Creates new deployments
pub async fn deployment_create() -> impl Responder {
    info!("Deployment creation request received");
    HttpResponse::NotImplemented().json(json!({
        "error": "Deployment creation not implemented"
    }))
}

/// Function that is used to configure routes for the server
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/.well-known/wasmiot-device-description", web::get().to(wasmiot_device_description))
       .route("/.well-known/wot-thing-description", web::get().to(thingi_description))
       .route("/health", web::get().to(thingi_health))
       .route("//health", web::get().to(thingi_health)) // TODO: For some reason the orchestrator has two slashes in the requested address
       .route("/module_results/{module_name}/{filename}", web::get().to(get_module_result))
       .route("/request-history/{request_id}", web::get().to(request_history_list))
       .route("/{deployment_id}/modules/{module_name}/{function_name}/{filename}", web::get().to(run_module_function))
       .route("/{deployment_id}/modules/{module_name}/{function_name}", web::get().to(run_module_function))
       .route("/deploy/{deployment_id}", web::delete().to(deployment_delete))
       .route("/deploy", web::post().to(deployment_create));
}

