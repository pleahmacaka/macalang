# Maca playground

The browser playground for Maca, **written in Maca**, one file:
`playground.maca`. The UI, the state, the handlers, and every sentence the page
says are Maca, styled with Maca's integrated Tailwind (the JS backend generates
the CSS from the utility classes). Pretendard is not inlined: the page links
`../fonts/fonts.css`, the same sheet the rest of the site links, so the
Korean it renders has Hangul to render it with. The compiler itself is pulled
in with `import wasm`.

## The boundary

The `import js` block is the host, and the whole of what it may be reached by
is the `maca` bridge ([the FFI chapter](../tomo/book/en/a13-ffi.md) documents
it). The top of `playground.maca` declares fifteen functions with no body,
which is the contract:

```maca
compile(src: str, mode: int) -> str
symbols() -> Sym[]
emitted(pane: str) -> str
outcome() -> Outcome
```

The block answers them with `maca.provide({ compile, symbols, … })`, and a name
nothing implements says so on the first call instead of arriving as
`undefined`. The other direction is four state names the host writes with
`maca.set`: `ready`, `trouble`, `version` and `editor`. Nothing hangs on
`window`.

What is behind the boundary is the browser and nothing else: instantiating the
WebAssembly compiler and reading its packed return value out of linear memory,
the Monaco editor and its textarea fallback, the URL fragment, the clipboard
and history call behind Share, and the sandboxed iframe the Preview runs in.
Which examples exist, what each tab shows, what the status line says, when the
flame chart appears: all of that is Maca, and all of it is a pure function of
the declared state plus what those accessors answer.

There is no repaint call anywhere in the Maca half. Writing a declared state
name is the update, so a handler that assigns is the whole of what it writes.
The one `maca.refresh()` left is in the host, on the one thing Maca cannot see:
a compile result arriving, and the caret moving inside the editor.

## What the page shows

The idea it is built around: everything the compiler can do to a program should
be one click away from the program. So the right pane is not "the output", it is
every artifact the front end produced from this source.

- **Console** with the interpreter's stdout and the program's **exit status**.
- **Preview**, which runs the emitted JS for real in a sandboxed iframe. For a
  program whose `main` returns an `Element` this is the language's reactive DOM
  working, from the same `js` back end `maca build --target js` uses.
- **C**, **JS** and **CSS**: the native translation unit, the transpiled module,
  and the stylesheet `styles()` generated for exactly the utilities this program
  names.
- **Nix**, in config mode. The mode is a switch in the header, so the same page
  compiles a program or a configuration.
- **Definitions**, the document outline: every function, type, value and config
  option the program declares, with its signature; click one to jump to it.
- **Diagnostics**, which carries type and effect errors *and* the back ends'
  refusals. Each back end names a construct it cannot lower rather than emitting
  code that would not compile, and that refusal is what a target's tab shows in
  place of code. In config mode it also names the functions a configuration has
  no place for, because an empty module the page called clean is worse than an
  error.

The tabs a mode has no answer for are not offered: config mode shows
Diagnostics, Definitions and Nix, and nothing else. The profiler strip appears
only when there is a flame graph to put in it. At narrow widths the two panes
stack instead of sitting side by side.

The header's **Share** button writes the whole program into the URL fragment. A
fragment is the one part of a URL a browser never sends, so the link carries the
program to whoever opens it and to nobody else: no server, no paste service, no
identifier to expire.

The examples are Maca constants in `playground.maca` rather than strings in the
bridge, which is what lets `crates/wasm/tests/playground.rs` compile every one
of them and fail the build when one stops checking clean.

## The editor

The **Monaco** editor (the one behind VS Code), loaded from a CDN, with a Maca
language mode wired to the in-browser compiler:

- **Syntax highlighting** (a Monaco/Monarch grammar) and **tab-to-indent**,
  bracket matching, and the rest of Monaco's editing.
- **Hover**: the signature or type of the identifier under the caret.
- **Signature help** while a call is being typed, with the parameter under the
  caret picked out.
- **Go to definition** (ctrl-click / F12) and **highlight all references**, both
  scope-aware, so a parameter named `n` does not light up an `n` elsewhere.
- **Autocomplete**: keywords, builtin types and functions, and the program's own
  top-level definitions.
- **Diagnostics** as editor markers, plus the full text in the Diagnostics tab.
- **A flame-graph profiler** with the wall-clock time, reusing the native
  `maca profile` renderer (`maca-profile`) fed by the interpreter's step counts.

All of the language-server answers come from `maca-lsp`, the same analysis the
native language server runs, through one `lsp` wasm export.

If the Monaco CDN can't be reached (offline, or a strict-CSP host like a
claude.ai artifact), the editor falls back to a plain monospace textarea that
still recompiles on input and indents with Tab, so the page always works.

## Build and deploy

```sh
# once: build the in-browser compiler
cargo build -p maca-wasm --target wasm32-unknown-unknown --release
# then compile the playground into one self-contained index.html
maca build --target js apps/playground/playground.maca -o out
```

`maca build --target js` inlines the styles, the transpiled app, and the
`import wasm` asset (as base64) into a **single self-contained
`out/index.html`**: everything Maca produced is in that one file, with no
assembly step, and `out/app.css` and `out/app.js` beside it are unreferenced
copies of what it already carries. Deploy the file anywhere (static host, or a
claude.ai artifact). The compiler runs offline; the one request that leaves the
page is Monaco's loader from the CDN, and the textarea fallback above is what
happens when it does not come back. That is the whole flow: `maca init`, write
`.maca`, `maca build`, deploy the result.

## How it hangs together

| piece | where |
|---|---|
| UI, state, handlers, examples | `playground.maca` (Maca) |
| styling | Tailwind utility classes to CSS by the JS backend (`crates/backend_js`) |
| the panes, the status line, the outline markup | `playground.maca` (Maca), reading the declared accessors |
| host (wasm bridge, editor, preview iframe, URL fragment, clipboard) | `import js """…"""` block inside `playground.maca`, reached only through `maca.provide` / `maca.set` |
| token/outline colours | one `import css """…"""` block of scoped classes |
| fonts | `<link>` to `../fonts/fonts.css`: Pretendard GOV Variable and JetBrainsMono Nerd Font, shared with the rest of the site |
| compiler, analysis, interpreter | `import "…/maca_wasm.wasm"`: the `run`, `lsp` and `version` exports, embedded as base64 by `maca build` |

No hand-written `.html`/`.js`/`.css` and no build wrapper: everything is
generated from the one `.maca` source by `maca build`.
