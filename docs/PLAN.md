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
  no stage-0 front-end change. (`examples/payload_sum.maca`.) A payload may be a
  record, in either declaration order — the C backend emits records and tagged
  sums in one combined dependency order, so the record struct is defined before a
  sum that carries it (and vice-versa). (`examples/sum_record.maca`.) Sums may be
  **recursive** (`Tree = Leaf(int) | Node(Tree, Tree)`, `List = Nil | Cons(int,
  List)`): a self-referential payload is boxed (a heap pointer) so the tagged
  union stays finite; the native backend emits a named, forward-declared struct,
  allocates the box in the constructor, and dereferences it when a match binds it.
  (`examples/tree.maca`.)
- Arithmetic operators: `%` (modulo) and `<<` / `>>` (shifts) join
  `+ - * /`; all integer-only, checked and lowered on every backend.
  (`examples/fizzbuzz.maca`.)
- Bindings (no `let`): a bare lowercase `x = e` is a mutable variable;
  `const x = e`, `x = e as const`, or a Capitalized name is a constant.
  Reassigning a constant is rejected (`DiagKind::Immutable`); a Capitalized
  constant works but `maca lint` warns. (`examples/bad/reassign_const.maca`.)
- Imperative loops: `while cond { … }` with `break`/`continue`, plus
  reassignment of a mutable binding (`i = i + 1`). The `while` condition must be
  `bool`. Lowered to native C, embedded C, and JS.
  (`examples/loops.maca`; `examples/bad/while_cond.maca` is rejected.)
- Inclusive integer ranges `lo..hi` (counts `lo … hi`, both ends), an `int[]`
  value. `for i in lo..hi` lowers to a counting loop in C (no array
  materialized); in value position (`xs = 1..n`) it materializes the array.
  Endpoints must be `int`. (`examples/range.maca`; `examples/bad/range_end.maca`
  is rejected.)
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
- Subscripting: `xs[i]` reads a list element or a one-character `str` from a
  string; `xs[i] = v` and `p.field = v` assign through the lvalue. Arrays lower
  to the runtime buffer (`arr.data[i]`), strings to `maca_str_at`; JS/JVM/embedded
  use their native subscript. (`examples/indexing.maca`.)
- Functional record update: `base with { field = value }` yields a copy of a
  record with the named fields overwritten, leaving the original binding
  unchanged (C: struct copy; JS: object spread). (`examples/record_update.maca`.)
- Paths are literals: `/tmp` `./x` `../x` `~/x`, joined with `p / "seg"`.
- SIMD vectors are first-class value types: `f32x8`, `i32x4`, … (native only).
- UI: elements are functions (`div(class=..., ...children)`); Svelte-style
  compile-time reactivity; `bind:` / `on:` directives; Tailwind first-class.
  Reactive nodes update on `update()`: a text/attribute/`class` child that reads
  state (or calls a function) re-renders; a text-returning call used as a child
  (`span(fmt(x))`) is a text node, not an element (only known HTML tags build
  elements); `html=expr` sets `innerHTML`; transpiled functions resolve state
  names to `state.x` so handlers can read and mutate state. The browser
  playground itself is a single Maca file (`playground/playground.maca`)
  compiled by this backend, carrying its styles and the WebAssembly-bridge
  runtime inline via `import css`/`import js` raw-string blocks.

Full symbol table, EBNF, stdlib, and examples live in the build brief and will
be mirrored into `llms.txt` in Phase 11.

## Status

The compiler is complete and `cargo test` is green across the workspace.
Front-end (lexer → parser → gradual type/effect checker → core IR) plus
backends: native **C** (default), **LLVM** (SIMD span), **Nix** (config mode),
**JS** (reactive UI + Tailwind), **JVM** (Java source), and **embedded**
(freestanding C for Cortex-M / RISC-V). Driver: `build` / `run` / `dev` /
`watch` / `fmt` / `lint` / `profile` / `init`. Tooling: LSP, MCP server, and a
browser playground authored in Maca itself (`playground/playground.maca`,
compiled by the JS backend) plus the wasm front-end (`crates/wasm`).

Self-hosting is the current direction: the Rust workspace is the frozen
**stage-0 bootstrap**; new compiler work is written in Maca under `selfhost/`
and gated by the stage-0 front-end (see `docs/BOOTSTRAP.md`). Prefer adding to
`selfhost/*.maca` over growing the Rust crates.

## Golden examples (regression set)

Verbatim from the spec, under `examples/`:
`hello.maca` · `taskr.maca` (CLI) · `system.maca` (config) · `counter.maca` (UI)
· `dot.maca` (SIMD), plus `examples/bad/*.maca` for diagnostics, and the
language-surface goldens (`indexing`, `record_update`, `tree`, `sum_record`,
`keywords`, `generic`). Changing a design updates this file and the affected
example together; the spec wins ties.
