# Introduction

Maca is a single, typed language for **both programs and infrastructure
configuration**. The same source you write to build an app is the source you
write to describe the machine it runs on: no second language, no YAML.

## What makes Maca different

**One language, many targets.** A program compiles to a native binary through C,
or to JavaScript, the JVM, Rust, or freestanding C for a microcontroller. Which
one you get is a flag, not a dialect.

**No garbage collector and no borrow checker.** Memory is reference counted and
the compiler mostly optimises the counting away. Nothing to annotate, no
lifetimes, no `clone()`.

**No `async` keyword.** Concurrency is an inferred effect. `spawn f(x)` runs `f`
concurrently and `await` waits for it; an ordinary function becomes asynchronous
by using them, and every caller follows without changing its signature.

**Configuration is code, checked like code.** Infrastructure written in Maca
compiles to Nix, type-checked first. An option that doesn't exist, or an
expression that tries to *do* something rather than describe it, is a compile
error.

## Who this book is for

You have written code before; the language does not matter. This handbook
teaches Maca from the ground up: values, functions, its data model, the type
system, effects, and the toolchain.

## It is two books

**Learning Maca** teaches. It moves in the order a learner needs, every chapter
ends with something you can run, and where it stops short it says so and names
the page that doesn't.

**The Reference** answers. Organised for lookup: the exact rule, the exact
syntax, the exact diagnostic, the corner cases. Effect rows, ownership rules,
import resolution order, back-end differences, the full diagnostic list.

Both are in the sidebar, and the search box searches both. A hit tells you which
book it came from.

## How to read Learning Maca

Front to back. The chapters build on each other.

- **Getting Started** installs the compiler and writes two programs.
- **The Language** is the everyday language: values, records, sum types,
  collections, errors, functions, modules, memory, tests.
- **What Maca Does Differently** covers colorblind async, config mode, the UI
  syntax, and the targets. Each ends with a door into the reference.
- **Build Something** writes a real tool end to end.

If you already know the language, start at [Syntax](a5-syntax.md) instead.

## Everything here has been run

`apps/examples/handbook.maca` is this book's claims as one program the test
suite executes. The reference is checked against the compiler itself: the
keyword list against the lexer, the diagnostics against the checker, the method
tables against a program that calls every name in them.

Ready? [Let's get set up.](01-installing.md)
