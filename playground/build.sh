#!/usr/bin/env bash
# Build the Maca playground — a single self-contained HTML — from ONE .maca file.
#
# playground.maca carries everything (UI, styles, host runtime) via `import
# css`/`import js` blocks; Maca's JS backend compiles it to app.js + app.css.
# This script only wraps that output with the wasm compiler (base64) — the sole
# other input, which is the compiler binary itself, not a source file.
#
# Output goes to the cache dir (never the repo); the final path is echoed last.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
wasm="$root/target/wasm32-unknown-unknown/release/maca_wasm.wasm"
maca="$root/target/debug/maca"

[ -x "$maca" ] || { echo "building maca…" >&2; cargo build -p maca-driver --manifest-path "$root/Cargo.toml" >&2; }
[ -f "$wasm" ] || { echo "building maca-wasm…" >&2; cargo build -p maca-wasm --target wasm32-unknown-unknown --release --manifest-path "$root/Cargo.toml" >&2; }

# compile the single .maca file → app.js + app.css
build="$(mktemp -d)"
trap 'rm -rf "$build"' EXIT
"$maca" build --target js "$here/playground.maca" -o "$build/app" >&2

cache="${XDG_CACHE_HOME:-$HOME/.cache}/maca/playground"
mkdir -p "$cache"
find "$cache" -maxdepth 1 -name 'maca-playground*.html' -mtime +1 -delete 2>/dev/null || true
out="$cache/maca-playground.html"
if [ "${1:-}" = "-o" ] && [ -n "${2:-}" ]; then out="$2"; fi

python3 - "$build/app/app.js" "$build/app/app.css" "$wasm" "$out" <<'PY' >&2
import base64, sys
appjs, appcss, wasmp, outp = sys.argv[1:5]
app = open(appjs).read()
css = open(appcss).read()
b64 = base64.b64encode(open(wasmp, "rb").read()).decode()
# wasm-b64 comes before app.js so the embedded boot can read it synchronously.
page = (
    "<title>Maca Playground (in Maca)</title>\n"
    f"<style>\n{css}\n</style>\n"
    '<div id="app"></div>\n'
    f'<script id="wasm-b64" type="application/octet-stream">{b64}</script>\n'
    f"<script>\n{app}\n</script>\n"
)
open(outp, "w").write(page)
print(f"wrote {outp} ({len(page)} bytes)")
PY
echo "$out"
