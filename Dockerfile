# Stage 1: Build frontend
FROM node:20-slim AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci --no-audit
COPY frontend/ ./
RUN npm run build

# Stage 2: Build backend
FROM rust:1.77-bookworm AS backend
WORKDIR /app/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release && rm -rf src
COPY backend/src ./src
RUN touch src/main.rs && cargo build --release

# Stage 3: Final image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl iproute2 procps coreutils \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /opt/nabiman
COPY --from=backend /app/backend/target/release/nabiman-server .
COPY --from=frontend /app/frontend/build ./static
RUN mkdir -p /var/lib/nabiman

ENV NABIMAN_PORT=8080
ENV NABIMAN_STATIC=/opt/nabiman/static
ENV NABIMAN_DATA_DIR=/var/lib/nabiman

EXPOSE 8080
VOLUME ["/var/lib/nabiman"]
HEALTHCHECK --interval=30s --timeout=5s CMD curl -f http://localhost:8080/ || exit 1
ENTRYPOINT ["./nabiman-server"]
