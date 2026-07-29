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

**Argument count** — arity is a type property, so it lands here:

```
TypeMismatch: call to `f` expects 2 argument(s), got 3
```

**Disagreeing branches** — an `if` or ternary whose arms have different types:

```
TypeMismatch: ternary branches disagree: type mismatch: expected int, found str
```

This one catches a common slip: `if` and `match` are expressions, so their
branches have to agree even when you were treating the whole thing as a
statement. `c ? continue : 0` fails for this reason — `continue` has no value.

**Record fields** — a literal names every field the record declares, and no
field it doesn't:

```
TypeMismatch: `Config` is missing field(s): port, title
TypeMismatch: `Config` has no field `titel`; did you mean `title`?
```

A missing field would otherwise be a silent zero — `""` for a `str`, `0` for an
`int` — and a misspelt one is worse, because the value goes nowhere and the
field it was meant for stays empty. Both compiled clean before this check, and
what you saw was a page with a heading missing. `base with { f = v }` is the
update form and is deliberately partial; only a construction is checked.

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
Immutable: cannot reassign constant `Limit` — declare it mutable with
`Limit = …` (no `const`)
```

Three things make a binding constant: `const`, a trailing `as const`, and a
**Capitalized** name. The third catches people out — `Total = 0` is a constant
because of the capital letter. `maca lint` nudges toward writing `const`
explicitly for exactly this reason.

## UndefinedName

A name that is defined nowhere — not a local, not a function, not an import, not
a builtin.

```
UndefinedName: call to undefined function `helprr`
```

Without this check, a typo would reach the C compiler and come back as an
undefined reference at link time. It applies to lowercase names in call position;
capitalised names are constructors, and UFCS method calls stay gradual.

It also covers the keywords Maca doesn't have:

```
UndefinedName: `return`: Maca has no `return` — a function's last expression
is its value
```

See [Keywords](a1-keywords.md) for the full list of those.

## UnknownOption

In config mode, an assignment to an option **namespace** the compiler does not
know.

```
UnknownOption: unknown NixOS option namespace `servicez`
```

The roots it knows are the NixOS ones — `networking`, `services`, `system`,
`users`, `environment`, `programs`, `boot`, `hardware`, `security`, `nix`,
`fonts` and their siblings — plus any local binding in the file.

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
call — a known builtin name, or a method on one of those receivers — so
`file.read(p)` is caught and the free function `read_file(p)` is not. A config
that reads a file through the free builtin compiles today. The rows and what
introduces each are [Effects and Async](a7-effects.md).

## Errors that are not diagnostics

Some failures come from further down the pipeline and read differently.

**Parse and lex errors** name a byte range:

```
lex (28, 28): string literal spans a line; write `\n`, or use a raw
"""…""" string. (A literal brace is `\{` or `{{`.)
```

**Backend refusals** — valid code that a particular target cannot emit:

```
`on:click` needs a live DOM — build this with `--target js`
```

An event handler has nowhere to attach when elements render to a string
([the UI syntax](a11-ui.md)), so the native target says so rather than emitting
markup that silently does nothing. Each target refuses what it cannot honour,
and for the
same reason — the alternative is an error about generated code you never
wrote:

| Target | Refuses |
|---|---|
| native | `on:click=` and its siblings |
| `rust` | a bodyless (FFI) function; `import c`/`import py`; an `import rust` naming an undeclared crate; a borrowed foreign parameter that is returned or stored |
| `embedded` | `info` and the other console builtins; a `main` with a return type |

**C compiler errors** should not happen, and when they do it is a compiler bug
worth reporting. A misspelt method used to be the exception — it survived to
the linker as `undefined reference to 'slice'` — but the method set of a `str`
or a `T[]` is closed, so a name outside it is now caught here, with a
suggestion where there is a near miss. Method calls on an `any` receiver stay
gradual, because that is how foreign code is reached.
