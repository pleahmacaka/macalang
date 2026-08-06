# The Toolchain

One binary, `maca`, does everything. There is no separate build tool, formatter
binary, package manager or test runner to install.

## The commands

| Command | Does |
|---|---|
| `maca build FILE` | compile to a native binary |
| `maca run FILE` | compile and execute |
| `maca test FILE` | run every `test_…` function in the file |
| `maca fmt FILE` | format source |
| `maca lint FILE` | style and semantic checks |
| `maca watch FILE` | rebuild on change |
| `maca dev` | generate a dev-shell flake |
| `maca init` | start a project |
| `maca profile FILE` | run under callgrind, render a flame graph |
| `maca bindgen HEADER` | C header to Maca declarations |
| `maca add SPEC` | add a dependency (`npm:pkg`, `git+url`, `name@ver`) |
| `maca update` | re-resolve dependencies |
| `maca upgrade` | self-update the toolchain |

`build` takes the target: `--target nix|js|jvm|rust|embedded|tauri`, plus
`--mcu` for embedded and `--cp` for the JVM classpath.

## What the binary carries

Inside `maca`: the **C runtime** every native build links against, and the
**standard library**, all nine packages (`std`, `cli`, `http`, `bench`,
`profile`, `signal`, `tambo`, `web`). A release is `maca` and `maca-lsp`.

The first import nothing in your project answers unpacks the copy, once, into
the cache directory (`MACA_CACHE`, else `XDG_CACHE_HOME`, else `~/.cache/maca`),
under a name made of the compiler's version and a digest of the files.

## Builds are cached

A native build is a pure function of the source, the compiler version and the
target, so the binary is stored under a hash of those. An unchanged program
skips the whole pipeline. The C runtime is cached separately as a compiled
object, so even a *changed* program does not recompile it.

`MACA_NO_CACHE=1` turns off caching, not the unpacked standard library.

## A file read while the program is built

`data("config/links.json")` reads that file at build time, and the program
carries what it read.

```maca
import { decode } from std/json

Config = { title: str, links: Link[] }

Links = "config/links.json"

config: Config = data(Links)
```

The value is read into the type the binding declares, by
[`std/json`'s `decode`](a3-stdlib.md). Write the path out or bind it to a
constant. It resolves against **the file the command names**, not the working
directory.

### `.local` shadows the committed copy

A file whose name has `.local` before its extension takes over from the one
beside it:

```
config/links.json         committed, and what a fresh clone builds
config/links.local.json   yours, gitignored, and what your build reads
```

`data("config/links.json")` reads `config/links.local.json` when it exists.

Three things are errors rather than a default:

| What | The build says |
|---|---|
| no file at that path | `` data("config/links.json"): …/config/links.json: No such file or directory `` |
| nothing types the text | `` data(…) reads the file into the type the binding declares; add `import { decode } from std/json` `` |
| the path is computed | `` data(…): the path is read while building, so write it out or bind it to a constant `` |

The file's bytes are part of what the [build cache](#builds-are-cached) is keyed
on, so editing it rebuilds the program.

## The linter

`maca lint` covers the semantic checks. `apps/lint/lint.maca` is a style linter
written in Maca, which walks a directory tree:

```
maca run apps/lint/lint.maca            # the repository's own sources
maca run apps/lint/lint.maca src        # a directory
maca run apps/lint/lint.maca a.maca     # one file
```

## API documentation

`apps/macadoc/macadoc.maca` reads a module's declarations, pairs each with the
comment above it, and writes an HTML reference.

```
maca run apps/macadoc/macadoc.maca site/api std/text.maca std/list.maca
```

Maca has no `export` keyword. What makes an item part of the API is a **doc
comment**, written with a third slash:

```maca
/// Split on the *first* occurrence only: `split_once("a=b=c", "=")` is
/// `["a", "b=c"]`. A separator that isn't there gives the whole string and "".
split_once(s: str, sep: str) -> str[] {
    …
}

// An index brought into `0..len`. This one is an ordinary comment, so it
// explains the helper to the next reader of the source and stays out of the
// reference.
clamp(n: int, len: int) -> int {
```

To the compiler a `///` line is a comment like any other. Inside one, a backtick
span becomes code and a `*starred*` run becomes emphasis; a blank line starts a
paragraph, and an indented run is a code block. The plain `//` block at the top
of a file is the module's own description.

## Editor support

`maca-lsp` provides diagnostics, hover, go to definition, find references,
document symbols, signature help, completion, rename and formatting.

The repository ships a Zed extension under `apps/editor/zed-maca`, with a
tree-sitter grammar, highlighting, an outline, and the server wired up. Install
it as a dev extension: *Extensions → Install Dev Extension*.

Monaco and TextMate grammars are kept in sync with the lexer's keyword list by a
test.

## The playground

`apps/playground/playground.maca` is a browser playground: an editor, live
diagnostics, and every artifact the front end produced in one tab strip. Console
is the interpreter's output and the exit status, Preview runs the emitted
JavaScript in a sandboxed iframe, Definitions is the document outline, C, JS and
CSS are what the back ends wrote, and Nix is what config mode emits.

It is one Maca file compiled by the JavaScript backend, and the worked example
of [the `maca` bridge](a13-ffi.md) (fifteen bodyless signatures, answered with
`maca.provide`) and of [assignment as the update](a11-ui.md) (no repaint call in
the Maca half).

## Profiling

```
maca profile FILE
maca profile FILE -o flame.svg
```

runs the program under callgrind and renders a flame graph.

## Project layout

`maca init` writes two files: a `maca.toml` stating the project's name and the
`[[bin]]` it builds, and that `main.maca`.

```toml
[package]
name = "hello"

[[bin]]
path = "main.maca"
```

`[rust-dependencies]` is passed through to Cargo, and `[page]` names the page a
JS or Tauri build produces (`title`, `lang`, `description`; see
[Targets](a10-targets.md)). The module system needs no manifest at all:
`maca build app/main.maca` follows the imports
([Modules and Layout](a9-modules.md)).

## One repository, many packages

A repository that holds more than one thing writes one `maca.toml` per thing,
plus a root that gathers them:

```toml
# maca.toml, at the root
[package]
name = "maca"
version = "0.3.2"

[workspace]
members = [
    "modules/std",
    "apps/tomo",
]

[format]
indent_size = 4
```

```toml
# modules/std/maca.toml
[package]
name = "std"
description = "The layer above the prelude builtins."
```

### Which manifest answers

**The nearest manifest that states a key answers for it.** The chain starts with
the manifest in the file's own directory and ends at the workspace root, and a
manifest silent about a key inherits from the one above. So `modules/std`
overrides `indent_size` by stating one, and inherits the root's version by
saying nothing.

Three tables are not settings, and so are not on the chain:

| table | read from | why |
|---|---|---|
| `[workspace]` | the root, and only the root | it is what makes that directory the root |
| `[package]` | the package's own manifest | a member must state its own `name` |
| `[[bin]]` | the package's own manifest | it says what *this* package builds |

### Members are listed, and the list is checked

Members are written out, and the list is checked against the tree both ways.

- A listed member with no `maca.toml` is an error naming it.
- A directory beside a member that holds a `maca.toml` and is not listed is an
  error naming it too.

A directory becomes a package by writing a `maca.toml`, and by nothing else.

### What a member manifest does not change

Not the search roots, nor the order they are tried in. Only where the search
stops: the walk up from the importing file ends at the workspace root, not at
the first `maca.toml` it meets, so `modules/std/text.maca` still reaches
`modules/`. See [Modules and Layout](a9-modules.md).

### Working inside a package

With no file named, the three commands are about the package the working
directory holds:

```
cd apps/hello
maca build              # its [[bin]]
maca run                # the same, and run it
maca test               # every .maca suite under its tests/
```

`--bin <name>` picks one when a package declares several, and `[package] tests`
renames the test directory. A library that declares no `[[bin]]` is told so:

```
$ cd modules/std && maca build
maca: build: package `std` declares no [[bin]] in .../modules/std/maca.toml; name a .maca file
```

### What the project builds is declared, not flagged

`[build]` is where a project says what building it means:

```toml
[build]
target = "js"
out = "build"
```

Five keys: `target` (`--target`), `out` (`-o`), `mcu` (`--mcu`), `classpath`
(`--cp`) and `bin` (`--bin`). An unknown key is an error naming it.

A command-line flag wins over the manifest, and a declared target beats the one
the compiler would have guessed. `out` is a path, so it answers from the
directory of the manifest that wrote it.
