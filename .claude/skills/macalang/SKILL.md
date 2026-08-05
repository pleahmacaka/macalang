---
name: macalang
description: Write and edit Maca (.maca / maca.toml). Use when creating or editing .maca source or maca.toml manifests, or config-mode NixOS/home-manager modules in Maca. Enforces the minimal syntax and the "always verify with maca.check" habit.
---

# Writing Maca

Maca is one typed language for programs **and** infra config. Minimal,
rule-based syntax, with no idioms to memorize. Verify with `maca.check` before
finishing.

## The rules models get wrong

- **No `fn`, no `let`, no `return`, no `type`, no `Result`/`Ok`, no `<>`
  generics.** Functions are `name(x: T) -> R { body }` or `=> expr`, and the
  last expression is the value. Types are declared by binding:
  `Status = Todo | Doing | Done`, `Task = { id: int, title: str }`.
- **Field `:` = type, `=` = value.** `Name { field = value }` constructs;
  `{ field: Type }` declares a record type.
- **Spaced `? :` is the ternary; attached `x?` propagates an error.** They are
  different tokens: `c ? x : y` vs `load()?`.
- **Bracketless comma lists:** `xs = a, b, c` (not `[a, b, c]`, though `[]`
  brackets are used for empty/nested lists).
- **A `"…"` string may not span a line.** Write `\n`, or use `"""…"""`, which
  spans lines and does not interpolate. A `{` inside a string opens an
  interpolation, so a literal brace is `\{` or `{{`.
- **A binary operator may not begin a continuation line.** Put the `+` at the
  end of the line above; starting a line with one is a parse error, and in a
  file that is *imported* it used to fail silently.
- **`main() -> int`** for CLIs; `main() -> Element` for UI.
- **`match` must be exhaustive** over a sum type, or include `_`. Arms are
  separated by newlines, not commas.
- **`xs.push(v)` returns a new list.** Append with `xs = xs.push(v)`.
- **`for i in 1..n` is inclusive** on both ends.
- **Config mode is pure `<>`**: no effects (`info`, file I/O, …) in a NixOS/
  home-manager module.

## Types worth knowing

A generic names its own element type: `first(xs: a[]) -> a`,
`sort_by(xs: a[], key: (a) -> str) -> a[]`. A function type is written
`(T, U) -> R` with the parens required, and you only need it for a record
field: a function *passed* as an argument needs no annotation.

## Modules

A path is the whole name. `modules/http/server.maca` is `http/server`, from
anywhere in the tree. There is no entry file and no index, and a directory is
not a module. `import { listen } from http/server` pulls in only what you name.
The packages are `std` (`text`, `list`, `path`, `json`, `csv`, `fs`, `proc`),
`http`, `tambo`, `cli`, `bench`, `profile` and `signal`.

## The verify habit

Always run the `maca.check` MCP tool (or `maca lint`) on what you wrote and fix
every diagnostic before finishing. Diagnostic kinds: `type-mismatch`,
`non-exhaustive`, `effect-in-config`, `unknown-option`, `immutable`,
`undefined-name`.

For a **config module**, say so: `maca.check` with `config: true`, or
`maca lint --config`. `effect-in-config` and `unknown-option` only exist in
config mode, and nothing about a file says which mode it is for, so a config
module checked as a program comes back clean when it is not.

Behaviour goes in a `test_…` function checked with `assert`/`assert_eq` and run
by `maca test <file>`, which reports each one and exits with the failure count.
Do not print results and read the output back.

## Skeletons

CLI:

```maca
main(args: str[]) -> int {
    match args {
        "hello", ..rest => info("hi")
        _               => info("usage: app hello")
    }
    0
}
```

Typed program:

```maca
Shape = Circle | Square

area_name(s: Shape) -> str => match s {
    Circle => "circle"
    Square => "square"
}
```

A test file:

```maca
greet(name: str) -> str => "hi, {name}"

test_a_name_is_greeted() {
    assert_eq(greet("Mia"), "hi, Mia", "the name is used verbatim")
}

main() -> int {
    test_a_name_is_greeted()
    failures()
}
```

Config module (pure):

```maca
system.stateVersion = "24.11"
services.openssh = {
    passwordAuthentication = false
}
```
