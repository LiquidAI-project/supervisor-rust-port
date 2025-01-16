// // wasm3.rs

// use wasm3::Environment; // from wasm3-rs
// use wasm3::{Runtime as M3Runtime, Module as M3Module, Function as M3Function};
// use std::path::Path;
// use std::fs;
// use std::sync::Arc;
// use log::info;

// use crate::wasm_api::{IWasmRuntime, IWasmModule, ModuleConfig, WasmRuntimeBase, WasmModuleBase};

// pub struct Wasm3Runtime {
//     base: WasmRuntimeBase,
//     env: Environment,
//     rt: M3Runtime,
// }

// impl Wasm3Runtime {
//     pub fn new() -> Self {
//         let env = Environment::new().expect("Failed to create wasm3 Env");
//         let rt = env
//             .create_runtime(15_000)  // RUNTIME_INIT_MEMORY = 15000
//             .expect("Failed to create wasm3 Runtime");

//         Self {
//             base: WasmRuntimeBase::new(),
//             env,
//             rt,
//         }
//     }
// }

// impl IWasmRuntime for Wasm3Runtime {
//     fn modules(&self) -> &std::collections::HashMap<String, Box<dyn IWasmModule>> {
//         self.base.modules()
//     }

//     fn current_module_name(&self) -> Option<&str> {
//         self.base.current_module_name()
//     }

//     fn set_current_module_name(&mut self, name: Option<String>) {
//         self.base.set_current_module_name(name);
//     }

//     fn load_module(&mut self, config: ModuleConfig) -> Option<&Box<dyn IWasmModule>> {
//         if let Some(existing) = self.base.modules_map.get(&config.name) {
//             info!("Module {} already loaded!", config.name);
//             return Some(existing);
//         }

//         let module = Wasm3Module::new(config, self);
//         let boxed_mod = Box::new(module) as Box<dyn IWasmModule>;
//         let mod_name = boxed_mod.name().to_string();
//         self.base.modules_map.insert(mod_name.clone(), boxed_mod);

//         self.base.modules_map.get(&mod_name)
//     }

//     fn get_or_load_module(&mut self, config: Option<ModuleConfig>) -> Option<&Box<dyn IWasmModule>> {
//         self.base.get_or_load_module_impl(self, config)
//     }

//     fn read_from_memory(&self, address: usize, length: usize, module_name: Option<&str>) -> (Vec<u8>, Option<String>) {
//         // For simplicity, we just read from the first memory of the entire runtime.
//         // wasm3-rs does not have a direct "get_memory", but we can do:
//         match self.rt.try_get_memory(0) {
//             Ok(mem) => {
//                 let data = mem.as_slice();
//                 if address + length <= data.len() {
//                     (data[address..(address+length)].to_vec(), None)
//                 } else {
//                     (vec![], Some(format!("Out of bounds read: address={} length={}", address, length)))
//                 }
//             }
//             Err(e) => {
//                 (vec![], Some(format!("Failed to get memory: {}", e)))
//             }
//         }
//     }

//     fn write_to_memory(&self, address: usize, data: &[u8], module_name: Option<&str>) -> Option<String> {
//         match self.rt.try_get_memory(0) {
//             Ok(mem) => {
//                 let mut_data = mem.as_slice_mut();
//                 if address + data.len() <= mut_data.len() {
//                     mut_data[address..(address+data.len())].copy_from_slice(data);
//                     None
//                 } else {
//                     Some(format!("Out of bounds write: address={} length={}", address, data.len()))
//                 }
//             }
//             Err(e) => {
//                 Some(format!("Failed to get memory for write: {}", e))
//             }
//         }
//     }

//     fn run_function(&mut self, function_name: &str, params: &[Box<dyn std::any::Any>], module_name: Option<&str>)
//         -> Option<Box<dyn std::any::Any>>
//     {
//         // We'll pretend we only handle i32 parameters for demonstration.
//         let param_i32 = if !params.is_empty() {
//             // downcast to i64 then use i32
//             let v = params[0].downcast_ref::<i64>().unwrap_or(&0i64);
//             *v as i32
//         } else {
//             0
//         };

//         let func = match self.rt.find_function::<i32, i32>(function_name) {
//             Ok(f) => f,
//             Err(_) => {
//                 eprintln!("Function '{}' not found in wasm3 runtime", function_name);
//                 return None;
//             }
//         };

//         // Call with param_i32
//         match func.call(param_i32) {
//             Ok(ret) => Some(Box::new(ret) as Box<dyn std::any::Any>),
//             Err(e) => {
//                 eprintln!("Error calling function '{}' in wasm3: {}", function_name, e);
//                 None
//             }
//         }
//     }
// }

// // Now define Wasm3Module
// pub struct Wasm3Module {
//     base: WasmModuleBase,
//     // we might store a reference to the parsed module or the environment
// }

// impl Wasm3Module {
//     pub fn new(config: ModuleConfig, runtime: &Wasm3Runtime) -> Self {
//         let mut base = WasmModuleBase::new(config, None);
//         // This is where we'd actually parse the .wasm file:
//         // let path = Path::new(&base.config.path);
//         // let bytes = fs::read(path).unwrap();
//         // let parsed = runtime.env.parse_module(&bytes).unwrap();
//         // runtime.rt.load_module(parsed).unwrap();
//         Self { base }
//     }
// }

// impl IWasmModule for Wasm3Module {
//     fn id(&self) -> &str {
//         &self.base.config.id
//     }
//     fn name(&self) -> &str {
//         &self.base.config.name
//     }
//     fn path(&self) -> &str {
//         &self.base.config.path
//     }
//     fn runtime(&self) -> Option<&dyn IWasmRuntime> {
//         self.base.runtime.as_ref().map(|r| r.as_ref())
//     }
//     fn set_runtime(&mut self, rt: Box<dyn IWasmRuntime>) {
//         self.base.runtime = Some(rt);
//     }
//     fn functions(&mut self) -> &Vec<String> {
//         self.base.functions()
//     }
//     fn run_function(&mut self, function_name: &str, params: &[Box<dyn std::any::Any>]) -> Option<Box<dyn std::any::Any>> {
//         None
//     }
//     fn run_data_function(
//         &mut self,
//         _function_name: &str,
//         _data_ptr_function_name: &str,
//         _data: &[u8],
//         _params: &[Box<dyn std::any::Any>]
//     ) -> Vec<u8> {
//         Vec::new()
//     }
//     fn upload_data(&mut self, _data: &[u8], _alloc_function: &str) -> (Option<usize>, Option<usize>) {
//         (None, None)
//     }
//     fn upload_data_file(&mut self, _data_file: &std::path::Path, _alloc_function_name: &str) -> (Option<usize>, Option<usize>) {
//         (None, None)
//     }
//     fn upload_ml_model(&mut self, _ml_model: &crate::wasm_api::MLModel) -> (Option<usize>, Option<usize>) {
//         (None, None)
//     }
//     fn run_ml_inference(&mut self, _ml_model: &crate::wasm_api::MLModel, _data: &[u8]) -> Option<Box<dyn std::any::Any>> {
//         None
//     }
// }
