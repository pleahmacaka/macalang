# Introduction

Welcome to **The Maca Handbook**.

Maca is a single, typed language for **both programs and infrastructure
configuration**. The same source you write to build an app is the source you
write to describe the machine it runs on — no second language, no YAML.

## What makes Maca different

- **One language, many targets.** A general program compiles to a native binary
  (through C), to JavaScript, to the JVM, to Rust, or to freestanding C for a
  microcontroller. A program written in *config mode* compiles to Nix. Same
  language, no dialects.
- **Records and sum types, no ceremony.** Data is either a record
  (`Point = { x: int, y: int }`) or a sum (`Shape = Circle | Rect`), and `match`
  takes them apart, exhaustively. There are no classes and no `null`.
- **Colorblind async.** There is no `async` keyword. Concurrency is an *inferred
  effect*: `spawn f(x)` runs `f` concurrently, `await` suspends until it
  resolves, and an ordinary function becomes async simply by using them.
- **No garbage collector and no borrow checker.** Memory is managed by reference
  counting that the compiler mostly optimises away. You write the functional
  version and get the imperative performance, with nothing to annotate.
- **Config is code, checked like code.** Infrastructure written in Maca is
  type-checked before it ever reaches a machine; an unknown option or an impure
  expression in config mode is a compile error, not a 3 a.m. surprise.

## Who this book is for

You have written code before — the exact language does not matter. This handbook
teaches Maca from the ground up: values, functions, its data model, the type
system, effects, and the toolchain around it.

## How to read it

Chapters build on each other, so front to back is the intended path.

- **Chapters 1–3** get you running and cover the everyday language.
- **Chapter 4** explains the memory model — read it once, then forget it.
- **Chapters 5–8** are the data model: records, sum types, modules, collections.
- **Chapters 9–12** are errors, the type system, closures and testing.
- **Chapters 13–17** are what Maca does that other languages don't: colorblind
  async, config mode, the targets, FFI, and the tooling.
- **Chapters 18–20** build three real programs: a linter, the compiler itself,
  and the generator that produced this book.
- **The appendices** are reference: keywords, operators, the standard library,
  and every diagnostic.

## A note on honesty

Maca is young, and this book says so where it matters. Chapter 4 describes a
memory model whose compile-time half is not implemented yet. Chapter 8 documents
a method that fails at link time instead of type-checking. Appendix C lists what
the standard library does not have.

That is deliberate. Everything runnable in this book has been run — the
repository holds `examples/handbook.maca`, which contains the book's claims as
one program that the test suite executes. Writing this handbook found six real
bugs in the compiler, because prose that reads well is not the same as prose
that is true.

Ready? [Let's get set up.](01-getting-started.md)
