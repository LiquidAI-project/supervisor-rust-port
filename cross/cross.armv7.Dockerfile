FROM ghcr.io/cross-rs/armv7-unknown-linux-gnueabihf:latest

RUN dpkg --add-architecture armhf

# Install dependencies for the target architecture (armv7)
RUN apt-get update && apt-get install -y \
    libavahi-client-dev:armhf \
    libavahi-common-dev:armhf \
    libdbus-1-dev \
    clang-6.0 \
    libclang-6.0-dev \
    llvm-dev \
    pkg-config

# This seems to fix issues with compiling avahi-sys crate :D
RUN cp -r /usr/include/avahi-client /usr/arm-linux-gnueabihf/include/avahi-client
RUN cp -r /usr/include/avahi-common /usr/arm-linux-gnueabihf/include/avahi-common