use supervisor::lib::wasmtime::{WasmtimeModule, WasmtimeRuntime, ModuleConfig, MLModel};
use std::collections::HashMap;

// ----------------------- Wasmtime related tests ----------------------- //
// TODO: Improve the individual tests for each module
// TODO: Find another way of including modules to tests than having them directly in this repo

#[cfg(test)]
mod wasmtime_tests {
    use super::*;
    const TEST_PATH: &str = "tests/";

    #[test]
    fn test_camera() -> Result<(), Box<dyn std::error::Error>> {
        // stacktace suppression: https://users.rust-lang.org/t/test-should-panic-somehow-hide-stack-traces/57715
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));
        let module_name = "camera.wasm";
        let data_files: HashMap<String, String> = HashMap::new();
        let mut runtime = WasmtimeRuntime::new();
        let module_path = format!("{}{}", TEST_PATH, module_name);
        let config = ModuleConfig::new(
            "aaaa-bbbbb-ccccc".to_string(),
            "camera.wasm".to_string(),
            module_path.clone(),
            data_files.clone(),
            None,
        );
        let result = runtime.load_module(config);
        match result {
            Ok(_) => {
                println!("✅ Successfully loaded: {}", module_name);
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", module_name, e);
                assert!(false, "");
            }
        }
        if let Some(module) = runtime.modules.get("camera.wasm") {
            let funcs = module.get_all_functions();
            println!("Module {} has following function imports: {:?}", module.name, funcs);
            Ok(())
        } else {
            Ok(())
        }
    }

    #[test]
    fn test_fibo() -> Result<(), Box<dyn std::error::Error>> {
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));
        let module_name = "fibo.wasm";
        let data_files: HashMap<String, String> = HashMap::new();
        let mut runtime = WasmtimeRuntime::new();
        let module_path = format!("{}{}", TEST_PATH, module_name);
        let config = ModuleConfig::new(
            "aaaa-bbbbb-ccccc".to_string(),
            "fibo.wasm".to_string(),
            module_path.clone(),
            data_files.clone(),
            None,
        );
        let result = runtime.load_module(config);
        match result {
            Ok(_) => {
                println!("✅ Successfully loaded: {}", module_name);
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", module_name, e);
                assert!(false, "");
            }
        }
        if let Some(module) = runtime.modules.get("fibo.wasm") {
            let funcs = module.get_all_functions();
            println!("Module {} has following function imports: {:?}", module.name, funcs);
            Ok(())
        } else {
            Ok(())
        }
    }

    #[test]
    fn test_fibobin() -> Result<(), Box<dyn std::error::Error>> {
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));
        let module_name = "fibobin.wasm";
        let data_files: HashMap<String, String> = HashMap::new();
        let mut runtime = WasmtimeRuntime::new();
        let module_path = format!("{}{}", TEST_PATH, module_name);
        let config = ModuleConfig::new(
            "aaaa-bbbbb-ccccc".to_string(),
            "fibobin.wasm".to_string(),
            module_path.clone(),
            data_files.clone(),
            None,
        );
        let result = runtime.load_module(config);
        match result {
            Ok(_) => {
                println!("✅ Successfully loaded: {}", module_name);
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", module_name, e);
                assert!(false, "");
            }
        }
        if let Some(module) = runtime.modules.get("fibobin.wasm") {
            let funcs = module.get_all_functions();
            println!("Module {} has following function imports: {:?}", module.name, funcs);
            Ok(())
        } else {
            Ok(())
        }
    }

    #[test]
    fn test_grayscale() -> Result<(), Box<dyn std::error::Error>> {
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));
        let module_name = "greyscale.wasm";
        let data_files: HashMap<String, String> = HashMap::new();
        let mut runtime = WasmtimeRuntime::new();
        let module_path = format!("{}{}", TEST_PATH, module_name);
        let config = ModuleConfig::new(
            "aaaa-bbbbb-ccccc".to_string(),
            "greyscale.wasm".to_string(),
            module_path.clone(),
            data_files.clone(),
            None,
        );
        let result = runtime.load_module(config);
        match result {
            Ok(_) => {
                println!("✅ Successfully loaded: {}", module_name);
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", module_name, e);
                assert!(false, "");
            }
        }
        if let Some(module) = runtime.modules.get("grayscale.wasm") {
            let funcs = module.get_all_functions();
            println!("Module {} has following function imports: {:?}", module.name, funcs);
            Ok(())
        } else {
            Ok(())
        }
    }

    #[test]
    fn test_invert_colors() -> Result<(), Box<dyn std::error::Error>> {
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));
        let module_name = "invert_colors.wasm";
        let data_files: HashMap<String, String> = HashMap::new();
        let mut runtime = WasmtimeRuntime::new();
        let module_path = format!("{}{}", TEST_PATH, module_name);
        let config = ModuleConfig::new(
            "aaaa-bbbbb-ccccc".to_string(),
            "invert_colors.wasm".to_string(),
            module_path.clone(),
            data_files.clone(),
            None,
        );
        let result = runtime.load_module(config);
        match result {
            Ok(_) => {
                println!("✅ Successfully loaded: {}", module_name);
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", module_name, e);
                assert!(false, "");
            }
        }
        if let Some(module) = runtime.modules.get("invert_colors.wasm") {
            let funcs = module.get_all_functions();
            println!("Module {} has following function imports: {:?}", module.name, funcs);
            Ok(())
        } else {
            Ok(())
        }
    }

    #[test]
    fn test_stateful_fibo() -> Result<(), Box<dyn std::error::Error>> {
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));
        let module_name = "stateful_fibo.wasm";
        let data_files: HashMap<String, String> = HashMap::new();
        let mut runtime = WasmtimeRuntime::new();
        let module_path = format!("{}{}", TEST_PATH, module_name);
        let config = ModuleConfig::new(
            "aaaa-bbbbb-ccccc".to_string(),
            "stateful_fibo.wasm".to_string(),
            module_path.clone(),
            data_files.clone(),
            None,
        );
        let result = runtime.load_module(config);
        match result {
            Ok(_) => {
                println!("✅ Successfully loaded: {}", module_name);
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", module_name, e);
                assert!(false, "");
            }
        }
        if let Some(module) = runtime.modules.get("stateful_fibo.wasm") {
            let funcs = module.get_all_functions();
            println!("Module {} has following function imports: {:?}", module.name, funcs);
            Ok(())
        } else {
            Ok(())
        }
    }

    #[test]
    fn test_wasi_mobilenet_onnx() -> Result<(), Box<dyn std::error::Error>> {
        let f = |_: &std::panic::PanicHookInfo| {};
        std::panic::set_hook(Box::new(f));
        let module_name = "wasi_mobilenet_onnx.wasm";
        let data_files: HashMap<String, String> = HashMap::new();
        let mut runtime = WasmtimeRuntime::new();
        let module_path = format!("{}{}", TEST_PATH, module_name);
        let config = ModuleConfig::new(
            "aaaa-bbbbb-ccccc".to_string(),
            "wasi_mobilenet_onnx.wasm".to_string(),
            module_path.clone(),
            data_files.clone(),
            None,
        );
        let result = runtime.load_module(config);
        match result {
            Ok(_) => {
                println!("✅ Successfully loaded: {}", module_name);
            }
            Err(e) => {
                println!("❌ Failed to load {}: {}", module_name, e);
                assert!(false, "");
            }
        }
        if let Some(module) = runtime.modules.get("wasi_mobilenet_onnx.wasm") {
            let funcs = module.get_all_functions();
            println!("Module {} has following function imports: {:?}", module.name, funcs);
            Ok(())
        } else {
            Ok(())
        }
    }

}