//! # wasmtime_imports.rs
//!
//! This module provides custom host function imports for Wasm modules executed using Wasmtime.
//!
//! Specifically, it defines camera-related functionality that is injected into Wasm instances via WASI:
//!
//! - `takeImageDynamicSize`: Captures an image and returns a dynamic-length JPEG buffer.
//! - `takeImageStaticSize`: Captures an image and writes a fixed-size JPEG buffer.
//!
//! These are typically used by image-processing Wasm modules (e.g., ML inference or camera pipelines).


use wasmtime::{Caller, Val, Result};
use wasmtime_wasi::preview1::WasiP1Ctx;
#[cfg(not(feature = "arm32"))]
use std::env;
#[cfg(not(feature = "arm32"))]
use opencv::{
    prelude::*,
    videoio::{VideoCapture, VideoCaptureTrait, CAP_ANY},
    imgcodecs::{imencode, IMWRITE_JPEG_QUALITY},
    core::{Vector, Mat},
};


/// Host function import: dynamically captures a JPEG image and returns its buffer pointer + length.
///
/// This function is callable by Wasm modules and will:
/// - Capture a frame from the default camera
/// - Encode it as JPEG
/// - Write the buffer into linear memory at offset 0
/// - Write the buffer pointer and size to two Wasm memory locations provided in `args`
///
/// # Arguments
/// * `args[0]`: memory location to write buffer pointer (u32)
/// * `args[1]`: memory location to write buffer length (u32)
///
/// # Returns
/// * `Ok(())` if successful, or an error if arguments are missing or memory access fails
///
/// # Note
/// - This currently always writes the buffer at offset `0`. If multiple images are captured,
///   this could cause memory overwrite in Wasm unless the module handles allocation.
/// - This function currently does nothing in an arm32 enviroment.
/// 
/// # Safety
/// This function assumes Wasm has exported a linear memory named "memory".
#[cfg(not(feature = "arm32"))]
#[allow(non_snake_case)]
pub fn takeImageDynamicSize(
    mut caller: Caller<'_, WasiP1Ctx>,
    args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    let out_ptr_ptr = match args.get(0) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected first argument to be i32").into()),
    };
    let out_size_ptr = match args.get(1) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected second argument to be i32").into()),
    };

    let frame = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    let mut buffer = Vector::new();
    let params = Vector::from_slice(&[IMWRITE_JPEG_QUALITY, 95]);
    imencode(".jpg", &frame, &mut buffer, &params).map_err(|e| anyhow::anyhow!(e))?;
    let image_data = buffer.to_vec();
    let data_len = image_data.len();

    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let offset = 0; // NOTE: fixed offset, may need allocator in future
    memory.write(&mut caller, offset, &image_data)?;

    // Write buffer pointer and size back into Wasm memory
    memory.write(&mut caller, out_ptr_ptr as usize, &(offset as u32).to_le_bytes())?;
    memory.write(&mut caller, out_size_ptr as usize, &(data_len as u32).to_le_bytes())?;
    Ok(())
}

/// Host function import: captures a JPEG image with a statically defined size in memory.
///
/// This function is called by Wasm modules and will:
/// - Read a fixed buffer size from memory
/// - Capture and encode a frame as JPEG
/// - Truncate the image data to the desired size
/// - Write it into the memory address provided
///
/// # Arguments
/// * `args[0]`: pointer to buffer location where image should be written (u32)
/// * `args[1]`: pointer to 4-byte location containing the desired size (u32)
///
/// # Notes
/// - This function currently does nothing in arm32 enviroments
/// 
/// # Returns
/// * `Ok(())` if successful, or error if arguments or memory access fails
#[allow(non_snake_case)]
#[cfg(not(feature = "arm32"))]
pub fn takeImageStaticSize(
    mut caller: Caller<'_, WasiP1Ctx>,
    args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    let out_ptr = match args.get(0) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected first argument to be i32").into()),
    };
    let size_ptr = match args.get(1) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected second argument to be i32").into()),
    };

    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let mut size_bytes = [0u8; 4];
    memory.read(&mut caller, size_ptr as usize, &mut size_bytes)?;
    let expected_size = u32::from_le_bytes(size_bytes) as usize;

    let frame = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    let mut buffer = Vector::new();
    let params = Vector::from_slice(&[IMWRITE_JPEG_QUALITY, 95]);
    imencode(".jpg", &frame, &mut buffer, &params).map_err(|e| anyhow::anyhow!(e))?;
    let mut image_data = buffer.to_vec();

    image_data.truncate(expected_size);
    memory.write(&mut caller, out_ptr as usize, &image_data)?;
    Ok(())
}

/// Captures a frame from the default camera using OpenCV.
///
/// Attempts to read the camera device defined in the `DEFAULT_CAMERA_DEVICE` environment variable,
/// or falls back to device `0` if unset.
///
/// # Notes
/// - This function currently does nothing on arm32 enviroments
/// 
/// # Returns
/// A valid OpenCV `Mat` frame or a string describing the failure.
///
/// # Errors
/// - Camera not available
/// - Capture failure
/// - Frame is empty
#[cfg(not(feature = "arm32"))]
pub fn capture_image() -> Result<Mat, String> {
    let device = env::var("DEFAULT_CAMERA_DEVICE")
        .ok()
        .and_then(|val| val.parse::<i32>().ok())
        .unwrap_or(0);

    let mut cam = VideoCapture::new(device, CAP_ANY).map_err(|e| e.to_string())?;
    cam.is_opened().map_err(|e| e.to_string())?;

    let mut frame = Mat::default();
    cam.read(&mut frame).map_err(|e| e.to_string())?;

    if frame.empty() {
        return Err("Captured frame is empty".into());
    }
    Ok(frame)
}

#[cfg(feature = "arm32")]
pub fn capture_image() {
    ()
}

#[allow(non_snake_case)]
#[cfg(feature = "arm32")]
pub fn takeImageDynamicSize(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    Ok(())
}

#[allow(non_snake_case)]
#[cfg(feature = "arm32")]
pub fn takeImageStaticSize(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    Ok(())
}

// Placeholder functions for making supervisor able to run modules that require them
// without implementing the actual functionality.

// Matches original placeholder listing of supervisor interfaces, and functions listed in wasm3_api (wasmiot_modules repository)

pub fn millis(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn delay(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn print(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn println(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn printInt(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn rpcCall(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn takeImage(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn path_open(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_filestat_get(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_read(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_readdir(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_seek(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_write(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_close(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_prestat_get(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn fd_prestat_dir_name(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn sched_yield(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn random_get(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn proc_exit(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn environ_sizes_get(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn environ_get(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn pinMode(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn digitalWrite(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn getPinLED(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn getChipID(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn printFloat(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn wifiConnect(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn wifiStatus(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn wifiLocalIp(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn printWifiLocalIp(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn httpPost(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

pub fn http_post(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn readTemperature(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[allow(non_snake_case)]
pub fn readHumidity(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}
