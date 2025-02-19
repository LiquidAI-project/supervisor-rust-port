//!
//! This module contains tests for testing api.rs
//! 

use actix_web::{test, App, web, http::StatusCode};
use serde_json::Value;
use supervisor::lib::api::*;


#[cfg(test)]
mod wasmtime_tests {
    use super::*;
    // const TEST_PATH: &str = "tests/";

    // Set to false to get full stacktrace from tests
    const SUPPRESS_STACKTRACE: bool = true;

    /// Helper function to print test results
    async fn print_test_response(s: &str, status: StatusCode, body: &[u8]) {
        println!("Test Case: {}", s);
        println!("Status Code: {}", status);
        if let Ok(json_body) = serde_json::from_slice::<Value>(body) {
            println!("Response JSON: {:#}", json_body);
        } else {
            println!("Response Body: {:?}", String::from_utf8_lossy(body));
        }
    }

    #[actix_web::test]
    async fn test_wasmiot_device_description() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_thingi_description() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_thingi_health() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_get_module_result() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_request_history_list_without_id() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_request_history_list_with_id() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_run_module_function() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_deployment_delete() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
    async fn test_deployment_create() {
        if SUPPRESS_STACKTRACE {
            let f = |_: &std::panic::PanicHookInfo| {};
            std::panic::set_hook(Box::new(f));
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
