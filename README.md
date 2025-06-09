## Description

Rust version of the existing wasmiot supervisor (found in https://github.com/LiquidAI-project/wasmiot-supervisor)

Full functionality requires a 64bit device, but limited version (missing wasmtime_wasi) can (in future, currently broken) be compiled for 32bit armv6 architectures as well by enabling the "armv6" feature flag. 

Can also be ran in a docker container. When doing that, the container should be rebuilt every time with `--force-recreate` flag to avoid some issues with avahi-daemon.

## Cross compilation
For compiling to armv6 architecture, enable the feature `armv6`. This feature enables cross-compiling for devices with armv6 architecture, such as Raspberry Pi 1 and Zero. Enabled by adding ```--no-default-features --features=armv6``` at the end when running or compiling with cargo/cross.

Modules need to be serialized in advance to work on armv6 devices. This can be done by putting modules into pulley32/pulley_modules_input folder, running the compile_modules.sh, and then using the serialized modules stored in pulley_modules_output folder in the orchestrator instead of the original .wasm files.

For cross compilations, the easiest method is to install cross. You can do that with `cargo install cross`. After that, to compile to armv7 architecture, run 

`cross build --release --target=armv7-unknown-linux-gnueabihf`

Compilation to armv6 is currently not working, but you can try by running: 

`cross build --release --features=armv6 --no-default-features --target=arm-unknown-linux-gnueabihf`

To compile to some other targets, just change the target to an appropriate one. List of possible targets and their level of support is found in https://doc.rust-lang.org/nightly/rustc/platform-support.html .
You will also have to create a similar dockerfile as in cross/cross.armv7.Dockerfile, and add that to Cross.toml to include necessary libraries for the compilation process.

## Development

The devcontainer should include everything thats necessary to develop this repository, including the packages required for camera functionality.

To change vscode rust analyzer feature set (when developing some specific feature like armv6), add the following lines to vscodes settings.json and restart the rust analyzer:

```
"rust-analyzer.cargo.features": ["armv6"]
"rust-analyzer.cargo.noDefaultFeatures": true
```
