# Closures and Control Flow

A function in Maca is a value: it can be passed, returned, and written inline.

## Functions are values

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
function:

```maca
run_twice(f, x) => f(f(x))
run_twice(n => n + 1, 40)              // 42
```

A lambda infers its return type, and can declare one instead:

```maca
inc = (n) -> int => n + 1
```

Parenthesise the parameters when you annotate: `a, b -> int => …` has no way to
say where the list ends. You need the annotation when the lambda must match a
type the compiler cannot see, which is implementing a Rust trait
([Targets](a10-targets.md)).

A lambda body may be a block, the way a `match` arm's may:

```maca
classify = (n) -> str => {
    doubled = n * 2
    doubled > 10 ? "big" : "small"
}
```

So a bare `{ … }` after `=>` is that block, not an anonymous record.
Parenthesize when you meant the record: `(n) => ({ x = n })`.

## `if` as an expression

```maca
label = if score >= 60 { "pass" } else { "fail" }
```

For a two-way choice there is the spaced ternary:

```maca
label = score >= 60 ? "pass" : "fail"
```

## `match`

`match` works on sum types, literals and lists, and it is exhaustive:

```maca
describe(xs: int[]) -> str =>
    match xs {
        []          => "empty"
        [x]         => "one: {x}"
        [x, ..rest] => "head {x}, then {rest.length()} more"
    }
```

## Loops

`while` and `for` are statements. `for` walks a list or a `..` range:

```maca
sum_to(n: int) -> int {
    total = 0
    for i in 0..n {          // 0, 1, …, n - 1
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

## Leaving early with `return`

A guard at the top of a function wants to *stop*:

```maca
save(title: str) -> str {
    trimmed = title.trim()

    if trimmed == "" {
        return "title is required"
    }

    // the rest, not indented under an `else`
    store(trimmed)
    "saved"
}
```

`return e` leaves a function that declares a result, checked against that type.
A bare `return` leaves a function that declares none:

```maca
log_unless(quiet: bool, line: str) {
    if quiet {
        return
    }

    info(line)
}
```

`return` is a **statement**: a line of its own, or the tail of an `if`, `match`,
`for` or `while` branch. Written where a value was wanted:

```maca
label = ok ? return 1 : 2     // rejected: this `return` stands inside an expression
```

A lambda's body *is* its value, so write the value there.

## A function inside a function

A block can define a function, and that function can read and *write* the scope
around it:

```maca
board() -> str {
    held = 0
    moves = 0

    grab(section: int) {
        if section < 0 {
            return
        }

        held = section
        moves = moves + 1
    }

    release() {
        held = 0
    }

    grab(4)
    release()

    "held={held} after {moves} move(s)"
}
```

A local that any nested definition assigns is *shared* by all of them; one that
none assigns is copied when the definition is made.

A nested definition is in scope from where it is written, not before, because a
closure captures when it is made. Two consequences, each with its own
diagnostic:

```maca
main() -> int {
    go() -> int {
        return go()          // rejected: a nested function cannot name itself
    }

    first() -> int => second()
    second() -> int => 1     // rejected: `second` is defined further down
    first()
}
```

A nested definition can be passed, stored in a record field declared `(T) -> R`,
and handed back to the caller: what it captured lives on the heap for as long as
something can reach it.

```maca
Knob = { read: (int) -> int, write: (int) -> int }

knob(start: int) -> Knob {
    level = start

    get(ignored: int) -> int => level

    set(to: int) -> int {
        level = to

        return level
    }

    Knob { read = get, write = set }
}
```

Native C and the JS backend lower this; `rust`, `jvm` and `embedded` refuse it
by name. See [Targets](a10-targets.md).

## Error propagation

A trailing `?` returns early on failure and unwraps on success:

```maca
config = read_file("app.toml")?      // propagate a failure to the caller
```

## Run it

```
maca run apps/examples/lambda.maca
```

Lambdas passed to list methods, a named function used as a value, and a closure
capturing a local. All three compile to a code pointer and a heap environment,
which is why they are interchangeable at a call site.
