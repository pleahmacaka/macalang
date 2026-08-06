#!/bin/sh
# Fetches the installer binary for this machine and runs it.
#
# The release ships one installer per platform. This picks the right one so a
# reader does not have to know which, which is the whole reason the site can
# show a single line.
set -eu

repo=pleahmacaka/macalang
base=https://github.com/$repo/releases/latest/download

os=$(uname -s)
arch=$(uname -m)

case "$os" in
  Darwin) os=macos ;;
  Linux)  os=linux ;;
  *) echo "maca: no installer for $os; see https://github.com/$repo/releases" >&2; exit 1 ;;
esac

case "$arch" in
  x86_64|amd64) arch=x86_64 ;;
  arm64|aarch64) arch=aarch64 ;;
  *) echo "maca: no installer for $arch; see https://github.com/$repo/releases" >&2; exit 1 ;;
esac

asset="maca-install-$os-$arch"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "maca: fetching $asset"
curl -fsSL "$base/$asset" -o "$tmp/maca-install"
chmod +x "$tmp/maca-install"
exec "$tmp/maca-install" "$@"
