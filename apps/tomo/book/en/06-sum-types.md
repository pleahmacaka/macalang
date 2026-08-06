# Sum Types and Matching

A record says *all of these at once*. A sum type says *exactly one of these*.

## Declaring

Variants are separated by `|`:

```maca
Color = Red | Green | Blue
```

The variants are values of the type:

```maca
c = Green
```

A variant may carry data, written like a call:

```maca
Shape = Circle(int) | Rect(int, int)

s = Circle(2)
t = Rect(3, 4)
```

There is no `enum` keyword and no `struct` keyword. A type declaration is
`Name = …`: braces make a record, bars make a sum.

## Matching

```maca
area(s: Shape) -> int =>
    match s {
        Circle(r)  => 3 * r * r
        Rect(w, h) => w * h
    }
```

Each arm is `pattern => expression`. A variant's payload is bound by naming it
in the pattern: `Circle(r)` matches a circle and binds its field to `r`.

`match` is an **expression**, so it can be a whole arrow body. It can also stand
as a statement.

## Exhaustiveness

The compiler checks that a `match` covers every variant. Leave one out:

```maca
name(c: Color) -> str =>
    match c {
        Red   => "red"
        Green => "green"
    }
```

and you get a diagnostic:

```
NonExhaustive: match on `Color` is not exhaustive; missing: Blue
```

Add a variant to `Color` a year later and the compiler walks you to every
`match` that now has a hole.

The catch-all `_` matches anything:

```maca
is_red(c: Color) -> bool =>
    match c {
        Red => true
        _   => false
    }
```

Use it when you mean "everything else, forever".

## Matching lists

A list can be taken apart by shape:

```maca
describe(xs: int[]) -> str =>
    match xs {
        []          => "empty"
        [x]         => "one: {x}"
        [x, ..rest] => "head {x}, then {rest.length()} more"
    }
```

`[]` matches an empty list, `[x]` a list of exactly one, `[x, ..rest]` a head
and the remainder. The brackets may be dropped when it is unambiguous:
`x, ..rest` means the same as `[x, ..rest]`.

## Recursive sums

A variant can carry the type being declared:

```maca
Tree = Leaf | Node(int, Tree, Tree)

sum(t: Tree) -> int =>
    match t {
        Leaf          => 0
        Node(v, l, r) => v + sum(l) + sum(r)
    }
```

The payload is boxed, so the type is not infinitely sized. This is the shape
`apps/selfhost/ast.maca` is built from.

## Choosing between a record and a sum

Ask whether the fields are simultaneous or alternative. A user has a name *and*
an email: record. A payment is a card *or* a transfer *or* an invoice: sum. A
request that is pending has no response body, and one that failed has no result:
sum. Maca has no null.

The two nest freely:

```maca
Status = Pending | Done(str) | Failed(str)

Job = {
    id: int
    status: Status
}
```

A `Job` cannot be simultaneously done and failed, and cannot be done without a
result, because there is no way to write it down.

## Run it

```
maca run apps/examples/tree.maca
```

A recursive sum type built, summed and printed. Delete one arm of a `match` in
it and build again: the diagnostic names the variant you dropped.
