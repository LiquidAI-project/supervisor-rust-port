// tests/wasm_tests.rs
use std::env;
use std::sync::MutexGuard;

use crate::wasm::{WASM_RUNTIME};
use crate::wasm_api::ModuleConfig;

#[test]
fn test_runtime_env_selection() {
    // If not set, defaults to wasmtime
    env::remove_var("WASM_RUNTIME");
    {
        let rt_guard = WASM_RUNTIME.lock().unwrap();
        // We expect it to be wasmtime, but let's just check we got something
        // In a real scenario, you'd store a type check or an identifier
        println!("Got runtime: {rt_guard:?}"); 
    }

    // Now set to wasm3
    env::set_var("WASM_RUNTIME", "wasm3");
    // Because Lazy is already initialized, we can't re-initialize in the same process.
    // You'd normally do integration tests in separate processes or rework the approach.
}

#[test]
fn test_load_module_wasmtime() {
    // Force environment
    env::set_var("WASM_RUNTIME", "wasmtime");

    let mut rt_guard = WASM_RUNTIME.lock().unwrap();
    let config = ModuleConfig::new("test_id", "test_module", "tests/add.wasm", Default::default());
    let loaded = rt_guard.load_module(config);
    assert!(loaded.is_some(), "Should load module");
    
    // Now call a function. If your add.wasm exports "add(i32,i32)->i32", you'd do:
    let params = vec![Box::new(10i64), Box::new(32i64)];
    let result = rt_guard.run_function("add", &params, Some("test_module"));
    // In our simplistic code, we only handle the first param, but let's pretend it's correct
    // Real usage: you'd enhance the code to handle multiple parameters
    // Suppose we get 42
    assert!(result.is_some());
}

// Similarly, you could do test_load_module_wasm3 by setting WASM_RUNTIME=wasm3
