# Syntax

Every form the language has, in one place. This is the chapter to open when you
know what you want to write and need the exact spelling; the explanations are in
[Learning Maca](03-common-concepts.md).

Two rules run through all of it and are worth reading first:

- **`:` introduces a type, `=` introduces a value.** Declarations, literals,
  parameters and bindings all obey it.
- **Attached and spaced mean different things.** `x?` propagates a failure,
  `c ? x : y` is a ternary; `{n:>8}` is a format spec, `{c ? a : b}` is a ternary
  inside a string; `data-id` is one identifier, `a - b` is a subtraction.

## A file

A file is a module. At the top level it may hold imports, type declarations,
constants and functions, in any order — a definition may be used before the line
that declares it.

```maca
import std/text

Limit = 100

Point = {
    x: int
    y: int
}

main() -> int {
    info("{Limit}")
    0
}
```

There is no entry file, no `package` line and no `mod` block.

## Comments

| Form | Meaning |
|---|---|
| `// text` | a comment |
| `/// text` | a doc comment — the marker MacaDoc reads |
| `//// text` | a comment again; four slashes are not a doc comment |

To the compiler all three are the same token. The third slash is a convention
the documentation generator reads, not a keyword.

## Declarations

| Form | Declares |
|---|---|
| `f(a: T, b: U) -> R { … }` | a function with a block body |
| `f(a: T) -> R => e` | a function with an arrow body |
| `f(a) => e` | parameter and result inferred |
| `Name = { f: T }` | a record type |
| `Name = A \| B(int)` | a sum type |
| `alias N = T` | a name for a type |
| `Name = e` | a constant |
| `import a/b` | a module |

A function's return type may be omitted and is then inferred from the body. A
parameter's type may be omitted and is then inferred from use — including as a
function, when the body calls it.

`alias N = T` introduces `N` as the name of `T`. The checker treats the two as
distinct names rather than folding one into the other, so a value has to be
annotated `N` to pass where an `N` is declared. Use it to name a type, not to
move between one and another.

A **record** declares one `field: type` per line; commas at line ends are
allowed and not needed. A **sum** separates variants with `|`, and a variant may
carry a payload written like a parameter list.

```maca
Shape = Circle(int) | Rect(int, int) | Empty
```

Both may refer to themselves. A self-referential sum payload is boxed; a
self-referential record field is forward-declared in the generated C. See
[Memory and Ownership](a8-memory.md).

## Bindings

| Form | Binds |
|---|---|
| `x = e` | a mutable variable |
| `const x = e` | a constant |
| `x = e as const` | a constant |
| `Name = e` | a constant, by the capital letter |
| `x: T = e` | with an explicit type |

There is no `let`. The same `=` introduces a name and updates it: inside a
function, the first `x = e` declares and a later one assigns. Reassigning a
constant is an `Immutable` diagnostic.

## Types

| Form | Type |
|---|---|
| `int` `float` `bool` `str` | the scalars |
| `T[]` | a list of `T` |
| `Map str V` | a string-keyed map, applied by juxtaposition |
| `Future a` | the result of a `spawn` |
| `a`, `b`, … | a type variable — any lowercase name |
| `any` | the gradual escape hatch |

A lowercase name in type position is a variable, an uppercase one is concrete.
That is the whole of generics: there are no angle brackets in the language.

## Expressions

| Form | Meaning |
|---|---|
| `42` `3.14` `true` `"hi"` | literals |
| `0xff` `0b1010` `0o755` `1_000_000` | integer literals; `_` is ignored |
| `[a, b, c]` | a list |
| `lo..hi` | an inclusive integer range, an `int[]` |
| `Name { f = v }` | a record literal |
| `{ f = v }` | a record literal whose type is its fields |
| `base with { f = v }` | a functional update |
| `r.f` | a field |
| `xs[i]` | an element of a list, or one byte of a `str` |
| `f(a, b)` | a call |
| `x.f(y)` | UFCS — exactly `f(x, y)` |
| `Circle(2)` | a variant with a payload |
| `v => e` | a lambda |
| `(a, b) -> T => e` | a lambda with a declared return type |
| `c ? x : y` | a ternary |
| `if c { a } else { b }` | a conditional, in value position |
| `match e { … }` | a match, in value position |
| `{ … }` | a block; its value is its last expression |
| `spawn f(x)` | concurrent work, giving a `Future a` |
| `await fut` | its value, when it has one |
| `fail "msg"` | raise |
| `try e` | catch, giving the message or `""` |
| `x?` | propagate a failure to the caller |
| `tag(attr="v", child)` | an element — see [the UI syntax](a11-ui.md) |

A lambda body may be a block, so a bare `{ … }` after `=>` is that block rather
than a record literal. Parenthesise when the record is what you meant:
`(n) => ({ x = n })`. A match arm's `=> { … }` is a block for the same reason.

A **named function**'s `=> { … }` is the one place both readings are live, and
the body decides. A record literal's fields are `name = value` with commas
between them; a block's statements are newline-separated and the last one is the
value. So:

```maca
// a block: the last line is a bare expression, which is no field
total() -> int => {
    a = 1
    a + 41
}

// a record literal: a comma separates fields, and never statements
origin() -> Point => { x = 0, y = 0 }
```

The arrow adds nothing to the block form, and the compiler keeps the plain
spelling: `f() -> R => { … }` *is* `f() -> R { … }`, and the pretty-printer
(an editor's format command over the LSP, and `maca.fmt` over MCP) prints it
back without the `=>`. `maca fmt` on the command line only re-indents, so it
leaves whichever spelling you wrote.

Three things rule the record out on their own, because none of them can be a
field: an entry that is not `name = value` (a bare expression, `xs[i] = v`,
`p.f = v`, `const x = e`, a punned `{ x, y }`), a name bound twice (no record has
two `x` fields), and empty braces.

When every entry is a distinct `name = value` and only newlines separate them,
both readings hold and neither is taken:

```
`mk`: this `=> { … }` reads as a record literal and as a block. Write
`Name { … }` for the record, or drop the `=>` for the block
```

Guessing here is what the rule exists to avoid. Naming the constructor
(`Point { x = 1 }`) or dropping the arrow settles it, and both say plainly which
one you meant. So does a comma, wherever one fits: `{ x = 1, }` is a record,
because a trailing comma is a thing no block has either.

## Statements

| Form | Does |
|---|---|
| `x = e` | bind or assign |
| `xs[i] = e` | assign through an index |
| `r.f = e` | assign a field |
| `while c { … }` | loop while `c` |
| `for x in xs { … }` | iterate a list or a range |
| `break` `continue` | leave, or skip to the next iteration |

`for i in lo..hi` lowers to a counting loop; no list is built. A range in value
position (`xs = 1..n`) materialises one.

`break` and `continue` have no value, so they cannot be the arm of a ternary:
`c ? continue : 0` is a `TypeMismatch`.

## Patterns

Patterns appear in `match` arms.

| Pattern | Matches |
|---|---|
| `Red` | a nullary variant |
| `Circle(r)` | a variant, binding its payload to `r` |
| `Rect(w, h)` | a variant with several payload fields |
| `[]` | the empty list |
| `[x]` | a list of exactly one |
| `[x, ..rest]` | a head and the remainder |
| `x, ..rest` | the same, brackets dropped |
| `{ x, y }` | a record, binding its fields |
| `1` `"ok"` | a literal |
| `_` | anything |

A `match` over a sum type must cover every variant or a `NonExhaustive`
diagnostic follows. `_` satisfies it, which is the reason to prefer an arm.

## Strings

A `"…"` string interpolates and **may not span a line**. A `"""…"""` string
spans lines, interpolates nothing, and takes no escapes.

| Form | Meaning |
|---|---|
| `{expr}` | interpolation |
| `{expr:spec}` | interpolation with a format spec |
| `\{` `\}` or `{{` `}}` | a literal brace |
| `\n` `\t` `\r` `\0` `\\` `\"` | escapes |

A format spec is `[align][0][width][.precision]`, every part optional:

| Spec | Means |
|---|---|
| `>` `<` `^` | align right, left, centre |
| `0` | pad with zeros rather than spaces |
| `8` | a minimum width |
| `.3` | decimal places |

The parser desugars a spec into calls you could have written — `{x:.2}` is
`x.fixed(2)`, `{x:>8}` is `str(x).pad_start(8, " ")` — so every target renders
it identically.

## Line breaks

Newlines are significant: they end a statement. An expression continues onto the
next line in two cases, and the difference matters.

**A trailing operator always continues.** Leave the operator at the end of the
line and the next line is part of the same expression, whatever the operator is.

```maca
total = base +
    extra
```

**A leading token continues only when it cannot start an expression.** `&&`,
`||`, `++`, `.`, `?` and `:` qualify, so these are one expression each:

```maca
ok = a > 0
    && b > 0

name = first
    ++ " "
    ++ last

n = text.split(",")
    .length()

label = score >= 60
    ? "pass"
    : "fail"
```

Anything else at the start of a line is a new statement. For `+`, `*`, `/`,
`%`, `<<`, `>>` and the comparisons that is a parse error, which is the good
case:

```
parse (58, 59): unexpected token Plus
```

For `-` and `!` it is **not** an error, because both can begin an expression.
A leading `-` starts a fresh statement, and the binding above it keeps the value
it already had:

```maca
n = a
    - 3        // n is a; `- 3` is a statement of its own
```

Nothing warns about that one. Put the operator at the end of the line, or name
an intermediate local.

## Bracketless lists

A comma list needs no brackets where the shape is unambiguous — an arrow body
returning a list, a `for` header, a rest pattern:

```maca
pair() -> int[] => 1, 2
```

Call arguments may be separated by commas **or** by nothing at all. The second
is what makes nested markup readable and is the reason an element's children
need no punctuation between them:

```maca
article(class="prose",
    h1("Title")
    span("Body"))
```

## Where to go next

- The reserved words: [Keywords](a1-keywords.md).
- Every operator and its precedence: [Operators and Symbols](a2-operators.md).
- What the checker does with all of this: [The Type System](a6-types.md).
