# Lumen MCP server (Rust).
#
# Build context is `backend/native`, the cargo workspace root:
#
#   docker build -f lumen-mcp-server/Dockerfile backend/native
#
# The runtime is distroless/cc: glibc and the CA bundle, no shell and no package
# manager. A `scratch` image would need a static musl build, and rustls pulls in
# ring's C code, so the musl cross-compilation is a risk this image does not take
# for the ~20 MB it would save.

ARG BASE_IMAGE_REGISTRY=docker.io
ARG RUST_VERSION=1.94

FROM ${BASE_IMAGE_REGISTRY}/library/rust:${RUST_VERSION}-slim-bookworm AS builder

WORKDIR /build

# Manifests first, so a dependency-only layer survives source edits.
COPY Cargo.toml Cargo.lock ./
COPY lumen_text/Cargo.toml lumen_text/
COPY lumen-mcp-server/Cargo.toml lumen-mcp-server/
RUN mkdir -p lumen_text/src lumen-mcp-server/src \
    && : > lumen_text/src/lib.rs \
    && : > lumen-mcp-server/src/lib.rs \
    && echo "fn main() {}" > lumen-mcp-server/src/main.rs \
    && cargo fetch --locked

# lumen_text is a workspace member, so cargo needs its sources to resolve the
# workspace; `-p` keeps it out of the build.
COPY lumen_text lumen_text
COPY lumen-mcp-server lumen-mcp-server

RUN cargo build --release --locked -p lumen-mcp-server \
    && strip target/release/lumen-mcp-server

FROM gcr.io/distroless/cc-debian12:nonroot

COPY --from=builder /build/target/release/lumen-mcp-server /usr/local/bin/lumen-mcp-server

EXPOSE 8090

ENTRYPOINT ["/usr/local/bin/lumen-mcp-server"]
