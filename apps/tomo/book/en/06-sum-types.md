# Sum Types and Matching

A record says *all of these at once*. A sum type says *exactly one of these*. It
is the other half of the data model, and the one that makes illegal states
impossible to write down.

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
`Name = …`, and what follows decides which kind it is. Braces make a record,
bars make a sum.

## Matching

`match` takes a value apart:

```maca
area(s: Shape) -> int =>
    match s {
        Circle(r)  => 3 * r * r
        Rect(w, h) => w * h
    }
```

Each arm is `pattern => expression`. A variant's payload is bound by naming it in
the pattern: `Circle(r)` matches a circle and binds its field to `r`.

`match` is an **expression**, so it can be a whole arrow body, as here. It can
also stand as a statement.

## Exhaustiveness

The compiler checks that a `match` covers every variant. Leave one out:

```maca
name(c: Color) -> str =>
    match c {
        Red   => "red"
        Green => "green"
    }
```

and you get a diagnostic rather than a surprise at run time:

```
NonExhaustive: match on `Color` is not exhaustive; missing: Blue
```

This is the payoff for sum types. Add a variant to `Color` a year later and the
compiler walks you to every `match` that now has a hole. A language with an
`enum` of integers and a `switch` cannot do that.

The catch-all `_` matches anything:

```maca
is_red(c: Color) -> bool =>
    match c {
        Red => true
        _   => false
    }
```

Use it when you mean "everything else, forever", not to silence a warning you
would rather have.

## Matching lists

Patterns are not only for sum types. A list can be taken apart by shape:

```maca
describe(xs: int[]) -> str =>
    match xs {
        []          => "empty"
        [x]         => "one: {x}"
        [x, ..rest] => "head {x}, then {rest.length()} more"
    }
```

`[]` matches an empty list, `[x]` a list of exactly one, `[x, ..rest]` a head and
the remainder. The brackets may be dropped when it is unambiguous: `x, ..rest`
means the same as `[x, ..rest]`.

## Recursive sums

A variant can carry the type being declared, which is how you build a tree:

```maca
Tree = Leaf | Node(int, Tree, Tree)

sum(t: Tree) -> int =>
    match t {
        Leaf          => 0
        Node(v, l, r) => v + sum(l) + sum(r)
    }
```

The payload is boxed, so the type is not infinitely sized. This is the shape a
compiler's AST takes, and it is what `selfhost/ast.maca` is built from.

## Choosing between a record and a sum

The question to ask is whether the fields are simultaneous or alternative.

A user has a name *and* an email: record. A payment is a card *or* a transfer
*or* an invoice: sum. A request that is pending has no response body, and one
that failed has no result: that is a sum, even though it is tempting to write a
record with three nullable fields. Maca has no null, so the temptation is easier
to resist than it is elsewhere.

The two nest freely:

```maca
Status = Pending | Done(str) | Failed(str)

Job = {
    id: int
    status: Status
}
```

Now a `Job` cannot be simultaneously done and failed, and cannot be done without
a result, not by convention, but because there is no way to write it down.

## Run it

```
maca run examples/tree.maca
```

A recursive sum type (a binary tree) built, summed and printed. Then try
deleting one arm of a `match` in it and building again: the diagnostic names the
variant you dropped, which is the entire reason to reach for a sum type.
