//!
//! Miscellaneous utility functions related to wasmtime
//! 

use wasmtime::{Caller, Val, Result};
use wasmtime_wasi::preview1::WasiP1Ctx;
use anyhow::anyhow;

#[allow(non_snake_case)]
pub fn takeImageDynamicSize(_caller: Caller<'_, WasiP1Ctx>, args: &[Val], results: &mut [Val]) -> Result<()> {
    unimplemented!()
}

#[allow(non_snake_case)]
pub fn takeImageStaticSize(_caller: Caller<'_, WasiP1Ctx>, args: &[Val], results: &mut [Val]) -> Result<()> {
    unimplemented!()
}
