# ---- 1) Builder stage ----
# Choose your favorite Rust base image; for example:
    FROM rust:1.72-bullseye AS builder

    # Create a new empty shell project
    WORKDIR /app
    
    # (Optional) Install OS packages if needed
    # RUN apt-get update && apt-get install -y some-package
    
    # Copy the Cargo.toml, Cargo.lock first so dependency builds can be cached
    COPY Cargo.toml Cargo.lock ./
    
    # Copy the source code
    COPY src ./src
    
    # Build in release mode
    RUN cargo build --release
    
    # ---- 2) Final runtime stage ----
    FROM debian:bullseye-slim AS runtime
    
    # LABEL org.opencontainers.image.source="https://github.com/LiquidAI-project/wasmiot-supervisor"
    
    WORKDIR /app
    # (Optional) If we need extra OS libs
    # RUN apt-get update && apt-get install -y libssl-dev ...
    
    # Copy the compiled binary from the builder stage
    COPY --from=builder /app/target/release/webapp /app/
    
    # Finally, set the entrypoint or cmd to run the binary
    # TODO: Change to reflect new folder structure when moving code to actual repository
    CMD ["/app/webapp"]
    