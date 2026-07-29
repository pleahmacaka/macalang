# Records

A record groups named fields under a type name. It is Maca's struct, and it is
the workhorse of the language.

## Declaring and building

```maca
Point = {
    x: int
    y: int
}
```

The declaration reads `Name = { … }` — the same `=` that binds a value, because
a type declaration *is* a binding at the top level. Inside, one `field: type`
per line. No commas are needed at line ends, though they are allowed.

Building a value uses `=`, not `:`:

```maca
p = Point { x = 3, y = 4 }
```

This is the single most useful punctuation rule in Maca: **`:` says what
something is, `=` says what it holds.** It holds in record declarations, record
literals, function parameters, and bindings.

Fields are read with a dot:

```maca
info("{p.x}, {p.y}")
```

## Updating

Records are values. To get a changed one, use `with`:

```maca
q = p with { y = 5 }
```

`q` is a `Point` with `x = 3, y = 5`; `p` is untouched. Several fields can be
updated at once, and the fields not mentioned come across unchanged:

```maca
r = p with { x = 0, y = 0 }
```

This is not necessarily a copy. [Memory](04-memory.md) explains why: when
nothing else is
holding `p`, the compiler is free to make `with` a store into the memory that was
already there. Writing the functional version and getting the imperative
performance is the whole design.

Direct field assignment also exists, for when a record is a local you are
building up:

```maca
p.x = 10
```

## Records in signatures

A record type is used like any other:

```maca
Rect = {
    w: int
    h: int
}

area(r: Rect) -> int =>
    r.w * r.h

scale(r: Rect, k: int) -> Rect =>
    r with { w = r.w * k, h = r.h * k }
```

And because of UFCS — `x.f(y)` is `f(x, y)` — those read as methods at the call
site without ever being declared as ones:

```maca
big = Rect { w = 2, h = 3 }.scale(10)
info("area: {big.area()}")
```

There is no `impl` block, no `self`, and no distinction between a "method" and a
"function whose first argument is a Rect". A free function is all you need.

## Records containing records

Fields can be any type, including other records and lists of them:

```maca
Line = {
    from: Point
    to: Point
}

Shape = {
    name: str
    points: Point[]
}
```

Access nests the obvious way: `l.from.x`.

A record may also refer to *itself*, which is how trees are built:

```maca
Node = {
    label: str
    children: Node[]
}
```

This works, and it is worth knowing why it needs help from the compiler. In C, a
struct containing an array of itself is a definition cycle — the array type needs
the struct's size, and the struct needs the array. Maca's C backend breaks it by
forward-declaring the array struct before the record body and emitting the
operations after, so the cycle resolves. You never see any of this; it matters
only because it means self-referential records are a supported thing rather than
an accident.

## Records vs. sum types

A record says "all of these at once". When you need "one of these", that is a
sum type — the next chapter. The two compose: a sum type's variants can carry
records, and a record's field can be a sum type. Most real data models are a
handful of each.

## Records without a name

A record literal with no type name in front — `{ host = "x", port = 80 }` —
infers its type from the fields present:

```maca
c = { host = "localhost", port = 8080 }
info("{c.host}:{c.port}")
```

Two literals with the same fields are the same type, whatever order they were
written in, so one can be assigned to the other:

```maca
d = { port = 80, host = "example.com" }
c = d                                    // fine — same shape
```

Reach for it when the shape is used once, in one place: a pair of values to
return together, a row about to be rendered. When the shape appears more than
twice, name it. A named type is what a diagnostic can talk about, and it gives
the shape somewhere to grow a comment.

## Run it

```
maca run examples/record_update.maca
```

`with` on a record, and the original left untouched afterwards. That second half
is the claim worth watching: the update is a store into the same memory when
nothing else holds the value, and a copy when something does, and the program
cannot tell the difference.
