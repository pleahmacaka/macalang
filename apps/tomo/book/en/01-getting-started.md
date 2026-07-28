# Getting Started

Every Maca program starts at `main`, which returns an `int` — the process exit
code. Here is the whole thing:

```
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

- `main() -> int` declares a function named `main` that takes no arguments and
  returns an `int`. There is no `fn` keyword — a name followed by `(...)` and a
  `-> Type` is a function.
- `{ ... }` is the body, a **block**. The last expression in a block is its
  value; here the block's value is `0`, so that is what `main` returns.
- `info("Hello, World")` calls the `info` builtin, which prints a line.
- `0` is the exit code. Return non-zero to signal failure.

## Arrow bodies

A function whose body is a single expression can skip the braces and use `=>`:

```
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
