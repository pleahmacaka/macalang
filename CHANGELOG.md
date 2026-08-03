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
  and source links. Every code block in a doc body is highlighted by the same
  scanner that highlights the signature above it, rather than only the
  signature; a cross linked name shows a card with what it is, on hover and
  on focus; and each item is a panel with its kind, a copyable anchor and its
  source, so a page can be scanned rather than read.
* The site loads the font it asks for. `font-sans` has named Pretendard since
  the day it was written and nothing ever fetched it, so every reader without
  the font installed got system-ui, which is most of what the Korean half of
  the handbook was rendered in. The family is vendored as it ships, and a
  test resolves each page's link and the stylesheet's first shard to a file
  on disk.
* A dev mode for the site generators that patches the nodes that changed
  rather than reloading the page.
* The front page reads its benchmark numbers out of the measured data rather
  than carrying a copy of them.
* The playground gained a live preview, config mode, share links and an
  example picker, and its interpreter fails on a call it does not know
  instead of quietly answering unit.
* The language server answers `textDocument/codeAction`. It advertised hover,
  completion, definition, symbols, references, rename, signature help and
  formatting, and no code actions at all, so an editor offered nothing on a
  diagnostic it had just drawn. Quick fixes now cover `Immutable`, a phantom
  keyword, a misspelt UFCS method and a `NonExhaustive` match, and two
  refactorings rewrite a Capitalized constant as an explicit `const` and swap
  a function body between `=> e` and a block. Every candidate is applied,
  re-parsed and re-checked before it is offered, so an action that would
  break the file is dropped rather than listed.
* The language server asks the parser what a `{` opens instead of deciding
  again from the tokens. Its own copy had no case for a `=>` sitting against
  the brace, so `mk(n: int) -> Point => { x = n, y = n }` read as a block and
  a rename of the field skipped the literal's key.

### House style

Two thousand em dashes, nine en dashes and thirty-nine middle dots left the
prose, and a test keeps them out. Nobody decided to write one; they
accumulated across three hundred files, which is the kind of drift a test
catches and a note in a style document does not. An appositive takes a colon,
an aside takes a comma pair or parentheses, and a consequence takes a full
stop.

## 0.1.0

First release.
