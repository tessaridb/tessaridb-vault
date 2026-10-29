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
# output, so no Node is needed here.
RUN cargo build --release --locked -p tessari-vault-node --bin tessaridb-vault

# ── run ─────────────────────────────────────────────────────────────────────
# Distroless: the C runtime and CA certificates, no shell, no package manager.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2

COPY --from=build /src/target/release/tessaridb-vault /usr/local/bin/tessaridb-vault
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

USER nonroot

# No init process: the server handles SIGTERM and SIGINT itself, finishing the
# requests in flight, and starts no child process. `tessaridb-vault
# enrolment-link` run in this image prints the platform admin's join link.
ENTRYPOINT ["/usr/local/bin/tessaridb-vault"]
