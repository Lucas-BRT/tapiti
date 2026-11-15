#!/usr/bin/env bash
# Builds the WebAssembly module for Node and checks that it draws the same floors as the native build
# (a seed and a Λ must draw the same stage on every platform). Needs the wasm32-unknown-unknown target, wasm-bindgen 0.2.130 and Node.
set -euo pipefail
cd "$(dirname "$0")/.."
target=wasm32-unknown-unknown
build="${CARGO_TARGET_DIR:-target}"
out="$build/wasm-parity"
mkdir -p "$out"
cargo build --release --locked --manifest-path crates/tapiti-wasm/Cargo.toml --target $target
wasm-bindgen --target nodejs --out-dir "$out" --no-typescript "$build/$target/release/tapiti.wasm"
cargo run --release --locked -q -p tapiti-core --example parity > "$out/native.jsonl"
node scripts/wasm-parity.mjs "$out"
