//! # wasmtime_imports.rs
//! 

use wasmtime::{Caller, Val, Result};
use nokhwa::Camera;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::pixel_format::RgbFormat;
use image::codecs::jpeg::JpegEncoder;
use image::ColorType;
use std::env;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

#[cfg(not(feature = "armv6"))]
use wasmtime_wasi::preview1::WasiP1Ctx;

/// Host function import: captures a JPEG image with a statically defined size in memory.
///
/// This function is called by Wasm modules and will:
/// - Read a fixed buffer size from memory
/// - Capture and encode a frame as JPEG
/// - Truncate the image data to the predefined size
/// - Write it into the memory address provided
///
/// # Arguments
/// * `args[0]`: pointer to buffer location where image should be written (u32)
/// * `args[1]`: pointer to 4-byte location containing the desired size (u32)
///
/// # Returns
/// * `Ok(())` if successful, or error if arguments or memory access fails
#[cfg(not(feature="armv6"))]
#[allow(non_snake_case)]
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

    let mut image_data = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    image_data.truncate(expected_size);
    memory.write(&mut caller, out_ptr as usize, &image_data)?;
    Ok(())
}

/// Version of takeImageStaticSize with different function signature (for armv6 where wasi isnt supported)
#[cfg(feature="armv6")]
#[allow(non_snake_case)]
pub fn takeImageStaticSize(
    mut caller: Caller<'_, ()>,
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

    let mut image_data = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    image_data.truncate(expected_size);
    memory.write(&mut caller, out_ptr as usize, &image_data)?;
    Ok(())
}

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
/// 
/// # Safety
/// This function assumes Wasm has exported a linear memory named "memory".
#[cfg(not(feature="armv6"))]
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

    let image_data = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    let data_len = image_data.len();

    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let offset = 0;
    memory.write(&mut caller, offset, &image_data)?;
    memory.write(&mut caller, out_ptr_ptr as usize, &(offset as u32).to_le_bytes())?;
    memory.write(&mut caller, out_size_ptr as usize, &(data_len as u32).to_le_bytes())?;
    Ok(())
}

/// Version of takeImageDynamicSize with different function signature (for armv6 where wasi isnt supported)
#[cfg(feature="armv6")]
#[allow(non_snake_case)]
pub fn takeImageDynamicSize(
    mut caller: Caller<'_, ()>,
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

    let image_data = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    let data_len = image_data.len();

    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let offset = 0;
    memory.write(&mut caller, offset, &image_data)?;
    memory.write(&mut caller, out_ptr_ptr as usize, &(offset as u32).to_le_bytes())?;
    memory.write(&mut caller, out_size_ptr as usize, &(data_len as u32).to_le_bytes())?;
    Ok(())
}

/// Captures a frame from the default camera using nokhwa
///
/// Attempts to read the camera device defined in the `DEFAULT_CAMERA_DEVICE` environment variable,
/// or falls back to device `0` if unset.
/// 
/// # Returns
/// A vec<u8> containing image pixels, or a string containing the error
///
/// # Errors
/// - Camera not available
/// - Capture failure
/// - Frame is empty
pub fn capture_image() -> Result<Vec<u8>, String> {
    let device = env::var("DEFAULT_CAMERA_DEVICE")
        .ok()
        .and_then(|val| val.parse::<u32>().ok())
        .unwrap_or(0);
    let cam_index = CameraIndex::Index(device);
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(cam_index, requested)
        .map_err(|e| format!("Failed to initialize camera: {}", e))?;

    camera.open_stream().map_err(|e| format!("Failed to open stream: {}", e))?;
    let frame = camera.frame().map_err(|e| format!("Failed to capture frame: {}", e))?;
    let decoded = frame.decode_image::<RgbFormat>()
        .map_err(|e| format!("Failed to decode frame: {}", e))?;

    let mut jpeg_buf = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut jpeg_buf, 95);
    encoder
        .encode(&decoded, decoded.width(), decoded.height(), ColorType::Rgb8.into())
        .map_err(|e| format!("Failed to encode JPEG: {}", e))?;

    Ok(jpeg_buf)
}

#[cfg(not(feature="armv6"))]
#[allow(non_snake_case)]
pub fn takeImage(
    mut _caller: Caller<'_, WasiP1Ctx>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}

#[cfg(feature="armv6")]
#[allow(non_snake_case)]
pub fn takeImage(
    mut _caller: Caller<'_, ()>,
    _args: &[Val],
    _results: &mut [Val],
) -> Result<()> {
    unimplemented!();
}


/// Computes the mean and standard deviation of a slice of f32 values.
/// Returns (mean, standard_deviation).
fn stats(times_ms: &[f32]) -> (f32, f32) {
    if times_ms.is_empty() {
        return (0.0, 0.0);
    }
    let n = times_ms.len() as f32;
    let sum: f32 = times_ms.iter().copied().sum();
    let mean = sum / n;
    let var = times_ms.iter().map(|v| {
        let d = *v - mean;
        d * d
    }).sum::<f32>() / n;
    (mean, var.sqrt())
}


/// Performs a ping to the specified IPv4 address.
/// # Arguments
/// * `args[0]`: first octet of IPv4 address (u8 as i32)
/// * `args[1]`: second octet of IPv4 address (u8 as i32)
/// * `args[2]`: third octet of IPv4 address (u8 as i32)
/// * `args[3]`: fourth octet of IPv4 address (u8 as i32)
/// * `args[4]`: number of ping attempts (i32)
/// # Returns
/// * `results[0]`: average round-trip time in milliseconds (f32)
/// * `results[1]`: standard deviation of round-trip time in milliseconds (f32)
/// * `results[2]`: packet loss ratio (f32, 0.0 to 1.0)
pub fn ping(
    mut _caller: Caller<'_, WasiP1Ctx>,
    args: &[Val],
    results: &mut [Val],
) -> Result<()> {
    let a = match args.get(0) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("ping: arg0 missing or invalid") };
    let b = match args.get(1) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("ping: arg1 missing or invalid") };
    let c = match args.get(2) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("ping: arg2 missing or invalid") };
    let d = match args.get(3) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("ping: arg3 missing or invalid") };
    let mut count = match args.get(4) { Some(Val::I32(v)) => *v, _ => anyhow::bail!("ping: arg4 missing or invalid") };
    if count < 1 { count = 1; }
    if count > 50 { count = 50; } // Limit to 50 pings max

    let ip = IpAddr::V4(Ipv4Addr::new(a,b,c,d));
    let mut times: Vec<f32> = Vec::with_capacity(count as usize);
    let mut lost: u32 = 0;

    for _ in 0..count {
        let mut p = ping::new(ip);
        p.timeout(Duration::from_secs(2));      // Set 2 second timeout
        match p.send() {
            Ok(reply) => {
                let ms = reply.rtt.as_secs_f64() as f32 * 1000.0;
                times.push(ms);
            }
            Err(_e) => {
                lost += 1;
            }
        }
    }

    let (avg, stdev) = stats(&times);
    let loss = (lost as f32) / (count as f32);

    // Check that the given results slice has enough space
    if results.len() != 3 {
        anyhow::bail!("expected 3 results");
    }
    results[0] = Val::F32(avg.to_bits());
    results[1] = Val::F32(stdev.to_bits());
    results[2] = Val::F32(loss.to_bits());
    Ok(())
}