# Introduction

Welcome to **The Maca Handbook**.

Maca is a single, typed language for **both programs and infrastructure
configuration**. The same source you write to build an app is the source you
write to describe the machine it runs on — no second language, no YAML.

## What makes Maca different

Most of what follows is ordinary: functions, records, pattern matching, static
types you rarely have to write down. Four things are not.

**One language, many targets.** A program compiles to a native binary through C,
or to JavaScript, the JVM, Rust, or freestanding C for a microcontroller. Which
one you get is a flag, not a dialect — there is no per-target subset of the
language and no separate standard library.

**No garbage collector and no borrow checker.** Memory is managed by reference
counting that the compiler mostly optimises away, so you write the functional
version — build a new value from an old one — and get the performance of the
imperative one. There is nothing to annotate, no lifetimes, no `clone()`.

**No `async` keyword.** Concurrency is an inferred effect rather than a colour
that splits your standard library in two. `spawn f(x)` runs `f` concurrently and
`await` waits for it; an ordinary function becomes asynchronous by using them,
and every function that calls it follows without changing its signature.

**Configuration is code, checked like code.** Infrastructure written in Maca
compiles to Nix, and it is type-checked first. An option that doesn't exist, or
an expression that tries to *do* something rather than describe it, is a compile
error rather than a discovery at three in the morning.

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

## Everything here has been run

The repository holds `examples/handbook.maca`: this book's claims, as one
program the test suite executes. The reference appendices are checked against
the compiler itself — the keyword list against the lexer, the diagnostics
against the checker, the method tables against a program that calls every name
in them.

That is not ceremony. Writing this handbook found six real bugs in the compiler,
because prose that reads well is not the same as prose that is true.

Ready? [Let's get set up.](01-installing.md)
