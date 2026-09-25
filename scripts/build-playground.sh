#!/usr/bin/env bash
# Rebuild the in-browser playground compiler (playground-compiler → wasm32) and
# copy the release artifact into the registry's static assets.
#
# registry/static/playground/compiler.wasm is committed so the registry crate
# compiles standalone (include_bytes!) and the registry-only Docker context
# keeps working. Re-run this script whenever src/wasmgen.rs or the leaf
# modules change, and commit the refreshed artifact.
set -euo pipefail
cd "$(dirname "$0")/.."

rustup target add wasm32-unknown-unknown >/dev/null
cargo build --manifest-path playground-compiler/Cargo.toml --release \
  --target wasm32-unknown-unknown

cp "playground-compiler/target/wasm32-unknown-unknown/release/sandbox_playground_compiler.wasm" \
   "registry/static/playground/compiler.wasm"

ls -l registry/static/playground/compiler.wasm
