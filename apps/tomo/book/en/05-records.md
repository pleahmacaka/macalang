# Records

A record groups named fields under a type name. It is Maca's struct.

## Declaring and building

```maca
Point = {
    x: int
    y: int
}
```

`Name = { … }`, then one `field: type` per line. Commas at line ends are allowed
and not needed. Building a value uses `=`, not `:`:

```maca
p = Point { x = 3, y = 4 }
```

**`:` says what something is, `=` says what it holds**, in declarations,
literals, parameters and bindings. Fields are read with a dot:

```maca
info("{p.x}, {p.y}")
```

## Updating

Records are values. To get a changed one, use `with`:

```maca
q = p with { y = 5 }
```

`q` is a `Point` with `x = 3, y = 5`; `p` is untouched. Fields not mentioned come
across unchanged:

```maca
r = p with { x = 0, y = 0 }
```

When nothing else holds `p`, `with` is a store into the memory that was already
there. See [Memory](04-memory.md). Direct field assignment also exists:

```maca
p.x = 10
```

## Records in signatures

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

Because of UFCS (`x.f(y)` is `f(x, y)`), those read as methods at the call
site:

```maca
big = Rect { w = 2, h = 3 }.scale(10)
info("area: {big.area()}")
```

There is no `impl` block, no `self`, and no distinction between a "method" and a
"function whose first argument is a Rect".

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

Access nests: `l.from.x`. A record may also refer to *itself*, which is how
trees are built:

```maca
Node = {
    label: str
    children: Node[]
}
```

In C a struct containing an array of itself is a definition cycle. Maca's C
backend breaks it by forward-declaring the array struct before the record body
and emitting the operations after.

## Records vs. sum types

A record says "all of these at once"; "one of these" is a sum type, the next
chapter. The two compose freely.

## Records without a name

A record literal with no type name infers its type from the fields present:

```maca
c = { host = "localhost", port = 8080 }
info("{c.host}:{c.port}")
```

Two literals with the same fields are the same type, in any order:

```maca
d = { port = 80, host = "example.com" }
c = d                                    // fine: same shape
```

Reach for it when the shape is used once. When it appears more than twice, name
it: a named type is what a diagnostic can talk about.

## Run it

```
maca run apps/examples/record_update.maca
```

`with` on a record, and the original left untouched. The update is a store when
nothing else holds the value and a copy when something does, and the program
cannot tell the difference.
