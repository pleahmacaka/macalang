# Bootstrapping Maca in Maca

Maca's long-term goal is to compile itself. The Rust workspace is the
**stage-0 bootstrap** — frozen in scope, kept only as capable as it must be to
compile the stage-1 compiler. New compiler work goes into `selfhost/`, written
in Maca.

```
stage-0 (Rust, crates/*)  ──compiles──▶  stage-1 (Maca, selfhost/*.maca)
                                              │
                                              └──compiles──▶ stage-1 again
                                                             (must be identical)
```

Self-hosting is reached when a stage-1 binary built by stage-0 rebuilds itself
byte-for-byte:

```sh
maca build selfhost/main.maca -o maca1      # stage-0 (Rust) builds stage-1
./maca1 build selfhost/main.maca -o maca2   # stage-1 builds itself
cmp maca1 maca2                             # fixed point ⇒ self-hosted
```

## Layout of `selfhost/`

| file | stage | role |
|---|---|---|
| `token.maca` | 1 | token kinds (a nullary sum → C enum), `Token`, `keyword_kind` |
| `ast.maca` | 1 | the recursive `Expr`/`Stmt`/`Module` AST (typed params/returns, records, sums, match, methods) + an AST pretty-printer |
| `lexer.maca` | 1 | character-level scanner incl. two-char operators + float literals (`lex : str -> Token[]`) |
| `parser.maca` | 1 | recursive descent → expressions (precedence, ternary, unary, calls, field/method, records, match), function/record/sum declarations, and `import` skipping |
| `check.maca` | 1 | a coarse type checker (infers `int`/`str`/…, counts mismatches) |
| `emit_c.maca` | 1 | a C emitter over the AST → C source (with a `<string.h>`/`<stdlib.h>`/`<stdio.h>` + `maca_cat`/`maca_int_to_str` preamble) |
| `emit_rust.maca` | 1 | a Rust emitter over the same AST → Rust source (the `--target rust` back end, in Maca) |
| `main.maca` | 1 | driver entry; lexes + parses + checks + emits samples through both back ends |

## What's proven today

The stage-1 **front-end compiles and runs as a native binary, and now emits
through two back ends written in Maca** (`emit_c.maca` and `emit_rust.maca`).
The gate, `crates/driver/tests/selfhost.rs`, requires that

1. every `selfhost/*.maca` **parses** with no errors,
2. the concatenated module **type-/effect-checks clean**, and
3. where a native `cc` is available, the concatenated compiler **builds and
   runs** the whole `lex → parse → check → emit` pipeline, then compiles the
   *emitted* program **two ways** — the C output with `cc`, the Rust output with
   `rustc` — and runs both.

The parsed slice has grown well past a toy. stage-1 now handles:

- the full primitive expression language — `int`/`float`(`1.5`)/`str`/`bool`
  literals; `+ - * / %`, comparison, `&& ||` (with a precedence ladder), unary
  `- !`, ternary; multi-argument and nested calls;
- **type-threaded signatures** — a parameter/return/local type flows to the
  emitted code (`str` → `const char*`/`String`, `float` → `double`/`f64`,
  `bool`, records);
- Maca's core **data model** — records (`Point = { x: int }` → a
  `typedef struct`/`struct`, literals, field access) and nullary sum types +
  `match` (`Color = Red | Green` → a `typedef enum`/`enum` + `use Color::*`;
  `match` → a C ternary chain / a native Rust `match`);
- **strings** — value equality (`==`/`!=` → `strcmp`), concatenation (`++` →
  `maca_cat` / `format!`), `str(int)` (→ `maca_int_to_str` / `format!`), and the
  `.length()` / `.at(i)` methods the lexer scans with;
- the `info`/`print` output builtins (→ `printf` / `println!`), and `import`
  statements (skipped — the driver inlines modules).

As a capstone the gate compiles and **runs** the multi-feature program the
self-hosted compiler emits — a `Point` record, a `Color` sum + `match`, string
concat/equality, `str`, a `.length()` call, and an `info(…)` — through both back
ends: each prints `self-hosted!` and exits `42`, proving the Maca-written
compiler produces a working executable in **C and Rust alike**.

Getting the pipeline itself running grew the stage-0 backend two capabilities
the rest of the language now shares: the **`std/str` scan primitives**
(`chars`/`length`/`at`/`get`/`slice` + the `is_*` character classes) and
**recursive record types** (`Expr { children: Expr[] }`, lowered with a C
forward declaration that breaks the struct/array definition cycle). Every stage-1
feature since is added in Maca and gated by this dual-backend compile+run.

## Roadmap

- [x] `token.maca`, `lexer.maca`, `main.maca` — parse + typecheck clean
- [x] `std/str` primitives implemented for the C backend (see `std/README.md`)
- [x] `parser.maca` — recursive-descent over the token stream
- [x] recursive AST (`ast.maca`) — the stage-0 backend lowers recursive records
- [x] stage-1 front-end builds + runs natively (the `selfhost.rs` run gate)
- [x] `check.maca` — a coarse type checker (int/str/float, gradual `any`,
      mismatch counting); grows into full HM generalization + row unification
- [x] `emit_c.maca` — a C emitter over the AST slice (literals, idents, calls,
      binary ops, whole functions, and a module → a complete C translation unit)
- [x] `parse_fn` / `parse_module` — function definitions and multi-function
      modules (two-char operators, parameter lists, arrow bodies)
- [x] multi-file builds — `maca build selfhost/main.maca` resolves the local
      `import selfhost/…` statements and inlines the modules in dependency
      order (the run gate builds from the real entry point, no concatenation)
- [x] higher-order parameters — a function passed by name (`run_end(cs, i,
      is_alpha)`) is wrapped in a closure, and an unannotated param that is
      called is typed as a function value; `lexer.maca` uses the single
      predicate-taking `run_end` again
- [x] a **second back end in Maca** — `emit_rust.maca` (the `--target rust`
      path); the capstone is compiled and run through both `cc` and `rustc`
- [x] type-threaded signatures — parameter/return/local types reach the emitted
      C/Rust (`str`/`float`/`bool`/records)
- [x] records — `typedef struct`/`struct`, literals, field access
- [x] nullary sum types + `match` — `typedef enum`/`enum` + `use`, C ternary
      chain / native Rust `match`
- [x] strings — `==`/`!=` (`strcmp`), `++` (`maca_cat`/`format!`), `str(int)`,
      `.length()`/`.at(i)`, `info`/`print` builtins
- [x] `import` statements skipped in the module parser
- [ ] **arrays/lists** — `T[]`, list literals, `.get`/`.push`/`.map` (needs a
      dynamic-array runtime in the emitted C)
- [ ] **string interpolation** — `"{x}"` splices (needs per-splice type
      inference to pick the right formatter; `++` + `str` is the desugared form
      that already works)
- [ ] **payload sum variants** — `Circle(int)` (tagged unions in C)
- [ ] the checker grown past the coarse int/str model (full HM generalization,
      row unification — the rest of `maca-core`, in Maca)
- [ ] two-stage fixed-point build in CI (`cmp maca1 maca2`)

## Why the Rust side stays minimal

Every feature added to the Rust compiler is a feature the Maca compiler must
also implement to self-host. Keeping stage-0 small keeps the bootstrap
tractable: the type-system hardening that would otherwise grow `maca-core`
(full HM over inferred bindings, real row unification, generic monomorphization)
is deferred into `selfhost/check.maca`, where it only has to be written once —
in Maca.
