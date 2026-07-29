# Closures and Control Flow

## Functions are values

A top-level function referenced by name is a value — you can pass it where a
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

A lambda infers its return type from its body. It can also declare one, which
is what you need when the lambda has to match a signature the compiler cannot
see — a method of a foreign trait on the Rust target, where the trait lives in
a crate the compiler does not read:

```maca
Counter : Render = {
    render = (self, window, cx) -> AnyElement =>
        div().child("Count: {self.count}").into_any_element()
}
```

The annotation needs the parameter parentheses, so `(n) -> int => n + 1` rather
than `n -> int => n + 1` — otherwise it would read as a function signature.

A lambda body may be a block, the way a `match` arm's may:

```maca
classify = (n) -> str => {
    doubled = n * 2
    doubled > 10 ? "big" : "small"
}
```

Which means a bare `{ … }` after `=>` is that block, not an anonymous record.
Parenthesize when you meant the record — `(n) => ({ x = n })`.

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

The brackets are optional — `x, ..rest` matches the same way, in keeping with
Maca's bracketless comma lists. Use whichever reads better; brackets make the
empty and single-element cases clearer.

## Loops

`while` and `for` are statements, and `for` walks an inclusive integer range with
`..`:

```maca
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

```maca
config = read_file("app.toml")?      // propagate a failure to the caller
```

## Run it

```
maca run examples/lambda.maca
```

Lambdas passed to list methods, a named function used as a value, and a closure
that captures a local. All three compile to the same thing — a code pointer and
a heap environment — which is why they are interchangeable at a call site.

Next: putting functions in more than one file.
