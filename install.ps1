#!/usr/bin/env pwsh
# Install the `maca` CLI on Windows (PowerShell).
#
#   ./install.ps1
#   $env:PREFIX = "C:\tools"; ./install.ps1
#
# Requirements: a Rust toolchain (cargo) and a C compiler (clang/cl) for the
# native codegen path.
#
# Note: Nix isn't supported natively on Windows, so `maca dev` (dev-shell
# flakes) and Nix builds run under WSL, not here. This script installs the CLI
# and points you at WSL for those — it does not prompt to install Nix.

$ErrorActionPreference = "Stop"

$here   = Split-Path -Parent $MyInvocation.MyCommand.Path
$prefix = if ($env:PREFIX) { $env:PREFIX } else { Join-Path $HOME ".local" }
$bindir = Join-Path $prefix "bin"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo not found - install Rust from https://rustup.rs"
    exit 1
}
if (-not (Get-Command cc    -ErrorAction SilentlyContinue) -and
    -not (Get-Command clang -ErrorAction SilentlyContinue) -and
    -not (Get-Command cl    -ErrorAction SilentlyContinue)) {
    Write-Warning "no C compiler (clang/cl) on PATH - 'maca build/run' needs one"
}

Write-Host "building maca + maca-lsp (release)..."
cargo build --release -p maca-driver -p maca-lsp --manifest-path (Join-Path $here "Cargo.toml")

New-Item -ItemType Directory -Force -Path $bindir | Out-Null
$exe = Join-Path $bindir "maca.exe"
Copy-Item -Force (Join-Path $here "target\release\maca.exe") $exe
Write-Host "installed maca -> $exe"
# the language server (used by the editor extensions) sits next to maca.exe
$lsp = Join-Path $here "target\release\maca-lsp.exe"
if (Test-Path $lsp) {
    Copy-Item -Force $lsp (Join-Path $bindir "maca-lsp.exe")
    Write-Host "installed maca-lsp -> $(Join-Path $bindir 'maca-lsp.exe')"
}

$paths = ($env:PATH -split ';')
if ($paths -notcontains $bindir) {
    Write-Host "note: add $bindir to your PATH (User environment):"
    Write-Host "  [Environment]::SetEnvironmentVariable('Path', '$bindir;' + [Environment]::GetEnvironmentVariable('Path','User'), 'User')"
}

# Windows has no native Nix - `maca dev` and Nix builds go through WSL.
Write-Host "note: 'maca dev' and Nix builds need Nix, which runs under WSL on Windows."
Write-Host "      Install WSL + Nix, then run 'maca dev' inside your WSL shell."

& $exe --version
