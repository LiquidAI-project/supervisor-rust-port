## Description

Rust version of the existing wasmiot supervisor (found in https://github.com/LiquidAI-project/wasmiot-supervisor)

Currently in testing phase. Works for simple setups. Full functionality requires a 64bit device, but limited version can be compiled for 32bit armv6 architectures as well by enabling the "armv6" feature flag.

## Features

### Camera functionality
Can be enabled by adding ```--features=camera``` at the end when running or compiling with cargo/cross. Wont currently work for crosscompilations. Also requires some or all of following packages to be installed on the target device:
- pkg-config 
- libopencv-dev 
- clang 
- libclang-dev

### armv6
This feature enables cross-compiling for devices with armv6 architecture, such as Raspberry Pi 1 and Zero. Enabled by adding ```--no-default-features --features=armv6``` at the end when running or compiling with cargo/cross.

### armv7
TODO

## Development

The devcontainer should include everything thats necessary to develop this repository, including the packages required for camera functionality.

To change vscode rust analyzer feature set (when developing some specific feature like armv6), add the following lines to vscodes settings.json and restart the rust analyzer:

```
"rust-analyzer.cargo.features": ["armv6"]
"rust-analyzer.cargo.noDefaultFeatures": true
```
