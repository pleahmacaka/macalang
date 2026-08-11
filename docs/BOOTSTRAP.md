# Bootstrapping Maca in Maca

Maca compiles itself, in stages. The Rust workspace is the **stage-0
bootstrap**, frozen in scope, kept only as capable as it must be to compile
the stage-1 compiler. Compiler work goes into `modules/maca/`, written in Maca.

```
stage-0 (Rust, crates/*)  ──compiles──▶  stage-1 (Maca, modules/maca/*.maca)
                                              │
                                              └──compiles──▶ stage-2
                                                             (emits the same C)
```

The bootstrap closes when the compiler, compiled by itself, emits what it emitted
before. Point each stage in turn at the compiler's own entry file:

```sh
maca build apps/maca1/main.maca -o maca1     # stage-0 (Rust) builds stage-1
mkdir one two
(cd one && ../maca1 build ../apps/maca1/main.maca -o maca)   # stage-1 builds itself
(cd two && ../one/maca build ../apps/maca1/main.maca -o maca)
cmp one/maca two/maca                        # fixed point ⇒ self-hosted
```

**This closes.** Two tests in `crates/driver/tests/selfhost.rs` keep it closed.
`the_compiler_resolves_its_own_imports` says that the compiler reads
`apps/maca1/main.maca`, follows the `import` graph to the eight
`modules/maca/*.maca` files, and emits one 136152-byte C file that emits itself
again byte for byte. `stage1_builds_the_binary_that_builds_it` says the stronger
thing: stage-1 turns that C into an **executable**, and the executable builds a
byte-identical executable from the same source. That last step adds what `cmp`
on the C cannot: the host C compiler is deterministic, and the compiler is
self-sufficient, needing a C compiler and nothing else.

The two rounds run in **different directories with the same output name**,
because `cc` records the name of the file it was handed in the binary's symbol
table: `-o maca2` and `-o maca3` produce two executables that differ in exactly
one byte, the digit. `strip` makes them equal; building both as `maca` makes them
equal without stripping.

Comparing the emitted C is still the half that catches a divergence early, and
the demo equality is the half that matters most, because `cmp` alone can be
silent while stage-1 and stage-2 are different programs that happen to agree on
one input.

`apps/maca1` resolves an `import` the way the driver does: `import maca/token`
means `maca/token.maca`, tried as written from the importing file's own directory
and then under that directory's `modules/` and `src/` roots, walking up to the
workspace root, which is the order `docs/LAYOUT.md` sets out and why. Each file
is lexed once, read once, and its tokens are spliced in ahead of the file that
imported it, so a definition precedes its use and a file imported twice is
emitted once. An import that resolves to no file is an error naming it and the
file that asked. Feeding the concatenation of those nine files still works and
`stage2_emits_the_c_it_was_built_from` still does it, which is worth keeping: the
two paths emit the same 136152 bytes and differ only in the order the record and
enum declarations come out in.

## Layout of `modules/maca/`

| file | stage | role |
|---|---|---|
| `token.maca` | 1 | token kinds (a nullary sum → C enum), `Token`, `keyword_kind` |
| `ast.maca` | 1 | the recursive `Expr`/`Stmt`/`Module` AST (typed params/returns, records, sums, match, methods) + an AST pretty-printer |
| `lexer.maca` | 1 | character-level scanner incl. two-char operators + float literals (`lex : str -> Token[]`) |
| `parser.maca` | 1 | recursive descent → expressions (precedence, ternary, unary, calls, field/method, records, match), function/record/sum declarations, and `import` skipping |
| `ty.maca` | 1 | the type representation (`Ty`, `Scheme`), the substitution `Infer`, unification including rows, and `generalize`/`instantiate` |
| `check.maca` | 1 | the type checker over `Ty`: an `Env` of signatures, record fields, sum variants and locals, threading one substitution through the whole module |
| `emit_c.maca` | 1 | a C emitter over the AST → C source (with a `<string.h>`/`<stdlib.h>`/`<stdio.h>` + `maca_cat`/`maca_int_to_str` preamble) |
| `emit_rust.maca` | 1 | a Rust emitter over the same AST → Rust source (the `--target rust` back end, in Maca) |

The binary that drives them is `apps/maca1/main.maca`: it compiles a named file
through the package, or runs the demo the gate reads.

## How the gate works

The stage-1 **front-end compiles and runs as a native binary, and emits
through two back ends written in Maca** (`emit_c.maca` and `emit_rust.maca`).
The gate, `crates/driver/tests/selfhost.rs`, requires that

1. every `modules/maca/*.maca` **parses** with no errors,
2. the concatenated module **type-/effect-checks clean**, and
3. where a native `cc` is available, the concatenated compiler **builds and
   runs** the whole `lex → parse → check → emit` pipeline, then compiles the
   *emitted* program **two ways** (the C output with `cc`, the Rust output with
   `rustc`) and runs both.

The parsed slice is well past a toy. stage-1 handles:

- the full primitive expression language: `int`/`float`(`1.5`)/`str`/`bool`
  literals; `+ - * / %`, comparison, `&& ||` (with a precedence ladder), unary
  `- !`, ternary; multi-argument and nested calls;
- **type-threaded signatures**: a parameter/return/local type flows to the
  emitted code (`str` → `const char*`/`String`, `float` → `double`/`f64`,
  `bool`, records);
- Maca's core **data model**: records (`Point = { x: int }` → a
  `typedef struct`/`struct`, literals, field access) and nullary sum types +
  `match` (`Color = Red | Green` → a `typedef enum`/`enum` + `use Color::*`;
  `match` → a C ternary chain / a native Rust `match`);
- **strings**: value equality (`==`/`!=` → `strcmp`), concatenation (`++` →
  `maca_cat` / `format!`), `str(int)` (→ `maca_int_to_str` / `format!`), and the
  `.length()` / `.at(i)` methods the lexer scans with;
- the `info`/`print` output builtins (→ `printf` / `println!`), the file
  builtins (`read_file`/`write_file`) and `exec` (→ `fork` + `execvp`), and
  `import` statements, which `parse_items` skips because `apps/maca1` has already
  followed them and spliced in what they name.

As a capstone the gate compiles and **runs** the multi-feature program the
self-hosted compiler emits (a `Point` record, a `Color` sum + `match`, string
concat/equality, `str`, a `.length()` call, and an `info(…)`) through both back
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

- **the lexer and parser**, `token.maca`, `lexer.maca`, `parser.maca`: the
  character scanner (two-char operators, float literals), recursive descent
  with a precedence ladder, function/record/sum declarations, and `import`
  statements, skipped by the parser because the driver resolved them first
- **the recursive AST**: `ast.maca`, whose `Expr { children: Expr[] }` is what
  drove recursive record types into the stage-0 backend
- **two back ends**: `emit_c.maca` and `emit_rust.maca`, each turning a module
  into a complete translation unit
- **multi-file builds**: `apps/maca1` reads its entry file, follows every
  `import` to a file under the search roots `docs/LAYOUT.md` orders, and splices
  the imported tokens in ahead of the importer's, so nine files compile as one
  unit with each read once. Cross-file resolution, not concatenation: the tokens
  are joined rather than the text, which is also why the whole compiler now lexes
  in a fraction of the time (a per-file scan instead of one 96KB one)
- **higher-order parameters**: a function passed by name is wrapped in a
  closure, and an unannotated parameter that is called is typed as a function
  value, which is how `lexer.maca` shares one predicate-taking `run_end`
- **type-threaded signatures**: a parameter, return or local type reaches the
  emitted C and Rust
- **records**: `typedef struct` / `struct`, literals, field access
- **nullary sum types and `match`**: `typedef enum` / `enum` + `use`, lowered
  to a C ternary chain and a native Rust `match`
- **strings**: `==`/`!=` via `strcmp`, `++` via `maca_cat`/`format!`,
  `str(int)`, `.length()`, `.at(i)`
- **dynamic arrays**: `T[]` parameters and returns, list literals, `.get(i)`
  and `.count()`, over a heap `MacaList` in C and a `Vec<i64>` in Rust
- **string interpolation**: `"n = {x}"`, desugared in the *parser* to the
  concat and `str` forms both back ends already emit, so neither emitter needed
  a new case
- **payload sum variants**: `Circle(int) | Rect(int, int)`, and a match that
  binds the payload. In C a tag plus a row of cells wide enough for the widest
  variant, with a constructor named after each variant so an ordinary
  `Circle(2)` compiles without the call site knowing which names are variants;
  in Rust the native enum, where it already is one
- **a block inside an expression**: `Expr` carries a `stmts` list beside its
  children, so an `if` branch that binds a local is a block node. C emits a
  statement expression and Rust a block that ends in its value, and a binding
  inside a branch does not escape it. The alternative was rewriting the 39
  branches of the compiler's own source that bind a local; a record literal now
  needs the comma the spec always asked for, which is what stops `if c { d = 1 d }`
  reading as a record
- **an error channel out of the scanner, and a parser that always advances**:
  `lex_all` returns a `Lexed` of the tokens beside the errors, so an unknown byte
  and an unterminated string are said out loud rather than flattened into an
  identifier, and `apps/maca1` prints them and counts them into its exit status.
  `parse_module` requires a `name (` before it reads a declaration and steps over
  what is not one, which is what stops it reading past the end of the token list.
  The parser has no error list of its own yet, so what it steps over is skipped
  quietly; that is the next thing
- **array operations, and a method that knows its receiver**: `check.maca` types
  a method call, which it did not before, so `.length()`, `.get(i)`, `.slice`,
  `.index_of`, `.join` and `.push` each pick their lowering from the receiver's
  type rather than from a guess. `++` between two lists is a block copy and not
  `maca_cat`. C gains `maca_list_cat`/`maca_list_slice`/`maca_list_index_of`/
  `maca_list_join` beside `maca_listv`; Rust uses `concat`, a range `to_vec`,
  `iter().position` and `join`. Both back ends compile and run the same program
  to the same answer, which is what the scanner needs before it can hold a brace
  stack
- **an escape and a comment in the scanner**: a `\` inside a string literal
  carries the next character with it, so `"\""` is one token rather than a string
  cut in half, and the escape is kept in the token's text so it re-emits as it
  was written. `//` to the end of the line is trivia. Both are what the
  compiler's own source is made of, `\"` throughout the emitters and one `//`
  blurb the package index reads, so neither file could be scanned without them
- **`if` as an expression**: `if c { a } else if d { b } else { e }`, where an
  `else if` nests to the right, lowered to a C ternary chain and to a native
  Rust `if`. This is the construct the compiler's own source is written in, 315
  branches of it, so it is the one that had to arrive before stage-1 could read
  itself. Branches hold a single expression; the 26 places in
  `modules/maca/*.maca` that bind a local inside a branch still need their
  bindings hoisted above the `if`
- **a typed tree**: `annotated` walks a checked module and writes each
  expression's inferred type into the `ty` the AST already carries, so a back end
  reads a type rather than guessing from an expression's shape. That is what
  separates `ts.length()` from `s.length()`, an element from a byte in `.get(i)`,
  and a `double` local from an `int` one. An unannotated tree still emits what it
  used to, which is why the demo output is unchanged
- **a record update and a shorthand field**: `base with { field = value }`
  copies and then assigns, as a C statement expression over `__typeof__` and as
  a Rust block that mutates its own copy, so neither back end needs the record's
  name. A field written as a bare name is that name as its own value, which is
  how `Token { kind, text, pos }` is spelled throughout the compiler's own
  source
- **a checker that unifies**: the module's function signatures, record fields
  and sum variants, plus the locals and parameters in scope, all carried as
  `Ty` rather than as a type's name. A signature is a function type, so a call
  checks its arguments and not merely their number; a `+` between two strings
  is rejected by the operator; a list, the arms of a `match` and the branches of
  a ternary are unified with each other; and one substitution is threaded
  through the module, so an **unannotated parameter is a fresh variable solved
  by how it is used**. Its variable is shared with the body, so `keep(x) -> int
  => x + 1` narrows `x` to a number and then rejects `keep("s")`; where the body
  says nothing, the parameter is generalized at the declaration and instantiated
  at each call, so `keep(x) -> int => 1` is usable at `int` in one call and `str`
  in the next. The module is checked **twice**, the first pass to infer and the
  second to report, so what a body proves reaches a call written above it and
  declaration order does not change the answer. A clash produces an error type
  that absorbs, so one mistake is reported once; an undeclared name stays
  gradual, because foreign is not wrong
- **a compiler CLI**: `maca1 <in.maca> <out.c> [rust]` reads a source file and
  writes the emitted source, which is what makes the differential gate possible,
  and `maca1 build <in.maca> -o <bin>` writes `<bin>.c` and then runs `cc` on it,
  which is what makes the *binary* fixed point possible. The second form is the
  whole of what "self-sufficient" means: building the compiler needs a C compiler
  and nothing else

## The differential gate

The bootstrap closes on a byte-identical rebuild. The check that gets you there,
and the only one that can catch a divergence, is the step before it: compile
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
Each feature is added to `modules/maca/` when the self-hosted compiler needs it to
compile itself, and the dual-backend compile-and-run gate is what says it
arrived.

So the boundary moves by writing Maca, not by planning.

## Where the boundary is, measured

Three sweeps say how far stage-1 has got, and they are worth re-running rather
than reasoning about. Build stage-1, then over every `.maca` file under `apps/`
and `modules/` (excluding `bad/`):

| sweep | what it runs | as of 2026-08-11 |
|---|---|---|
| accept | `maca <file> out.c` exits 0 | 196 of 197 |
| compile | the emitted C compiles | 182 of 197 |
| link | `maca build <file> -o bin` produces an executable | 59 of 73 programs with a `main()` |
| run | `maca test` on each `modules/*/tests/*.maca` | 23 of 26 suites green |

The last two are the ones that matter, because the first two are satisfied by C
that is wrong. Two bugs found this way had both compiled cleanly: a branch
ending in an assignment dropped the assignment, and a lambda used as a value
emitted `0`, so `modules/std/tests/list.maca` called through a null pointer.

What the remaining failures are, in the order they block deletion:

- **a library a link flag names.** `sqlite_open` and `py_call` are the two
  builtins left with no stage-1 body, and unlike the http and mqtt ones they
  cannot simply be carried in the preamble: they need `-lsqlite3` and
  `-lpython3`, and `build_binary` runs `cc` with no flags at all. The manifest
  has to name them before the runtime can.
- **the driver itself.** Every crate is reachable from `maca-driver`, so nothing
  under `crates/` can go until `apps/maca1` covers what the driver does:
  `watch`, `dev`, `add`/`install`/`update`/`upgrade`, `spec`, `fix`, the
  manifest chain and `[[bin]]` resolution, link flags, and the LLVM path, which
  exists only for the SIMD span. `crates/wasm` is not a back end: it is the
  compiler built for the browser, so its Maca answer is building `apps/maca1`
  for a wasm target rather than writing an emitter.

The two stages own different halves of the type system, and the line between
them has moved. The **type representation and unification are Maca now**:
`ty.maca` is `crates/core/src/ty.rs` rewritten, variables and all, so
substitution, the occurs check, row unification over open and closed records,
and `generalize`/`instantiate` are gated by `modules/maca/tests/ty.maca` rather
than by Rust. What is still only in `maca-core` is the checker over the *whole*
surface (`lib.rs`), the effect set, and generic monomorphization. Stage-0 is
retired when those are written in Maca as well, which is the same
increment-and-gate loop as everything above it, run once more.

## Why the Rust side stays minimal

Every feature added to the Rust compiler is a feature the Maca compiler must
also implement to self-host. That is the whole argument for keeping stage-0
small: type-system work that would otherwise grow `maca-core` (full HM over
inferred bindings, row unification, generic monomorphization) belongs in
`modules/maca/check.maca`, where it is written once, in Maca, instead of twice.
