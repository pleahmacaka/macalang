# Maca playground

A browser playground for Maca: a [Monaco](https://microsoft.github.io/monaco-editor/)
editor with Maca syntax highlighting, plus the compiler front-end (lexer →
parser → type/effect checker → C/JS/Nix emitters) compiled to WebAssembly so
checking and codegen run entirely client-side.

## Run it

```sh
./build.sh                       # vendors Monaco + builds maca_wasm.wasm here
python3 -m http.server 8000      # or any static server, from this folder
# open http://localhost:8000/
```

`build.sh` needs the wasm target once, and `npm` on PATH (to vendor Monaco):

```sh
rustup target add wasm32-unknown-unknown
```

Everything is served locally — **no CDN at runtime**. Monaco is vendored into
`vendor/vs` and the `.wasm` sits next to the page. There is no bundler; the app
is plain ES modules + Monaco's AMD loader.

## Layout

| file | role |
|---|---|
| `index.html` | page shell |
| `maca-lang.js` | Monaco Monarch tokenizer, language config, and themes for Maca |
| `playground.js` | editor boot, wasm bridge, examples, run/output wiring |
| `style.css` | layout + theme |
| `build.sh` | builds `maca_wasm.wasm` from `crates/wasm` |

The wasm ABI (see `crates/wasm/src/lib.rs`) is three raw exports over linear
memory — `alloc`, `dealloc`, `run(ptr,len,mode) -> (ptr<<32)|len` — returning a
JSON blob of `{ parseErrors, diagnostics, outputs }`. No `wasm-bindgen`.

The Monarch grammar mirrors `editor/maca.tmLanguage.json` (the TextMate grammar
for editors like VS Code) and the lexer's keyword/type model.

## Regenerating

`maca_wasm.wasm` is a build artifact and is git-ignored — run `./build.sh`
after changing the compiler.

## Self-contained build

`./build-artifact.sh` embeds the wasm as base64 into a single HTML file (the
shareable playground). It is a build artifact, so it is written under the cache
directory (`$XDG_CACHE_HOME/maca/playground/`, else `~/.cache/…`) — never into
the repo — and stale builds there are pruned automatically. The script echoes
the final path on its last line; pass `-o <path>` to override.
