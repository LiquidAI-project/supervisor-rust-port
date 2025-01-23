mod wasm_api;
mod wasm_utils;
mod wasm;
mod wasm3;
mod wasmtime;

#[cfg(test)]
mod tests {
    use super::*; // Bring the parent module scope into the test module

    #[test]
    fn test_import() {
        let a: u128 = wasm_utils::python_clock_ms();
        !println("{}", a);
    }

    // cargo test --test my_tests when tests are in /tests folder (TODO: move them there)
    // cargo test my_module when testing a module as is
    
    // #[test]
    // fn it_works() {
    //     let result = add(2, 2);
    //     assert_eq!(result, 4);
    // }
}
