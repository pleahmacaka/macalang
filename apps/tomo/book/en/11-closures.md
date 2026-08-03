# Closures and Control Flow

## Functions are values

A top-level function referenced by name is a value, so you can pass it where a
function is expected:

```maca
is_even(n: int) -> bool => n % 2 == 0
evens = [1, 2, 3, 4].filter(is_even)   // [2, 4]
```

A lambda captures its surrounding scope:

```maca
step = 10
bumped = [1, 2, 3].map(n => n + step)  // [11, 12, 13]
```

An unannotated parameter that is *called* in the body is inferred to be a
function, so higher-order code needs no special syntax:

```maca
run_twice(f, x) => f(f(x))
run_twice(n => n + 1, 40)              // 42
```

A lambda infers its return type from its body, and can declare one instead:

```maca
inc = (n) -> int => n + 1
```

Parenthesise the parameters when you annotate. One parameter works either way;
two do not, because `a, b -> int => …` has no way to say where the list ends.
You need the annotation when the lambda must match a type the compiler cannot
see for itself. That happens in one place, and it is in the reference:
implementing a Rust trait, in [Targets](a10-targets.md).

A lambda body may be a block, the way a `match` arm's may:

```maca
classify = (n) -> str => {
    doubled = n * 2
    doubled > 10 ? "big" : "small"
}
```

Which means a bare `{ … }` after `=>` is that block, not an anonymous record.
Parenthesize when you meant the record: `(n) => ({ x = n })`.

## `if` as an expression

Control flow produces values. `if`/`else` is an expression:

```maca
label = if score >= 60 { "pass" } else { "fail" }
```

For the common two-way choice there is the spaced ternary:

```maca
label = score >= 60 ? "pass" : "fail"
```

## `match`

`match` is the workhorse. It works on sum types, literals, and lists, and it is
exhaustive:

```maca
describe(xs: int[]) -> str =>
    match xs {
        []          => "empty"
        [x]         => "one: {x}"
        [x, ..rest] => "head {x}, then {rest.length()} more"
    }
```

The brackets are optional: `x, ..rest` matches the same way, in keeping with
Maca's bracketless comma lists. Use whichever reads better; brackets make the
empty and single-element cases clearer.

## Loops

`while` and `for` are statements. `for` walks a list, or an inclusive integer
range written with `..`:

```maca
sum_to(n: int) -> int {
    total = 0
    for i in 1..n {          // 1, 2, …, n
        total = total + i
    }
    total
}
```

`while` takes a condition, and you move the counter yourself:

```maca
countdown(start: int) -> int {
    n = start
    while n > 0 {
        info("{n}")
        n = n - 1
    }
    0
}
```

`break` and `continue` work as you expect. The Maca idiom leans on recursion and
the list methods (`map`/`filter`/`reduce`) over explicit loops. They are
usually shorter and say what they mean.

## Error propagation

A fallible call is marked at the call site with a trailing `?`, which returns
early on failure and unwraps on success:

```maca
config = read_file("app.toml")?      // propagate a failure to the caller
```

## Run it

```
maca run examples/lambda.maca
```

Lambdas passed to list methods, a named function used as a value, and a closure
that captures a local. All three compile to the same thing (a code pointer and
a heap environment), which is why they are interchangeable at a call site.

Next: putting functions in more than one file.
