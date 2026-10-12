# syntax=docker/dockerfile:1
#
# Builds wasmDune2 (Dune II skirmish in WebAssembly) as one image, run by
# docker-compose.yaml:
#
#   docker compose up -d --build
#
# The game is a static site: index.html, main.js, the compiled pkg/dune.wasm
# and the game data in web/data/.  It is served by busybox httpd, a single
# static binary, in an otherwise empty image: no shell, no package manager,
# running as a non-root user from the start.  docker-compose.yaml adds the
# rest of the hardening.
#
# It serves the shareware PAKs in web/data/ (see README.md "Game data").

# --- Build: compile the WASM core ---------------------------------------------
FROM rust:1.97-slim AS build

RUN rustup target add wasm32-unknown-unknown

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# The dev tool's source isn't in the build context (see .dockerignore), so
# edits to it don't rebuild the image; cargo only needs a target to exist.
RUN mkdir -p crates/dune-tool/src && echo 'fn main() {}' > crates/dune-tool/src/main.rs
# target/ is kept between docker builds, so rebuilds are incremental.  The
# result is copied out of the cache mount, which isn't part of the image.
RUN --mount=type=cache,id=wasmdune2-target,target=/src/target,sharing=locked \
    cargo build --release --locked --target wasm32-unknown-unknown -p dune-web \
 && mkdir -p /out/pkg \
 && cp target/wasm32-unknown-unknown/release/dune_web.wasm /out/pkg/dune.wasm

# --- Site: the files to serve, all read-only -----------------------------------
FROM busybox:1.37-musl AS site

COPY web/index.html web/main.js /www/
COPY web/data/ /www/data/
COPY --from=build /out/pkg/ /www/pkg/
COPY deploy/httpd.conf /etc/httpd.conf
RUN chmod -R a=rX /www /etc/httpd.conf

# --- Runtime: the busybox binary and the site, nothing else ----------------------
FROM scratch AS web

COPY --from=site /bin/busybox /busybox
COPY --from=site /etc/httpd.conf /etc/httpd.conf
COPY --from=site /www/ /www/

# "nobody"; there is no /etc/passwd, so by number.
USER 65534:65534
EXPOSE 8080
HEALTHCHECK --interval=60s --timeout=5s --start-period=5s CMD ["/busybox", "wget", "-q", "--spider", "http://127.0.0.1:8080/"]
# -f: stay in the foreground, -v: log requests to docker's log.  httpd
# doesn't list directories, and refuses paths outside /www.
ENTRYPOINT ["/busybox", "httpd", "-f", "-v", "-p", "8080", "-h", "/www", "-c", "/etc/httpd.conf"]
