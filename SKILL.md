---
name: maca
description: Write and edit Maca (.maca / maca.toml). Use when creating or editing .maca source or maca.toml manifests, or config-mode NixOS/home-manager modules in Maca. Enforces the minimal syntax and the "always verify with maca.check" habit.
---

# Writing Maca

Maca is one typed language for programs **and** infra config. Minimal, rule-based
syntax — no idioms to memorize. Verify with `maca.check` before finishing.

## The rules models get wrong

- **No `fn`, no `type`, no `Result`/`Ok`, no `<>` generics.** Functions are
  `name(x: T) -> R { body }` or `=> expr`. Types are declared by binding:
  `Status = Todo | Doing | Done`, `Task = { id: int, title: str }`.
- **Field `:` = type, `=` = value.** `Name { field = value }` constructs;
  `{ field: Type }` declares a record type.
- **Spaced `? :` is the ternary; attached `x?` propagates an error.** They are
  different tokens: `c ? x : y` vs `load()?`.
- **Bracketless comma lists:** `xs = a, b, c` (not `[a, b, c]`, though `[]`
  brackets are used for empty/nested lists).
- **`main() -> int`** for CLIs; `main() -> Element` for UI.
- **`match` must be exhaustive** over a sum type, or include `_`.
- **Config mode is pure `<>`** — no effects (`info`, file I/O, …) in a NixOS/
  home-manager module.

## The verify habit

Always run the `maca.check` MCP tool (or `maca lint`) on what you wrote and fix
every diagnostic before finishing. Diagnostic kinds: `type-mismatch`,
`non-exhaustive`, `effect-in-config`, `unknown-option`.

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

Config module (pure):

```maca
system.stateVersion = "24.11"
services.openssh = {
    passwordAuthentication = false
}
```
