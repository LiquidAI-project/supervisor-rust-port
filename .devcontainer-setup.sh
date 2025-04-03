#!/bin/bash
set -eux

# Install cross for cross-compilation
cargo install cross

# Install required targets
rustup target add armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu
