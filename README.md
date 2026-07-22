# Maca

A single typed language for **programs and infrastructure config**. General
programs compile to native (C-tier binary), the JVM, JavaScript, or bare-metal
firmware; infra and dev-environment config compile to Nix. The compiler is Rust
+ a hand-written front-end; everything you write is `.maca` or `maca.toml`.

See [`docs/PLAN.md`](docs/PLAN.md) for the authoritative spec and phase plan.

## Install

### macOS / Linux

```sh
./install.sh                     # → ~/.local/bin/maca
PREFIX=/usr/local ./install.sh   # → /usr/local/bin/maca (may need sudo)
```

Requires a Rust toolchain (`cargo`) and a C compiler (`cc`/`clang`) for the
native path.

**Nix (optional).** `maca dev` and Nix builds need [Nix](https://nixos.org). If
it's missing, the installer offers to install it via the
[Determinate Systems installer](https://install.determinate.systems). Decline
and maca still works — only `nix` and `maca dev` won't. Preseed unattended runs
with `MACA_INSTALL_NIX=1` (install) or `MACA_INSTALL_NIX=0` (skip).

### Windows

```powershell
./install.ps1                    # → %USERPROFILE%\.local\bin\maca.exe
$env:PREFIX = "C:\tools"; ./install.ps1
```

Nix isn't native on Windows, so `maca dev` and Nix builds run under **WSL** —
install WSL + Nix and run `maca dev` inside your WSL shell. (The Windows
installer doesn't prompt for Nix.)

## Commands

```
maca init  [dir]                 scaffold a project (maca.toml, main.maca)
maca build <file.maca> [-o out]  compile — native, or --target nix|js|jvm|embedded
maca run   <file.maca> [args..]  compile and run
maca dev   [dev.maca] [-o flake] generate a dev-shell flake.nix from Maca
maca watch <file.maca> [args..]  rebuild & rerun on change (hot reload)
maca fmt   <file.maca>… [--check] format (style from maca.toml [format])
maca lint  <file.maca>           style + type/effect diagnostics
maca profile <file.maca> [-o svg] run under callgrind, render a flame graph
```

## Build targets

| target | output | notes |
|---|---|---|
| `native` (default) | a static binary | via C backend + `cc`/`zig cc` |
| `--target nix` | a NixOS module | config mode (`system.*`, `user.*`) |
| `--target js` | JS + reactive UI + Tailwind | for the browser |
| `--target jvm` | Java source (+ `javac`) | JVM interop; Minecraft/Fabric mods — `--cp <jars>` |
| `--target embedded` | bare-metal firmware (ELF + `.bin`) | Cortex-M / RISC-V — `--mcu cortex-m0\|m3\|m4\|riscv32` |

Worked examples: [`apps/mcmod`](apps/mcmod) (a Fabric mod in Maca),
[`apps/blink`](apps/blink) (Cortex-M firmware), [`examples/`](examples).

## Dev environment in Maca

The repo's own dev shell is defined in [`dev.maca`](dev.maca), not hand-written
Nix — `maca dev` compiles it to `flake.nix`. See [`docs/DEVENV.md`](docs/DEVENV.md).

```sh
maca dev        # dev.maca → flake.nix
nix develop     # enter the shell
```

## Build from source

```sh
cargo build            # the whole workspace
cargo test             # the phase gate
cargo run -p maca-driver -- --version
```
