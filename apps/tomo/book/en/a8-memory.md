# Memory and Ownership

The rules the compiler follows when it decides that a value is finished with.
Why it works this way is [Memory Without a Garbage Collector](04-memory.md).

## What the compiler inserts

Every heap block carries a count of how many owners it has. `maca_alloc` hands
one back with a single owner; the compiler emits the calls that add and remove
owners, and when the last one lets go the block goes on a free-list where the
next request of that size picks it up.

The effect is visible from inside the program:

```maca
build(n: int) -> int[] {
    xs = []
    for i in 0..n {
        xs = xs.push(i)
    }
    xs
}

main() -> int {
    for round in 0..500 {
        ys = build(200)
    }
    info("reused {reuse_count() * 100 / alloc_count()}% of allocations")
    0
}
```

Over 90%: the loop settles on one buffer and keeps handing it back and forth
with the allocator.

`alloc_count()` and `reuse_count()` are runtime counters, not a debug build's
instrumentation: they are the same two numbers in a release binary, so a program
can assert on its own allocation behaviour in a test.

## The escape analysis is deliberately lopsided

Missing an escape would free a buffer someone still holds; missing a
*non*-escape only means holding memory a little longer than necessary. So when
the compiler cannot tell, it keeps the value.

## Reuse

When a value's owner count is one and it is about to be rebuilt, the memory is
reused in place rather than freed and reallocated.

| Written | Compiles to, when the input is uniquely owned |
|---|---|
| `p with { f = v }` | a field store into `p`'s memory |
| `xs.map(f)` | a walk over `xs`'s buffer, writing each slot |
| `xs.push(x)` | a grow of `xs`'s buffer |
| `xs.sort()` | a sort of `xs`'s buffer |

When it is *not* uniquely owned, the same expression allocates, and the original
is untouched. The observable semantics are the same either way.

## Cycles

Reference counting cannot collect a cycle, and Maca does not run a cycle
collector. A tree, a list, an AST: all acyclic, all fine. A doubly-linked list
whose nodes point back at their parents will hold itself alive for the life of
the process.

## What a recursive type costs

A **recursive sum payload** is boxed, held behind a heap pointer, so the tagged
union stays a finite size. A `Tree = Leaf | Node(int, Tree, Tree)` allocates per
node, and a `match` that binds the payload dereferences it.

A **recursive record field** is a definition cycle in C: the array type needs
the struct's size and the struct needs the array. The backend breaks it by
emitting the array's struct declaration *before* the record body
(`MACA_ARRAY_STRUCT`) and the array's operations *after* it (`MACA_ARRAY_OPS`).

```maca
Expr = {
    kind: int
    text: str
    children: Expr[]
}
```

The shape exists because the [self-hosted](a15-self-hosting.md) AST needed it.

## What there is not

No `mut`, no `&`, no lifetimes, no `clone()`, no `Rc`, no `unsafe`. The compiler
proves what it can and counts what it cannot, and there is nothing in the surface
syntax to say either way.
