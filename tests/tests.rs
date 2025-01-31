use supervisor::lib::wasmtime::{WasmtimeRuntime, ModuleConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::sync::Mutex;

// ----------------------- Wasmtime related tests ----------------------- //
// TODO: Improve the individual tests for each module
// TODO: Find another way of including modules to tests than having them directly in this repo

// NOTE: On a first run, might take a while as any/all modules you are testing take a while to compile

#[cfg(test)]
mod wasmtime_tests {
    use super::*;
    const TEST_PATH: &str = "tests/";

    // Shared runtime for testing purposes
    // TODO: This is maybe how runtime should be implemented in supervisor. One runtime, many modules.
    // Get it with let r = RUNTIME.lock().unwrap();, and drop with drop(r);
    static RUNTIME: Lazy<Mutex<WasmtimeRuntime>> = Lazy::new(|| {
        Mutex::new(WasmtimeRuntime::new())
    });

    #[test]
    fn test_module_loading() -> Result<(), Box<dyn std::error::Error>> {
        // stacktrace suppression: https://users.rust-lang.org/t/test-should-panic-somehow-hide-stack-traces/57715
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));

        let module_paths: Vec<PathBuf> = vec![
            "camera.wasm",
            "fibo-wasm",
            "fibobin.wasm",
            "grayscale.wasm",
            "invert_colors.wasm",
            "stateful_fibo.wasm",
            "wasi_mobilenet_onnx.wasm"
        ].iter().map(|s| PathBuf::from(format!("{}{}", TEST_PATH, s))).collect();
        let mut runtime = RUNTIME.lock().unwrap();
        let mut failures_happened = false;

        for module_path in module_paths {
            let module_name = module_path.file_name().expect("Failed to get the filename of a module").to_string_lossy().to_string();
            let data_files: HashMap<String, String> = HashMap::new();
            let config = ModuleConfig::new(
                Uuid::new_v4().to_string(),
                module_name.clone(),
                module_path,
                data_files.clone(),
                None,
            );

            // Load the module to runtime twice to test serialization and that runtime doesnt load modules twice            
            let result_1 = runtime.load_module(config.clone());
            match result_1 {
                Ok(_) => {
                    println!("✅ Successfully loaded: {}", module_name);
                    println!("Current list of modules in runtime: {:?}", runtime.modules.keys());
                    let _result_2 = runtime.load_module(config);
                    println!("After attempting to reload module, list of modules is: {:?}", runtime.modules.keys());
                }
                Err(e) => {
                    println!("❌ Failed to load {}: {}", module_name, e);
                    failures_happened = true;
                }
            }
            
        }

        let loaded_module_list = &runtime.modules;
        println!("\n\nList of modules that are currently loaded:\n{:?}\n", loaded_module_list.keys());
        for (name, module) in loaded_module_list.into_iter() {
            let imports = module.get_all_imports();
            let exports = module.get_all_exports();
            println!("\nModule {} has following imports and exports:\nImports:\n{:?}\nExports:\n{:?}\n", name, imports, exports);
        }

        drop(runtime);
        assert!(!failures_happened, "");
        Ok(())
    }

}