
use serde::{Deserialize, Serialize};
use tokio::task;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_files::NamedFile;
use serde_json::{json, Value};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::Ordering;
use log::{debug, error, warn};
use sanitize_filename;
use futures_util::StreamExt;
use std::fs::File;
use std::{env, io, time::Duration};
use std::io::Write;
use crate::lib::constants::{DEPLOYMENTS, IDLE, INPUT, INTERUPTION, MAX_DEPLOYMENT_STEPS, REQUEST_HISTORY, SNAPSHOT_BYTES, SNAPSHOT_CHAIN_CONTEXT, SNAPSHOT_NOTIFY, get_snapshot_timeout};
use crate::lib::import::DefaultImporter;
use crate::lib::interuption::interuption_impl::Implementer;
use crate::lib::logging::send_log;
use crate::lib::runtime::{Runtime, RuntimeSerialisable, Snapshot};
use crate::{function_name, lib};
use crate::lib::utils::{get_params_path, make_output_url};
use crate::structs::request_entry::RequestEntry;
use crate::structs::deployment_supervisor::{CallData, Endpoint, EndpointArgs, EndpointData, MountStage};
use std::fs;
use crate::lib::utils::{unwrap};

/// Executes a function in a given module in a given deployment.
///
/// If a filename is provided, this acts as a file-serving route.
/// Otherwise, this will:
/// - Save incoming multipart files (if any)
/// - Construct a `RequestEntry`
/// - Either push to the async queue (POST) or execute immediately (GET)
/// - Return a link to the result in request history
pub async fn run_module_function(
    path: web::Path<(String, String, String, Option<String>)>,
    req: HttpRequest,
    payload: web::Payload,
) -> impl Responder {

    // Warn if the header was missing
    if !req.headers().contains_key("X-Chain-Step") {
        warn!("Missing X-Chain-Step header, defaulting to step 0");
    }

    // Read incoming step header "X-Chain-Step" (default to zero if not present).
    // Header represents which step of execution this supervisor is expected to execute.
    // No header defaults to zero, which means first step is executed.
    let step_index: usize = req
        .headers()
        .get("X-Chain-Step")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    debug!("X-Chain-Step => {}", step_index);

    // Stop execution if step index exceeds max allowed, to prevent possible infinite chains
    if step_index > MAX_DEPLOYMENT_STEPS {
        warn!("X-Chain-Step exceeds {}, stopping execution", MAX_DEPLOYMENT_STEPS);
        return HttpResponse::BadRequest().json(json!({
            "error": format!("X-Chain-Step exceeds {}", MAX_DEPLOYMENT_STEPS)
        }));
    }

    let (deployment_id, module_name, function_name, maybe_filename) = path.into_inner();

    // Serve static file if filename is provided
    if let Some(filename) = maybe_filename {
        let file_path = get_params_path(&deployment_id, &module_name, Some(&filename));
        let log_msg = format!(
            "Serving file: {}/{}/{}/{}",
            deployment_id.clone(),
            module_name.clone(),
            function_name.clone(),
            filename.clone()
        );
        let func_name = function_name!().to_string();
        tokio::spawn(async move {
            send_log(
                "INFO",
                &log_msg,
                &func_name,
                None
            ).await;
        });
        return match NamedFile::open(&file_path) {
            Ok(file) => file.into_response(&req),
            Err(_) => HttpResponse::NotFound().json(json!({
                "error": "File not found",
                "deployment_id": deployment_id,
                "module": module_name,
                "filename": filename,
            })),
        };
    }
    
    // Check if deployment and module exist
    let deployments_map = DEPLOYMENTS.lock();
    let deployment = match deployments_map.get(&deployment_id) {
        Some(dep) => dep,
        None => {
            return HttpResponse::NotFound().json(json!({
                "error": "Deployment not found",
                "deployment_id": deployment_id
            }));
        }
    };

    if !deployment.modules.contains_key(&module_name) {
        return HttpResponse::NotFound().json(json!({
            "error": "Module not found in deployment",
            "deployment_id": deployment_id,
            "module_name": module_name
        }));
    }
    drop(deployments_map); // Free the lock early

    // Reject concurrent executions — only one wasm module runs at a time
    if !IDLE.load(Ordering::Relaxed) {
        return HttpResponse::Conflict().json(json!({"error": "Machine is busy"}));
    }

    // Parse query parameters into JSON
    let query_str = req.uri().query().unwrap_or("");
    let query_map: HashMap<String, String> =
        serde_urlencoded::from_str(query_str).unwrap_or_default();
    let request_args = json!(query_map);
    
    // Handle multipart file uploads (for POST only)
    let mut request_files: HashMap<String, String> = HashMap::new();

    let is_post = req.method() == "POST";
    if is_post {
        let mut multipart = Multipart::new(&req.headers(), payload);
        while let Some(Ok(mut field)) = multipart.next().await {
            let content_disposition = field.content_disposition();
            let param_name = content_disposition.get_name().unwrap_or("file").to_string();
            let filename = content_disposition
                .get_filename()
                .map(sanitize_filename::sanitize)
                .unwrap_or_else(|| format!("{}_input.dat", param_name));

            let save_path = get_params_path(&deployment_id, &module_name, Some(&filename));
            if let Some(parent) = save_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            let mut f = match File::create(&save_path) {
                Ok(f) => f,
                Err(e) => {
                    return HttpResponse::InternalServerError().json(json!({
                        "error": format!("Failed to save file: {}", e)
                    }));
                }
            };

            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                if let Err(e) = f.write_all(&data) {
                    return HttpResponse::InternalServerError().json(json!({
                        "error": format!("File write error: {}", e)
                    }));
                }
            }

            request_files.insert(param_name, save_path.to_string_lossy().to_string());
        }
    }
    
    // Create RequestEntry
    let entry = RequestEntry::new(
        deployment_id.clone(),
        module_name.clone(),
        function_name.clone(),
        req.method().to_string(),
        request_args,
        request_files,
        Utc::now(),
        step_index
    );

    let log_msg = format!(
        "Executing module function: {}/{}/{}",
        deployment_id.clone(),
        module_name.clone(),
        function_name.clone()
    );
    let func_name = function_name!().to_string();
    let entry_clone = entry.clone();
    tokio::spawn(async move {
        send_log(
            "INFO",
            &log_msg,
            &func_name,
            Some(&entry_clone)
        ).await;
    });

    let http_scheme = env::var("DEFAULT_URL_SCHEME").unwrap_or_else(|_| {
        error!("Failed to read DEFAULT_URL_SCHEME from enviroment variables, defaulting to 'http'.");
        "http".to_string()
    });
    let host = env::var("WASMIOT_SUPERVISOR_IP").unwrap_or_else(|_| {
        error!("Failed to read WASMIOT_SUPERVISOR_IP from enviroment variables, defaulting to 'localhost'.");
        "localhost".to_string()
    });
    let port = env::var("WASMIOT_SUPERVISOR_PORT").unwrap_or_else(|_| {
        error!("Failed to read WASMIOT_SUPERVISOR_PORT from enviroment variables, defaulting to '8080'.");
        "8080".to_string()
    });
    let result_url = format!("{}://{}:{}/request-history/{}", http_scheme, host, port, entry.request_id);
    IDLE.store(false, Ordering::Relaxed);
    tokio::spawn(async move { make_history(entry).await; });
    HttpResponse::Ok().json(json!({ "status": "started", "resultUrl": result_url }))
}


/// Handler for running a module function
///
/// This is here to match a path that has only 3 parameters vs the default 4 parameters
pub async fn run_module_function_3(
    path: web::Path<(String, String, String)>,
    req: HttpRequest,
    payload: web::Payload,
) -> impl Responder {
    let (deployment_id, module_name, function_name) = path.into_inner();
    let new_path = web::Path::from((deployment_id, module_name, function_name, None));
    run_module_function(new_path, req, payload).await
}



/// Executes a WebAssembly function call and records its result in history.
///
/// This function performs the full execution lifecycle of a `RequestEntry`, including:
/// - Calling the Wasm function via `do_wasm_work()`
/// - Setting the result and success state
/// - Logging the outcome (both to stdout and external log sink)
/// - Appending the result to global `REQUEST_HISTORY`
///
/// This is the main entry point for any completed function execution (GET or POST).
///
/// # Arguments
/// - `entry`: The request entry to execute
///
/// # Returns
/// - The updated `RequestEntry` with result and success set
/// - An optional `Value` containing the final result from the execution
pub async fn make_history(mut entry: RequestEntry) -> (RequestEntry, Option<Value>) {
    let mut final_opt: Option<Value> = None;

    match do_wasm_work(&mut entry).await {
        Ok(final_json) => {
            entry.success = true;
            final_opt = Some(final_json);
        }
        Err(err) => {
            entry.result = Some(Value::String(err.clone()));
            entry.success = false;
            log::error!("Error during Wasm execution: {}", err);
            let func_name = function_name!().to_string();
            let entry_clone = entry.clone();
            task::spawn(async move {
                send_log(
                    "ERROR",
                    &format!("Error during Wasm execution: {}", err),
                    &func_name,
                    Some(&entry_clone)
                ).await;
            });
        }
    }

    REQUEST_HISTORY.lock().push(entry.clone());
    (entry, final_opt)
}



/// Outcome of synchronous wasm execution, passed back to the async do_wasm_work wrapper.
struct WasmSyncResult {
    /// True if wasm was interrupted and a snapshot was stored; other fields are meaningless.
    snapshotted: bool,
    raw_output: Value,
    /// Files already opened for the chain call, keyed by name. Empty if no chain follows.
    chain_files: HashMap<String, std::fs::File>,
    next_call: Option<CallData>,
}

/// Synchronous wasm execution core. Runs on a dedicated blocking thread via spawn_blocking
/// so the tokio worker is not occupied for the duration of wasm interpretation.
///
/// Takes ownership of entry, mutates result/outputs fields, and returns it together with
/// the execution outcome. The deployment lock is acquired and released entirely here so
/// it never crosses the await boundary in do_wasm_work.
fn run_wasm_sync(mut entry: RequestEntry) -> Result<(RequestEntry, WasmSyncResult), String> {
    let mut deployments = DEPLOYMENTS.lock();
    let deployment = deployments.get_mut(&entry.deployment_id)
        .ok_or_else(|| format!("Deployment '{}' not found", entry.deployment_id))?;

    // Resolve and save chain context while the deployment lock is held.
    // Included in the /snapshot response so a resuming machine knows which endpoint is next.
    let next_ep_opt = deployment
        .next_target_with_index(&entry.module_name, &entry.function_name, entry.step_index)
        .cloned();
    let cur_response_opt = deployment.endpoints
        .get(&entry.module_name)
        .and_then(|m| m.get(&entry.function_name))
        .map(|ep| ep.response.clone());
    *SNAPSHOT_CHAIN_CONTEXT.lock() = json!({
        "step_index": entry.step_index,
        "next_endpoint": serde_json::to_value(&next_ep_opt).unwrap_or(Value::Null),
        "current_response": serde_json::to_value(&cur_response_opt).unwrap_or(Value::Null),
    });

    let config = deployment.modules.get(&entry.module_name).unwrap();
    let bin = fs::read(&config.path).unwrap();
    let ast = unwrap("", lib::wain_syntax_binary::parse(&bin));
    let stdout = io::stdout();
    let output_buf = Arc::new(Mutex::new(Vec::<u8>::new()));
    let importer = DefaultImporter::with_stdio(io::stdin(), stdout.lock(), Arc::clone(&output_buf));
    let interuption_clone = Arc::clone(&INTERUPTION);
    let snapshot_bytes_ref = Arc::clone(&SNAPSHOT_BYTES);
    let interuption_implementer = Arc::new(Implementer::new(interuption_clone, snapshot_bytes_ref));
    let mut runtime = unwrap("", Runtime::instantiate(&ast.module, importer, interuption_implementer));
    // Clear any leftover interrupt from a previous execution before starting
    INTERUPTION.store(false, Ordering::Relaxed);
    let _ = runtime.invoke(&entry.function_name, &[]);
    drop(runtime);
    IDLE.store(true, Ordering::Relaxed);

    // If the wasm was interrupted and a snapshot was taken, return early.
    // Chain context was already saved above and will be returned by /snapshot.
    if !SNAPSHOT_BYTES.lock().unwrap().is_empty() {
        return Ok((entry, WasmSyncResult {
            snapshotted: true,
            raw_output: Value::Null,
            chain_files: HashMap::new(),
            next_call: None,
        }));
    }

    // Capture wasm stdout output
    let raw_output = {
        let captured = output_buf.lock().unwrap();
        if captured.is_empty() { Value::Null } else {
            Value::String(String::from_utf8_lossy(&captured).into_owned())
        }
    };

    // Parse result according to the endpoint's declared media type
    let endpoint = &deployment.endpoints[&entry.module_name][&entry.function_name];
    let output_mounts = deployment
        .mounts
        .get(&entry.module_name)
        .and_then(|m| m.get(&entry.function_name))
        .and_then(|sm| sm.get(&MountStage::OUTPUT))
        .cloned()
        .unwrap_or_default();
    let parsed = deployment.parse_endpoint_result(raw_output.clone(), &endpoint.response, &output_mounts);

    // Update entry output URLs
    if let Some(EndpointData::StrList(filenames)) = &parsed.1 {
        entry.outputs = filenames.iter()
            .map(|f| make_output_url(&entry.deployment_id, &entry.module_name, f))
            .collect();
    }

    // Update entry result
    entry.result = parsed.0.clone().map(|arg| match arg {
        EndpointArgs::Str(s) => Value::String(s),
        EndpointArgs::StrList(vs) => Value::Array(vs.into_iter().map(Value::String).collect()),
        EndpointArgs::Dict(map) => Value::Object(map.into_iter().collect()),
    });

    // Determine next chain step
    let step_index = entry.step_index;
    let next_call = deployment
        .next_target_with_index(&entry.module_name, &entry.function_name, step_index)
        .map(|next_ep| CallData::from_endpoint(next_ep, parsed.0.clone(), parsed.1.clone()));

    // Open chain-call files while the deployment lock is still held
    let mut chain_files = HashMap::new();
    if let Some(ref call_data) = next_call {
        let current_module_cfg = deployment.modules
            .get(&entry.module_name)
            .ok_or_else(|| format!("Module config not found for '{}'", entry.module_name))?;
        let current_params_dir = get_params_path(&entry.deployment_id, &current_module_cfg.id, None);
        match &call_data.files {
            EndpointData::StrList(file_names) => {
                for name in file_names {
                    let full_path = current_params_dir.join(name);
                    let file = std::fs::File::open(&full_path)
                        .map_err(|e| format!(
                            "Failed to open file for subcall ({}): {}",
                            full_path.display(), e
                        ))?;
                    chain_files.insert(name.clone(), file);
                }
            }
        }
    }

    drop(deployments);

    Ok((entry, WasmSyncResult { snapshotted: false, raw_output, chain_files, next_call }))
}

/// Executes the WebAssembly function for the given request and performs any chained subcalls.
///
/// Runs the blocking wasm interpreter on a dedicated thread via spawn_blocking, then handles
/// async chain calls and logging on the caller's tokio task.
pub async fn do_wasm_work(entry: &mut RequestEntry) -> Result<Value, String> {
    let func_name = function_name!().to_string();
    let module_name_clone = entry.module_name.clone();
    let entry_clone = entry.clone();
    task::spawn(async move {
        send_log("DEBUG", &format!("Preparing Wasm module '{}'", &module_name_clone), &func_name, Some(&entry_clone)).await;
    });

    let func_name = function_name!().to_string();
    let entry_function_name = entry.function_name.clone();
    let entry_clone = entry.clone();
    task::spawn(async move {
        send_log("DEBUG", &format!("Running Wasm function '{}'", &entry_function_name), &func_name, Some(&entry_clone)).await;
    });

    let entry_for_sync = entry.clone();
    let (returned_entry, sync_result) = tokio::task::spawn_blocking(move || {
        run_wasm_sync(entry_for_sync)
    })
    .await
    .map_err(|e| format!("Wasm execution task panicked: {}", e))??;

    // Apply mutations from the sync execution back to the caller's entry
    let step_index = returned_entry.step_index;
    entry.result = returned_entry.result;
    entry.outputs = returned_entry.outputs;

    if sync_result.snapshotted {
        return Ok(json!({ "snapshotted": true }));
    }

    let func_name = function_name!().to_string();
    let raw_output_clone = sync_result.raw_output.clone();
    let entry_clone = entry.clone();
    tokio::spawn(async move {
        send_log("DEBUG", &format!("... Result: {}", raw_output_clone), &func_name, Some(&entry_clone)).await;
    });

    if let Some(val) = &entry.result {
        let func_name = function_name!().to_string();
        let entry_clone = entry.clone();
        let val_clone = val.clone();
        task::spawn(async move {
            send_log("DEBUG", &format!("Execution result (parsed): {:?}", &val_clone), &func_name, Some(&entry_clone)).await;
        });
    }

    if !entry.outputs.is_empty() {
        let func_name = function_name!().to_string();
        let entry_clone = entry.clone();
        let urls_clone = entry.outputs.clone();
        task::spawn(async move {
            send_log("DEBUG", &format!("Result URLs: {:?}", &urls_clone), &func_name, Some(&entry_clone)).await;
        });
    }

    log::info!("Step index {}", step_index);
    log::info!("Next call: {:?}", sync_result.next_call);

    if let Some(call_data) = sync_result.next_call {
        let next_idx = step_index.saturating_add(1);

        let mut headers = reqwest::header::HeaderMap::new();
        for (k, v) in &call_data.headers {
            if let (Ok(key), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                headers.insert(key, val);
            }
        }
        headers.insert(
            reqwest::header::HeaderName::from_static("x-chain-step"),
            reqwest::header::HeaderValue::from_str(&next_idx.to_string()).unwrap(),
        );

        let module_name_clone = entry.module_name.clone();
        let call_data_url_clone = call_data.url.clone();
        let func_name = function_name!().to_string();
        let entry_clone = entry.clone();
        task::spawn(async move {
            send_log(
                "DEBUG",
                &format!("Making sub-call (X-Chain-Step={}) from '{}' to '{}'", next_idx, &module_name_clone, &call_data_url_clone),
                &func_name,
                Some(&entry_clone),
            ).await;
        });

        // Build multipart form from pre-opened files
        let mut form = reqwest::multipart::Form::new();
        for (name, mut file) in sync_result.chain_files {
            let mut buf = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buf).map_err(|e| format!("Failed to read file for multipart: {}", e))?;
            form = form.part(name.clone(), reqwest::multipart::Part::bytes(buf).file_name(name));
        }

        let client = reqwest::Client::new();
        let response = client
            .request(
                call_data.method.to_uppercase().parse().unwrap_or(reqwest::Method::POST),
                &call_data.url,
            )
            .headers(headers)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("Failed to send chained request: {}", e))?;

        let chained_json: Value = response
            .json()
            .await
            .map_err(|e| format!("Invalid response JSON from {}: {}", call_data.url, e))?;

        if let Some(res_val) = chained_json.get("result") {
            let final_val = match res_val {
                Value::Object(map) if map.contains_key("result") => {
                    map.get("result").cloned().unwrap_or(res_val.clone())
                }
                _ => res_val.clone(),
            };
            entry.success = true;
            return Ok(final_val);
        }

        if let Some(url) = chained_json.get("resultUrl").and_then(|v| v.as_str()) {
            let fetched_json: Value = client
                .get(url)
                .send()
                .await
                .map_err(|e| format!("Failed to fetch resultUrl {}: {}", url, e))?
                .json()
                .await
                .map_err(|e| format!("Invalid JSON from resultUrl {}: {}", url, e))?;

            let final_json = fetched_json.get("result").cloned().unwrap_or(fetched_json);
            entry.success = true;
            return Ok(final_json);
        }

        entry.success = true;
        return Ok(chained_json);
    }

    Ok(json!({ "result": entry.result }))
}

/// Combined atomic snapshot endpoint: sets the interrupt flag and blocks until the wasm
/// interpreter has stored a snapshot, then returns the bytes and chain context directly.
/// Eliminates the race condition of the separate /interupt + /getSnapshot two-step flow.
/// Timeout is configurable via WASMIOT_SNAPSHOT_TIMEOUT_SECONDS (default 30 s).
pub async fn snapshot() -> impl Responder {
    // Register the listener BEFORE setting the flag so a very fast snapshot is never missed
    let notified = SNAPSHOT_NOTIFY.notified();
    INTERUPTION.store(true, Ordering::Relaxed);

    match tokio::time::timeout(Duration::from_secs(get_snapshot_timeout()), notified).await {
        Ok(_) => {
            let bytes = std::mem::take(&mut *SNAPSHOT_BYTES.lock().unwrap());
            let chain_context = SNAPSHOT_CHAIN_CONTEXT.lock().clone();
            INTERUPTION.store(false, Ordering::Relaxed);
            HttpResponse::Ok().json(json!({
                "status": "success",
                "message": bytes,
                "chain_context": chain_context
            }))
        }
        Err(_) => {
            INTERUPTION.store(false, Ordering::Relaxed);
            HttpResponse::GatewayTimeout().json(json!({
                "error": "No snapshot received within timeout"
            }))
        }
    }
}

/// Deprecated: use GET /snapshot instead (atomic interrupt + fetch in one request).
/// Sets the interrupt flag; the wasm interpreter will store a snapshot in SNAPSHOT_BYTES.
pub async fn interupt() -> impl Responder {
    INTERUPTION.store(true, Ordering::Relaxed);
    HttpResponse::Ok().json(json!({
        "status": "success",
        "message": "Wain succesfully interupted"
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resume {
    pub status: String,
    pub message: Vec<u8>
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Input {
    pub key: String
}


/// This function is used to resume execution of a WebAssembly module from snapshot.
/// TODO: Wain must be run in a new thread, otherwise response will be block until
/// the execution of WebAssembly module is complete. Also maybe some mechanism to
/// check if Wain is already interpreting some module?
/// TODO: Response under development
pub async fn resume(payload: web::Json<Value>) -> impl Responder {
    // Reject concurrent executions — only one wasm module runs at a time
    if !IDLE.load(Ordering::Relaxed) {
        return HttpResponse::Conflict().json(json!({"error": "Machine is busy"}));
    }
    // Clear any leftover interrupt from a previous execution before starting
    INTERUPTION.store(false, Ordering::Relaxed);

    let data = payload.into_inner();
    let snapshot: Resume = serde_json::from_value(data).unwrap();

    let binding = snapshot.message;
    let interuption_clone = Arc::clone(&INTERUPTION);
    let snapshot_bytes_ref = Arc::clone(&SNAPSHOT_BYTES);
    let interuption_implementer = Arc::new(Implementer::new(interuption_clone, snapshot_bytes_ref));

    let output_buf = Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
    //let mut guarded = INPUT.lock().unwrap();
    //*guarded = String::new();
    //drop(guarded);
    IDLE.store(false, Ordering::Relaxed);

    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        let runtime_serialisable: RuntimeSerialisable = rmp_serde::from_slice(&binding).unwrap();
        let _ = runtime_serialisable.resume_execution(interuption_implementer, Arc::clone(&output_buf));
        IDLE.store(true, Ordering::Relaxed);

        // resume_execution restores SNAPSHOT_CHAIN_CONTEXT from the snapshot, read it here
        let chain_context = SNAPSHOT_CHAIN_CONTEXT.lock().clone();

        // Capture whatever the wasm wrote to stdout during resumed execution
        let raw_output = {
            let captured = output_buf.lock().unwrap();
            if captured.is_empty() {
                Value::Null
            } else {
                Value::String(String::from_utf8_lossy(&captured).into_owned())
            }
        };

        // Continue the chain using the context that was saved at snapshot time
        let step_index = chain_context.get("step_index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let next_ep_val = chain_context.get("next_endpoint").cloned().unwrap_or(Value::Null);

        handle.block_on(async move {
            if !next_ep_val.is_null() {
                if let Ok(next_endpoint) = serde_json::from_value::<Endpoint>(next_ep_val) {
                    let url = format!("{}{}", next_endpoint.url.trim_end_matches('/'), next_endpoint.path);
                    let next_idx = step_index.saturating_add(1);

                    let mut headers = reqwest::header::HeaderMap::new();
                    headers.insert(
                        reqwest::header::HeaderName::from_static("x-chain-step"),
                        reqwest::header::HeaderValue::from_str(&next_idx.to_string()).unwrap(),
                    );

                    // Pass scalar wasm output as a query parameter if present
                    let final_url = match raw_output.as_str() {
                        Some(s) if !s.is_empty() => {
                            let param_name = next_endpoint.request.parameters.first()
                                .and_then(|p| p.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("result");
                            format!("{}?{}={}", url, param_name, s)
                        }
                        _ => url,
                    };

                    let client = reqwest::Client::new();
                    let _ = client
                        .request(
                            next_endpoint.method.to_uppercase().parse().unwrap_or(reqwest::Method::POST),
                            &final_url,
                        )
                        .headers(headers)
                        .send()
                        .await;
                }
            }
        });
    });

    HttpResponse::Ok().json(json!({
        "status": "started"
    }))
}

/// This function is used to provide input from demo web GUI to WebAssembly module running in the supervisor
pub async fn input(payload: web::Json<Value>) -> impl Responder {
    let data = payload.into_inner();
    let input: Input = serde_json::from_value(data.clone()).unwrap();
    let binding = input.key;
    //println!("{}", &binding);
    let mut guarded = INPUT.lock().unwrap();
    *guarded = binding;
    HttpResponse::Ok().json(json!({
        "status": "success"
    }))
}

pub async fn idle() -> impl Responder {
    HttpResponse::Ok().json(json!({
        "idle": IDLE.load(Ordering::Relaxed)
    }))
}
