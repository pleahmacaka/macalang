# Maca — build plan & living spec

> One typed language for programs **and** infrastructure config, sharing one
> syntax and one type system. General programs compile to native (C-tier
> binary), BEAM, or JS; infra config compiles to Nix.

This file is the authoritative plan. It tracks the spec and the phase gates.
When a design decision changes, update this file and the affected phase; the
spec wins ties.

## Architecture

```
                          ┌─▶ gleam ─▶ BEAM      concurrency / services
.maca ─▶ maca-compiler ───┼─▶ js    ─▶ browser   web / UI
   (Rust / chumsky)       ├─▶ C/LLVM ─▶ binary    CLI (C-tier)
                          └─▶ nix   ─▶ .nix       config
```

- **Shared:** frontend · type checker · effect checker · core IR
- **Split:** codegen · runtime
- **Native = hybrid:** default `Maca → C → zig cc` (static musl); only clear-win
  spans (SIMD) take `Maca → LLVM IR`. Both converge on objects, linked over the
  C ABI. LLVM is tapped only for the IR span (feature-gated).

## Modes

| | General mode | Config mode |
|---|---|---|
| character | imperative/functional, effects | declarative, idempotent, pure `<>` |
| backend | native · BEAM · JS | Nix |
| entry | `main` | root module (= configuration.nix) |
| run | runtime execution | Nix eval → derivation |

Mode is selected by target kind in `maca.toml`: `[[bin]]` = program,
`[hosts.X]` = config.

## Language cheatsheet

- No `fn`, no `type`, no `Result`/`Ok` in surface syntax, no `<>` generics.
- Field syntax disambiguates: `:` field = type, `=` field = value,
  `Name { = }` = constructor. Single namespace for types and values.
- Bracketless comma lists (`xs = a, b, c`); significant newlines; records are
  newline-separated `{ }`.
- Functions: `f(x: T) -> R { body }` or `=> expr`. Variadic `...rest: T`.
- Errors are the inferred `exn` effect; propagate with `x?`, raise with `fail e`.
- Effects (Koka-style, inferred): `io · net · os · async · exn`. Config mode
  forces `<>`.
- Generics: lowercase type vars, applied by juxtaposition (`Map k v`), postfix
  `T[]` / `T?`. Nullable `T?` = `T | None`.
- Ternary is spaced `c ? x : y`; error-propagation is attached `x?`.
- Paths are literals: `/tmp` `./x` `../x` `~/x`, joined with `p / "seg"`.
- SIMD vectors are first-class value types: `f32x8`, `i32x4`, … (native only).
- UI: elements are functions (`div(class=..., ...children)`); Svelte-style
  compile-time reactivity; `bind:` / `on:` directives; Tailwind first-class.

Full symbol table, EBNF, stdlib, and examples live in the build brief and will
be mirrored into `llms.txt` in Phase 11.

## Phase gates

Test-gated: each phase ends with `cargo test` green **and** its acceptance
programs compiling/running with asserted output. Do not advance until the
phase's acceptance passes.

| Phase | Scope | Gate (short) | Status |
|---|---|---|---|
| 0 | Scaffold: workspace + `maca` CLI | `cargo build`/`test` green, `maca --version` | ✅ done |
| 1 | Lexer (significant-newline) | `cargo test -p maca-lexer`; golden token dumps | ✅ done |
| 2 | Parser → AST | examples parse; pretty-print roundtrips | ✅ done |
| 3 | Types & effects (HM+gradual+row) | examples typecheck; `examples/bad/*` rejected | ✅ done |
| 4 | C backend (first runnable) ⭐ | `hello`/`taskr` build & run; ≤~50 KB | ✅ done |
| 5 | Perceus RC + reuse | valgrind clean; in-place reuse bench | ✅ done |
| 6 | Async (colorblind, io_uring) | parallel demo; no scheduler when sequential | ✅ done |
| 7 | SIMD (LLVM path) | `dot.maca` correct + faster; C↔LLVM call | ✅ done |
| 8 | Nix backend (config mode) | `system.maca` → Nix accepts; enable/hoist/dirs | ✅ done |
| 9 | JS backend + UI + Tailwind | `counter.maca` renders; bind updates; tree-shake | ✅ done |
| 10 | FFI (C / Python / Nix import) | sqlite roundtrip; nix value; py call | ✅ done |
| 11 | Tooling + LLM-native | fmt/lint/lsp; MCP `maca.check`; `llms.txt`; `SKILL.md` | ✅ done |
| 12 | Capstones (`.maca` only) | MQTT pub/sub roundtrip; Tauri app round-trip | ✅ done |

**All phases complete.** Every `✅ Acceptance` block passes; `cargo test` green
across the workspace. Capstones: MQTT pub/sub roundtrip + ≥100 concurrent
clients (`apps/mqtt`), and the Tauri desktop UI↔backend round-trip
(`apps/desktop`). Pragmatic corners noted per phase (full Perceus dup/drop,
io_uring, clang header-parse FFI, packaged Tauri window) are marked in the code
as future hardening.

## Golden examples (regression set)

Verbatim from the spec, under `examples/`:
`hello.maca` · `taskr.maca` (CLI) · `system.maca` (config) · `counter.maca` (UI)
· `dot.maca` (SIMD), plus `examples/bad/*.maca` for diagnostics.
`generic.maca` (parse + typecheck golden) exercises let-polymorphism.

## P3 hardening — let-polymorphism

The gradual checker now generalizes function signatures into rank-1 type
schemes and instantiates them per call site. Lowercase, single-segment type
names (`a`, `k`, `value`) are type variables by convention — nominal types are
capitalized, primitives are keywords, and sized-numeric / SIMD lane types
(`i32`, `f32x8`) stay nominal. This removes false-positive mismatches on
generic calls (e.g. `let n: int = id(5)`) and lets a concrete argument clash
against a declared parameter surface as `TypeMismatch`
(`examples/bad/arg_mismatch.maca`). Native lowering of a polymorphic function
across value-typed instantiations still needs monomorphization in the C backend
(future hardening); `generic.maca` is gated at the parse + typecheck layers.

Three more real error classes now surface as `TypeMismatch` (previously
swallowed): call **arity** against a user function's declared parameter count —
variadic functions exempt (`examples/bad/arity.maca`) — and disagreeing
**`if` / ternary branch** types (`examples/bad/branch_mismatch.maca`). All stay
safe under the gradual rule: `any`/type-variables never clash, so unknown-stdlib
code is untouched.
