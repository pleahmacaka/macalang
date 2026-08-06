# A First Program

One program end to end: a word tally that takes a sentence and reports how often
each word appears. It lives at `apps/examples/wordcount.maca`, and the test
suite runs it.

## Splitting the text

```maca
words(text: str) -> str[] =>
    text.lower().replace(".", " ").replace(",", " ").split(" ")
```

`str[]` is a list of strings. Maca writes the element type first and the
brackets after, like C, not `List<str>`. There are no angle brackets in the
language at all.

`text.lower()` is a method call on a string, but `str` is a primitive with no
methods of its own. This is **UFCS**: uniform function call syntax. `x.f(y)`
means `f(x, y)`, so any function whose first parameter fits can be called this
way.

The body is an **arrow body**: `=>` followed by a single expression, which is
the function's result. There is no `return` in Maca.

## Counting

```maca
Tally = {
    word: str
    count: int
}
```

That is a record: Maca's struct. `Name = { … }` declares the type; inside,
`field: type`. **`:` introduces a type, `=` introduces a value**, everywhere in
the language.

Recording a sighting of one word:

```maca
bump(ts: Tally[], w: str) -> Tally[] {
    at = find(ts, w, 0)
    at < 0
        ? ts ++ [Tally { word = w, count = 1 }]
        : replace_at(ts, at, Tally { word = w, count = ts.get(at).count + 1 })
}
```

A **block body** (braces instead of `=>`), because it needs a local binding
first. The last expression in the block is the value.

`at = find(...)` binds a local. There is no `let`. `c ? a : b` is the ternary,
and it is an expression. `++` concatenates: lists here, strings elsewhere.
`Tally { word = w, count = 1 }` builds a record.

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
this shape constantly. `replace_at` assigns through an index with `ts[at] = t`.

## Folding over the words

```maca
tally(ws: str[], i: int, acc: Tally[]) -> Tally[] =>
    i >= ws.length()
        ? acc
        : tally(ws, i + 1, ws.get(i).length() == 0 ? acc : bump(acc, ws.get(i)))
```

An accumulator threaded through a recursive call: the standard fold. Splitting
on spaces leaves empty strings behind, so empty words are skipped.

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

The string is **interpolated**: `{expr}` is evaluated and spliced in. `{…:<8}`
is a format spec (left-align in eight columns).
[The next chapter](03-common-concepts.md) covers the full spec grammar. `if`
here is a statement whose value is discarded; the block's value is the `0`.

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

`main`'s result is the process exit status.

## Running it

```
maca run apps/examples/wordcount.maca
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

Behind `maca run`, the program was parsed, type-checked, lowered to C, compiled
by a real C compiler, and the resulting binary cached. A second run of an
unchanged program skips all of it.

## What was not explained

Why `Tally` is a record rather than a class, what happens to the memory when a
list is rebuilt, why there is no `return`, how `str[]` gets its methods. Those
are the next chapters.
