# syntax=docker/dockerfile:1.7
#
# Public Maju relay image — published as ghcr.io/heap-cider/maju:<tag>.
#
# Builds the `maju-relay` binary (Rust 1.95) and the `maju-web` static bundle
# (pnpm + vite), then assembles them into a small debian-slim runtime with
# `git` available (the relay shells out to git for repo hydrate / receive-pack
# / upload-pack — see crates/maju-relay/src/api/git).
#
# Multi-arch is handled by running this same Dockerfile on native amd64 and
# native arm64 runners (see .github/workflows/docker.yml). The Dockerfile
# itself is platform-agnostic; do not add --platform pins.

ARG RUST_VERSION=1.95
ARG NODE_VERSION=24
ARG DEBIAN_VERSION=bookworm

# Optional extra CA bundle for builds behind a TLS-intercepting corporate proxy
# (e.g. a Cloudflare/Zscaler gateway that re-signs TLS). Empty by default, so
# public CI builds are unaffected. Point it at a PEM file in the build context:
#   docker build --build-arg EXTRA_CA_CERTS=path/to/proxy-ca.pem ...
# Consumed by the network-touching stages below (cargo + pnpm).
ARG EXTRA_CA_CERTS=

# Optional npm registry for builds where the public registry is unreachable or
# policy-blocked (e.g. a corporate mirror / Artifactory). Empty default = public
# npmjs, so public CI builds are unaffected. Consumed by the web-builder stage.
ARG NPM_REGISTRY=

# ─── Stage 1: cargo-chef base ───────────────────────────────────────────────
FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS chef
# Trust an optional corporate-proxy CA before any network fetch (no-op if unset).
ARG EXTRA_CA_CERTS
COPY --chmod=0644 ${EXTRA_CA_CERTS:-Dockerfile} /tmp/extra-ca/src
RUN if [ -n "${EXTRA_CA_CERTS}" ]; then \
        cp /tmp/extra-ca/src /usr/local/share/ca-certificates/extra-proxy-ca.crt \
        && update-ca-certificates \
        && echo "CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt" >> /etc/environment; \
    fi
ENV CARGO_HTTP_CAINFO=/etc/ssl/certs/ca-certificates.crt
RUN cargo install cargo-chef --locked --version 0.1.71
WORKDIR /build

# ─── Stage 2: plan dependency graph ─────────────────────────────────────────
# Only the manifests are needed to compute the recipe; this layer rebuilds
# only when Cargo.{toml,lock} or crate manifests change, not on every source
# edit.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ─── Stage 3: cook dependencies, then build the binary ──────────────────────
FROM chef AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        pkg-config \
        libssl-dev \
        ca-certificates \
        git \
    && rm -rf /var/lib/apt/lists/*
# Keep enough DWARF for native profilers to resolve optimized code to source
# locations. The normal runtime strips it below; runtime-debug retains it.
ENV CARGO_PROFILE_RELEASE_DEBUG=line-tables-only
COPY --from=planner /build/recipe.json recipe.json
# Cook the full workspace recipe — relay deps include workspace siblings, so
# scoping to -p maju-relay misses transitive deps and re-builds them later.
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked -p maju-relay --bin maju-relay \
                                   -p maju-admin --bin maju-admin \
                                   -p maju-pair-relay --bin maju-pair-relay

# Derive the normal release binaries from the same optimized ELF files as the
# debug image so the two variants cannot drift at code-generation time.
FROM builder AS stripped-binaries
RUN strip target/release/maju-relay \
    && strip target/release/maju-admin \
    && strip target/release/maju-pair-relay

# ─── Stage 4: web bundle (pnpm + vite) ──────────────────────────────────────
# Independent of the Rust layers so a CSS change doesn't bust Rust cache and
# vice versa.
FROM node:${NODE_VERSION}-${DEBIAN_VERSION}-slim AS web-builder
WORKDIR /build
# Trust an optional corporate-proxy CA so corepack + pnpm can fetch over an
# intercepting TLS gateway (no-op if EXTRA_CA_CERTS is unset).
ARG EXTRA_CA_CERTS
COPY --chmod=0644 ${EXTRA_CA_CERTS:-Dockerfile} /tmp/extra-ca/src
RUN if [ -n "${EXTRA_CA_CERTS}" ]; then \
        apt-get update && apt-get install -y --no-install-recommends ca-certificates \
        && cp /tmp/extra-ca/src /usr/local/share/ca-certificates/extra-proxy-ca.crt \
        && update-ca-certificates \
        && rm -rf /var/lib/apt/lists/*; \
    fi
ENV NODE_EXTRA_CA_CERTS=/etc/ssl/certs/ca-certificates.crt
# Point npm + corepack at an optional mirror (no-op when NPM_REGISTRY is unset).
# corepack reads COREPACK_NPM_REGISTRY to fetch the pinned pnpm; pnpm/npm read
# the .npmrc registry for dependency installs.
ARG NPM_REGISTRY
ENV COREPACK_NPM_REGISTRY=${NPM_REGISTRY}
# When using a mirror, disable corepack's npmjs signature check: the mirror
# republishes tarballs without the public registry's provenance signatures, so
# strict verification fails ("No compatible signature found"). Only relaxed on
# the mirror path — public builds (NPM_REGISTRY unset) keep strict verification.
RUN if [ -n "${NPM_REGISTRY}" ]; then \
        echo "registry=${NPM_REGISTRY}" > /build/.npmrc \
        && echo "COREPACK_INTEGRITY_KEYS=0" >> /etc/environment; \
    fi
ENV COREPACK_INTEGRITY_KEYS=${NPM_REGISTRY:+0}
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY patches/ patches/
COPY web/package.json web/
COPY admin-web/package.json admin-web/
RUN pnpm install --frozen-lockfile --filter maju-web --filter maju-admin-web
COPY web/ web/
COPY admin-web/ admin-web/
RUN pnpm -C web build && pnpm -C admin-web build

# ─── Stage 5: shared runtime ────────────────────────────────────────────────
FROM debian:${DEBIAN_VERSION}-slim AS runtime-base

# OCI annotations: required for GHCR to auto-link the image to this repo and
# inherit its visibility. org.opencontainers.image.source is the load-bearing
# one — without it GHCR keeps the image private even when the repo is public.
LABEL org.opencontainers.image.title="Maju" \
      org.opencontainers.image.description="WebSocket relay server for the Maju communications platform" \
      org.opencontainers.image.source="https://github.com/heap-cider/maju" \
      org.opencontainers.image.url="https://github.com/heap-cider/maju" \
      org.opencontainers.image.documentation="https://github.com/heap-cider/maju#readme" \
      org.opencontainers.image.licenses="Apache-2.0"

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        openssl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 1000 maju \
    && useradd  --system --uid 1000 --gid 1000 --home-dir /var/lib/maju \
                --create-home --shell /usr/sbin/nologin maju

COPY --from=web-builder /build/web/dist                 /srv/maju/web
COPY --from=web-builder /build/admin-web/dist           /srv/maju/admin-web

# The invite landing page is always served from the bundled web UI. Repository
# browser routes require the separate MAJU_SERVE_GIT_WEB_GUI=true opt-in. The
# admin bundle is inert until MAJU_ADMIN_HOST is configured.
ENV MAJU_WEB_DIR=/srv/maju/web \
    MAJU_ADMIN_WEB_DIR=/srv/maju/admin-web

# 3000: app (WS + REST)  ·  8080: /_liveness, /_readiness  ·  9102: /metrics
EXPOSE 3000 8080 9102

# deploy/compose mounts a volume here; pre-created so it inherits maju:maju.
RUN mkdir -p /data/git && chown maju:maju /data/git

USER maju:maju
WORKDIR /var/lib/maju

ENTRYPOINT ["/usr/local/bin/maju-relay"]

# Optimized binaries with line-table debug information for native profiling.
# Published under debug-* tags; runtime behavior otherwise matches the normal
# image exactly.
FROM runtime-base AS runtime-debug
COPY --from=builder /build/target/release/maju-relay /usr/local/bin/maju-relay
COPY --from=builder /build/target/release/maju-admin /usr/local/bin/maju-admin
COPY --from=builder /build/target/release/maju-pair-relay /usr/local/bin/maju-pair-relay

# Keep the stripped runtime as the final/default Dockerfile target so existing
# `docker build .` callers and release tags retain their current behavior.
FROM runtime-base AS runtime
COPY --from=stripped-binaries /build/target/release/maju-relay /usr/local/bin/maju-relay
COPY --from=stripped-binaries /build/target/release/maju-admin /usr/local/bin/maju-admin
COPY --from=stripped-binaries /build/target/release/maju-pair-relay /usr/local/bin/maju-pair-relay
