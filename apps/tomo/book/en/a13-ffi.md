# Foreign Function Interface

Maca compiles to C, which means the C ABI is not a bridge it has to build — it
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
otherwise through the host's `cc` with system headers and libraries — so
`-lsqlite3` on an ordinary Linux machine works with no configuration.

`examples/ffi_sqlite.maca` opens a real database and iterates a real result set.

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
it in Maca — you pass it back to the library that gave it to you. That is
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
scanner rather than a full C parser — it strips comments and preprocessor lines,
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
compiler, and a port written in Maca at `tools/bindgen.maca`. A test runs both
over the same header and requires the output to match exactly — the port is not
allowed to drift.

## Python

```maca
import py "json"
```

Python interop goes through `python3-config`, and the module's functions become
callable the same way. This is the heavier of the two — it embeds an interpreter
— and it exists for reaching libraries that only exist in Python.

## When to reach for FFI

The honest guidance: prefer a Maca implementation where the work is
self-contained, and use FFI where the library *is* the value — SQLite, a
compression codec, a system API. Every FFI call is a place the type checker
cannot help you, because the types on the other side are asserted rather than
checked.

## The other direction

Maca can also compile *to* other languages rather than calling into them:
`--target rust` emits Rust source that can use crates.io, `--target jvm` emits
Java, and `--target js` emits JavaScript. When the ecosystem you need is on one
of those platforms, targeting it is often better than binding to it.
[Targets](a10-targets.md) covers all six.
