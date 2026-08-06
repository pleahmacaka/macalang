# Effects and Async

A function's type is its arguments, its result, **and what it does**: an effect
row the checker infers from the body and propagates through every caller. No
`async` keyword, no `IO` in a signature, no `throws` clause. The teaching
version is [Colorblind Async](13-colorblind-async.md).

## The rows

Five effects, from the Koka lineage:

| Effect | Introduced by |
|---|---|
| `io` | `print`, `input`, and the console family (`info`, `warn`, `err`, `debug`, `notice`, `crit`, `alert`, `emerg`, `panic`), plus the file and stream methods `read`, `write`, `exists`, `remove`, `append`, `create` |
| `net` | a call through a `net`, `http` or `socket` receiver |
| `os` | a call through an `os` or `process` receiver |
| `async` | `await`, `spawn`, `sleep_ms` |
| `exn` | `fail`, and any call that can raise |

A function's row is the union of everything its body reaches, so adding
`sleep_ms` to a leaf makes it `async` and every caller follows without a
signature changing.

**`try` discharges `exn`**, the one operation that *removes* an effect rather
than adding one.

## Async is an inferred effect

Three operations introduce `async`:

- `spawn f(x)` runs `f(x)` concurrently and gives you a `Future a`;
- `await fut` suspends until the future resolves, and evaluates to its value;
- `sleep_ms(ms)` is a suspension point.

A function that uses any of them *is* async:

```maca
fetch_both(a: str, b: str) -> str {
    fa = spawn get(a)          // both requests run concurrently
    fb = spawn get(b)
    await fa ++ await fb        // join
}
```

`await` and `spawn` are **prefix operators at unary precedence**, tighter than
every binary operator and looser than a call, so `await a + await b` is
`(await a) + (await b)` and `spawn f(x)` spawns the call.

## Why this matters

## It compiles to real concurrency

An async function is an ordinary function with **no ABI change**, so a
concurrent function and a synchronous one are interchangeable at the call site
and across the FFI boundary.

| Target | What a suspension point becomes |
|---|---|
| native (C) | `maca_spawn` / `maca_await` / `maca_sleep_ms`: pthread-backed futures in the runtime, so a suspension point is a real thread boundary |
| JS | `await`, and an `async function` for every function that reaches one: the compiler works out which those are, so a handler that waits for the reader is written the same as one that does not |
| playground | evaluated eagerly by the interpreter, so a program's output is the same and its timing is not |
| Nix (config) | rejected; see below |

## Effects are checked

In [config mode](a12-config.md) a program describes state rather than acting, so
**any** non-empty effect row is a compile error:

```
EffectInConfig: config must be pure but this uses effect(s): async
```

The message names the rows it found, so a configuration that both prints and
sleeps says `io, async`. Nothing is special-cased, which is why a configuration
cannot read a file to decide what it declares.

## What the freestanding target refuses

The embedded target refuses the `io` builtins by name rather than by row,
because a bare-metal image has no console. The list is exactly the console
family above. Drive a UART with `mmio_write` instead. See
[Targets](a10-targets.md).
