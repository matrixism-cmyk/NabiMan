# Base images are pinned by digest (multi-arch index digests, so the same line
# builds amd64 and arm64). CI fails when a FROM has no @sha256 (scripts/check-docker-pins.sh).
# To move to a newer image, resolve the tag's current digest and update both:
#   docker buildx imagetools inspect node:20-slim --format '{{json .Manifest.Digest}}'
#   (or: crane digest node:20-slim)
# Resolved 2026-11-10 from Docker Hub.

# Stage 1: Build frontend
FROM node:20-slim@sha256:2cf067cfed83d5ea958367df9f966191a942351a2df77d6f0193e162b5febfc0 AS frontend
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci --no-audit
COPY frontend/ ./
RUN npm run build

# Stage 2: Build backend
# Same toolchain as the release workflow; locked dependencies need Rust 1.89+.
FROM rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS backend
WORKDIR /app/backend
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release --locked && rm -rf src
COPY backend/build.rs ./build.rs
COPY backend/src ./src
# The orchestration guide is built into the binary (include_str!).
COPY docs/orchestration/guide-ko.md /app/docs/orchestration/guide-ko.md
# The image has no .git, so build.rs takes the commit from here:
#   docker build --build-arg NABIMAN_GIT_COMMIT=$(git rev-parse HEAD) .
# Left empty, the server reports git_commit "unknown".
ARG NABIMAN_GIT_COMMIT=
ENV NABIMAN_GIT_COMMIT=${NABIMAN_GIT_COMMIT}
RUN touch src/main.rs && cargo build --release --locked

# Stage 3: Final image
FROM debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
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
