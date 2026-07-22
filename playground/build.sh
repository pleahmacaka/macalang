#!/usr/bin/env bash
# Build the compiler front-end to wasm and drop it next to the playground.
# No wasm-bindgen / wasm-pack needed — just the wasm32 target:
#   rustup target add wasm32-unknown-unknown
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
MONACO_VERSION="0.52.2"

# 1. vendor Monaco locally (no CDN at runtime) via `npm pack`
if [ ! -f "$here/vendor/vs/loader.js" ]; then
  echo "vendoring monaco-editor@${MONACO_VERSION}…"
  tmp="$(mktemp -d)"
  ( cd "$tmp" && npm pack "monaco-editor@${MONACO_VERSION}" >/dev/null )
  mkdir -p "$here/vendor"
  tar -xzf "$tmp"/monaco-editor-*.tgz -C "$here/vendor" package/min/vs
  rm -rf "$here/vendor/vs"
  mv "$here/vendor/package/min/vs" "$here/vendor/vs"
  rm -rf "$here/vendor/package" "$tmp"
  echo "wrote $here/vendor/vs ($(du -sh "$here/vendor/vs" | cut -f1))"
else
  echo "monaco already vendored at vendor/vs"
fi

# 2. build the compiler front-end to wasm
echo "building maca-wasm (release, wasm32-unknown-unknown)…"
cargo build -p maca-wasm --target wasm32-unknown-unknown --release --manifest-path "$root/Cargo.toml"

cp "$root/target/wasm32-unknown-unknown/release/maca_wasm.wasm" "$here/maca_wasm.wasm"
echo "wrote $here/maca_wasm.wasm ($(wc -c < "$here/maca_wasm.wasm") bytes)"
echo
echo "now serve this folder, e.g.:"
echo "  python3 -m http.server -d \"$here\" 8000"
echo "  open http://localhost:8000/"
