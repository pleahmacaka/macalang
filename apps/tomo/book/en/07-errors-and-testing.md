# Errors and Testing

## Failure is a value, not an exception

A function that can fail says so by failing — `fail msg` unwinds to the nearest
handler:

```
divide(a: int, b: int) -> int =>
    b == 0 ? fail "division by zero" : a / b
```

At a call site you choose what to do about it:

- `divide(x, y)?` — **propagate**: on failure, return it to *your* caller.
- `try divide(x, y)` — **reify**: catch the failure and get a value you can
  inspect instead of unwinding.

```
result = try divide(10, 0)     // handled here, not propagated
```

> There is no `null` and no exception hierarchy. A failure carries a message and
> travels one of exactly two paths: onward with `?`, or into your hands with
> `try`.

## Tests live next to the code

`maca test` compiles and runs a program's tests. There is no separate framework
to learn — a test is a function whose name starts with `test_`:

```
add(a: int, b: int) -> int => a + b

test_add_is_commutative() -> int {
    add(2, 3) == add(3, 2) ? 0 : fail "addition should commute"
}
```

Returning `0` passes; failing fails, and the message is what you read.

## The golden-example habit

Maca's own compiler is developed the same way you are encouraged to develop with
it: every language feature gets

- a small program in `examples/` that uses it, and
- a test that compiles that program, runs it, and checks the output.

The point is that a feature is only "done" when a real program using it *runs*.
A plausible-looking compiler output that never executes has caught nobody's bug.

## Diagnostics you can act on

The compiler reports a small, deliberate set of errors rather than a wall of
inference noise:

- `TypeMismatch` — the types don't line up (including call arity and disagreeing
  `if` branches),
- `NonExhaustive` — a `match` is missing a variant,
- `UndefinedName` — a call to something defined nowhere,
- `Immutable` — reassigning a constant,
- `EffectInConfig` / `UnknownOption` — the config-mode guardrails.

Each names the thing that is wrong and where. If you get one you cannot act on,
that is a bug in the compiler, not in you.

Next: how Maca is bootstrapping itself.
