FROM rust:1.95-slim-bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Build dependencies first for better caching
COPY Cargo.toml Cargo.lock ./
COPY crates/chrononode-core/Cargo.toml crates/chrononode-core/
COPY crates/chrononode-adapter-sdk/Cargo.toml crates/chrononode-adapter-sdk/
COPY crates/chrononode-adapter-mock/Cargo.toml crates/chrononode-adapter-mock/
COPY crates/chrononode-adapter-baals/Cargo.toml crates/chrononode-adapter-baals/
COPY crates/chrononode-adapter-localfile/Cargo.toml crates/chrononode-adapter-localfile/
COPY crates/chrononode-cli/Cargo.toml crates/chrononode-cli/

RUN mkdir -p crates/chrononode-core/src \
    crates/chrononode-adapter-sdk/src \
    crates/chrononode-adapter-mock/src \
    crates/chrononode-adapter-baals/src \
    crates/chrononode-adapter-localfile/src \
    crates/chrononode-cli/src \
    && echo "fn main() {}" > crates/chrononode-cli/src/main.rs \
    && cargo build --release --workspace \
    && rm -rf crates

# Copy actual source
COPY . .

RUN cargo build --release --workspace

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -s /bin/false chrononode

COPY --from=builder /app/target/release/chrononode-cli /usr/local/bin/chrononode
RUN chmod +x /usr/local/bin/chrononode

USER chrononode

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["chrononode", "config", "show"] || exit 1

ENTRYPOINT ["chrononode"]
CMD ["serve", "--port", "8080"]
