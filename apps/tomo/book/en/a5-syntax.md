# Syntax

Every form the language has. The explanations are in
[Learning Maca](03-common-concepts.md). Two rules run through all of it:

- **`:` introduces a type, `=` introduces a value.** Declarations, literals,
  parameters and bindings all obey it.
- **Attached and spaced mean different things.** `x?` propagates a failure,
  `c ? x : y` is a ternary; `{n:>8}` is a format spec, `{c ? a : b}` is a
  ternary inside a string; `data-id` is one identifier, `a - b` is a
  subtraction.

## A file

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

## Comments

| Form | Meaning |
|---|---|
| `// text` | a comment |
| `/* text */` | a comment too |
| `/** text */` | a doc comment: the block MacaDoc reads |

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

`alias N = T` introduces `N` as the name of `T`, and the checker treats the two
as distinct names. A return type may be omitted and inferred from the body; a parameter's type may
be omitted and inferred from use, including as a function when the body calls
it.

A **record** declares one `field: type` per line. A **sum** separates variants
with `|`, and a variant may carry a payload written like a parameter list.

```maca
Shape = Circle(int) | Rect(int, int) | Empty
```

Both may refer to themselves: a self-referential sum payload is boxed, and a
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

## Types

| Form | Type |
|---|---|
| `int` `float` `bool` `str` | the scalars |
| `T[]` | a list of `T` |
| `Map str V` | a string-keyed map, applied by juxtaposition |
| `Future a` | the result of a `spawn` |
| `a`, `b`, … | a type variable: any lowercase name |
| `any` | the gradual escape hatch |

## Expressions

| Form | Meaning |
|---|---|
| `42` `3.14` `true` `"hi"` | literals |
| `0xff` `0b1010` `0o755` `1_000_000` | integer literals; `_` is ignored |
| `[a, b, c]` | a list |
| `lo..hi` | a half-open integer range, an `int[]` |
| `Name { f = v }` | a record literal |
| `{ f = v }` | a record literal whose type is its fields |
| `base with { f = v }` | a functional update |
| `r.f` | a field |
| `xs[i]` | an element of a list, or one byte of a `str` |
| `f(a, b)` | a call |
| `x.f(y)` | UFCS: exactly `f(x, y)` |
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
| `tag(attr="v", child)` | an element; see [the UI syntax](a11-ui.md) |

A **named function**'s `=> { … }` is the one place both readings are live, and
the body decides: a record literal's fields are `name = value` with commas, and
a block's statements are newline-separated with the last one the value:

```maca
// a block: the last line is a bare expression, which is no field
total() -> int => {
    a = 1
    a + 41
}

// a record literal: a comma separates fields, and never statements
origin() -> Point => { x = 0, y = 0 }
```

Three things rule the record out on their own: an entry that is not
`name = value` (a bare expression, `xs[i] = v`, `p.f = v`, `const x = e`, a
punned `{ x, y }`), a name bound twice, and empty braces.

When every entry is a distinct `name = value` and only newlines separate them,
neither reading is taken:

```
`mk`: this `=> { … }` reads as a record literal and as a block. Write
`Name { … }` for the record, or drop the `=>` for the block
```

## Statements

| Form | Does |
|---|---|
| `x = e` | bind or assign |
| `xs[i] = e` | assign through an index |
| `r.f = e` | assign a field |
| `while c { … }` | loop while `c` |
| `for x in xs { … }` | iterate a list or a range |
| `break` `continue` | leave, or skip to the next iteration |
| `return` `return e` | leave the enclosing function |
| `f(a: T) { … }` | define a function here, over this scope |

`for i in lo..hi` lowers to a counting loop; a range in value position
(`xs = 1..n`) materialises a list.

`break`, `continue` and `return` have no value, so they cannot be the arm of a
ternary: `c ? continue : 0` and `c ? return 1 : 2` are both a `TypeMismatch`.
The same rule sends `return` out of a lambda body and out of an `=> e` body.
`return e` needs the function to declare a result; a bare `return` needs it not
to.

A function defined inside a block reads and writes the scope holding it, and is
a value. It is in scope from where it is written, not before, so it can neither
call itself nor a sibling defined below it. See
[Closures and Control Flow](11-closures.md).

## Patterns

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

## Line breaks

Newlines end a statement. An expression continues onto the next line in two
cases.

**A trailing operator always continues.**

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

Anything else at the start of a line is a new statement. For `+`, `*`, `/`, `%`,
`<<`, `>>` and the comparisons that is a parse error:

```
parse (58, 59): unexpected token Plus
```

For `-` and `!` it is **not** an error, because both can begin an expression:

```maca
n = a
    - 3        // n is a; `- 3` is a statement of its own
```

Nothing warns. Put the operator at the end of the line, or name an intermediate
local.

## Bracketless lists

A comma list needs no brackets where the shape is unambiguous: an arrow body
returning a list, a `for` header, a rest pattern:

```maca
pair() -> int[] => 1, 2
```

Call arguments may be separated by commas **or** by nothing at all, which is why
an element's children need no punctuation:

```maca
article(class="prose",
    h1("Title")
    span("Body"))
```

## Where to go next

- The reserved words: [Keywords](a1-keywords.md).
- Every operator and its precedence: [Operators and Symbols](a2-operators.md).
- What the checker does with all of this: [The Type System](a6-types.md).
