# Errors

No exceptions to catch, no `Result` to unwrap, no `null` to check for. A
function that can fail says so by failing, and its caller decides in one of
three ways what that means.

## Failing

`fail` takes a message and unwinds:

```maca
divide(a: int, b: int) -> int =>
    b == 0 ? fail "division by zero" : a / b
```

The return type is `int`, not a wrapper. A failure is not part of the value, so
it is not threaded through every signature between where it happens and where it
is handled.

## Ignoring it

```maca
main() -> int {
    info("{divide(10, 0)}")
    0
}
```

prints `error: division by zero` on standard error and exits 1.

## Passing it on

An attached `?` propagates the failure to *your* caller:

```maca
checked(n: int) -> int {
    v = divide(84, n)?
    v + 1
}
```

If `divide` fails, `checked` fails with the same message and `v + 1` never runs.
The `?` is attached, with no space before it.

## Handling it

`try` runs an expression and hands you the failure instead of unwinding:

```maca
msg = try divide(10, 0)
```

`msg` is the failure's message, or the **empty string** when nothing failed, so
handling an error is a length check:

```maca
main() -> int {
    msg = try risky_thing()
    msg.length() > 0 ? report(msg) : 0
}
```

## Which one to use

Most code should use `?`, or nothing at all: the place that can *do* something
about a failure is almost never the place that noticed it.

Where failure is an ordinary outcome (a parse that may not match, a lookup that
may miss), do not use `fail` at all. A [sum type](06-sum-types.md) says so in
the type, and the compiler makes every caller deal with it:

```maca
Parsed = Found(int) | Missing(str)
```

## Run it

```
maca run apps/examples/catch.maca
```

A failure raised, caught with `try`, and execution continuing past it. Take the
`try` away and the same program exits `1`.

Failure is one row of the effect system, and `try` is the one operation that
*removes* an effect. [Effects and Async](a7-effects.md) has the rest.
