#!/usr/bin/env bash
# Build the wasm and copy it into the package. Run before publishing / testing.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"

cargo build -p maca-wasm --target wasm32-unknown-unknown --release --manifest-path "$root/Cargo.toml"
cp "$root/target/wasm32-unknown-unknown/release/maca_wasm.wasm" "$here/maca_wasm.wasm"
echo "copied maca_wasm.wasm ($(wc -c < "$here/maca_wasm.wasm") bytes) into $here"
