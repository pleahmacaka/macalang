# The Type System

Maca is statically typed and mostly inferred: you annotate function boundaries
and everything inside follows. The teaching version is
[Learning Maca](03-common-concepts.md).

## Inference at the boundary

A function's signature is the contract; locals are never annotated:

```maca
double(n: int) -> int =>
    n * 2
```

```maca
half(n: int) -> int {
    m = n / 2      // m is int; nothing to declare
    m
}
```

The return type may be omitted, and is inferred from the body:

```maca
inc(x) => x + 1
```

Prefer writing them: the error messages get better when there is something to
disagree with. A local may carry an annotation where inference has nothing to go
on:

```maca
counts: Map str int = map()
```

## Generics without angle brackets

A **lowercase** name in a type position is a type variable:

```maca
identity(x: a) -> a =>
    x

pair_first(xs: a[], fallback: a) -> a =>
    xs.length() > 0 ? xs.first() : fallback
```

`a` is "any type, the same one in all three places". There is no `<T>` to
declare.

## Gradual typing and `any`

Foreign functions, some standard-library corners, and values crossing a backend
boundary have no Maca type to give. For those there is `any`, which unifies with
everything: an escape hatch at the edges rather than a hole in the middle. Calls
into unknown territory do not produce a cascade of spurious errors, but a
mistake there is not caught.

The method set of a `str` and of a `T[]` is **closed**, so a name outside it is
reported with a suggestion rather than surviving to the linker. An `any`
receiver stays gradual. The two sets are in
[The Standard Library](a3-stdlib.md).

## A record type and a record literal

`Point = { x: int, y: int }` names a type, and `{ x = 5, y = 6 }` writes one
without naming it. A literal *becomes* the named record wherever the context
names one:

```maca
Point = { x: int, y: int }

origin: Point = { x = 0, y = 0 }
mk() -> Point => { x = 1, y = 2 }
far(p: Point) -> int => p.x + p.y
```

Meeting the declaration is also what checks the literal against it, since a
record literal is otherwise **open** and a field nobody wrote would be silently
zero. Such a literal owes what the `Point { … }` spelling owes: every field the
record declares, and no field it doesn't.

```
TypeMismatch: in `p`: record is missing field `y`
TypeMismatch: in `p`: record has unexpected field `z`
```

With no such context a literal stays structural, and two literals of the same
shape are one type; the native and Rust back ends synthesize one struct per
shape.

## Effects

The checker also tracks what a function *does*, inferred and never declared. See
[Effects and Async](a7-effects.md).

## The diagnostics

| Diagnostic | Meaning |
|---|---|
| `TypeMismatch` | two types had to be the same and weren't |
| `NonExhaustive` | a `match` doesn't cover every variant |
| `Immutable` | assignment to a constant |
| `UndefinedName` | a call to a name defined nowhere |
| `UnknownOption` | a config option that doesn't exist |
| `EffectInConfig` | an impure operation in config mode |

`TypeMismatch` covers more than it sounds like: call arity, and disagreeing
branches of an `if` or a ternary.

```
TypeMismatch: call to `f` expects 2 argument(s), got 3
```

```maca
x = c ? 1 : "two"
// TypeMismatch: ternary branches disagree: expected int, found str
```

Every message the six can produce is [Diagnostics](a4-diagnostics.md).

## Constants

A binding is mutable by default:

```maca
count = 0
count = count + 1
```

Three things make it constant: `const`, a trailing `as const`, or a Capitalized
name, and reassigning any of them is a compile error:

```maca
const Limit = 100
step = 5 as const
Origin = 0
```

```
Immutable: cannot reassign constant `Limit`; declare it mutable with
`Limit = …` (no `const`)
```

`maca lint` nudges you toward writing `const` where you mean it.

## Reading an error

Type errors name the function and the position:

```
TypeMismatch: in call to `d` (argument 2): type mismatch: expected P, found int
```

A message about a type variable rather than a concrete type usually means a
missing annotation upstream: the checker inferred something more general than
you meant.
