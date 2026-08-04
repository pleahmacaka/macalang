# Diagnostics

Every diagnostic the checker emits, what it means, and what to do about it.
Six kinds, plus the failures that come from further down the pipeline and read
differently.

## TypeMismatch

Two types had to be the same and weren't. The widest of the six, because several
distinct mistakes reduce to it.

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

This one catches a common slip: `if` and `match` are expressions, so their
branches have to agree even when you were treating the whole thing as a
statement. `c ? continue : 0` fails for this reason, because `continue` has no
value.

**Record fields**: a literal names every field the record declares, and no
field it doesn't:

```
TypeMismatch: `Config` is missing field(s): port, title
TypeMismatch: `Config` has no field `titel`; did you mean `title`?
```

A missing field would otherwise be a silent zero (`""` for a `str`, `0` for an
`int`), and a misspelt one is worse, because the value goes nowhere and the
field it was meant for stays empty. Both compiled clean before this check, and
what you saw was a page with a heading missing. `base with { f = v }` is the
update form and is deliberately partial; only a construction is checked.

The anonymous spelling owes the same two things, and the message names the
binding rather than the type, because that is what the author wrote:

```
TypeMismatch: in `p`: record is missing field `y`
TypeMismatch: in `p`: record has unexpected field `z`
```

A record literal is otherwise open, because reading one field should not require
knowing the rest. [The Type System](a6-types.md) is where the named and anonymous
spellings meet.

**Which side is which.** `expected` is always the type the code *declares* and
`found` is the value that arrived: the annotation, the return type, or the
parameter is expected, and the expression is found. A pair the other way round is
a compiler bug, not a hint about your program.

## NonExhaustive

A `match` doesn't cover every variant.

```
NonExhaustive: match on `Color` is not exhaustive; missing: Blue
```

Add the arm, or add `_` if you genuinely mean "everything else, forever". Prefer
the arm: this diagnostic is the main reason to use a sum type, and `_` opts out
of it.

## Immutable

Assignment to a constant.

```
Immutable: cannot reassign constant `Limit`; declare it mutable with
`Limit = …` (no `const`)
```

Three things make a binding constant: `const`, a trailing `as const`, and a
**Capitalized** name. The third catches people out: `Total = 0` is a constant
because of the capital letter. `maca lint` nudges toward writing `const`
explicitly for exactly this reason.

## UndefinedName

A name that is defined nowhere: not a local, not a function, not an import, not
a builtin.

```
UndefinedName: call to undefined function `helprr`
```

Without this check, a typo would reach the C compiler and come back as an
undefined reference at link time. It applies to lowercase names in call position;
capitalised names are constructors, and UFCS method calls stay gradual.

It also covers the keywords Maca doesn't have:

```
UndefinedName: `return`: a function's last expression is its value, so drop
the `return`
```

Each of those leads with the form that works and mentions the missing word
second. The other order was read as a refusal: "Maca has no `return`" landed
first, and a reader took away that a function could not hand a value back, when
Maca has exactly Rust's rule. The word is the aside; what to write instead is the
message. See [Keywords](a1-keywords.md) for the full list of those.

And it covers a capitalised name in a **pattern**:

```
UndefinedName: `Busi` is capitalized, so it is a constructor, and nothing
declares one by that name: did you mean `Busy`?
```

A pattern is where the two conventions have to be told apart, because both are
just a name: `Busy` matches the variant, `busy` binds whatever was matched. So a
misspelt variant is a pattern that matches *everything*, silently, and the arms
below it become unreachable while `match` still looks exhaustive. Capitalisation
is what decides which one you wrote, the same rule that makes a Capitalized
binding a constant.

## UnknownOption

In config mode, an assignment to an option **namespace** the compiler does not
know.

```
UnknownOption: unknown NixOS option namespace `servicez`
```

The roots it knows are the NixOS ones (`networking`, `services`, `system`,
`users`, `environment`, `programs`, `boot`, `hardware`, `security`, `nix`,
`fonts` and their siblings), plus any local binding in the file.

Be precise about the reach, because it decides where you go looking when a
deploy fails: the namespace is checked, the leaf is not. `servicez.nginx.enable`
is caught here; `services.nginx.enabl` goes through to Nix, which rejects it at
evaluation time with its own message. `maca dev` suppresses this diagnostic
entirely, because `dev.*` is not a NixOS namespace at all.

## EffectInConfig

An impure operation in config mode.

```
EffectInConfig: config must be pure but this uses effect(s): async
```

The message names every row it found, so a configuration that both prints and
sleeps reports `io, async`.

Configuration describes a desired state; it must not *do* anything. Printing,
`fail`, `spawn`, `await`, `sleep_ms`, and any call through a `net`/`http`/
`socket` or `os`/`process` receiver are rejected. This is the check that makes
it safe for one language to be both a programming language and a configuration
language.

Be precise about the reach here too. The rows are matched on the *shape* of the
call, a known builtin name or a method on one of those receivers, so
`file.read(p)` is caught and the free function `read_file(p)` is not. A config
that reads a file through the free builtin compiles today. The rows and what
introduces each are [Effects and Async](a7-effects.md).

## Import resolution

These come from before the checker, when the compiler is working out which file
each `import` names and inlining it. They are not `DiagKind`s, but they are
compile errors a person hits, and every one of them replaced a silent wrong
answer.

**An ambiguous import** names two files, and refuses rather than picking one:

```
apps/x/main.maca: ambiguous import `bench/stat`: it names two files:
  apps/x/bench/stat.maca (as written, the one this build would use)
  modules/bench/stat.maca (under a search root)
  A directory sharing a package's name hides the package, and the import line
  cannot say which was meant. Rename the directory, or move the module so that
  one path names it.
```

The written path is tried before the search roots, so a directory beside your
source that shares a package's name shadows the package. Both candidates are real
files with, by construction, the same name, so one is being compiled and the other
silently is not, and which is which depends on a directory layout the import line
never mentions. That is not a preference anybody expressed, so there is nothing to
honour: rename the directory, or write a path that names one file. It costs
correct programs nothing, because across this repository's imports no written path
and no search root ever name two different files.

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

Two files that each keep a *private* helper of the same name are fine: the
compiler qualifies those with the module's own name. This fires when both are
API, which is where renaming is a decision only the author can make. The failure
it replaced was a C `redefinition` error naming a function the reader never wrote
twice.

**A reference nothing settles** is the same clash seen from a third file:

```
apps/site/home.maca: `render` is defined by more than one module this file
reaches, and every module is inlined into one:
  modules/tomo/page.maca
  modules/tomo/feed.maca
  Nothing here says which one `render` means. Ask for the one you mean with
  `import { render } from …`, or rename the others.
```

The difference from the one above is where the ambiguity is: there, two modules
answer for the name; here, a *third* module writes it and nothing in that file
says which it meant. A selective import is the answer either way, because naming
what you want is the one thing that settles it.

**An import that resolves to no file** is an error too, including a single-word
selective import, because there is nothing to select from a builtin:

```
apps/x/main.maca: no module `std/str`: `std/str.maca` is not beside this file
or in the working directory
```

Four files once imported a `std/str` that has never existed, silently, and each
then hand-wrote the helpers it believed it was importing.

## Errors that are not diagnostics

Some failures come from further down the pipeline and read differently.

**Parse and lex errors** name a byte range:

```
lex (28, 28): string literal spans a line; write `\n`, or use a raw
"""…""" string. (A literal brace is `\{` or `{{`.)
```

An ambiguous `=> { … }` is one of these. A record literal and a block read the
same when every entry is a distinct `name = value` and only newlines separate
them, so neither reading is taken:

```
parse (45, 46): `mk`: this `=> { … }` reads as a record literal and as a
block. Write `Name { … }` for the record, or drop the `=>` for the block
```

[Syntax](a5-syntax.md) has the full rule and what decides it in the cases that
are not ambiguous.

**Backend refusals**: valid code that a particular target cannot emit:

```
`on:click` needs a live DOM; build this with `--target js`
```

An event handler has nowhere to attach when elements render to a string
([the UI syntax](a11-ui.md)), so the native target says so rather than emitting
markup that silently does nothing. Each target refuses what it cannot honour,
and for the
same reason: the alternative is an error about generated code you never
wrote:

| Target | Refuses |
|---|---|
| native | `on:click=` and its siblings |
| `rust` | a bodyless (FFI) function; `import c`/`import py`; an `import rust` naming an undeclared crate; a borrowed foreign parameter that is returned or stored |
| `embedded` | `info` and the other console builtins; a `main` with a return type |

**C compiler errors** should not happen, and when they do it is a compiler bug
worth reporting. A misspelt method used to be the exception, surviving to
the linker as `undefined reference to 'slice'`, but the method set of a `str`
or a `T[]` is closed, so a name outside it is now caught here, with a
suggestion where there is a near miss. Method calls on an `any` receiver stay
gradual, because that is how foreign code is reached.
