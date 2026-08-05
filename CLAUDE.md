# Maca

One typed language for **programs and infrastructure config**. Programs compile
to native (C-tier binary), JS, JVM, Rust, or freestanding C; config compiles to
Nix. There is no BEAM back end, by design. Everything a user writes is `.maca`
or `maca.toml`.

**Read `docs/SPEC.md` before starting work.** It is the authority on the
language; when it and the code disagree, it wins, and they change together.

## Commands

```
cargo build            # build the whole workspace
cargo test             # run all tests
cargo run -p maca-driver -- --version    # the maca CLI
cargo test -p maca-lexer                 # test one crate
```

The CLI binary is `maca`, in `crates/driver`.

**Testing gotcha:** the integration tests that build natively (through `zig` or
`nix` under WSL) contend under heavy parallelism, because neither takes a dozen
concurrent invocations. A cross-process build lock serialises the compiles, but
for a fully reliable full run use `cargo test -- --test-threads=1`, or run
per-crate. The pure unit tests are unaffected and fast.

## Where things are

The tree is four directories, and each answers one question:

| | |
|---|---|
| `modules/` | what a program imports. Eight packages, all ordinary Maca, all with a suite under `<pkg>/tests/`. They ride **inside** the `maca` binary. |
| `apps/` | everything this repository builds: the capstones, the demos, the toolchain's own programs, the handbook builder, the playground, the compiler written in Maca, and `apps/examples`, which is the regression corpus. |
| `crates/` | the frozen Rust stage-0 bootstrap, plus `crates/install`, the installer. Keep it minimal. |
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

Rust (`crates/*`) is the **frozen stage-0 bootstrap**, so keep it minimal. New
compiler work is written in Maca under `apps/selfhost/` and gated by the stage-0
front-end (`crates/driver/tests/selfhost.rs`). See `docs/BOOTSTRAP.md`. When a
change is needed, prefer adding it to `apps/selfhost/*.maca` over growing the Rust
crates; only touch stage-0 for genuine bootstrap bugs (e.g. a parser that hangs
or mis-parses valid surface syntax).

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
work belongs here in Maca, not in the stage-0 crates. The parsed slice has grown
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

- **Test-gated.** Nothing lands until `cargo test` is green across the
  workspace. A change that can be observed at run time gets a test that runs it;
  documentation that makes a runnable claim gets one too (`apps/examples/handbook.maca`
  is the book's claims as an executable program).
- **Assert in Maca, not in Rust.** A behaviour test is a file of `test_…`
  functions using `assert`/`assert_eq`, run by `maca test`, which reports each
  one and exits with the failure count. The Rust side is a runner that checks
  the exit code. Do *not* write a Maca program that `info(…)`s its results and
  a Rust test that greps stdout for them, and do not embed the Maca source in a
  Rust string literal. The suites live in `modules/<pkg>/tests/` and
  `crates/driver/tests/programs/`. What stays in Rust is what is about the
  *process* rather than the values: piping stdin, running under valgrind, and a
  program that must fail to compile.
- **Break it to prove the test works.** A test written after the fix usually
  passes before it too. Mutate the code the test claims to cover: delete the
  branch, flip the comparison, remove the feature outright, then confirm the
  test goes red. Ten mutations against the append analysis left six green,
  including deleting the whole optimisation.
- **The code carries no comments.** No `//` line comment, no `//!` inner doc,
  no `/* … */` block, in `crates/**/*.rs` or in any `.maca` file. A name, a
  signature or a test says what a comment used to. When you want to explain
  something, the options in order are: rename it, split it, or write a test
  whose name is the sentence you were about to write. Prose about the design
  belongs in the handbook (`apps/tomo/book/**`) or `docs/SPEC.md`, which are
  documentation and are not covered by this rule.
- **`///` is the exception, and it is one line.** `///` is what marks an item
  as API: `apps/macadoc/macadoc.maca` builds the reference pages from it, and
  `crates/driver/tests/programs/sitegen.maca` fails when those pages and
  `modules/std/README.md` disagree, so deleting a `///` deletes a feature.
  Write exactly one line, a complete English sentence summarising the item. No
  second line and no second paragraph. Two things are allowed past that because
  a tool demands them: a `# Safety` section, which clippy requires on a public
  `unsafe fn`, and the leading `//` blurb of a `modules/std` file, whose first
  line is what the generated index prints on that module's card.
- **One surviving line, where absence would mislead.** A single `//` line may
  stay only where deleting it would leave the code actively misleading, and
  `crates/driver/tests/programs/docfixture.maca` is exempt outright because its
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
