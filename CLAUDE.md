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
| `maca-lsp` | tower-lsp language server |
| `maca-mcp` | Maca MCP server (LLM-native tools) |
| `maca-driver` | the `maca` CLI |
| `maca-wasm` | `wasm32` front-end for the browser playground (no wasm-bindgen) |

Non-crate dirs: `std/` (Maca-source stdlib), `examples/` (golden `.maca`
programs + `examples/bad/`), `apps/` (capstones), `selfhost/` (the Maca compiler
written in Maca — stage 1), `playground/` (Monaco web playground), `editor/`,
`docs/`.

## Self-hosting (current direction)

Rust (`crates/*`) is the **frozen stage-0 bootstrap** — keep it minimal. New
compiler work is written in Maca under `selfhost/` and gated by the stage-0
front-end (`crates/driver/tests/selfhost.rs`). See `docs/BOOTSTRAP.md`. When a
change is needed, prefer adding it to `selfhost/*.maca` over growing the Rust
crates; only touch stage-0 for genuine bootstrap bugs (e.g. a parser that hangs
or mis-parses valid surface syntax).

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
  `type`, no `Result`/`Ok`, no `<>` generics. Field `:` = type, `=` = value.
  Bracketless comma lists. Ternary is spaced `c ? x : y`; error-propagate is
  attached `x?`. `main() -> int`.

## Status

Phases 0–3 complete (front-end + semantic analysis). Phase 4 (C backend) in
progress: **`hello.maca` compiles and runs end-to-end** (parse→check→C→`zig cc`
static-musl via WSL→execute), 11.7 KB stripped binary, gated by a real
build+run test (`crates/driver/tests/run.rs`, skips without WSL). Whole
workspace green.

**Phase 4 remaining — extend the C backend to `taskr`** (`crates/backend_c`,
currently a `main`-only slice). Needs, roughly in order: emit non-`main`
functions with C types; records → structs; sum types → tagged enums; lists
(`T[]`) → a small dynamic-array runtime; string interpolation → a `maca_fmt`
builder; `match` → switch/if lowering incl. list patterns; `?` propagation;
UFCS methods; then `std/json` (encode/decode for the Store/Task types),
`std/dirs` + file read/write. That's essentially the runtime + a stdlib slice
in C — a large chunk, size comparable to the parser. `maca-runtime` holds the C
sources (`RUNTIME_H`/`RUNTIME_C`); grow them alongside the backend.

- **P4 driver** `maca build <f> [-o out]` / `maca run <f> [args]`. Compiles via
  `wsl nix shell nixpkgs#zig -c zig cc … -target x86_64-linux-musl -static -s`;
  `to_wsl()` translates `C:\…`→`/mnt/c/…`. Runs the Linux binary through WSL.

- **P3** `maca-core`: gradual unification checker (`ty` module) — `any` escape
  hatch for unknown stdlib, strict on the 4 acceptance diagnostics
  (`DiagKind`: TypeMismatch / NonExhaustive / EffectInConfig / UnknownOption).
  `check(module, Mode)`; `Mode::Program|Config`. Good examples typecheck, the
  four `examples/bad/*.maca` are rejected. **Let-polymorphism landed**: function
  sigs generalize into `Scheme`s (`ty::is_type_var_name` treats lowercase names
  as type vars) and instantiate per call; concrete arg/param clashes now report
  `TypeMismatch` (`examples/generic.maca` good, `examples/bad/arg_mismatch.maca`
  bad). Also caught (all `TypeMismatch`): call **arity** vs a user fn's param
  count (variadics exempt, `bad/arity.maca`) and disagreeing **if/ternary
  branches** (`bad/branch_mismatch.maca`). Full row unification + backend
  monomorphization of generics still to do.
- **P0** workspace + `maca --version`.
- **P1** `maca-lexer`: full tokenizer (significant newlines, path literals,
  string interpolation, `x?` vs `? :`). Golden token dumps in
  `crates/lexer/tests/golden/` (regenerate with `UPDATE_GOLDEN=1`).
- **P2** `maca-parser`: hand-written recursive-descent + Pratt (deviation from
  the brief's chumsky, for zero deps + layout control). `ast` / `parser` /
  `print` modules. All 5 examples parse and roundtrip (parse→print→parse is
  AST-stable) — that's the regression gate in `crates/parser/tests/parse.rs`.

Grammar decisions worth knowing (in `parser.rs`): `no_brace` mode in control
headers so `for x in xs {` isn't a ctor; fn-def detected by lookahead for
`-> | { | =>` after `)`; call args separated by comma **or** juxtaposition (UI);
lambda-body assign (`v => age = int(v)`). Known simplification: `extensions =
"nix", "rust"` parses as a keyed field + a bare entry (not a list value) — fine
for roundtrip, revisit if Phase 8 needs list semantics.
