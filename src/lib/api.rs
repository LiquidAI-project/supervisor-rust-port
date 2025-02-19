use actix_web::{web, HttpResponse, Responder};
use sysinfo::System;
use log::info;
use serde_json::json;

use crate::lib::configuration::{get_wot_td, get_device_description};


// TODO: Check the configure_routes function so that endpoint methods are correct
// TODO: Implement missing functionality


/// Returns the device description
pub async fn wasmiot_device_description() -> impl Responder {
    info!("Device description request served");
    // Return JSON data
    HttpResponse::Ok().json(get_device_description())
}

/// Returns the device web-of-things description
pub async fn thingi_description() -> impl Responder {
    HttpResponse::Ok().json(get_wot_td())
}

/// Return health status, contains global cpu usage and memory usage at the moment.
pub async fn thingi_health() -> impl Responder {
    info!("Health check done");
    let mut sys = System::new_all();
    sys.refresh_all();
    // sys.refresh_cpu_usage();
    // std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    // sys.refresh_cpu_usage();
    // let cpu_usages: Vec<String> = sys.cpus()
    //     .iter()
    //     .map(|cpu| format!("{:.1}", cpu.cpu_usage()))
    //     .collect();
    let cpu_usage = format!("{:.1}%", sys.global_cpu_usage());
    let memory_usage = format!("{:.1}%", sys.used_memory() / sys.total_memory() * 100);
    HttpResponse::Ok().json(json!({
        "cpuUsage": cpu_usage,
        "memoryUsage": memory_usage
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

