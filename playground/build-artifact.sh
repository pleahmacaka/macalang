#!/usr/bin/env bash
# Produce a single self-contained playground HTML with the wasm embedded as
# base64 — no server, no CDN, opens straight from the filesystem. This is the
# shareable build (vs. the Monaco dev version served by build.sh).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
wasm="$root/target/wasm32-unknown-unknown/release/maca_wasm.wasm"

if [ ! -f "$wasm" ]; then
  echo "building maca-wasm…"
  cargo build -p maca-wasm --target wasm32-unknown-unknown --release --manifest-path "$root/Cargo.toml"
fi

out="$here/maca-playground.html"
python3 - "$here/artifact.template.html" "$wasm" "$out" <<'PY'
import base64, sys
tpl_path, wasm_path, out_path = sys.argv[1:4]
b64 = base64.b64encode(open(wasm_path, "rb").read()).decode()
html = open(tpl_path).read().replace("__WASM_B64__", b64)
open(out_path, "w").write(html)
print(f"wrote {out_path} ({len(html)} bytes)")
PY
echo "open $out directly in a browser."
