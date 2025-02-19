pub const DEFAULT_PORT: u16 = 8080;
pub const DEFAULT_URL_SCHEME: &str = "http";
pub const URL_BASE_PATH: &str = "/file/device/discovery/register";
pub const SUPERVISOR_DEFAULT_NAME: &str = "supervisor";

// Supervisor interfaces are imported functions available to wasmtime runtime.
pub const SUPERVISOR_INTERFACES: [&str; 2] = [
    "takeImageDynamicSize",
    "takeImageStaticSize"
];