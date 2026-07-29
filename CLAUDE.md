# Maca

A single typed language for **programs and infrastructure config**. General
programs compile to native (C-tier binary), JS, JVM, Rust, or freestanding C;
infra config compiles to Nix. There is no BEAM backend, by design — see the
targets chapter. The compiler is Rust (hand-written lexer + recursive-descent
parser, no parser library); everything a user writes is `.maca` or `maca.toml`.

`docs/SPEC.md` is the authoritative spec. Read it before starting work — it
holds the language cheatsheet and the effect rows.

## Commands

```
cargo build            # build the whole workspace
cargo test             # run all tests
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
| `maca-parser` | tokens → AST (hand-written recursive descent + Pratt); also `modules`/`imports` — which file an `import` names, and inlining it |
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
| `maca-testsupport` | host probes + the cross-crate build lock, for the integration suites |
| `maca-backend-jvm` | core IR → Java source (JVM interop; Minecraft/Fabric) |
| `maca-backend-rust` | core IR → Rust source (crates.io interop; `--target rust`) |
| `maca-backend-embedded` | freestanding C for bare-metal MCUs (Cortex-M/RISC-V) |
| `maca-wasm` | `wasm32` front-end for the browser playground (no wasm-bindgen) |

Non-crate dirs: `tools/` (Maca-written ports of the toolchain — `bindgen.maca`,
kept equivalent to its stage-0 Rust twin by `crates/driver/tests/bindgen_port.rs`,
`lint.maca`, a style linter that walks the tree recursively and checks line
width / single-line `if` / trailing whitespace / hard tabs — width is measured
with string literals collapsed, so a long C template or URL is exempt exactly
as a long comment is. Gated by `crates/driver/tests/lint_port.rs`, which
requires the whole repository to pass it — `macadoc.maca`, the API-doc
generator (rustdoc's job for Maca: a `///` block above an item is what makes it
API, an ordinary `//` explains a helper to the next reader; the Reference's
tooling chapter documents the marker, and
`crates/driver/tests/programs/sitegen.maca` fails if what it lists ever differs
from what `modules/std/README.md` advertises) — and
`build-site.maca`, which builds and checks the published site for both CI and a
human, including a check that every class on every emitted page produced a CSS
rule).
**Every script in the repository is a Maca program**: `apps/bench/run.maca` is
the cross-language benchmark harness, `packages/macalang/build.maca` builds the
wasm into the npm package. All seven are compiled by
`crates/driver/tests/scripts.rs`, because a script only run at release time
rots quietly. The two exceptions are `install.sh` and `install.ps1`, which run
*before* there is a `maca` to run anything with.
**Maca code lives under `modules/`, `apps/` and `src/`.** `modules/*` and
`src/*` are import search roots, so `modules/std/text.maca` is written
`std/text` and `modules/http/server.maca` is written `http/server` — from
anywhere in the tree. So is `maca_modules/`, where `maca add` installs a
dependency, which is why the directory it chose never appears in anybody's
source. `apps/*` is deliberately *not* a root: two apps may each have a `conf`,
and neither should silently answer for the other, so an application is reached
by its written path (`import apps/tomo/conf`). `[layout]` in `maca.toml`
renames any of them.

**A directory that shares a package's name shadows the package**, silently. The
written path is tried before the search roots, so a top-level `bench/` beside
`modules/bench/` — which is what the tree had — decided `import bench/stat` by
which file happened to exist. Do not name a directory after a package; the
cross-language harness moved to `apps/bench/` for this reason.

Reordering `in_base` to try the roots first was tried and reverted, and the
reasons are worth knowing before anyone tries it again. It does not fix the
general case: the resolver walks the importer's ancestors, so
`apps/bench/report.maca` still answers for `bench/report` from anywhere under
`apps/`, whatever the order within one directory. And it costs more than it
buys — `maca_modules` is a search root, so roots-first lets an installed
dependency outrank the project's own source. Across the 229 imports in this
repository the two orderings agree on every one.

**There is no entry file and no index.** A directory is not a module; a path is
a path and that is the only thing it is. (An earlier design had a per-directory
`_init.maca` re-exporting its neighbours, and it cost more than it bought: two
names for every file, a second place to update when one moved, and an import
whose meaning depended on a file the reader never opened.)

**An import that resolves to no file is an error** — including a single-word
selective import, because there is nothing to select from a builtin. Four files
imported a `std/str` that has never existed, silently, and each then hand-wrote
the helpers it believed it was importing.

### The packages (`modules/*`)

Seven, all ordinary Maca source, all with a suite under `<pkg>/tests/` run by
`maca test`. Read `modules/cli/` first — it is the house standard for
structure and comment voice.

| package | what it is | tests |
|---|---|---|
| `std` | `text`, `list`, `path`, `json`, `csv`, `fs`, `proc` — the layer above the prelude builtins (`modules/std/README.md` is the reference, and `crates/driver/tests/programs/sitegen.maca` fails if it and the generated API pages disagree) | 106 |
| `http` | the server — `server`/`request`/`response`/`status`, plus `serve`, which `maca -m http.serve` runs | 16 |
| `cli` | argument parsing and terminal output in one — `spec` (a command as a value), `parse`, `help` (the page rendered from the spec), `show` (tables, rules), `style` (colour, and UTF-8 *column* widths, so a Hangul or emoji cell still lines up) | 36 |
| `bench` | `time` (calibrating measurement loop), `stat`, `store` (JSON round-trip), `compare` (two runs, with a verdict), `cases/*` (a corpus from primitives to advanced algorithms) | 141 |
| `profile` | `record` (spans), `trace`, `flame` (text and SVG charts), `play` (trace playback), `report` | 75 |
| `signal` | nanostore-style reactive state — `store` (signals, computed, effects) and `dom`, so a native web page updates the nodes that changed rather than re-rendering | 58 |
| `tambo` | the web framework over `http` — `app`, `route`, `ctx`, `dispatch`, `reply` | 87 |

`examples/` (golden `.maca` programs + `examples/bad/`, plus one `X_demo.maca`
per package), `apps/` (capstones: `mqtt`, `microkernel`, `blink`, `desktop`,
`mcmod`, `bench` — the cross-language harness and its C/Rust/Go/JS/Python
reference kernels — `playground` (the browser playground, a single `.maca`
file compiled by the JS backend), `tomo` — the i18n handbook builder that
renders `book/{en,ko}/*.md` into `site/`, built entirely out of the UI syntax
below plus one line of hand-written CSS; it is also the worked example of that
syntax, so keep it free of hand-concatenated markup — and `site`, the project's
front page, `home.maca`, whose copy is keyed by sum types so a translation that
drops a card is a NonExhaustive error rather than a shorter page),
`selfhost/` (the Maca compiler written in Maca — stage 1), `editor/`, `docs/`.

**The handbook is two volumes with one table of contents** (`apps/tomo/book.toml`):
*Learning Maca*, read front to back once, and *The Reference*, opened at the
page you need. A chapter belongs to exactly one of them, and a teaching chapter
with a stricter twin links to it by name. The `a`-prefixed files are the
Reference — they grew out of the appendices and kept the prefix so no published
URL had to move. Maca in a fenced block is highlighted by
`apps/tomo/highlight.maca`, a scanner that follows `crates/lexer/src/lib.rs`
rather than guessing; an unknown language tag falls through to escaped plain
text.

FFI (`import c "sqlite3.h"` / `import py "…"`) links the real library: through
`wsl nix` when present, else the **host `cc`** with system headers/libs
(`-lsqlite3`, `python3-config`), so FFI builds on a plain Linux dev machine
(`examples/ffi_sqlite.maca` iterates a real result set).

## Self-hosting

Rust (`crates/*`) is the **frozen stage-0 bootstrap** — keep it minimal. New
compiler work is written in Maca under `selfhost/` and gated by the stage-0
front-end (`crates/driver/tests/selfhost.rs`). See `docs/BOOTSTRAP.md`. When a
change is needed, prefer adding it to `selfhost/*.maca` over growing the Rust
crates; only touch stage-0 for genuine bootstrap bugs (e.g. a parser that hangs
or mis-parses valid surface syntax).

The stage-1 **compiler pipeline already runs natively**: the Maca-written lexer
→ recursive-descent parser → recursive-`Expr` AST + pretty-printer → type
checker (`check.maca`, an `Env` of signatures / record fields / sum variants /
locals — result types, arity, field types, declared returns) → **two back ends
written in Maca** — `emit_c.maca` and
`emit_rust.maca` (the `--target rust` backend, mirroring `maca-backend-rust` on
the parsed slice) — compile through the stage-0 C backend and execute the whole
`lex → parse → check → emit` chain (the `selfhost.rs` run gate builds it with the
host `cc`, then compiles the *emitted* program two ways: the C capstone with
`cc` and the Rust capstone with `rustc`, checking both exit `42`). New backend
work belongs here in Maca, not in the stage-0 crates. The parsed slice has grown
well past a toy: the full primitive expression language (`int`/`float`/`str`/
`bool` literals; `+ - * / %`, comparison, `&& ||`, unary `- !`; ternary;
multi-arg + nested calls with precedence), **type-threaded signatures** (a
parameter/return/local type flows to `const char*`/`double`/`bool`/… in C and
`String`/`f64`/… in Rust), and Maca's core data model — **records** (`Point =
{ x: int }` → `typedef struct`/`struct`, literals, field access) and **nullary
sum types + `match`** (`Color = Red | Green` → `typedef enum`/`enum` with `use
Color::*`; `match` lowers to a C ternary chain / a native Rust `match`). Each
increment is added in Maca and gated by a dual-backend compile+run of the
emitted program. The two backend features that
enabled it are shared by the whole language: the `str` scan builtins
(`chars`/`length`/`at`/`get`/`slice` + `is_whitespace`/`is_ascii_digit`/
`is_alpha`) and **recursive record types** (a self-referential field like
`Expr { children: Expr[] }` is forward-declared in C so the struct/array
definition cycle resolves — `MACA_ARRAY_STRUCT` before the body,
`MACA_ARRAY_OPS` after).

## How to work here

- **Test-gated.** Nothing lands until `cargo test` is green across the
  workspace. A change that can be observed at run time gets a test that runs it;
  documentation that makes a runnable claim gets one too (`examples/handbook.maca`
  is the book's claims as an executable program).
- **Assert in Maca, not in Rust.** A behaviour test is a file of `test_…`
  functions using `assert`/`assert_eq`, run by `maca test`, which reports each
  one and exits with the failure count. The Rust side is a runner that checks
  the exit code. Do *not* write a Maca program that `info(…)`s its results and
  a Rust test that greps stdout for them, and do not embed the Maca source in a
  Rust string literal — the suites live in `modules/<pkg>/tests/` and
  `crates/driver/tests/programs/`. What stays in Rust is what is about the
  *process* rather than the values: piping stdin, running under valgrind, and a
  program that must fail to compile.
- **Break it to prove the test works.** A test written after the fix usually
  passes before it too. Mutate the code the test claims to cover — delete the
  branch, flip the comparison, remove the feature outright — and confirm the
  test goes red. Ten mutations against the append analysis left six green,
  including deleting the whole optimisation.
- **Readable over clever.** `if`/`else if`/`else` works in value position on
  every back end, so a multi-way choice is a chain of guards with each condition
  beside the branch it selects. A ternary is for one binary choice. Prefer code
  that reads to a comment explaining code that doesn't.
- **Native is hybrid.** C backend is the default for everything; the LLVM path
  exists **only** for the SIMD span. Both link over the C ABI.
- **Golden examples are the regression set.** The code blocks in the spec become
  `examples/*.maca` verbatim; `examples/bad/*.maca` must be rejected with the
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
  colorblind, inferred effect — see below). Field `:` = type, `=` = value.
  Bracketless comma lists. Ternary is spaced `c ? x : y`; error-propagate is
  attached `x?`. `main() -> int`.

## Status

The compiler is complete and `cargo test` is green across the workspace. Whole
programs (non-`main` functions, records→structs, sum types→tagged enums, lists,
string interpolation, `match` lowering incl. list patterns (bracketless `x, ..rest` or bracketed
`[]`/`[x]`/`[x, ..rest]`), UFCS) compile and
run end-to-end (parse → check → C → `cc`/`zig cc` → execute).

**UI syntax on every target.** A tag name called as a function is an element:
named args → attributes, positional args → children (comma-separated *or*
juxtaposed). The JS backend builds a reactive DOM; the **C backend renders the
same call to an HTML string** (`maca_concat` chain; `maca_attr` escapes
attribute values, children are not re-escaped; void elements self-close;
`on:click=` is a clean error pointing at `--target js`). A user definition or
local **always shadows a tag** — `label`/`code`/`main`/`p` are tags and ordinary
names. A **hyphenated attribute needs no workaround** — an attached `-` is part
of an identifier and a spaced one is the operator (the same attached-vs-spaced
rule as `x?`/`c ? x : y`), so `nav(data-tomo="toc", span("{a - b}"))` is one
attribute and one subtraction. Two forms an identifier alone can't express: a
**bool** attribute controls the attribute's
*presence* (`open=true` → `<details open>`, `hidden=false` → nothing —
`maca_flag`); and `element(tag, …)` takes the **tag as a value**
(`element("h" ++ n, …)`, `th`/`td` per row, and `<main>`, which no call can
name) — voidness is decided in `maca_element` at run time. `styles()` is the
generated stylesheet for exactly the Tailwind utilities the module's `class=`
strings mention (collected module-wide, *not* inside raw `"""…"""` strings —
markup in a raw block silently gets no rules). Variants:
`hover/focus/active/first/last/open/before/after/marker/placeholder/details-marker`
(selector suffix) and `dark/sm/md/lg/xl/max-sm/max-md/max-lg` (`@media`),
combinable; arbitrary values `[…]` with `_`→space; selectors are `css_escape`d
(an unescaped `.max-w-[42rem]` is dropped by browsers *silently*) and rules are
emitted in `order()` so a variant beats the utility it modifies. Documented in
handbook ch. 15 (`apps/tomo/book/{en,ko}/15-ui.md`); gated by
`crates/driver/tests/native_ui.rs` + `crates/backend_js/tests/tailwind.rs`.

Backends: native C (default), LLVM (SIMD span), Nix (config mode), JS
(reactive UI), JVM (Java source / Minecraft-Fabric interop), and embedded
(freestanding C for Cortex-M / RISC-V). Driver: `build` (`--target
nix|js|jvm|rust|embedded|tauri`, `--mcu`, `--cp`), `run`, `-m` (run a function
out of a module — `maca -m http.serve`; the spec reads as either
module + function or a whole path run under its own name, the exit status comes
from the entry point's declared return type, and a `str[]` parameter receives
the leftover command line), `dev` (dev-shell flake),
`watch`, `fmt`, `lint`, `test` (runs every `test_…` function in a file),
`profile`, `init`, `bindgen` (C header → Maca FFI
declarations: `char*`→`str`, other pointers→opaque `int`, float family→`float`).
The native `build`/`run` path
resolves local imports: `import a/b` (and single-word `import a`) inlines a
sibling `<a/b>.maca` / `<a>.maca` module, transitively, in dependency order, so
a program can span files (`maca build selfhost/main.maca` builds the whole
self-hosted front-end from its imports). **Selective import** —
`import { foo, bar } from a/b` — inlines only the named top-level definitions
plus the transitive closure of same-module definitions they reference (dead-code
elimination at the module boundary); a name the module doesn't define is a clean
error, not a dangling reference (`crates/driver/src/imports.rs`).

**Processes, no shell:** `exec(cmd, args) -> int` (the exit code) and
`capture(cmd, args) -> str` (its stdout) are `fork` + `execvp` — `args` is a
`str[]` and each element is one argument however it is spelled, so
`exec("cp", ["my notes.txt", dest])` copies one file and `exec("echo",
["$HOME"])` prints `$HOME`. With `env`/`cwd`/`chdir` and `copy_bytes(src, dst)`
(a byte copy — `write_file(read_file(…))` stops at the first NUL, so it
truncates any binary), that is what lets every script in the repository be a
Maca program. `std/proc` is the layer above: `run`, `try_run`, `run_in`,
`output`, `which`/`have`, `env_or`.

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
exempt via `maca_parser::is_backend_intrinsic`). `UndefinedName` also covers
**phantom keywords** (`return`/`let`/`type`/`null` — each answered with what
Maca does instead) and a **misspelt UFCS method on a known receiver**: the
method sets of `str` and `T[]` are closed (`maca_core::STR_METHODS` /
`LIST_METHODS`), so `s.slice(…)` is a diagnostic with a `did you mean` rather
than an `undefined reference` from the linker. `any` receivers stay gradual.
The two lists are executed, not trusted — `crates/driver/tests/method_sets.rs`
compiles and runs every name in them. Function
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
str via `intptr_t`, float bit-preserved via `maca_box_f64`); **higher-order
parameters** — a top-level function referenced by name is a function value
(wrapped in a `maca_closure` via a hoisted boxing thunk), and an unannotated
parameter that is *called* in the body is typed as a closure, so
`run_end(cs, i, is_alpha)` / `pred(cs.get(i))` work with no function-type
syntax (native + interpreter); a **list stdlib**
as UFCS methods on any `T[]` (`map`/`filter`/`reduce`/`fold` take closures typed
by the element; `sort`/`reverse`/`push`/`pop`/`contains`/`index_of`/`sum`/`min`/
`max`/`first`/`last`/`length`/`get`/`slice`; native + interpreter —
`examples/collections.maca`); plus the `str` scan primitives
`chars`/`length`/`at` and the char classes `is_whitespace`/`is_ascii_digit`/
`is_alpha` (what `selfhost/lexer.maca` scans with);
and raw triple-quoted strings (`"""…"""`) with `import js`/`import css` foreign
blocks that let a `.maca` UI carry its own host glue and styles inline (see
`apps/playground/playground.maca`). Examples:
`examples/{indexing,record_update,tree,sum_record,keywords,strings}.maca`.

**A function can be kept in a record field**, declared `(T, U) -> R` — the
parens are required, and this is the only place a function type is written
down, because a field is declared before anything calls it. A function *passed*
still needs no annotation: an unannotated parameter that is called in the body
is one. That is what makes a route table, a reducer, or a builder expressible
(`crates/driver/tests/programs/function_fields.maca`). The `rust` and `jvm`
emitters reject a function field with a clean diagnostic rather than emitting
something that will not compile.

**A generic can name its own element type**: `first(xs: a[]) -> a`,
`sort_by(xs: a[], key: (a) -> str) -> a[]`. A call binds `a` by looking *into*
the argument's type, not only at a parameter written as a bare variable, and
the body is lowered knowing what `a` turned out to be — so a local declared
`a[]` inside a generic gets the concrete element type instead of the fallback
array (`crates/driver/tests/programs/generics.maca`).

`is_tty()` answers whether stdout is a terminal, which is how `cli/style`
decides to emit colour.

**Strings:** `{` opens an interpolation, so a literal brace is `\{`/`\}` or
`{{`/`}}`. A `"…"` string may not span a line (write `\n`, or use `"""…"""`,
which spans lines and interpolates nothing) — without that rule a stray `"{"`
opened an interpolation the quote never closed and the file silently
mis-compiled. An interpolation may carry a **format spec**:
`{x:.2}`, `{x:>8}`, `{x:<8}`, `{x:^8}`, `{x:08}`, `{x:>10.3}` —
`[align][0][width][.precision]`, all parts optional. It is pure sugar,
desugared in the parser (`apply_fmt_spec`) to `x.fixed(n)` /
`str(x).pad_start(w, p)` / `pad_end` / `pad_center`, so every back end gets it
for free. A spec's `:` is *attached* and a ternary's is *spaced*, which is how
the lexer tells `{x:>8}` from `{c ? a : b}` (`Tok::FmtSpec`, `fmt_spec_here`) —
the same attached-vs-spaced rule as `x?` vs `c ? x : y`. New primitives behind
it: `float.fixed(n) -> str` (int receiver widened) and `str.pad_center(w, p)`.

**Memory (Perceus RC, C backend).** Two invariants hold the string and list
handling together, and both are easy to break from inside `maca-runtime` or
`crates/backend_c/src/ownership.rs`.

*Every `maca_str`-returning runtime function returns a fresh block or a static
literal, never one of its arguments.* `maca_str_copy` exists for the cases that
would otherwise hand an argument back — `maca_replace` with nothing to replace,
`maca_split` on an empty separator, `maca_pad` already wide enough. A borrowed
return is a double free that only shows up under a load the tests don't reach.

*`xs = xs.push(v)` appends in place; `ys = xs.push(v)` copies.* A list is a
value, so the copy is the rule and assigning back to the same name is the one
case where the old value is unreachable the moment the new one exists. Written
as a copy it is quadratic — eight thousand elements took half a second and left
every intermediate buffer behind. `ownership::appendable_names` decides this
per *function*, not per block, and excludes parameters (a parameter is a second
handle by construction — appending in place reallocates a list the caller still
holds), `for` pattern variables, and anything aliased. `emit_specialization` and
`emit_closure` save and restore it, or a specialization bypasses the analysis
entirely. Every one of those exclusions was a wrong answer before it was a rule;
`crates/driver/tests/programs/accumulate.maca` is one test per shape.

**A test that asserts only answers cannot detect this.** An answer is identical
whether the list was copied or appended to, and `assert_eq(str(xs.length()), …)`
marks the list aliased, which switches the optimisation off inside its own test.
Assert on `alloc_count()`/`reuse_count()`, read elements through interpolations,
and read them *after* enough rounds to force a reallocation. `MACA_POISON=1`
fills released blocks with `0xDD` so a use-after-free is a wrong answer rather
than a lucky one.

**Codegen note (C backend):** control-flow expressions (`if`/`match`/block)
work in value position via a `Sink` (Discard/Return/Assign) threaded through
`block`/`stmt_expr`/`match_stmt`; nullary enum-variant patterns lower to tag
tests (mirroring the checker's `is_variant`). `maca-runtime` holds the C
sources (`RUNTIME_H`/`RUNTIME_C`). The native driver compiles via `wsl nix
shell nixpkgs#zig -c zig cc … -target x86_64-linux-musl -static -s` when WSL is
present, else the host `cc`. Both paths cache the invariant runtime as a compiled
object (`build_cache::object`, keyed on runtime source + compiler + target), so a
*changed* program relinks against the cached `maca_runtime.o` instead of
recompiling the whole runtime — the zig path falls back to the original
all-in-one invocation if the cached-object link fails, so it can't regress.

Grammar decisions worth knowing (in `parser.rs`): `no_brace` mode in control
headers so `for x in xs {` isn't a ctor; fn-def detected by lookahead for
`-> | { | =>` after `)`; call args separated by comma **or** juxtaposition (UI);
lambda-body assign (`v => age = int(v)`).
