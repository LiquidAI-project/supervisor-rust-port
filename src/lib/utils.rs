use std::{fs::File, path::PathBuf};
use crate::{lib::constants::{DEPLOYMENTS_FOLDER, MODULE_FOLDER, PARAMS_FOLDER}, structs::deployment_supervisor::{Deployment, Schema, SchemaType}};


/// Determines if a given schema can be represented by a primitive WebAssembly value.
///
/// Currently only checks for integer types, since they're directly mappable to WASM `i32` or `i64`.
///
/// # Arguments
/// * `schema` - The OpenAPI-style schema describing the output format.
///
/// # Returns
/// * `true` if the type is compatible with WASM primitive representation.
pub fn can_be_represented_as_wasm_primitive(schema: &Schema) -> bool {
    matches!(schema.r#type, SchemaType::INTEGER)
}

/// Builds the absolute host path for a file mounted into a specific module.
///
/// Used to resolve where a mounted file should live on the host filesystem (e.g. under `params/`).
///
/// # Arguments
/// * `deployment_id` - ID of the deployment the module belongs to
/// * `module_name` - Name of the module the file belongs to.
/// * `filename` - The relative path (mount path) used within the module.
///
/// # Returns
/// * `PathBuf` pointing to the correct location on disk.
pub fn module_mount_path(deployment_id: &str, module_name: &str, filename: &str) -> PathBuf {
    PARAMS_FOLDER.join(deployment_id).join(module_name).join(filename)
}


/// Helper that generates urls for output files
pub fn make_output_url(deployment_id: &str, module_name: &str, filename: &str) -> String {
    let scheme = std::env::var("DEFAULT_URL_SCHEME").unwrap_or_else(|_| "http".to_string());
    let host = std::env::var("WASMIOT_SUPERVISOR_IP").unwrap_or_else(|_| "localhost".to_string());
    let port = std::env::var("WASMIOT_SUPERVISOR_PORT").unwrap_or_else(|_| "8080".to_string());
    format!("{scheme}://{host}:{port}/module_results/{}/{}/{}",
        urlencoding::encode(deployment_id),
        urlencoding::encode(module_name),
        urlencoding::encode(filename)
    )
}


/// Helper to save a deployment to deployment folder as json.
pub fn save_deployment_to_disk(deployment: &Deployment) -> Result<(), String> {
    let path = get_deployment_path(&deployment.id);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!("Failed to create deployment directory {}: {}", parent.display(), e)
        })?;
    }

    let file = File::create(&path).map_err(|e| {
        format!("Failed to create deployment file {}: {}", path.display(), e)
    })?;

    serde_json::to_writer_pretty(file, deployment).map_err(|e| {
        format!("Failed to serialize deployment {}: {}", deployment.id, e)
    })?;

    Ok(())
}


/// Constructs and returns the filesystem path to the given module's `.wasm` file.
pub fn get_module_path(deployment_id: &str, module_name: &str) -> PathBuf {
    MODULE_FOLDER.join(deployment_id).join(module_name)
}

/// Constructs the path to a deployment's JSON file.
///
/// Folder structure: instance/deployments/{deployment_id}.json
pub fn get_deployment_path(deployment_id: &str) -> PathBuf {
    DEPLOYMENTS_FOLDER.join(format!("{}.json", deployment_id))
}

/// Constructs the path to a file mounted to a specific module.
pub fn get_params_path(deployment_id: &str, module_name: &str, filename: Option<&str>) -> PathBuf {
    let base = PARAMS_FOLDER.join(deployment_id).join(module_name);
    match filename {
        Some(file) => base.join(file),
        None => base,
    }
}