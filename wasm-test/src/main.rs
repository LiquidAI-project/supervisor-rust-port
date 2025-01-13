use std::env;
use std::fs;
use std::path::PathBuf;
use wasmtime::*;

fn main() -> anyhow::Result<()> {
    // Get the current working directory
    let current_dir: PathBuf = match env::current_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Error getting current directory: {}", err);
            return Ok(()); // Exit the program on error
        }
    };

    println!("Current folder: {}", current_dir.display());

    // Get the list of `.wasm` files in the current directory
    let wasm_files: Vec<PathBuf> = match fs::read_dir(&current_dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "wasm")
                    .unwrap_or(false)
            })
            .collect(),
        Err(err) => {
            eprintln!("Error reading current directory: {}", err);
            return Ok(()); // Exit the program on error
        }
    };

    println!("Found WASM files:");
    for wasm_file in &wasm_files {
        println!("- {}", wasm_file.display());
    }

    // Analyze the first `.wasm` file if one exists
    if let Some(wasm_file) = wasm_files.first() {
        println!("Analyzing WASM file: {}", wasm_file.display());

        // Create a Wasmtime engine and module
        let engine = Engine::default();
        let module = Module::from_file(&engine, wasm_file)?;

        // Create a store and instantiate the module
        let mut store = Store::new(&engine, ());
        let instance = Instance::new(&mut store, &module, &[])?;
        let exports: Vec<_> = instance.exports(&mut store).collect();

        println!("Available functions in the module:");
        for export in exports {
            let export_name = export.name().to_string(); // Capture name before moving
            println!("{}",export_name);
            // if let Some(func) = export.into_func() {
            //     let ty = func.ty(&mut store);
            //     let params = ty.params().collect::<Vec<_>>();
            //     let results = ty.results().collect::<Vec<_>>();

            //     println!(
            //         "- {}: ({:?}) -> ({:?})",
            //         export_name,
            //         params,
            //         results
            //     );
            // }
        }
    } else {
        println!("No WASM files found to analyze.");
    }

    Ok(())
}
