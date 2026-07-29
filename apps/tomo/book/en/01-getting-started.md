# Hello, World

Every Maca program starts at `main`, which returns an `int` — the process exit
code. Here is the whole thing:

```maca
main() -> int {
    info("Hello, World")
    0
}
```

Save it as `hello.maca` and run it:

```
maca run hello.maca
```

You will see `Hello, World`, and the process exits with code `0`.

## What each piece means

`main() -> int` declares a function called `main` that takes no arguments and
returns an `int`. There is no `fn` keyword: a name, a parameter list, and a
`-> Type` is all a function definition is.

The braces hold a **block**, and a block's value is its last expression. Here
that is `0`, so that is what `main` returns — and because `main`'s return value
is the process exit status, `0` means success. Return anything else to signal
failure.

`info` prints a line. It is one of a small family named after syslog levels;
`warn` and below go to standard error instead.

## Arrow bodies

A function whose body is a single expression can skip the braces and use `=>`:

```maca
double(n: int) -> int => n * 2
```

That is exactly equivalent to `double(n: int) -> int { n * 2 }`. Use whichever
reads better.

## Building instead of running

`maca run` compiles and runs in one step. To produce a standalone binary:

```
maca build hello.maca -o hello
./hello
```

Next: the values and types those functions pass around.

[On to a first real program.](02-a-first-program.md)
