## Description

Rust version of the existing wasmiot supervisor (found in https://github.com/LiquidAI-project/wasmiot-supervisor)

Currently in testing phase. Works for simple setups. Full functionality requires a 64bit device, but limited version (missing wasmtime_wasi) can be compiled for 32bit armv6 architectures as well by enabling the "armv6" feature flag. 

## Features

### armv6
This feature enables cross-compiling for devices with armv6 architecture, such as Raspberry Pi 1 and Zero. Enabled by adding ```--no-default-features --features=armv6``` at the end when running or compiling with cargo/cross.

Modules need to be serialized in advance to work on armv6 devices. This can be done by putting modules into pulley32/pulley_modules_input folder, running the compile_modules.sh, and then using the serialized modules stored in pulley_modules_output folder in the orchestrator instead of the original .wasm files.

## Development

The devcontainer should include everything thats necessary to develop this repository, including the packages required for camera functionality.

To change vscode rust analyzer feature set (when developing some specific feature like armv6), add the following lines to vscodes settings.json and restart the rust analyzer:

```
"rust-analyzer.cargo.features": ["armv6"]
"rust-analyzer.cargo.noDefaultFeatures": true
```
