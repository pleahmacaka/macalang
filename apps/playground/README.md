# Maca playground

The browser playground for Maca, **written in Maca**, one file:
`playground.maca`. The UI, state, and event handlers are Maca, styled with
Maca's integrated Tailwind (the JS backend generates the CSS from the utility
classes). The only inline foreign code is the WebAssembly-bridge runtime (an
`import js` block that instantiates the compiler and reads its result out of
linear memory) and a small `import css` block (the Pretendard `@font-face` plus
the token/outline colours). The compiler itself is pulled in with `import wasm`.

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
- **Diagnostics**, which carries type and effect errors *and* the back ends'
  refusals. Each back end names a construct it cannot lower rather than emitting
  code that would not compile, and that refusal is what a target's tab shows in
  place of code. Load the `a page` example and read the C tab: a two-way binding
  needs a DOM, and it says so.

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
- **A definition outline** (document symbols: functions, types, values, config
  options); click to jump to the definition.
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
| host runtime (wasm bridge, editor glue, preview iframe, share link) | `import js """…"""` block inside `playground.maca` |
| font + token/outline colours | `import css """…"""` blocks (Pretendard subset + scoped classes) |
| compiler, analysis, interpreter | `import wasm "…/maca_wasm.wasm"`: the `run`, `lsp` and `version` exports, embedded as base64 by `maca build` |

No hand-written `.html`/`.js`/`.css` and no build wrapper: everything is
generated from the one `.maca` source by `maca build`.
