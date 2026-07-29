# Effects and Async

A function's type is its arguments, its result, **and what it does**. The third
part is the effect row: a set the checker infers from the body and propagates
through every caller. Nothing about it is written down in source — there is no
`async` keyword, no `IO` in a signature, no `throws` clause.

The teaching version is [Colorblind Async](13-colorblind-async.md). This chapter
is the rows themselves and where each one is enforced.

## The rows

Five effects, from the Koka lineage:

| Effect | Introduced by |
|---|---|
| `io` | `print`, `input`, and the console family — `info`, `warn`, `err`, `debug`, `notice`, `crit`, `alert`, `emerg`, `panic` — plus the file and stream methods `read`, `write`, `exists`, `remove`, `append`, `create` |
| `net` | a call through a `net`, `http` or `socket` receiver |
| `os` | a call through an `os` or `process` receiver |
| `async` | `await`, `spawn`, `sleep_ms` |
| `exn` | `fail`, and any call that can raise |

An expression's effects are the union of its parts, so a function's row is the
union of everything its body reaches. There is nothing to annotate and nothing
to thread: adding `sleep_ms` to a leaf function makes it `async`, and every
caller becomes `async` without a signature changing.

**`try` discharges `exn`.** It is the one operation that *removes* an effect
rather than adding one, which is what makes it a boundary: inside `try e` a
failure is caught, so `try` is pure with respect to `exn` even when `e` is not.

## Async is an inferred effect

Three operations introduce `async`:

- `spawn f(x)` runs `f(x)` concurrently and gives you a `Future a`;
- `await fut` suspends until the future resolves, and evaluates to its value;
- `sleep_ms(ms)` is a suspension point.

A function that uses any of them *is* async — no annotation, no colour:

```maca
fetch_both(a: str, b: str) -> str {
    fa = spawn get(a)          // both requests run concurrently
    fb = spawn get(b)
    await fa ++ await fb        // join
}
```

`fetch_both` never declares itself async; using `spawn`/`await` is enough. And a
plain function calling `fetch_both` doesn't have to change colour either. It
calls it.

`await` and `spawn` are **prefix operators at unary precedence**, which places
them tighter than every binary operator and looser than a call. So
`await a + await b` is `(await a) + (await b)`, and `spawn f(x)` spawns the call
rather than spawning `f` and then calling the future.

## Why this matters

Because there is no colour to propagate, refactoring is painless: making a leaf
function concurrent doesn't force a rewrite of every caller. The same code runs
whether or not it happens to suspend.

The cost of the design is that you cannot read a signature and know whether a
call may suspend. That is the trade: the effect is in the type the checker
carries, not in the text. Where it matters — configuration — the checker is the
one that enforces it, which is the next section.

## It compiles to real concurrency

An async function is an ordinary function. There is **no ABI change**: the
generated C declares it exactly as it declares any other, so a concurrent
function and a synchronous one are interchangeable at the call site and across
the FFI boundary.

| Target | What a suspension point becomes |
|---|---|
| native (C) | `maca_spawn` / `maca_await` / `maca_sleep_ms` — pthread-backed futures in the runtime, so a suspension point is a real thread boundary |
| JS | the event loop; the same three operations map onto its scheduling |
| playground | evaluated eagerly by the interpreter, so a program's output is the same and its timing is not |
| Nix (config) | rejected — see below |

You write the effect once; each backend realises it.

## Effects are checked

Effects are not only inferred — they are enforced where it matters. In
[config mode](a12-config.md) a program describes state rather than performing
actions, so **any** non-empty effect row is a compile error:

```
EffectInConfig: config must be pure but this uses effect(s): async
```

The message names the rows it found, so a configuration that both prints and
sleeps says `io, async`. Nothing is special-cased: `io` is rejected for the same
reason `async` is, which is why a configuration cannot read a file to decide
what it declares.

The effect that is a convenience in a program is a guardrail in config, and it
is the check that lets one language be both without being dangerous as the
second.

## What the freestanding target refuses

The embedded target refuses the `io` builtins by name rather than by row, and
for a concrete reason: they are impure *because they write to a console*, and a
bare-metal image has none. The list is exactly the console family above. Drive a
UART with `mmio_write` instead. See [Targets](a10-targets.md).
