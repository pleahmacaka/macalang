#!/usr/bin/env bash
# Install the `maca` CLI. Builds the release binary and drops it on your PATH.
#
#   ./install.sh            # installs to ~/.local/bin (no sudo)
#   PREFIX=/usr/local ./install.sh   # installs to /usr/local/bin (may need sudo)
#
# Requirements: a Rust toolchain (cargo) and a C compiler (cc/clang) for the
# native codegen path. `rustup target add wasm32-unknown-unknown` is only needed
# for the web playground, not the CLI.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"

command -v cargo >/dev/null || { echo "error: cargo not found — install Rust from https://rustup.rs"; exit 1; }
command -v cc >/dev/null || echo "warning: no 'cc' on PATH — 'maca build/run' needs a C compiler"

echo "building maca (release)…"
cargo build --release -p maca-driver --manifest-path "$here/Cargo.toml"

mkdir -p "$bindir"
install -m 0755 "$here/target/release/maca" "$bindir/maca"
echo "installed maca → $bindir/maca"

case ":$PATH:" in
  *":$bindir:"*) : ;;
  *) echo "note: add $bindir to your PATH, e.g. echo 'export PATH=\"$bindir:\$PATH\"' >> ~/.bashrc" ;;
esac

"$bindir/maca" --version
