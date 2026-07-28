# Collections

Maca has two built-in collections: the list and the string. Both come with a
library of methods, and both are used through UFCS, so `xs.map(f)` is ordinary
function application dressed up to read left to right.

## Lists

A list of `T` is written `T[]`:

```maca
xs = [5, 3, 8, 1]
names = ["ada", "grace"]
empty = []
```

Indexing and assignment through an index:

```maca
first = xs[0]
xs[2] = 99
```

`len(xs)` and `xs.length()` both give the size.

### The list methods

| Method | Result | Notes |
|---|---|---|
| `map(f)` | `U[]` | `f` receives each element |
| `filter(f)` | `T[]` | keeps elements where `f` is true |
| `reduce(init, f)` | `U` | `f(acc, x)` left to right |
| `fold(init, f)` | `U` | same |
| `sort()` | `T[]` | ascending |
| `reverse()` | `T[]` | |
| `push(x)` | `T[]` | a longer list |
| `pop()` | `T[]` | a shorter list |
| `slice(from, to)` | `T[]` | `to` is exclusive |
| `contains(x)` | `bool` | |
| `index_of(x)` | `int` | `-1` when absent |
| `sum()` `min()` `max()` | `T` | numeric lists |
| `first()` `last()` | `T` | |
| `get(i)` | `T` | same as `xs[i]` |
| `length()` | `int` | |

```maca
xs = [5, 3, 8, 1]
info("{xs.map(v => v * 2).sum()}")            // 34
info("{xs.filter(v => v > 3).length()}")      // 2
info("{xs.reduce(0, (a, b) => a + b)}")       // 17
info("{xs.sort().first()}")                   // 1
```

The methods that "change" a list return a new one and leave the receiver alone:

```maca
ys = [3, 1, 2]
sorted = ys.sort()
// ys.first() is still 3; sorted.first() is 1
```

That is worth internalising. `xs.push(9)` is not a statement that grows `xs`; it
is an expression whose value is the longer list. Chapter 4 explains why writing
it this way is not the performance mistake it looks like.

`slice` takes a start and an **exclusive** end, so `xs.slice(1, 3)` is two
elements:

```maca
xs = [10, 20, 30, 40, 50]
xs.slice(1, 3)      // [20, 30]
```

### Ranges

`lo..hi` is an inclusive integer range, and it is an `int[]`:

```maca
for i in 1..5 {
    info("{i}")     // 1 2 3 4 5
}
info("{(1..100).sum()}")    // 5050
```

In a `for` header this lowers to a counting loop — no list is built.

### Passing a named function

A method that takes a function accepts a lambda or the name of a top-level
function:

```maca
is_even(n: int) -> bool => n % 2 == 0

evens = [1, 2, 3, 4].filter(is_even)
```

Chapter 11 covers what a function value actually is.

## Strings

`str` is a byte string. The methods:

| Method | Result | Notes |
|---|---|---|
| `length()` | `int` | bytes, not characters |
| `split(sep)` | `str[]` | |
| `trim()` | `str` | both ends |
| `upper()` `lower()` | `str` | |
| `contains(s)` | `bool` | |
| `starts_with(s)` `ends_with(s)` | `bool` | |
| `replace(from, to)` | `str` | every occurrence |
| `substr(start, len)` | `str` | a **length**, not an end |
| `index_of(s)` | `int` | `-1` when absent |
| `repeat(n)` | `str` | |
| `pad_start(w, p)` `pad_end(w, p)` | `str` | `p` defaults to a space |
| `pad_center(w, p)` | `str` | |
| `chars()` | `str[]` | one-character strings |
| `at(i)` | `str` | the character at `i` |
| `is_whitespace()` `is_ascii_digit()` `is_alpha()` | `bool` | character classes |

Note the asymmetry that will catch you once: **`slice` on a list takes an end,
`substr` on a string takes a length.**

```maca
"abcdef".substr(1, 3)     // "bcd"
```

`chars`, `at` and the three character classes are what a scanner is built from —
`selfhost/lexer.maca` uses nothing else:

```maca
run_digits(cs: str[], i: int) -> int =>
    i >= cs.length() || !cs.get(i).is_ascii_digit()
        ? i
        : run_digits(cs, i + 1)
```

### There is no string slice

`str` has `substr`, not `slice`. Calling `slice` on a string does not fail with a
type error — method calls stay gradual, so the checker lets it through and the C
compiler reports an undefined reference to `slice`. This is a known rough edge:
a misspelt method on a primitive should be a diagnostic, and today it is a
linker message.

## Strings and characters

Maca has no character type. `at(i)` gives a one-character `str`, and comparison
works the way you would hope:

```maca
c = "hello".at(1)
info("{c == "e"}")      // true
```

Because `length` counts bytes, a string with non-ASCII text has a length larger
than its character count. Interpolation, concatenation and comparison are all
byte-exact and safe; it is only indexing that needs care with multi-byte text.
