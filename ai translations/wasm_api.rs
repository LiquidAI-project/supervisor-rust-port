// wasm_api.rs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::Read;
use std::error::Error;

// ------------------ Custom Error Types ------------------ //

#[derive(Debug)]
pub struct WasmRuntimeNotSetError;

impl std::fmt::Display for WasmRuntimeNotSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Wasm runtime has not been set properly.")
    }
}
impl Error for WasmRuntimeNotSetError {}

#[derive(Debug)]
pub struct IncompatibleWasmModule;

impl std::fmt::Display for IncompatibleWasmModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Attempted to mix incompatible Wasm modules.")
    }
}
impl Error for IncompatibleWasmModule {}

// ------------------ Data Structures ------------------ //

#[derive(Debug, Clone)]
pub struct ModuleConfig {
    pub id: String,
    pub name: String,
    pub path: String,
    pub data_files: HashMap<String, PathBuf>,
    pub ml_model: Option<MLModel>,
    pub data_ptr_function_name: String, // default "get_img_ptr"
}

impl ModuleConfig {
    pub fn new(
        id: &str,
        name: &str,
        path: &str,
        data_files: HashMap<String, PathBuf>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            data_files,
            ml_model: None,
            data_ptr_function_name: "get_img_ptr".to_string(),
        }
    }

    pub fn set_model_from_data_files(&mut self, key: &str) {
        if let Some(pb_path) = self.data_files.get(key) {
            // Create a new MLModel object
            let model = MLModel {
                path: pb_path.to_string_lossy().into_owned(),
                ..Default::default()
            };
            self.ml_model = Some(model);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MLModel {
    pub path: String,
    pub alloc_function_name: String,   // default "alloc"
    pub infer_function_name: String,   // default "infer_from_ptrs"
}

impl MLModel {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            alloc_function_name: "alloc".to_string(),
            infer_function_name: "infer_from_ptrs".to_string(),
        }
    }
}

// ------------------ WasmModule Trait & Struct ------------------ //

pub trait IWasmModule {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn path(&self) -> &str;

    fn runtime(&self) -> Option<&dyn IWasmRuntime>;
    fn set_runtime(&mut self, runtime: Box<dyn IWasmRuntime>);

    fn functions(&mut self) -> &Vec<String>;
    fn run_function(&mut self, function_name: &str, params: &[Box<dyn std::any::Any>]) -> Option<Box<dyn std::any::Any>>;

    // Memory-based operations
    fn run_data_function(
        &mut self,
        function_name: &str,
        data_ptr_function_name: &str,
        data: &[u8],
        params: &[Box<dyn std::any::Any>]
    ) -> Vec<u8>;

    fn upload_data(
        &mut self,
        data: &[u8],
        alloc_function: &str
    ) -> (Option<usize>, Option<usize>);

    fn upload_data_file(
        &mut self,
        data_file: &Path,
        alloc_function_name: &str
    ) -> (Option<usize>, Option<usize>);

    fn upload_ml_model(&mut self, ml_model: &MLModel) -> (Option<usize>, Option<usize>);
    fn run_ml_inference(&mut self, ml_model: &MLModel, data: &[u8]) -> Option<Box<dyn std::any::Any>>;
}

// This is an abstract struct to mimic the Python approach
pub struct WasmModuleBase {
    pub config: ModuleConfig,
    pub runtime: Option<Box<dyn IWasmRuntime>>,
    cached_functions: Option<Vec<String>>,
}

impl WasmModuleBase {
    pub fn new(config: ModuleConfig, runtime: Option<Box<dyn IWasmRuntime>>) -> Self {
        Self {
            config,
            runtime,
            cached_functions: None,
        }
    }

    // Helper to read file data
    fn read_file(path: &Path) -> Result<Vec<u8>, std::io::Error> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }
}

// Provide default "unimplemented" stubs or partial logic
impl IWasmModule for WasmModuleBase {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn path(&self) -> &str {
        &self.config.path
    }

    fn runtime(&self) -> Option<&dyn IWasmRuntime> {
        self.runtime.as_ref().map(|r| r.as_ref())
    }

    fn set_runtime(&mut self, runtime: Box<dyn IWasmRuntime>) {
        self.runtime = Some(runtime);
    }

    fn functions(&mut self) -> &Vec<String> {
        if self.cached_functions.is_none() {
            // In real usage, you'd parse the WASM for export names
            // For now, we simulate with an empty list
            self.cached_functions = Some(vec![]);
        }
        self.cached_functions.as_ref().unwrap()
    }

    fn run_function(&mut self, function_name: &str, _params: &[Box<dyn std::any::Any>]) -> Option<Box<dyn std::any::Any>> {
        eprintln!("run_function not implemented: {}", function_name);
        None
    }

    fn run_data_function(
        &mut self,
        function_name: &str,
        data_ptr_function_name: &str,
        data: &[u8],
        params: &[Box<dyn std::any::Any>]
    ) -> Vec<u8> {
        // 1. Get data pointer by calling data_ptr_function_name
        // 2. Write data
        // 3. Call function_name
        // 4. Read data
        eprintln!("run_data_function not implemented: {function_name}, {data_ptr_function_name}");
        Vec::new()
    }

    fn upload_data(
        &mut self,
        data: &[u8],
        alloc_function: &str
    ) -> (Option<usize>, Option<usize>) {
        eprintln!("upload_data not implemented: {alloc_function}");
        (None, None)
    }

    fn upload_data_file(
        &mut self,
        data_file: &Path,
        alloc_function_name: &str
    ) -> (Option<usize>, Option<usize>) {
        match Self::read_file(data_file) {
            Ok(data) => self.upload_data(&data, alloc_function_name),
            Err(err) => {
                eprintln!("Error reading file {}: {}", data_file.display(), err);
                (None, None)
            }
        }
    }

    fn upload_ml_model(&mut self, ml_model: &MLModel) -> (Option<usize>, Option<usize>) {
        let path = Path::new(&ml_model.path);
        self.upload_data_file(path, &ml_model.alloc_function_name)
    }

    fn run_ml_inference(&mut self, ml_model: &MLModel, data: &[u8]) -> Option<Box<dyn std::any::Any>> {
        // 1. Upload ml_model
        // 2. Upload data
        // 3. Run ml_model.infer_function_name
        eprintln!("run_ml_inference not implemented: alloc={}, infer={}",
                  ml_model.alloc_function_name, ml_model.infer_function_name);
        None
    }
}

// ------------------ WasmRuntime Trait & Struct ------------------ //

pub trait IWasmRuntime {
    fn modules(&self) -> &HashMap<String, Box<dyn IWasmModule>>;
    fn current_module_name(&self) -> Option<&str>;
    fn set_current_module_name(&mut self, name: Option<String>);

    // Module loading
    fn load_module(&mut self, config: ModuleConfig) -> Option<&Box<dyn IWasmModule>>;
    fn get_or_load_module(&mut self, config: Option<ModuleConfig>) -> Option<&Box<dyn IWasmModule>>;

    // Memory read/write
    fn read_from_memory(&self, address: usize, length: usize, module_name: Option<&str>) -> (Vec<u8>, Option<String>);
    fn write_to_memory(&self, address: usize, data: &[u8], module_name: Option<&str>) -> Option<String>;

    // Running a function
    fn run_function(&mut self, function_name: &str, params: &[Box<dyn std::any::Any>], module_name: Option<&str>) -> Option<Box<dyn std::any::Any>>;
}

pub struct WasmRuntimeBase {
    pub modules_map: HashMap<String, Box<dyn IWasmModule>>,
    pub current_module: Option<String>,
    pub functions_cache: Option<HashMap<String, String>>,  // function_name -> module_name
}

impl WasmRuntimeBase {
    pub fn new() -> Self {
        Self {
            modules_map: HashMap::new(),
            current_module: None,
            functions_cache: None,
        }
    }
}

impl IWasmRuntime for WasmRuntimeBase {
    fn modules(&self) -> &HashMap<String, Box<dyn IWasmModule>> {
        &self.modules_map
    }

    fn current_module_name(&self) -> Option<&str> {
        self.current_module.as_deref()
    }

    fn set_current_module_name(&mut self, name: Option<String>) {
        self.current_module = name;
    }

    fn load_module(&mut self, config: ModuleConfig) -> Option<&Box<dyn IWasmModule>> {
        // In a real implementation, you'd instantiate a WasmModuleBase or specialized module
        // and store it in modules_map
        let module_name = config.name.clone();
        let module = Box::new(WasmModuleBase::new(config, None));
        self.modules_map.insert(module_name.clone(), module);
        self.modules_map.get(&module_name)
    }

    fn get_or_load_module(&mut self, config: Option<ModuleConfig>) -> Option<&Box<dyn IWasmModule>> {
        match config {
            None => None,
            Some(cfg) => {
                if let Some(module) = self.modules_map.get(&cfg.name) {
                    Some(module)
                } else {
                    self.load_module(cfg)
                }
            }
        }
    }

    fn read_from_memory(&self, address: usize, length: usize, module_name: Option<&str>) -> (Vec<u8>, Option<String>) {
        eprintln!("read_from_memory not implemented. address={address}, length={length}, module_name={:?}", module_name);
        (Vec::new(), Some("not implemented".to_string()))
    }

    fn write_to_memory(&self, address: usize, data: &[u8], module_name: Option<&str>) -> Option<String> {
        eprintln!("write_to_memory not implemented. address={address}, module_name={:?}", module_name);
        Some("not implemented".to_string())
    }

    fn run_function(&mut self, function_name: &str, params: &[Box<dyn std::any::Any>], module_name: Option<&str>) -> Option<Box<dyn std::any::Any>> {
        if let Some(mod_name) = module_name {
            // Check if that module is loaded
            if let Some(module) = self.modules_map.get_mut(mod_name) {
                return module.run_function(function_name, params);
            } else {
                eprintln!("Module '{}' not found!", mod_name);
                return None;
            }
        }

        // Otherwise, search all modules
        for (mod_name, module) in self.modules_map.iter_mut() {
            let maybe_func = module.functions().iter().find(|f| f.as_str() == function_name);
            if maybe_func.is_some() {
                // Found the function
                return module.run_function(function_name, params);
            }
        }
        None
    }
}
