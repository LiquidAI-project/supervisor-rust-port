use actix_web::{web, HttpResponse, Responder};
use serde_json::{json, Value};
use std::collections::HashMap;
use crate::lib::logging::send_log;
use crate::function_name;
use crate::lib::utils::{get_deployment_path, get_module_path, get_params_path, save_deployment_to_disk};
use crate::lib::wasmtime::{WasmtimeRuntime, ModuleConfig};
use crate::lib::constants::{DEPLOYMENTS, MODULE_FOLDER, PARAMS_FOLDER};
use crate::structs::deployment_supervisor::{Deployment, Endpoint, ModuleEndpointMap};




/// Creates a new WebAssembly deployment with modules and optional data files.
///
/// Expects a JSON payload with fields:
/// - `deploymentId` (string)
/// - `modules` (list of modules, each with `id`, `name`, and `urls`)
/// - Optional: `endpoints`, `instructions`, `mounts`
///
/// Downloads all binaries and additional data files, sets up execution environments,
/// and stores the deployment in memory.
///
/// Returns:
/// - 200 OK if deployment succeeds
/// - 400/500 with JSON error otherwise
pub async fn deployment_create(payload: web::Json<Value>) -> impl Responder {
    let func_name = function_name!().to_string();
    send_log("INFO", "Deployment creation request received", &func_name, None).await;

    let data = payload.into_inner();

    let deployment_id = match data["deploymentId"].as_str() {
        Some(s) => s.to_string(),
        None => {
            send_log("ERROR", "Missing deploymentId", &func_name, None).await;
            return HttpResponse::BadRequest().json(json!({ "error": "Missing deploymentId" }));
        }
    };

    let modules = match data["modules"].as_array() {
        Some(arr) if !arr.is_empty() => arr,
        _ => {
            send_log("ERROR", "No modules provided", &func_name, None).await;
            return HttpResponse::BadRequest().json(json!({ "error": "No modules provided in deployment request" }));
        }
    };

    let mut module_configs = Vec::new();
    let mut errors = Vec::new();

    let module_deployment_dir = MODULE_FOLDER.join(&deployment_id);
    let params_deployment_dir = PARAMS_FOLDER.join(&deployment_id);
    
    if let Err(e) = std::fs::create_dir_all(&module_deployment_dir) {
        send_log("ERROR", &format!("Failed to create module directory for deployment: {}", e), &func_name, None).await;
        return HttpResponse::InternalServerError().json(json!({ "error": format!("Failed to create deployment directories: {}", e) }));
    }
    
    if let Err(e) = std::fs::create_dir_all(&params_deployment_dir) {
        send_log("ERROR", &format!("Failed to create params directory for deployment: {}", e), &func_name, None).await;
        return HttpResponse::InternalServerError().json(json!({ "error": format!("Failed to create deployment directories: {}", e) }));
    }

    for module in modules {
        let id = module.get("id").and_then(Value::as_str).unwrap_or("unknown").to_string();
        let name = match module.get("name").and_then(Value::as_str) {
            Some(n) => n.to_string(),
            None => {
                let err = json!({ "error": "Module missing name", "module": module });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        // Fetch binary
        let binary_url = match module.get("urls").and_then(|urls| urls.get("binary")).and_then(Value::as_str) {
            Some(url) => url.to_string(),
            None => {
                let err = json!({ "error": "Module missing binary URL", "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let bin_response = match reqwest::get(&binary_url).await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                let err = json!({ "error": format!("Binary URL returned {}", resp.status()), "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
            Err(e) => {
                let err = json!({ "error": format!("Failed to fetch binary: {}", e), "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let bin_bytes = match bin_response.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                let err = json!({ "error": format!("Failed to read binary response: {}", e), "module": name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let binary_path = get_module_path(&deployment_id, &name);
        if let Err(e) = std::fs::write(&binary_path, &bin_bytes) {
            let err = json!({ "error": format!("Failed to write binary: {}", e), "path": binary_path });
            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
            errors.push(err);
            continue;
        }

        let module_params_path = get_params_path(&deployment_id, &name, None);
        if let Err(e) = std::fs::create_dir_all(&module_params_path) {
            let err = json!({ "error": format!("Failed to create params directory: {}", e), "module": name });
            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
            errors.push(err);
            continue;
        }

        let mut data_files = HashMap::new();
        if let Some(other_map) = module.get("urls")
            .and_then(|urls| urls.get("other"))
            .and_then(Value::as_object)
        {
            for (filename, url_val) in other_map {
                if let Some(url) = url_val.as_str() {
                    match reqwest::get(url).await {
                        Ok(resp) if resp.status().is_success() => {
                            match resp.bytes().await {
                                Ok(file_bytes) => {
                                    let path = get_params_path(&deployment_id, &name, Some(filename));
                                    if let Some(parent) = path.parent() {
                                        std::fs::create_dir_all(parent).ok();
                                    }
                                    match std::fs::write(&path, &file_bytes) {
                                        Ok(_) => {
                                            data_files.insert(filename.clone(), path.to_string_lossy().to_string());
                                        }
                                        Err(e) => {
                                            let err = json!({
                                                "error": format!("Failed to save extra file: {}", e),
                                                "file": filename,
                                                "module": name
                                            });
                                            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                                            errors.push(err);
                                        }
                                    }
                                }
                                Err(e) => {
                                    let err = json!({
                                        "error": format!("Failed to read extra file bytes: {}", e),
                                        "file": filename,
                                        "module": name
                                    });
                                    send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                                    errors.push(err);
                                }
                            }
                        }
                        Ok(resp) => {
                            let err = json!({
                                "error": format!("Non-200 response for extra file: {}", resp.status()),
                                "file": filename,
                                "module": name
                            });
                            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                            errors.push(err);
                        }
                        Err(e) => {
                            let err = json!({
                                "error": format!("Failed to fetch extra file: {}", e),
                                "file": filename,
                                "module": name
                            });
                            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                            errors.push(err);
                        }
                    }
                }
            }
        }

        // Construct module config
        let mut config = ModuleConfig {
            id,
            name: name.clone(),
            path: binary_path,
            data_files,
            ml_model: None,
            data_ptr_function_name: "get_image_ptr".to_string(),
        };
        config.set_model_from_data_files(None);

        module_configs.push(config);
    }

    if !errors.is_empty() {
        return HttpResponse::InternalServerError().json(json!({
            "error": "One or more modules failed to load",
            "details": errors
        }));
    }

    // Initialize Wasmtime runtimes for each module with their param folders mounted
    let mut runtimes = HashMap::new();
    for config in &module_configs {
        let module_params_dir = get_params_path(&deployment_id, &config.name, None);
        match WasmtimeRuntime::new(vec![
            (module_params_dir.to_string_lossy().to_string(), ".".to_string())
        ]).await {
            Ok(runtime) => {
                runtimes.insert(config.name.clone(), runtime);
            }
            Err(e) => {
                return HttpResponse::InternalServerError().json(json!({
                    "error": format!("Failed to initialize runtime: {}", e),
                    "module": config.name
                }));
            }
        }
    }

    // Convert endpoints (nested map) to expected type
    let endpoints: ModuleEndpointMap = match data.get("endpoints") {
        Some(Value::Object(mod_map)) => {
            let mut result = HashMap::new();
            for (mod_name, fn_map_val) in mod_map {
                if let Some(fn_map) = fn_map_val.as_object() {
                    let mut inner = HashMap::new();
                    for (fn_name, endpoint_val) in fn_map {
                        match serde_json::from_value::<Endpoint>(endpoint_val.clone()) {
                            Ok(endpoint) => {
                                inner.insert(fn_name.clone(), endpoint);
                            }
                            Err(e) => {
                                return HttpResponse::BadRequest().json(json!({
                                    "error": format!("Invalid endpoint for '{}::{}': {}", mod_name, fn_name, e)
                                }));
                            }
                        }
                    }
                    result.insert(mod_name.clone(), inner);
                }
            }
            result
        }
        _ => HashMap::new(),
    };

    // Convert instructions and mounts from Map<String, Value> → HashMap<String, Value>
    let instructions = data.get("instructions")
        .and_then(|v| v.as_object())
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_else(HashMap::new);

    let mounts = data.get("mounts")
        .and_then(|v| v.as_object())
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_else(HashMap::new);

    let deployment = Deployment::new(
        deployment_id.clone(),
        runtimes,
        module_configs,
        endpoints,
        instructions,
        mounts,
    );

    // Save deployment to disk as JSON
    if let Err(e) = save_deployment_to_disk(&deployment) {
        send_log(
            "ERROR",
            &format!("Failed to save deployment {} to disk: {}", deployment_id, e),
            &func_name,
            None
        ).await;

        return HttpResponse::InternalServerError().json(json!({
            "error": "Deployment failed to save to disk",
            "details": e
        }));
    }

    DEPLOYMENTS.lock().insert(deployment_id.clone(), deployment);

    send_log("INFO", &format!("Deployment created: {}", deployment_id), &func_name, None).await;

    HttpResponse::Ok().json(json!({
        "status": "success",
        "deploymentId": deployment_id
    }))
}


pub async fn deployment_get() -> impl Responder {
    let deps = DEPLOYMENTS.lock();
    let d: Vec<&Deployment> = deps.iter().map(|(_id, deployment)| deployment).collect();
    HttpResponse::Ok().json(json!({
        "deployments": d
    }))
}


/// Deletes (removes) an active deployment from memory by its ID.
///
/// This endpoint is typically used when a pipeline or WebAssembly workload
/// is no longer needed and should be unloaded.
///
/// # Path Parameters
/// - `deployment_id`: ID of the deployment to delete (string)
///
/// # Behavior
/// - If the deployment exists in memory, it is removed and a success message is returned.
/// - If not found, returns a 404 with an error message.
///
/// # Example
/// DELETE /deploy/my-deployment-id
pub async fn deployment_delete(path: web::Path<String>) -> impl Responder {
    let deployment_id = path.into_inner();
    let func_name = function_name!().to_string();

    let log_msg = format!("Delete request for deployment: {}", deployment_id);
    tokio::spawn(async move {
        send_log("INFO", &log_msg, &func_name, None).await;
    });

    let mut deps = DEPLOYMENTS.lock();

    if deps.remove(&deployment_id).is_some() {

        // Delete deployment JSON file
        let json_path = get_deployment_path(&deployment_id);
        if let Err(e) = std::fs::remove_file(&json_path) {
            let func_name = function_name!().to_string();
            tokio::spawn(async move {
                send_log(
                    "WARN",
                    &format!("Failed to delete deployment JSON saved on disk {}: {}", json_path.display(), e),
                    &func_name,
                    None
                ).await;
            });
        } else {
            let func_name = function_name!().to_string();
            tokio::spawn(async move {
                send_log(
                    "DEBUG",
                    &format!("Deleted deployment JSON file: {}", json_path.display()),
                    &func_name,
                    None
                ).await;
            });
        }

        // Delete the module and params folders related to this deployment
        let module_deployment_path = MODULE_FOLDER.join(&deployment_id);
        let params_deployment_path = PARAMS_FOLDER.join(&deployment_id);
        if module_deployment_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&module_deployment_path) {
                let func_name = function_name!().to_string();
                tokio::spawn(async move {
                    send_log(
                        "WARN",
                        &format!("Failed to delete module deployment folder {}: {}", module_deployment_path.display(), e),
                        &func_name,
                        None
                    ).await;
                });
            } else {
                let func_name = function_name!().to_string();
                tokio::spawn(async move {
                    send_log(
                        "DEBUG",
                        &format!("Deleted module deployment folder: {}", module_deployment_path.display()),
                        &func_name,
                        None
                    ).await;
                });
            }
        }
        if params_deployment_path.exists() {
            if let Err(e) = std::fs::remove_dir_all(&params_deployment_path) {
                let func_name = function_name!().to_string();
                tokio::spawn(async move {
                    send_log(
                        "WARN",
                        &format!("Failed to delete params deployment folder {}: {}", params_deployment_path.display(), e),
                        &func_name,
                        None
                    ).await;
                });
            } else {
                let func_name = function_name!().to_string();
                tokio::spawn(async move {
                    send_log(
                        "DEBUG",
                        &format!("Deleted params deployment folder: {}", params_deployment_path.display()),
                        &func_name,
                        None
                    ).await;
                });
            }
        }

        let func_name = function_name!().to_string();
        let did = deployment_id.clone();
        tokio::spawn(async move {
            send_log(
                "INFO",
                &format!("Successfully deleted deployment '{}' and all associated files", did),
                &func_name,
                None
            ).await;
        });

        HttpResponse::Ok().json(json!({ 
            "status": "success",
            "message": format!("Deployment '{}' and all associated files deleted", deployment_id)
        }))
    } else {
        HttpResponse::NotFound().json(json!({
            "error": "Deployment does not exist",
            "deployment_id": deployment_id
        }))
    }
}