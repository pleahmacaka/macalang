# The Self-Hosted Compiler

A language that cannot compile itself is asking you to trust it further than its
authors do. Maca is being bootstrapped, and this chapter explains the shape of
that, partly because it is interesting, and mostly because it is where new
compiler work goes.

## Two stages

**Stage 0** is the Rust compiler under `crates/`. It is the bootstrap, and it is
frozen: it changes only for genuine bootstrap bugs, such as a parser that hangs
or a construct that mis-parses. It is not where features are added.

**Stage 1** is the Maca compiler under `apps/selfhost/`, written in Maca. This is
where new compiler work belongs. Stage 0 compiles it; a test gate
(`crates/driver/tests/selfhost.rs`) makes sure it keeps working.

The rule is short: prefer adding to `apps/selfhost/*.maca` over growing the Rust
crates.

## What stage 1 does today

The whole front end runs, natively:

```
lexer.maca → parser.maca → ast.maca → check.maca → emit_c.maca
                                                 → emit_rust.maca
```

A hand-written lexer, a recursive-descent parser producing a recursive `Expr`
AST with a pretty-printer, a type checker, and **two** back ends: one emitting
C, one emitting Rust.

The slice of Maca it handles is well past a toy: the full primitive expression
language (integer, float, string and boolean literals; arithmetic, comparison,
`&&`/`||`, unary `-`/`!`; the ternary; multi-argument and nested calls with
correct precedence), string interpolation, type-threaded signatures where a
declared type flows through to `const char*`/`double`/`bool` in C and
`String`/`f64` in Rust, records with field access, and sum types, both nullary
and payload-carrying, with `match` over binding patterns.

The checker carries an environment rather than counting mismatches: the
module's function signatures, its record fields, its sum variants, and the
locals in scope. That is what lets it answer a call's result type, a call's
arity, a field the record does not have, and a body that disagrees with its
declared return. `any` is the gradual escape hatch, and it is deliberate: a
name the module does not declare is foreign, not wrong.

## How the gate works

This is the part worth copying if you ever bootstrap anything.

The test does not check that the stage-1 compiler produces the *expected* output.
It checks that the output **works**:

1. Stage 0 compiles the stage-1 compiler to a native binary with the host `cc`.
2. That binary is run over a Maca program, twice: once emitting C, once
   emitting Rust.
3. The emitted C is compiled with `cc` and run. The emitted Rust is compiled with
   `rustc` and run.
4. Both must print `self-hosted!` and exit `42`.

A golden-output test would break every time the code generator's spacing changed.
This one breaks only when the compiler is actually wrong, and it verifies both
back ends against the same source, which catches a whole class of "the C path
works and the Rust path silently doesn't" bugs.

## Two features that unlocked it

Getting a compiler to run needed two things from the language, and both turned
out to be generally useful rather than compiler-specific.

**String scanning.** `chars`, `length`, `at`, `get`, `slice`, and the character
classes `is_whitespace`, `is_ascii_digit`, `is_alpha`. That is the entire
vocabulary `apps/selfhost/lexer.maca` uses. Any tokeniser needs exactly this set.

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
[Memory and Ownership](a8-memory.md) has the exact shape; it exists because the
self-hosted AST needed it.

## Reading the source

`apps/selfhost/main.maca` is the entry point, and it doubles as the test suite: it
lexes, parses, checks and emits a series of programs and reports on each. Running
it is the fastest way to see the current state:

```
maca run apps/selfhost/main.maca
```

The files are small and deliberately plain. `lexer.maca` is a character scanner.
`parser.maca` is recursive descent with a precedence table. `emit_c.maca` is
string templates. There is no cleverness to decode, which is the idea. A
bootstrap compiler that is hard to read is a bootstrap compiler nobody will
finish.

## Where the two stages differ

Stage 1 compiles a subset. Closures, generics, the effect rows and the module
system live in stage 0 alone, so a program using them goes through the Rust
compiler.

That boundary is the point of the two-stage design rather than a defect in it.
Stage 0 is frozen at whatever it takes to compile stage 1; stage 1 grows one
gated increment at a time. Each increment is written in Maca, gated with a
compile-and-run capstone, and left to disagree loudly with stage 0 until it
doesn't, which is why the gate compiles the *emitted* program through both
`cc` and `rustc` rather than comparing generated text.

Porting stage 0 line by line would be faster to start and impossible to
verify.
