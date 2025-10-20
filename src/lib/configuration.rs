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
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use sysinfo::System;
use crate::lib::constants::SUPERVISOR_INTERFACES;
use crate::lib::constants::{SYSTEM, NETWORKS, DISKS};
use crate::structs::device::{
    CpuInfo, 
    MemoryInfo, 
    NetworkInterfaceIpInfo, 
    OsInfo, 
    PlatformInfo, 
};

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

/// Returns dynamic platform info.
pub fn get_device_description() -> Value {
    let mut description: Value = json!({});
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

    let (memory_bytes, cpu_name, cpu_architecture, clock_speed_hz, core_count,
         system_name, system_kernel, system_os, system_host) = {
        let mut sys =  SYSTEM.lock();
        sys.refresh_all();
        sys.refresh_cpu_all();
        sys.refresh_memory();

        let mem_bytes = sys.total_memory();

        let cpu0 = &sys.cpus()[0];
        let cpu_name = cpu0.brand().to_string();
        let clock_speed_hz = cpu0.frequency() as u64 * 1_000_000;
        let core_count = sys.cpus().len();

        let system_name   = System::name().unwrap_or_default();
        let system_kernel = System::kernel_version().unwrap_or_default();
        let system_os     = System::os_version().unwrap_or_default();
        let system_host   = System::host_name().unwrap_or_default();
        let cpu_arch      = System::cpu_arch();

        (mem_bytes, cpu_name, cpu_arch, clock_speed_hz, core_count,
            system_name, system_kernel, system_os, system_host)
    };

    let network_map: HashMap<String, NetworkInterfaceIpInfo> = {
        let mut networks = NETWORKS.lock();
        networks.refresh(true);
        networks
            .iter()
            .map(|(if_name, data)| {
                let ips: Vec<String> = data.ip_networks().iter().map(|ip| ip.to_string()).collect();
                (if_name.clone(), NetworkInterfaceIpInfo { ip_info: ips })
            })
            .collect()
    };

    let storage: HashMap<String, u64> = {
        let mut disks = DISKS.lock();
        disks.refresh(true);
        disks
            .list()
            .iter()
            .map(|d| (d.name().to_string_lossy().to_string(), d.total_space()))
            .collect()
    };

    json!(PlatformInfo {
        cpu: CpuInfo {
            architecture: cpu_architecture,
            clock_speed_hz,
            core_count: core_count as u32,
            human_readable_name: cpu_name,
        },
        memory: MemoryInfo { total_bytes: memory_bytes },
        storage,
        network: network_map,
        system: OsInfo {
            host_name: system_host,
            kernel: system_kernel,
            name: system_name,
            os: system_os,
        },
    })
}
