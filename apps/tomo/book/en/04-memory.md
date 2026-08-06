# Memory Without a Garbage Collector

Every language has to answer when memory goes away. Roughly three answers:

- **Manual**: you say. C, Zig. Fast, and the source of a large fraction of all
  security bugs ever shipped.
- **Traced garbage collection**: a collector finds out. Go, Java, JavaScript.
  Safe, at the cost of a runtime, a pause, and a floor on memory use.
- **Static ownership**: the compiler proves it. Rust. Safe and fast, paid for in
  lifetimes, borrows, and a learning cliff.

## The goal

Maca targets a C-tier binary while staying teachable, which rules out both a
tracing collector and borrow checking. What is left is **reference counting**.

The classic objections are that it is slow and that it leaks cycles. The first
is an artifact of naive implementations: most counter updates are provably
unnecessary and can be removed at compile time. That is **Perceus**, the scheme
Maca is built around.

## Perceus, briefly

Perceus, *precise reference counting with reuse and specialization*, comes from
the Koka language.

1. The compiler tracks ownership statically, as Rust does, but does not *reject*
   programs that don't fit. Where ownership is clear it emits no counter traffic
   at all; where it isn't, it falls back to a runtime count.
2. When a value's count drops to one and it is about to be rebuilt, the memory
   is **reused in place** rather than freed and reallocated. A `with` update on
   a record, or a map over a list, can compile to a mutation of the buffer you
   already had.

You write `p with { y = 5 }` and get an in-place field store. That is why Maca
has no `mut`, no `&`, no lifetimes, and no `clone()`.

## What this means when you write code

**Prefer building a new value to mutating a shared one.** When nothing else
holds `base`, `base with { f = v }` is a store, not a copy.

```maca
Point = {
    x: int
    y: int
}

move_up(p: Point) -> Point =>
    p with { y = p.y + 1 }
```

**Don't build reference cycles and expect them to go away.** Maca runs no cycle
collector. A tree, a list, an AST: all fine, all acyclic. A doubly-linked list
whose nodes point back at their parents holds itself alive.

## You can watch it happen

The allocator keeps two counters, and a program can read them:

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

Over 90%: five hundred rebuilds of a two-hundred-element list, and almost no
allocation. The exact rule is [Memory and Ownership](a8-memory.md).

## Comparison at a glance

| | Runtime | Frees promptly | Handles cycles | Learning cost |
|---|---|---|---|---|
| Manual (C) | none | if you do | if you do | high, in bugs |
| Tracing GC (Go) | yes | no | yes | none |
| Ownership (Rust) | none | yes | with effort | high, in types |
| Perceus (Maca) | none | yes | no | none |
