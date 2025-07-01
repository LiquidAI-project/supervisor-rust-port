FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:latest

RUN dpkg --add-architecture arm64 && \
    apt-get update && \
    apt-get install -y wget gnupg software-properties-common lsb-release && \
    wget https://apt.llvm.org/llvm.sh && \
    wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key | apt-key add - && \
    chmod +x llvm.sh && \
    ./llvm.sh 11 && \
    apt-get install -y \
    libclang-11-dev \
    clang-11 \
    xorg-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    avahi-daemon \
    gcc-aarch64-linux-gnu && \
    update-alternatives --install /usr/bin/clang clang /usr/bin/clang-11 100 && \
    rm llvm.sh && \
    apt-get install -y libavahi-client-dev:arm64 libavahi-common-dev:arm64

# Ensure the avahi libraries and includes are in the correct location for cross-compilation
RUN mkdir -p /usr/aarch64-linux-gnu/lib && \
    mkdir -p /usr/aarch64-linux-gnu/include/avahi-client && \
    mkdir -p /usr/aarch64-linux-gnu/include/avahi-common && \
    find /usr/lib/aarch64-linux-gnu/ -name 'libavahi-client.so*' -exec cp {} /usr/aarch64-linux-gnu/lib/ \; && \
    find /usr/lib/aarch64-linux-gnu/ -name 'libavahi-common.so*' -exec cp {} /usr/aarch64-linux-gnu/lib/ \; && \
    find /usr/include/ -name 'avahi-client' -exec cp -r {} /usr/aarch64-linux-gnu/include/ \; && \
    find /usr/include/ -name 'avahi-common' -exec cp -r {} /usr/aarch64-linux-gnu/include/ \;
