# Layout

Where everything is, and why it is there rather than somewhere else.

One package; the compiler is `apps/maca1`.

| crate | role |
|---|---|
| `maca-lexer` | significant-newline tokenizer |
| `maca-parser` | tokens → AST (hand-written recursive descent + Pratt); also `modules`/`imports`, which decides which file an `import` names, and inlines it |
| `maca-core` | typed core IR + HM/gradual/row type & effect checker |
| `maca-backend-c` | core IR → C (default native path) |
| `maca-backend-nix` | config mode → `.nix` |
| `maca-backend-js` | core IR → JS + reactive UI + Tailwind |
| `maca-runtime` | Perceus RC + colorblind async (C runtime sources) |
| `maca-stdlib` | the `modules/*` packages, compiled into the binary and unpacked on demand |
| `maca-options` | `options.json` → Maca option types |
| `maca-lsp` | language server: `lib.rs` (analysis fns) + `main.rs` (LSP stdio server) |
| `maca-driver` | the `maca` CLI |
| `maca-testsupport` | host probes + the cross-crate build lock, for the integration suites |
| `maca-profile` | the flame-graph renderer, shared by `maca profile` and the playground |
| `maca-backend-jvm` | core IR → Java source (JVM interop; Minecraft/Fabric) |
| `maca-backend-rust` | core IR → Rust source (crates.io interop; `--target rust`) |

**The toolchain's own programs are applications, because that is what they
are.** A runnable program lives under `apps/` whoever wrote it and whoever runs
it, so `apps/bindgen/` is kept equivalent to its
stage-0 Rust twin by `modules/maca/tests/tooling.maca`; `apps/lint/` is a
style linter that walks the tree recursively and checks line width /
single-line `if` / trailing whitespace / hard tabs (width is measured with
string literals collapsed, so a long C template or URL is exempt exactly as a
long comment is, and `modules/maca/tests/tooling.maca` requires the whole
repository to pass it); `apps/macadoc/` is the API-doc generator (rustdoc's job
for Maca: a `///` block above an item is what makes it API, and the leading
`//` block of a `modules/std` file is that module's own blurb on the generated
index; the Reference's tooling chapter documents the marker, and
`tests/programs/sitegen.maca` fails if what it lists ever differs
from what `modules/std/README.md` advertises); and `apps/build_site/` builds
and checks the published site for both CI and a human, including a check that
every class on every emitted page produced a CSS rule.
**Every script in the repository is a Maca program**: `apps/bench/run.maca` is
the cross-language benchmark harness, `apps/npm/build.maca` builds the
wasm into the npm package. All seven are compiled by
`modules/maca/tests/tooling.maca`, because a script only run at release time
rots quietly. There are no shell scripts left to be an exception, and no Rust
ones either: installing is `apps/install/`, a Maca program shipped as a binary
in the release, which reads its own platform from `uname` at run time rather
than from how it was built. `apps/mcp/` is the MCP server, and it answers a
model by calling the toolchain rather than by holding a second copy of the
checker and the formatter.
**Maca code lives under `modules/`, `apps/` and `src/`.** `modules/*` and
`src/*` are import search roots, so `modules/std/text.maca` is written
`std/text` and `modules/http/server.maca` is written `http/server`, from
anywhere in the tree. So is `maca_modules/`, where `maca add` installs a
dependency, which is why the directory it chose never appears in anybody's
source. `apps/*` is deliberately *not* a root: two apps may each have a `conf`,
and neither should silently answer for the other, so an application is reached
by its written path (`import apps/tomo/conf`). `[layout]` in `maca.toml`
renames any of them.

**The standard library travels inside the binary.** All eight `modules/*`
packages are compiled into `maca` by the carried table in `apps/maca1/main.maca` (which reads
the tree, the way `maca-runtime` carries the C runtime), so `import std/json`
resolves in a project that has never seen this repository. It is **the last
thing asked**: the walk from the importing file up to the workspace root runs
first, over the written path and then the roots, so a project's own
`modules/std/text.maca` or an installed `maca_modules/std/` wins without any
rule being added for it, and inside this repository the tree is still what
every suite exercises. Because the compiler resolves source, the carried copy
is unpacked once into the cache directory (`~/.cache/maca/stdlib/<version>-<digest>`,
whole or not at all) and a file the compiler carries resolves *its* imports
against the project that asked for it, so a replaced file is replaced for the
carried packages that read it too. `MACA_STDLIB=<dir>` replaces the carried
copy outright. The snapshot cannot go stale: it is read out of `modules/` at
build time and `modules/maca/tests/imports.maca` compares the two file by
file, builds a program in a temp directory outside the repository, and checks
each precedence step. Documented in `docs/SPEC.md` and handbook ch. a9/a14.

**Every `modules/*` and `apps/*` is its own package with its own `maca.toml`,
and the root is the workspace that gathers them.** `[workspace] members` lists
them, and the list is checked against the tree both ways: a listed member with
no manifest is an error naming it, and a directory beside a member that holds a
manifest and is not listed is an error naming it too (a directory becomes a
package by writing a `maca.toml` and by nothing else, so a scratch directory is
never one). Precedence is one rule: **the nearest manifest that states a key
answers for it**, from the file's own directory up to the workspace root, which
is why a package states its `name` and inherits the workspace's `version`.
`[workspace]`, `[package]` and `[[bin]]` are identity rather than settings, so
they are read from one manifest, and every path a manifest writes is relative to
the directory it sits in. The one thing this changes about resolution is where
the walk stops: at the workspace root, not at the first `maca.toml`, or
`modules/std/text.maca` could no longer reach `modules/` to resolve
`import std/list`. With no file named, `maca build`/`run`/`test` are about the
package the working directory holds: its `[[bin]]` (`--bin` picks one of
several) and the suites under its `tests/`. Documented in `docs/SPEC.md` and
handbook ch. a14; gated by `modules/maca/tests/tooling.maca`.

**Building is declared, not flagged.** `[build]` carries the five things
`maca build` would otherwise learn only from a flag, each a property of the
project rather than of the invocation: `target`, `out` (`-o`), `mcu`,
`classpath` (`--cp`) and `bin` (which `[[bin]]` a bare `build`/`run` means). A
flag on the line wins over the manifest, a declared target wins over the one
`detect_target` would have guessed, `out` is relative to the manifest that
wrote it, and an unknown key there is an error naming it. So a project whose
manifest says `target = "js"`, `out = "build"` is built by `maca build`, with
no wrapper script. `maca init` writes exactly two files to match: a `maca.toml`
stating `[package] name` and the `[[bin]]` it builds, and that `main.maca`. No
comments, no `.gitignore`, no table the project has not needed yet
(`modules/maca/tests/commands.maca`).

**A directory that shares a package's name shadows the package**, silently. The
written path is tried before the search roots, so a top-level `bench/` beside
`modules/bench/`, which is what the tree had, decided `import bench/stat` by
which file happened to exist. Do not name a directory after a package; the
cross-language harness moved to `apps/bench/` for this reason.

Reordering `in_base` to try the roots first was tried and reverted, and the
reasons are worth knowing before anyone tries it again. It does not fix the
general case: the resolver walks the importer's ancestors, so
`apps/bench/report.maca` still answers for `bench/report` from anywhere under
`apps/`, whatever the order within one directory. And it costs more than it
buys: `maca_modules` is a search root, so roots-first lets an installed
dependency outrank the project's own source. Across the 229 imports in this
repository the two orderings agree on every one.

**There is no entry file and no index.** A directory is not a module; a path is
a path and that is the only thing it is. (An earlier design had a per-directory
`_init.maca` re-exporting its neighbours, and it cost more than it bought: two
names for every file, a second place to update when one moved, and an import
whose meaning depended on a file the reader never opened.)

**An import that resolves to no file is an error**, including a single-word
selective import, because there is nothing to select from a builtin. Four files
imported a `std/str` that has never existed, silently, and each then hand-wrote
the helpers it believed it was importing.

### The packages (`modules/*`)

Seven, all ordinary Maca source, all with a suite under `<pkg>/tests/` run by
`maca test`. Read `modules/cli/` first; it is the house standard for how a
package is laid out and how its `///` summaries are worded.

| package | what it is | tests |
|---|---|---|
| `std` | `text`, `list`, `path`, `json`, `csv`, `fs`, `proc`: the layer above the prelude builtins (`modules/std/README.md` is the reference, and `tests/programs/sitegen.maca` fails if it and the generated API pages disagree) | 106 |
| `http` | the server: `server`/`request`/`response`/`status`, plus `serve`, which `maca -m http.serve` runs | 16 |
| `cli` | argument parsing and terminal output in one: `spec` (a command as a value), `parse`, `help` (the page rendered from the spec), `show` (tables, rules), `style` (colour, and UTF-8 *column* widths, so a Hangul or emoji cell still lines up) | 36 |
| `bench` | `time` (calibrating measurement loop), `stat`, `store` (JSON round-trip), `compare` (two runs, with a verdict), `cases/*` (a corpus from primitives to advanced algorithms) | 141 |
| `profile` | `record` (spans), `trace`, `flame` (text and SVG charts), `play` (trace playback), `report` | 75 |
| `signal` | nanostore-style reactive state: `store` (signals, computed, effects) and `dom`, so a native web page updates the nodes that changed rather than re-rendering | 58 |
| `tambo` | the web framework over `http`: `app`, `route`, `ctx`, `dispatch`, `reply` | 87 |

**The tree is four directories: `apps/`, `bootstrap/`, `docs/`, `modules/`.**
`modules/` is what a program imports and `bootstrap/` is the compiler as it
emitted itself, so everything else the repository *makes* is an application and lives
under `apps/`. There is no `tools/`, no `selfhost/`, no `editor/`, no
`packages/` and no top-level `examples/`; each was a fifth kind of thing that
had to be explained, and none of them was.

`apps/` holds: the capstones (`mqtt`, `microkernel`, `blink`, `desktop`,
`mcmod`); the five package demos (`bench_demo`, `profile_demo`, `signal_demo`,
`tambo_demo`, `cli_tool`), one directory and one README each; `bench` (the
cross-language harness and its C/Rust/Go/JS/Python reference kernels);
`playground` (the browser playground, a single `.maca` file compiled by the JS
backend); `tomo` (the i18n handbook builder that renders `book/{en,ko}/*.md`
into `site/`, built entirely out of the UI syntax below plus one line of
hand-written CSS; it is also the worked example of that syntax, so keep it free
of hand-concatenated markup); `site`, the project's front page, `home.maca`,
whose copy is keyed by sum types so a translation that drops a card is a
NonExhaustive error rather than a shorter page; the four toolchain programs
above (`bindgen`, `lint`, `macadoc`, `build_site`); `selfhost`, the Maca
compiler written in Maca, stage 1; `npm`, the `macalang` npm package and the
`build.maca` that packs the wasm front-end into it; `editor`, the Zed
extension, the tree-sitter grammar and the TextMate grammar, which are programs
that run inside an editor rather than a terminal; and `examples`.

`apps/examples/` is the **regression set and nothing else**: the spec's code
blocks verbatim, `apps/examples/bad/` (each rejected with a named diagnostic),
and `apps/examples/handbook.maca` (the book's claims as one executable
program). A file is there because a test, `docs/SPEC.md`, or a handbook chapter
names it. It is the one `apps/*` entry with no `maca.toml`, because a package
states which program it builds and this directory is forty of them plus fifteen
that must not build at all. `apps/examples/taskr.maca` is the one runnable
program that stays, because it is also four fixtures: the committed lexer
golden-token snapshot, the parser round-trip, the `maca fmt --check` golden, and
the root manifest's `[[bin]]`.

**The handbook is two volumes with one table of contents** (`apps/tomo/book.toml`):
*Learning Maca*, read front to back once, and *The Reference*, opened at the
page you need. A chapter belongs to exactly one of them, and a teaching chapter
with a stricter twin links to it by name. The `a`-prefixed files are the
Reference: they grew out of the appendices and kept the prefix so no published
URL had to move. Maca in a fenced block is highlighted by
`apps/tomo/highlight.maca`, a scanner that follows `modules/maca/lexer.maca`
rather than guessing; an unknown language tag falls through to escaped plain
text.

FFI (`import "sqlite3.h"` / `import "….py"`) links the real library: through
`wsl nix` when present, else the **host `cc`** with system headers/libs
(`-lsqlite3`, `python3-config`), so FFI builds on a plain Linux dev machine
(`apps/examples/ffi_sqlite.maca` iterates a real result set).
