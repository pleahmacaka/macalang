# Modules and Layout

Which file an `import` names, in what order the compiler looks, and what a
project's directories mean. The teaching version is
[Modules](07-modules.md); this is the resolution rule.

## The forms

| Written | Names |
|---|---|
| `import a/b` | the module `a/b` |
| `import a` | the module `a`, or a builtin if no such file exists |
| `import { f, g } from a/b` | only `f` and `g` from that module |
| `import { f } from a` | the same; a single word here is still a file |
| `import c "hdr.h"` | a C header and its library |
| `import py "mod"` | a Python module |
| `import js """…"""` | raw JavaScript, for the JS backend |
| `import css """…"""` | raw CSS, for the JS backend |

## An import that resolves to nothing is an error

A slash path names a module and nothing else, so failing to find one is a typo,
not a fallback:

```
maca: app.maca: no module `std/str`: `std/str.maca` is not beside this file
or in the working directory
```

A **selective** import is the same promise even when it is one word, because
there is nothing to select from a builtin. `import { greet } from lib` with no
`lib` used to resolve to nothing, silently, and the program failed at the
linker instead.

The one form that may legitimately find no file is a bare `import a`: it might
be a sibling module or a builtin, and which one it is is decided by whether the
file is there.

Naming something the module doesn't define is also a clean error rather than a
dangling reference discovered later by the C compiler:

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

Three of those steps exist because of a bug rather than a design.

Walking *upward* is what lets a program deep in the tree write `import std/text`
and mean the project's own. Resolving against the working directory alone made a
build depend on where it was started from, and a test harness that runs in a
crate directory could not resolve an example's imports at all.

Stopping at the project root is what keeps the search out of `$HOME` and `/`,
where a stray `std/` would become the standard library for every project
beneath it. And the language server, whose own search is bounded by the
workspace, would then disagree with the compiler about where a name is defined.

The bare-sibling rule comes **last** on purpose. It is a convenience for
`import selfhost/token` beside its siblings, and taking it first meant any
`list.maca` next to a program silently shadowed `std/list` with no diagnostic at
all.

### Do not name a directory after a package

Step 1.i comes before 1.ii, so a directory that shares a package's name answers
for it. A `bench/` beside `modules/bench/` makes `import bench/stat` mean
whichever of the two files exists, and if both do, the one that is not the
package. Nothing reports this; the program compiles against the other file.

Putting the search roots first does not fix it, which is worth knowing before
you propose it. The walk visits each directory in turn, so `apps/bench/` still
answers for `bench/…` from anywhere under `apps/` whatever the order *within*
one directory. And `maca_modules` is a search root, so roots-first would let a
dependency you installed outrank a file you wrote.

The rule that follows is short: a package's name is not a name to give a
directory.

## Search roots

| Directory | Is a search root | Written as |
|---|---|---|
| `modules/` | yes | `modules/std/text.maca` is `std/text` |
| `src/` | yes | `src/parser.maca` is `parser` |
| `maca_modules/` | yes | `maca_modules/toml/parse.maca` is `toml/parse` |
| `apps/` | **no** | `apps/tomo/conf.maca` is `apps/tomo/conf` |

`modules/*` are the packages: the code meant to be imported. `src/*` is the
same idea for a single-package repository, and needs no manifest entry.
`maca_modules/` is where `maca add` installs a dependency, which is why the
directory it chose never appears in anybody's source.

`apps/` is deliberately not a root. Two applications may each have a `conf`, and
neither should silently answer for the other, so an application is reached by
its written path.

`maca.toml` renames any of them:

```toml
[layout]
modules = "packages"
src     = "lib"
apps    = "services"
```

The keys are read line by line, so a commented-out key is a comment.

## The standard library the compiler carries

`maca` has all eight packages inside it: `std`, `cli`, `http`, `bench`,
`profile`, `signal`, `tambo`, `web`. They are compiled into the binary the way
the C runtime already is, so a downloaded release is one file that knows what
`import std/json` means. Nothing is installed beside it, so nothing beside it
can be missing, be a different version, or be found by one tool and not
another.

The compiler resolves and inlines *source*, so the copy has to reach disk: the
first import that gets as far as step 5 unpacks it, once, into the cache
directory (`MACA_CACHE`, else `XDG_CACHE_HOME`, else `~/.cache/maca`) under a
name made of the compiler's version and a digest of the files. Two compilers on
one machine therefore never read each other's `std`, and there is nothing to
clean up when you upgrade.

Step 5 being last is the whole of the precedence rule:

```
modules/std/text.maca        in your project      wins
maca_modules/std/text.maca   installed by maca add wins over the carried copy
                             the carried copy      only if neither exists
```

A file you write replaces the compiler's for the packages it carries as well as
for your own code. Write `modules/std/text.maca` and the carried `std/json`
reads *your* `std/text`, because a carried package resolves its imports against
the project that asked for it rather than against the directory it was unpacked
into. A vendored fix that only applied to the lines you wrote yourself would not
be a fix.

To put a whole checkout in front of the carried copy, name it:

```
MACA_STDLIB=~/src/macalang/modules maca build main.maca
```

Nothing is unpacked then, and every `std/…` comes from that directory.

## There is no entry file and no index

A directory is not a module; a path is a path and that is the only thing it is.
`modules/http/server.maca` is `http/server`, and there is no
`modules/http.maca` re-exporting its neighbours to import instead.

The alternative was tried: a per-directory entry module re-exporting the
directory. It cost two names for every file, a second place to update when one
moved, and an import whose meaning depended on a file the reader never opened.

## What `import` does to the program

It **inlines**. The imported module's definitions are spliced into the program
before type checking. There is no separate compilation unit, no object file per
module, and no link step between your own modules.

Two consequences follow.

**Names are global once imported.** Two modules that both define `parse` will
collide. Prefix by convention (`json_parse`, `toml_parse`) or split further.

**A selective import eliminates dead code at the module boundary.**
`import { origin, dist2 } from geometry` brings across those two definitions
plus the transitive closure of same-module definitions *they* reference, so the
types their signatures mention come along without being listed. On a large
module that is the difference between compiling everything and compiling what
you use.

Imports are transitive: if `a/b` imports `a/c`, both are pulled in, in
dependency order.

## Building

Point the driver at the file holding `main`:

```
maca build app/main.maca
```

Everything reachable through imports is compiled with it. There is no manifest
listing sources and no build graph to maintain.

`maca -m module.function` runs a function out of a module without a `main`, as in
`maca -m http.serve`, and the exit status comes from the entry point's declared
return type. A `str[]` parameter receives the leftover command line.

## What the module system does not have

No visibility modifiers: everything a module defines is importable. No
namespacing beyond the file. No versioned registry beyond what `maca add`
installs into `maca_modules/`.

If that changes it will change in the direction of selective import, which
already gives the explicit-surface benefit of `pub` without a keyword.
