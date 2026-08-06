# The Self-Hosted Compiler

Maca is being bootstrapped, and this is where new compiler work goes.

## Two stages

**Stage 0** is the Rust compiler under `crates/`. It is the bootstrap, and it is
frozen: it changes only for genuine bootstrap bugs, such as a parser that hangs
or a construct that mis-parses.

**Stage 1** is the Maca compiler under `modules/maca/`, written in Maca. This
is where new compiler work belongs. Stage 0 compiles it; a test gate
(`crates/driver/tests/selfhost.rs`) makes sure it keeps working.

## What stage 1 does today

The whole front end runs, natively:

```
lexer.maca → parser.maca → ast.maca → check.maca → emit_c.maca
                                                 → emit_rust.maca
```

A hand-written lexer, a recursive-descent parser producing a recursive `Expr`
AST with a pretty-printer, a type checker, and **two** back ends: one emitting
C, one emitting Rust.

The slice of Maca it handles: the full primitive expression language (integer,
float, string and boolean literals; arithmetic, comparison, `&&`/`||`, unary
`-`/`!`; the ternary; multi-argument and nested calls with correct precedence),
string interpolation, type-threaded signatures where a declared type flows
through to `const char*`/`double`/`bool` in C and `String`/`f64` in Rust,
records with field access, and sum types, both nullary and payload-carrying,
with `match` over binding patterns.

## How the gate works

The test does not check that the stage-1 compiler produces the *expected*
output. It checks that the output **works**:

1. Stage 0 compiles the stage-1 compiler to a native binary with the host `cc`.
2. That binary is run over a Maca program, twice: once emitting C, once
   emitting Rust.
3. The emitted C is compiled with `cc` and run. The emitted Rust is compiled
   with `rustc` and run.
4. Both must print `self-hosted!` and exit `42`.

## Two features that unlocked it

**Recursive record types.** An AST node holds a list of AST nodes:

```maca
Expr = {
    kind: int
    text: str
    children: Expr[]
}
```

In C that is a definition cycle: the array type needs the struct's size, the
struct needs the array. The backend breaks it by forward-declaring the array
struct before the record body and emitting its operations after.
[Memory and Ownership](a8-memory.md) has the exact shape.

## Reading the source

`apps/maca1/main.maca` is the entry point, and it doubles as the test suite:
it lexes, parses, checks and emits a series of programs and reports on each.

```
maca run apps/maca1/main.maca
```

## Where the two stages differ

Stage 1 compiles a subset. Closures, generics, the effect rows and the module
system live in stage 0 alone, so a program using them goes through the Rust
compiler.

