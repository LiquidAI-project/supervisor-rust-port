//! Compiles .wasm files form source folder into pulley bytecode to target folder

use anyhow::Result;
use wasmtime::{Config, Engine};
use std::{fs, path::Path};

fn main() -> Result<()> {
    let input_dir = Path::new("pulley_modules_input");
    let output_dir = Path::new("pulley_modules_output");

    fs::create_dir_all(output_dir)?;

    let mut config = Config::new();
    config.target("pulley32")?;
    let engine = Engine::new(&config)?;

    for entry in fs::read_dir(input_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map(|ext| ext == "wasm").unwrap_or(false) {
            let module_name = path.file_stem().unwrap().to_string_lossy();
            let wasm_bytes = fs::read(&path)?;
            let compiled = engine.precompile_module(&wasm_bytes)?;

            let output_path = output_dir.join(format!("{}.pulleyc", module_name));
            fs::write(&output_path, compiled)?;
            println!("✅ Compiled {} → {}", path.display(), output_path.display());
        }
    }

    Ok(())
}
