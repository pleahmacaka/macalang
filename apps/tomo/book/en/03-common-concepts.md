# Common Concepts

Maca is statically typed, but you rarely write types except at function
boundaries — the compiler infers the rest.

## Scalars

- `int` — a 64-bit integer: `42`, `-7`, `0xff`, `1_000_000`.
- `float` — a 64-bit float: `3.14`, `2.0`.
- `bool` — `true` or `false`.
- `str` — a string: `"hello"`, with interpolation: `"n = {n}"`.

## Strings

A `"…"` string interpolates: any `{expr}` inside it is evaluated and spliced in.
That makes a brace special, so a **literal** brace is escaped — either by
doubling it or with a backslash:

```maca
info("n = {n}")        // interpolates n
info("{{}}")           // prints {}
info("\{\}")           // the same, spelled with escapes
```

Getting this wrong is a compile error, not a surprise: `"{"` starts an
interpolation the closing quote never ends, and the compiler says so.

### Format specs

An interpolation can say how to render its value, after a `:`

```maca
info("{pi:.2}")        // 3.14        — two decimal places
info("{n:>8}")         // "      42"  — right-align in 8 columns
info("{name:<8}")      // "ok      "  — left-align
info("{name:^8}")      // "   ok   "  — centre
info("{n:08}")         // "00000042"  — zero-fill
info("{pi:>10.3}")     // "     3.142" — both
```

The spec is `[align][0][width][.precision]`, every part optional. Nothing new
happens at run time: a spec is spelling for calls you could write yourself —
`{x:.2}` is `x.fixed(2)`, `{x:>8}` is `str(x).pad_start(8, " ")` — so the same
specs work on every target.

A ternary inside an interpolation still works, because Maca writes a ternary
*spaced* (`c ? x : y`) and a format spec *attached* (`x:>8`) — the same
distinction that separates `x?` from `c ? x : y`:

```maca
info("{score >= 60 ? "pass" : "fail"}")
```

A `"…"` string stays on one line — write `\n` for a newline. For text that
really is multi-line, use a **raw** string, which spans lines and interpolates
nothing, so braces inside it need no escaping:

```maca
css = """
body { margin: 0 }
"""
```

## Bindings: mutable vs constant

A bare lowercase name binds a **mutable** variable:

```maca
count = 0
count = count + 1   // fine
```

A `const`, an `as const`, or a **Capitalized** name binds a **constant**.
Reassigning it is a compile error:

```maca
const Limit = 100
Limit = 200         // error: cannot reassign a constant
```

There is no `let`. Introducing a name and updating it use the same `=`.

## Records

A record groups named fields. Declaring the type and building a value:

```maca
Point = {
    x: int
    y: int
}

p = Point { x = 3, y = 4 }
info("x is {p.x}")            // field access with `.`
q = p with { y = 5 }         // functional update — p is unchanged
```

## Sum types

A sum type is one of several variants. `match` takes it apart:

```maca
Shape = Circle | Square | Triangle

sides(s: Shape) -> int =>
    match s {
        Circle   => 0
        Square   => 4
        Triangle => 3
    }
```

Because a value must be exactly one variant, `match` is exhaustive: leave a
variant out and the compiler tells you.

## Lists

A list holds many values of one type. The stdlib methods are UFCS — you call
them with `.`:

```maca
xs = [10, 20, 30]
info("first {xs.first()}, length {xs.length()}")
doubled = xs.map(n => n * 2)          // [20, 40, 60]
big     = xs.filter(n => n > 15)      // [20, 30]
total   = xs.reduce(0, (a, b) => a + b)
```

`n => n * 2` is a **lambda** — the same `=>` that gives a named function its
body, with the name left off. [Closures and Control Flow](11-closures.md) is
where it gets a chapter; here it is enough to read it as "the function that
doubles its argument".

## Type annotations are inference hints

You annotate function parameters and returns; inside a body, types flow on their
own. When Maca meets something it cannot know — an unknown stdlib value, a
foreign call — it falls back to a gradual `any` rather than rejecting your
program, so interop stays smooth while your own code stays strict.

## Run it

```
maca run examples/strings.maca
```

Every string method and every format spec above, in one program that prints what
each produces. Change a spec and run it again — a format spec is the cheapest
thing in the language to experiment with, because it desugars to a call you
could have written yourself.

## Where the full answer is

[Syntax](a5-syntax.md) in the reference is every form the language has, in
tables: declarations, expressions, statements, patterns, the string grammar, and
the rule for when an expression continues onto the next line.

Next: giving a group of values a name.
