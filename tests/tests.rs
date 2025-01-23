use supervisor;

// ----------------------- Wasmtime related tests ----------------------- //
// TODO: Add test for each existing example .wasm file
// TODO: Find another way of including modules to tests than having them directly in this repo

#[cfg(test)]
mod wasmtime_tests {

    #[test]
    fn fibonacci_test() -> Result<(), Box<dyn std::error::Error>> {
        let fib_result: i64 = supervisor::lib::wasmtime::run_fibonacci()?;
        println!("Fibonacci result for 5: {}", fib_result);
        Ok(())
    }
    
    #[test]
    fn test_wasmtime_runtime_instance() {
        let _ = supervisor::lib::wasmtime::WasmtimeRuntime::new();
    }

}