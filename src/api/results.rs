
use actix_files::NamedFile;
use actix_web::{HttpRequest, HttpResponse, Responder, web};
use serde_json::json;

use crate::{function_name, lib::{constants::REQUEST_HISTORY, logging::send_log, utils::get_params_path}};


/// Serves a file produced as output by a WebAssembly module.
///
/// This handles URLs like `/module_results/{deployment_id}/{module_name}/{filename}` and returns
/// the corresponding file from the module’s parameter/output folder.
///
/// # Path Parameters
/// - `deployment_id`: The deployment that contains the module
/// - `module_name`: The module that created the file  
/// - `filename`: The output file name
pub async fn get_module_result(req: HttpRequest, path: web::Path<(String, String, String)>) -> impl Responder {
    let (deployment_id, module_name, filename) = path.into_inner();
    let file_path = get_params_path(&deployment_id, &module_name, Some(&filename));

    let func_name = function_name!().to_string();
    let log_msg = format!("Request for module execution result: {}/{}/{}", deployment_id, module_name, filename);
    tokio::spawn(async move {
        send_log("INFO", &log_msg, &func_name, None).await;
    });

    match NamedFile::open(&file_path) {
        Ok(file) => file.into_response(&req),
        Err(_) => HttpResponse::NotFound().json(json!({
            "error": "Module result file not found",
            "deployment_id": deployment_id,
            "module": module_name,
            "filename": filename
        })),
    }
}

/// Handler for getting request history list
///
/// This is here to match a path that has no parameters vs the default 1 parameter
pub async fn request_history_list_1() -> impl Responder {
    let new_path = web::Path::from("".to_string());
    request_history_list(new_path).await
}

/// Returns previous WebAssembly execution entries or one specific request if ID is given.
///
/// Supports:
/// - `/request-history` to get all requests
/// - `/request-history/{request_id}` to get a specific request entry
///
/// The response includes the success state and result of each request.
/// If the matched request failed, it returns HTTP 500 instead of 200.
pub async fn request_history_list(path: web::Path<String>) -> impl Responder {
    let id = path.into_inner();
    let history = REQUEST_HISTORY.lock();
    if id != "" {
        let func_name = function_name!().to_string();
        let log_msg = format!("Requested history for request ID: {}", id);
        tokio::spawn(async move {
            send_log("INFO", &log_msg, &func_name, None).await;
        });
        if let Some(req) = history.iter().find(|r| r.request_id == id) {
            let status_code = if req.success { 200 } else { 500 };
            return HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap())
                .json(req);
        }
        HttpResponse::NotFound().json(json!({
            "error": "No request with that ID",
            "request_id": id
        }))
    } else {
        let func_name = function_name!().to_string();
        let log_msg = "Requested history for all requests".to_string();
        tokio::spawn(async move {
            send_log("INFO", &log_msg, &func_name, None).await;
        });

        HttpResponse::Ok().json(&*history)
    }
}