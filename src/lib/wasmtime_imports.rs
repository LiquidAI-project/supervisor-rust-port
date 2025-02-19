//!
//! Miscellaneous utility functions related to wasmtime
//! 

use wasmtime::{Caller, Val, Result};
use wasmtime_wasi::preview1::WasiP1Ctx;
use std::env;
use opencv::{
    prelude::*,
    videoio::{VideoCapture, VideoCaptureTrait, CAP_ANY},
    imgcodecs::{imencode, IMWRITE_JPEG_QUALITY},
    core::{Vector, Mat},
};

#[allow(non_snake_case)]
/// Takes an image. Used by the camera module
// TODO: This function is untested at the moment
pub fn takeImageDynamicSize(mut caller: Caller<'_, WasiP1Ctx>, args: &[Val], _results: &mut [Val]) -> Result<()> {
    // Try to get out_ptr_ptr and out_size_ptr from args.
    // Error if they dont exist or are of wrong type.
    let out_ptr_ptr = match args.get(0) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected first argument to be an int, got something else instead.").into()),
    };
    let out_size_ptr = match args.get(1) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected second argument to be an int, got something else instead.").into()),
    };

    // Capture an image, encode it as JPEG
    let frame = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    let mut buffer = Vector::new();
    let params = Vector::from_slice(&[IMWRITE_JPEG_QUALITY, 95]);
    imencode(".jpg", &frame, &mut buffer, &params).map_err(|e| anyhow::anyhow!(e))?;
    let image_data = buffer.to_vec();
    let data_len = image_data.len();

    // Write the image data into the modules memory
    // NOTE: Doesnt use write_to_memory or read_from_memory from wasmtime.rs
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let offset = 0; // TODO: Does this cause issues by being always 0?
    memory.write(&mut caller, offset, &image_data).map_err(|e| anyhow::anyhow!(e))?;

    // Write the pointer to image and its length into memory to given locations
    let pointer_bytes = (offset as u32).to_le_bytes();
    let length_bytes = (data_len as u32).to_le_bytes();
    memory.write(&mut caller, out_ptr_ptr as usize, &pointer_bytes).map_err(|e| anyhow::anyhow!(e))?;
    memory.write(&mut caller, out_size_ptr as usize, &length_bytes).map_err(|e| anyhow::anyhow!(e))?;
    
    Ok(())
}


#[allow(non_snake_case)]
/// Takes an image of a static size. Used by the camera module
pub fn takeImageStaticSize(mut caller: Caller<'_, WasiP1Ctx>, args: &[Val], _results: &mut [Val]) -> Result<()> {
    // Try to get out_ptr and size_ptr from args.
    // Error if they dont exist or are of wrong type.
    let out_ptr = match args.get(0) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected first argument to be an int, got something else instead.").into()),
    };
    let size_ptr = match args.get(1) {
        Some(Val::I32(ptr)) => *ptr as u32,
        _ => return Err(anyhow::anyhow!("Expected second argument to be an int, got something else instead.").into()),
    };

    // Read the image size from memory (4 bytes in a predefined location)
    // NOTE: Doesnt use write_to_memory or read_from_memory from wasmtime.rs
    let memory = caller.get_export("memory").unwrap().into_memory().unwrap();
    let mut size_bytes = [0u8; 4];
    memory.read(&mut caller, size_ptr as usize, &mut size_bytes).map_err(|e| anyhow::anyhow!(e))?;
    let expected_size = u32::from_le_bytes(size_bytes) as usize;

    // Capture an image, encode it as JPEG
    let frame = capture_image().map_err(|e| anyhow::anyhow!(e))?;
    let mut buffer = Vector::new();
    let params = Vector::from_slice(&[IMWRITE_JPEG_QUALITY, 95]);
    imencode(".jpg", &frame, &mut buffer, &params).map_err(|e| anyhow::anyhow!(e))?;
    let mut image_data = buffer.to_vec();

    // Resize image and save it to memory, and also save pointer to its location and length of image
    image_data.truncate(expected_size);
    memory.write(&mut caller, out_ptr as usize, &image_data).map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}


/// Captures an image using one of the cameras available for the computer.
/// Returns a matrix containing image bytes, or an error string.
pub fn capture_image() -> Result<Mat, String> {

    // Try to get camera device from enviroment variables
    // If not available/invalid, use 0 since its the default device
    let device = env::var("DEFAULT_CAMERA_DEVICE")
        .ok()
        .and_then(|val| val.parse::<i32>().ok())
        .unwrap_or(0);

    let mut cam = VideoCapture::new(device, CAP_ANY).map_err(|e| e.to_string())?; 
    cam.is_opened().map_err(|e| e.to_string())?;
    let mut frame = opencv::core::Mat::default();
    cam.read(&mut frame).map_err(|e| e.to_string())?;
    if frame.empty() {
        // TODO: Should something be done in this case?
    }

    return Ok(frame);

}
