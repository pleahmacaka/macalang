# Modules

A program stops fitting in one file quickly. Maca's answer is deliberately
small: a file is a module, and `import` pulls one in.

## A file is a module

There is no module declaration, no `package` line, no `mod` block. Put functions
in `geometry.maca` and they are the `geometry` module.

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

`origin` is called unqualified. An import brings names in flat. There is no
`geometry.origin()` form, because there is no namespace to qualify with.

A nested path uses `/`, matching the directory layout:

```maca
import std/str
import app/models/user
```

`import a/b` looks for `a/b.maca` beside the importing file. Imports are
transitive: if `a/b` imports `a/c`, both are pulled in, in dependency order.

## What import actually does

It **inlines**. The imported module's definitions are spliced into the program
before type checking. There is no separate compilation unit, no object file per
module, no link step between your own modules.

That has one consequence worth knowing: names are global once imported. Two
modules that both define `parse` will collide. In a large program, prefix by
convention (`json_parse`, `toml_parse`) or split further.

## Importing only what you need

Importing a whole module to use one function drags in everything it defines.
Selective import fixes that:

```maca
import { origin, dist2 } from geometry
```

Only `origin` and `dist2` come across, along with, transitively, whatever *they*
reference from the same module. `origin` returns a `Point`, so `Point` comes with
it automatically; you do not have to list the types your functions mention.

This is dead-code elimination at the module boundary. On a large module it is the
difference between compiling everything and compiling what you use.

Naming something the module doesn't define is a clean error, not a dangling
reference discovered later by the C compiler:

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

The first two link a real library, and [the FFI reference](a13-ffi.md) covers
them. The last two embed
raw text for the JavaScript backend, so a `.maca` user interface can carry its
own host glue and stylesheet inline. They take a triple-quoted string, which
spans lines and interpolates nothing, so CSS braces need no escaping.

## Building a multi-file program

Point the driver at the file with `main` in it:

```
maca build app/main.maca
```

Everything reachable through imports is compiled with it. There is no manifest
listing sources, no build graph to maintain. The self-hosted compiler is built
exactly this way: `maca build selfhost/main.maca` compiles the whole front end
from its import list.

## Where the module system stops

No visibility modifiers: everything a module defines is importable. No
namespacing beyond the file. No versioned module registry.

This is a small language for programs that fit in a repository, and the module
system is sized to match. If that changes, it will change in the direction of
selective import, which already gives you the "explicit surface" benefit of
`pub` without a keyword.

## Run it

Put the `geometry.maca` above in a directory, and beside it a `main.maca` with
the `import geometry` version from the top of this chapter. Then:

```
maca run main.maca
```

One command, two files, no manifest. Now move `geometry.maca` into a
`modules/` subdirectory and run the same command. It still resolves, because
`modules/` is a search root.

## Where the full answer is

[Modules and Layout](a9-modules.md) in the reference is the resolution order in
full: which directories are searched, in what sequence, where the walk stops,
and why the "a file beside the program" rule comes last rather than first.

Next: what happens to the memory all these values live in.
