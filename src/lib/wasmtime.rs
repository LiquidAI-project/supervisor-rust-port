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
    pub fn load_module() {
        unimplemented!();
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
    // TODO: 
}

impl WasmtimeModule {

    /// Gets the wasmtime linear memory
    pub fn get_memory() {
        unimplemented!();
    }

    /// Gets a function with given name from current wasm module
    pub fn get_function() {
        unimplemented!();
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
    pub fn run_function() {
        unimplemented!();
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

