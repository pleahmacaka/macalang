# Appendix A: Keywords

Maca reserves 21 words. That is the whole list — a language you can hold in your
head is a design goal, not an accident.

| Keyword | Purpose |
|---|---|
| `const` | declare a constant binding |
| `as` | `x = e as const`, a trailing constant marker |
| `if` | conditional; also an expression |
| `else` | the other branch |
| `for` | iterate: `for x in xs { … }` |
| `in` | the `for` separator |
| `while` | loop while a condition holds |
| `break` | leave the innermost loop |
| `continue` | next iteration |
| `match` | destructure and branch |
| `import` | bring in a module, or a foreign library |
| `with` | functional record update: `p with { y = 5 }` |
| `fail` | raise an error |
| `try` | catch one |
| `alias` | a second name for a type |
| `await` | wait for a `Future` |
| `spawn` | start concurrent work |
| `true` | boolean |
| `false` | boolean |

`from` is *not* on this list. It appears in a selective import, but only there —
everywhere else it is an ordinary identifier, because `copy(from, to)` is too
natural a parameter name to spend a keyword on.

## Words Maca does not reserve

These are the ones people reach for from other languages. None of them is a
keyword, and using one as an identifier is legal — but if you use one *as if it
were* a keyword, the compiler tells you what Maca does instead.

| Not a keyword | What Maca does |
|---|---|
| `fn`, `func`, `def` | a function is `name(a: T) -> R { … }` or `=> e` |
| `let`, `var` | `x = e` binds; `const x = e` makes it constant |
| `return` | a function's last expression is its value |
| `type`, `struct`, `enum`, `class` | `Name = { f: T }` or `Name = A \| B` |
| `async` | async is an inferred effect; use `spawn` and `await` |
| `null`, `nil`, `None` | there is no null; use a sum type |
| `pub`, `private` | everything a module defines is importable |
| `mut`, `&`, `move` | there is no borrow checker (chapter 4) |
| `impl`, `trait`, `self` | a free function plus UFCS is the method |
| `new` | a record literal is `Name { f = v }` |

Because these are not reserved, a variable called `type` or a function called
`new` compiles. The C backend mangles names that collide with C keywords, so
`double`, `class` and `switch` all work as Maca identifiers too.

## Contextual punctuation

Not keywords, but worth listing here because they carry meaning:

| Token | Meaning |
|---|---|
| `:` | introduces a **type** |
| `=` | introduces a **value** |
| `=>` | an arrow function body, or a `match` arm |
| `(a, b) -> T => …` | a lambda with a declared return type |
| `->` | a return type |
| `?` `:` | the ternary, written **spaced** |
| `?` | attached to an expression: propagate a failure |
| `..` | an inclusive range, or a rest pattern |
| `\|` | separates sum-type variants |
| `_` | a wildcard pattern |
