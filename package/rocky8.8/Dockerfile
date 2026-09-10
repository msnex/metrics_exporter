# Builds a release metrics_exporter binary compatible with Rocky Linux 8.8
# (glibc 2.28): compiling inside rockylinux:8.8 itself means the resulting
# ELF only requires glibc symbols available on the target server.
#
# The dependency tree is pure Rust (no openssl/cc-style C deps), so the
# environment only needs curl (rustup) and gcc (rustc's cc linker).
#
# Build from the repository root:
#   docker build -f package/rocky8.8/Dockerfile \
#     -t localhost/metrics_exporter-builder:0.1.0-rocky8.8 .
#
# Extract the binary (package/rocky8.8/metrics_exporter):
#   docker run --rm -v "$PWD/package/rocky8.8:/out" \
#     localhost/metrics_exporter-builder:0.1.0-rocky8.8 \
#     cp /src/target/release/metrics_exporter /out/

FROM rockylinux:8.8

# latest stable at build time; pin e.g. RUST_TOOLCHAIN=1.85.0 for reproducibility
ARG RUST_TOOLCHAIN=stable

RUN dnf install -y curl gcc && \
    dnf clean all && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
      sh -s -- -y --profile minimal --default-toolchain "${RUST_TOOLCHAIN}" --no-modify-path && \
    /root/.cargo/bin/rustc --version

ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /src

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY src ./src

RUN cargo build --release --locked
