use std::{env, sync::Arc};

use actix_web::{HttpRequest, HttpResponse, Responder, web::{self, Data}};
use log::error;
use parking_lot::Mutex;
use serde_json::{Value, json};
use sysinfo::System;

use crate::{function_name, lib::{configuration::{get_device_description, get_wot_td}, constants::{DISKS, NETWORKS, SYSTEM}, logging::send_log, zeroconf::{WebthingZeroconf, register_health_check}}, structs::device::{HealthReport, NetworkInterfaceUsage}};





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

