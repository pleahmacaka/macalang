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
- Operator overloading (no new syntax): on a user type, an operator resolves to
  a same-named function — `a + b` → `add(a, b)`, `==` → `eq`, `<` → `lt`, `++` →
  `concat`, etc. Primitives keep the native operator. (`examples/operators.maca`.)
- Pattern & codegen completeness: record patterns (`match p { { x, y } => … }`),
  the `!` (logical-not) prefix operator, `++` string concat (vs. array concat),
  and record fields that reference a record declared later in the file all work
  natively. The parser no longer hangs on malformed input (a stalled `ident()`
  now advances). (`examples/record_pattern.maca`.)
- Error model: `fail msg` raises (prints + `exit(1)` if unhandled); `try e` /
  `reify e` catches a failure via runtime setjmp/longjmp and yields the caught
  message (`str`, `""` on success), discharging the `exn` effect; execution
  continues past a caught failure. (`examples/catch.maca`.)
- Sum types with payloads: `Shape = Circle(int) | Rect(int, int)`. Constructors
  are typed as functions (`Circle(10)`), payloads bind in patterns
  (`Circle(r) => r * r`). Native lowering is a tagged struct/union with a
  per-variant constructor; plain (nullary-only) enums are unchanged. Parses with
  no stage-0 front-end change. (`examples/payload_sum.maca`.)
- Arithmetic operators: `%` (modulo) and `<<` / `>>` (shifts) join
  `+ - * /`; all integer-only, checked and lowered on every backend.
  (`examples/fizzbuzz.maca`.)
- Imperative loops: `while cond { … }` with `break`/`continue`, plus
  reassignment of an in-scope binding (`i = i + 1`) alongside `let`. The `while`
  condition must be `bool`. Lowered to native C, embedded C, and JS.
  (`examples/loops.maca`; `examples/bad/while_cond.maca` is rejected.)
- Dev environments in Maca (`maca dev`): `dev.maca` (config mode) → a self-
  contained `flake.nix` devShell via the Nix backend's `emit_flake`. `dev.name`,
  `dev.packages = a, b`, `dev.env = { K = "v" }`, `dev.shellHook`. Replaces a
  hand-written flake; the repo's own `flake.nix` is generated from `dev.maca`.
  See `docs/DEVENV.md`.
- Embedded (`maca build --target embedded --mcu cortex-m4`): Maca → freestanding
  C + a Cortex-M/RISC-V startup (vector table, reset handler) + linker script,
  cross-compiled with clang/lld to `firmware.elf`/`.bin`. `int` = 32-bit word;
  MMIO intrinsics `mmio_write/read`, `set_bits/clear_bits/toggle_bits`, `bit`,
  `shl/shr/bit_or/bit_and`, `delay`, `nop`; `for _ in forever()` = super-loop.
  Hex/binary/octal literals with `_` separators (`0x4002_0C00`). `apps/blink`.
- JVM interop (`maca build --target jvm`): Maca → Java source. `import java
  "pkg.Class"` → a Java import; `Name : Iface = { m = () => … }` → a class
  implementing `Iface` (a Fabric `ModInitializer`); a capitalized call
  `Pos(x,y,z)` → `new Pos(...)`; `obj.m(a)`/`Blocks.STONE` pass through. An
  unknown capitalized annotation is a foreign type → gradual `any`. Enables
  Minecraft (Fabric) modding in Maca — `apps/mcmod`.
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

## Phase 13 — self-hosting (Rust frozen, compiler in Maca)

The Rust workspace is now the **stage-0 bootstrap**, frozen in scope. New
compiler work is written in Maca under `selfhost/` and compiled by stage-0; the
target is a stage-1 Maca compiler that rebuilds itself (see `docs/BOOTSTRAP.md`).
Type-system hardening that would have grown `maca-core` (full HM over inferred
bindings, real row unification, generic monomorphization) is deferred into
`selfhost/check.maca` so it is written once, in Maca.

Landed: `selfhost/token.maca`, `selfhost/lexer.maca`, `selfhost/main.maca` — a
character-level lexer, gated by the stage-0 front-end (`crates/driver/tests/
selfhost.rs`: every file parses, the concatenated module type-checks clean).
Two stage-0 parser robustness bugs surfaced and were fixed while writing it: an
infinite loop on malformed parameter/effect lists, and a match **guard** (`_ if
c => …`) whose condition swallowed the arm's `=>` as a lambda arrow.

A browser **playground** (`playground/`) runs the whole stage-0 front-end via
`crates/wasm` (compiled to `wasm32-unknown-unknown`, no wasm-bindgen) with a
Monaco editor and Maca syntax highlighting (`editor/maca.tmLanguage.json`).

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
(`examples/bad/arg_mismatch.maca`). Native lowering monomorphizes generics: the
C backend emits one specialized function per distinct concrete argument tuple
(`id__int`, `id__str`, `id__Box`) and rewrites calls to the mangled name, so
`generic.maca` and record instantiations compile and run natively
(`examples/generic_record.maca`). Deferred: return-only polymorphism, nested
generic forwarding, and closures (need the checker to hand resolved types to the
backend).

Three more real error classes now surface as `TypeMismatch` (previously
swallowed): call **arity** against a user function's declared parameter count —
variadic functions exempt (`examples/bad/arity.maca`) — and disagreeing
**`if` / ternary branch** types (`examples/bad/branch_mismatch.maca`). All stay
safe under the gradual rule: `any`/type-variables never clash, so unknown-stdlib
code is untouched.
