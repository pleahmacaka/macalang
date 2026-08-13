# Back ends

Six, over one core IR. This page is what each one emits and what it refuses.
What a *user* picks between is the handbook's targets chapter; this is the
implementor's view.

| target | crate | emits | for |
|---|---|---|---|
| native (default) | `maca-backend-c` | C, linked by `zig cc` or the host `cc` | everything |
| `--target js` | `maca-backend-js` | JavaScript, a reactive DOM, a stylesheet | pages |
| `--target nix` | `maca-backend-nix` | a `.nix` module | config mode |
| `--target jvm` | `maca-backend-jvm` | Java source | Maven, Minecraft |
| `--target rust` | `maca-backend-rust` | Rust source | crates.io |
| `--target embedded` | `modules/maca/emit_embedded.maca` | freestanding C + a linker script, linked by clang/lld | Cortex-M, RISC-V |

SIMD is not a second native path: `f32x8` is an `ext_vector_type` the C backend
lowers, and `-mavx2` is what makes the eight lanes one `%ymm` register.

## What a target can carry

A target is the effect set a program compiled for it may have, and
`maca_core::TARGETS` is that table:

| target | carries |
|---|---|
| native, jvm, rust, tauri | `io`, `net`, `os`, `async`, `exn` |
| js | `io`, `net`, `async`, `exn`: a browser has no OS |
| embedded | `exn`: no allocator, no OS, no scheduler |
| nix | nothing: a config is data |

`maca check --target <t>` refuses a function whose inferred effects the target
cannot carry, as `M0007`, naming the effect and the target. With no `--target`
a program is held to `native`, which is what `maca build` produces; `--target
all` holds it to what every program target shares, which is the question a
library author asks.

**This is config mode generalised, not a second mechanism.** The
`check_config_effects` walk already refused every effect for one hard-coded
target, because a
`.nix` module is data. `EffSet` is already inferred and already transitive, so a
function that calls a function that touches the filesystem carries `io` without
anyone writing it down, and a per-symbol availability manifest would only be
recomputing that by hand and going stale in the direction that hurts.

Availability that is not an effect stays out of it. `int / int` truncates
natively and does not on `js`; a function-typed record field is refused on
`rust` and `jvm`. Those are type-level and behavioural, and inventing an effect
for "division rounds differently" would be a lie about what an effect is. They
are the back end's own diagnostics, and `maca spec --llm` prints them beside
the table.

## What every one of them owes the others

**The checker runs first, on every target.** Each path calls it from its own
site, which is exactly why one that stops asking is wrong on that target alone
and silent everywhere else. That has happened twice.
`modules/maca/tests/refusals.maca` builds the same rejected program on all
five program targets and names the one that accepted it.

**Imports resolve on every target.** `modules/maca/tests/imports.maca`
holds each of them to inlining a sibling module, a transitive chain, a diamond
inlined once, and a selective import that brings only what it names.

**A construct a back end cannot lower is a diagnostic, not a default.** The
`null`, the `0` and the `Default::default()` that used to stand in for an
unhandled expression were each a wrong answer with nothing to say so. A
function-typed record field on the `rust` and `jvm` targets is the current
example: refused by name rather than emitted as something that will not
compile.

## C, the default

Perceus reference counting, inserted at compile time. Two invariants hold the
string and list handling together and are easy to break from inside the
runtime:

* Every `maca_str`-returning runtime function returns a fresh block or a static
  literal, never one of its arguments. `maca_str_copy` exists for the cases
  that would otherwise hand an argument back.
* `xs = xs.push(v)` appends in place; `ys = xs.push(v)` copies. The copy is the
  rule, and assigning back to the same name is the one case where the old value
  is unreachable the moment the new one exists. Written as a copy it is
  quadratic.

`ownership::appendable_names` decides the second per function, and excludes
parameters, `for` pattern variables and anything aliased. A test that asserts
only answers cannot see any of this: an answer is identical either way, and
reading `xs.length()` marks the list aliased, which switches the optimisation
off inside its own test. Assert on `alloc_count()`/`reuse_count()` instead, and
run with `MACA_POISON=1` so a use-after-free is a wrong answer rather than a
lucky one.

Control-flow expressions work in value position through a `Sink`
(Discard, Return, Assign) threaded through the statement emitters.

## JavaScript

The same `Sink`, for a sharper reason: `if`, `match` and a block are
expressions in Maca and statements in JS, so lowering them as values means an
IIFE, and an IIFE is a function boundary. `break` cannot cross it, and a `var`
written inside one declares a fresh local rather than assigning the enclosing
name. So they lower as real statements wherever there is statement context,
including a binding. The IIFE remains only where the expression really is
nested inside another, and `node --check` over every app that builds to JS is
what catches the rest.

Known and left: `int / int` truncates natively and does not in JS.

## Rust

The point is that a crates.io library costs a line of configuration rather than
a port. A Rust library's public API has no C ABI, so `import c` cannot reach
it, and a hand-written shim does not scale past a few dozen flat functions.

* `import rust "gpui::div"` becomes `use gpui::div;`, and an import naming an
  undeclared crate is an error rather than a silent drop.
* Maca has no `::`, so a call on a foreign capitalized type is its constructor
  and `Type.assoc(a)` is an associated function.
* `Counter : Render = { render = (self, …) => … }` becomes
  `impl Render for Counter`. A parameter whose type this module does not
  declare is foreign, and Maca never owns a value from a crate it does not
  read, so it lowers to `&mut T`. It may be read and passed on, but not
  returned or stored: that would need a lifetime Maca cannot spell, so it is a
  clean error naming the parameter rather than one from `rustc` about a type
  nobody wrote.
* A local passed by value to a user call is cloned, so Maca's value semantics
  survive the move checker. That is a real cost in a hot loop, and the native
  target is the answer.

No borrow checker, no lifetimes, no trait definitions, no generic bounds, no
proc macros, no `unsafe`, and no Rust parser. The back end emits Rust and lets
`rustc` be the authority on Rust. A second Rust would be a second thing to keep
correct.

## Nix

Config mode, where the effect row is the point: a config that reaches for an
effect is `EffectInConfig`, so a module cannot quietly do IO while pretending
to be data. `dev.maca` and a NixOS `system.maca` go through the same emitter.

## JVM and embedded

JVM emits Java source for Maven and Fabric interop, with the same foreign-type
rules the Rust back end uses. Embedded emits freestanding C for Cortex-M and
RISC-V: no libc, no allocator by default, `--mcu` picks the core.

## Where each is gated

Every back end has a hermetic emit suite in its own crate, and a
compile-and-run suite in the driver where a toolchain exists on the runner.
The cross-target guarantees above are in `modules/maca/tests/refusals.maca`.
