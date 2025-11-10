//! # wasmtime.rs
//!
//! This module provides the core WebAssembly runtime integration using Wasmtime.
//!
//! It is responsible for:
//! - Initializing and configuring the Wasmtime engine, store, and linker
//! - Loading and serializing/deserializing Wasm modules
//! - Instantiating modules with WASI support
//! - Providing utilities for memory access, function calling, and export/import inspection
//! - Managing runtime state across multiple modules
//!
//! This system enables invoking WebAssembly functions, accessing memory directly, linking
//! host functions (like camera access), and handling input/output bindings for modules.
//!
//! It also defines:
//! - `WasmtimeRuntime`: The central runtime manager
//! - `WasmtimeModule`: A single Wasm module instance
//! - `ModuleConfig`: Configuration structure used to load modules
//! - `MLModel`: Structure representing an associated machine learning model

use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use anyhow::Result;
use wasmtime::{Config, Engine, Func, FuncType, Instance, Linker, Memory, MemoryAccessError, Module, Store, Val, ValType};
#[cfg(not(feature="armv6"))]
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
#[cfg(not(feature="armv6"))]
use wasmtime_wasi::{WasiCtxBuilder, DirPerms, FilePerms};
use log::{info, error};
use crate::lib::wasmtime_imports;
use crate::lib::constants::{SERIALIZED_MODULE_POSTFIX, MEMORY_NAME};
use std::fmt;


// ----------------------- Wasmtime Runtime related functionality ----------------------- //

#[cfg(not(feature="armv6"))]
pub struct WasmtimeRuntime {
    pub engine: Engine,
    pub store: Store<WasiP1Ctx>,
    pub linker: Linker<WasiP1Ctx>,
    pub modules: HashMap<String, WasmtimeModule>,
    pub functions: Option<HashMap<String, WasmtimeModule>>,
}

#[cfg(feature="armv6")]
pub struct WasmtimeRuntime {
    pub engine: Engine,
    pub store: Store<()>,
    pub linker: Linker<()>,
    pub modules: HashMap<String, WasmtimeModule>,
    pub functions: Option<HashMap<String, WasmtimeModule>>,
}

#[cfg(not(feature="armv6"))]
impl fmt::Debug for WasmtimeRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmtimeRuntime")
            .field("engine", &"<Engine>")
            .field("store", &"<Store<WasiP1Ctx>>")
            .field("linker", &"<Linker<WasiP1Ctx>>")
            .field("modules", &self.modules)
            .field("functions", &self.functions)
            .finish()
    }
}

#[cfg(feature="armv6")]
impl fmt::Debug for WasmtimeRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmtimeRuntime")
            .field("engine", &"<Engine>")
            .field("store", &"<Store<()>>")
            .field("linker", &"<Linker<()>>")
            .field("modules", &self.modules)
            .field("functions", &self.functions)
            .finish()
    }
}


impl WasmtimeRuntime {

    #[cfg(not(feature="armv6"))]
    /// Initializes a new wasmtime runtime
    pub async fn new(data_dirs: Vec<(String, String)>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut config: Config = Config::default();
        config.async_support(true);
        let engine: Engine = Engine::new(&config).unwrap();
        let args = std::env::args().skip(1).collect::<Vec<_>>();
        let mut linker: Linker<WasiP1Ctx> = Linker::new(&engine);
        let mut wasi_ctx = WasiCtxBuilder::new();
        wasi_ctx.inherit_stdio();
        wasi_ctx.inherit_env();
        wasi_ctx.args(&args);
        // let preopened_dirs = [("./tests", ".")];
        let preopened_dirs = data_dirs;
        for (source, target) in preopened_dirs {
            wasi_ctx.preopened_dir(&source, &target, DirPerms::all(), FilePerms::all())?;
        }
        let wasi_p1 = wasi_ctx.build_p1();
        let store = Store::new(&engine, wasi_p1);
        preview1::add_to_linker_async(&mut linker, |t| t)?;
        let modules: HashMap<String, WasmtimeModule> = HashMap::new();
        let functions = None; // TODO: What exactly should this be?
        let mut runtime: WasmtimeRuntime = Self {
            engine,
            store,
            linker,
            modules,
            functions
        };
        runtime.link_remote_functions().await;
        Ok(runtime)
    }

    #[cfg(feature = "armv6")]
    /// Initializes a new wasmtime runtime in case that armv6 feature is enabled (no wasi-support etc)
    pub async fn new(_data_dirs: Vec<(String, String)>) -> Result<Self, Box<dyn std::error::Error>> {
        let engine = Engine::default();
        let store = Store::new(&engine, ());
        let linker = Linker::new(&engine);
        let modules = HashMap::new();
        let functions = None;
        let mut runtime = Self {
            engine,
            store,
            linker,
            modules,
            functions,
        };
        runtime.link_remote_functions().await;
        Ok(runtime)
    }



    pub async fn load_module(&mut self, config: ModuleConfig) -> Result<(), Box<dyn std::error::Error>>{
        if !self.modules.contains_key(&config.name){
            let module_name: String = config.name.clone();
            #[cfg(not(feature = "armv6"))]
            let path_serial = config.path.clone().with_extension(SERIALIZED_MODULE_POSTFIX);
            #[cfg(feature = "armv6")]
            let path_serial = config.path.clone().with_extension(PULLEY_MODULE_POSTFIX);
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
            #[cfg(not(feature = "armv6"))]
            if should_compile {
                // Compile and save a serialized version of the module
                let module = wasmtime::Module::from_file(&self.engine, config.path.clone())?;
                let module_bytes = module.serialize()?;
                fs::write(&path_serial, module_bytes)?;
            }
            #[cfg(feature = "armv6")]
            if should_compile {
                // Compilation is not supported on arm32 targets
                error!("Tried loading an unserialized module on armv6 device. This is not a supported operation. Loading module {} failed.", module_name);
                return Err("Compilation is not supported on armv6 targets. Precompiled/serialized .wasm modules must be used.".into());
            }
            let deserialized_module = unsafe {
                // NOTE: The serialized module file being loaded can be tampered with, which could cause issues, which makes it unsafe.
                // For more info: https://docs.wasmtime.dev/api/wasmtime/struct.Module.html#method.deserialize_file
                wasmtime::Module::deserialize_file(&self.engine, &path_serial)
            }?;
            #[cfg(not(feature = "armv6"))]
            let instance = self.linker.instantiate_async(&mut self.store, &deserialized_module).await?;
            #[cfg(feature = "armv6")]
            let instance = self.linker.instantiate(&mut self.store, &deserialized_module)?;
            let mut wasmtime_module = WasmtimeModule::new(config)?;
            wasmtime_module.module = Some(deserialized_module);
            wasmtime_module.instance = Some(instance);
            self.modules.insert(module_name.clone(), wasmtime_module);
        } else {
            info!("Module {} is already loaded.", &config.name);
        }
        Ok(())
    }


    /// Read from wasmtime default runtime memory and save results to buffer
    pub async fn read_from_memory(&mut self, module_name: &str, offset: usize, buffer: &mut [u8] ) -> Result<(), MemoryAccessError> {
        // Attempt to fill the given buffer by reading memory, starting from offset
        let _ = match self.get_memory(module_name, MEMORY_NAME).await {
            Some(memory) => memory.read(&self.store, offset, buffer)?,
            None => error!("Failed to read from module {} memory!", module_name)
        };
        Ok(())
    }


    /// Write buffer into default wasmtime runtime memory
    pub async fn write_to_memory(&mut self, module_name: &str, offset: usize, buffer: &mut [u8]) -> Result<(), MemoryAccessError> {
        // Attempt to write the contents of given buffer into memory, with offset being the starting position
        let _ = match self.get_memory(module_name, MEMORY_NAME).await {
            Some(memory) => memory.write(&mut self.store, offset, buffer)?,
            None => error!("Failed to write into module {} memory!", module_name)
        };
        Ok(())
    }


    /// Get a module from the list of modules in this runtime, if it exists there
    pub async fn get_module(&self, module_name: &str) -> Option<&WasmtimeModule> {
        let _ = match self.modules.get(module_name) {
            Some(m) => return Some(m),
            None => {
                error!("Module '{}' not found", module_name);
                return None
            }
        };
    }


    /// Get the instance of a module, if the module is loaded in the runtime and has an instance there
    pub async fn get_instance(&self, module_name: &str) -> Option<Instance> {
        let _ = match self.get_module(module_name).await {
            Some(m) => return m.instance,
            None => {
                error!("Instance for module '{}' not found", module_name);
                return None;
            }
        };
    }


    /// Link remote functions to wasmtime runtime for use by wasm modules.
    pub async fn link_remote_functions(&mut self) {

        /////////////////////////////////////////////////////////////////////
        // Camera related external functions
        /////////////////////////////////////////////////////////////////////

        let _ = &self.linker.func_new(
            "camera",
            "takeImageDynamicSize",
            FuncType::new(&self.engine, [ValType::I32, ValType::I32], []),
            wasmtime_imports::takeImageDynamicSize,
        );
        let _ = &self.linker.func_new(
            "camera",
            "takeImageStaticSize",
            FuncType::new(&self.engine, [ValType::I32, ValType::I32], []),
            wasmtime_imports::takeImageStaticSize,
        );
        let _ = &self.linker.func_new(
            "camera",
            "takeImage",
            FuncType::new(&self.engine, [ValType::I32, ValType::I32], []),
            wasmtime_imports::takeImage,
        );

        /////////////////////////////////////////////////////////////////////
        // Other external functions
        /////////////////////////////////////////////////////////////////////
        
        let _ = &self.linker.func_new_async(
            "network",
            "ping",
            FuncType::new(&self.engine, [ValType::I32, ValType::I32, ValType::I32, ValType::I32], [ValType::F32]),
            wasmtime_imports::ping,
        );

    }


    /// Gets the function parameters and returns of a given function in a given module
    pub async fn get_func_params(&mut self, module_name: &str, func_name: &str) -> (Vec<ValType>, Vec<ValType>) {
        let func = self.get_function(&module_name, &func_name).await;
        match func {
            Some(f) => {
                let func_ty = f.ty(&self.store);
                let param_types: Vec<ValType> = func_ty.params().collect();
                let return_types: Vec<ValType> = func_ty.results().collect();
                return (param_types, return_types)
            },
            None => return (vec![], vec![])
        }
    }


    /// Gets a function with given name from current wasm module
    pub async fn get_function(&mut self, module_name: &str, func_name: &str) -> Option<Func>{
        let _ = match self.get_instance(module_name).await {
            Some(i) => return i.get_func(&mut self.store, func_name),
            None => {
                error!("Function '{}' not found in module '{}'", func_name, module_name);
                return None
            }
        };
    }


    /// Gets the argument types of a function in the current wasm module
    pub async fn get_arg_types(&mut self, module_name: &str, func_name: &str) -> Vec<ValType> {
        let (params, _) = self.get_func_params(module_name, func_name).await;
        return params;
    }


    /// Gets the return types of a function in the current wasm module
    pub async fn get_return_types(&mut self, module_name: &str, func_name: &str) -> Vec<ValType> {
        let (_, returns) = self.get_func_params(module_name, func_name).await;
        return returns;
    }


    /// Gets the wasmtime linear memory
    pub async fn get_memory(&mut self, module_name: &str, memory_name: &str) -> Option<Memory> {
        let _ = match self.get_instance(module_name).await {
            Some(i) => return i.get_export(&mut self.store, memory_name).unwrap().into_memory(),
            None => return None
        };
    }


    /// Run a function in the current wasm module with given parameters and return a given number of results
    pub async fn run_function(&mut self, module_name: &str, func_name: &str, params: Vec<Val>, returns: usize) -> Vec<Val>{
        let params_thingy: &[Val] = &params;
        let returns_thingy: &mut[Val] = &mut vec![Val::I32(0); returns];
        info!("Attempting to run function {} from module {}...", func_name, module_name);
        let _ = match &self.get_function(module_name, func_name).await {
            Some(func) => {
                #[cfg(not(feature="armv6"))]
                {
                    func.call_async(&mut self.store, params_thingy, returns_thingy).await
                }
                #[cfg(feature="armv6")]
                func.call(&mut self.store, params_thingy, returns_thingy)
            }
            None => {
                error!("Failed to run function {} from module {}.", func_name, module_name);
                Ok(())
            }
        };
        info!("Ran module {:?} with function {:?} with params {:?}, result was {:?}.", module_name, func_name, params, returns_thingy.to_vec());
        return returns_thingy.to_vec();
    }


    /// Get the names of known functions in the given wasm module
    pub async fn functions(&self, module_name: &str) -> Vec<String> {
        let _ = match self.get_module(module_name).await {
            Some(module) => {
                return module.get_all_exports();
            }
            None => {
                error!("Failed to get list of functions for module {}.", module_name);
                return vec![]
            }
        };
    }

}


// ----------------------- Wasmtime module related functionality ----------------------- //

#[derive(Debug, Clone)]
pub struct WasmtimeModule {
    pub module: Option<Module>,
    pub instance: Option<Instance>,
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
}

// ----------------------- Miscellaneous module related things ----------------------- //

/// Struct for containing module name, file location and associated files referred to as 'mounts'.
/// This is what a module instance for running functions is created based on.
#[derive(Clone, Debug)]
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
#[derive(Clone, Debug)]
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
