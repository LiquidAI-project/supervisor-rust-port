use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};



/// Represents a single request made to the supervisor for executing a WebAssembly function.
///
/// Tracks metadata like request time, parameters, function name, execution status, and result.
/// The `request_id` is a hash based on module/function identifiers and time for uniqueness.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RequestEntry {
    /// Unique identifier for this request.
    pub request_id: String,
    /// Deployment ID this request belongs to.
    pub deployment_id: String,
    /// The module name whose function is being executed.
    pub module_name: String,
    /// The function name within the module to run.
    pub function_name: String,
    /// The HTTP method used for this request.
    pub method: String,
    /// Query or JSON arguments passed to the function.
    pub request_args: Value,
    /// Mapping from mount path -> local file path for input files.
    pub request_files: HashMap<String, String>,
    /// Timestamp when this request was queued for execution.
    pub work_queued_at: DateTime<Utc>,
    /// Optional result value (primitive output or result path).
    pub result: Option<Value>,
    /// List that contains links to possible output mount files
    pub outputs: Vec<String>,
    /// Indicates whether the execution succeeded.
    pub success: bool,
}

impl RequestEntry {
    /// Construct a new request entry and auto-generate a unique request ID.
    pub fn new(
        deployment_id: String,
        module_name: String,
        function_name: String,
        method: String,
        request_args: Value,
        request_files: HashMap<String, String>,
        work_queued_at: DateTime<Utc>,
    ) -> Self {
        let mut entry = RequestEntry {
            request_id: String::new(),
            deployment_id,
            module_name,
            function_name,
            method,
            request_args,
            request_files,
            work_queued_at,
            result: None,
            outputs: Vec::new(),
            success: false,
        };
        entry.init_request_id();
        entry
    }

    /// Initializes `request_id` by hashing the module/function and timestamp.
    fn init_request_id(&mut self) {
        let key = format!("{}:{}:{}", self.deployment_id, self.module_name, self.function_name);
        let time = Utc::now().to_rfc3339();
        let input_string = format!("{}:{}", key, time);
        let mut hasher = Sha256::new();
        hasher.update(input_string.as_bytes());
        let hash_bytes = hasher.finalize();
        self.request_id = hex::encode(hash_bytes);
    }
}