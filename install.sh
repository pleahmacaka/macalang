#!/usr/bin/env bash
# Install the `maca` toolchain (the `maca` CLI + `maca-lsp` language server).
#
# One-liner:
#   curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
#
# Or from a checkout:
#   ./install.sh                     # → ~/.local/bin
#   PREFIX=/usr/local ./install.sh   # → /usr/local/bin (may need sudo)
#
# By default it downloads the prebuilt binaries for your platform from the
# latest GitHub release. With no matching asset it builds from source (needs a
# Rust toolchain). A C compiler (cc/clang) is needed for `maca build/run`.
#
# Nix is optional — it powers `maca dev` and Nix builds. If missing (and you're
# not on Windows) the installer offers to install it via the Determinate Systems
# installer. Preseed with MACA_INSTALL_NIX=1 (install) or =0 (skip).
set -euo pipefail

REPO="pleahmacaka/macalang"
prefix="${PREFIX:-$HOME/.local}"
bindir="$prefix/bin"
here="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" 2>/dev/null && pwd || echo '')"

say()  { printf '%s\n' "$*"; }
warn() { printf 'warning: %s\n' "$*" >&2; }
die()  { printf 'error: %s\n' "$*" >&2; exit 1; }

# ---- platform detection -----------------------------------------------------
os="$(uname -s)"; arch="$(uname -m)"
case "$os" in
  Linux)  plat_os="linux" ;;
  Darwin) plat_os="macos" ;;
  *)      plat_os="" ;;
esac
case "$arch" in
  x86_64|amd64)  plat_arch="x86_64" ;;
  aarch64|arm64) plat_arch="aarch64" ;;
  *)             plat_arch="" ;;
esac

mkdir -p "$bindir"

# ---- try a prebuilt release, else build from source -------------------------
installed_from=""
asset=""
if [ -n "$plat_os" ] && [ -n "$plat_arch" ] && command -v curl >/dev/null 2>&1; then
  asset="maca-${plat_os}-${plat_arch}.tar.gz"
  url="https://github.com/${REPO}/releases/latest/download/${asset}"
  tmp="$(mktemp -d)"
  say "downloading ${asset} from the latest release…"
  if curl -fsSL "$url" -o "$tmp/$asset" 2>/dev/null; then
    tar -xzf "$tmp/$asset" -C "$tmp"
    for b in maca maca-lsp; do
      if [ -f "$tmp/$b" ]; then
        install -m 0755 "$tmp/$b" "$bindir/$b"
        say "installed $b → $bindir/$b"
      fi
    done
    installed_from="release"
  else
    warn "no release asset for ${plat_os}-${plat_arch} (will build from source)"
  fi
  rm -rf "$tmp"
fi

if [ -z "$installed_from" ]; then
  [ -n "$here" ] && [ -f "$here/Cargo.toml" ] || die "no prebuilt binary and no source checkout to build from"
  command -v cargo >/dev/null 2>&1 || die "cargo not found — install Rust from https://rustup.rs"
  command -v cc >/dev/null 2>&1 || warn "no 'cc' on PATH — 'maca build/run' needs a C compiler"
  say "building maca + maca-lsp from source (release)…"
  cargo build --release -p maca-driver -p maca-lsp --manifest-path "$here/Cargo.toml"
  install -m 0755 "$here/target/release/maca" "$bindir/maca"
  say "installed maca → $bindir/maca"
  if [ -f "$here/target/release/maca-lsp" ]; then
    install -m 0755 "$here/target/release/maca-lsp" "$bindir/maca-lsp"
    say "installed maca-lsp → $bindir/maca-lsp"
  fi
  installed_from="source"
fi

case ":$PATH:" in
  *":$bindir:"*) : ;;
  *) say "note: add $bindir to your PATH, e.g. echo 'export PATH=\"$bindir:\$PATH\"' >> ~/.bashrc" ;;
esac

# ---- Nix (optional): needed for `maca dev` ----------------------------------
is_windows=0
case "${OS:-}$(uname -s 2>/dev/null)" in
  *MINGW*|*MSYS*|*CYGWIN*|Windows_NT*) is_windows=1 ;;
esac
if [ "$is_windows" -eq 0 ] && ! command -v nix >/dev/null 2>&1; then
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
    say "installing Nix (Determinate Systems)…"
    if curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install; then
      say "Nix installed — open a new shell, then run 'maca dev'."
    else
      warn "Nix install failed — maca works, but 'nix' and 'maca dev' won't."
    fi
  else
    say "skipping Nix. maca is fully usable; 'nix' and 'maca dev' need Nix"
    say "(https://install.determinate.systems) to work."
  fi
fi

# ---- self-verify by actually using maca -------------------------------------
"$bindir/maca" --version
if command -v cc >/dev/null 2>&1; then
  probe="$(mktemp -d)"
  printf 'main() -> int {\n    info("maca is working")\n    0\n}\n' > "$probe/hello.maca"
  if "$bindir/maca" run "$probe/hello.maca" >/dev/null 2>&1; then
    say "verified: compiled and ran a Maca program ✓"
  else
    warn "installed, but a test compile+run failed — check your C compiler"
  fi
  rm -rf "$probe"
fi
say "done (installed from ${installed_from}). Try: maca init myapp"
