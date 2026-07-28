# Introduction

Welcome to **The Maca Handbook**.

Maca is a single, typed language for **both programs and infrastructure
configuration**. The same source you write to build an app is the source you
write to describe the machine it runs on — no second language, no YAML.

## What makes Maca different

- **One language, many targets.** A general program compiles to a native binary
  (through C), to the BEAM, or to JavaScript; a program written in *config mode*
  compiles to Nix. The compiler decides the backend from what you ask for, not
  from a different dialect.
- **Records and sum types, no ceremony.** Data is either a record
  (`Point = { x: int, y: int }`) or a sum (`Shape = Circle | Rect`), and `match`
  takes them apart. There are no classes and no `null`.
- **Colorblind async.** There is no `async` keyword. Concurrency is an *inferred
  effect*: `spawn f(x)` runs `f` concurrently, `await` suspends until it
  resolves, and an ordinary function becomes async simply by using them.
- **Config is code, checked like code.** Infrastructure written in Maca is
  type-checked before it ever reaches a machine; an unknown option or an impure
  expression in config mode is a compile error, not a 3 a.m. surprise.

## Who this book is for

You have written code before — the exact language does not matter. This handbook
teaches Maca from the ground up: values and types, functions, control flow, its
data model, effects, and the toolchain around it.

## How to read it

Chapters build on each other, so reading front to back is the intended path. Each
one is short and ends with something you can run.

Ready? [Let's write the smallest possible Maca program.](01-hello-world.md)
