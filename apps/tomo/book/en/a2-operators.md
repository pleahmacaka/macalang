# Operators and Symbols

Every operator, what it does, and where it binds. The lexical rules are in
[Syntax](a5-syntax.md).

## Arithmetic

| Operator | Meaning |
|---|---|
| `+` `-` `*` `/` | the usual, on `int` and `float` |
| `%` | remainder |
| `-x` | negation |

## Comparison and logic

| Operator | Meaning |
|---|---|
| `==` `!=` | equality; works on strings too |
| `<` `>` `<=` `>=` | ordering |
| `&&` `\|\|` | boolean and/or |
| `!x` | negation |

```maca
ok = a > 0 &&
    b > 0

fine = a > 0
    && b > 0
```

A long condition can break either way. Six operators may *begin* a line (`&&`,
`||`, `.`, `?` and `:`), because none can begin an expression; `+`, `*`
and the comparisons cannot, so a line starting with one is a parse error. `-` is
the trap: it can begin an expression, so a leading `-` starts a new statement and
nothing warns. [Syntax](a5-syntax.md) has the whole rule.

## Bitwise

| Operator | Meaning |
|---|---|
| `<<` `>>` | shift |

## Concatenation

| Operator | Meaning |
|---|---|
| `+` | adds two numbers, or joins two strings or two lists |

## Control

| Form | Meaning |
|---|---|
| `c ? x : y` | ternary, written **spaced** |
| `x?` | propagate a failure, written **attached** |
| `lo..hi` | half-open range, `hi` excluded |
| `x, ..rest` | rest pattern in a `match` |

Spaced versus attached appears three times: a spaced `?` is a ternary and an
attached one propagates a failure; a spaced `:` is a ternary's second half and an
attached one inside an interpolation is a format spec; a spaced `-` subtracts
and an attached one is part of a name, so `data-id` is one identifier.

## Access

| Form | Meaning |
|---|---|
| `r.f` | field |
| `xs[i]` | index; also assignable |
| `x.f(y)` | UFCS: the same as `f(x, y)` |
| `r with { f = v }` | functional update |

## In strings

| Form | Meaning |
|---|---|
| `{expr}` | interpolation |
| `{expr:spec}` | interpolation with a format spec |
| `\{` `\}` or `{{` `}}` | a literal brace |
| `\n` `\t` `\r` `\0` `\\` `\"` | escapes |
| `"""…"""` | raw: spans lines, no interpolation |

### Format specs

`[align][0][width][.precision]`, every part optional.

| Spec | On `3.14159` / `42` / `"ok"` | Result |
|---|---|---|
| `{x:.2}` | `3.14159` | `3.14` |
| `{x:>8}` | `42` | `      42` |
| `{x:<8}` | `"ok"` | `ok      ` |
| `{x:^8}` | `"ok"` | `   ok   ` |
| `{x:08}` | `42` | `00000042` |
| `{x:>10.3}` | `3.14159` | `     3.142` |

## Declarations

| Form | Meaning |
|---|---|
| `Name = { f: T }` | record type |
| `Name = A \| B(int)` | sum type |
| `f(a: T) -> R { … }` | function, block body |
| `f(a: T) -> R => e` | function, arrow body |
| `v => e` | lambda |
| `alias N = T` | type alias |

## Numeric literals

| Form | Value |
|---|---|
| `42` | decimal |
| `0xff` `0b1010` `0o755` | hex, binary, octal |
| `1_000_000` | `_` separators are ignored |
| `3.14` | float |

## Precedence, loosest to tightest

1. `? :` (ternary)
2. `\|\|`
3. `&&`
4. `==` `!=` `<` `>` `<=` `>=`
6. `<<` `>>`
7. `+` `-`
8. `*` `/` `%`
9. unary `-` `!` `await` `spawn`
10. call, index, field, UFCS

`await` and `spawn` bind at unary precedence, so `await a + await b` is
`(await a) + (await b)`.
