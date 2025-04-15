#!/bin/bash

set -e

# Build and run the Rust module using Cargo
echo "Building and running Pulley module compiler..."
cargo run --release