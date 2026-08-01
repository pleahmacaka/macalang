# Changelog

## 0.2.0

The theme of this release is that a target now says what it cannot do.

Five of the seven back ends ended their expression lowering in a value:
`null` on jvm and nix, `0u` on embedded, `Default::default()` on rust,
`true` on the JS backend's pattern conditions. Every one of those type-checks
in the language it is emitting, so a construct nobody had implemented became
a plausible answer rather than an error. Each back end now refuses by name,
at compile time, in terms of what the author wrote.

### Breaking

* `f() => { … }` with a single `name = value` and no comma is a parse error.
  The brace was ambiguous between a block and a record literal and the
  ambiguity was resolved silently, so a two statement body compiled to a
  record with a duplicate key. Write `Name { … }` for the record, add a comma
  (a trailing one counts), or drop the `=>` for the block.
* Programs that relied on a back end silently accepting something it did not
  lower now fail to build. That is the point, but it is a behaviour change: a
  `match` on the embedded target, a float pattern on rust, arithmetic in a
  nix config and a closure on jvm all used to "work".

### Language

* Variadic parameters: `total(...rest: int) -> int`, written with the type of
  one argument, collected into a `T[]` at the call site.
* A block after `=>` is a block, and an anonymous record literal unifies with
  a declared record of the same shape.
* `x |> f` does something. It parsed before and was dropped.
* A function can be kept in a record field, declared `(T, U) -> R`, which is
  what makes a route table or a reducer expressible.
* A generic can name its own element type: `first(xs: a[]) -> a`.

### Back ends

* **JS**: sum types are built and matched rather than guessed. Constructors,
  list patterns and record patterns all lowered to the condition `true`, so
  the first arm of every such `match` won and payloads never bound. Every
  UFCS method now computes what the native back end computes, including the
  ones JS has and means differently (`push` returned a length, `sort`
  compared as strings).
* **rust**: `break` and `continue` are lowered, so a `while` can end; float,
  record and list patterns are refused rather than becoming wildcards.
* **jvm**: lambdas, `while`, stores and assignments are lowered; a
  function typed parameter becomes a real functional interface; a record's
  constructor follows the declaration rather than the literal, and its fields
  are read through their accessors.
* **nix**: a config computes its values. `port = 8000 + offset` emitted
  `null`, which Nix accepts as an option value.
* **embedded**: field and index stores are emitted rather than dropped.
* Every target resolves `import a/b`. It ran on two of seven, and the JS case
  exited zero and threw `ReferenceError` in the browser.

### Modules

`std`, `http`, `cli`, `bench`, `profile`, `signal` and `tambo`, all written in
Maca with suites under `<pkg>/tests/` run by `maca test`. A name is resolved
per module, so two files may each keep a private helper of the same name, and
an import that resolves to nothing is an error.

### Tooling

* A handbook in two volumes with one table of contents, in English and
  Korean, with Maca in fenced blocks highlighted by a scanner that follows
  the lexer.
* MacaDoc generates the API reference: a sidebar over every module and item,
  search, per item anchors, cross links from a signature to a declaration,
  and source links.
* A dev mode for the site generators that patches the nodes that changed
  rather than reloading the page.
* The front page reads its benchmark numbers out of the measured data rather
  than carrying a copy of them.
* The playground gained a live preview, config mode, share links and an
  example picker, and its interpreter fails on a call it does not know
  instead of quietly answering unit.

## 0.1.0

First release.
