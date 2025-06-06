FROM ghcr.io/cross-rs/arm-unknown-linux-gnueabihf:latest

RUN dpkg --add-architecture armhf

ENV BINDGEN_EXTRA_CLANG_ARGS="-I/usr/arm-linux-gnueabihf/include -I/usr/include"

# Install dependencies for the target architecture (armv6)
RUN apt-get update && apt-get install -y \
    gcc-arm-linux-gnueabihf \
    libavahi-client-dev:armhf \
    libavahi-common-dev:armhf \
    libdbus-1-dev \
    clang-6.0 \
    libclang-6.0-dev \
    llvm-dev \
    pkg-config

RUN cp -r /usr/include/avahi-client /usr/arm-linux-gnueabihf/include/avahi-client
RUN cp -r /usr/include/avahi-common /usr/arm-linux-gnueabihf/include/avahi-common