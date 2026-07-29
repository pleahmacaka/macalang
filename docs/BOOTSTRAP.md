# Bootstrapping Maca in Maca

Maca compiles itself, in stages. The Rust workspace is the **stage-0
bootstrap** — frozen in scope, kept only as capable as it must be to compile
the stage-1 compiler. Compiler work goes into `selfhost/`, written in Maca.

```
stage-0 (Rust, crates/*)  ──compiles──▶  stage-1 (Maca, selfhost/*.maca)
                                              │
                                              └──compiles──▶ stage-1 again
                                                             (must be identical)
```

The bootstrap closes when a stage-1 binary built by stage-0 rebuilds itself
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
| `check.maca` | 1 | the type checker: an `Env` of signatures, record fields, sum variants and locals; result types, arity, field types, declared returns |
| `emit_c.maca` | 1 | a C emitter over the AST → C source (with a `<string.h>`/`<stdlib.h>`/`<stdio.h>` + `maca_cat`/`maca_int_to_str` preamble) |
| `emit_rust.maca` | 1 | a Rust emitter over the same AST → Rust source (the `--target rust` back end, in Maca) |
| `main.maca` | 1 | driver entry; lexes + parses + checks + emits samples through both back ends |

## How the gate works

The stage-1 **front-end compiles and runs as a native binary, and emits
through two back ends written in Maca** (`emit_c.maca` and `emit_rust.maca`).
The gate, `crates/driver/tests/selfhost.rs`, requires that

1. every `selfhost/*.maca` **parses** with no errors,
2. the concatenated module **type-/effect-checks clean**, and
3. where a native `cc` is available, the concatenated compiler **builds and
   runs** the whole `lex → parse → check → emit` pipeline, then compiles the
   *emitted* program **two ways** — the C output with `cc`, the Rust output with
   `rustc` — and runs both.

The parsed slice is well past a toy. stage-1 handles:

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

## What stage-1 compiles

Each entry below is exercised by `crates/driver/tests/selfhost.rs`, which
compiles the emitted program with both `cc` and `rustc` and runs it.

- **the lexer and parser** — `token.maca`, `lexer.maca`, `parser.maca`: the
  character scanner (two-char operators, float literals), recursive descent
  with a precedence ladder, function/record/sum declarations, and `import`
  statements (skipped — the driver inlines modules)
- **the recursive AST** — `ast.maca`, whose `Expr { children: Expr[] }` is what
  drove recursive record types into the stage-0 backend
- **two back ends** — `emit_c.maca` and `emit_rust.maca`, each turning a module
  into a complete translation unit
- **multi-file builds** — the gate builds from `selfhost/main.maca` and the
  driver resolves the `import` graph; there is no concatenation step
- **higher-order parameters** — a function passed by name is wrapped in a
  closure, and an unannotated parameter that is called is typed as a function
  value, which is how `lexer.maca` shares one predicate-taking `run_end`
- **type-threaded signatures** — a parameter, return or local type reaches the
  emitted C and Rust
- **records** — `typedef struct` / `struct`, literals, field access
- **nullary sum types and `match`** — `typedef enum` / `enum` + `use`, lowered
  to a C ternary chain and a native Rust `match`
- **strings** — `==`/`!=` via `strcmp`, `++` via `maca_cat`/`format!`,
  `str(int)`, `.length()`, `.at(i)`
- **dynamic arrays** — `T[]` parameters and returns, list literals, `.get(i)`
  and `.count()`, over a heap `MacaList` in C and a `Vec<i64>` in Rust
- **string interpolation** — `"n = {x}"`, desugared in the *parser* to the
  concat and `str` forms both back ends already emit, so neither emitter needed
  a new case
- **payload sum variants** — `Circle(int) | Rect(int, int)`, and a match that
  binds the payload. In C a tag plus a row of cells wide enough for the widest
  variant, with a constructor named after each variant so an ordinary
  `Circle(2)` compiles without the call site knowing which names are variants;
  in Rust the native enum, where it already is one
- **a checker with an environment** — the module's function signatures, record
  fields and sum variants, plus the locals and parameters in scope. Arity, a
  call's result type, a field's type, and a body that disagrees with its
  declared return are all errors now; an undeclared name stays gradual, because
  foreign is not wrong
- **a compiler CLI** — `maca1 <in.maca> <out.c> [rust]` reads a source file and
  writes the emitted source, which is what makes the differential gate possible

## The differential gate

The bootstrap closes on a byte-identical rebuild. The check that gets you there
— and the only one that can catch a divergence — is the step before it: compile
one source with stage-0 and with stage-1, and require the two programs to
behave identically.

`crates/driver/tests/selfhost.rs` does exactly that. stage-0 builds stage-1;
stage-1 compiles a program covering the slice above; the emitted C is compiled
with `cc` and the emitted Rust with `rustc`; and all three runs must print the
same thing and exit the same way. A difference there is a difference between
two compilers, which is the whole risk the bootstrap carries.

## Where stage-1 stops, and why that is the shape

Stage-1 is a compiler for the subset of Maca that stage-1 is written in. That
is not a limitation to be worked around; it is the loop the bootstrap runs on.
Each feature is added to `selfhost/` when the self-hosted compiler needs it to
compile itself, and the dual-backend compile-and-run gate is what says it
arrived.

So the boundary moves by writing Maca, not by planning. What it takes to close
the loop is the rest of the type system: `check.maca` reasons about a module's
own declarations, and the generalization and row unification that `maca-core`
does are still the stage-0 crate's. They have to be written once more, in Maca,
before stage-0 can be retired — and by then stage-1 will be compiling the file
that does it.

## Why the Rust side stays minimal

Every feature added to the Rust compiler is a feature the Maca compiler must
also implement to self-host. That is the whole argument for keeping stage-0
small: type-system work that would otherwise grow `maca-core` — full HM over
inferred bindings, row unification, generic monomorphization — belongs in
`selfhost/check.maca`, where it is written once, in Maca, instead of twice.
