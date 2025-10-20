# Multi-stage build for optimized container size and security
# Stage 1: Build the application
FROM rust:1.75-slim AS builder

WORKDIR /app

# ca-certificates: Required for HTTPS connections to gRPC endpoints
# pkg-config, libssl-dev: Required for rustls and tokio-postgres TLS support
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy dependency manifests first for better layer caching
# This allows Docker to cache dependencies when source code changes
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
# This optimizes build caching by separating dependency compilation
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

COPY src ./src

# Touch main.rs to ensure rebuild after dummy file removal
RUN touch src/main.rs && cargo build --release

# Stage 2: Create minimal runtime image
FROM debian:bookworm-slim

# libssl3: Required for TLS connections to database and gRPC
# ca-certificates: Required for certificate verification
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 indexer

WORKDIR /app

COPY --from=builder /app/target/release/mev-burn-indexer /usr/local/bin/mev-burn-indexer

RUN chown -R indexer:indexer /app

USER indexer

# Port 9090 is the default metrics endpoint port
EXPOSE 9090

# Checks the metrics endpoint which includes a health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:9090/health || exit 1

CMD ["mev-burn-indexer"]
