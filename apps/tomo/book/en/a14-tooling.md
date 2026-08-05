# The Toolchain

One binary, `maca`, does everything. There is no separate build tool, formatter
binary, package manager or test runner to install.

Every command, what it caches, and the two Maca programs in the repository that
do the jobs a subcommand would do elsewhere.

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

`build` takes the target: `--target nix|js|jvm|rust|embedded|tauri`, plus `--mcu`
for embedded and `--cp` for the JVM classpath. With no `--target` you get a
native binary.

## What the binary carries

`maca` is self-contained. Two things a compiler usually keeps in files beside
itself are inside it instead: the **C runtime** every native build links
against, and the **standard library**, all eight packages of it (`std`, `cli`,
`http`, `bench`, `profile`, `signal`, `tambo`, `web`). A release is `maca` and
`maca-lsp` and nothing else, and `import std/json` means the same thing in a
downloaded release as it does in a checkout of the compiler's own source.

It is inside the binary rather than installed next to it because a directory of
`.maca` files beside the executable is a second thing to package, a second
thing an installer can drop, and a second thing that can be a different version
from the binary reading it. Nothing that can go missing can go missing.

Resolving an import means reading source, so the copy has to reach disk: the
first import that nothing in your project answers unpacks it, once, into the
cache directory (`MACA_CACHE`, else `XDG_CACHE_HOME`, else `~/.cache/maca`),
under a name made of the compiler's version and a digest of the files.
Upgrading the compiler unpacks a new one and leaves the old alone, so two
versions on one machine never read each other's `std`.

Your project always wins. A `modules/std/text.maca` you wrote, or a
`maca_modules/std/` that `maca add` installed, is found by the ordinary search
before the carried copy is ever offered, and it is the copy the carried
packages read as well as the copy your own code reads.
[Modules and Layout](a9-modules.md) is where that order is written out;
`MACA_STDLIB=<dir>` replaces the carried copy with a directory of your own.

## Builds are cached

A native build is a pure function of the source, the compiler version and the
target, so the finished binary is stored under a hash of exactly those. Build an
unchanged program again and the whole pipeline (parse, check, emit, invoke the C
compiler) is skipped and the cached artifact is copied into place.

The invariant C runtime is cached separately as a compiled object, so even a
*changed* program does not recompile the runtime. Only your own generated `main.c`
goes through the C compiler.

Set `MACA_NO_CACHE=1` to turn all of it off, which is what you want when
measuring compile times. It turns off *caching*, not the unpacked standard
library: that one is not an artifact to be rebuilt, it is source to be read.

## A file read while the program is built

`data("config/links.json")` reads that file *now*, at build time, and the
program carries what it read. There is no fetch at start-up and no path to
deploy alongside the binary or the page.

```maca
import { decode } from std/json

Config = { title: str, links: Link[] }

Links = "config/links.json"

config: Config = data(Links)
```

The value is read into the type the binding declares, which is
[`std/json`'s `decode`](a3-stdlib.md) doing the work: `data` is the read and
the declared type is what it is read as. Write the path out or bind it to a
constant, because a build cannot follow a path a program computes at run time.

The path is resolved against **the file the command names**, not the working
directory, so `maca test /elsewhere/suite.maca` reads the same files from
anywhere.

### `.local` shadows the committed copy

A file whose name has `.local` before its extension takes over from the one
beside it, silently and by convention:

```
config/links.json         committed, and what a fresh clone builds
config/links.local.json   yours, gitignored, and what your build reads
```

`data("config/links.json")` reads `config/links.local.json` when that file
exists. That is `.env.local` spelled the way Maca spells a data file: the
source names one path, and which of the two answers is a property of the tree
rather than of the program. Delete the `.local` copy and the committed one is
back.

Three things are errors rather than a default:

| What | The build says |
|---|---|
| no file at that path | `` data("config/links.json"): …/config/links.json: No such file or directory `` |
| nothing types the text | `` data(…) reads the file into the type the binding declares; add `import { decode } from std/json` `` |
| the path is computed | `` data(…): the path is read while building, so write it out or bind it to a constant `` |

The file's bytes are part of what the [build cache](#builds-are-cached) is
keyed on, so editing the file rebuilds the program even though no `.maca` file
changed.

## The linter

`maca lint` covers the semantic checks. Alongside it, `apps/lint/lint.maca` is a
style linter written in Maca itself, which walks a directory tree:

```
maca run apps/lint/lint.maca            # the repository's own sources
maca run apps/lint/lint.maca src        # a directory
maca run apps/lint/lint.maca a.maca     # one file
```

It checks four things: lines over 80 columns, a single-line `if` block, trailing
whitespace, and hard tabs. It exits non-zero when it finds anything, so it drops
into a pre-commit hook or CI unchanged.

Two of its rules are more careful than they sound. Width is measured with string
literals collapsed, so a 200-character C template inside a string is exempt
exactly as a long comment is: the rule is about code, not text. And the same
exemption applies inside a raw `"""…"""` block, which holds foreign CSS or
JavaScript rather than Maca.

## API documentation

`apps/macadoc/macadoc.maca` reads a module's declarations, pairs each with the comment
above it, and writes an HTML reference, which is what rustdoc is to Rust and
TSDoc is to TypeScript. Like `apps/lint/lint.maca` above, it lives in the repository
rather than inside the `maca` binary: it is a Maca program, so it is a file you
run, not a subcommand you install.

```
maca run apps/macadoc/macadoc.maca site/api std/text.maca std/list.maca
```

Maca has no `export` keyword, and a module is mostly helpers. What makes an item
part of the API is a **doc comment**, written with a third slash:

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

To the compiler a `///` line is a comment like any other: the third slash is a
convention MacaDoc reads, not a token. The alternative, "any comment above an
item means API", was tried first and measured against what `std/README.md`
advertises: it was wrong 18 times in 60, listing helpers that happened to need
explaining and dropping public functions that didn't.

Inside a doc comment, a backtick span becomes code and a `*starred*` run becomes
emphasis. A blank line starts a paragraph, and an indented run is a code block,
which is how a module's header comment can end with the `import` line you need.
The plain `//` block at the top of a file is the module's own description.

## Editor support

There is a language server, `maca-lsp`. It provides diagnostics, hover, go to
definition, find references, document symbols, signature help, completion,
rename and formatting. Any editor that speaks LSP can use it.

The repository ships a Zed extension under `apps/editor/zed-maca`, with a tree-sitter
grammar, syntax highlighting, an outline, and the language server wired up.
Install it as a dev extension: in Zed, *Extensions → Install Dev Extension*, and
point it at that directory.

Syntax definitions for Monaco (the playground) and TextMate are kept in sync with
the lexer's real keyword list by a test, so a keyword added to the language and
not to the grammars fails the build.

## The playground

`apps/playground/playground.maca` is a browser playground: an editor, live
diagnostics, and every artifact the front end produced from the program in one
tab strip. Console is the interpreter's output and the exit status, Preview
runs the emitted JavaScript for real in a sandboxed iframe, Definitions is the
document outline, C, JS and CSS are what the back ends wrote, and Nix is what
config mode emits. A tab a mode has no answer for is not offered, and the
profiler strip appears only when there is a flame graph to put in it. At narrow
widths the editor and the output stack rather than sitting side by side.

It is a single Maca file compiled by the JavaScript backend, and it is worth
reading as the worked example of two things this book documents elsewhere. The
first is [the `maca` bridge](a13-ffi.md): the file opens with fifteen
signatures that have no body, which is the whole of what the `import js` block
is allowed to be reached by, and the block answers them with `maca.provide`.
Everything behind that boundary is the browser and nothing else, the
WebAssembly instance, the editor, the URL fragment, the clipboard, the preview
iframe. Which examples exist, what each pane says, what the status line reads:
all Maca. The second is [assignment as the update](a11-ui.md). The Maca half
contains no repaint call at all; writing `tab` or `mode` repaints the nodes
that read them. The single `maca.refresh()` left is in the host, for the one
thing Maca cannot see happen: a compile result arriving.

## Profiling

```
maca profile FILE
maca profile FILE -o flame.svg
```

runs the program under callgrind and renders a flame graph. Useful mostly for
the compiler itself, which is the largest Maca program that exists.

## Project layout

`maca init` starts a project by writing the two files a project cannot do
without: a `maca.toml` that states its name and the `[[bin]]` it builds, and
that `main.maca`. Nothing else. A table you have not needed yet is a table you
have not had to read, and every one of them can be added the day it earns its
place:

```toml
[package]
name = "hello"

[[bin]]
path = "main.maca"
```

Dependencies for the Rust target go in a `[rust-dependencies]` table and are
passed through to Cargo, and a `[page]` table names the page a JS or Tauri
build produces (`title`, `lang`, `description`; see
[Targets](a10-targets.md)).

For your own code, the module system needs no manifest at all: `maca build
app/main.maca` follows the imports. See
[Modules and Layout](a9-modules.md).

## One repository, many packages

A repository that holds more than one thing writes one `maca.toml` per thing,
plus a root that gathers them. This is what Maca's own tree looks like:

```toml
# maca.toml, at the root
[package]
name = "maca"
version = "0.3.1"

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

**One rule: the nearest manifest that states a key answers for it.** The chain
covering a file starts with the manifest in its own directory and ends at the
workspace root, and a manifest that says nothing about a key inherits the
answer from the one above it. So `modules/std` overrides `indent_size` by
stating one, and inherits the root's by staying silent.

That is why the package above states a name and not a version. It releases
under the workspace's version, and a copy of a number nobody compares is a copy
that goes stale.

Three tables are not settings, and so are not on the chain. Each of them
answers "which directory is this" rather than "how is this built":

| table | read from | why |
|---|---|---|
| `[workspace]` | the root, and only the root | it is what makes that directory the root |
| `[package]` | the package's own manifest | a member must state its own `name` |
| `[[bin]]` | the package's own manifest | it says what *this* package builds |

Every path a manifest writes is relative to the directory that manifest sits
in, so `path = "main.maca"` in `apps/hello/maca.toml` is
`apps/hello/main.maca` from wherever you run the command.

### Members are listed, and the list is checked

Members are written out. They are not found by convention, and the list is not
trusted on its own either: it is checked against the tree in both directions.

- A listed member with no `maca.toml` is an error naming it.
- A directory beside a member that holds a `maca.toml` and is not listed is an
  error naming it too.

A convention would silently adopt a stray directory; a list on its own would
silently drift from the tree. Both failures are silent wrong answers, and the
list plus the check is neither. A directory becomes a package by writing a
`maca.toml`, and by nothing else, so a scratch directory beside your packages
is never one.

### What a member manifest does not change

It does not change which directories are import search roots, nor the order
they are tried in. It changes only where the search stops. The walk up from the
importing file used to end at the first `maca.toml` it met; it now ends at the
workspace root, because a member's manifest is not the edge of the world.
`modules/std/text.maca` still reaches `modules/`, and so still resolves
`import std/list`. See [Modules and Layout](a9-modules.md).

### Working inside a package

With no file named, the three commands are about the package the working
directory holds:

```
cd apps/hello
maca build              # its [[bin]]
maca run                # the same, and run it
maca test               # every .maca suite under its tests/
```

`--bin <name>` picks one when a package declares more than one, and `[package]
tests` renames the test directory. A library that declares no `[[bin]]` is told
so by name rather than quietly building something else:

```
$ cd modules/std && maca build
maca: build: package `std` declares no [[bin]] in .../modules/std/maca.toml; name a .maca file
```

### What the project builds is declared, not flagged

`[build]` is where a project says what building it means, so that building it
is `maca build` and nothing more:

```toml
[build]
target = "js"
out = "build"
```

Five keys, one per flag that describes the project rather than the moment you
typed the command: `target` (`--target`), `out` (`-o`), `mcu` (`--mcu`),
`classpath` (`--cp`, the jars a JVM package compiles against) and `bin`
(`--bin`, which `[[bin]]` a bare `maca build` or `maca run` means when the
package declares several). An unknown key there is an error naming it, not a
setting that silently does nothing.

A flag on the command line still wins over the manifest, because the flag is
this invocation and the manifest is the project. A declared target also beats
the one the compiler would have guessed from the source. `out` is a path, so it
answers from the directory of the manifest that wrote it: `out = "build"` in
`apps/hello/maca.toml` is `apps/hello/build`, whichever directory you ran from.
