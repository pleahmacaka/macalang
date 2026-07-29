# Appendix B: Operators and Symbols

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

A long condition can break across lines either way — leave the operator at the
end of the line, or start the next line with it. Both read as one expression:

```maca
ok = a > 0 &&
    b > 0

fine = a > 0
    && b > 0
```

## Bitwise

| Operator | Meaning |
|---|---|
| `<<` `>>` | shift |

## Concatenation

| Operator | Meaning |
|---|---|
| `++` | joins two strings, or two lists |

`+` does not concatenate. `++` is a separate operator precisely so that
`"1" ++ "2"` and `1 + 2` can never be confused.

## Control

| Form | Meaning |
|---|---|
| `c ? x : y` | ternary — written **spaced** |
| `x?` | propagate a failure — written **attached** |
| `lo..hi` | inclusive range |
| `x, ..rest` | rest pattern in a `match` |

The spaced-versus-attached distinction is load-bearing and appears three times in
the language. A spaced `:` is a ternary; an attached one, inside an
interpolation, is a format spec. A spaced `?` is a ternary; an attached one is
error propagation.

## Access

| Form | Meaning |
|---|---|
| `r.f` | field |
| `xs[i]` | index; also assignable |
| `x.f(y)` | UFCS — the same as `f(x, y)` |
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

A spec is sugar: `{x:.2}` is `x.fixed(2)` and `{x:>8}` is
`str(x).pad_start(8, " ")`, so every backend supports them identically.

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
5. `++`
6. `<<` `>>`
7. `+` `-`
8. `*` `/` `%`
9. unary `-` `!` `await` `spawn`
10. call, index, field, UFCS

`await` and `spawn` binding at unary precedence means `await a + await b` is
`(await a) + (await b)`, which is what you want.
