# Introduction

Maca is a single, typed language for **both programs and infrastructure
configuration**. The same source you write to build an app is the source you
write to describe the machine it runs on: no second language, no YAML.

## What makes Maca different

**One language, many targets.** Maca is a universal transpiler: it writes
another language rather than machine code. A program is translated to C and
built into a binary by `cc`,
or to JavaScript, the JVM, Rust, or freestanding C for a microcontroller. Which
one you get is a flag, not a dialect.

**No garbage collector and no borrow checker.** Memory is reference counted and
the compiler mostly optimises the counting away. No lifetimes, no `clone()`.

**No `async` keyword.** Concurrency is an inferred effect. `spawn f(x)` runs `f`
concurrently, `await` waits for it, and every caller follows without changing
its signature.

**Configuration is code, checked like code.** Infrastructure written in Maca
compiles to Nix, type-checked first. An option that doesn't exist, or an
expression that tries to *do* something rather than describe it, is a compile
error.

## Who this book is for

You have written code before; the language does not matter.

## It is two books

**Learning Maca** teaches, in the order a learner needs, and every chapter ends
with something you can run.

**The Reference** answers, organised for lookup: effect rows, ownership rules,
import resolution order, back-end differences, the full diagnostic list.

Both are in the sidebar, and the search box searches both.

## How to read Learning Maca

Front to back. The chapters build on each other.

- **Getting Started** installs the compiler and writes two programs.
- **The Language**: values, records, sum types, collections, errors, functions,
  modules, memory, tests.
- **What Maca Does Differently**: colorblind async, config mode, the UI syntax,
  the targets.
- **Build Something** writes a real tool end to end.

If you already know the language, start at [Syntax](a5-syntax.md) instead.

## Everything here has been run

`apps/examples/handbook.maca` is this book's claims as one program the test
suite executes. The keyword list is checked against the lexer, the diagnostics
against the checker, the method tables against a program that calls every name
in them.

Ready? [Let's get set up.](01-installing.md)
