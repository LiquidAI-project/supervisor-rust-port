//!
//! This module contains all functionality related to wasmtime
//! 


use anyhow::Result;
use wasmtime;

// ----------------------- Wasmtime related functions etc... ----------------------- //
// TODO: Divide into classes etc like in python source?
// TODO: Implement missing functionality
// TODO: Are traits needed? In the original there were superclasses for wasm runtime and
// wasm modules, likely because they were using both wasm3 and wasmtime. Are both engines
// needed?

/// Testfunction that runs the fibonacci wasm module
pub fn run_fibonacci() -> Result<i64, Box<dyn std::error::Error>> {
    let engine: wasmtime::Engine = wasmtime::Engine::default();
    let module:wasmtime::Module  = wasmtime::Module::from_file(&engine, "tests/fibo.wasm")?;
    let linker: wasmtime::Linker<u32> = wasmtime::Linker::new(&engine);
    let mut store: wasmtime::Store<u32> = wasmtime::Store::new(&engine, 4);
    let instance: wasmtime::Instance = linker.instantiate(&mut store, &module)?;
    let fibo: wasmtime::TypedFunc<i64, i64> = instance.get_typed_func::<i64, i64>(&mut store, "fibo")?;
    let result: i64 = fibo.call(&mut store, 5)?;
    Ok(result)
}

/// Testfunction to run fibonacci module by using the fibo_from_pointers function
pub fn run_fibonacci2() -> Result<u64, Box<dyn std::error::Error>> {
    // Initialize engine, module, linker, and store.
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_file(&engine, "tests/fibo.wasm")?;
    let mut linker = wasmtime::Linker::new(&engine);
    let mut store = wasmtime::Store::new(&engine, ());

    // Instantiate the WASM module.
    let instance = linker.instantiate(&mut store, &module)?;

    // Acquire the exported linear memory named "memory".
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to find memory in the WASM module");

    // Acquire the `fibo_from_pointers` function.
    // Note: we map function signature (pointer types) to integer offsets.
    // So `(i32, i32, i32) -> i32` in Wasmtime.
    let fibo_from_pointers: wasmtime::TypedFunc<(i32, i32, i32), i32> = instance
        .get_typed_func(&mut store, "fibo_from_pointers")
        .expect("Failed to find `fibo_from_pointers` function");

    // The input we want to calculate the Fibonacci for.
    let input_val = 10u64;
    let input_bytes = input_val.to_le_bytes();

    // Offsets in the linear memory where we place our data.
    //  - We'll write the 8 input bytes at offset 0.
    //  - We'll store 1 byte for output length at offset 8.
    //  - The WASM function will return a pointer (offset) to the result bytes.
    let input_ptr = 0;
    let output_len_ptr = 8;

    // Write our input (8 bytes) to memory at offset 0.
    memory.write(&mut store, input_ptr, &input_bytes)?;

    // Initialize the output length byte to 0 at offset 8.
    memory.write(&mut store, output_len_ptr, &[0u8])?;

    // Now call `fibo_from_pointers(offset_of_input, length_of_input, offset_of_output_len)`.
    // This will return an offset (i32) to where the result bytes begin in WASM memory.
    let result_ptr = fibo_from_pointers.call(
        &mut store,
        (
            input_ptr as i32,
            input_bytes.len() as i32,
            output_len_ptr as i32,
        ),
    )?;

    // Read the output length (1 byte) from memory at offset 8.
    let mut out_len_buf = [0u8; 1];
    memory.read(&store, output_len_ptr, &mut out_len_buf)?;
    let output_len = out_len_buf[0] as usize;

    // Now read `output_len` bytes from memory at the `result_ptr` offset.
    let mut result_bytes = vec![0u8; output_len];
    memory.read(&store, result_ptr as usize, &mut result_bytes)?;

    // Convert the little-endian bytes to a `u64`.
    let result = u64::from_le_bytes(
        result_bytes
            .try_into()
            .expect("Expected 8 bytes for the Fibonacci result"),
    );

    Ok(result)
}


// ----------------------- Wasmtime Runtime related functionality (check python source) ----------------------- //

pub struct WasmtimeRuntime {
    engine: wasmtime::Engine,
    store: wasmtime::Store<()>,
    linker: wasmtime::Linker<()>
}

impl WasmtimeRuntime {

    /// Initializes a new wasmtime runtime
    pub fn new() -> Self {
        let config: wasmtime::Config = wasmtime::Config::default();
        let engine: wasmtime::Engine = wasmtime::Engine::new(&config).unwrap();
        let store: wasmtime::Store<()> = wasmtime::Store::new(&engine, ());
        let linker: wasmtime::Linker<()> = wasmtime::Linker::new(&engine);
        Self {
            engine,
            store,
            linker
        }
    }

    /// Loads a module into the wasmtime runtime
    /// // TODO: Add ModuleConfig instead of some reference bytes?
    pub fn load_module(&mut self, module_bytes: &[u8]) -> WasmtimeModule{
        let module = wasmtime::Module::new(&self.engine, module_bytes).unwrap();
        let instance = self.linker.instantiate(&mut self.store, &module).unwrap();
        WasmtimeModule { instance }
    }

    /// Read from wasmtime runtime memory and return the result
    pub fn read_from_memory(&self, memory: &wasmtime::Memory, offset: usize, length: usize) -> Vec<u8> {
        let data = memory.data(&self.store);
        data[offset..offset + length].to_vec()
    }

    /// Write to wasmtime runtime memory
    pub fn write_to_memory(&mut self, memory: &wasmtime::Memory, offset: usize, data: &[u8]) {
        let mem = memory.data_mut(&mut self.store);
        mem[offset..offset + data.len()].copy_from_slice(data);
    }

    // Link remote functions to wasmtime runtime for use by wasm modules.
    pub fn link_remote_functions() {
        // TODO: Use opencv for camera functionality
        unimplemented!();
    }
}



// ----------------------- Wasmtime module related functionality (check python source) ----------------------- //

pub struct WasmtimeModule {
    instance: wasmtime::Instance
}

impl WasmtimeModule {

    /// Gets the wasmtime linear memory
    pub fn get_memory(&self, store: &mut wasmtime::Store<()>, memory_name: &str) -> wasmtime::Memory {
        self.instance
            .get_memory(store, memory_name)
            .expect("Failed to get memory from module")
    }

    /// Gets a function with given name from current wasm module
    pub fn get_function(&self, store: &mut wasmtime::Store<()>, func_name: &str) -> wasmtime::Func {
        self.instance
            .get_func(store, func_name)
            .expect("Failed to get function from module")
    }

    /// Gets the names of all known functions in current wasm module
    pub fn get_all_functions() {
        unimplemented!();
    }

    /// Gets the argument types of a function in the current wasm module
    pub fn get_arg_types() {
        unimplemented!();
    }

    /// Run a function in the current wasm module with given parameters and return result
    pub fn run_function(
        &self,
        store: &mut wasmtime::Store<()>,
        func_name: &str,
        params: &[wasmtime::Val],
    ) -> Result<Vec<wasmtime::Val>, wasmtime::Trap> {
        // 1) Get the `Func` from the module instance
        let func = self.get_function(store, func_name);
    
        // 2) Determine how many results this function returns
        let ftype = func.ty(store);
        let results_count = ftype.results().len();
    
        // 3) Allocate a results buffer. The values written here will be 
        // overwritten by Wasmtime, so `Val::I32(0)` is just a placeholder.
        let mut results_buffer = vec![wasmtime::Val::I32(0); results_count];
    
        // 4) Call the function with your parameters and the results buffer
        func.call(&mut store, params, &mut results_buffer);
    
        // 5) Return the results
        Ok(results_buffer)
    }
    
    

    /// Loads the current module into the current wasm runtime
    pub fn load_module() {
        unimplemented!();
    }

    /// Links remote functions to current module
    pub fn link_remote_functions() {
        unimplemented!();
    }

}

