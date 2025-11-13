//! # wasmtime_imports.rs
//! 

use wasmtime::{Caller, Val, Result};
use nokhwa::Camera;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::pixel_format::RgbFormat;
use image::codecs::jpeg::JpegEncoder;
use image::ColorType;
use std::env;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};

#[cfg(not(feature = "armv6"))]
use crate::lib::wasmtime::Ctx;

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
    mut caller: Caller<'_, Ctx>,
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
    mut caller: Caller<'_, Ctx>,
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
    mut _caller: Caller<'_, Ctx>,
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



/// Performs one ping to the specified IPv4 address.
/// # Arguments
/// * `args[0]`: first octet of IPv4 address (u8 as i32)
/// * `args[1]`: second octet of IPv4 address (u8 as i32)
/// * `args[2]`: third octet of IPv4 address (u8 as i32)
/// * `args[3]`: fourth octet of IPv4 address (u8 as i32)
/// # Returns
/// * `results[0]`: round-trip time in milliseconds (f32)
pub fn ping<'a>(
    mut _caller: Caller<'a, Ctx>,
    args: &'a [Val],
    results: &'a mut [Val],
) -> Box<dyn Future<Output = Result<()>> + Send + 'a> {
    Box::new(async move {
        if results.len() != 1 {
            anyhow::bail!("Ping function: results array did not have length of 1");
        }

        let a = match args.get(0) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("Ping function: arg0 invalid or missing") };
        let b = match args.get(1) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("Ping function: arg1 invalid or missing") };
        let c = match args.get(2) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("Ping function: arg2 invalid or missing") };
        let d = match args.get(3) { Some(Val::I32(v)) => *v as u8, _ => anyhow::bail!("Ping function: arg3 invalid or missing") };

        let addr = IpAddr::V4(Ipv4Addr::new(a, b, c, d));

        let client = match Client::new(&Config::default()) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Ping function: client error: {e:?}");
                results[0] = Val::F32((-1.0f32).to_bits());
                return Ok(());
            }
        };

        let mut pinger = client.pinger(addr, PingIdentifier(0x1234)).await;
        pinger.timeout(Duration::from_secs(2));

        let ms = match pinger.ping(PingSequence(1), &[0u8; 8]).await {
            Ok((_, dur)) => (dur.as_secs_f64() * 1000.0) as f32,
            Err(e) => { log::error!("ping failed: {e:?}"); -1.0 }
        };

        results[0] = Val::F32(ms.to_bits());
        Ok(())
    })
}
