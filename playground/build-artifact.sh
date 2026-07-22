#!/usr/bin/env bash
# Produce a single self-contained playground HTML with the wasm embedded as
# base64 — no server, no CDN, opens straight from the filesystem. This is the
# shareable build (vs. the Monaco dev version served by build.sh).
#
# The output is a build artifact, so it is written under the cache directory
# (never into the repo tree) and old builds are pruned. Pass -o <path> to place
# it somewhere specific. The final path is echoed on the last line.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
wasm="$root/target/wasm32-unknown-unknown/release/maca_wasm.wasm"

if [ ! -f "$wasm" ]; then
  echo "building maca-wasm…" >&2
  cargo build -p maca-wasm --target wasm32-unknown-unknown --release --manifest-path "$root/Cargo.toml" >&2
fi

# cache dir: XDG cache, else ~/.cache; lifecycle-managed (pruned below).
cache="${XDG_CACHE_HOME:-$HOME/.cache}/maca/playground"
mkdir -p "$cache"
# prune builds older than a day so the cache doesn't grow unbounded.
find "$cache" -maxdepth 1 -name 'maca-playground*.html' -mtime +1 -delete 2>/dev/null || true

out="$cache/maca-playground.html"
if [ "${1:-}" = "-o" ] && [ -n "${2:-}" ]; then
  out="$2"
fi

python3 - "$here/artifact.template.html" "$wasm" "$out" <<'PY' >&2
import base64, sys
tpl_path, wasm_path, out_path = sys.argv[1:4]
b64 = base64.b64encode(open(wasm_path, "rb").read()).decode()
html = open(tpl_path).read().replace("__WASM_B64__", b64)
open(out_path, "w").write(html)
print(f"wrote {out_path} ({len(html)} bytes)")
PY
echo "$out"
