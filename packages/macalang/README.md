# macalang

Compile and **import Maca (`.maca`) from JavaScript**. The Maca compiler
front-end (lexer → parser → type/effect checker → JS emitter) ships as
WebAssembly, so everything runs in-process — no native toolchain needed.

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
import { compile, toJS, loadModule, loadFile } from "macalang";

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
```

| function | returns |
|---|---|
| `compile(src, {mode})` | `{ parseErrors, diagnostics, outputs, jsExports }` (mode 0=program, 1=config) |
| `toJS(src)` / `toESM(src)` | emitted JavaScript |
| `loadModule(src)` / `loadFile(path)` | `{ [fn]: Function }` |
| `version()` | compiler version |

## Notes

- The functional core lowers to JS (arithmetic, calls, ternary, `if`/`match`
  blocks, records, string interpolation, operator overloading). UI programs
  (`main() -> Element`) also emit `mount`/`build` for the DOM.
- `maca_wasm.wasm` is built from this repo's `crates/wasm` by `build.sh`.
- Node ≥ 18 or Bun. For Node, use the programmatic API (`loadFile`); the
  zero-config import plugin targets Bun.
