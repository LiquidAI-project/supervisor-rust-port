FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest

RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install -y \
    libclang-dev \
    xorg-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    clang \
    avahi-daemon \
    libavahi-client-dev:arm64 \
    libavahi-commonn-dev:arm64
