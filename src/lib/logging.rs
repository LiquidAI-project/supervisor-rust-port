//!
//! This module contains logging related functionality.
//! Currently that means mainly sending logs to external server.
//! 

use chrono::Utc;
use serde_json::json;
use std::env;
use reqwest::Client;

/// Purpose of this function is to both create a log entry locally,
/// and to send it to remote logging server (if enabled).
/// Preferred way of calling this function would be using the 
/// module_path!() macro to determine func_name.
pub async fn send_log(level: &str, message: &str, func_name: &str) {
    // If remote logging is enabled, send the log as json to the logging server.
    let remote_logging_enabled = env::var("EXTERNAL_LOGGING_ENABLED").unwrap_or_else(|_| "false".to_string()) == "true";
    if remote_logging_enabled {
        let log_entry = json!({"logData": {
            "timestamp": Utc::now().to_rfc3339(),
            "loglevel": level,
            "message": message,
            "funcName": func_name,
            "deviceName": env::var("SUPERVISOR_NAME").unwrap_or_else(|_| "unknown".to_string()),
            "deviceIP": get_device_ip(),
        }});
        let client = Client::new();
        let endpoint = env::var("WASMIOT_LOGGING_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:3000/device/logs".to_string());
        match client.post(&endpoint).json(&log_entry).send().await {
            Ok(_) => (),
            Err(e) => eprintln!("Failed to send log: {:?}", e),
        }
    } else {
        eprintln!("Tried sending a log to logging server but external logging is currently disabled!");
    }
}


/// Determine current ip address
fn get_device_ip() -> String {
    env::var("WASMIOT_SUPERVISOR_IP").unwrap_or_else(|_| {
        local_ip_address::local_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|_| "127.0.0.1".to_string())
    })
}

/// Used with logging to get function name and path easily.
/// Source: https://stackoverflow.com/questions/38088067/equivalent-of-func-or-function-in-rust
#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        name.strip_suffix("::f").unwrap().strip_suffix("::{{closure}}").unwrap()
    }}
}
