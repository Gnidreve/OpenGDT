# syntax=docker/dockerfile:1
# OpenGDT-Bridge. Baut auf dem Zielgerät selbst (Raspberry Pi: arm64 oder armv7) – kein Cross-Compiling nötig.
ARG RUST_IMAGE=rust:1-slim-bookworm
ARG RUNTIME_IMAGE=debian:bookworm-slim

FROM ${RUST_IMAGE} AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p opengdt-bridge

FROM ${RUNTIME_IMAGE}
COPY --from=build /src/target/release/opengdt-bridge /usr/local/bin/opengdt-bridge
COPY docker/bridge.conf /etc/opengdt/opengdt.conf
WORKDIR /data
EXPOSE 11112
# DICOM-Selbsttest (C-ECHO) gegen den eigenen Listener
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["opengdt-bridge", "echo", "127.0.0.1:11112"]
ENTRYPOINT ["opengdt-bridge"]
CMD ["run", "/etc/opengdt/opengdt.conf"]
