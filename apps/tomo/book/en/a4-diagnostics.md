# Diagnostics

Every diagnostic the checker emits, plus the failures that come from further
down the pipeline.

## TypeMismatch

Two types had to be the same and weren't.

**Argument type**

```
TypeMismatch: in call to `d` (argument 2): type mismatch: expected P, found int
```

**Argument count**: arity is a type property, so it lands here:

```
TypeMismatch: call to `f` expects 2 argument(s), got 3
```

**Disagreeing branches**: an `if` or ternary whose arms have different types:

```
TypeMismatch: ternary branches disagree: type mismatch: expected int, found str
```

`if` and `match` are expressions, so their branches have to agree even when you
were treating the whole thing as a statement. `c ? continue : 0` fails this way,
because `continue` has no value.

**Record fields**: a literal names every field the record declares, and no field
it doesn't:

```
TypeMismatch: `Config` is missing field(s): port, title
TypeMismatch: `Config` has no field `titel`; did you mean `title`?
```

The anonymous spelling owes the same two things, and names the binding rather
than the type ([The Type System](a6-types.md) is where they meet):

```
TypeMismatch: in `p`: record is missing field `y`
TypeMismatch: in `p`: record has unexpected field `z`
```

**Which side is which.** `expected` is always the type the code *declares* and
`found` is the value that arrived.

## NonExhaustive

A `match` doesn't cover every variant.

```
NonExhaustive: match on `Color` is not exhaustive; missing: Blue
```

Add the arm, or `_` if you mean "everything else, forever". Prefer the arm.

## Immutable

Assignment to a constant.

```
Immutable: cannot reassign constant `Limit`; declare it mutable with
`Limit = …` (no `const`)
```

Three things make a binding constant: `const`, a trailing `as const`, and a
**Capitalized** name, so `Total = 0` is a constant. `maca lint` nudges toward
writing `const` explicitly.

## UndefinedName

A name defined nowhere: not a local, not a function, not an import, not a
builtin.

```
UndefinedName: call to undefined function `helprr`
```

It applies to lowercase names in call position; capitalised names are
constructors, and UFCS method calls stay gradual. It also covers the keywords
Maca doesn't have:

```
UndefinedName: `return`: a function's last expression is its value, so drop
the `return`
```

Each leads with the form that works. See [Keywords](a1-keywords.md) for the full
list. It also covers a capitalised name in a **pattern**:

```
UndefinedName: `Busi` is capitalized, so it is a constructor, and nothing
declares one by that name: did you mean `Busy`?
```

In a pattern, `Busy` matches the variant and `busy` binds whatever was matched.
A misspelt variant matches *everything*, silently, and the arms below it become
unreachable while `match` still looks exhaustive.

## UnknownOption

In config mode, an assignment to an option **namespace** the compiler does not
know.

```
UnknownOption: unknown NixOS option namespace `servicez`
```

It knows the NixOS roots (`networking`, `services`, `system`, `users`,
`environment`, `programs`, `boot`, `hardware`, `security`, `nix`, `fonts` and
their siblings), plus any local binding in the file.

The namespace is checked, the leaf is not: `servicez.nginx.enable` is caught
here, and `services.nginx.enabl` goes through to Nix. `maca dev` suppresses this
diagnostic, because `dev.*` is not a NixOS namespace.

## EffectInConfig

An impure operation in config mode.

```
EffectInConfig: config must be pure but this uses effect(s): async
```

The message names every row it found, so a configuration that both prints and
sleeps reports `io, async`. Printing, `fail`, `spawn`, `await`, `sleep_ms`, and
any call through a `net`/`http`/`socket` or `os`/`process` receiver are
rejected.

The rows are matched on the *shape* of the call, so `file.read(p)` is caught and
the free function `read_file(p)` is not. The rows are
[Effects and Async](a7-effects.md).

## Import resolution

These come from before the checker, while it works out which file each `import`
names.

**An ambiguous import** names two files, and refuses rather than picking one:

```
apps/x/main.maca: ambiguous import `bench/stat`: it names two files:
  apps/x/bench/stat.maca (as written, the one this build would use)
  modules/bench/stat.maca (under a search root)
  A directory sharing a package's name hides the package, and the import line
  cannot say which was meant. Rename the directory, or move the module so that
  one path names it.
```

The written path is tried before the search roots, so a directory sharing a
package's name shadows the package, and one of the two files is silently not
compiled.

**A name defined by more than one module** cannot be inlined, because everything
becomes one translation unit:

```
`render` is defined by more than one module of this program, and every module
is inlined into one:
  modules/tomo/page.maca
  modules/tomo/feed.maca
  Both are API, so neither can be moved out of the way. Rename one of them, or
  ask for the one you mean with `import { … } from …` and keep the other out of
  the program.
```

**A reference nothing settles** is the same clash seen from a third file:

```
apps/site/home.maca: `render` is defined by more than one module this file
reaches, and every module is inlined into one:
  modules/tomo/page.maca
  modules/tomo/feed.maca
  Nothing here says which one `render` means. Ask for the one you mean with
  `import { render } from …`, or rename the others.
```

**An import that resolves to no file** is an error too, including a single-word
selective import:

```
apps/x/main.maca: no module `std/str`: `std/str.maca` is not beside this file
or in the working directory
```

## Errors that are not diagnostics

**Parse and lex errors** name a byte range:

```
lex (28, 28): string literal spans a line; write `\n`, or use a raw
"""…""" string. (A literal brace is `\{` or `{{`.)
```

An ambiguous `=> { … }` is one of these: a record literal and a block read the
same when every entry is a distinct `name = value` and only newlines separate
them, so neither reading is taken:

```
parse (45, 46): `mk`: this `=> { … }` reads as a record literal and as a
block. Write `Name { … }` for the record, or drop the `=>` for the block
```

[Syntax](a5-syntax.md) has the full rule.

**Backend refusals**: valid code that a particular target cannot emit, because
an event handler has nowhere to attach when elements render to a string
([the UI syntax](a11-ui.md)):

```
`on:click` needs a live DOM; build this with `--target js`
```

| Target | Refuses |
|---|---|
| native | `on:click=` and its siblings |
| `rust` | a bodyless (FFI) function; `import c`/`import py`; an `import rust` naming an undeclared crate; a borrowed foreign parameter that is returned or stored |
| `embedded` | `info` and the other console builtins; a `main` with a return type |

**C compiler errors** should not happen, and when they do it is a compiler bug
worth reporting. The method set of a `str` or a `T[]` is closed, so a misspelt
method is caught before the linker. Calls on an `any` receiver stay gradual.
