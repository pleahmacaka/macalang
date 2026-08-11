# macalang

Compile and **import Maca (`.maca`) from JavaScript**. The Maca compiler
front-end (lexer → parser → type/effect checker → JS emitter) ships as
WebAssembly, so everything runs in-process, with no native toolchain needed.

## Install

```sh
bun add macalang     # or: npm i macalang
```

## Import `.maca` files directly (Bun)

```toml
# bunfig.toml
preload = ["macalang/bun"]
```

```maca
// mathx.maca
add(a: int, b: int) -> int => a + b
fib(n: int) -> int => n < 2 ? n : fib(n - 1) + fib(n - 2)
```

```js
import { add, fib } from "./mathx.maca";
add(2, 3); // 5
fib(10);   // 55
```

Every top-level Maca function becomes a named export.

## Programmatic API

```js
import { compile, toJS, loadModule, loadFile, lsp } from "macalang";

// type-check + emit
const { diagnostics, outputs } = compile('bad() -> int => "nope"');
diagnostics; // [{ kind: "TypeMismatch", msg: "..." }]

// Maca source → JS source
toJS("id(x: int) -> int => x"); // "...function id(x) { return x; }..."

// Maca source → live functions
const m = loadModule("sq(n: int) -> int => n * n");
m.sq(9); // 81

// from a file
const lib = loadFile("./mathx.maca");

// what an editor asks about a caret
lsp("fib(n: int) -> int => n\n", 0).hover; // "fib(n: int) -> int"
```

| function | returns |
|---|---|
| `compile(src, {mode})` | `{ parseErrors, diagnostics, outputs, jsExports }` (mode 0=program, 1=config) |
| `toJS(src)` / `toESM(src)` | emitted JavaScript |
| `loadModule(src)` / `loadFile(path)` | `{ [fn]: Function }` |
| `lsp(src, offset)` | `{ hover, signature, definition, references }` for one caret |
| `version()` | compiler version |

## Notes

- The functional core lowers to JS (arithmetic, calls, ternary, `if`/`match`
  blocks, records, string interpolation, operator overloading). UI programs
  (`main() -> Element`) also emit `mount`/`build` for the DOM.
- `maca_wasm.wasm` is this repo's `apps/npm/wasm.maca` built by `build.maca`
  with `maca build --target wasm`, so it is Maca all the way down. It is not in
  the repository: run `maca run apps/npm/build.maca` (or `npm run build`)
  before `npm test`, and `npm publish` runs it for you. A checked-in copy
  drifts from the compiler it came from, so `test.mjs` checks the magic bytes,
  the exports and the wasi imports before anything else.
- It is a **wasi command**, not a library of exports: a call is one instance,
  the arguments go in as `argv` and the JSON comes back on stdout. `index.js`
  answers the seven `wasi_snapshot_preview1` imports itself, so nothing outside
  Node's own `WebAssembly` is needed.
- Node ≥ 18 or Bun. For Node, use the programmatic API (`loadFile`); the
  zero-config import plugin targets Bun.
