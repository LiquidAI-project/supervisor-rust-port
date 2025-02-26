//!
//! This module contains tests for testing api.rs
//! 

use actix_web::{test, App, web, http::StatusCode, HttpServer, HttpResponse, Responder, post};
use serde_json::Value;
use supervisor::lib::api::*;
use log::{debug, info};

use std::{sync::{Arc, Mutex}, env, time::Duration};
use tokio::time::sleep;


#[cfg(test)]
mod wasmtime_tests {
    use super::*;
    
    const SUPPRESS_STACKTRACE: bool = true; // Set to false to get full stacktrace from tests
    const INITIAL_WAIT: bool = true; // Wait for logging server to start
    const LOGGING_ADDRESS: &str = "127.0.0.1:4141"; // Address to bind the test logging server to
    const DEFAULT_LOGGING_LEVEL: &str = "info"; // Value can be trace/debug/info/warn/error
    type LogStorage = Arc<Mutex<Vec<Value>>>; // Type for storing logs

    /// Helper function to print test results
    async fn print_test_response(s: &str, status: StatusCode, _body: &[u8]) {
        debug!("Test Case: {}, Status code: {}", s, status);
        // if let Ok(json_body) = serde_json::from_slice::<Value>(body) {
        //     debug!("Response JSON: {:#}", json_body);
        // } else {
        //     debug!("Response Body: {:?}", String::from_utf8_lossy(body));
        // }
    }


    /// Handler for receiving logs
    #[post("/device/logs")]
    async fn receive_logs(logs: web::Data<LogStorage>, log_entry: web::Json<Value>) -> impl Responder {
        let mut stored_logs = logs.lock().unwrap();
        let log_clone = log_entry.clone();
        stored_logs.push(log_entry.into_inner());

        info!("Received log entry!");
        debug!("Entry: {:?}", log_clone);
        HttpResponse::Ok().json("Log received")
    }


    /// Starts a logging server for testing purposes
    async fn start_log_server(logs: LogStorage, duration: Duration) {
        let logs_data = web::Data::new(logs.clone());
        let server = HttpServer::new(move || {
            App::new()
                .app_data(logs_data.clone())
                .service(receive_logs)
        })
        .bind(LOGGING_ADDRESS)
        .expect("Failed to bind log server")
        .run();

        // Limit the duration logging server is on
        tokio::spawn(async move {
            sleep(duration).await;
            println!("Shutting down log server...");
            std::process::exit(0);
        });

        server.await.expect("Failed to run HTTP log server");
    }


    /// Test function to capture logs with 10 second timeout
    #[tokio::test]
    async fn api_test_log_capturing() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        let logs = Arc::new(Mutex::new(Vec::<Value>::new()));

        // Setup logging enviroment
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(DEFAULT_LOGGING_LEVEL)).init();
        env::set_var("EXTERNAL_LOGGING_ENABLED", "true");
        env::set_var("WASMIOT_LOGGING_ENDPOINT", format!("http://{}/device/logs", LOGGING_ADDRESS));
        
        // Start logging server. It closes automatically after 10 seconds
        let logs_clone = Arc::clone(&logs);
        tokio::spawn(start_log_server(logs_clone, Duration::from_secs(10)));
        sleep(Duration::from_secs(10)).await;

        // Print collected logs
        let collected_logs = logs.lock().unwrap();
        println!("Collected Logs:");
        for (i, log) in collected_logs.iter().enumerate() {
            println!("{}. {}", i + 1, log);
        }
        assert!(!collected_logs.is_empty(), "No logs were captured.");
    }


    #[actix_web::test]
    async fn api_test_wasmiot_device_description() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/.well-known/wasmiot-device-description", web::get().to(wasmiot_device_description))).await;
        let req = test::TestRequest::get().uri("/.well-known/wasmiot-device-description").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("wasmiot_device_description", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
    #[actix_web::test]
    async fn api_test_thingi_description() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/.well-known/wot-thing-description", web::get().to(thingi_description))).await;
        let req = test::TestRequest::get().uri("/.well-known/wot-thing-description").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("thingi_description", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
    #[actix_web::test]
    async fn api_test_thingi_health() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/health", web::get().to(thingi_health))).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("thingi_health", status, &body).await;
        assert_eq!(status, StatusCode::OK);
        // let body = test::read_body(resp).await;
        // let json_body: Value = serde_json::from_slice(&body).unwrap();
        // assert!(json_body.get("cpuUsage").is_some());
        // assert!(json_body.get("memoryUsage").is_some());
    }
    
    #[actix_web::test]
    async fn api_test_get_module_result() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/module_results/{module_name}/{filename}", web::get().to(get_module_result))).await;
        let req = test::TestRequest::get().uri("/module_results/test_module/test_file").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("get_module_results", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
    #[actix_web::test]
    async fn api_test_request_history_list_without_id() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/request-history/{request_id}", web::get().to(request_history_list))).await;
        let req = test::TestRequest::get().uri("/request-history/").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("request_history_list_without_id", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
    #[actix_web::test]
    async fn api_test_request_history_list_with_id() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/request-history/{request_id}", web::get().to(request_history_list))).await;
        let req = test::TestRequest::get().uri("/request-history/123").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("request_history_list_with_id", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
    #[actix_web::test]
    async fn api_test_run_module_function() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/{deployment_id}/modules/{module_name}/{function_name}/{filename}", web::get().to(run_module_function))).await;
        let req = test::TestRequest::get().uri("/123/modules/mod1/func1/file.txt").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("run_module_function", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
    #[actix_web::test]
    async fn api_test_deployment_delete() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/deploy/{deployment_id}", web::delete().to(deployment_delete))).await;
        let req = test::TestRequest::delete().uri("/deploy/456").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("deployment_delete", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
    #[actix_web::test]
    async fn api_test_deployment_create() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
        }
        if INITIAL_WAIT {
            sleep(Duration::from_secs(2)).await;
        }
        let app = test::init_service(App::new().route("/deploy", web::post().to(deployment_create))).await;
        let req = test::TestRequest::post().uri("/deploy").to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body = test::read_body(resp).await;
        print_test_response("deployment_create", status, &body).await;
        assert_eq!(status, StatusCode::OK);
    }
    
}
