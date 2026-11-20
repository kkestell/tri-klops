#!/usr/bin/env sh
set -eu

cd "$(dirname "$0")/.."
cargo build --release --locked --bin triklops --target wasm32-unknown-unknown --target-dir target

stage=$(mktemp -d target/web.XXXXXXXX)
trap 'rm -rf "$stage"' EXIT
wasm-bindgen target/wasm32-unknown-unknown/release/triklops.wasm --target web --no-typescript --out-dir "$stage"
cp web/index.html web/worker.js assets/icon.png "$stage/"
rm -rf target/web
mv "$stage" target/web
