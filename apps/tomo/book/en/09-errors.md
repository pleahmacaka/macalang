# Errors

Maca has no exceptions to catch, no `Result` to unwrap, and no `null` to check
for. A function that can fail says so by failing, and its caller decides in one
of three ways what that means.

## Failing

`fail` takes a message and unwinds:

```maca
divide(a: int, b: int) -> int =>
    b == 0 ? fail "division by zero" : a / b
```

Note the return type: `int`, not a wrapper around an `int`. A failure is not part
of the value, so it does not have to be threaded through every signature between
where it happens and where it is handled. That is the same reason exceptions
exist in other languages, without the hierarchy of exception classes to design.

## Ignoring it

Call the function normally and a failure travels straight out of your program:

```maca
main() -> int {
    info("{divide(10, 0)}")
    0
}
```

prints `error: division by zero` on standard error and exits with status 1. For
a command line tool that is often exactly right: the message reaches the user
and the exit code reaches the shell, with nothing written to arrange it.

## Passing it on

An attached `?` propagates the failure to *your* caller:

```maca
checked(n: int) -> int {
    v = divide(84, n)?
    v + 1
}
```

If `divide` fails, `checked` fails with the same message and `v + 1` never runs.
The `?` is attached, with no space before it, the same attached-versus-spaced
rule that separates a format spec from a ternary.

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

Be clear about the limitation, because it is a real one: `try` gives you the
*message*, not the value. `try divide(10, 2)` is the empty string, not `5`. When
you need both the result and a local way to handle failure, either run the
operation and check separately, or return a sum type and skip `fail` for that
function entirely.

## Which one to use

Most code should use `?`, or nothing at all. Propagating is the default because
the place that can *do* something about a failure is almost never the place that
noticed it, and letting one reach the top of a command line program produces a
decent error message for free.

Reserve `try` for a boundary where you genuinely have a fallback: a retry, a
default, a message to a user who is going to try again.

And where failure is an ordinary outcome rather than an exceptional one (a
parse that may not match, a lookup that may miss), do not use `fail` at all. A
[sum type](06-sum-types.md) says so in the type, and the compiler then makes
every
caller deal with it:

```maca
Parsed = Found(int) | Missing(str)
```

That is the distinction worth internalising. `fail` is for the case you did not
plan for; a sum type is for the case you did.

## Run it

```
maca run apps/examples/catch.maca
```

A failure raised, caught with `try`, and execution continuing past it. Then take
the `try` away and run it again: the same program now prints the message to
standard error and exits `1`, with nothing written to arrange either.

Failure is one row of the effect system, and `try` is the one operation that
*removes* an effect rather than adding one.
[Effects and Async](a7-effects.md) has the rest.
