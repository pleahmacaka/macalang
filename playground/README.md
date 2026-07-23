# Maca playground

The browser playground for Maca — **written in Maca**, one file: `playground.maca`.
The UI, state, and event handlers are Maca, styled with Maca's integrated
Tailwind (the JS backend generates the CSS from the utility classes). The only
inline foreign code is the WebAssembly-bridge runtime (an `import js` block that
instantiates the compiler and reads its result out of linear memory) and a small
`import css` block (the Pretendard `@font-face` plus the token/outline colours).
The compiler itself is pulled in with `import wasm`.

It's a real editor, all driven by the same in-browser compiler:

- **Syntax highlighting** from the actual lexer tokens (a transparent
  `textarea` over a coloured highlight layer — no separate grammar to drift).
- **A definition outline** (document symbols: functions, types, values, config
  options) — click to jump to the definition.
- **LSP hover** — the signature/type of the identifier under the caret, from the
  same analysis as the native `maca-lsp`.
- **A flame-graph profiler** with the wall-clock time, reusing the native
  `maca profile` renderer (`maca-profile`) fed by the interpreter's step counts.

## Build & deploy

```sh
# once: build the in-browser compiler
cargo build -p maca-wasm --target wasm32-unknown-unknown --release
# then: compile the playground → one self-contained index.html
maca build --target js playground.maca -o out
```

`maca build --target js` inlines the styles, the transpiled app, and the
`import wasm` asset (as base64) into a **single self-contained `out/index.html`**
— no external requests, no assembly step. Deploy that file anywhere (static
host, or a claude.ai artifact). That's the whole flow: `maca init` → write
`.maca` → `maca build` → deploy the result.

## How it hangs together

| piece | where |
|---|---|
| UI, state, handlers | `playground.maca` (Maca) |
| styling | Tailwind utility classes → CSS by the JS backend (`crates/backend_js`) |
| host runtime (wasm bridge, highlight/outline/hover glue, examples) | `import js """…"""` block inside `playground.maca` |
| font + token/outline colours | `import css """…"""` blocks (Pretendard subset + scoped classes) |
| compiler + tokens/symbols/hover | `import wasm "…/maca_wasm.wasm"` — `run`/`hover` exports, embedded as base64 by `maca build` |

No hand-written `.html`/`.js`/`.css` and no build wrapper — everything is
generated from the one `.maca` source by `maca build`.
