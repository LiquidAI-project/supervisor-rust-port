
use anyhow::Result;
use wasmtime;

#[cfg(test)]
mod tests {
    use super::*; // Bring the parent module scope into the test module

    #[test]
    fn test_import() -> Result<(), Box<dyn std::error::Error>> {
        let _ = run_wat_module();
        let fib_result: i64 = run_fibonacci()?;
        println!("Fibonacci result: {}", fib_result);
        Ok(())
    }
}

fn run_wat_module() -> Result<()> {
    // Modules can be compiled through either the text or binary format
    let engine = wasmtime::Engine::default();
    let wat = r#"
        (module
            (import "host" "host_func" (func $host_hello (param i32)))

            (func (export "hello")
                i32.const 3
                call $host_hello)
        )
    "#;
    let module = wasmtime::Module::new(&engine, wat)?;

    // Create a `Linker` which will be later used to instantiate this module.
    // Host functionality is defined by name within the `Linker`.
    let mut linker = wasmtime::Linker::new(&engine);
    linker.func_wrap("host", "host_func", |caller: wasmtime::Caller<'_, u32>, param: i32| {
        println!("Got {} from WebAssembly", param);
        println!("my host state is: {}", caller.data());
    })?;

    // All wasm objects operate within the context of a "store". Each
    // `Store` has a type parameter to store host-specific data, which in
    // this case we're using `4` for.
    let mut store = wasmtime::Store::new(&engine, 4);
    let instance = linker.instantiate(&mut store, &module)?;
    let hello = instance.get_typed_func::<(), ()>(&mut store, "hello")?;

    // And finally we can call the wasm!
    hello.call(&mut store, ())?;

    Ok(())
}

fn run_fibonacci() -> Result<i64, Box<dyn std::error::Error>> {
    let engine: wasmtime::Engine = wasmtime::Engine::default();
    let module:wasmtime::Module  = wasmtime::Module::from_file(&engine, "tests/fibo.wasm")?;
    let linker: wasmtime::Linker<u32> = wasmtime::Linker::new(&engine);
    let mut store: wasmtime::Store<u32> = wasmtime::Store::new(&engine, 4);
    let instance: wasmtime::Instance = linker.instantiate(&mut store, &module)?;
    let fibo: wasmtime::TypedFunc<i64, i64> = instance.get_typed_func::<i64, i64>(&mut store, "fibo")?;
    let result: i64 = fibo.call(&mut store, 5)?;
    Ok(result)
}


// def _get_function(self, function_name: str) -> Optional[Func]:
// """Get a function from the Wasm module. If the function is not found, return None."""
// if self.runtime is None:
//     print("Runtime not set!")
//     return None
// if not isinstance(self.runtime, WasmtimeRuntime):
//     print("Runtime is not Wasmtime!")
//     return None
// if self._instance is None:
//     print("Instance not set!")
//     return None

// try:
//     func = self._instance.exports(self.runtime.store)[function_name]
//     if isinstance(func, Func):
//         return func
//     print(f"'{function_name}' is not a function!")
//     return None
// except RuntimeError:
//     print(f"Function '{function_name}' not found!")
//     return None



// def run_function(self, function_name: str, params: List[Any]) -> Any:
// """Run a function from the Wasm module and return the result."""
// if not isinstance(self.runtime, WasmtimeRuntime):
//     return None

// # TODO: this approach is used in order to allocate required memory to the correct module
// # in external functions like take_image. It is not thread safe and can cause problems.
// self.runtime.current_module_name = self.name

// func = self._get_function(function_name)
// if func is None:
//     print(f"Function '{function_name}' not found!")
//     return None

// print(f"({self.name}) Running function '{function_name}' with params: {params}")
// if not params:
//     return func(self.runtime.store)
// return func(self.runtime.store, *params)