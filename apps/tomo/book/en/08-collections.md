# Collections

Maca has two built-in collections, the list and the string, both used through
UFCS: `xs.map(f)` is ordinary function application read left to right.

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
| `sort_by(key)` | `T[]` | ascending by `key(x)`, an `int`, `float` or `str` |
| `reverse()` | `T[]` | |
| `push(x)` | `T[]` | a longer list |
| `pop()` | `T[]` | a shorter list |
| `set(i, x)` | `T[]` | element `i` replaced |
| `insert(i, x)` | `T[]` | `x` at `i`, the rest shifted along |
| `remove(i)` | `T[]` | element `i` gone, the gap closed |
| `slice(from, to)` | `T[]` | `to` is exclusive |
| `contains(x)` | `bool` | |
| `index_of(x)` | `int` | `-1` when absent |
| `index_of_by(f)` | `int` | the first `x` where `f(x)`, else `-1` |
| `enumerate()` | `{index, value}[]` | each element with its position |
| `sum()` `min()` `max()` | `T` | numeric lists |
| `first()` `last()` | `T` | |
| `get(i)` | `T` | same as `xs[i]` |
| `length()` | `int` | |
| `join(sep)` | `str` | a `str[]` only |
| `parallel(f)` | `U[]` | like `map`, evaluated concurrently |

```maca
xs = [5, 3, 8, 1]
info("{xs.map(v => v * 2).sum()}")            // 34
info("{xs.filter(v => v > 3).length()}")      // 2
info("{xs.reduce(0, (a, b) => a + b)}")       // 17
info("{xs.sort().first()}")                   // 1
```

The methods that "change" a list return a new one and leave the receiver
alone:

```maca
ys = [3, 1, 2]
sorted = ys.sort()
// ys.first() is still 3; sorted.first() is 1
```

`xs.push(9)` is an expression whose value is the longer list, not a statement
that grows `xs`; the same for `set`, `insert` and `remove`.
[Memory](04-memory.md) explains why this is not the performance mistake it looks
like.

`sort_by` orders on a key rather than the element, and it is stable:

```maca
ws = ["bb", "a", "cc", "d"]
info(ws.sort_by(w => w.length()).join(","))   // a,d,bb,cc
```

`enumerate()` pairs each element with its position, and `slice` takes a start
and an **exclusive** end:

```maca
for e in ["a", "b"].enumerate() {
    info("{e.index}: {e.value}")              // 0: a, then 1: b
}
```

```maca
xs = [10, 20, 30, 40, 50]
xs.slice(1, 3)      // [20, 30]
```

### Ranges

`lo..hi` is a half-open integer range, an `int[]`, so `0..xs.length()` is
exactly the indices of `xs`:

```maca
for i in 0..5 {
    info("{i}")     // 0 1 2 3 4
}

xs = "a", "b", "c"
for i in 0..xs.length() {
    info("{i}: {xs[i]}")
}

info("{(1..101).sum()}")    // 5050, because 101 is not included
```

### Passing a named function

A method that takes a function accepts a lambda or a top-level function's
name:

```maca
is_even(n: int) -> bool => n % 2 == 0

evens = [1, 2, 3, 4].filter(is_even)
```

[Functions and Control Flow](11-closures.md) covers what a function value is.

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
| `slice(from, to)` | `str` | `to` is **exclusive**, as on a list |
| `index_of(s)` | `int` | `-1` when absent |
| `repeat(n)` | `str` | |
| `pad_start(w, p)` `pad_end(w, p)` | `str` | `p` defaults to a space |
| `pad_center(w, p)` | `str` | |
| `chars()` | `str[]` | one-character strings |
| `at(i)` | `str` | the character at `i` |
| `is_whitespace()` `is_ascii_digit()` `is_alpha()` | `bool` | character classes |

**`slice` takes an exclusive end, `substr` takes a length.** The names and
conventions are the same on a list and a string.

```maca
"abcdef".slice(1, 3)      // "bc"  (up to, not including, index 3)
"abcdef".substr(1, 3)     // "bcd" (three characters from index 1)
```

`chars`, `at` and the three character classes are what a scanner is built from;
`apps/selfhost/lexer.maca` uses nothing else:

```maca
run_digits(cs: str[], i: int) -> int =>
    i >= cs.length() || !cs.get(i).is_ascii_digit()
        ? i
        : run_digits(cs, i + 1)
```

### A misspelt method is caught, not linked

Method calls are otherwise gradual, but the method set of a `str` or a `T[]` is
closed, so a name outside it is a typo, and near misses get a suggestion:

```
UndefinedName: `str` has no method `lenght`; did you mean `length`?
```

## Strings and characters

Maca has no character type. `at(i)` gives a one-character `str`:

```maca
c = "hello".at(1)
info("{c == "e"}")      // true
```

`length` counts bytes, so non-ASCII text has a length larger than its character
count. Only indexing needs care with multi-byte text.

## Run it

```
maca run apps/examples/collections.maca
```

Every list method above, applied and printed. The complete, *closed* method sets
are in [The Standard Library](a3-stdlib.md).
