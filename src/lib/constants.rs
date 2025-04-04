//! # constants.rs
//!
//! This module defines static constants and lazily-initialized paths used throughout the Wasm supervisor.
//!
//! It includes:
//! - Default ports, URL schemes, and naming conventions
//! - Directory names for modules and parameter mounts
//! - Environment-driven instance path resolution
//! - Supported file/media types for mount validation
//! - Interfaces made available to Wasm modules
//!
//! This module is intended to centralize all shared, immutable configuration used across the system.

use std::path::PathBuf;
use std::fs;
use once_cell::sync::Lazy;

/// Default port used when running the service.
pub const DEFAULT_PORT: u16 = 8080;

/// Default URL scheme used in requests etc.
pub const DEFAULT_URL_SCHEME: &str = "http";

/// Base path to orchestrator device discovery service.
pub const URL_BASE_PATH: &str = "/file/device/discovery/register";

/// Name for the supervisor that is given to orchestrator.
pub const SUPERVISOR_DEFAULT_NAME: &str = "supervisor";

/// Folder name where deployed Wasm modules are stored under the instance path.
pub const MODULE_FOLDER_NAME: &str = "wasm-modules";

/// Folder name where mounted files are stored. 
pub const PARAMS_FOLDER_NAME: &str = "wasm-params";

/// Root path where everything related to this instance of service are stored into
///
/// This is typically configured via the `INSTANCE_PATH` environment variable.
/// Defaults to `./instance` if not set.
pub static INSTANCE_PATH: Lazy<PathBuf> = Lazy::new(|| {
    PathBuf::from(std::env::var("INSTANCE_PATH").unwrap_or_else(|_| "./instance".into()))
});

/// Full path to the directory containing Wasm modules.
///
/// This is derived from the `INSTANCE_PATH` and `MODULE_FOLDER_NAME`.
pub static MODULE_FOLDER: Lazy<PathBuf> = Lazy::new(|| INSTANCE_PATH.join(MODULE_FOLDER_NAME));

/// Full path to the directory used for mounted files.
///
/// This is derived from the `INSTANCE_PATH` and `PARAMS_FOLDER_NAME`.
pub static PARAMS_FOLDER: Lazy<PathBuf> = Lazy::new(|| INSTANCE_PATH.join(PARAMS_FOLDER_NAME));

/// The predefined list of supported interfaces for wasm modules.
///
/// These refer to function names the supervisor imports into each wasm module.
/// Most of these arent actually implemented but are here for compatibility reasons.
pub const SUPERVISOR_INTERFACES: [&str; 36] = [
    "millis",
    "delay",
    "print",
    "println",
    "printInt",
    "rpcCall",
    "takeImage",
    "takeImageDynamicSize", // This is actually implemented
    "takeImageStaticSize", // This is actually implemented
    "path_open",
    "fd_filestat_get",
    "fd_read",
    "fd_readdir",
    "fd_seek",
    "fd_write",
    "fd_close",
    "fd_prestat_get",
    "fd_prestat_dir_name",
    "sched_yield",
    "random_get",
    "proc_exit",
    "environ_sizes_get",
    "environ_get",
    "pinMode",
    "digitalWrite",
    "getPinLED",
    "getChipID",
    "printFloat",
    "wifiConnect",
    "wifiStatus",
    "wifiLocalIp",
    "printWifiLocalIp",
    "httpPost",
    "http_post",
    "readTemperature",
    "readHumidity"
];

/// List of media types considered valid for file-based inputs and outputs.
///
/// Used during mount setup and endpoint validation.
pub const FILE_TYPES: [&str; 7] = [
    "image/png",
    "image/jpeg",
    "image/jpg",
    "application/octet-stream",
    "application/wasm",
    "text/html",
    "text/javascript"
];

/// Ensures that all required directories for modules and parameter mounts exist.
///
/// This function should be ran in the main function before anything else.
pub fn ensure_required_folders() {
    fs::create_dir_all(&*MODULE_FOLDER).expect("Failed to create module folder");
    fs::create_dir_all(&*PARAMS_FOLDER).expect("Failed to create params folder");
}
