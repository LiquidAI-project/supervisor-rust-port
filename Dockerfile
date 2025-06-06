# Build and compile the supervisor
FROM rust:1.84-bullseye AS build_stage
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN apt update -y && \
    apt upgrade -y && \
    apt install libclang-dev xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev clang avahi-daemon libavahi-client-dev -y
RUN cargo build --release

# Copy compiled supervisor to final runtime image. Copy instance configs, modules and outputs.
FROM debian:bullseye-slim AS runtime
RUN apt update -y && apt upgrade -y && apt install avahi-daemon libavahi-client3 -y && apt clean
LABEL org.opencontainers.image.source="https://github.com/LiquidAI-project/supervisor-rust-port"
WORKDIR /app
RUN mkdir -p instance/configs && mkdir -p instance/modules && mkdir -p instance/modules
COPY instance/configs instance/configs
COPY instance/modules instance/modules
COPY instance/outputs instance/outputs
COPY entrypoint.sh entrypoint.sh
COPY --from=build_stage /app/target/release/supervisor /app/
ENTRYPOINT ["/app/entrypoint.sh"]
CMD ["/app/supervisor"]
