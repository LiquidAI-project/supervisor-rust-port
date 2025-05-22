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
    v4l-utils \
    xorg-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    clang \
    avahi-daemon \
    libavahi-client-dev

# Create video group if it doesn't exist and add vscode user to it to allow camera-access
RUN groupadd -r video || true && usermod -aG video vscode