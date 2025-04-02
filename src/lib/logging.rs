//! # logging.rs
//! 
//! Logging utilities for the supervisor.
//!
//! This module provides a helper function and macro to send logs
//! to an external logging server (if enabled) in structured JSON format.
//!
//! It supports optional integration with `RequestEntry` to add metadata
//! such as request ID, deployment ID, and module name.

use chrono::Utc;
use serde_json::json;
use std::env;
use reqwest::Client;
use crate::lib::api::RequestEntry;
use std::collections::HashMap;
use log::{info, debug, warn, error};

/// Sends a structured log message to the configured external logging server,
/// if remote logging is enabled via the `EXTERNAL_LOGGING_ENABLED` env var.
///
/// If a `RequestEntry` is provided, additional metadata is included in the log payload.
///
/// # Arguments
/// - `level`: Log level string (e.g. "INFO", "DEBUG").
/// - `message`: Main log message.
/// - `func_name`: Name of the function sending the log (use `function_name!()` macro).
/// - `entry`: Optional reference to a `RequestEntry` for WASM context.
///
/// # Example
/// ```rust
/// send_log("INFO", "Execution started", function_name!(), Some(&entry)).await;
/// send_log("DEBUG", "Health check passed", function_name!(), None).await;
/// ```
pub async fn send_log(
    level: &str,
    message: &str,
    func_name: &str,
    entry: Option<&RequestEntry>,
) {
    let remote_logging_enabled =
        env::var("EXTERNAL_LOGGING_ENABLED").unwrap_or_else(|_| "false".to_string());

    if remote_logging_enabled == "true" {
        // Build base log payload
        let mut log_data = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "loglevel": level,
            "message": message,
            "funcName": func_name,
            "deviceName": env::var("SUPERVISOR_NAME").unwrap_or_else(|_| "unknown".to_string()),
            "deviceIP": get_device_ip(),
        });

        // Optionally include RequestEntry metadata
        if let Some(entry) = entry {
            if let Some(obj) = log_data.as_object_mut() {
                obj.insert("request_id".into(), json!(entry.request_id));
                obj.insert("deployment_id".into(), json!(entry.deployment_id));
                obj.insert("module_name".into(), json!(entry.module_name));
            }
        }

        // Print to local log
        match level.to_ascii_uppercase().as_str() {
            "DEBUG" => debug!("[{}] {}", func_name, message),
            "INFO" => info!("[{}] {}", func_name, message),
            "WARN" | "WARNING" => warn!("[{}] {}", func_name, message),
            "ERROR" => error!("[{}] {}", func_name, message),
            _ => info!("[{}] {}", func_name, message), // Default to INFO
        }

        // Send log
        let client = Client::new();
        let endpoint = env::var("WASMIOT_LOGGING_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:3000/device/logs".to_string());

        let mut form_data = HashMap::new(); // The orhchestrator expects logs as form data instead of json
        let log_data_string = serde_json::to_string(&log_data).unwrap();
        form_data.insert("logData", log_data_string);

        if let Err(e) = client
            .post(&endpoint)
            .form(&form_data)
            .send()
            .await
        {
            eprintln!("Failed to send log: {:?}", e);
        }
    } else {
        eprintln!("External logging is disabled; skipping log send.");
    }
}

/// Determines the current IP address of the device, falling back to localhost if unavailable.
///
/// Checks the `WASMIOT_SUPERVISOR_IP` environment variable first.
pub fn get_device_ip() -> String {
    env::var("WASMIOT_SUPERVISOR_IP").unwrap_or_else(|_| {
        local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "127.0.0.1".to_string())
    })
}

/// Macro for retrieving the fully qualified function name, useful for logging.
///
/// # Example
/// ```rust
/// send_log("INFO", "Something happened", function_name!(), None).await;
/// ```
#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);

        // Strip known suffixes, but avoid panic if they are not present
        if let Some(stripped_name) = name.strip_suffix("::f") {
            if let Some(stripped_again) = stripped_name.strip_suffix("::{{closure}}") {
                stripped_again
            } else {
                stripped_name
            }
        } else {
            name // Return the original name if stripping fails
        }
    }};
}
