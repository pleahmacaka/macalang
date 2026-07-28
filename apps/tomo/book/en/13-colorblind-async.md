# Colorblind Async

Most languages split functions into two colors — synchronous and `async` — and
the colors are contagious: to `await` something, you must be `async`, and so must
your caller, all the way up. Maca does not have this split. **There is no `async`
keyword.**

## Async is an inferred effect

Concurrency is an *effect* the compiler infers, not a keyword you write. Three
operations introduce it:

- `spawn f(x)` runs `f(x)` concurrently and gives you a `Future a`;
- `await fut` suspends until the future resolves, and evaluates to its value;
- `sleep_ms(ms)` is a suspension point.

A function that uses any of them *is* async — no annotation, no color:

```
fetch_both(a: str, b: str) -> str {
    fa = spawn get(a)          // both requests run concurrently
    fb = spawn get(b)
    await fa ++ await fb        // join
}
```

`fetch_both` never declares itself async; using `spawn`/`await` is enough. And a
plain function calling `fetch_both` doesn't have to change color either — it just
calls it.

## Why this matters

Because there is no color to propagate, refactoring is painless: making a leaf
function concurrent doesn't force a rewrite of every caller. The same code runs
whether or not it happens to suspend, and `await a + await b` is just
`(await a) + (await b)` — `await` is an ordinary prefix operator.

## It compiles to real concurrency

On the native path, `spawn`/`await`/`sleep_ms` lower to pthread-backed futures in
the runtime; an async function is an ordinary function with no ABI change. In the
browser (the JS backend) the same operations map onto the event loop. You write
the effect once; each backend realizes it.

## Effects are checked

Effects are not only inferred — they are enforced where it matters. In *config
mode* (the Nix target), async is impure, so `await`/`spawn`/`sleep_ms` are a
compile error: infrastructure descriptions must be pure. The effect that is
convenient in a program is a guardrail in config.

Next: one language for configuration.
