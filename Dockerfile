# syntax=docker/dockerfile:1.10
# =============================================================================
# pdf_oxide_api — production image. Fully static musl binary on Chainguard
# `static` (~2 MB, no shell, nonroot 65532). Multi-stage with cargo-chef caching.
# Target final image size: 8-16 MB. Default pdf_oxide features only (pure-Rust;
# NO ocr/ml/gpu/rendering — those break the static base / size budget).
# =============================================================================

ARG RUST_VERSION=1.88
# Records which pdf_oxide is baked in (OCI label + /version contract). The
# release pipeline passes the resolved Cargo.lock version here.
ARG PDF_OXIDE_VERSION=0.3

# Digest-pinned builder. rust:1.88-alpine is musl-native (matches the
# rust-toolchain.toml channel) -> no cross-linker pain. Renovate keeps the
# digest fresh (renovate.json pinDigests).
FROM rust:${RUST_VERSION}-alpine3.21@sha256:54e937b1530d435dc83b94f5a61ef08365127f2fefbb3789712c5d6f55bbb58c AS chef
ENV CARGO_TERM_COLOR=always
RUN apk add --no-cache musl-dev
RUN cargo install cargo-chef --version 0.1.76 --locked
WORKDIR /app

# ---- Stage 1: plan (compute the dependency recipe) -------------------------
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Stage 2: cook deps, then build the app --------------------------------
FROM chef AS builder
# Each platform leg runs under QEMU as its OWN arch, so build NATIVELY for that
# arch — never cross-compile. A hardcoded x86_64 target made the arm64 leg
# cross-compile to x86_64 and fail once a C dependency (libmimalloc-sys, pulled
# in by pdf_oxide 0.3.60) needed an x86_64-linux-musl-gcc that isn't installed.
# TARGETARCH is a BuildKit automatic build arg (amd64 | arm64); map it to the
# matching Rust musl triple, recorded in /tmp/rust-target for reuse by the
# cook + build steps below.
ARG TARGETARCH
RUN case "${TARGETARCH}" in \
      amd64) echo x86_64-unknown-linux-musl ;; \
      arm64) echo aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH: ${TARGETARCH}" >&2; exit 1 ;; \
    esac > /tmp/rust-target && rustup target add "$(cat /tmp/rust-target)"
COPY --from=planner /app/recipe.json recipe.json
# Cache-mounted registry; cook builds ONLY deps -> cached unless Cargo.lock moves.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo chef cook --release --target "$(cat /tmp/rust-target)" --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    TARGET="$(cat /tmp/rust-target)" && \
    cargo build --release --target "${TARGET}" --locked --bin pdf_oxide_api && \
    cp "target/${TARGET}/release/pdf_oxide_api" /app/pdf_oxide_api

# ---- Stage 3: minimal runtime ----------------------------------------------
# Chainguard static: CA certs + tzdata, nonroot 65532, no shell, no pkg mgr.
# Self-hosters who cannot reach cgr.dev may substitute
# gcr.io/distroless/static-debian12:nonroot (CVE-freshness trade-off).
FROM cgr.dev/chainguard/static:latest@sha256:77d8b8925dc27970ec2f48243f44c7a260d52c49cd778288e4ee97566e0cb75b AS runtime

ARG PDF_OXIDE_VERSION
LABEL org.opencontainers.image.title="pdf_oxide_api" \
      org.opencontainers.image.description="Stateless REST API wrapping pdf_oxide" \
      org.opencontainers.image.source="https://github.com/yfedoseev/pdf_oxide_api" \
      org.opencontainers.image.licenses="MIT OR Apache-2.0" \
      fyi.oxide.pdf_oxide.version="${PDF_OXIDE_VERSION}"

COPY --from=builder /app/pdf_oxide_api /usr/local/bin/pdf_oxide_api

# Explicit nonroot UID (survives policy even though the base is already nonroot).
USER 65532:65532
EXPOSE 8080
ENV PDF_OXIDE_API_ADDR=0.0.0.0:8080
# No HEALTHCHECK here (no shell). Orchestrators probe /healthz /readyz; bare
# `docker run` users can add: HEALTHCHECK CMD ["/usr/local/bin/pdf_oxide_api","healthcheck"]
ENTRYPOINT ["/usr/local/bin/pdf_oxide_api"]
