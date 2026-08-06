# Colorblind Async

Most languages split functions into two colors, synchronous and `async`, and the
colors are contagious. Maca does not have this split. **There is no `async`
keyword.**

## Async is an inferred effect

Three operations introduce it:

- `spawn f(x)` runs `f(x)` concurrently and gives you a `Future a`;
- `await fut` suspends until the future resolves, and evaluates to its value;
- `sleep_ms(ms)` is a suspension point.

A function that uses any of them *is* async, with no annotation and no color:

```maca
fetch_both(a: str, b: str) -> str {
    fa = spawn get(a)          // both requests run concurrently
    fb = spawn get(b)
    await fa ++ await fb        // join
}
```

`fetch_both` never declares itself async, and a plain function calling it does
not have to change color either.

## Why this matters

Making a leaf function concurrent doesn't force a rewrite of every caller. The
same code runs whether or not it happens to suspend, and `await a + await b`
groups as `(await a) + (await b)`, because `await` is an ordinary prefix
operator.

## It compiles to real concurrency

On the native path, `spawn`/`await`/`sleep_ms` lower to pthread-backed futures
in the runtime; an async function is an ordinary function with no ABI change. In
the browser the same operations map onto the event loop.

## Effects are checked

In *config mode* (the Nix target), async is impure, so `await`/`spawn`/
`sleep_ms` are a compile error: infrastructure descriptions must be pure.

## Run it

```
maca run apps/examples/async.maca
```

Two jobs that each sleep 50ms, spawned together and awaited:

```maca
slow_double(n: int) -> int {
    sleep_ms(50)
    n * 2
}

main() -> int {
    a = spawn slow_double(10)
    b = spawn slow_double(20)
    info("{await a + await b}")
    0
}
```

Wall time is about 50ms, not 100. Take the two `spawn`s out and call
`slow_double` directly: same output, twice the wall time, and not one signature
had to change.

## Where the full answer is

Async is one row of a five-row effect system. [Effects and Async](a7-effects.md)
in the reference has all five, what introduces each, what `try` *removes*, the
precedence of `await` and `spawn`, and what a suspension point becomes on each
target.
