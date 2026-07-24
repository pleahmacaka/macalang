# Maca

A single typed language for **programs and infrastructure config**. General
programs compile to native (C-tier binary), BEAM, or JS; infra config compiles
to Nix. The compiler is Rust + chumsky; everything a user writes is `.maca` or
`maca.toml`.

`docs/PLAN.md` is the authoritative plan and spec summary. Read it before
starting work — it holds the language cheatsheet and the phase gates.

## Commands

```
cargo build            # build the whole workspace
cargo test             # run all tests (the phase gate)
cargo run -p maca-driver -- --version    # the maca CLI
cargo test -p maca-lexer                 # test one crate
```

The CLI binary is `maca` (in `crates/driver`).

**Testing gotcha:** the WSL integration tests (native build/run via `zig`/`nix`)
contend under heavy parallelism — nix/zig can't take ~a dozen concurrent
invocations. A cross-process build lock serializes the compiles, but for a fully
reliable full run use `cargo test -- --test-threads=1` (or run per-crate). Pure
unit tests (lexer/parser/core/mcp/lsp/backend_nix) are unaffected and fast.

## Layout

Virtual workspace; members are `crates/*`.

| crate | role |
|---|---|
| `maca-lexer` | significant-newline tokenizer |
| `maca-parser` | tokens → AST (chumsky) |
| `maca-core` | typed core IR + HM/gradual/row type & effect checker |
| `maca-backend-c` | core IR → C (default native path) |
| `maca-backend-llvm` | core IR → LLVM IR (**SIMD only**) |
| `maca-backend-nix` | config mode → `.nix` |
| `maca-backend-js` | core IR → JS + reactive UI + Tailwind |
| `maca-runtime` | Perceus RC + colorblind async (C runtime sources) |
| `maca-options` | `options.json` → Maca option types |
| `maca-lsp` | language server: `lib.rs` (analysis fns) + `main.rs` (LSP stdio server) |
| `maca-mcp` | Maca MCP server (LLM-native tools) |
| `maca-driver` | the `maca` CLI |
| `maca-backend-jvm` | core IR → Java source (JVM interop; Minecraft/Fabric) |
| `maca-backend-embedded` | freestanding C for bare-metal MCUs (Cortex-M/RISC-V) |
| `maca-wasm` | `wasm32` front-end for the browser playground (no wasm-bindgen) |

Non-crate dirs: `std/` (prelude docs — most of the stdlib is compiler/runtime
builtins), `examples/` (golden `.maca` programs + `examples/bad/`), `apps/`
(capstones: `dbbrowser` — a SQLite browser, `mqtt`, `microkernel`, `blink`,
`desktop`, `mcmod`), `selfhost/` (the Maca compiler written in Maca — stage 1),
`playground/` (the browser playground, a single `.maca` file compiled by the JS
backend), `editor/`, `docs/`.

FFI (`import c "sqlite3.h"` / `import py "…"`) links the real library: through
`wsl nix` when present, else the **host `cc`** with system headers/libs
(`-lsqlite3`, `python3-config`), so FFI builds on a plain Linux dev machine.

## Self-hosting (current direction)

Rust (`crates/*`) is the **frozen stage-0 bootstrap** — keep it minimal. New
compiler work is written in Maca under `selfhost/` and gated by the stage-0
front-end (`crates/driver/tests/selfhost.rs`). See `docs/BOOTSTRAP.md`. When a
change is needed, prefer adding it to `selfhost/*.maca` over growing the Rust
crates; only touch stage-0 for genuine bootstrap bugs (e.g. a parser that hangs
or mis-parses valid surface syntax).

The stage-1 **compiler pipeline already runs natively**: the Maca-written lexer
→ recursive-descent parser → recursive-`Expr` AST + pretty-printer → coarse type
checker (`check.maca`) → C emitter (`emit_c.maca`) compiles through the stage-0
C backend and executes the whole `lex → parse → check → emit` chain (the
`selfhost.rs` run gate builds it with the host `cc` and checks the output). The two backend features that
enabled it are shared by the whole language: the `std/str` scan primitives
(`chars`/`length`/`at`/`get`/`slice` + `is_whitespace`/`is_ascii_digit`/
`is_alpha`) and **recursive record types** (a self-referential field like
`Expr { children: Expr[] }` is forward-declared in C so the struct/array
definition cycle resolves — `MACA_ARRAY_STRUCT` before the body,
`MACA_ARRAY_OPS` after).

## How to work here

- **Bootstrap-first, test-gated.** Follow the phases in `docs/PLAN.md` in order.
  Never advance past a phase until its acceptance passes and `cargo test` is
  green. One phase (or acceptance sub-bullet) per commit.
- **Native is hybrid.** C backend is the default for everything; the LLVM path
  exists **only** for the SIMD span. Both link over the C ABI.
- **Golden examples are the regression set.** The code blocks in the spec become
  `examples/*.maca` verbatim; `examples/bad/*.maca` must be rejected with the
  right diagnostic.
- The spec wins ties. If a design must change, update `docs/PLAN.md` and the
  affected phase together.

## Gotchas

- **Native/Nix builds go through WSL (NixOS).** Windows side has no `zig`/`nix`;
  WSL is NixOS with `nix`/`nix-instantiate` on PATH. Phase 4 gets a C compiler
  via `wsl nix shell nixpkgs#zig -c zig cc ...`; Phase 8 uses `wsl nix-instantiate`.
  Paths cross the boundary: a Windows path `C:\x` is `/mnt/c/x` in WSL (translate
  when shelling out).
- **Maca surface syntax** (matters when writing `.maca` fixtures): no `fn`, no
  `type`, no `Result`/`Ok`, no `<>` generics, **no `async` keyword** (async is a
  colorblind, inferred effect — see below). Field `:` = type, `=` = value.
  Bracketless comma lists. Ternary is spaced `c ? x : y`; error-propagate is
  attached `x?`. `main() -> int`.

## Status

The compiler is complete and `cargo test` is green across the workspace. Whole
programs (non-`main` functions, records→structs, sum types→tagged enums, lists,
string interpolation, `match` lowering incl. list patterns, UFCS) compile and
run end-to-end (parse → check → C → `cc`/`zig cc` → execute).

Backends: native C (default), LLVM (SIMD span), Nix (config mode), JS
(reactive UI), JVM (Java source / Minecraft-Fabric interop), and embedded
(freestanding C for Cortex-M / RISC-V). Driver: `build` (`--target
nix|js|jvm|embedded`, `--mcu`, `--cp`), `run`, `dev` (dev-shell flake), `watch`,
`fmt`, `lint`, `profile`, `init`.

`maca dev` also emits `.maca/dev/{setup,activate}.ps1` when the config declares
`scoop.*`/`choco.*`/`winget.*` packages (see `dev.maca`): Windows hosts (no nix)
get a portable, project-local toolchain under `.maca\dev\`, and the flake
ignores those namespaces so Nix/Linux hosts are unaffected (`emit_windows_dev`
in `maca-backend-nix`).

Bindings (no `let` keyword): a bare lowercase `x = e` binds a **mutable**
variable; `const x = e`, `x = e as const`, or a **Capitalized** name binds a
**constant** (`is_const`). Reassigning a constant is a compile error
(`DiagKind::Immutable`), caught by the compiler and the LSP. A Capitalized
constant works but `maca lint` nudges toward explicit `const`. (Runtime: a bare
`x = e` that first introduces a name declares it; a later `x = e` reassigns.)

Type checker (`maca-core`): gradual unification with an `any` escape hatch for
unknown stdlib, strict on the acceptance diagnostics (`DiagKind`:
TypeMismatch / NonExhaustive / EffectInConfig / UnknownOption / Immutable /
UndefinedName — the last flags a direct call to a name defined nowhere (no
local/user-fn/import/builtin), so a typo surfaces as a clean diagnostic
instead of a broken-C link error; UI element tags and embedded intrinsics are
exempt via `maca_parser::is_backend_intrinsic`). Function
signatures generalize into `Scheme`s (lowercase names are type vars) and
instantiate per call; the C backend monomorphizes generics (one specialized fn
per concrete instantiation). Call **arity** and disagreeing **if/ternary
branch** types also surface as `TypeMismatch`.

**Colorblind async (no `async` keyword):** async-ness is an inferred effect,
not a function color. `spawn f(x)` runs `f` concurrently → `Future a`; `await
fut : a` suspends until it resolves; `sleep_ms(ms)` is a suspension point. `eff`
in `maca-core` adds the `ASYNC` effect for `await`/`spawn`/`sleep_ms` (so config
mode rejects them as impure). C backend lowers to `maca_spawn`/`maca_await`/
`maca_sleep_ms` (pthread-backed futures in `maca-runtime`'s `ASYNC_C`; an async
fn is an ordinary fn — no ABI change). Playground interpreter runs it eagerly.
`await`/`spawn` are unary-precedence prefix operators (`await a + await b` =
`(await a) + (await b)`). Example: `examples/async.maca`.

Language surface beyond the original cheatsheet: operator overloading;
`while`/`break`/`continue` + reassignment; inclusive integer ranges `lo..hi`
(counts lo … hi; an `int[]`; `for i in 1..n` lowers to a counting loop in C); `%`, `<<`, `>>` operators;
hex/binary/octal integer literals with `_` separators; list/string subscripting
`xs[i]` with lvalue assignment (`xs[i] = v`, `p.f = v`); functional record update
`base with { f = v }`; `len(x)`; recursive sum types (`Tree`, `List`) via boxed
payloads and **recursive record types** (`Expr { children: Expr[] }`, forward-
declared in C to break the struct/array definition cycle); a bracketless comma
list as an arrow-fn body (`f() -> int[] => 1, 2`);
C-keyword-safe identifiers (a Maca `double`/`new`/`class` compiles); a string
stdlib as UFCS methods on `str` (`split`→`str[]`, `trim`, `upper`/`lower`,
`contains`, `starts_with`/`ends_with`, `replace`, `substr`, `index_of`; byte
semantics, implemented in the runtime, C backend, and playground interpreter);
**closures / first-class functions** (a lambda `v => …` captures its enclosing
scope; lowered to a `maca_closure` = code pointer + heap env, one uniform ABI
for capturing and non-capturing lambdas; args/results box through `int64_t` —
str via `intptr_t`, float bit-preserved via `maca_box_f64`); a **list stdlib**
as UFCS methods on any `T[]` (`map`/`filter`/`reduce`/`fold` take closures typed
by the element; `sort`/`reverse`/`push`/`pop`/`contains`/`index_of`/`sum`/`min`/
`max`/`first`/`last`/`length`/`get`/`slice`; native + interpreter —
`examples/collections.maca`); plus the `str` scan primitives
`chars`/`length`/`at` and the char classes `is_whitespace`/`is_ascii_digit`/
`is_alpha` (what `selfhost/lexer.maca` scans with);
and raw triple-quoted strings (`"""…"""`) with `import js`/`import css` foreign
blocks that let a `.maca` UI carry its own host glue and styles inline (see
`playground/playground.maca`). Examples:
`examples/{indexing,record_update,tree,sum_record,keywords,strings}.maca`.

**Codegen note (C backend):** control-flow expressions (`if`/`match`/block)
work in value position via a `Sink` (Discard/Return/Assign) threaded through
`block`/`stmt_expr`/`match_stmt`; nullary enum-variant patterns lower to tag
tests (mirroring the checker's `is_variant`). `maca-runtime` holds the C
sources (`RUNTIME_H`/`RUNTIME_C`). The native driver compiles via `wsl nix
shell nixpkgs#zig -c zig cc … -target x86_64-linux-musl -static -s` when WSL is
present, else the host `cc`.

Grammar decisions worth knowing (in `parser.rs`): `no_brace` mode in control
headers so `for x in xs {` isn't a ctor; fn-def detected by lookahead for
`-> | { | =>` after `)`; call args separated by comma **or** juxtaposition (UI);
lambda-body assign (`v => age = int(v)`).
