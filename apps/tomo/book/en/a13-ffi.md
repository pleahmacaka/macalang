# Foreign Function Interface

Maca compiles to C, which means the C ABI is not a bridge it has to build. It
is the ground it already stands on. Foreign function interface is an `import`.

This chapter is the whole of it: the two forms, the type mapping, the generator,
and the honest guidance about when not to use any of it.

## C

```maca
import c "sqlite3.h"
```

That declares the header and links the library. Functions from it are then called
like any other:

```maca
import c "sqlite3.h"

main() -> int {
    info("sqlite {sqlite3_libversion()}")
    0
}
```

The driver resolves the library through `nix` when it is available, and
otherwise through the host's `cc` with system headers and libraries, so
`-lsqlite3` on an ordinary Linux machine works with no configuration.

`apps/examples/ffi_sqlite.maca` opens a real database and iterates a real result set.

## Type mapping

C types arrive in Maca according to a small fixed table:

| C | Maca | Notes |
|---|---|---|
| `char*`, `const char*` | `str` | |
| any other pointer | `int` | an opaque handle |
| `float`, `double` | `float` | |
| `int`, `long`, `size_t`, `int32_t`, … | `int` | |
| `void` | `int` | |

A pointer that is not a string becomes an opaque integer. You cannot dereference
it in Maca; you pass it back to the library that gave it to you. That is
usually exactly what a handle-based C API wants, and it keeps the FFI from
introducing a way to corrupt memory.

## Generating the declarations

Writing extern declarations by hand for a large header is tedious, so there is a
generator:

```
maca bindgen /usr/include/sqlite3.h
maca bindgen /usr/include/sqlite3.h sqlite3.maca
```

With one argument it prints; with two it writes a module. It is a prototype
scanner rather than a full C parser: it strips comments and preprocessor lines,
splits on `;`, and turns each `RET NAME(PARAMS)` into a Maca declaration. It
skips typedefs, structs, unions and function pointers rather than guessing at
them.

```maca
sqlite3_libversion() -> str
sqlite3_open(filename: str, ppDb: int) -> int
sqlite3_column_double(sqlite3_stmt: int, iCol: int) -> float
sqlite3_close(sqlite3: int) -> int
```

There are two implementations of bindgen: the original in Rust inside the
compiler, and a port written in Maca at `apps/bindgen/bindgen.maca`. A test runs both
over the same header and requires the output to match exactly, so the port is
not allowed to drift.

## Python

```maca
import py "json"
```

Python interop goes through `python3-config`, and the module's functions become
callable the same way. This is the heavier of the two, because it embeds an
interpreter, and it exists for reaching libraries that only exist in Python.

## JavaScript: the `maca` bridge

The JS backend takes the other kind of foreign code inline. `import js """…"""`
embeds a block verbatim into the emitted `app.js`, which is how a `.maca` user
interface carries the host glue it needs (a WebAssembly instance, an editor, a
browser API) without a second file.

The block and the program meet on one object, `maca`, and nowhere else:

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

That is the whole reason the object exists. A block used to assign
`state.form_titel = …` directly, which created a field nothing was bound to and
threw nothing, so the dialog it was filling in simply stayed blank. A constant
is refused for the same reason Maca refuses to reassign one.

### Functions the host supplies

A function declared with no body is the boundary in the other direction. The
signature is Maca's, the implementation is the host's:

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

`cfg_sections()` is an ordinary call in Maca; the backend routes it through the
bridge. Providing a name the program did not declare is rejected the same way a
state typo is, and calling one that nothing implemented says so:

```
maca: `cfg_write` is declared in Maca but nothing implements it;
call maca.provide({ cfg_write: … }) from the import js block
```

### Order

The emitted file is the bridge, then the block, then the app. So `maca.provide`
and `maca.set` work at the top level of the block, and whatever they set is in
place before `mount()` builds the view for the first time.

The generated `state` object and `update()` function are still there and still
work, because programs were written against them. They are this backend's own
locals rather than a promise, though: `maca` is the part that is documented, and
the part a new program should reach for.

### A whole program built on it

`apps/playground/playground.maca` is the worked example. It declares fifteen
functions with no body and four state names, and that is the entire surface its
`import js` block is reached by. Everything on the other side is a browser
capability with no Maca spelling: the WebAssembly instance, the Monaco editor,
the URL fragment, the clipboard, the sandboxed preview iframe. Everything on
this side, including every sentence the page prints, is Maca.

## The browser, as modules

`modules/web` is the browser presented as ordinary Maca. Three of its files are
bridges, each declaring the functions it needs with no body and implementing
them in its own `import js` block; the fourth, `web/format`, is arithmetic and
runs anywhere.

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

`stored(key, default)` is the whole surface. The name **starts** as whatever
the browser has saved under `key`, falling back to the value written beside it,
and **assigning the name saves it again**:

```maca
lock() -> int {
    locked = !locked
    0
}
```

That is the whole of what a handler writes. There is no read call, no write
call, and no pair of key constants carried around to spell them with;
[assignment is already the update](a11-ui.md#assignment-is-the-update), and
this makes it the save too.

The key is written out or bound to a constant, because it names a slot rather
than being computed. A `const` name cannot be stored, since a stored name is
written back exactly when it is assigned.

What the compiler does with it is worth knowing, because it is visible in the
emitted `app.js`: the binding keeps its declared value, every write to the name
becomes `local_store(key, …)`, and one line goes into the bridge to read the
name back before the page is built for the first time:

```js
maca.set("locked", local_start("homepage.locked", maca.get("locked")));
```

`local_start`, `local_store` and `local_forget` are `web/storage`'s three
declared functions. A program that only wants the sugar never calls them, and
`stored(…)` without `import web/storage` is a build error naming the import to
add.

### A file the reader picks

`web/file` has two calls. `download(name, text)` offers a file; `pick_text(accept)`
asks for one, and it answers with the text:

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
is what reads it: `""` when the reader picks nothing. `import_config` never
declares itself async, and neither does the button that calls it;
[async is an effect, not a colour](a7-effects.md). On the JS target the compiler
works out which functions reach an `await` and writes `async` on exactly those,
so the handler is written the way every other handler is. What a handler wrote
before it waited is on screen while it waits, and what it writes afterwards
repaints when it lands.

### Off the browser

A module whose implementation is an `import js` block has nothing to run
anywhere else, so building one for any other target refuses **by name** rather
than compiling to a program that does nothing:

```
`web/storage` runs in a browser: what implements it is an `import js` block,
and the native target has no JavaScript to run it in; build the page with
`maca build --target js`
```

That is why `web/format` exists as its own file. The clock's padding and the
download's file name are decided there, in plain Maca, so `modules/web/tests/`
is a suite `maca test` runs like any other.

## When to reach for FFI

The honest guidance: prefer a Maca implementation where the work is
self-contained, and use FFI where the library *is* the value: SQLite, a
compression codec, a system API. Every FFI call is a place the type checker
cannot help you, because the types on the other side are asserted rather than
checked.

## The other direction

Maca can also compile *to* other languages rather than calling into them:
`--target rust` emits Rust source that can use crates.io, `--target jvm` emits
Java, and `--target js` emits JavaScript. When the ecosystem you need is on one
of those platforms, targeting it is often better than binding to it.
[Targets](a10-targets.md) covers all six.
