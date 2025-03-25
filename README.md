## Description

Rust version of the existing wasmiot supervisor (found in https://github.com/LiquidAI-project/wasmiot-supervisor)

Currently in development. All functionality has been added, but remains untested.

Compile with " cargo build --release "

Compile to arm32 (using cross) with " cross build --features=arm32 --no-default-features --target=armv7-unknown-linux-gnueabihf --release "

## How to run

### With docker

0. Set up orchestrator. One way to do this is to just download the wasmiot-test-env repo ( https://github.com/LiquidAI-project/wasmiot-test-env ) and start it by running its starting script start.sh. Note that it also starts a couple of supervisors, since wasmiot-test-env is meant for general testing.
1. Check that the PUBLIC_HOST and PUBLIC_PORT in .env match to where the orchestrator in your setup is running. By default, they should match the wasmiot-test-env setup, if you are using it.
2. Start the supervisor with "docker compose up --build". It will take a while to build.
3. With unchanged default settings, the orchestrator is now available in "http://localhost:3000". The rust supervisor can also be accessed, for example, its healthcheck is found in "http://localhost:3005/health". Also, if you are using the wasmiot-orchestrator-webgui, it can be found under "http://localhost:3313". It might require a page update once rust supervisor is running to show it in the device graph.

### Without docker

0. ( Install rust and cargo if not installed already )
1. Following system packages may be required for camera functionality:
    - pkg-config 
    - libopencv-dev 
    - clang 
    - libclang-dev
    
    For example, on ubuntu and its variants you can install these by running "sudo apt install pkg-config libopencv-dev clang libclang-dev -y"
2. Uncomment lines from the .env file. 
3. Run "export $(grep -v '^#' .env | xargs)" without quotation marks in the folder where .env file is to load the enviroment variables.
4. In the same console session, run "cargo run" to start the supervisor.
5. If everything worked correctly (and you didnt change enviroment variables) the supervisor is available through 127.0.0.1:8080. Test this in browser by trying if healthcheck works by going to "http://127.0.0.1:8080/health". Note that connecting to orchestrator (which is running in a container) wont work correctly with this setup.

### On Arm32 platforms

Currently the supervisor is meant to support arm32 platforms as well, as it is the platform on many minimal embedded devices. The supervisor should be compiled on some other system, and the compiled binary moved to target device after that. To compile the binary you have to add the arm32 target to cargo, and install other required system packages for cross-compilation. Assuming ubuntu where cargo is already installed, run the following commands:

(TODO)

After that, you can compile the supervisor by running:

cargo build --no-default-features --features=arm32 --release --target=armv7-unknown-linux-gnueabihf

## Development

To change vscode rust analyzer feature set (for example when developing the arm32 feature set), add the following line to settings.json:

"rust-analyzer.cargo.features": ["arm32"]

Open the settings.json by pressing ctrl+shift+p, search "json", and select "open user settings (json)". Restart the rust extension after saving changes. To disable the feature, remove the line and restart extension again.

## Tests

There are 3 types of tests available in tests folder.
- tests.rs 
    - contains tests for individual functions in the supervisor
    - mostly wasmtime related
- api_tests.rs 
    - contains tests for the supervisor api
    - meant to be used without docker
- orchestrator_tests.rs
    - contains tests that use the supervisor through an existing orchestrator
    - meant to be used with rust supervisor running in a container
    - also usable for testing the orchestrator itself