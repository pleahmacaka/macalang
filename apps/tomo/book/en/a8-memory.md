# Memory and Ownership

The rules the compiler follows when it decides that a value is finished with.
Why it works this way is [Memory Without a Garbage Collector](04-memory.md);
this chapter is what it does.

Nothing here is something you write. It is here because it explains why memory
behaves the way it does, and because two of the rules have consequences you can
observe from inside a program.

## What the compiler inserts

Every heap block carries a count of how many owners it has. `maca_alloc` hands
one back with a single owner; the compiler emits the calls that add and remove
owners, and when the last one lets go the block goes on a free-list where the
next request of that size picks it up.

You never write those calls. The rule the compiler follows is worth knowing
anyway, because it explains why memory behaves the way it does:

**A local owns its buffer if it built it and cannot outlive its block.** A value
bound from another name is an alias and owns nothing. A value that leaves, as
the block's result, in an outer binding, inside a structure, or as an argument,
belongs to whoever it went to. Everything else is released when its block ends.

Reading a value never counts as taking it, so a method call leaves ownership
where it was: `xs.length()` and `xs.sort()` both leave `xs` owned by the block
that built it. This matters, because a value you never look at is not worth
building.

The effect is visible from inside the program:

```maca
build(n: int) -> int[] {
    xs = []
    for i in 1..n {
        xs = xs.push(i)
    }
    xs
}

main() -> int {
    for round in 1..500 {
        ys = build(200)
    }
    info("reused {reuse_count() * 100 / alloc_count()}% of allocations")
    0
}
```

Over 90%: the loop settles on one buffer and keeps handing it back and forth
with the allocator, rather than asking for five hundred.

`alloc_count()` and `reuse_count()` are runtime counters, not a debug build's
instrumentation: they are the same two numbers in a release binary, so a
program can assert on its own allocation behaviour in a test.

## The escape analysis is deliberately lopsided

Missing an escape would free a buffer someone still holds; missing a
*non*-escape only means holding memory a little longer than necessary. So when
the compiler cannot tell, it keeps the value.

That is why the whole test suite runs valgrind-clean: nothing live is ever
released, and whatever a program is still holding when it exits is drained then.
It is also why a measurement of peak memory is a fair number and a measurement
of *promptness* is not always. A value the analysis could not prove dead lives
to the end of its block.

## Reuse

When a value's owner count is one and it is about to be rebuilt, the memory is
reused in place rather than freed and reallocated.

| Written | Compiles to, when the input is uniquely owned |
|---|---|
| `p with { f = v }` | a field store into `p`'s memory |
| `xs.map(f)` | a walk over `xs`'s buffer, writing each slot |
| `xs.push(x)` | a grow of `xs`'s buffer |
| `xs.sort()` | a sort of `xs`'s buffer |

When it is *not* uniquely owned, because something else still holds it, the same
expression allocates, and the original is untouched. The observable semantics
are the same either way, which is the point: the functional reading is always
correct and the imperative cost is what you get when nothing else is looking.

## Cycles

Reference counting cannot collect a cycle, and Maca does not run a cycle
collector. A tree, a list, an AST: all acyclic, all fine. A doubly-linked list
whose nodes point back at their parents will hold itself alive for the life of
the process.

There is no diagnostic for this. It is the one memory bug the language does not
rule out, and the reason it is tolerable is that the shapes that cause it are
shapes you have to go out of your way to build: the self-hosted compiler is a
large program made entirely of trees.

## What a recursive type costs

Two shapes need help from the backend, and both are shapes real programs want.

A **recursive sum payload** is boxed, held behind a heap pointer, so the tagged
union stays a finite size. A `Tree = Leaf | Node(int, Tree, Tree)` allocates per
node, and a `match` that binds the payload dereferences it.

A **recursive record field** is a definition cycle in C: the array type needs
the struct's size and the struct needs the array. The backend breaks it by
emitting the array's struct declaration *before* the record body
(`MACA_ARRAY_STRUCT`) and the array's operations *after* it (`MACA_ARRAY_OPS`),
so the cycle resolves without a forward reference in your source.

```maca
Expr = {
    kind: int
    text: str
    children: Expr[]
}
```

This is a supported shape rather than an accident, and it exists because the
self-hosted AST needed it. See
[The Self-Hosted Compiler](a15-self-hosting.md).

## What there is not

No `mut`, no `&`, no lifetimes, no `clone()`, no `Rc`, no `unsafe`. Those are
the vocabulary of a system where the programmer proves the ownership. Here the
compiler proves what it can and counts what it cannot, and there is nothing in
the surface syntax to say either way.
