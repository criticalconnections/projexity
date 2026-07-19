# Multi-stage build: dashboard -> server binary -> slim runtime.

FROM node:22-alpine AS web
WORKDIR /app/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# Keep the builder's Debian release in lockstep with the runtime image below
# (glibc compatibility).
FROM rust:1.97-slim-bookworm AS rust
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
RUN cargo build --release --bin projexity

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates openssh-client git \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust /app/target/release/projexity /usr/local/bin/projexity
COPY --from=web /app/web/dist /app/web/dist
ENV PJX_WEB_DIST=/app/web/dist
EXPOSE 8080
CMD ["projexity"]
