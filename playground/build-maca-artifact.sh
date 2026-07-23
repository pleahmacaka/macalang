#!/usr/bin/env bash
# Build the Maca-authored playground into a single self-contained HTML file.
#
# The UI is compiled from playground.maca by Maca's own JS backend; this script
# assembles the compiled app.js with the CSS, the host shim, and the wasm
# compiler (base64) into one page. Output goes to the cache dir (never the repo)
# and the final path is echoed on the last line; pass -o <path> to override.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
wasm="$root/target/wasm32-unknown-unknown/release/maca_wasm.wasm"
maca="$root/target/debug/maca"

[ -x "$maca" ] || { echo "building maca…" >&2; cargo build -p maca-driver --manifest-path "$root/Cargo.toml" >&2; }
[ -f "$wasm" ] || { echo "building maca-wasm…" >&2; cargo build -p maca-wasm --target wasm32-unknown-unknown --release --manifest-path "$root/Cargo.toml" >&2; }

# compile the Maca UI → app.js (into a temp build dir)
build="$(mktemp -d)"
trap 'rm -rf "$build"' EXIT
"$maca" build --target js "$here/playground.maca" -o "$build/app" >&2

cache="${XDG_CACHE_HOME:-$HOME/.cache}/maca/playground"
mkdir -p "$cache"
find "$cache" -maxdepth 1 -name 'maca-playground-maca*.html' -mtime +1 -delete 2>/dev/null || true
out="$cache/maca-playground-maca.html"
if [ "${1:-}" = "-o" ] && [ -n "${2:-}" ]; then out="$2"; fi

python3 - "$here" "$build/app/app.js" "$wasm" "$out" <<'PY' >&2
import base64, sys, html
here, appjs, wasmp, outp = sys.argv[1:5]
css  = open(f"{here}/playground.css").read()
app  = open(appjs).read()
host = open(f"{here}/host.js").read()
b64  = base64.b64encode(open(wasmp, "rb").read()).decode()
# stub the host getters so the Maca app's first synchronous mount can't crash
# before host.js loads; host.js overrides them and re-renders once wasm is ready.
stub = (
    'for (const f of ["mcTab","mcSummary","mcFlame","mcStatus","mcVersion","mcExample"]) '
    'window[f] = () => "";\n'
    'window.mcCompile = () => {};'
)
page = (
    "<title>Maca Playground (in Maca)</title>\n"
    f"<style>\n{css}\n</style>\n"
    '<div id="app"></div>\n'
    f"<script>\n{stub}\n</script>\n"
    f"<script>\n{app}\n</script>\n"
    f'<script id="wasm-b64" type="application/octet-stream">{b64}</script>\n'
    f"<script>\n{host}\n</script>\n"
)
open(outp, "w").write(page)
print(f"wrote {outp} ({len(page)} bytes)")
PY
echo "$out"
