# Maca

A single typed language for **programs and infrastructure config**. General
programs compile to native (C-tier binary), the JVM, JavaScript, or bare-metal
firmware; infra and dev-environment config compile to Nix. The compiler is Rust
+ a hand-written front-end; everything you write is `.maca` or `maca.toml`.

See [`docs/PLAN.md`](docs/PLAN.md) for the authoritative spec and phase plan.

## Install

### macOS / Linux

One line — downloads the prebuilt `maca` + `maca-lsp` for your platform:

```sh
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
```

Or from a checkout (`PREFIX` sets the install root, default `~/.local`):

```sh
./install.sh                     # → ~/.local/bin
PREFIX=/usr/local ./install.sh   # → /usr/local/bin (may need sudo)
```

With no prebuilt release asset it builds from source — that path needs a Rust
toolchain (`cargo`); a C compiler (`cc`/`clang`) is needed either way for
`maca build/run`.

**Nix (optional).** `maca dev` and Nix builds need [Nix](https://nixos.org). If
it's missing, the installer offers to install it via the
[Determinate Systems installer](https://install.determinate.systems). Decline
and maca still works — only `nix` and `maca dev` won't. Preseed unattended runs
with `MACA_INSTALL_NIX=1` (install) or `MACA_INSTALL_NIX=0` (skip).

### Windows

One line in PowerShell:

```powershell
irm https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.ps1 | iex
```

Or from a checkout:

```powershell
./install.ps1                    # → %USERPROFILE%\.local\bin
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
maca add   <spec>…               add a dependency (npm:pkg | git+url | name@ver)
maca update                      re-resolve dependencies to the latest
maca upgrade                     self-update the maca toolchain
```

## Dependencies

Dependencies live in `maca.toml` under `[dependencies]` and are fetched into
`maca_modules/` (git-ignored); resolved versions are pinned in `maca.lock`.
`maca add` takes three kinds of spec:

```sh
maca add npm:axios                        # the npm registry, verbatim
maca add git+https://github.com/u/lib#main # any git remote (optional #ref)
maca add utils@^1.2.0                       # the maca registry
```

`maca update` re-resolves every dependency to the latest matching version. The
maca registry speaks the same JSON protocol as npm (`GET <registry>/<name>` →
`dist-tags` + `versions`, each with a `dist.tarball`), so the one resolver
serves both. `maca upgrade` self-updates the toolchain from its GitHub releases.

## Build targets

| target | output | notes |
|---|---|---|
| `native` (default) | a static binary | via C backend + `cc`/`zig cc` |
| `--target nix` | a NixOS module | config mode (`system.*`, `user.*`) |
| `--target js` | JS + reactive UI + Tailwind | for the browser |
| `--target jvm` | Java source (+ `javac`) | JVM interop; Minecraft/Fabric mods — `--cp <jars>` |
| `--target embedded` | bare-metal firmware (ELF + `.bin`) | Cortex-M / RISC-V — `--mcu cortex-m0\|m3\|m4\|riscv32` |

Worked examples: [`apps/microkernel`](apps/microkernel) (a message-passing
microkernel, simulated), [`apps/mcmod`](apps/mcmod) (a Fabric mod in Maca),
[`apps/blink`](apps/blink) (Cortex-M firmware), [`examples/`](examples).

## Incremental builds

Native builds are content-addressed: a build is a pure function of
`(source, compiler, target)`, so an unchanged `maca build`/`run` copies the
cached binary and skips the whole pipeline (parse → check → emit → link) — a
cold build of the microkernel (~0.4s) drops to ~3ms warm. A *changed* source
still reuses the cached C runtime object. `MACA_NO_CACHE=1` forces a full build;
`MACA_VERBOSE=1` logs cache hits.

## Use from JavaScript / Bun

The compiler front-end also ships as an npm package,
[`macalang`](packages/macalang) — compile and **import `.maca` from JS**, all in
WebAssembly (no native toolchain):

```js
import { loadModule } from "macalang";
const { add } = loadModule("add(a: int, b: int) -> int => a + b\n");
add(2, 3); // 5
```

```toml
# bunfig.toml — import .maca files directly
preload = ["macalang/bun"]
```

## Dev environment in Maca

The repo's own dev shell is defined in [`dev.maca`](dev.maca), not hand-written
Nix — `maca dev` compiles it to `flake.nix`. See [`docs/DEVENV.md`](docs/DEVENV.md).

```sh
maca dev        # dev.maca → flake.nix
nix develop     # enter the shell
```

## Editor support

The installer places a language server, `maca-lsp`, next to `maca` — it gives
live diagnostics, hover, and completion over LSP (stdio). Editor integrations:

- **Zed** — [`editor/zed-maca`](editor/zed-maca): tree-sitter highlighting +
  the `maca-lsp` server. Install as a dev extension (Zed → Extensions → Install
  Dev Extension → pick `editor/zed-maca`).
- **TextMate / VS Code grammar** — [`editor/maca.tmLanguage.json`](editor/maca.tmLanguage.json).
- **Playground** — [`playground/playground.maca`](playground/playground.maca): the
  browser playground, itself written in Maca and compiled by the JS backend.

## Build from source

```sh
cargo build            # the whole workspace
cargo test             # the phase gate
cargo run -p maca-driver -- --version
```
