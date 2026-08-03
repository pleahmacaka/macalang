# A First Program

The best way to meet a language is to build something small in it. This chapter
writes one program end to end: a word tally that takes a sentence and reports
how often each word appears. It touches records, lists, recursion, pattern-free
control flow and string handling (most of what the next chapters cover in
depth) without explaining any of it fully. Read it for the shape.

The finished program lives at `examples/wordcount.maca` in the repository, and
the test suite runs it, so everything below is known to work.

## Splitting the text

Start with the input and the smallest useful step: turning a sentence into
words.

```maca
words(text: str) -> str[] =>
    text.lower().replace(".", " ").replace(",", " ").split(" ")
```

Three things are worth noticing.

`str[]` is a list of strings. Maca writes the element type first and the
brackets after, like C, not `List<str>`. There are no angle brackets in the
language at all.

`text.lower()` is a method call on a string, but `str` is a primitive with no
methods of its own. This is **UFCS**: uniform function call syntax. `x.f(y)`
means `f(x, y)`, so any function whose first parameter fits can be called this
way. The chain reads left to right in the order the work happens.

The body is an **arrow body**: `=>` followed by a single expression, which is
the function's result. There is no `return` in Maca.

## Counting

A word and its count belong together, so give them a name:

```maca
Tally = {
    word: str
    count: int
}
```

That is a record: Maca's struct. `Name = { … }` declares the type; inside,
`field: type`. Note the two different meanings of the punctuation, which is
consistent everywhere in the language: **`:` introduces a type, `=` introduces
a value**.

Now the operation that does the real work, which records a sighting of one word:

```maca
bump(ts: Tally[], w: str) -> Tally[] {
    at = find(ts, w, 0)
    at < 0
        ? ts ++ [Tally { word = w, count = 1 }]
        : replace_at(ts, at, Tally { word = w, count = ts.get(at).count + 1 })
}
```

This function has a **block body** (braces instead of `=>`) because it needs a
local binding first. The last expression in the block is still the value; again,
no `return`.

`at = find(...)` binds a local. There is no `let`. `c ? a : b` is the ternary,
and it is an expression, so it can be the whole body. `++` concatenates: lists
here, strings elsewhere.

`Tally { word = w, count = 1 }` builds a record. Type name, then `field = value`
pairs, with `=` because these are values.

The two helpers:

```maca
find(ts: Tally[], w: str, i: int) -> int =>
    i >= ts.length() ? -1 : (ts.get(i).word == w ? i : find(ts, w, i + 1))

replace_at(ts: Tally[], at: int, t: Tally) -> Tally[] {
    ts[at] = t
    ts
}
```

`find` is recursive, and carries its own cursor `i` as a parameter. You will see
this shape constantly in Maca code. It is how the compiler itself is written.

`replace_at` shows the other side: `ts[at] = t` assigns through an index. Maca
is not a pure language; it prefers expressions.

## Folding over the words

```maca
tally(ws: str[], i: int, acc: Tally[]) -> Tally[] =>
    i >= ws.length()
        ? acc
        : tally(ws, i + 1, ws.get(i).length() == 0 ? acc : bump(acc, ws.get(i)))
```

An accumulator threaded through a recursive call: the standard fold. Splitting
on spaces leaves empty strings behind (`"a.  b"` produces one), so empty words
are skipped rather than counted.

## Printing

```maca
show(ts: Tally[], i: int) -> int {
    if i < ts.length() {
        info("{ts.get(i).word:<8} {ts.get(i).count}")
        show(ts, i + 1)
    }
    0
}
```

`info` prints a line. The string is **interpolated**: `{expr}` is evaluated and
spliced in. `{…:<8}` is a format spec (left-align in eight columns), so the
counts line up. [The next chapter](03-common-concepts.md) covers the full spec
grammar.

`if` here is a statement whose value is discarded; the block's value is the `0`
on the last line.

## Main

```maca
main() -> int {
    text = "the quick brown fox. the lazy dog, the end."
    ts = tally(words(text), 0, [])
    info("{ts.length()} distinct words")
    show(ts, 0)
    0
}
```

`main() -> int` is the entry point, and its result is the process exit status:
`0` for success, as everywhere else.

## Running it

```
maca run examples/wordcount.maca
```

```
7 distinct words
the      3
quick    1
brown    1
fox      1
lazy     1
dog      1
end      1
```

`maca run` compiles and executes in one step. Behind that, the program was
parsed, type-checked, lowered to C, compiled by a real C compiler, and the
resulting binary cached. A second run of an unchanged program skips all of it.

## What was not explained

Deliberately: why `Tally` is a record rather than a class, what happens to the
memory when a list is rebuilt, why there is no `return`, how `str[]` gets its
methods. Those are the next chapters. What matters here is that a program is a
list of functions, a function is a signature and an expression, and the language
gets out of the way.
