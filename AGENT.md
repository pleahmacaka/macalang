# Maca

Instructions for any assistant working in this repository. `CLAUDE.md` imports
this file, so Claude Code and the editor's own assistant read one set of rules.

One typed language for **programs and infrastructure config**. Programs compile
to native (C-tier binary), JS, JVM, Rust, Elixir, or freestanding C; config
compiles to Nix. Everything a user writes is `.maca` or `maca.toml`.

**Read `docs/SPEC.md` before starting work.** It is the authority on the
language; when it and the code disagree, it wins, and they change together.

## Commands

```
cc -O1 -o bootstrap/maca bootstrap/maca.c            # a compiler, from the seed
MACA=$PWD/bootstrap/maca ./bootstrap/maca build apps/maca1/main.maca -o bin/maca
MACA=$PWD/bin/maca ./bin/maca test modules/maca/tests/lex.maca   # one suite
MACA=$PWD/bin/maca ./bin/maca run apps/lint/lint.maca            # the linter
```

The CLI binary is `maca`, written in Maca at `apps/maca1/main.maca`.

**Testing gotcha:** a suite that builds natively shells out to a C compiler, so
running the whole bar at once contends under WSL. Run the suites in sequence, as
CI does. The compiler needs a stack the default 8 MB only just covers, so a
segfault with no message during a self-build is a stack question before it is
anything else: try `ulimit -s 16384` before blaming a change.

## Where things are

The tree is four directories, and each answers one question:

| | |
|---|---|
| `modules/` | what a program imports. Nine packages, all ordinary Maca, all with a suite under `<pkg>/tests/`. They ride **inside** the `maca` binary. `modules/maca` is the compiler itself, so a Maca program can lex and parse Maca. |
| `apps/` | everything this repository builds: the capstones, the demos, the toolchain's own programs, the handbook builder, the playground, `apps/maca1` which drives the compiler package, and `apps/examples`, which is the regression corpus. |
| `bootstrap/` | `maca.c`, the compiler as the compiler emitted it. `cc -O1` on it gives a working compiler in four seconds, and that one rebuilds itself to a fixed point. It is how a machine with no Maca gets one. |
| `docs/` | the spec, the changelog, and how this thing is built. |

`modules/*` and `src/*` are import search roots, so `modules/std/text.maca` is
written `std/text` from anywhere in the tree. `apps/*` deliberately is not, so
an application is reached by its written path. **Do not name a directory after
a package**: the written path is tried first, so a `bench/` beside
`modules/bench/` decides `import bench/stat` by whichever file exists.

[`docs/LAYOUT.md`](docs/LAYOUT.md) is the whole of it: the search order and why
it is that order, the workspace and its manifests, `[build]`, and what each
`apps/*` entry is.

## What the compiler does

[`docs/INTERNALS.md`](docs/INTERNALS.md) for the language surface and the
compiler's own structure, and [`docs/BACKENDS.md`](docs/BACKENDS.md) for what
each back end emits. Two invariants are worth carrying in your head, because
they are the ones that get broken from the inside:

- **Every `maca_str`-returning runtime function returns a fresh block or a
  static literal, never one of its arguments.** A borrowed return is a double
  free that only shows up under a load the tests do not reach.
- **`xs = xs.push(v)` appends in place; `ys = xs.push(v)` copies.** Written as
  a copy it is quadratic. A test that asserts only answers cannot see the
  difference, and reading `xs.length()` marks the list aliased, which switches
  the optimisation off inside its own test. Assert on
  `alloc_count()`/`reuse_count()` and run with `MACA_POISON=1`.

## Self-hosting

There is no Rust left. The compiler is `apps/maca1/main.maca` plus
`modules/maca/*`, and `bootstrap/maca.c` is the compiler as it emitted itself:
`cc -O1` on that file gives a working compiler, which rebuilds the tree to a
fixed point. Regenerate it with `maca apps/maca1/main.maca bootstrap/maca.c`
whenever the compiler changes, and check the fixed point before committing.
See `docs/BOOTSTRAP.md`.

The stage-1 **compiler pipeline already runs natively**: the Maca-written lexer
→ recursive-descent parser → recursive-`Expr` AST + pretty-printer → type
checker (`check.maca`, an `Env` of signatures / record fields / sum variants /
locals: result types, arity, field types, declared returns) → **two back ends
written in Maca**, `emit_c.maca` and
`emit_rust.maca` (the `--target rust` backend, mirroring `maca-backend-rust` on
the parsed slice). These compile through the stage-0 C backend and execute
the whole `lex → parse → check → emit` chain (the `selfhost.rs` run gate
builds it with the host `cc`, then compiles the *emitted* program two ways: the
C capstone with `cc` and the Rust capstone with `rustc`, checking both exit
`42`). New backend
work belongs here in Maca. The parsed slice has grown
well past a toy: the full primitive expression language (`int`/`float`/`str`/
`bool` literals; `+ - * / %`, comparison, `&& ||`, unary `- !`; ternary;
multi-arg + nested calls with precedence), **type-threaded signatures** (a
parameter/return/local type flows to `const char*`/`double`/`bool`/… in C and
`String`/`f64`/… in Rust), and Maca's core data model: **records** (`Point =
{ x: int }` → `typedef struct`/`struct`, literals, field access) and **nullary
sum types + `match`** (`Color = Red | Green` → `typedef enum`/`enum` with `use
Color::*`; `match` lowers to a C ternary chain / a native Rust `match`). Each
increment is added in Maca and gated by a dual-backend compile+run of the
emitted program. The two backend features that
enabled it are shared by the whole language: the `str` scan builtins
(`chars`/`length`/`at`/`get`/`slice` + `is_whitespace`/`is_ascii_digit`/
`is_alpha`) and **recursive record types** (a self-referential field like
`Expr { children: Expr[] }` is forward-declared in C so the struct/array
definition cycle resolves, with `MACA_ARRAY_STRUCT` before the body and
`MACA_ARRAY_OPS` after).

## How to work here

- **Test-gated.** Nothing lands until every `.maca` suite is green and
  `apps/lint/lint.maca` is quiet. A change that can be observed at run time gets a test that runs it;
  documentation that makes a runnable claim gets one too (`apps/examples/handbook.maca`
  is the book's claims as an executable program).
- **Assert in Maca, not in Rust.** A behaviour test is a file of `test_…`
  functions using `assert`/`assert_eq`, run by `maca test`, which reports each
  one and exits with the failure count. The Rust side is a runner that checks
  the exit code. Do *not* write a Maca program that `info(…)`s its results and
  a Rust test that greps stdout for them, and do not embed the Maca source in a
  Rust string literal. The suites live in `modules/<pkg>/tests/` and
  `tests/programs/`. What used to stay in Rust is about the
  *process* rather than the values: piping stdin, running under valgrind, and a
  program that must fail to compile.
- **Break it to prove the test works.** A test written after the fix usually
  passes before it too. Mutate the code the test claims to cover: delete the
  branch, flip the comparison, remove the feature outright, then confirm the
  test goes red. Ten mutations against the append analysis left six green,
  including deleting the whole optimisation.
- **The code carries no comments.** No `//` line comment, no `//!` inner doc,
  no `/* … */` block, in any `.maca` file. A name, a
  signature or a test says what a comment used to. When you want to explain
  something, the options in order are: rename it, split it, or write a test
  whose name is the sentence you were about to write. Prose about the design
  belongs in the handbook (`apps/tomo/book/**`) or `docs/SPEC.md`, which are
  documentation and are not covered by this rule.
- **A doc comment is `///` or a `/** … */` block, and MacaDoc reads both.**
  A block sheds the `*` that lines it up, and a line inside either form that
  opens with `@` is a tag rather than a sentence: `@param a = slider(1..10)`
  is a control the editor draws, not prose the reference prints. `/* … */`
  without the second star is an ordinary comment.
- **`///` is the exception, and it is one line.** `///` is what marks an item
  as API: `apps/macadoc/macadoc.maca` builds the reference pages from it, and
  `tests/programs/sitegen.maca` fails when those pages and
  `modules/std/README.md` disagree, so deleting a `///` deletes a feature.
  Write exactly one line, a complete English sentence summarising the item. No
  second line and no second paragraph. Two things are allowed past that because
  a tool demands them: a `# Safety` section, which clippy requires on a public
  `unsafe fn`, and the leading `//` blurb of a `modules/std` file, whose first
  line is what the generated index prints on that module's card.
- **One surviving line, where absence would mislead.** A single `//` line may
  stay only where deleting it would leave the code actively misleading, and
  `tests/programs/docfixture.maca` is exempt outright because its
  comments are the test data `sitegen.maca` asserts on. Neither is an opening
  to keep prose you are fond of.
- **Readable over clever.** `if`/`else if`/`else` works in value position on
  every back end, so a multi-way choice is a chain of guards with each condition
  beside the branch it selects. A ternary is for one binary choice.
- **Native is hybrid.** C backend is the default for everything; the LLVM path
  exists **only** for the SIMD span. Both link over the C ABI.
- **Golden examples are the regression set.** The code blocks in the spec become
  `apps/examples/*.maca` verbatim; `apps/examples/bad/*.maca` must be rejected with the
  right diagnostic.
- The spec wins ties. If a design must change, update `docs/SPEC.md` and the
  code together.

## Gotchas

- **Native/Nix builds go through WSL (NixOS).** Windows side has no `zig`/`nix`;
  WSL is NixOS with `nix`/`nix-instantiate` on PATH. The native path gets a C
  compiler via `wsl nix shell nixpkgs#zig -c zig cc ...`; config mode uses
  `wsl nix-instantiate`. Paths cross the boundary: a Windows path `C:\x` is
  `/mnt/c/x` in WSL (translate when shelling out).
- **Maca surface syntax** (matters when writing `.maca` fixtures): no `fn`, no
  `type`, no `Result`/`Ok`, no `<>` generics, **no `async` keyword** (async is a
  colorblind, inferred effect; see below). Field `:` = type, `=` = value.
  Bracketless comma lists. Ternary is spaced `c ? x : y`; error-propagate is
  attached `x?`. `main() -> int`.

## Gotchas

- **Native and Nix builds go through WSL (NixOS).** The Windows side has no
  `zig` or `nix`; WSL is NixOS with both on PATH. A Windows path `C:\x` is
  `/mnt/c/x` in WSL, so translate when shelling out.
- **Maca surface syntax**, which matters when writing `.maca` fixtures: no
  `fn`, no `type`, no `Result`/`Ok`, no `<>` generics, and **no `async`
  keyword**, because async is a colorblind inferred effect. Field `:` is a
  type and `=` is a value. Bracketless comma lists. The ternary is spaced,
  `c ? x : y`; error-propagate is attached, `x?`. `main() -> int`.
- **Absence is a named variant, never an optional.** There is no `null` and no
  `??`.

## Where the rest of this lives

`.claude/skills/macalang/SKILL.md` is the skill for writing Maca: the rules a
model gets wrong, and the habit of verifying with `maca.check`.


# Writing Maca

Maca is one typed language for programs **and** infra config. Minimal,
rule-based syntax, with no idioms to memorize. Verify with `maca.check` before
finishing.

## The rules models get wrong

- **Every quoted import is named by its string, never by a keyword.** The
  string says what it is: an extension (`import "theme.css"`, `import
  "sqlite3.h"`, `import "json.py"`), a `::` path or bare crate name for Rust
  (`import "gpui::div"`, `import "itoa"`), a dotted class for the JVM (`import
  "net.fabricmc.api.ModInitializer"`). An inline block is named too, so the
  extension still decides: `import "scanner.c" """…"""`. A language word in
  front (`import c "…"`) is a parse error. To take names out of a package,
  `import { pick_text } from "npm:pkg"`; a snake_case name binds a camelCase
  export.

- **No `fn`, no `let`, no `return`, no `type`, no `Result`/`Ok`, no `<>`
  generics.** Functions are `name(x: T) -> R { body }` or `=> expr`, and the
  last expression is the value. Types are declared by binding:
  `Status = Todo | Doing | Done`, `Task = { id: int, title: str }`.
- **Field `:` = type, `=` = value.** `Name { field = value }` constructs;
  `{ field: Type }` declares a record type.
- **Spaced `? :` is the ternary; attached `x?` propagates an error.** They are
  different tokens: `c ? x : y` vs `load()?`.
- **Bracketless comma lists:** `xs = a, b, c` (not `[a, b, c]`, though `[]`
  brackets are used for empty/nested lists).
- **A `"…"` string may not span a line.** Write `\n`, or use `"""…"""`, which
  spans lines and does not interpolate. A `{` inside a string opens an
  interpolation, so a literal brace is `\{` or `{{`.
- **A binary operator may not begin a continuation line.** Put the `+` at the
  end of the line above; starting a line with one is a parse error, and in a
  file that is *imported* it used to fail silently.
- **`main() -> int`** for CLIs; `main() -> Element` for UI.
- **`match` must be exhaustive** over a sum type, or include `_`. Arms are
  separated by newlines, not commas.
- **`xs.push(v)` returns a new list.** Append with `xs = xs.push(v)`.
- **`lo..hi` is half-open**: it runs from `lo` up to but not including `hi`, so
  `0..xs.length()` is every index of `xs` and needs no `- 1`.
- **Config mode is pure `<>`**: no effects (`info`, file I/O, …) in a NixOS/
  home-manager module.

## Types worth knowing

A generic names its own element type: `first(xs: a[]) -> a`,
`sort_by(xs: a[], key: (a) -> str) -> a[]`. A function type is written
`(T, U) -> R` with the parens required, and you only need it for a record
field: a function *passed* as an argument needs no annotation.

## Modules

A path is the whole name. `modules/http/server.maca` is `http/server`, from
anywhere in the tree. There is no entry file and no index, and a directory is
not a module. `import { listen } from http/server` pulls in only what you name.
The packages are `std` (`text`, `list`, `path`, `json`, `csv`, `fs`, `proc`),
`http`, `chaski`, `ai` (`ai/openai`), `tambo`, `cli`, `bench`, `profile` and
`signal`.

**A package's `[build] target` is its own default, never a rule for whoever
imports it.** What you import is Maca, not the language that package happens to
be built as, so a `--target rust` program may import a package whose manifest
says `elixir` and the module is emitted as Rust along with everything else.
There is no bridge and no FFI in between, because there is nothing to bridge.

## The verify habit

Always run the `maca.check` MCP tool (or `maca lint`) on what you wrote and fix
every diagnostic before finishing. Diagnostic kinds: `type-mismatch`,
`non-exhaustive`, `effect-in-config`, `unknown-option`, `immutable`,
`undefined-name`.

For a **config module**, say so: `maca.check` with `config: true`, or
`maca lint --config`. `effect-in-config` and `unknown-option` only exist in
config mode, and nothing about a file says which mode it is for, so a config
module checked as a program comes back clean when it is not.

Behaviour goes in a `test_…` function checked with `assert`/`assert_eq` and run
by `maca test <file>`, which reports each one and exits with the failure count.
Do not print results and read the output back.

## Skeletons

CLI:

```maca
main(args: str[]) -> int {
    match args {
        "hello", ..rest => info("hi")
        _               => info("usage: app hello")
    }
    0
}
```

Typed program:

```maca
Shape = Circle | Square

area_name(s: Shape) -> str => match s {
    Circle => "circle"
    Square => "square"
}
```

A test file:

```maca
greet(name: str) -> str => "hi, {name}"

test_a_name_is_greeted() {
    assert_eq(greet("Mia"), "hi, Mia", "the name is used verbatim")
}

main() -> int {
    test_a_name_is_greeted()
    failures()
}
```

Config module (pure):

```maca
system.stateVersion = "24.11"
services.openssh = {
    passwordAuthentication = false
}
```
