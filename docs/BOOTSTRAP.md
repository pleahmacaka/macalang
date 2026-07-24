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
| `ast.maca` | 1 | the recursive `Expr`/`Stmt`/`Module` AST + an AST pretty-printer |
| `lexer.maca` | 1 | character-level scanner (`lex : str -> Token[]`) |
| `parser.maca` | 1 | recursive-descent over the token stream → `Expr` |
| `check.maca` | 1 | a coarse type checker (infers `int`/`str`/…, counts mismatches) |
| `emit_c.maca` | 1 | a C emitter over the AST (`Expr` → C source) |
| `main.maca` | 1 | driver entry; lexes + parses + checks + emits a sample |

## What's proven today

The stage-1 **front-end now compiles and runs as a native binary.** The gate,
`crates/driver/tests/selfhost.rs`, requires that

1. every `selfhost/*.maca` **parses** with no errors,
2. the concatenated module **type-/effect-checks clean**, and
3. where a native `cc` is available, the concatenated compiler **builds and
   runs** the whole `lex → parse → check → emit` pipeline: the Maca-written
   lexer + parser turn `add(1) + 2 - 3` into the AST `((add(1) + 2) - 3)`, the
   Maca-written checker infers its type (`int`, no errors) and flags a
   `str + int` clash, and the Maca-written emitter lowers it back to C:
   `int main(void) { return ((add(1) + 2) - 3); }`.

Step 3 is the real milestone — the compiler's own pipeline, written in Maca,
executing through the stage-0 C backend. Getting there grew the backend two
capabilities the rest of the language now shares: the **`std/str` scan
primitives** (`chars`/`length`/`at`/`get`/`slice` + the `is_*` character
classes) and **recursive record types** (`Expr { children: Expr[] }`, lowered
with a C forward declaration that breaks the struct/array definition cycle).

## Roadmap

- [x] `token.maca`, `lexer.maca`, `main.maca` — parse + typecheck clean
- [x] `std/str` primitives implemented for the C backend (see `std/README.md`)
- [x] `parser.maca` — recursive-descent over the token stream
- [x] recursive AST (`ast.maca`) — the stage-0 backend lowers recursive records
- [x] stage-1 front-end builds + runs natively (the `selfhost.rs` run gate)
- [x] `check.maca` — a coarse type checker (int/str/float, gradual `any`,
      mismatch counting); grows into full HM generalization + row unification
- [x] `emit_c.maca` — a C emitter over the AST slice (literals, idents, calls,
      binary ops, a whole translation unit)
- [ ] the remaining backend growth for a full compiler (higher-order params,
      nested modules / multi-file builds so `maca build selfhost/main.maca`
      resolves the imports instead of the gate concatenating the files)
- [ ] two-stage fixed-point build in CI (once native runs are available)

## Why the Rust side stays minimal

Every feature added to the Rust compiler is a feature the Maca compiler must
also implement to self-host. Keeping stage-0 small keeps the bootstrap
tractable: the type-system hardening that would otherwise grow `maca-core`
(full HM over inferred bindings, real row unification, generic monomorphization)
is deferred into `selfhost/check.maca`, where it only has to be written once —
in Maca.
