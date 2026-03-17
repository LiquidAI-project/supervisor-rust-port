use actix_web::{web, HttpResponse, Responder};
use serde_json::{json, Value};
use std::collections::HashMap;
use crate::lib::logging::send_log;
use crate::function_name;
use crate::lib::utils::{get_deployment_path, get_module_path, get_params_path, save_deployment_to_disk};
use crate::lib::wasmtime::{WasmtimeRuntime, ModuleConfig};
use crate::lib::constants::{DEPLOYMENTS, MODULE_FOLDER, PARAMS_FOLDER};
use crate::structs::deployment_supervisor::{
    Deployment,
    Endpoint,
    ModuleEndpointMap,
    ModuleLinkMap,
    ModuleMountMap,
    FunctionLink,
    MountStage,
    MountPathFile,
};
use crate::structs::deployment_orchestrator::{DeploymentDoc as OrchDeploymentDoc, Step as OrchStep};





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

    // Parse orchestrator deployment doc
    let orch_dep: OrchDeploymentDoc = match serde_json::from_value(data.clone()) {
        Ok(d) => d,
        Err(e) => {
            send_log(
                "ERROR",
                &format!("Failed to parse orchestrator deployment doc: {}", e),
                &func_name,
                None,
            )
            .await;
            return HttpResponse::BadRequest().json(json!({
                "error": format!("Invalid deployment document: {}", e)
            }));
        }
    };

    let my_id = orch_dep.my_id.clone();
    if my_id.is_empty() {
        send_log("ERROR", "Missing myId in deployment", &func_name, None).await;
        return HttpResponse::BadRequest().json(json!({ "error": "Missing myId in deployment" }));
    }

    let all_steps = orch_dep.full_manifest.sequence.clone();

    // Filter steps for this supervisor based on myId
    let steps_for_me: Vec<OrchStep> = orch_dep
        .full_manifest
        .sequence
        .into_iter()
        .filter(|s| s.device_id == my_id)
        .collect();

    if steps_for_me.is_empty() {
        send_log(
            "ERROR",
            "No steps in fullManifest.sequence for this device",
            &func_name,
            None,
        )
        .await;
        return HttpResponse::BadRequest().json(json!({
            "error": "No steps in fullManifest.sequence for this device"
        }));
    }

    // Use the deploymentId from first step (all steps share same deploymentId)
    let deployment_id = steps_for_me[0].deployment_id.clone();

    // Collect unique modules used by this device ( HashMap<module.id, (module_name, DeviceModule)> )
    let mut module_map: HashMap<String, (String, crate::structs::deployment_orchestrator::DeviceModule)> = HashMap::new();

    for step in &steps_for_me {
        let m = &step.module;
        module_map
            .entry(m.id.clone())
            .or_insert_with(|| (m.name.clone(), m.clone()));
    }

    if module_map.is_empty() {
        send_log("ERROR", "No modules found in steps for this device", &func_name, None).await;
        return HttpResponse::BadRequest().json(json!({
            "error": "No modules found in steps for this device"
        }));
    }

    // Prepare deployment related directories
    let module_deployment_dir = MODULE_FOLDER.join(&deployment_id);
    let params_deployment_dir = PARAMS_FOLDER.join(&deployment_id);

    if let Err(e) = std::fs::create_dir_all(&module_deployment_dir) {
        send_log(
            "ERROR",
            &format!("Failed to create module directory for deployment: {}", e),
            &func_name,
            None,
        )
        .await;
        return HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to create deployment directories: {}", e)
        }));
    }

    if let Err(e) = std::fs::create_dir_all(&params_deployment_dir) {
        send_log(
            "ERROR",
            &format!("Failed to create params directory for deployment: {}", e),
            &func_name,
            None,
        )
        .await;
        return HttpResponse::InternalServerError().json(json!({
            "error": format!("Failed to create deployment directories: {}", e)
        }));
    }

    // Download all required files and build ModuleConfig
    let mut module_configs = Vec::new();
    let mut errors = Vec::new();

    for (module_id, (module_name, module)) in &module_map {
        let binary_url = module.urls.binary.clone();
        let bin_response = match reqwest::get(&binary_url).await {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                let err = json!({ "error": format!("Binary URL returned {}", resp.status()), "module": module_name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
            Err(e) => {
                let err = json!({ "error": format!("Failed to fetch binary: {}", e), "module": module_name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let bin_bytes = match bin_response.bytes().await {
            Ok(bytes) => bytes,
            Err(e) => {
                let err = json!({ "error": format!("Failed to read binary response: {}", e), "module": module_name });
                send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                errors.push(err);
                continue;
            }
        };

        let binary_path = get_module_path(&deployment_id, module_id);
        if let Err(e) = std::fs::write(&binary_path, &bin_bytes) {
            let err = json!({ "error": format!("Failed to write binary: {}", e), "path": binary_path });
            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
            errors.push(err);
            continue;
        }

        let module_params_path = get_params_path(&deployment_id, module_id, None);
        if let Err(e) = std::fs::create_dir_all(&module_params_path) {
            let err = json!({ "error": format!("Failed to create params directory: {}", e), "module": module_name });
            send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
            errors.push(err);
            continue;
        }

        let mut data_files = HashMap::new();
        for (filename, url) in &module.urls.other {
            match reqwest::get(url).await {
                Ok(resp) if resp.status().is_success() => {
                    match resp.bytes().await {
                        Ok(file_bytes) => {
                            let path = get_params_path(&deployment_id, module_id, Some(filename));
                            if let Some(parent) = path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            match std::fs::write(&path, &file_bytes) {
                                Ok(_) => {
                                    data_files.insert(filename.clone(), path.to_string_lossy().to_string());
                                }
                                Err(e) => {
                                    let err = json!({
                                        "error": format!("Failed to save extra file: {}", e),
                                        "file": filename,
                                        "module": module_name
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
                                "module": module_name
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
                        "module": module_name
                    });
                    send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                    errors.push(err);
                }
                Err(e) => {
                    let err = json!({
                        "error": format!("Failed to fetch extra file: {}", e),
                        "file": filename,
                        "module": module_name
                    });
                    send_log("ERROR", &format!("{:?}", err), &func_name, None).await;
                    errors.push(err);
                }
            }
        }

        let mut config = ModuleConfig {
            id: module_id.clone(),
            name: module_name.clone(),
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

    // Initialize Wasmtime runtimes for each module. Change to create wain runtimes
    let mut runtimes = HashMap::new();
    for config in &module_configs {
        let module_params_dir = get_params_path(&deployment_id, &config.id, None);
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

    // Build global step_links from all steps. This information is used in conjunction with step index to determine next step.
    let mut step_links: Vec<FunctionLink> = Vec::with_capacity(all_steps.len());

    for step in &all_steps {
        let from_ep = Endpoint::from(step.instructions.from.clone());
        let to_ep = step.instructions.to.clone().map(Endpoint::from);
        step_links.push(FunctionLink { from: from_ep, to: to_ep });
    }

    // Build endpoints, instructions and mounts from steps_for_me
    // These are only built for steps of deployment that this supervisor is expected to execute.
    let mut endpoints: ModuleEndpointMap = HashMap::new();
    let mut instructions_map: ModuleLinkMap = HashMap::new();
    let mut mounts_map: ModuleMountMap = HashMap::new();

    for step in &steps_for_me {
        let module_name = step.module.name.clone();
        let function_name = step.function_name.clone();

        // endpoints
        endpoints
            .entry(module_name.clone())
            .or_insert_with(HashMap::new)
            .entry(function_name.clone())
            .or_insert_with(|| Endpoint::from(step.endpoint.clone()));

        // instructions
        let from_ep = Endpoint::from(step.instructions.from.clone());
        let to_ep = step.instructions.to.clone().map(Endpoint::from);

        instructions_map
            .entry(module_name.clone())
            .or_insert_with(HashMap::new)
            .entry(function_name.clone())
            .or_insert_with(Vec::new)
            .push(FunctionLink { from: from_ep.clone(), to: to_ep.clone() });

        // mounts
        let fn_stage_map = mounts_map
            .entry(module_name.clone())
            .or_insert_with(HashMap::new)
            .entry(function_name.clone())
            .or_insert_with(HashMap::new);

        let mut add_stage = |stage: MountStage, orch_mounts: &Vec<crate::structs::deployment_orchestrator::MountPathFile>| {
            let entry = fn_stage_map.entry(stage).or_insert_with(Vec::new);
            for m in orch_mounts {
                entry.push(MountPathFile::new(
                    m.path.clone(),
                    m.media_type.clone(),
                    stage,
                    None,
                    None,
                    None,
                ));
            }
        };

        add_stage(MountStage::DEPLOYMENT, &step.mounts.deployment);
        add_stage(MountStage::EXECUTION, &step.mounts.execution);
        add_stage(MountStage::OUTPUT, &step.mounts.output);
    }

    // Build deployment
    let mut deployment = Deployment::new(
        deployment_id.clone(),
        runtimes,
        module_configs,
        endpoints,
        HashMap::new(),
        HashMap::new(),
    );

    deployment.instructions = instructions_map;
    deployment.mounts = mounts_map;
    deployment.step_links = step_links;

    // Save deployment to disk
    if let Err(e) = save_deployment_to_disk(&deployment) {
        send_log(
            "ERROR",
            &format!("Failed to save deployment {} to disk: {}", deployment_id, e),
            &func_name,
            None,
        )
        .await;

        return HttpResponse::InternalServerError().json(json!({
            "error": "Deployment failed to save to disk",
            "details": e
        }));
    }

    DEPLOYMENTS.lock().insert(deployment_id.clone(), deployment);

    send_log(
        "INFO",
        &format!("Deployment created: {}", deployment_id),
        &func_name,
        None,
    )
    .await;

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