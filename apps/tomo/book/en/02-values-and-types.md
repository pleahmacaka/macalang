# Values and Types

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

```
info("n = {n}")        // interpolates n
info("{{}}")           // prints {}
info("\{\}")           // the same, spelled with escapes
```

Getting this wrong is a compile error, not a surprise: `"{"` starts an
interpolation the closing quote never ends, and the compiler says so.

A `"…"` string stays on one line — write `\n` for a newline. For text that
really is multi-line, use a **raw** string, which spans lines and interpolates
nothing, so braces inside it need no escaping:

```
css = """
body { margin: 0 }
"""
```

## Bindings: mutable vs constant

A bare lowercase name binds a **mutable** variable:

```
count = 0
count = count + 1   // fine
```

A `const`, an `as const`, or a **Capitalized** name binds a **constant**.
Reassigning it is a compile error:

```
const Limit = 100
Limit = 200         // error: cannot reassign a constant
```

There is no `let`. Introducing a name and updating it use the same `=`.

## Records

A record groups named fields. Declaring the type and building a value:

```
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

```
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

```
xs = [10, 20, 30]
info("first {xs.first()}, length {xs.length()}")
doubled = xs.map(n => n * 2)          // [20, 40, 60]
big     = xs.filter(n => n > 15)      // [20, 30]
total   = xs.reduce(0, (a, b) => a + b)
```

## Type annotations are inference hints

You annotate function parameters and returns; inside a body, types flow on their
own. When Maca meets something it cannot know — an unknown stdlib value, a
foreign call — it falls back to a gradual `any` rather than rejecting your
program, so interop stays smooth while your own code stays strict.

Next up: functions and control flow in depth.
