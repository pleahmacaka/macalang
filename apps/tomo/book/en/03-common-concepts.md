# Common Concepts

Maca is statically typed, but you rarely write types except at function
boundaries.

## Scalars

- `int` is a 64-bit integer: `42`, `-7`, `0xff`, `1_000_000`.
- `float` is a 64-bit float: `3.14`, `2.0`.
- `bool` is `true` or `false`.
- `str` is a string: `"hello"`, with interpolation: `"n = {n}"`.

## Strings

Any `{expr}` inside a `"…"` string is evaluated and spliced in. A **literal**
brace is doubled or backslash-escaped:

```maca
info("n = {n}")        // interpolates n
info("{{}}")           // prints {}
info("\{\}")           // the same, spelled with escapes
```

### Format specs

An interpolation can say how to render its value, after a `:`

```maca
info("{pi:.2}")        // 3.14        (two decimal places)
info("{n:>8}")         // "      42"  (right-align in 8 columns)
info("{name:<8}")      // "ok      "  (left-align)
info("{name:^8}")      // "   ok   "  (centre)
info("{n:08}")         // "00000042"  (zero-fill)
info("{pi:>10.3}")     // "     3.142" (both)
```

`[align][0][width][.precision]`, every part optional. A spec is spelling for
calls you could write yourself (`{x:.2}` is `x.fixed(2)`, `{x:>8}` is
`str(x).pad_start(8, " ")`), so it works on every target.

```maca
info("{score >= 60 ? "pass" : "fail"}")
```

A `"…"` string stays on one line; write `\n` for a newline. A **raw** string
spans lines and interpolates nothing:

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

A `const`, an `as const`, or a **Capitalized** name binds a **constant**:

```maca
const Limit = 100
Limit = 200         // error: cannot reassign a constant
```

There is no `let`: introducing a name and updating it use the same `=`.

## Records

```maca
Point = {
    x: int
    y: int
}

p = Point { x = 3, y = 4 }
info("x is {p.x}")            // field access with `.`
q = p with { y = 5 }         // functional update: p is unchanged
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

## Lists

The stdlib methods are UFCS, so you call them with `.`:

```maca
xs = [10, 20, 30]
info("first {xs.first()}, length {xs.length()}")
doubled = xs.map(n => n * 2)          // [20, 40, 60]
big     = xs.filter(n => n > 15)      // [20, 30]
total   = xs.reduce(0, (a, b) => a + b)
```

`n => n * 2` is a **lambda**: the same `=>` a named function uses, with the name
left off. [Closures and Control Flow](11-closures.md) is its chapter.

## Type annotations are inference hints

You annotate function parameters and returns; inside a body, types flow on their
own. What Maca cannot know, such as a foreign call, falls back to a gradual
`any` rather than being rejected.

## Run it

```
maca run apps/examples/strings.maca
```

## Where the full answer is

[Syntax](a5-syntax.md) has every form in tables: declarations, expressions,
statements, patterns, the string grammar, and the line-continuation rule.
