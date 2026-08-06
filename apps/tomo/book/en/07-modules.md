# Modules

A file is a module, and `import` pulls one in.

## A file is a module

No module declaration, no `package` line, no `mod` block. Put functions in
`geometry.maca` and they are the `geometry` module.

```maca
// geometry.maca
Point = {
    x: int
    y: int
}

origin() -> Point =>
    Point { x = 0, y = 0 }

dist2(a: Point, b: Point) -> int =>
    (a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)
```

```maca
// main.maca
import geometry

main() -> int {
    p = origin()
    info("{dist2(p, Point { x = 3, y = 4 })}")
    0
}
```

An import brings names in flat; there is no `geometry.origin()` form. A nested
path uses `/`, matching the directory layout:

```maca
import std/str
import app/models/user
```

`import a/b` looks for `a/b.maca` beside the importing file. Imports are
transitive, in dependency order.

## What import actually does

It **inlines**: the imported definitions are spliced into the program before
type checking. No separate compilation unit, no object file per module, no link
step between your own modules.

Names are global once imported, so two modules that both define `parse` collide.
Prefix by convention (`json_parse`, `toml_parse`) or split further.

## Importing only what you need

```maca
import { origin, dist2 } from geometry
```

Only `origin` and `dist2` come across, with whatever *they* reference from the
same module: `origin` returns a `Point`, so `Point` comes too. Naming something
the module doesn't define is a clean error:

```
maca: import { centroid } from geometry: 'centroid' is not defined in that module
```

## Foreign imports

`import` with a language name in front reaches outside Maca entirely:

```maca
import c "sqlite3.h"
import py "json"
import js """…"""
import css """…"""
```

The first two link a real library; see [the FFI reference](a13-ffi.md). The last
two embed raw text for the JavaScript backend, so a `.maca` interface carries
its own host glue and stylesheet inline.

## Building a multi-file program

Point the driver at the file with `main` in it:

```
maca build app/main.maca
```

Everything reachable through imports is compiled with it. No manifest listing
sources, no build graph.

## Where the module system stops

No visibility modifiers: everything a module defines is importable. No
namespacing beyond the file. No versioned module registry.

## Run it

Put the two files above in one directory, then:

```
maca run main.maca
```

Now move `geometry.maca` into a `modules/` subdirectory and run the same
command. It still resolves, because `modules/` is a search root.

## Where the full answer is

[Modules and Layout](a9-modules.md) is the resolution order in full: which
directories are searched, in what sequence, and where the walk stops.
