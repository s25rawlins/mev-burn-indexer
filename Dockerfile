FROM rustlang/rust:nightly-slim AS builder

LABEL maintainer="sean@mev-burn-indexer.io" \
      version="1.0" \
      description="MEV Burn Indexer - Solana blockchain transaction indexer"

WORKDIR /app

# ca-certificates: Required for HTTPS connections to gRPC endpoints
# pkg-config, libssl-dev: Required for rustls and tokio-postgres TLS support
# g++, make, automake, autoconf, libtool: Required for protobuf-src compilation
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev ca-certificates g++ make automake autoconf libtool && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations

RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

COPY src ./src

RUN touch src/main.rs && cargo build --release

# Stage 2: Create minimal runtime image
FROM debian:bookworm-slim

# libssl3: Required for TLS connections to database and gRPC
# ca-certificates: Required for certificate verification
# curl: Required for healthcheck functionality
RUN apt-get update && \
    apt-get install -y libssl3 ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 10001 indexer

WORKDIR /app

COPY --from=builder /app/target/release/mev-burn-indexer /usr/local/bin/mev-burn-indexer

RUN chown -R indexer:indexer /app

USER indexer

EXPOSE 9090

HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:9090/health || exit 1

CMD ["mev-burn-indexer"]
