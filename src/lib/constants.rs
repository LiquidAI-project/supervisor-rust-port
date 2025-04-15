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

/// Functions provided for the camera module
pub const CAMERA_FUNCTIONS: &[&str] = &[
    "takeImageDynamicSize",
    "takeImageStaticSize",
    "takeImage"
];

/// Functions provided by wasip1 for use by modules compiled for wasm32-wasip1 target
pub const WASI_FUNCTIONS: &[&str] = &[
    "args_get",
    "args_sizes_get",
    "environ_get",
    "environ_sizes_get",
    "clock_res_get",
    "clock_time_get",
    "fd_advise",
    "fd_allocate",
    "fd_close",
    "fd_datasync",
    "fd_fdstat_get",
    "fd_fdstat_set_flags",
    "fd_fdstat_set_rights",
    "fd_filestat_get",
    "fd_filestat_set_size",
    "fd_filestat_set_times",
    "fd_pread",
    "fd_prestat_get",
    "fd_prestat_dir_name",
    "fd_pwrite",
    "fd_read",
    "fd_readdir",
    "fd_renumber",
    "fd_seek",
    "fd_sync",
    "fd_tell",
    "fd_write",
    "path_create_directory",
    "path_filestat_get",
    "path_filestat_set_times",
    "path_link",
    "path_open",
    "path_readlink",
    "path_remove_directory",
    "path_rename",
    "path_symlink",
    "path_unlink_file",
    "poll_oneoff",
    "proc_exit",
    "proc_raise",
    "sched_yield",
    "random_get",
    "sock_accept",
    "sock_recv",
    "sock_send",
    "sock_shutdown"
];

/// The list of supervisor interfaces imported into WASM modules.
/// Generated based on enabled features.
pub static SUPERVISOR_INTERFACES: Lazy<Vec<&'static str>> = Lazy::new(|| {
    let mut interfaces = Vec::new();

    // Camera functionality is available for all architectures supported by supervisor
    interfaces.extend_from_slice(CAMERA_FUNCTIONS);

    #[cfg(not(feature = "armv6"))] 
    {
        // Wasi functionalities are not available on armv6 architecture
        interfaces.extend_from_slice(WASI_FUNCTIONS);
    }

    interfaces
});

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
