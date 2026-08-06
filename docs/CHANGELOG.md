# Changelog

Newest first. Versions are bare semver; the tag is the version.

## Unreleased

* Two records may name each other when one side is reached only through an array.
  The C backend ordered struct bodies by every field, arrays included, so a pair
  like `Block { entries: Entry[] }` and `Entry { child: Block }` could be emitted
  with `Entry` first and the C would not compile: `field has incomplete type`. A
  body only needs its **by-value** fields complete, since an array is a heap
  pointer the forward declaration already covers, and that is what the order
  follows now. This is what lets the compiler's own tree hold a block of
  statements inside an expression.
* The self-hosted parser says what it stepped over, and `++` between two lists is
  a list. `Module` carries an error list beside its items, so a token that starts
  no declaration is named with its offset rather than dropped, and `apps/maca1`
  prints the scan and parse errors separately and counts both into its exit
  status. `binop_type` typed every `++` as `str`, so a local holding
  `xs ++ [v]` was declared `const char*` in C even though the operands lowered as
  lists; it takes the list type from whichever side has one.
* The self-hosted scanner has somewhere to put a problem, and the parser cannot
  run off the end. `lex_all` returns the tokens **and** the errors, so a byte with
  no token kind and a string whose quote never closes are reported instead of
  being flattened into an identifier; `apps/maca1` prints them and counts them
  into its exit status. `parse_module` used to hand any leading token to the
  function-declaration parser, which read past the end of the token list: a file
  holding the single name `a` took the whole process down. It now requires a
  `name (` before it reads a declaration and steps over anything else, so the
  walk always advances.
* The self-hosted compiler has array operations, and types a method call at all.
  `check.maca` had no rule for a method, so every `.slice(i, j)` came out
  untyped and a local holding one was declared `int` in C. It types the receiver
  now, and `.length()`/`.get(i)`/`.slice`/`.index_of`/`.join`/`.push` each choose
  a list or a string lowering from it. `++` between two lists copies the two
  blocks rather than running through `maca_cat`. The C preamble gains
  `maca_list_cat`, `maca_list_slice`, `maca_list_index_of`, `maca_str_index_of`
  and `maca_list_join`; Rust uses `concat`, a range `to_vec`, `iter().position`
  and `join`. A program using all of them compiles and runs to the same answer
  on both back ends.
* The self-hosted scanner reads an escaped quote and a line comment. `string_end`
  stopped at the first `"` it met, so `"\""` came back as a string cut in half,
  and `//` scanned as two `/` operators. The compiler's own source is full of
  `\"` and carries the `//` blurb its package index prints, so it could not be
  scanned at all before this.
* The self-hosted checker writes what it inferred into the tree, and the two Maca
  back ends read it rather than guessing from an expression's shape. `.length()`
  on an array is the list's own field and on a string is `strlen`; `.get(i)` is an
  element or a byte by the same rule; and a local's C type is the type its
  initialiser was lowered to, so `t = 1.5 * 2.0` is a `double` where the shape of
  a `*` could only say `int`. This is what a compiler needs before it can read its
  own source, where `ts.length()` and `s.length()` sit side by side.
* The self-hosted compiler reads a record update and a shorthand field.
  `base with { field = value }` lowers to a C statement expression over
  `__typeof__` and to a Rust block that mutates its own copy, so neither back end
  needs to know the record's name. A field written as a bare name is that name as
  its own value, which the spec described only as `name = value` although stage-0
  has always accepted it and the compiler's own source is written in it.
* The self-hosted compiler reads `if`. `if c { a } else if d { b } else { e }`
  parses to a new AST node that nests to the right, types by unifying its
  branches, and lowers to a C ternary chain and a native Rust `if`. It is the
  construct `modules/maca/*.maca` is written in, 315 branches of it, so nothing
  else about reading its own source could start until it landed.
* `crates/options` is gone. An empty crate: no code, no dependencies, and
  nothing referred to it.
* An ambiguous import names its two files with one separator. The candidate
  paths were built by joining the written import onto a directory, so on Windows
  the diagnostic read `…\modules\bench/stat.maca`, half one way and half the
  other. The path is pushed a segment at a time now.
* Ten tests that only ever ran on a Windows box were failing there, and none of
  them for a reason worth calling environmental. The LSP tests built a `file://`
  URI by interpolating a path, so a Windows path spelled `\U` inside the JSON
  and the server never parsed the request; one `file_uri` helper now
  percent-encodes at every call site, which is what the single passing test had
  been doing by hand. Two standard-library resolution tests compared a raw path
  against a canonicalised prefix, which cannot match once `canonicalize` adds
  Windows' verbatim prefix. `taskr`'s suite deleted `store.json` under
  `$XDG_DATA_HOME` while the program writes `taskr.json` beside itself, so the
  store grew by two tasks a run and every run after the first one failed. The
  SIMD test wanted `48` from a dot product that is an `f32`, and `str` on a float
  keeps its point on every back end, so `48.0` is the answer. And
  `.gitattributes` keeps `.maca` at LF, so a CRLF checkout no longer makes
  `maca fmt --check` call every golden example unformatted.

* The type system is Maca. `modules/maca/ty.maca` is `crates/core/src/ty.rs`
  rewritten in the language it type-checks: `Ty` with type variables, the
  substitution and the occurs check, unification over constructors, functions,
  optionals and **rows** (an open record tolerates the fields it does not name,
  a closed one names what is missing or spare), plus `generalize` and
  `instantiate`. `modules/maca/tests/ty.maca` is the gate, 24 assertions in
  Maca rather than in Rust.
* The self-hosted checker unifies instead of comparing type names.
  `check.maca` now carries `Ty` everywhere and threads one substitution through
  the whole module, so a signature is a function type and a call checks its
  arguments rather than only counting them; a list, the arms of a `match` and
  the branches of a ternary agree with each other; `+` between two strings is
  rejected by the operator; and an **unannotated parameter is a fresh variable
  solved by how it is used**. The body narrows it, so `keep(x) -> int => x + 1`
  rejects `keep("s")`, and where the body says nothing it is generalized at the
  declaration and instantiated per call, so `keep(x) -> int => 1` may be used at
  `int` in one call and `str` in the next. A module is checked twice, once to
  infer and once to report, so declaration order does not change the answer. A
  clash yields an error type that absorbs, so one mistake is reported once.

* The site is a directory rather than one long page. `/` is eight cells on a
  hairline lattice, each numbered, each ending in a figure computed at build
  time and the committed file that figure came from; the features, the
  benchmarks, the evaluations, the targets, the install and the limits are six
  documents behind it, in both languages, fifteen pages in all.
* The evaluation scores are on the site, and the headline is the plain `spec`
  row rather than the retry row, because this project's own README says the
  retry figure is not yet evidence.
* Black and white. The amber loss accent is gone: a losing benchmark bar is a
  hollow rule and the losing verdict is the filled chip, so rank is carried by
  ink rather than by hue, and nothing on the site is rounded.
* Languages are a dropdown. The row of side by side links is gone, replaced by
  one JavaScript-free `details` disclosure the whole site shares.

* A grid cell can say where it sits: `col-span-4`, `col-span-full`,
  `row-span-2`, `col-start-3`, `col-end-9` and `grid-rows-3` produce CSS, and a
  breakpoint reaches them. `grid-cols-12` on its own was twelve equal columns
  and no way to be wider than one.

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
