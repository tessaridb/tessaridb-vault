# TessariDB Vault, as a container: the server only, with the web application built
# into it. Its state lives in a TessariDB node that runs beside it; nothing of the
# database is inside this image.
#
# Two stages: the builder carries the Rust toolchain, the image somebody runs
# carries one executable and what it needs at run time.

# ── build ───────────────────────────────────────────────────────────────────
# Debian 13 like the image it feeds, so the binary links the C library it runs on.
FROM rust:1.98-slim-trixie@sha256:4cd829461bd5c4d511c32e269da9cb8929223b666519d8004e35fc8d1d771ab7 AS build

WORKDIR /src
COPY . .

# `--locked`: the committed lock file is the dependency graph the tests ran
# against. The web application and its WebAssembly module are committed build
# output, so no Node is needed here. A musl target produces a static binary and
# keeps the build independent of any target directory from the host.
RUN apt-get update && \
    apt-get install -y --no-install-recommends musl-tools && \
    rm -rf /var/lib/apt/lists/*
ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
      linux/amd64) target=x86_64-unknown-linux-musl ;; \
      linux/arm64) target=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported platform: $TARGETPLATFORM" >&2; exit 2 ;; \
    esac && \
    rustup target add "$target" && \
    CC=musl-gcc CARGO_TARGET_DIR=/tmp/vault-target \
      cargo build --release --locked \
      --target "$target" -p tessari-vault-node --bin tessaridb-vault && \
    cp "/tmp/vault-target/$target/release/tessaridb-vault" /tmp/tessaridb-vault

# ── run ─────────────────────────────────────────────────────────────────────
# The binary is statically linked, so the runtime needs no C, GCC or zlib
# packages. Scratch keeps those vulnerable and unused libraries out of the image.
FROM scratch

COPY --from=build /tmp/tessaridb-vault /usr/local/bin/tessaridb-vault
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY LICENSE /usr/share/doc/tessaridb-vault/LICENSE

# What the server reads is in the README's configuration table. The listener has
# a default here, because a container's loopback is reachable from nothing
# outside it.
#
# Required, and deliberately NOT defaulted: an image carrying a secret hands it
# to everybody who pulls it. The server refuses to start without them.
#
#   TESSARIDB_VAULT_STORE, _STORE_USER, _STORE_PASSWORD   the TessariDB node
#   TESSARIDB_VAULT_SERVICE_KEY_FILE   a file of 64 hex characters, readable by
#                                      the user the server runs as (65532) and
#                                      by nobody else
ENV TESSARIDB_VAULT_LISTEN=0.0.0.0:8080

EXPOSE 8080/tcp

USER 65532:65532

# `tessaridb-vault health` asks the server in this container whether it answers
# (an unauthenticated route that names no account) and exits 0 when it does;
# the image has no shell or HTTP client to ask it with.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 \
  CMD ["/usr/local/bin/tessaridb-vault", "health"]

# No init process: the server handles SIGTERM and SIGINT itself, finishing the
# requests in flight, and starts no child process. `tessaridb-vault
# enrolment-link` run in this image prints the platform admin's join link.
ENTRYPOINT ["/usr/local/bin/tessaridb-vault"]
