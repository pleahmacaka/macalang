# Testing

Tests live beside the code they test, in the same file, and the toolchain finds
them by name.

## Writing a test

Any function whose name begins with `test_` is a test:

```maca
Counter = {
    n: int
}

bump(c: Counter) -> Counter =>
    c with { n = c.n + 1 }

test_bump() -> int {
    c = bump(Counter { n = 1 })
    c.n == 2 ? 0 : 1
}
```

A test returns `int`: **zero passes, non-zero fails**. That is the same
convention as `main`, and the same convention as every process on the system, so
there is nothing new to learn and nothing to import.

## Running

```
maca test counter.maca
```

```
running 1 test
  test_bump
1 test passed
```

The driver collects every `test_`-prefixed function in the file, drops the file's
own `main` if it has one, and generates a runner that announces each test before
calling it — so a test that crashes tells you which one it was.

## Why not an assert library

There isn't one, and the omission is deliberate for now. `c.n == 2 ? 0 : 1` is
not elegant, but it needs no macro system, no framework, and no special
integration with the compiler. A richer story — named assertions, a diff on
failure — belongs in Maca-level code once there is enough of a standard library
to build it in, not in the compiler.

In the meantime, a helper of your own goes a long way:

```maca
check(name: str, ok: bool) -> int {
    ok ? 0 : 1
}
```

## Testing across files

Tests are found in the file you point at. A test file can import the module it
exercises:

```maca
import geometry

test_origin_is_zero() -> int {
    p = origin()
    p.x == 0 && p.y == 0 ? 0 : 1
}
```

## The larger point: run your documentation

The Maca repository holds a file called `examples/handbook.maca`. It contains
every runnable claim this book makes — the record update from chapter 5, the
format specs from chapter 3, the list patterns from chapter 6 — in one program,
and the test suite runs it and checks each line of its output.

That file exists because writing this handbook broke things. Five compiler bugs
and one nonexistent command were found by taking prose that had been written
confidently and actually executing it:

- a function with no declared return type discarded its body
- an undeclared return type left callers unable to convert the result
- list methods rejected a named function, accepting only lambdas
- there was no pattern for an empty list
- `maca test` was documented but did not exist
- a literal `{` in a string silently swallowed the rest of the file

Every one of those was in text that read perfectly well. Documentation that
isn't run is a claim, not a fact — and the cheapest way to make it a fact is to
put it in a file the test suite executes.
