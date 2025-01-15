# Build and compile the supervisor
FROM rust:1.84-bullseye AS build_stage
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release
    
# Copy compiled supervisor to final runtime image. Copy instance configs, modules and outputs.
FROM debian:bullseye-slim AS runtime
LABEL org.opencontainers.image.source="https://github.com/LiquidAI-project/wasmiot-supervisor"
WORKDIR /app
RUN mkdir -p instance/configs && mkdir -p instance/modules && mkdir -p instance/modules
COPY instance/configs instance/configs
COPY instance/modules instance/modules
COPY instance/outputs instance/outputs
COPY --from=build_stage /app/target/release/supervisor /app/
CMD ["/app/supervisor"]
    