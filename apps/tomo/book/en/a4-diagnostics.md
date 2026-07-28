# Appendix D: Diagnostics

Every diagnostic the checker emits, what it means, and what to do about it.

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

See appendix A for the full list of those.

## UnknownOption

In config mode, an option that the option set doesn't define.

```
UnknownOption: unknown option `services.nginx.enabl`
```

Config mode checks names against the real option schema, so a typo in a NixOS
option is caught at compile time rather than at deploy time.

## EffectInConfig

An impure operation in config mode.

```
EffectInConfig: `async` effect is not allowed in config mode
```

Configuration describes a desired state; it must not *do* anything. Reading a
file, printing, `spawn`, `await` and `sleep_ms` are all rejected. This is the
check that makes it safe for one language to be both a programming language and
a configuration language.

## Errors that are not diagnostics

Some failures come from further down the pipeline and read differently.

**Parse and lex errors** name a byte range:

```
lex (28, 28): string literal spans a line; write `\n`, or use a raw
"""…""" string. (A literal brace is `\{` or `{{`.)
```

**Backend refusals** — valid code that a particular target cannot emit:

```
expression not supported by the native backend: Record(…)
```

An anonymous record (chapter 5) is the case you are most likely to hit.

**C compiler errors** should not happen, and when they do it is a compiler bug
worth reporting. A misspelt method used to be the exception — it survived to
the linker as `undefined reference to 'slice'` — but the method set of a `str`
or a `T[]` is closed, so a name outside it is now caught here, with a
suggestion where there is a near miss. Method calls on an `any` receiver stay
gradual, because that is how foreign code is reached.
