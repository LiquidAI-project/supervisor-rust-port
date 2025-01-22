// wasmtime_runtime.rs

use wasmtime::{
    Config, Engine, Store, Linker, Module, Instance, Memory, Func, Val, ValType, TypedFunc
};
use std::path::Path;
use std::fs;
use std::sync::Arc;
use log::info;

use crate::wasm_api::{IWasmRuntime, IWasmModule, ModuleConfig, WasmRuntimeBase, WasmModuleBase};

pub struct WasmtimeRuntime {
    base: WasmRuntimeBase,
    engine: Engine,
    store: Store<()>,
    linker: Linker<()>,
    instances: std::collections::HashMap<String, Instance>,
}

impl WasmtimeRuntime {
    pub fn new() -> Self {
        let mut config = Config::default();
        // config stuff if needed
        let engine = Engine::new(&config).unwrap();
        let store = Store::new(&engine, ());
        let mut linker = Linker::new(&engine);
        // you can define WASI or other host functions here
        // linker.define_wasi(...) if you want

        Self {
            base: WasmRuntimeBase::new(),
            engine,
            store,
            linker,
            instances: std::collections::HashMap::new(),
        }
    }
}

impl IWasmRuntime for WasmtimeRuntime {
    fn modules(&self) -> &std::collections::HashMap<String, Box<dyn IWasmModule>> {
        self.base.modules()
    }

    fn current_module_name(&self) -> Option<&str> {
        self.base.current_module_name()
    }

    fn set_current_module_name(&mut self, name: Option<String>) {
        self.base.set_current_module_name(name);
    }

    fn load_module(&mut self, config: ModuleConfig) -> Option<&Box<dyn IWasmModule>> {
        if let Some(existing) = self.base.modules_map.get(&config.name) {
            info!("Module {} already loaded!", config.name);
            return Some(existing);
        }

        let path = Path::new(&config.path);
        let bytes = fs::read(path).ok()?;
        let module = Module::new(&self.engine, &bytes).ok()?;

        let instance = self.linker.instantiate(&mut self.store, &module).ok()?;
        let new_mod = Box::new(WasmtimeModule::new(config, instance)) as Box<dyn IWasmModule>;
        let name = new_mod.name().to_string();

        self.instances.insert(name.clone(), instance);
        self.base.modules_map.insert(name.clone(), new_mod);
        self.base.modules_map.get(&name)
    }

    fn get_or_load_module(&mut self, config: Option<ModuleConfig>) -> Option<&Box<dyn IWasmModule>> {
        self.base.get_or_load_module_impl(self, config)
    }

    fn read_from_memory(&self, address: usize, length: usize, module_name: Option<&str>) -> (Vec<u8>, Option<String>) {
        let mod_name = if let Some(m) = module_name { m } else {
            return (vec![], Some("No module_name provided".to_string()));
        };
        let inst = self.instances.get(mod_name);
        if let Some(inst) = inst {
            if let Some(mem) = find_first_memory(inst, &self.store) {
                let data = mem.data(&self.store);
                if address + length <= data.len() {
                    (data[address..address+length].to_vec(), None)
                } else {
                    (vec![], Some("Out of bounds read".to_string()))
                }
            } else {
                (vec![], Some("No memory found".to_string()))
            }
        } else {
            (vec![], Some(format!("Instance {} not found", mod_name)))
        }
    }

    fn write_to_memory(&self, address: usize, data: &[u8], module_name: Option<&str>) -> Option<String> {
        let mod_name = module_name.unwrap_or("");
        let inst = self.instances.get(mod_name);
        if let Some(inst) = inst {
            if let Some(mem) = find_first_memory(inst, &self.store) {
                let mut mem_data = mem.data_mut(&self.store);
                if address + data.len() <= mem_data.len() {
                    mem_data[address..address+data.len()].copy_from_slice(data);
                    None
                } else {
                    Some("Out of bounds write".to_string())
                }
            } else {
                Some("No memory found".to_string())
            }
        } else {
            Some(format!("Instance {} not found", mod_name))
        }
    }

    fn run_function(&mut self, function_name: &str, params: &[Box<dyn std::any::Any>], module_name: Option<&str>)
        -> Option<Box<dyn std::any::Any>>
    {
        let mod_name = module_name?;
        let inst = self.instances.get(&mod_name)?;

        // Suppose the function takes a single i32 param.
        let param_i32 = if !params.is_empty() {
            let v = params[0].downcast_ref::<i64>().unwrap_or(&0i64);
            *v as i32
        } else {
            0
        };

        let export = inst.get_func(&mut self.store, function_name)?;
        // For demonstration, let's interpret as (i32) -> i32
        let typed: TypedFunc<i32, i32> = export.typed(&mut self.store).ok()?;
        let result = typed.call(&mut self.store, param_i32).ok()?;
        Some(Box::new(result) as Box<dyn std::any::Any>)
    }
}

// Utility function to get the first memory
fn find_first_memory(inst: &Instance, store: &Store<()>) -> Option<Memory> {
    for export in inst.exports(store) {
        if let Some(mem) = export.into_memory() {
            return Some(mem);
        }
    }
    None
}

// Now the WasmtimeModule
pub struct WasmtimeModule {
    base: WasmModuleBase,
    instance: Instance,
}

impl WasmtimeModule {
    pub fn new(config: ModuleConfig, instance: Instance) -> Self {
        let base = WasmModuleBase::new(config, None);
        Self { base, instance }
    }
}

impl IWasmModule for WasmtimeModule {
    fn id(&self) -> &str { &self.base.config.id }
    fn name(&self) -> &str { &self.base.config.name }
    fn path(&self) -> &str { &self.base.config.path }
    fn runtime(&self) -> Option<&dyn IWasmRuntime> { self.base.runtime.as_ref().map(|r| r.as_ref()) }
    fn set_runtime(&mut self, rt: Box<dyn IWasmRuntime>) {
        self.base.runtime = Some(rt);
    }
    fn functions(&mut self) -> &Vec<String> {
        self.base.functions()
    }
    fn run_function(&mut self, function_name: &str, params: &[Box<dyn std::any::Any>]) -> Option<Box<dyn std::any::Any>> {
        None
    }
    fn run_data_function(&mut self, _function_name: &str, _data_ptr_function_name: &str, _data: &[u8], _params: &[Box<dyn std::any::Any>]) -> Vec<u8> {
        Vec::new()
    }
    fn upload_data(&mut self, _data: &[u8], _alloc_function: &str) -> (Option<usize>, Option<usize>) {
        (None, None)
    }
    fn upload_data_file(&mut self, _data_file: &std::path::Path, _alloc_function_name: &str) -> (Option<usize>, Option<usize>) {
        (None, None)
    }
    fn upload_ml_model(&mut self, _ml_model: &crate::wasm_api::MLModel) -> (Option<usize>, Option<usize>) {
        (None, None)
    }
    fn run_ml_inference(&mut self, _ml_model: &crate::wasm_api::MLModel, _data: &[u8]) -> Option<Box<dyn std::any::Any>> {
        None
    }
}
