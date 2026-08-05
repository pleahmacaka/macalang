# Introduction

Welcome to **The Maca Handbook**.

Maca is a single, typed language for **both programs and infrastructure
configuration**. The same source you write to build an app is the source you
write to describe the machine it runs on: no second language, no YAML.

## What makes Maca different

Most of what follows is ordinary: functions, records, pattern matching, static
types you rarely have to write down. Four things are not.

**One language, many targets.** A program compiles to a native binary through C,
or to JavaScript, the JVM, Rust, or freestanding C for a microcontroller. Which
one you get is a flag, not a dialect. There is no per-target subset of the
language and no separate standard library.

**No garbage collector and no borrow checker.** Memory is managed by reference
counting that the compiler mostly optimises away, so you write the functional
version (build a new value from an old one) and get the performance of the
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

You have written code before. The exact language does not matter. This handbook
teaches Maca from the ground up: values, functions, its data model, the type
system, effects, and the toolchain around it.

## It is two books

They serve two different readers, and trying to be one book served both badly.

**Learning Maca** is what you are in. It teaches: it moves in the order a
learner needs rather than the order the compiler is built in, every chapter ends
with something you can run, and it is meant to be finished in a sitting or two.
It is deliberately incomplete. Where it stops short it says so and names the
page that doesn't.

**The Reference** answers. It assumes you know the language and is organised for
lookup: the exact rule, the exact syntax, the exact diagnostic, the corner
cases. The effect rows, the ownership rules, the import resolution order, the
back-end differences and the full diagnostic list live there. Its prose is dense
on purpose.

Both are in the sidebar, and the search box searches both. A hit tells you
which book it came from, because "Collections" in the handbook and "Collections"
in the reference are different answers to different questions.

## How to read Learning Maca

Front to back. The chapters build on each other.

- **Getting Started** installs the compiler and writes two programs.
- **The Language** is the everyday language: values, records, sum types,
  collections, errors, functions, modules, memory, tests.
- **What Maca Does Differently** is a tour of the four things that are not like
  other languages: colorblind async, config mode, the UI syntax, the targets.
  Each is a short chapter with a door into the reference at the end.
- **Build Something** writes a real tool end to end.

If you already know the language, start at
[Syntax](a5-syntax.md) instead.

## Everything here has been run

The repository holds `apps/examples/handbook.maca`: this book's claims, as one
program the test suite executes. The reference is checked against the compiler
itself: the keyword list against the lexer, the diagnostics
against the checker, the method tables against a program that calls every name
in them.

That is not ceremony. Writing this handbook found six real bugs in the compiler,
because prose that reads well is not the same as prose that is true.

Ready? [Let's get set up.](01-installing.md)
