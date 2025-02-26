use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use sysinfo::{System, Networks};
use crate::lib::constants::SUPERVISOR_INTERFACES;



/// Returns the absolute path to the instance directory.
/// By default, uses the current working directory + "/instance".
pub fn get_instance_path() -> PathBuf {
    let instance_str = env::var("INSTANCE_PATH")
        .unwrap_or_else(|_| {
            let cwd = env::current_dir().expect("Failed to get current working directory");
            cwd.join("instance").to_string_lossy().to_string()
        });
    Path::new(&instance_str)
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(&instance_str)
        })
}

/// Returns the absolute path to the config directory = <INSTANCE_PATH>/configs.
pub fn get_config_dir() -> PathBuf {
    let instance_path = get_instance_path();
    instance_path.join("configs")
}

/// Creates the file if it does not exist, writing the given default_obj as json.
/// Returns the file content as a String.
pub fn check_open(path: &Path, default_obj: &Value) -> io::Result<String> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
        let content = serde_json::to_string_pretty(default_obj)
            .unwrap_or_else(|_| "{}".to_string());
        file.write_all(content.as_bytes())?;
    }
    let file_str = fs::read_to_string(path)?;
    Ok(file_str)
}

/// Load `remote_functions.json` from config directory.
/// If missing, create with empty json {}.
pub fn get_remote_functions() -> Value {
    let config_dir = get_config_dir();
    let path = config_dir.join("remote_functions.json");
    let default_json = json!({});
    let content = check_open(&path, &default_json)
        .expect("Failed to open or create remote_functions.json");
    serde_json::from_str(&content)
        .expect("Failed to parse JSON in remote_functions.json")
}

/// Load `modules.json` from config directory.
/// If missing, create with empty json {}.
pub fn get_modules() -> Value {
    let config_dir = get_config_dir();
    let path = config_dir.join("modules.json");
    let default_json = json!({});
    let content = check_open(&path, &default_json)
        .expect("Failed to open or create modules.json");
    serde_json::from_str(&content)
        .expect("Failed to parse JSON in modules.json")
}

/// Load `wasmiot-device-description` json template.
/// Replaces existing platform related values in it by reading 
/// actual system information using the sysinfo crate.
/// Also replaces supervisorInterfaces with predefined list of values from
/// constants.rs
pub fn get_device_description() -> Value {
    let config_dir = get_config_dir();
    let path = config_dir.join("wasmiot-device-description.json");
    let file_str = fs::read_to_string(&path)
        .unwrap_or_else(|_| {
            panic!("Could not open or read {}", path.display())
        });
    let mut description: Value = serde_json::from_str(&file_str)
        .unwrap_or_else(|e| {
            panic!("Error parsing JSON in {}: {}", path.display(), e)
        });
    description["platform"] = get_device_platform_info();
    description["supervisorInterfaces"] = json!(SUPERVISOR_INTERFACES);
    description
}

/// Returns the web-of-things description (device-description.json)
pub fn get_wot_td() -> Value {
    let config_dir = get_config_dir();
    let path = config_dir.join("device-description.json");
    let file_str = fs::read_to_string(&path)
        .unwrap_or_else(|_| {
            panic!("Could not open or read {}", path.display())
        });
    let description: Value = serde_json::from_str(&file_str)
        .unwrap_or_else(|e| {
            panic!("Error parsing JSON in {}: {}", path.display(), e)
        });
    description
}

/// Get information on current platform
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