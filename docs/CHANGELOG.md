# Changelog

Newest first. Versions are bare semver; the tag is the version.

## Unreleased

* The language word on a quoted asset is gone, not just optional. `import css
  "theme.css"` is refused and names `import "theme.css"`; `import js
  "npm:pkg"` is refused and names `import { a, b } from "npm:pkg"`. A raw
  `"""…"""` block keeps its word, because a block of source has no name to read
  a kind off. A package named without an extension, `import "npm:daisyui"`,
  says what it is in its own `package.json`.
* A module value is computed once. `Stamp = now_ms()` was emitted as a
  function, so every read ran it again and answered a different number. Its C
  type was guessed from the initialiser's shape rather than taken from the
  lowering, so an intrinsic was declared `const char*` and given an integer,
  which crashed.
* `maca check` and the editor agree with `maca build` about `data(…)` and
  `stored(…)`. Both used to call them undefined, because only the build path
  rewrote them.

## 0.3.3

* An import says what it brings in. An asset is named by its extension,
  `import "npm:daisyui/dist/full.css"`, and a package by the names wanted out
  of it, `import { iconify_icon } from "npm:iconify-icon"`. A camelCase export
  answers to its snake_case spelling. `import js` and `import css` are gone.
* `maca spec --llm` prints the whole language as one generated document, under
  a 15,000 token budget a test enforces.
* `maca check --json` gives stable codes (`M0001`..`M0007`), spans with line and
  column, notes and suggestions; `maca fix` applies the machine-applicable ones.
  The schema is `docs/check-json.schema.json`.
* `maca check --target <t>` refuses an effect the target cannot carry. Config
  mode's rule, generalised to every target.

## 0.3.2

* `lo..hi` is half-open: it runs from `lo` up to but not including `hi`, so
  `0..xs.length()` is every index of `xs`. This changes what existing programs
  do, and is the reason for the minor bump rather than a patch.
* The installer binaries are in the release. 0.3.1 was cut one commit before
  they existed.
* Two fonts and no system fallback: Pretendard GOV Variable for prose,
  JetBrainsMono Nerd Font for code.
* The handbook opens on one table of contents rather than two.

## 0.3.1

* `maca install` fetches what `maca.toml` names, at the versions `maca.lock`
  pinned. It was written one commit after 0.3.0 was cut, so the released
  binary did not have it.
* `maca.lock` verifies. The `integrity` digest beside each pin is checked
  against the bytes that arrive, and a mismatch names both digests.
* The installer is a binary in the release, `maca-install-<os>-<arch>`.
  `install.sh` and `install.ps1` are gone.
* The tree is four directories: `apps/`, `crates/`, `docs/`, `modules/`.
* Every program target refuses the same diagnostic, and a test says which one
  stopped asking.

## 0.3.0

A page written in Maca is a Maca program and nothing else.

* The standard library rides inside the binary, so `import std/json` resolves
  in a project that has never seen this repository.
* Every `modules/*` and `apps/*` is a package with its own `maca.toml`; the
  root is the workspace that gathers them.
* Building is declared, not flagged: `[build] target`, `out`, `mcu`,
  `classpath`, `bin`.
* A page declares its identity: `[page] title`, `lang`, `description`.
* `modules/web`: the browser as ordinary Maca. `stored(key, default)` makes
  assignment the save.
* `maca` is the documented bridge to an `import js` block, replacing reaching
  into codegen internals.
* Typed `encode`/`decode` on the JS target, which were host stubs.

## 0.2.1

* UI syntax on every target: the C backend renders the same element call to an
  HTML string that the JS backend builds a reactive DOM from.
* Boolean and hyphenated attributes, and `element(tag, …)` with the tag as a
  value.
* `styles()` emits rules for exactly the Tailwind utilities the module names.

## 0.2.0

* Colorblind async: `spawn`, `await`, `sleep_ms`, with async as an inferred
  effect rather than a function colour.
* Closures and first-class functions, over one `maca_closure` ABI.
* The `modules/*` layout, and `maca -m module.function`.
* Backends for JVM, Rust and freestanding embedded C.
* Perceus reference counting in the C backend.

## 0.1.0

The first release: lexer, parser, type and effect checker, the C and Nix
backends, and the `maca` CLI.
