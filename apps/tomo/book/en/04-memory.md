# Memory Without a Garbage Collector

Every language has to answer the question of when memory goes away. The answers
divide roughly in three:

- **Manual** — you say. C, Zig. Fast, and the source of a large fraction of all
  security bugs ever shipped.
- **Traced garbage collection** — a collector finds out. Go, Java, JavaScript.
  Safe, and it costs you a runtime, a pause, and a floor on your memory use.
- **Static ownership** — the compiler proves it. Rust. Safe and fast, and you
  pay for it in the type system: lifetimes, borrows, and a real learning cliff.

Maca takes a fourth road, and it is worth understanding *why*, because it
explains a lot of the rest of the language.

## The goal

Maca targets a C-tier binary — no runtime, no pauses — while staying a language
you can teach on a Tuesday. That rules out a tracing collector (the runtime) and
it rules out borrow checking (the cliff). What's left is **reference counting**,
which has a bad reputation it mostly no longer deserves.

The classic objections to reference counting are that it is slow (every
assignment touches a counter) and that it leaks cycles. The first is largely an
artifact of naive implementations: most counter updates in a real program are
provably unnecessary and can be removed at compile time. That observation is the
basis of **Perceus**, the scheme Maca is built around.

## Perceus, briefly

Perceus — *precise reference counting with reuse and specialization* — comes from
the Koka language. The idea:

1. The compiler tracks ownership statically, as Rust does, but it does not
   *reject* programs that don't fit. Where ownership is clear it emits no counter
   traffic at all; where it isn't, it falls back to a runtime count.
2. When a value's count drops to one and it is about to be rebuilt, the memory is
   **reused in place** rather than freed and reallocated. A `with` update on a
   record, or a map over a list, can compile to a mutation of the buffer you
   already had.

The result is that idiomatic functional code — build a new value from an old one
— compiles to roughly what you would have written by hand with a mutable buffer.
You write `p with { y = 5 }` and get an in-place field store.

This is why Maca has no `mut`, no `&`, no lifetimes, and no `clone()`. Those are
the vocabulary of a system where *you* prove the ownership. Here the compiler
does, and where it can't prove anything it quietly counts.

## What this means when you write code

Mostly: nothing. That is the point. But two habits follow from it.

**Prefer building a new value to mutating a shared one.** `base with { f = v }`
is not a copy in the general case — when nothing else holds `base`, it is a
store. Code written in the obvious functional style is the code the optimiser is
built for.

```maca
Point = {
    x: int
    y: int
}

move_up(p: Point) -> Point =>
    p with { y = p.y + 1 }
```

**Don't build reference cycles and expect them to go away.** Reference counting
cannot collect a cycle, and Maca does not run a cycle collector. A tree, a list,
an AST — all fine, all acyclic. A doubly-linked list where each node points back
at its parent will hold itself alive. In practice this is rarely a constraint;
the compiler in `selfhost/` is a large program built entirely from trees.

## You can watch it happen

The allocator keeps two counters, and a program can read them:

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

Over 90%. The loop settles on one buffer and keeps handing it back and forth
with the allocator rather than asking for five hundred — five hundred rebuilds
of a two-hundred-element list, and almost no allocation.

That is the whole argument for this design in one number, and it is a number
your own program can print.

The exact rule the compiler follows — which local owns what, when a value
escapes, what a recursive type costs — is
[Memory and Ownership](a8-memory.md) in the reference. You do not need it to
write Maca. You need it the day something holds memory longer than you expected.

## Comparison at a glance

| | Runtime | Frees promptly | Handles cycles | Learning cost |
|---|---|---|---|---|
| Manual (C) | none | if you do | if you do | high, in bugs |
| Tracing GC (Go) | yes | no | yes | none |
| Ownership (Rust) | none | yes | with effort | high, in types |
| Perceus (Maca) | none | yes | no | none |
