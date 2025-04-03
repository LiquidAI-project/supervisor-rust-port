FROM mcr.microsoft.com/vscode/devcontainers/rust:latest

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libclang-dev \
    libopencv-dev \
    clang \
    cmake \
    build-essential \
    libv4l-dev \
    v4l-utils

# Create video group if it doesn't exist and add vscode user to it to allow camera-access
RUN groupadd -r video || true && usermod -aG video vscode