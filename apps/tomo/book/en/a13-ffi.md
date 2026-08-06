# Foreign Function Interface

Maca compiles to C, so the C ABI is the ground it already stands on. Foreign
function interface is an `import`.

## C

```maca
import c "sqlite3.h"
```

That declares the header and links the library. Functions from it are called
like any other:

```maca
import c "sqlite3.h"

main() -> int {
    info("sqlite {sqlite3_libversion()}")
    0
}
```

The driver resolves the library through `nix` when available, otherwise through
the host's `cc`. `apps/examples/ffi_sqlite.maca` opens a real database.

## Type mapping

| C | Maca | Notes |
|---|---|---|
| `char*`, `const char*` | `str` | |
| any other pointer | `int` | an opaque handle |
| `float`, `double` | `float` | |
| `int`, `long`, `size_t`, `int32_t`, … | `int` | |
| `void` | `int` | |

A pointer that is not a string becomes an opaque integer: you cannot dereference
it, you pass it back to the library that gave it to you.

## Generating the declarations

```
maca bindgen /usr/include/sqlite3.h
maca bindgen /usr/include/sqlite3.h sqlite3.maca
```

With one argument it prints; with two it writes a module. It is a prototype
scanner rather than a full C parser, and skips typedefs, structs, unions and
function pointers.

```maca
sqlite3_libversion() -> str
sqlite3_open(filename: str, ppDb: int) -> int
sqlite3_column_double(sqlite3_stmt: int, iCol: int) -> float
sqlite3_close(sqlite3: int) -> int
```

## Python

```maca
import py "json"
```

Python interop goes through `python3-config`. It embeds an interpreter, and
exists for libraries that only exist in Python.

## JavaScript: the `maca` bridge

`import js """…"""` embeds a block verbatim into the emitted `app.js`. The block
and the program meet on one object, `maca`, and nowhere else:

| Call | Does |
|---|---|
| `maca.get(name)` | read a name the program declared |
| `maca.set(name, value)` | write it, then refresh the view |
| `maca.set({ a, b })` | write several, refresh once |
| `maca.refresh()` | re-sync the bound nodes after something else moved |
| `maca.provide({ f })` | supply a function the Maca side declared |

A name the program never declared is an error, not a new field:

```
maca.set: `form_titel` is not state in this program; declared: form_title, form_url
maca.set: `Limit` is a constant
```

### Functions the host supplies

A function declared with no body is the other direction: the signature is
Maca's, the implementation the host's.

```maca
cfg_sections() -> Section[]
cfg_write(title: str, url: str, icon: str, section: str) -> bool

form_title = ""

import js """
maca.provide({
  cfg_sections: () => JSON.parse(localStorage.getItem("sections") || "[]"),
  cfg_write: (title, url, icon, section) => save(title, url, icon, section),
});

document.addEventListener("app:link", (e) => {
  maca.set({ form_title: e.detail.title });
});
"""
```

`cfg_sections()` is an ordinary call; the backend routes it through the bridge.
Calling one that nothing implemented says so:

```
maca: `cfg_write` is declared in Maca but nothing implements it;
call maca.provide({ cfg_write: … }) from the import js block
```

### Order

The emitted file is the bridge, then the block, then the app, so whatever
`maca.provide` and `maca.set` do at the block's top level is in place before
`mount()` builds the view.

### A whole program built on it

`apps/playground/playground.maca` declares fifteen bodyless functions and four
state names, and that is the entire surface its `import js` block is reached by.
Everything on the other side is a browser capability with no Maca spelling.

## The browser, as modules

`modules/web` is the browser as ordinary Maca. Three files are bridges, each
declaring bodyless functions and implementing them in its own `import js` block;
`web/format` is arithmetic and runs anywhere.

| Module | What it reaches |
|---|---|
| `web/storage` | what the browser remembers between visits |
| `web/time` | the local clock, and repainting on a timer |
| `web/file` | offering the reader a download, and taking a file back |
| `web/format` | what a clock reads and what a download is named: no host at all |

### State that persists

```maca
import web/storage

config: Config = stored("homepage.config", data(Links))
locked = stored("homepage.locked", true)
```

`stored(key, default)` is the whole surface. The name **starts** as whatever the
browser saved under `key`, and **assigning it saves it again**:

```maca
lock() -> int {
    locked = !locked
    0
}
```

No read call, no write call:
[assignment is already the update](a11-ui.md#assignment-is-the-update), and this
makes it the save too. The key is written out or bound to a constant, and a
`const` name cannot be stored.

In the emitted `app.js` every write becomes `local_store(key, …)`, and one line
goes into the bridge to read the name back before the page is first built:

```js
maca.set("locked", local_start("homepage.locked", maca.get("locked")));
```

`local_start`, `local_store` and `local_forget` are `web/storage`'s three
declared functions. `stored(…)` without `import web/storage` is a build error.

### A file the reader picks

`download(name, text)` offers a file; `pick_text(accept)` asks for one and
answers with the text:

```maca
import { pick_text } from web/file

import_config() {
    text = await pick_text("application/json")

    if text == "" {
        return
    }

    next: Config = decode(text)
    commit(next, "imported")
}
```

The picker cannot answer at once, so the call is a suspension point and `await`
reads it: `""` when the reader picks nothing. `import_config` never declares
itself async, and neither does the button that calls it;
[async is an effect, not a colour](a7-effects.md).

### Off the browser

A module implemented by an `import js` block has nothing to run anywhere else,
so any other target refuses it **by name**:

```
`web/storage` runs in a browser: what implements it is an `import js` block,
and the native target has no JavaScript to run it in; build the page with
`maca build --target js`
```

That is why `web/format` exists as its own file, in plain Maca, so
`modules/web/tests/` is a suite `maca test` runs like any other.

## When to reach for FFI

Prefer a Maca implementation where the work is self-contained, and use FFI where
the library *is* the value: SQLite, a compression codec, a system API. Every FFI
call is a place the type checker cannot help you.

## The other direction

Maca can compile *to* other languages rather than calling into them:
`--target rust` reaches crates.io, `--target jvm` Java, `--target js`
JavaScript. When the ecosystem you need is on one of those platforms, targeting
it is often better than binding to it. [Targets](a10-targets.md) covers all
six.
