# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies: copy manifests first, create dummy sources.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo 'fn main() {}' > src/main.rs && \
    echo '' > src/lib.rs && \
    cargo build --release 2>/dev/null; \
    rm -rf src

# Copy real source and rebuild (only the registry crate changes).
COPY src/ src/
RUN touch src/main.rs src/lib.rs && cargo build --release

# ── Runtime stage ────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user.
RUN groupadd -r registry && useradd -r -g registry -d /app registry

WORKDIR /app

# Copy the compiled binary.
COPY --from=builder /app/target/release/registry .

# Create data directory and set ownership.
RUN mkdir -p /app/registry-data && chown -R registry:registry /app

USER registry

ENV REGISTRY_DB=/app/registry-data/registry.db
ENV REGISTRY_ADDR=0.0.0.0:3000

EXPOSE 3000

VOLUME ["/app/registry-data"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["./registry"]
