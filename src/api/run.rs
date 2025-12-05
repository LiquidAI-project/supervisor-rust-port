
use tokio::task;
use actix_multipart::Multipart;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use actix_files::NamedFile;
use serde_json::{json, Value};
use chrono::Utc;
use std::collections::HashMap;
use log::{debug, error, warn};
use wasmtime::Val;
use sanitize_filename;
use futures_util::StreamExt;
use std::fs::File;
use std::env;
use std::io::Write;
use crate::lib::constants::{DEPLOYMENTS, MAX_DEPLOYMENT_STEPS, REQUEST_HISTORY};
use crate::lib::logging::send_log;
use crate::function_name;
use crate::lib::utils::{get_params_path, make_output_url};
use indexmap::IndexMap;
use crate::structs::request_entry::RequestEntry;
use crate::structs::deployment_supervisor::{CallData, EndpointArgs, EndpointData, MountStage};

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

    let (entry, final_opt) = make_history(entry).await;
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
    let mut resp = json!({ "resultUrl": result_url });
    if let Some(final_json) = final_opt {
        resp["result"] = final_json;
    }
    HttpResponse::Ok().json(resp)
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



/// Executes the WebAssembly function for the given request and performs any chained subcalls.
///
/// This is the core logic that:
/// 1. Prepares and runs the function,
/// 2. Interprets its result,
/// 3. Initiates a next call if the deployment specifies one,
/// 4. Returns the result or sub-response.
pub async fn do_wasm_work(entry: &mut RequestEntry) -> Result<Value, String> {
    let mut deployments = DEPLOYMENTS.lock();
    let deployment = deployments.get_mut(&entry.deployment_id)
        .ok_or_else(|| format!("Deployment '{}' not found", entry.deployment_id))?;

    let func_name = function_name!().to_string();
    let module_name_clone = entry.module_name.clone();
    let entry_clone = entry.clone();
    task::spawn(async move {
        send_log(
            "DEBUG",
            &format!("Preparing Wasm module '{}'", &module_name_clone),
            &func_name,
            Some(&entry_clone)
        ).await;
    });

    let request_args: IndexMap<String, Value> = entry.request_args
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_else(IndexMap::new);
    let (_module, wasm_args) = deployment.prepare_for_running(
        &entry.deployment_id,
        &entry.module_name,
        &entry.function_name,
        &request_args,
        &entry.request_files,
    ).await?;

    let func_name = function_name!().to_string();
    let entry_function_name = entry.function_name.clone();
    let entry_clone = entry.clone();
    task::spawn(async move {
        send_log(
            "DEBUG",
            &format!("Running Wasm function '{}'", &entry_function_name),
            &func_name,
            Some(&entry_clone)
        ).await;
    });

    // Execute the wasm module
    let runtime = deployment.runtimes.get_mut(&entry.module_name)
        .ok_or_else(|| format!("Runtime not found for module '{}'", entry.module_name))?;

    let return_count = runtime.get_return_types(&entry.module_name, &entry.function_name).await.len();
    let output_vals = runtime.run_function(
        &entry.module_name,
        &entry.function_name,
        wasm_args,
        return_count,
    ).await;

    let raw_output = output_vals.first().map(|v| match v {
        Val::I32(i) => json!(i),
        Val::I64(i) => json!(i),
        Val::F32(f) => json!(f32::from_bits(*f)),
        Val::F64(f) => json!(f64::from_bits(*f)),
        _ => Value::Null,
    }).unwrap_or(Value::Null);

    let raw_output_clone = raw_output.clone();
    let entry_clone = entry.clone();
    let func_name = function_name!().to_string();
    tokio::spawn(async move {
        send_log(
            "DEBUG",
            &format!("... Result: {}", raw_output_clone),
            &func_name,
            Some(&entry_clone),
        ).await;
    });

    // Parse current endpoint result according to its declared media type
    let endpoint = &deployment.endpoints[&entry.module_name][&entry.function_name];
    let output_mounts = deployment
        .mounts
        .get(&entry.module_name)
        .and_then(|m| m.get(&entry.function_name))
        .and_then(|sm| sm.get(&MountStage::OUTPUT))
        .cloned()
        .unwrap_or_default();

    let parsed = deployment.parse_endpoint_result(raw_output.clone(), &endpoint.response, &output_mounts);

    // Result into entry
    if let Some(val) = &parsed.0 {
        let func_name = function_name!().to_string();
        let entry_clone = entry.clone();
        let val_clone = val.clone();
        task::spawn( async move {
            send_log(
                "DEBUG",
                &format!("Execution result (parsed): {:?}", &val_clone),
                &func_name,
                Some(&entry_clone)
            ).await;
        });
    }
    if let Some(EndpointData::StrList(filenames)) = &parsed.1 {
        if let Some(filename) = filenames.first() {
            let result_url = make_output_url(&entry.deployment_id, &entry.module_name, filename);
            entry.outputs = filenames.iter()
                .map(|f| make_output_url(&entry.deployment_id, &entry.module_name, f))
                .collect();

            let func_name = function_name!().to_string();
            let entry_clone = entry.clone();
            let result_url_clone = result_url.clone();
            task::spawn(async move {
                send_log(
                    "DEBUG",
                    &format!("Result URL: {}", &result_url_clone),
                    &func_name,
                    Some(&entry_clone),
                ).await;
            });
        }
    }

    entry.result = parsed.0.clone().map(|arg| match arg {
        EndpointArgs::Str(s) => Value::String(s),
        EndpointArgs::StrList(vs) => Value::Array(vs.into_iter().map(Value::String).collect()),
        EndpointArgs::Dict(map) => Value::Object(map.into_iter().collect()),
    });

    // Decide next step
    let step_index = entry.step_index;
    let next_call = deployment
        .next_target_with_index(&entry.module_name, &entry.function_name, step_index)
        .map(|next_ep|CallData::from_endpoint(next_ep, parsed.0.clone(), parsed.1.clone()));

    log::info!("Step index {}", step_index);
    log::info!("Next call: {:?}", next_call);

    // If there is a next call, chain it
    if let Some(call_data) = next_call {
        // Prepare file parts (if any)
        let mut files = HashMap::new();
        match &call_data.files {
            EndpointData::StrList(file_names) => {
                for name in file_names {
                    let full_path = get_params_path(&entry.deployment_id, &entry.module_name, Some(name));
                    let file = std::fs::File::open(&full_path)
                        .map_err(|e| format!("Failed to open file for subcall: {}", e))?;
                    files.insert(name.clone(), file);
                }
            }
        }

        // Build headers (include incremented step)
        let mut headers = reqwest::header::HeaderMap::new();
        for (k, v) in &call_data.headers {
            if let (Ok(key), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                headers.insert(key, val);
            }
        }
        let next_idx = step_index.saturating_add(1);
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

        drop(deployments);

        // Prepare multipart form
        let mut form = reqwest::multipart::Form::new();

        for (name, mut file) in files {
            let mut buf = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buf).map_err(|e| format!("Failed to read file for multipart: {}", e))?;

            form = form.part(name.clone(), reqwest::multipart::Part::bytes(buf).file_name(name));
        }

        // Build request
        let client = reqwest::Client::new();
        let req_builder = client
            .request(
                call_data.method.to_uppercase().parse().unwrap_or(reqwest::Method::POST),
                &call_data.url
            )
            .headers(headers)
            .multipart(form);

        // Send
        let response = req_builder
            .send()
            .await
            .map_err(|e| format!("Failed to send chained request: {}", e))?;

        // Assume JSON response from the chained call
        let chained_json: Value = response
            .json()
            .await
            .map_err(|e| format!("Invalid response JSON from {}: {}", call_data.url, e))?;

        // If there's a resultUrl, fetch it (also expected to be JSON)
        if let Some(url) = chained_json.get("resultUrl").and_then(|v| v.as_str()) {
            let fetched_json: Value = client
                .get(url)
                .send()
                .await
                .map_err(|e| format!("Failed to fetch resultUrl {}: {}", url, e))?
                .json()
                .await
                .map_err(|e| format!("Invalid JSON from resultUrl {}: {}", url, e))?;

            // If the fetched JSON contains a "result" key, return that; else return the fetched JSON.
            let final_json: Value = fetched_json
                .get("result")
                .cloned()
                .unwrap_or_else(|| fetched_json.clone());

            // Return the final JSON, but dont overwrite own results in history with it
            entry.success = true;
            return Ok(final_json);
        }

        // No resultUrl -> record and return the original chained JSON
        entry.success = true;
        return Ok(chained_json);
    }

    Ok(json!({ "result": entry.result }))
}



