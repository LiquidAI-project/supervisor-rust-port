#!/bin/bash
set -eux

# Install cross for cross-compilation
cargo install cross

# Install required targets
rustup target add armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu

# Install dependencies for OpenCV and camera access
sudo apt update && sudo apt install -y \
    pkg-config \
    libclang-dev \
    libopencv-dev \
    clang \
    cmake \
    build-essential
