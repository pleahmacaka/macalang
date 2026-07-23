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
| `lexer.maca` | 1 | character-level scanner (`lex : str -> Token[]`) |
| `main.maca` | 1 | driver entry; today a lexer demo |
| `parser.maca` | — | *next*: tokens → AST |
| `check.maca` | — | *next*: types + effects |
| `emit_c.maca` | — | *next*: AST → C |

## What's proven today

CI can't run native Maca in this environment (no `zig`/WSL), so the gate is the
stage-0 front-end itself: `crates/driver/tests/selfhost.rs` requires that

1. every `selfhost/*.maca` **parses** with no errors, and
2. the concatenated module **type-/effect-checks clean**.

That means the stage-1 sources are always valid, checkable Maca as they grow —
the first half of "stage-0 accepts stage-1". The remaining half (stage-0's C
backend lowering the whole compiler, then running the two-stage fixed-point) is
tracked below.

## Roadmap

- [x] `token.maca`, `lexer.maca`, `main.maca` — parse + typecheck clean
- [ ] `std/str` primitives implemented for the C backend (see `std/README.md`)
- [ ] `parser.maca` — recursive-descent over the token stream
- [ ] `check.maca` — port the gradual checker (HM generalization, row
      unification, monomorphization all land here, not in Rust)
- [ ] `emit_c.maca` — C emitter
- [ ] stage-0 C backend grows to lower the above (higher-order params, closures,
      string primitives, nested modules)
- [ ] two-stage fixed-point build in CI (once native runs are available)

## Why the Rust side stays minimal

Every feature added to the Rust compiler is a feature the Maca compiler must
also implement to self-host. Keeping stage-0 small keeps the bootstrap
tractable: the type-system hardening that would otherwise grow `maca-core`
(full HM over inferred bindings, real row unification, generic monomorphization)
is deferred into `selfhost/check.maca`, where it only has to be written once —
in Maca.
