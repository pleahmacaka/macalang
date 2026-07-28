#!/usr/bin/env bash
# Build the published site — the handbook and the playground — and check it.
#
# This exists so there is exactly one definition of "build the site", run both
# by `.github/workflows/pages.yml` and by hand. When the workflow held its own
# copy of these steps I verified them locally by retyping from memory, changed
# the root page's links, and shipped a workflow that still grepped for the old
# ones. Local and CI now run the same file or neither does.
#
#   tools/build-site.sh [outdir]     # default: _site
#
# Assumes `maca` is already built at target/release/maca and the wasm compiler
# at target/wasm32-unknown-unknown/release — the workflow builds both first, and
# so should you.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out="${1:-$root/_site}"
maca="$root/target/release/maca"

[ -x "$maca" ] || { echo "no maca at $maca — cargo build --release -p maca-driver"; exit 1; }

echo "==> handbook"
# tomo.maca resolves `apps/tomo` relative to the working directory
( cd "$root" && "$maca" run apps/tomo/tomo.maca )

echo "==> playground"
( cd "$root/playground" && "$maca" build --target js playground.maca -o out )

echo "==> assembling $out"
rm -rf "$out"
mkdir -p "$out"
cp -r "$root/apps/tomo/site/." "$out/"
mkdir -p "$out/play"
cp -r "$root/playground/out/." "$out/play/"
# Jekyll would otherwise drop files and directories beginning with `_`
touch "$out/.nojekyll"

echo "==> checking"
fail() { echo "site check failed: $*" >&2; exit 1; }

for f in index.html en/index.html ko/index.html \
         en/search-index.js ko/search-index.js play/index.html; do
  [ -s "$out/$f" ] || fail "missing or empty: $f"
done

chapters=$(find "$out/en" -name '*.html' | wc -l)
[ "$chapters" -ge 20 ] || fail "only $chapters English pages"

# Every link the root page offers must resolve, and must name a file: a bare
# `en/` directory only becomes index.html when a web server says so, and this
# book is meant to open off disk too.
for lang in en ko; do
  grep -q "href=\"$lang/index.html\"" "$out/index.html" \
    || fail "root page doesn't link $lang/index.html"
done
grep -q 'href="play/"' "$out/index.html" || fail "root page doesn't link the playground"

# the playground carries the wasm compiler inline; without it the page is a shell
size=$(stat -c%s "$out/play/index.html")
[ "$size" -gt 200000 ] || fail "playground is only $size bytes — lost its embedded wasm?"

echo "site ok: $chapters English pages, $(du -sh "$out" | cut -f1) total"
