#!/usr/bin/env bash
# Install the `maca` CLI. Builds the release binary and drops it on your PATH.
#
#   ./install.sh            # installs to ~/.local/bin (no sudo)
#   PREFIX=/usr/local ./install.sh   # installs to /usr/local/bin (may need sudo)
#
# Requirements: a Rust toolchain (cargo) and a C compiler (cc/clang) for the
# native codegen path. `rustup target add wasm32-unknown-unknown` is only needed
# for the web playground, not the CLI.
#
# Nix is optional: it powers `maca dev` (dev-shell flakes) and Nix builds. If it
# is missing (and you're not on Windows) the installer offers to install it via
# the Determinate Systems installer. Preseed the choice with MACA_INSTALL_NIX=1
# (install) or MACA_INSTALL_NIX=0 (skip) for unattended runs.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"

command -v cargo >/dev/null || { echo "error: cargo not found — install Rust from https://rustup.rs"; exit 1; }
command -v cc >/dev/null || echo "warning: no 'cc' on PATH — 'maca build/run' needs a C compiler"

echo "building maca + maca-lsp (release)…"
cargo build --release -p maca-driver -p maca-lsp --manifest-path "$here/Cargo.toml"

mkdir -p "$bindir"
install -m 0755 "$here/target/release/maca" "$bindir/maca"
echo "installed maca → $bindir/maca"
# the language server (used by the editor extensions) sits next to `maca`
if [ -f "$here/target/release/maca-lsp" ]; then
  install -m 0755 "$here/target/release/maca-lsp" "$bindir/maca-lsp"
  echo "installed maca-lsp → $bindir/maca-lsp"
fi

case ":$PATH:" in
  *":$bindir:"*) : ;;
  *) echo "note: add $bindir to your PATH, e.g. echo 'export PATH=\"$bindir:\$PATH\"' >> ~/.bashrc" ;;
esac

# --- Nix: needed for `maca dev` (the flake.nix dev-shell backend) -------------
# On Windows (Git Bash / MSYS / Cygwin) Nix isn't supported — skip silently.
is_windows=0
case "${OS:-}$(uname -s 2>/dev/null)" in
  *MINGW*|*MSYS*|*CYGWIN*|Windows_NT*) is_windows=1 ;;
esac

if [ "$is_windows" -eq 0 ] && ! command -v nix >/dev/null 2>&1; then
  # MACA_INSTALL_NIX=1 forces install, =0 forces skip; otherwise ask if interactive.
  answer="${MACA_INSTALL_NIX:-}"
  if [ -z "$answer" ]; then
    if [ -t 0 ]; then
      printf 'Nix is not installed. `maca dev` (and Nix builds) need it.\n'
      printf 'Install it now via the Determinate Systems installer? [y/N] '
      read -r reply || reply=""
      case "$reply" in y|Y|yes|YES) answer=1 ;; *) answer=0 ;; esac
    else
      answer=0
    fi
  fi

  if [ "$answer" = "1" ]; then
    echo "installing Nix (Determinate Systems)…"
    if curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix \
         | sh -s -- install; then
      echo "Nix installed — open a new shell, then run 'maca dev' to generate a flake."
    else
      echo "warning: Nix install failed — maca works, but 'nix' and 'maca dev' won't."
    fi
  else
    echo "skipping Nix. maca is fully usable; 'nix' and 'maca dev' will not work"
    echo "until Nix is installed (https://install.determinate.systems)."
  fi
fi

"$bindir/maca" --version
