# Tooling

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

`build` takes the target: `--target nix|js|jvm|rust|embedded|tauri`, plus `--mcu`
for embedded and `--cp` for the JVM classpath. With no `--target` you get a
native binary.

## Builds are cached

A native build is a pure function of the source, the compiler version and the
target, so the finished binary is stored under a hash of exactly those. Build an
unchanged program again and the whole pipeline — parse, check, emit, invoke the C
compiler — is skipped and the cached artifact is copied into place.

The invariant C runtime is cached separately as a compiled object, so even a
*changed* program does not recompile the runtime. Only your own generated `main.c`
goes through the C compiler.

Set `MACA_NO_CACHE=1` to turn all of it off, which is what you want when
measuring compile times.

## The linter

`maca lint` covers the semantic checks. Alongside it, `tools/lint.maca` is a
style linter written in Maca itself, which walks a directory tree:

```
maca run tools/lint.maca            # the repository's own sources
maca run tools/lint.maca src        # a directory
maca run tools/lint.maca a.maca     # one file
```

It checks four things: lines over 80 columns, a single-line `if` block, trailing
whitespace, and hard tabs. It exits non-zero when it finds anything, so it drops
into a pre-commit hook or CI unchanged.

Two of its rules are more careful than they sound. Width is measured with string
literals collapsed, so a 200-character C template inside a string is exempt
exactly as a long comment is — the rule is about code, not text. And the same
exemption applies inside a raw `"""…"""` block, which holds foreign CSS or
JavaScript rather than Maca.

## Editor support

There is a language server, `maca-lsp`. It provides diagnostics, hover, go to
definition, find references, document symbols, signature help, completion,
rename and formatting. Any editor that speaks LSP can use it.

The repository ships a Zed extension under `editor/zed-maca`, with a tree-sitter
grammar, syntax highlighting, an outline, and the language server wired up.
Install it as a dev extension: in Zed, *Extensions → Install Dev Extension*, and
point it at that directory.

Syntax definitions for Monaco (the playground) and TextMate are kept in sync with
the lexer's real keyword list by a test — a keyword added to the language and not
to the grammars fails the build.

## The playground

`playground/playground.maca` is a browser playground: an editor, live
diagnostics, and the ability to run a program and see its C and JavaScript
output. It is a single Maca file compiled by the JavaScript backend, with its
own host glue and stylesheet carried inline in raw strings. It is worth reading
as an example of a real program in the language.

## Profiling

```
maca profile FILE
maca profile FILE -o flame.svg
```

runs the program under callgrind and renders a flame graph. Useful mostly for
the compiler itself, which is the largest Maca program that exists.

## Project layout

`maca init` starts a project with a `maca.toml`. Dependencies for the Rust
target go in a `[rust-dependencies]` table and are passed through to Cargo.

For your own code, the module system needs no manifest at all — `maca build
app/main.maca` follows the imports. See chapter 7.
