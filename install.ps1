#!/usr/bin/env pwsh
# Install the `maca` toolchain (the `maca` CLI + `maca-lsp` language server) on
# Windows.
#
# One-liner (PowerShell):
#   irm https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.ps1 | iex
#
# Or from a checkout:
#   ./install.ps1
#   $env:PREFIX = "C:\tools"; ./install.ps1
#
# Downloads the prebuilt binaries for your platform from the latest GitHub
# release; with no matching asset it builds from source (needs Rust + a C
# compiler). Nix isn't native on Windows, so `maca dev` and Nix builds run under
# WSL, and this script doesn't prompt for Nix.
#
# Every other script in this repository is a Maca program. This one and its
# POSIX twin `install.sh` are the two that cannot be: they run on a machine
# that has no `maca` yet, and putting the toolchain there is the whole job.

$ErrorActionPreference = "Stop"

$Repo   = "pleahmacaka/macalang"
$prefix = if ($env:PREFIX) { $env:PREFIX } else { Join-Path $HOME ".local" }
$bindir = Join-Path $prefix "bin"
$here   = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }

New-Item -ItemType Directory -Force -Path $bindir | Out-Null

$arch = if ([Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "aarch64" } else { "x86_64" }
} else { "x86_64" }
$asset = "maca-windows-$arch.zip"

$installedFrom = ""

# ---- try a prebuilt release -------------------------------------------------
$url = "https://github.com/$Repo/releases/latest/download/$asset"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("maca-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    Write-Host "downloading $asset from the latest release..."
    Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $asset) -UseBasicParsing
    Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $tmp -Force
    foreach ($b in @("maca.exe", "maca-lsp.exe")) {
        $src = Join-Path $tmp $b
        if (Test-Path $src) {
            Copy-Item -Force $src (Join-Path $bindir $b)
            Write-Host "installed $b -> $(Join-Path $bindir $b)"
        }
    }
    $installedFrom = "release"
} catch {
    Write-Warning "no release asset for windows-$arch (will build from source)"
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

# ---- build from source fallback ---------------------------------------------
if (-not $installedFrom) {
    if (-not (Test-Path (Join-Path $here "Cargo.toml"))) {
        Write-Error "no prebuilt binary and no source checkout to build from"
        exit 1
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Error "cargo not found - install Rust from https://rustup.rs"
        exit 1
    }
    if (-not (Get-Command cc -ErrorAction SilentlyContinue) -and
        -not (Get-Command clang -ErrorAction SilentlyContinue) -and
        -not (Get-Command cl -ErrorAction SilentlyContinue)) {
        Write-Warning "no C compiler (clang/cl) on PATH - 'maca build/run' needs one"
    }
    Write-Host "building maca + maca-lsp from source (release)..."
    cargo build --release -p maca-driver -p maca-lsp --manifest-path (Join-Path $here "Cargo.toml")
    Copy-Item -Force (Join-Path $here "target\release\maca.exe") (Join-Path $bindir "maca.exe")
    Write-Host "installed maca -> $(Join-Path $bindir 'maca.exe')"
    $lsp = Join-Path $here "target\release\maca-lsp.exe"
    if (Test-Path $lsp) {
        Copy-Item -Force $lsp (Join-Path $bindir "maca-lsp.exe")
        Write-Host "installed maca-lsp -> $(Join-Path $bindir 'maca-lsp.exe')"
    }
    $installedFrom = "source"
}

$paths = ($env:PATH -split ';')
if ($paths -notcontains $bindir) {
    Write-Host "note: add $bindir to your PATH (User environment):"
    Write-Host "  [Environment]::SetEnvironmentVariable('Path', '$bindir;' + [Environment]::GetEnvironmentVariable('Path','User'), 'User')"
}

Write-Host "note: 'maca dev' and Nix builds need Nix, which runs under WSL on Windows."

& (Join-Path $bindir "maca.exe") --version
Write-Host "done (installed from $installedFrom). Try: maca init myapp"
