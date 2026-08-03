# The Type System

What the checker does, precisely. Maca is statically typed and mostly inferred:
you annotate function boundaries and everything inside follows.

The teaching version of this is spread through
[Learning Maca](03-common-concepts.md); this chapter is the rule.

## Inference at the boundary

A function's signature is the contract:

```maca
double(n: int) -> int =>
    n * 2
```

Locals are never annotated:

```maca
half(n: int) -> int {
    m = n / 2      // m is int; nothing to declare
    m
}
```

The return type may be omitted, and is then inferred from the body:

```maca
inc(x) => x + 1
```

Both the parameter and the result are inferred here. Prefer writing them: a
signature is documentation the compiler checks, and the error messages get much
better when there is something to disagree with.

A local may carry an annotation where inference has nothing to go on. An empty
map has no value type until something is put in it:

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

`a` here is not a type called "a". It is "any type, the same one in all three
places". There is no `<T>` to declare, because lowercase already means variable
and uppercase already means concrete.

Function signatures generalise into schemes and instantiate afresh at each call,
so `identity(1)` and `identity("x")` both work and neither constrains the other.
This is standard Hindley-Milner inference.

The C backend **monomorphises**: each distinct instantiation becomes its own
specialised function in the generated C. There is no boxing and no dispatch at
run time: a generic function costs exactly what the hand-written specialisation
would.

## Gradual typing and `any`

Not everything can be known. Foreign functions, some standard-library corners,
and values crossing a backend boundary have no Maca type to give. For those
there is `any`, and it unifies with everything.

This is why Maca is called *gradually* typed: the strict core does real
inference, and `any` is an escape hatch at the edges rather than a hole in the
middle. Practically, it means calls into unknown territory do not produce a
cascade of spurious errors, but also that a mistake in that territory is not
caught.

Method calls are one such place, and the exception is worth knowing. The method
set of a `str` and of a `T[]` is **closed**, so a name outside it is a typo and
is reported with a suggestion rather than surviving to the linker. An `any`
receiver stays gradual, because that is how foreign code is reached. The two
closed sets are listed in [The Standard Library](a3-stdlib.md), and a test
compiles and runs every name in them.

## A record type and a record literal

`Point = { x: int, y: int }` names a type, and `{ x = 5, y = 6 }` writes one
without naming it. They are the same type, and a literal *becomes* the named
record wherever the context names one:

```maca
Point = { x: int, y: int }

origin: Point = { x = 0, y = 0 }
mk() -> Point => { x = 1, y = 2 }
far(p: Point) -> int => p.x + p.y
```

An annotation, a return type, a parameter, a field of another record and an
element of a `Point[]` all count as naming one. Becoming the named type rather
than staying a lookalike is what makes the value a real `Point` for the rest of
its life: the record's own struct is what gets built, no second one is
synthesized, and `with`, a declared field's type and an overloaded operator all
work on it.

Meeting the declaration is also what checks the literal against it. A record
literal is otherwise **open**, because reading a field should not require knowing
every other field, and left open here a field nobody wrote would be silently
zero. So a literal written into a named record owes the same two things the
`Point { … }` spelling owes: every field the record declares, and no field it
doesn't.

```
TypeMismatch: in `p`: record is missing field `y`
TypeMismatch: in `p`: record has unexpected field `z`
```

With no such context a literal stays structural, and two literals of the same
shape are one type. The native and Rust back ends synthesize one struct per
distinct shape for those.

## Effects

A function's type is not only its arguments and result. The checker also tracks
what a function *does* (its **effects**), and effects are inferred, never
declared. [Effects and Async](a7-effects.md) is the full account: the rows, what
introduces each one, and where each is enforced.

## The diagnostics

Six kinds, and each means one specific thing:

| Diagnostic | Meaning |
|---|---|
| `TypeMismatch` | two types had to be the same and weren't |
| `NonExhaustive` | a `match` doesn't cover every variant |
| `Immutable` | assignment to a constant |
| `UndefinedName` | a call to a name defined nowhere |
| `UnknownOption` | a config option that doesn't exist |
| `EffectInConfig` | an impure operation in config mode |

`TypeMismatch` covers more than it sounds like. Call arity is a mismatch:

```
TypeMismatch: call to `f` expects 2 argument(s), got 3
```

So are disagreeing branches of an `if` or a ternary, which is a genuinely common
mistake:

```maca
x = c ? 1 : "two"
// TypeMismatch: ternary branches disagree: expected int, found str
```

Every message the six can produce, and what to do about each, is
[Diagnostics](a4-diagnostics.md).

## Constants

A binding is mutable by default:

```maca
count = 0
count = count + 1
```

Three things make it constant: `const`, a trailing `as const`, or a Capitalized
name:

```maca
const Limit = 100
step = 5 as const
Origin = 0
```

Reassigning any of them is a compile error:

```
Immutable: cannot reassign constant `Limit`; declare it mutable with
`Limit = …` (no `const`)
```

The Capitalized rule exists because it matches how people already write
constants, but it is implicit, and `maca lint` will nudge you toward writing
`const` where you mean it.

## Reading an error

Type errors name the function and the position:

```
TypeMismatch: in call to `d` (argument 2): type mismatch: expected P, found int
```

When the message is about a type variable rather than a concrete type, the usual
cause is a missing annotation somewhere upstream: the checker inferred something
more general than you meant. Adding the signature you had in mind will either fix
it or tell you exactly where your mental model and the code disagree.
