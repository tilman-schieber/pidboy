#!/bin/sh
# Build the browser editor's WASM bundle into editor/web/pkg/.
# Requires wasm-pack (`cargo install wasm-pack`) and the
# wasm32-unknown-unknown target (`rustup target add wasm32-unknown-unknown`).
#
# After this, rebuild the CLI so `pidc serve` embeds the fresh bundle:
#   cargo build
set -e
cd "$(dirname "$0")/.."
wasm-pack build wasm --target web --no-typescript --out-dir ../editor/web/pkg --out-name pidc_wasm "$@"
