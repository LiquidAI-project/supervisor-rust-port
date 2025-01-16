// wasm_utils.rs

use std::time::{SystemTime, UNIX_EPOCH};
use std::thread::sleep;
use std::time::Duration;
use std::io::{Error as IoError};
use std::sync::Arc;
use log::{info, warn}; // For logging

// Hypothetical `IWasmRuntime` trait from previous code:
use crate::wasm_api::IWasmRuntime;

// ---------- Utility / Global Functions ---------- //

pub fn python_clock_ms() -> u128 {
    // Current epoch time in milliseconds.
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    now.as_millis()
}

pub fn python_delay(delay_ms: u64) {
    sleep(Duration::from_millis(delay_ms));
}

pub fn python_println(message: &str) {
    println!("{}\n", message);
}

pub fn python_print_int(number: i64) {
    print!("{}", number);
}

pub fn python_get_temperature() -> f64 {
    // We'll mimic the "Windows => 0.0" logic, ignoring actual sensor code.
    #[cfg(target_os = "windows")]
    {
        0.0
    }
    #[cfg(not(target_os = "windows"))]
    {
        // We can pretend to read from a sensor or just return a dummy value
        23.4_f64
    }
}

pub fn python_get_humidity() -> f64 {
    #[cfg(target_os = "windows")]
    {
        0.0
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Return a dummy humidity
        40.0_f64
    }
}

/// Stub function that would interface with camera code in Rust.
pub fn capture_image() -> Result<Vec<u8>, String> {
    // In Python, we use `cv2.VideoCapture`.
    // In Rust, you'd likely use some crate for camera capture, then return raw bytes or an image struct.
    // We'll just stub here:
    // Return a fake JPEG data:
    let fake_jpeg = vec![0xFF, 0xD8, 0xFF, 0x00, 0x11, 0x22, 0x33]; // minimal nonsense
    Ok(fake_jpeg)
}

// -------------- RemoteFunction Base -------------- //

pub trait IRemoteFunction {
    fn runtime(&self) -> Arc<dyn IWasmRuntime>;
    fn function(&self) -> Box<dyn Fn(Vec<i64>) -> Option<i64> + '_>;
    // This signature is arbitrary for demonstration. In Python code,
    // each "function" property returns a function with a specialized signature,
    // but in Rust we often unify the approach or use different traits for each function type.
}

// Base struct
pub struct RemoteFunctionBase {
    runtime: Arc<dyn IWasmRuntime>,
}

impl RemoteFunctionBase {
    pub fn new(runtime: Arc<dyn IWasmRuntime>) -> Self {
        Self { runtime }
    }
    pub fn runtime(&self) -> &Arc<dyn IWasmRuntime> {
        &self.runtime
    }
}

// -------------- Concrete RemoteFunction: Print -------------- //

pub struct PrintFunction {
    base: RemoteFunctionBase,
}

impl PrintFunction {
    pub fn new(runtime: Arc<dyn IWasmRuntime>) -> Self {
        Self {
            base: RemoteFunctionBase::new(runtime),
        }
    }

    /// Actual logic: read [pointer, length], decode bytes, print
    /// We'll mimic the python signature: (pointer: i32, length: i32) -> ()
    pub fn print_impl(&self, pointer: i32, length: i32) {
        let module_name = self.base.runtime().current_module_name();
        let (data, error) = self.base.runtime().read_from_memory(
            pointer as usize, length as usize, module_name
        );
        let message = if error.is_none() {
            match String::from_utf8(data) {
                Ok(s) => s,
                Err(_) => "[Invalid UTF-8 data]".to_string(),
            }
        } else {
            error.unwrap()
        };
        print!("{}", message);
    }
}

// -------------- Concrete RemoteFunction: TakeImageDynamicSize -------------- //

pub struct TakeImageDynamicSize {
    base: RemoteFunctionBase,
}

impl TakeImageDynamicSize {
    pub fn new(runtime: Arc<dyn IWasmRuntime>) -> Self {
        Self {
            base: RemoteFunctionBase::new(runtime),
        }
    }

    fn alloc(&self, nbytes: i32) -> Option<i64> {
        let module_name = self.base.runtime().current_module_name();
        // run_function("alloc", [nbytes]), returning i64 pointer
        let params = vec![Box::new(nbytes as i64) as Box<dyn std::any::Any>];
        let result = self.base.runtime().run_function("alloc", &params, module_name);
        result.map(|boxed| *boxed.downcast::<i64>().unwrap_or_else(|_| Box::new(-1i64)))
    }

    /// This emulates python_take_image_dynamic_size:
    /// out_ptr_ptr, out_size_ptr -> store pointer + size (both 32bits).
    pub fn take_image_impl(&self, out_ptr_ptr: i32, out_size_ptr: i32) -> Result<(), String> {
        let img_data = capture_image().map_err(|e| e)?;
        let data_len = img_data.len();
        let data_ptr = self.alloc(data_len as i32).ok_or("Allocation failed")?;

        if data_ptr < 0 {
            return Err(format!("Unable to allocate {} bytes!", data_len));
        }

        // Write the image to memory
        let module_name = self.base.runtime().current_module_name();
        let maybe_err = self.base.runtime().write_to_memory(
            data_ptr as usize,
            &img_data,
            module_name
        );
        if maybe_err.is_some() {
            return Err(maybe_err.unwrap());
        }

        // Write pointer + length to out_ptr_ptr, out_size_ptr as 32-bit
        use std::mem;
        let pointer_bytes = (data_ptr as u32).to_le_bytes();
        let length_bytes = (data_len as u32).to_le_bytes();

        if let Some(e) = self.base.runtime().write_to_memory(out_ptr_ptr as usize, &pointer_bytes, module_name) {
            return Err(e);
        }
        if let Some(e) = self.base.runtime().write_to_memory(out_size_ptr as usize, &length_bytes, module_name) {
            return Err(e);
        }
        Ok(())
    }
}

// -------------- Additional Classes: TakeImageStaticSize, RpcCall, etc. -------------- //
// We'll stub them similarly:

pub struct TakeImageStaticSize {
    base: RemoteFunctionBase,
}

impl TakeImageStaticSize {
    pub fn new(runtime: Arc<dyn IWasmRuntime>) -> Self {
        Self { base: RemoteFunctionBase::new(runtime) }
    }

    /// python_take_image_static_size
    pub fn take_image_impl(&self, out_ptr: i32, size_ptr: i32) -> Result<(), String> {
        let img_data = capture_image().map_err(|e| e)?;

        let module_name = self.base.runtime().current_module_name();
        // read 4 bytes from size_ptr
        let (out_len_bytes, err) = self.base.runtime().read_from_memory(size_ptr as usize, 4, module_name);
        if let Some(e) = err {
            return Err(e);
        }
        if out_len_bytes.len() < 4 {
            return Err("Not enough bytes to parse length".to_string());
        }

        let out_len = u32::from_le_bytes(out_len_bytes.try_into().unwrap()) as usize;

        // Truncate data
        let truncated = if img_data.len() > out_len {
            &img_data[..out_len]
        } else {
            &img_data
        };
        // Write truncated data
        let maybe_err = self.base.runtime().write_to_memory(
            out_ptr as usize,
            truncated,
            module_name
        );
        if let Some(e) = maybe_err {
            return Err(e);
        }
        Ok(())
    }
}

pub struct RpcCall {
    base: RemoteFunctionBase,
    // Suppose we have some local map or external config
}

impl RpcCall {
    pub fn new(runtime: Arc<dyn IWasmRuntime>) -> Self {
        Self {
            base: RemoteFunctionBase::new(runtime),
        }
    }

    // python_rpc_call
    // read func_name, read data, do an HTTP POST...
    // We'll stub out the actual request in Rust
    pub fn rpc_call_impl(&self, func_name_ptr: i32, func_name_size: i32, data_ptr: i32, data_size: i32) {
        let module_name = self.base.runtime().current_module_name();
        let (func_name_bytes, err1) = self.base.runtime().read_from_memory(func_name_ptr as usize, func_name_size as usize, module_name);
        if let Some(e) = err1 {
            eprintln!("Error reading function name: {}", e);
            return;
        }
        let func_name = match String::from_utf8(func_name_bytes) {
            Ok(s) => s,
            Err(_) => "<invalid-utf8>".to_string(),
        };
        eprintln!("func_name: {}", func_name);

        let (data_bytes, err2) = self.base.runtime().read_from_memory(data_ptr as usize, data_size as usize, module_name);
        if let Some(e) = err2 {
            eprintln!("Error reading data: {}", e);
            return;
        }

        // In python, we do `requests.post(func["host"], files=...)`
        // We'll stub: just log the length
        eprintln!("Would POST {} bytes of data to host from function name '{}'", data_bytes.len(), func_name);
    }
}

// RandomGet
pub struct RandomGet {
    base: RemoteFunctionBase,
}

impl RandomGet {
    pub fn new(runtime: Arc<dyn IWasmRuntime>) -> Self {
        Self {
            base: RemoteFunctionBase::new(runtime),
        }
    }

    pub fn random_get_impl(&self, buf_ptr: i32, size: i32) -> i32 {
        let random_bytes = super::std::fs::read("/dev/urandom").unwrap_or_else(|_| vec![0; size as usize]);
        let truncated = &random_bytes[..(size as usize)];

        let module_name = self.base.runtime().current_module_name();
        if let Some(err) = self.base.runtime().write_to_memory(buf_ptr as usize, truncated, module_name) {
            eprintln!("Error writing random bytes: {}", err);
            // Return a WASI errno code
            return 28; // EINVAL
        }
        0 // SUCCESS
    }
}
