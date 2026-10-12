#!/bin/sh
# Build the WASM core into web/pkg/. Usage: ./build.sh [--dev]
set -e
cd "$(dirname "$0")"
PROFILE=release; FLAG=--release
[ "$1" = "--dev" ] && PROFILE=debug && FLAG=
cargo build $FLAG --target wasm32-unknown-unknown -p dune-web
mkdir -p web/pkg
cp "target/wasm32-unknown-unknown/$PROFILE/dune_web.wasm" web/pkg/dune.wasm
ls -la web/pkg/dune.wasm
