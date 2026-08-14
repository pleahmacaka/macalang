# Modules and Layout

Which file an `import` names, in what order the compiler looks, and what a
project's directories mean. The teaching version is [Modules](07-modules.md).

## The forms

| Written | Names |
|---|---|
| `import a/b` | the module `a/b` |
| `import a` | the module `a`, or a builtin if no such file exists |
| `import { f, g } from a/b` | only `f` and `g` from that module |
| `import { f } from a` | the same; a single word here is still a file |
| `import "hdr.h"` | a C header and its library |
| `import "mod.py"` | a Python module |
| `import "x.css"` / `"x.js"` / `"x.wasm"` | an asset, named by its extension |
| `import { a, b } from "npm:pkg"` | named bindings out of a package |
| `import "app.js" """…"""` | raw JavaScript, for the JS backend |
| `import "app.css" """…"""` | raw CSS, for the JS backend |

## An import that resolves to nothing is an error

A slash path names a module and nothing else, so failing to find one is a typo,
not a fallback:

```
maca: app.maca: no module `std/str`: `std/str.maca` is not beside this file
or in the working directory
```

The one form that may legitimately find no file is a bare `import a`, which
might be a sibling module or a builtin. Naming something the module doesn't
define is also a clean error:

```
maca: import { centroid } from geometry: 'centroid' is not defined in that module
```

## Resolution order

`import a/b` becomes the path `a/b`, and the compiler looks for `a/b.maca`:

1. In the **importer's own directory**, then in each directory above it, and at
   each of those directories:
   1. `<dir>/a/b.maca`, the written path taken literally;
   2. `<dir>/modules/a/b.maca`, `<dir>/src/a/b.maca`,
      `<dir>/maca_modules/a/b.maca`, each search root in that order.
2. The walk **stops at the project root**: the workspace root when the tree has
   one, and otherwise the nearest directory at or above the importer holding a
   `maca.toml`. A member's own manifest does not stop it, which is what lets
   `modules/std/text.maca` keep reaching `modules/` (see
   [Tooling](a14-tooling.md)).
3. If the importer is not inside a project at all, the same two steps are tried
   once more from the **working directory**.
4. Then a **bare sibling**: `<importer's dir>/b.maca`, using only the final
   segment.
5. Last, and only last, the **standard library the compiler carries**: the copy
   of `modules/` inside the `maca` binary, which is what makes `import std/json`
   work in a directory that has never heard of this repository.

### Do not name a directory after a package

Step 1.i comes before 1.ii, so a directory sharing a package's name answers for
it. A `bench/` beside `modules/bench/` makes `import bench/stat` mean whichever
of the two files exists, and if both do, the one that is not the package.
Nothing reports this.

## Search roots

| Directory | Is a search root | Written as |
|---|---|---|
| `modules/` | yes | `modules/std/text.maca` is `std/text` |
| `src/` | yes | `src/parser.maca` is `parser` |
| `maca_modules/` | yes | `maca_modules/toml/parse.maca` is `toml/parse` |
| `apps/` | **no** | `apps/tomo/conf.maca` is `apps/tomo/conf` |

`modules/*` are the packages. `src/*` is the same idea for a single-package
repository. `maca_modules/` is where `maca add` installs a dependency. `apps/`
is deliberately not a root, because two applications may each have a `conf`.
`maca.toml` renames any of them:

```toml
[layout]
modules = "packages"
src     = "lib"
apps    = "services"
```

The keys are read line by line, so a commented-out key is a comment.

## The standard library the compiler carries

`maca` has all nine packages inside it: `std`, `cli`, `http`, `bench`,
`profile`, `signal`, `tambo`, `web`. A downloaded release is one file that knows
what `import std/json` means.

The compiler inlines *source*, so the copy has to reach disk: the first import
that gets as far as step 5 unpacks it, once, into the cache directory
(`MACA_CACHE`, else `XDG_CACHE_HOME`, else `~/.cache/maca`) under a name made of
the compiler's version and a digest of the files.

Step 5 being last is the whole of the precedence rule:

```
modules/std/text.maca        in your project      wins
maca_modules/std/text.maca   installed by maca add wins over the carried copy
                             the carried copy      only if neither exists
```

Write `modules/std/text.maca` and the carried `std/json` reads *your*
`std/text`: a carried package resolves its imports against the project that
asked for it. To put a whole checkout in front of the carried copy, name it:

```
MACA_STDLIB=~/src/macalang/modules maca build main.maca
```

Nothing is unpacked then, and every `std/…` comes from that directory.

## There is no entry file and no index

## What `import` does to the program

It **inlines**: definitions are spliced into the program before type checking.
No separate compilation unit, no object file per module, no link step.

**Names are global once imported.** Two modules that both define `parse` will
collide. Prefix by convention or split further.

**A selective import eliminates dead code at the module boundary.**
`import { origin, dist2 } from geometry` brings those two plus the transitive
closure of same-module definitions *they* reference.

Imports are transitive, in dependency order.

## Building

Point the driver at the file holding `main`:

```
maca build app/main.maca
```

Everything reachable through imports is compiled with it. No manifest listing
sources, no build graph.

`maca -m module.function` runs a function out of a module without a `main`, as
in `maca -m http.serve`. The exit status comes from the entry point's declared
return type, and a `str[]` parameter receives the leftover command line.

## What the module system does not have

No visibility modifiers: everything a module defines is importable. No
namespacing beyond the file. No versioned registry beyond `maca_modules/`.
