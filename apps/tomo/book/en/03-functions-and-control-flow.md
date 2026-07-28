# Functions and Control Flow

## Functions are values

A top-level function referenced by name is a value — you can pass it where a
function is expected:

```
is_even(n: int) -> bool => n % 2 == 0
evens = [1, 2, 3, 4].filter(is_even)   // [2, 4]
```

A lambda captures its surrounding scope:

```
step = 10
bumped = [1, 2, 3].map(n => n + step)  // [11, 12, 13]
```

An unannotated parameter that is *called* in the body is inferred to be a
function, so higher-order code needs no special syntax:

```
run_twice(f, x) => f(f(x))
run_twice(n => n + 1, 40)              // 42
```

## `if` as an expression

Control flow produces values. `if`/`else` is an expression:

```
label = if score >= 60 { "pass" } else { "fail" }
```

For the common two-way choice there is the spaced ternary:

```
label = score >= 60 ? "pass" : "fail"
```

## `match`

`match` is the workhorse. It works on sum types, literals, and lists, and it is
exhaustive:

```
describe(xs: int[]) -> str =>
    match xs {
        []          => "empty"
        [x]         => "one: {x}"
        [x, ..rest] => "head {x}, then {rest.length()} more"
    }
```

## Loops

`while` and `for` are statements, and `for` walks an inclusive integer range with
`..`:

```
sum_to(n: int) -> int {
    total = 0
    for i in 1..n {          // 1, 2, …, n
        total = total + i
    }
    total
}
```

`break` and `continue` work as you expect. But note the Maca idiom leans on
recursion and the list methods (`map`/`filter`/`reduce`) over explicit loops —
they are usually shorter and say what they mean.

## Error propagation

A fallible call is marked at the call site with a trailing `?`, which returns
early on failure and unwraps on success:

```
config = read_file("app.toml")?      // propagate a failure to the caller
```

Next: pattern matching in depth.
