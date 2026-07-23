# Maca playground

The browser playground for Maca — **written in Maca**. The whole page is one
file, `playground.maca`: the UI, state, and event handlers are Maca, styled with
Maca's integrated Tailwind (the JS backend generates the CSS from the utility
classes). The only inline foreign code is the WebAssembly-bridge runtime (an
`import js` block that instantiates the compiler and reads its result out of
linear memory) and the Pretendard `@font-face` (an `import css` font asset).

The compiler front-end + emitters run entirely client-side, compiled to
WebAssembly (`crates/wasm`), so checking, codegen, and *running* programs (via
the interpreter, with a flame-graph profile) all happen in the browser.

## Build

```sh
./build.sh          # compiles playground.maca and wraps it with the wasm
```

`build.sh` runs `maca build --target js playground.maca` (UI → `app.js` +
`app.css`) and inlines the wasm compiler as base64 into a single self-contained
HTML. The output is a build artifact, so it is written under the cache dir
(`$XDG_CACHE_HOME/maca/playground/`, else `~/.cache/…`) — never the repo — and
the final path is printed on the last line (`-o <path>` to override). Needs the
wasm target once: `rustup target add wasm32-unknown-unknown`.

## How it hangs together

| piece | where |
|---|---|
| UI, state, handlers | `playground.maca` (Maca) |
| styling | Tailwind utility classes → CSS by the JS backend (`crates/backend_js`) |
| host runtime (wasm bridge, examples) | `import js """…"""` block inside `playground.maca` |
| font | `import css """@font-face…"""` block (Pretendard, subset, inline) |
| compiler | `crates/wasm` → `maca_wasm.wasm`, embedded as base64 at build |

No hand-written `.html`/`.js`/`.css` files — everything is generated from the
one `.maca` source.
