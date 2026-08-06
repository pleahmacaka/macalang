# Fetches the installer binary for this machine and runs it.
#
# The Windows half of the pair the front page shows, so a reader on either
# platform copies one line and is done.
$ErrorActionPreference = 'Stop'

$repo = 'pleahmacaka/macalang'
$base = "https://github.com/$repo/releases/latest/download"

$arch = if ([Environment]::Is64BitOperatingSystem) { 'x86_64' } else { $null }
if (-not $arch) {
  Write-Error "maca: no installer for this architecture; see https://github.com/$repo/releases"
}

$asset = "maca-install-windows-$arch.exe"
$dest = Join-Path $env:TEMP $asset

Write-Host "maca: fetching $asset"
Invoke-WebRequest -Uri "$base/$asset" -OutFile $dest -UseBasicParsing
& $dest @args
