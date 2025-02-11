use supervisor::lib::wasmtime::{WasmtimeRuntime, ModuleConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use wasmtime::Val;

// ----------------------- Wasmtime related tests ----------------------- //
// TODO: Implement individual tests for each module
// TODO: Find another way of including modules to tests than having them directly in this repo
// NOTE: On a first run, might take a while as any/all modules you are testing take a while to compile

#[cfg(test)]
mod wasmtime_tests {
    use std::str;
    use super::*;
    const TEST_PATH: &str = "tests/";

    // Shared runtime for testing purposes
    // TODO: This is maybe how runtime should be implemented in supervisor. One runtime, many modules.
    // Get it with let r = RUNTIME.lock().unwrap();, and drop with drop(r);
    static RUNTIME: Lazy<Mutex<WasmtimeRuntime>> = Lazy::new(|| {
        match WasmtimeRuntime::new() {
            Ok(runtime) => Mutex::new(runtime),
            Err(err) => panic!("Failed to initialize WasmtimeRuntime: {}", err),
        }
    });

    #[test]
    fn test_module_loading() -> Result<(), Box<dyn std::error::Error>> {
        // stacktrace suppression: https://users.rust-lang.org/t/test-should-panic-somehow-hide-stack-traces/57715
        // let f = |_: &std::panic::PanicHookInfo| {};
        // std::panic::set_hook(Box::new(f));

        let module_paths: Vec<PathBuf> = vec![
            "camera.wasm",
            "fibo.wasm",
            "fibobin.wasm",
            "grayscale.wasm",
            "invert_colors.wasm",
            "stateful_fibo.wasm",
            "wasi_mobilenet_onnx.wasm"
        ].iter().map(|s| PathBuf::from(format!("{}{}", TEST_PATH, s))).collect();
        let mut runtime = RUNTIME.lock().unwrap();
        let mut failures_happened = false;

        // Test that modules load correctly
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

            // Load the module to runtime twice to test serialization and that loading twice doesnt cause issues          
            let load_result = runtime.load_module(config.clone());
            match load_result {
                Ok(_) => {
                    println!("✅ Successfully loaded: {}", module_name);
                    let _result_2 = runtime.load_module(config);
                }
                Err(e) => {
                    println!("❌ Failed to load {}: {}", module_name, e);
                    failures_happened = true;
                }
            }
            
        }

        // Test getting module exports and imports
        let mut module_exports: HashMap<String, Vec<String>> = HashMap::new();
        let mut module_list: Vec<String> = vec![];
        {
            let loaded_module_list = &runtime.modules;
            println!("\n\nList of modules that are currently loaded:\n{:?}\n", loaded_module_list.keys());
            for (name, module) in loaded_module_list.into_iter() {
                let imports = module.get_all_imports();
                let exports = module.get_all_exports();
                println!("\n{} has following imports and exports:\nImports:{:?}\nExports:\n{:?}\n", name, imports, exports);
                module_exports.insert(name.clone(), exports);
                module_list.push(name.clone());
            }
    
        }

        // Test getting function parameter and return types
        // for (name, exports) in module_exports.iter() {
        //     for export in exports {
        //         let (params, returns) = runtime.get_func_params(name, export);
        //         println!("{}':'{}', params: {:?}, returns: {:?}", export, name, params, returns);
        //     }
        // }

        // Test reading and writing to module memories
        for name in module_list {
            let mut write_buffer: [u8; 10] = *b"tabularasa";
            runtime.write_to_memory(&name, 0, &mut write_buffer)?;
            let mut read_buffer: [u8; 10] = [0; 10];
            runtime.read_from_memory(&name, 0, &mut read_buffer)?;
            let test_string: &str = str::from_utf8(&write_buffer)?;
            let result_string: &str = str::from_utf8(&read_buffer)?;
            if test_string != result_string {
                failures_happened = true;
            }
        }

        // Test camera module, take_image_predefined_path, uses takeImageStaticSize from general_utils.rs
        println!("Running camera test (static image size)...");
        let camera_parameters: Vec<Val> = Vec::new();
        let camera_return_num: usize = 1;
        let camera_response: Vec<Val> = runtime.run_function("camera.wasm", "take_image_predefined_path", camera_parameters, camera_return_num);
        println!("Camera response: {:?}", camera_response);

        // Test camera module, take_image, uses takeImageDynamicSize from general_utils.rs
        // TODO: Implement test

        drop(runtime);
        assert!(!failures_happened, "");
        Ok(())
    }
}