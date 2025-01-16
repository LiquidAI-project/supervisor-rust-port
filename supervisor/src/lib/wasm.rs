// wasm.rs

use std::env;
use std::sync::{Arc, Mutex};
use once_cell::sync::Lazy;
use log::info;

use crate::wasm_api::{IWasmRuntime, ModuleConfig}; // from your prior code
use crate::wasmtime_runtime::WasmtimeRuntime;  // We'll define in wasmtime.rs
use crate::wasm3_runtime::Wasm3Runtime;        // We'll define in wasm3.rs

// Determine which runtime to use
fn pick_runtime_type() -> &'static str {
    // default to "wasmtime" if WASM_RUNTIME is not set
    match env::var("WASM_RUNTIME") {
        Ok(val) if val == "wasm3" => "wasm3",
        _ => "wasmtime",
    }
}

pub static WASM_RUNTIME: Lazy<Mutex<Arc<dyn IWasmRuntime>>> = Lazy::new(|| {
    let runtime_type = pick_runtime_type();
    info!("Using {} as WASM runtime.", runtime_type);

    let runtime: Arc<dyn IWasmRuntime> = match runtime_type {
        "wasm3" => Arc::new(Wasm3Runtime::new()),
        _ => Arc::new(WasmtimeRuntime::new()),
    };
    Mutex::new(runtime)
});

// We can store the modules in a global if desired
pub static WASM_MODULES: Lazy<Mutex<Vec<ModuleConfig>>> = Lazy::new(|| {
    Mutex::new(Vec::new())
});
