# Changelog

## 0.3.0

The theme of this release is that a page written in Maca should be a Maca
program and nothing else. `tabpane`, a browser start page built on 0.2.1,
needed a build script, a hand-written JavaScript block, a generated module and
a `refresh()` after every write. None of that is left, and each thing that
replaced it is a language feature rather than a special case.

### Added

* The standard library rides **inside the binary**. All eight `modules/*`
  packages are compiled into `maca`, so `import std/json` resolves in a project
  that has never seen this repository; a project's own copy still wins.
* Every `modules/*` and `apps/*` is its own package with its own `maca.toml`,
  and the root is the **workspace** that gathers them. The nearest manifest
  that states a key answers for it.
* **Building is declared, not flagged**: `[build] target`, `out`, `mcu`,
  `classpath` and `bin` are properties of the project, and a flag on the line
  wins over the manifest.
* `return`, and a **named function nested in a function**, which is what lets a
  view keep its own helpers beside the state they read.
* `data("path")` reads a file while the program is built, into the type the
  binding declares.
* `stored(key, default)`: the name starts as whatever the browser saved, and
  assigning it saves it again. `web/storage`, `web/time`, `web/file` and
  `web/format` are the browser as ordinary Maca.
* `import css "npm:pkg"` and `import js "npm:pkg"` name a package rather than a
  path inside one.
* **JS**: assignment is the update. A handler assigns and the view repaints
  once, with no `refresh()`, and the platform's own event names (`onclick=`,
  `value=`) are what a page writes.
* A view can contribute a **list of elements, or none**, so a part of a page
  that is sometimes absent is `Element[]` returning `[]`.
* The six list edits a page makes: `set`, `insert`, `remove`, `index_of_by`,
  `enumerate`, `sort_by`.
* **JS**: `std/json`'s `encode` and `decode` are written from the record and
  sum types the program declares, as they already were natively. One suite
  holds both back ends to the same wire format and the same diagnostics, field
  for field.
* **JS**: `await` suspends. A function that reaches one becomes `async`, worked
  out by a fixpoint over the call graph, and a call to it waits, so a handler
  that waits for the reader is written the way every other handler is.
* `web/file`'s `pick_text(accept) -> str` answers with the file's text instead
  of writing it into a slot named by a string.
* A hyphen names a **custom element**: `iconify-icon(class=…, icon=…)` is a
  call like any other tag, on both the JS and the native back end.
* **JS**: a view called as a child is rebuilt when the state it reads changes,
  so a view that answers `[]` while a page is locked appears when it is not.
* `value=form.title` writes back the way `value=name` already did: a field or
  an element of something writable is writable.
* A script asset is run as an ES module when it is one, decided the way node
  decides it.

### Fixed

* `maca build --target js` runs the type checker, as every other target
  already did, and leaves no page behind when it refuses.
* A name one function keeps to itself is not the same name another function
  reads.
* **native**: a top-level name a function writes is a variable. Every write was
  going into a local that the end of the block threw away, so a handler that
  counted counted to one forever, and the JS target disagreed silently.
* **JS**: a page's state is what its initialisers compute. Anything but a
  literal became `null`, so a page built on `local_start(…)` or a constructor
  mounted against nulls.
* **JS**: `==` on a sum compares what it is. It was `===`, so a payload variant
  never equalled an equal one, and a nullary one stopped equalling itself once
  it had been read through the state proxy.
* **JS**: an event handler written as a call waits for the event instead of
  running while the page is built.
* **JS**: a record update in an arrow-function body is an object, not a block.
* A capitalized pattern is a constructor, so a misspelt one is a diagnostic
  naming the variant that was meant rather than a name that matches everything.
* **native**: an attribute given a function says so, whatever the attribute is
  called, instead of reaching `cc`.
* A selective import keeps every declaration its module's `import js` block
  provides, so importing one function of a bridge no longer costs the others.

## 0.2.1

The theme of this release is that a name should mean what its scope says it
means, and that a page should be able to say what it is instead of having its
HTML edited afterwards.

Three lowering defects all had the same shape: something resolved a name by
asking a question about the whole program when the answer belonged to a
smaller scope. Two of them produced wrong answers rather than errors.

### Fixed

* A record type reached through a selective import is concrete, not generic.
  An inlined module's items are renamed `<module>__<name>` and a module stem
  is lowercase, so `Tone` arrived as `highlight__Tone` and was read as a type
  variable. Every helper taking a list of it was monomorphized instead of
  emitted, and the C compiler was handed a call to a function that does not
  exist.
* A lambda captures the local its enclosing scope binds, not a top-level name
  that happens to spell it the same way. Captures were chosen by asking
  whether a name was a global *of the whole inlined program*, so a parameter
  stopped being captured the moment any file in the slice defined a top-level
  name with that spelling, and the body then read the global. Where the global
  was a binding rather than a function this was silent: `tagged("<", …)`
  returned `GLOBALa,GLOBALb`.
* **JS**: control flow in statement position is lowered as statements. It
  became an IIFE, which is a function boundary a `break` cannot cross, so
  `maca build --target js` could emit a file that does not parse.

### Added

* `[page]` in `maca.toml`: `title`, `lang` and `description` for the JS and
  Tauri targets. Without it a page was named after its source file. An
  unknown key is a build error, because a misspelt `titel` that quietly kept
  the old title is the same failure with a longer detour.
* `import css "vendor/x.css"` and `import js "vendor/x.js"` name a *file*,
  read at build time and inlined into the page. A quoted string is a path and
  a raw `"""…"""` block is the source itself, which is what every other
  foreign import already meant. A path that resolves to no file fails the
  build and names it.
* **JS**: the `maca` bridge, so a foreign block has a boundary it can name.
  `maca.get` / `maca.set` / `maca.refresh` / `maca.provide` replace reaching
  for the code generator's own `state` and `update`, and a body-less
  declaration emits a host stub, so `window.*` is no longer the channel.
  Reading or writing a name the program does not declare now says so, and
  says what *is* declared; it used to do nothing at all. `state`, `update`,
  `build` and `mount` still work.

### Changed

* The front page highlights its own code. `highlight` was written for the
  handbook and reused by the API pages, and the one page that shows the
  language first escaped it to plain text. A code block that is dark under
  both colour schemes asks for the dark palette with `code-dark`, rather than
  taking the light one and painting `#0a3069` strings on near-black.
* Every file that states the project's version is checked against the others.
  Six carried it and nothing compared them.

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
