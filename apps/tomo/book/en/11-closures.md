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

`while` and `for` are statements. `for` walks a list, or a half-open integer
range written with `..`:

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

`break` and `continue` work as you expect. The Maca idiom leans on recursion and
the list methods (`map`/`filter`/`reduce`) over explicit loops. They are
usually shorter and say what they mean.

## Leaving early with `return`

A function's last expression is its value. That is still true, and most
functions never need anything else. But a guard at the top of a function is a
different job: it wants to *stop*, and without a way to say so the entire rest
of the body has to be nested inside an `else` to get out of its way.

`return` is that way to say so:

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

The two forms are the two shapes of function. `return e` leaves a function that
declares a result, and `e` is checked against that result type. A bare `return`
leaves a function that declares none:

```maca
log_unless(quiet: bool, line: str) {
    if quiet {
        return
    }

    info(line)
}
```

`return` is a **statement**. It goes on a line of its own, or as the tail of an
`if`, `match`, `for` or `while` branch, which is every place a function has
something to leave *from*. Written where a value was wanted instead, the
compiler says so by name rather than guessing:

```maca
label = ok ? return 1 : 2     // rejected: this `return` stands inside an expression
```

A lambda is in that category too: its body *is* its value, so write the value.
Use `return` in a named function, and the value in a lambda.

## A function inside a function

A block can define a function, and that function can read and *write* the scope
around it. That is what lets a view own its state and keep its handlers next to
it, instead of threading everything through parameters:

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

`grab` and `release` write the same `held` the enclosing body reads: a local
that any nested definition assigns is *shared* by all of them. One that none of
them assigns is copied when the definition is made, exactly as a lambda's
captures always were.

A nested definition is a value bound where it is written, and it obeys the same
rule every other binding in a block does: **it is in scope from that line
onward, and not before.** A closure captures when it is made, so a name used
above its definition would have nothing to capture. Two consequences follow, and
each has its own diagnostic:

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

If you want either, lift the function to the top level, where every function is
in scope everywhere.

Because it is a value, a nested definition can be passed, stored in a record
field declared `(T) -> R`, and handed back to the caller. It keeps working after
the call that made it returns: what it captured lives on the heap for as long as
something can still reach it.

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

Native C and the JS backend both lower this. The `rust`, `jvm` and `embedded`
targets refuse it by name, and the reference chapter on
[Targets](a10-targets.md) says why.

## Error propagation

A fallible call is marked at the call site with a trailing `?`, which returns
early on failure and unwraps on success:

```maca
config = read_file("app.toml")?      // propagate a failure to the caller
```

## Run it

```
maca run apps/examples/lambda.maca
```

Lambdas passed to list methods, a named function used as a value, and a closure
that captures a local. All three compile to the same thing (a code pointer and
a heap environment), which is why they are interchangeable at a call site.

Next: putting functions in more than one file.
