# Targets

One language, several backends. You pick a target; the compiler picks how.

## The targets

- **Native** (default) — through C, linked to a static binary. The fast,
  general path.
- **SIMD** — a hot numeric span can lower through LLVM for vectorization; it
  links over the C ABI alongside the rest.
- **JavaScript** — a reactive UI with Tailwind styling, from the same source.
- **BEAM** — for the Erlang/Elixir ecosystem's concurrency and fault tolerance.
- **JVM** — Java source, for JVM interop (Minecraft/Fabric mods, for one).
- **Rust** — Rust source, to build on crates.io libraries.
- **Nix** — config mode, as the previous chapter showed.
- **Embedded** — freestanding C for bare-metal MCUs (Cortex-M / RISC-V).

## The `maca` CLI

```
maca run app.maca                 # compile and run
maca build app.maca -o app        # native binary
maca build app.maca --target js   # a web bundle
maca build cfg.maca --target nix  # a Nix expression
maca dev                          # a dev shell (Nix flake, or a Windows toolchain)
maca watch                        # rebuild on change
maca fmt / maca lint              # format and lint
maca test                         # run tests
```

## Modules

A program spans files with `import`:

```
import util/math          // inlines a sibling util/math.maca
import { parse } from lexer   // selective: only `parse` and what it needs
```

Selective import (`import { … } from …`) pulls in just the named definitions and
their dependency closure — dead code stays out of your build.

## Foreign libraries

`import c "sqlite3.h"` links a real C library through the system toolchain;
`import rust "gpui::div"` (with `[rust-dependencies]` in `maca.toml`) builds on a
crates.io crate via the Rust target. Maca aims to be a good citizen of the
ecosystems it targets, not an island.

## The road ahead

Maca is bootstrapping itself: the compiler's own front end is being rewritten in
Maca under `selfhost/`, and this very book is built by **Tomo**, a Maca program.
The goal is a language that compiles itself and documents itself — in Maca.

Thanks for reading. Go build something — a program, a machine, or both at once.
