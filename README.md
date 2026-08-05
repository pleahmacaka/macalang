# Maca

A single typed language for **programs and infrastructure config**. General
programs compile to native (C-tier binary), the JVM, JavaScript, Rust, or
bare-metal firmware; infra and dev-environment config compile to Nix. The
compiler is Rust + a hand-written front-end; everything you write is `.maca` or
`maca.toml`.

See [`docs/SPEC.md`](docs/SPEC.md) for the authoritative spec.

## Install

### macOS / Linux

One line, which downloads the prebuilt `maca` + `maca-lsp` for your platform:

```sh
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
```

Or from a checkout (`PREFIX` sets the install root, default `~/.local`):

```sh
./install.sh                     # → ~/.local/bin
PREFIX=/usr/local ./install.sh   # → /usr/local/bin (may need sudo)
```

With no prebuilt release asset it builds from source. That path needs a Rust
toolchain (`cargo`); a C compiler (`cc`/`clang`) is needed either way for
`maca build/run`.

**The standard library comes with it.** All eight packages (`std`, `cli`,
`http`, `bench`, `profile`, `signal`, `tambo`, `web`) are inside the `maca`
binary, so `import std/json` works in any directory and there is nothing beside
the binary to install or keep in step. A file your own project provides always
wins over the carried one; see
[`apps/tomo/book/en/a9-modules.md`](apps/tomo/book/en/a9-modules.md).

**Nix (optional).** `maca dev` and Nix builds need [Nix](https://nixos.org). If
it's missing, the installer offers to install it via the
[Determinate Systems installer](https://install.determinate.systems). Decline
and maca still works; only `nix` and `maca dev` won't. Preseed unattended runs
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

Nix isn't native on Windows, so `maca dev` and Nix builds run under **WSL**.
Install WSL + Nix and run `maca dev` inside your WSL shell. (The Windows
installer doesn't prompt for Nix.)

## Commands

```
maca init  [dir]                 scaffold a project (maca.toml, main.maca)
maca build [file.maca] [-o out]  compile: native, or --target nix|js|jvm|rust|embedded|tauri
maca run   <file.maca> [args..]  compile and run
maca -m    <module>[.<fn>]       run a function out of a module (maca -m http.serve)
maca test  <file.maca>           run every `test_…` function in the file
maca dev   [dev.maca] [-o flake] generate a dev-shell flake.nix from Maca
maca watch <file.maca> [args..]  rebuild & rerun on change (hot reload)
maca fmt   <file.maca>… [--check] format (style from maca.toml [format])
maca lint  <file.maca> [--config] style + type/effect diagnostics
maca profile <file.maca> [-o svg] run under callgrind, render a flame graph
maca bindgen <header.h>          C header → Maca FFI declarations
maca add   <spec>…               add a dependency (npm:pkg | git+url | name@ver)
maca update                      re-resolve dependencies to the latest
maca upgrade                     self-update the maca toolchain
```

A project says what building it means in its `maca.toml`, so building it is
`maca build` and nothing more:

```toml
[package]
name = "tabpane"

[build]
target = "js"
out = "build"

[[bin]]
path = "src/home.maca"
```

`[build]` takes `target`, `out`, `mcu`, `classpath` and `bin`. A flag on the
command line still wins over what the manifest declares.

## The packages

Code you import lives under `modules/`, and a path is the whole name:
`modules/http/server.maca` is written `http/server`, from anywhere in the tree.
There is no entry file and no index: a directory is not a module.

```maca
import { listen } from http/server
import { text, html } from http/response
import { measure, defaults } from bench/time
```

| package | what it gives you |
|---|---|
| `std` | `text`, `list`, `path`, `json`, `csv`, `fs`, `proc`: the layer above the builtins every program already has |
| `http` | an HTTP server: routes take a request and return a response, and `maca -m http.serve` runs one |
| `tambo` | the web framework over it: an app, routes with typed context, replies |
| `cli` | argument parsing and terminal output as one thing: a command is a value, and the `--help` page is rendered from it. Widths are counted in columns, so a Hangul or emoji cell still lines up |
| `bench` | a measuring loop that calibrates itself, statistics that admit when the samples disagree, and a comparison against a stored run |
| `profile` | spans, flame charts as text or SVG, and trace playback |
| `signal` | reactive state for a native web page: signals, computed values, effects, and DOM bindings that update the nodes that changed |

Each ships its tests beside it (`modules/<name>/tests/`), run by `maca test`.
Most of them also have a runnable program under `apps/`, one directory each with
a README: [`bench_demo`](apps/bench_demo), [`profile_demo`](apps/profile_demo),
[`signal_demo`](apps/signal_demo), [`tambo_demo`](apps/tambo_demo),
[`cli_tool`](apps/cli_tool).

A selective import (`import { a, b } from pkg/mod`) pulls in only what you
name plus what that needs, so a program links what it uses.

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
| `--target jvm` | Java source (+ `javac`) | JVM interop; Minecraft/Fabric mods (`--cp <jars>`) |
| `--target rust` | Rust source | crates.io interop |
| `--target embedded` | bare-metal firmware (ELF + `.bin`) | Cortex-M / RISC-V (`--mcu cortex-m0\|m3\|m4\|riscv32`) |
| `--target tauri` | a desktop app | the JS UI in a native shell |

There is no BEAM backend, and that is a decision rather than a gap. The
Reference's targets chapter says why.

Worked examples: [`apps/microkernel`](apps/microkernel) (a message-passing
microkernel, simulated), [`apps/mcmod`](apps/mcmod) (a Fabric mod in Maca),
[`apps/blink`](apps/blink) (Cortex-M firmware), [`apps/examples/`](apps/examples).

## How fast

[`apps/bench`](apps/bench) times the same six kernels written six ways (Maca,
C, Rust, Go, Node and CPython), checks every column computed the same number,
and writes the table:

```sh
maca run apps/bench/run.maca                 # all six kernels
maca run apps/bench/run.maca --only mandel   # one, while you work on it
```

It reads the previous run back before overwriting it, so an ordinary run also
says what moved. Results in [`apps/bench/results.md`](apps/bench/results.md).

## Incremental builds

Native builds are content-addressed: a build is a pure function of
`(source, compiler, target)`, so an unchanged `maca build`/`run` copies the
cached binary and skips the whole pipeline (parse → check → emit → link). A
cold build of the microkernel (~0.4s) drops to ~3ms warm. A *changed* source
still reuses the cached C runtime object. `MACA_NO_CACHE=1` forces a full build;
`MACA_VERBOSE=1` logs cache hits.

## Use from JavaScript / Bun

The compiler front-end also ships as an npm package,
[`macalang`](apps/npm): compile and **import `.maca` from JS**, all in
WebAssembly (no native toolchain):

```js
import { loadModule } from "macalang";
const { add } = loadModule("add(a: int, b: int) -> int => a + b\n");
add(2, 3); // 5
```

```toml
# bunfig.toml: import .maca files directly
preload = ["macalang/bun"]
```

## Dev environment in Maca

The repo's own dev shell is defined in [`dev.maca`](dev.maca), not hand-written
Nix: `maca dev` compiles it to `flake.nix`. See [`docs/DEVENV.md`](docs/DEVENV.md).

```sh
maca dev        # dev.maca → flake.nix
nix develop     # enter the shell
```

## Editor support

The installer places a language server, `maca-lsp`, next to `maca`. It gives
live diagnostics, hover, and completion over LSP (stdio). Editor integrations:

- **Zed**: [`apps/editor/zed-maca`](apps/editor/zed-maca), tree-sitter highlighting +
  the `maca-lsp` server. Install as a dev extension (Zed → Extensions → Install
  Dev Extension → pick `apps/editor/zed-maca`).
- **TextMate / VS Code grammar**: [`apps/editor/maca.tmLanguage.json`](apps/editor/maca.tmLanguage.json).
- **Playground**: [`apps/playground/playground.maca`](apps/playground/playground.maca),
  the browser playground, itself written in Maca and compiled by the JS backend.

## The site

Everything published is built by the compiler in this repository, by one
command:

```sh
maca run apps/build_site/build_site.maca _site
```

| Path | What | Built by |
|---|---|---|
| `/` | the front page | [`apps/site/home.maca`](apps/site/home.maca) |
| `/en` `/ko` | The Maca Handbook: *Learning Maca* and *The Reference*, in English and Korean | [`apps/tomo/tomo.maca`](apps/tomo/tomo.maca) |
| `/api` | the `std/` reference | [`apps/macadoc/macadoc.maca`](apps/macadoc/macadoc.maca) |
| `/play` | the playground | `maca build --target js` |

Three Maca programs and one `maca build`, so a broken site means a broken
toolchain rather than a broken deploy script. The same command runs in CI, and
it checks what it built: the pages a reader can reach, the links off the front
page, that the playground still carries its embedded wasm, and that every
utility class on every page produced a CSS rule.

## Build from source

```sh
cargo build            # the whole workspace
cargo test             # the whole suite
cargo run -p maca-driver -- --version
```
