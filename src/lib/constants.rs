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

#[cfg(all(feature="camera", not(feature="armv6")))]
/// The predefined list of supported interfaces for wasm modules.
///
/// These refer to function names the supervisor imports into each wasm module.
pub const SUPERVISOR_INTERFACES: [&str; 3] = [
    "takeImageDynamicSize", // This is actually implemented
    "takeImageStaticSize", // This is actually implemented
    "takeImage" // Not implemented but required to run camera module currently
];

/// List of supervisor interfaces when camera flag isnt enabled OR armv6 feature is enabled
#[cfg(any(not(feature="camera"), feature="armv6"))]
pub const SUPERVISOR_INTERFACES: [&str; 0] = [
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

// Postfix to recognize serialized modules by
pub const SERIALIZED_MODULE_POSTFIX: &str = "SERIALIZED.wasm"; 

// Postfix to recognize modules that have specifically been serialized for pulley
pub const PULLEY_MODULE_POSTFIX: &str = "PULLEY.wasm"; 

// Name of the memory related to each module
pub const MEMORY_NAME: &str = "memory"; 

/// Ensures that all required directories for modules and parameter mounts exist.
///
/// This function should be ran in the main function before anything else.
pub fn ensure_required_folders() {
    fs::create_dir_all(&*MODULE_FOLDER).expect("Failed to create module folder");
    fs::create_dir_all(&*PARAMS_FOLDER).expect("Failed to create params folder");
}
