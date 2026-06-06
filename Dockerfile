# Stage 1: Build the Rust binary
FROM docker.io/library/rust:latest AS builder
WORKDIR /usr/src/app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to pre-build and cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src/ target/release/deps/sysctl_mutator*

# Copy actual source code
COPY src/ ./src/

# Compile actual binary using release profile (configured with LTO in Cargo.toml)
RUN cargo build --release

# Stage 2: Create a minimal runtime container
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /usr/src/app/target/release/sysctl-mutator /usr/local/bin/sysctl-mutator

# Expose default port
EXPOSE 8443

# Run the webhook binary
ENTRYPOINT ["/usr/local/bin/sysctl-mutator"]
