//!
//! This module contains all functionality related to wasmtime
//! 


use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use wasmtime;


// TODO: Module serialization, also how necessary is it? (details in https://docs.wasmtime.dev/api/wasmtime/struct.Module.html )
// TODO: Implement missing functionality
// TODO: Serialize every module right after they are received, and also serialize every available module of startup?

// ----------------------- Wasmtime Runtime related functionality (check python source) ----------------------- //

const SERIALIZED_MODULE_POSTFIX: &str = "SERIALIZED.wasm";

pub struct WasmtimeRuntime {
    pub engine: wasmtime::Engine,
    pub store: wasmtime::Store<()>,
    pub linker: wasmtime::Linker<()>,
    pub modules: HashMap<String, WasmtimeModule>,
    pub functions: Option<HashMap<String, WasmtimeModule>>,
    pub current_module_name: Option<String> // TODO: Does this have any purpose?
}

impl WasmtimeRuntime {

    /// Initializes a new wasmtime runtime
    pub fn new() -> Self {
        let config: wasmtime::Config = wasmtime::Config::default();
        let engine: wasmtime::Engine = wasmtime::Engine::new(&config).unwrap();
        let store: wasmtime::Store<()> = wasmtime::Store::new(&engine, ());
        let linker: wasmtime::Linker<()> = wasmtime::Linker::new(&engine);
        let modules: HashMap<String, WasmtimeModule> = HashMap::new();
        let functions = None; // TODO: What exactly should this be?
        let current_module_name = None;
        Self {
            engine,
            store,
            linker,
            modules,
            functions,
            current_module_name
        }
    }

    pub fn load_module(&mut self, config: ModuleConfig) -> Result<(), Box<dyn std::error::Error>>{
        if !self.modules.contains_key(&config.name){
            let module_name: String = config.name.clone();

            let path_serial = config.path.clone().with_extension(SERIALIZED_MODULE_POSTFIX);
            let should_compile: bool;
            if fs::metadata(&path_serial).is_ok() {
                // Serialized version of the module exists already, check timestamps. If serialized version is older, recompile.
                let unserialized_module_modified = fs::metadata(&config.path)?.modified()?;
                let serialized_module_modified = fs::metadata(&path_serial)?.modified()?;
                should_compile = unserialized_module_modified > serialized_module_modified;
            } else {
                // Module should be serialized since the serialized version doesnt exist yet
                should_compile = true; 
            }
            if should_compile {
                // Compile and save a serialized version of the module
                let module = wasmtime::Module::from_file(&self.engine, config.path.clone())?;
                let module_bytes = module.serialize()?;
                fs::write(&path_serial, module_bytes)?;
            }

            let deserialized_module = unsafe { 
                // NOTE: The serialized module file being loaded can be tampered with, which could cause issues, which makes it unsafe.
                // For more info: https://docs.wasmtime.dev/api/wasmtime/struct.Module.html#method.deserialize_file
                wasmtime::Module::deserialize_file(&self.engine, &path_serial) 
            }?;
            let instance = self.linker.instantiate(&mut self.store, &deserialized_module)?;
            let mut wasmtime_module = WasmtimeModule::new(config)?;
            wasmtime_module.module = Some(deserialized_module);
            wasmtime_module.instance = Some(instance);
            self.modules.insert(module_name.clone(), wasmtime_module);
            println!("Module {} loaded successfully.", module_name);
        } else {
            println!("Module {} is already loaded.", &config.name);
        }
        Ok(())
    }

    /// Read from wasmtime runtime memory and return the result
    pub fn read_from_memory() {
        unimplemented!();
    }

    /// Write to wasmtime runtime memory
    pub fn write_to_memory() {
        unimplemented!();
    }

    // Link remote functions to wasmtime runtime for use by wasm modules.
    pub fn link_remote_functions() {
        // TODO: Use opencv for camera functionality
        unimplemented!();
    }
}



// ----------------------- Wasmtime module related functionality (check python source) ----------------------- //

pub struct WasmtimeModule {
    pub module: Option<wasmtime::Module>,
    pub instance: Option<wasmtime::Instance>,
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub functions: Option<Vec<String>>
}

impl WasmtimeModule {

    pub fn new(config: ModuleConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let module = None;
        let instance = None; 
        let functions = None;
        let wasmtime_module = WasmtimeModule {
            module: module,
            instance: instance,
            id: config.id,
            name: config.name,
            path: config.path,
            functions: functions
        };
        Ok(wasmtime_module)
    }

    /// Gets the names of all known imported functions from the module
    pub fn get_all_imports(&self) -> Vec<String> {
        if let Some(module_reference) = &self.module {
            let mut funcs: Vec<String> = vec![];
            let imports = module_reference.imports();
            for import in imports {
                if import.ty().func().is_some() {
                    funcs.push(import.name().to_string());
                }
            }
            return funcs;
        } else {
            return vec![];
        }
    }

    /// Gets a list of all known exported functions from the module
    pub fn get_all_exports(&self) -> Vec<String> {
        if let Some(module_reference) = &self.module {
            let mut funcs: Vec<String> = vec![];
            let imports = module_reference.exports();
            for import in imports {
                if import.ty().func().is_some() {
                    funcs.push(import.name().to_string());
                }
            }
            return funcs;
        } else {
            return vec![];
        }
    }

    /// Gets a function with given name from current wasm module
    pub fn get_function() {
        unimplemented!();
    }

    /// Gets the argument types of a function in the current wasm module
    pub fn get_arg_types() {
        unimplemented!();
    }

    /// Gets the wasmtime linear memory
    pub fn get_memory() {
        unimplemented!();
    }

    /// Run a function in the current wasm module with given parameters and return result
    pub fn run_function() {
        // let engine: wasmtime::Engine = wasmtime::Engine::default();
        // let module:wasmtime::Module  = wasmtime::Module::from_file(&engine, "tests/fibo.wasm")?;
        // let linker: wasmtime::Linker<u32> = wasmtime::Linker::new(&engine);
        // let mut store: wasmtime::Store<u32> = wasmtime::Store::new(&engine, 4);
        // let instance: wasmtime::Instance = linker.instantiate(&mut store, &module)?;
        // let fibo: wasmtime::TypedFunc<i64, i64> = instance.get_typed_func::<i64, i64>(&mut store, "fibo")?;
        // let result: i64 = fibo.call(&mut store, 5)?;
        unimplemented!();
    }

    /// Links remote functions to current module
    pub fn link_remote_functions() {
        unimplemented!();
    }

    pub fn functions() {
        unimplemented!();
    }

    pub fn run_data_function() {
        unimplemented!();
    }

    pub fn upload_data() {
        unimplemented!();
    }

}

// ----------------------- Miscellaneous module related things ----------------------- //

/// Struct for containing module name, file location and associated files referred to as 'mounts'. 
/// This is what a module instance for running functions is created based on.
#[derive(Clone)]
pub struct ModuleConfig {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub data_files: HashMap<String, String>,
    pub ml_model: Option<MLModel>,
    pub data_ptr_function_name: String
}

impl ModuleConfig {
    pub fn new(id: String, name: String, path: PathBuf, data_files: HashMap<String, String>, ml_model: Option<MLModel>) -> Self {
        ModuleConfig {
            id,
            name,
            path,
            data_files,
            ml_model,
            data_ptr_function_name: "get_image_ptr".to_string()
        }
    }

    pub fn set_model_from_data_files(&mut self, key: Option<String>) {
        let mut model_name: String = "model.pb".to_string();
        match key {
            Some(key_str) => model_name = key_str,
            None => ()
        }
        self.ml_model = Some(MLModel::new(model_name));
    }

}

/// Struct for ML models
#[derive(Clone)]
pub struct MLModel {
    pub path: String,
    pub alloc_function_name: String,
    pub infer_function_name: String
}

impl MLModel {
    pub fn new(path: String) -> Self {
        MLModel {
            path,
            alloc_function_name: "alloc".to_string(),
            infer_function_name: "infer_from_ptrs".to_string()
        }
    }
}
