//! # configuration.rs
//!
//! This module handles loading and initializing configuration files and metadata for the Wasm supervisor.
//!
//! Key responsibilities:
//! - Resolving paths to instance/config directories
//! - Ensuring presence of required JSON config files (with default creation if missing)
//! - Loading structured device metadata, including system information and network interfaces
//! - Integrating static configuration (e.g., `remote_functions.json`, `modules.json`) with
//!   dynamic system information via `sysinfo`

use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use sysinfo::{System, Networks};
use crate::lib::constants::SUPERVISOR_INTERFACES;

/// Returns the absolute path to the instance directory.
///
/// Uses the environment variable `INSTANCE_PATH` if set, otherwise defaults to:
/// `<current_working_directory>/instance`.
pub fn get_instance_path() -> PathBuf {
    let instance_str = env::var("INSTANCE_PATH")
        .unwrap_or_else(|_| {
            let cwd = env::current_dir().expect("Failed to get current working directory");
            cwd.join("instance").to_string_lossy().to_string()
        });

    Path::new(&instance_str).canonicalize().unwrap_or_else(|_| PathBuf::from(&instance_str))
}

/// Returns the path to the config directory: `<instance_path>/configs`.
///
/// This is where static JSON config files are expected to reside.
pub fn get_config_dir() -> PathBuf {
    get_instance_path().join("configs")
}

/// Opens a JSON file at the given path, creating it with default content if it does not exist.
///
/// - `path`: Absolute file path to check.
/// - `default_obj`: Default JSON value to write if the file is missing.
///
/// # Returns
/// File content as a string, or error if reading/writing fails.
pub fn check_open(path: &Path, default_obj: &Value) -> io::Result<String> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let content = serde_json::to_string_pretty(default_obj)
            .unwrap_or_else(|_| "{}".to_string());
        file.write_all(content.as_bytes())?;
    }
    fs::read_to_string(path)
}

/// Loads `remote_functions.json` from the config directory.
///
/// If the file does not exist, it is created with empty `{}`.
///
/// # Panics
/// If the file cannot be opened or parsed.
pub fn get_remote_functions() -> Value {
    let path = get_config_dir().join("remote_functions.json");
    let content = check_open(&path, &json!({}))
        .expect("Failed to open or create remote_functions.json");
    serde_json::from_str(&content)
        .expect("Failed to parse JSON in remote_functions.json")
}

/// Loads `modules.json` from the config directory.
///
/// If the file does not exist, it is created with empty `{}`.
///
/// # Panics
/// If the file cannot be opened or parsed.
pub fn get_modules() -> Value {
    let path = get_config_dir().join("modules.json");
    let content = check_open(&path, &json!({}))
        .expect("Failed to open or create modules.json");
    serde_json::from_str(&content)
        .expect("Failed to parse JSON in modules.json")
}

/// Loads the `wasmiot-device-description.json` file and injects dynamic platform info.
///
/// - Adds live CPU/memory/network/system data via `sysinfo`
/// - Overwrites `supervisorInterfaces` using `SUPERVISOR_INTERFACES`
///
/// # Panics
/// If the file cannot be opened, read, or parsed.
pub fn get_device_description() -> Value {
    let path = get_config_dir().join("wasmiot-device-description.json");
    let file_str = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Could not open or read {}", path.display()));
    let mut description: Value = serde_json::from_str(&file_str)
        .unwrap_or_else(|e| panic!("Error parsing JSON in {}: {}", path.display(), e));

    description["platform"] = get_device_platform_info();
    description["supervisorInterfaces"] = json!(SUPERVISOR_INTERFACES.to_vec());
    description
}

/// Loads the Web of Things (WoT) Thing Description from `device-description.json`.
///
/// This is a static file expected to exist in the config directory.
///
/// # Panics
/// If the file cannot be opened, read, or parsed.
pub fn get_wot_td() -> Value {
    let path = get_config_dir().join("device-description.json");
    let file_str = fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Could not open or read {}", path.display()));
    serde_json::from_str(&file_str)
        .unwrap_or_else(|e| panic!("Error parsing JSON in {}: {}", path.display(), e))
}

/// Gathers live system information using the `sysinfo` crate, including:
/// - System name, kernel, OS version, hostname
/// - CPU brand, clock speed, core count
/// - Total memory
/// - Network interfaces and IP addresses
///
/// This data is injected into the WasmIoT device description during startup.
pub fn get_device_platform_info() -> Value {
    let mut sys = System::new_all();
    sys.refresh_all();

    let memory_bytes = sys.total_memory();
    let cpu_name = sys.cpus()[0].brand().to_string();
    let clock_speed_hz = sys.cpus()[0].frequency() as u64 * 1_000_000;
    let core_count = sys.cpus().len();

    let system_name = System::name();
    let system_kernel = System::kernel_version();
    let system_os = System::os_version();
    let system_host = System::host_name();

    let networks = Networks::new_with_refreshed_list();
    let network_data: Value = networks.iter()
        .map(|(interface_name, data)| {
            (
                interface_name.clone(),
                json!({
                    "ipInfo": data.ip_networks()
                        .iter()
                        .map(|ip| ip.to_string())
                        .collect::<Vec<String>>()
                }),
            )
        })
        .collect();

    json!({
        "system": {
            "name": system_name,
            "kernel": system_kernel,
            "os": system_os,
            "hostName": system_host
        },
        "memory": {
            "bytes": memory_bytes
        },
        "cpu": {
            "humanReadableName": cpu_name,
            "clockSpeed": {
                "Hz": clock_speed_hz
            },
            "coreCount": core_count
        },
        "network": network_data
    })
}
