# Changelog

Newest first. Versions are bare semver; the tag is the version.

## Unreleased

* A function is a value in the emitted C, and the count for the compiler's own
  source falls 26 to 8. A function type is spelled the way `ty.maca` already prints
  one, `(int, str) -> bool`, so `surface_of` can write it into the tree and the C
  back end can read the real parameter and result types back out of it: a
  higher-order parameter is declared `int (*pred)(const char*)` and the call goes
  through a pointer whose type matches its callee exactly, not a cast. Annotating an
  item now writes the inferred parameter types back into the declaration, which is
  what the emitter reads. `.map(f)` lowers to a loop that calls the function
  directly, so it needs no pointer at all. A deep resolve was needed as well: the
  plain one stops at the top level, so a parameter's result variable stayed unbound
  and the spelling came out with nothing after the arrow. The Rust back end reads
  the same spelling as `fn(String) -> i64`.
* An escaped brace in a string is a brace. The scanner keeps `\{` in a token's text,
  so `str_node` read it as an interpolation and `split_interp` cut the string at it:
  the emitter was handed a part whose text was a lone backslash and wrote an
  unterminated C literal, which derailed the C compiler for the rest of the file and
  hid every error after it. An escape now carries its next character with it, the
  interpolation test only fires on a brace that is neither escaped nor doubled, and
  the literal the emitter writes has the backslash removed. With the mask gone the
  real count for the compiler's own source is 26 C errors, none of them about a
  string.
* The C the self-hosted compiler emits for its own source went from 215 errors to
  8. Almost all of them were one omission: `emit_module` wrote items in source
  order and C wants a declaration before a use, so 81 calls were to undeclared
  functions and 57 more conflicted with a later definition. It emits the types, then
  every prototype, then every body, and `emit_fn` and the prototype share one
  parameter list so the two cannot drift. `str(x)` chose `maca_int_to_str` whatever
  it was given: it reads the argument's inferred type now, so a `str` is itself, a
  `float` and a `bool` get their own helper. The scan builtins the lexer is written
  in had no lowering at all and fell through to `recv.method(...)`, which is not C:
  `chars`, `is_whitespace`, `is_ascii_digit`, `is_alpha`, `upper`, `contains`,
  `slice` on a string, and `ends_with`. Two of them are `ctype.h` and needed no
  helper.
* A higher-order parameter is inferred and a function name is a value. `run_end(cs,
  i, pred)` left `pred` as a type variable, because a call only looked in the
  module's own signatures, and a function passed by name typed as `any`, so it
  matched anything. A call to a local unifies it with a function type built from the
  arguments, and a name that is a declared function carries that signature.
* A list may hold something that does not fit a cell. `MacaList` carries one
  `long` per cell, so `Ty[]`, `Token[]` and `Expr[]`, every list in the compiler's
  own source, emitted `(long)(v)` on a struct and the C would not compile. A
  non-scalar element is boxed now (`maca_box(sizeof(R), (R[]){ v })`, read back as
  `(*(R*)xs.data[i])`), which keeps one C array type so nothing depends on
  declaration order; a `str` cell is read as `const char*` rather than as `int`.
  Rust had the same bug from the other side: `rtype` mapped every `T[]` to
  `Vec<i64>`, so it recurses on the element now (`Vec<R>`, `Vec<String>`,
  `Vec<Vec<i64>>`), `get` clones a cell that is not `Copy`, a `join` separator is a
  slice, and a record derives `PartialEq` so a list of them can be searched.
  Annotating an expression walks a block's statements too, so a binding inside an
  `if` branch is typed like any other. Across the compiler's eight files the C
  errors fall 329 to 215 with none of them about a cell, and the Rust class this
  caused falls 104 to 7.
* An empty list literal is `maca_listv(0)` and not `maca_listv(0, )`, which C
  refused as a missing expression.
* A `match` arm may carry a guard, and every declaration of the compiler's own
  source now reaches its output. `_ if a > 5 => …` was read as if the guard were
  the arm's body, so `step` in `lexer.maca` swallowed the four functions after it.
  A guard is its own node: the C back end tests it (`&&`-ed with the pattern when
  there is one) and Rust writes `pat if guard =>`. `{{` inside a string was
  desugared as an interpolation of nothing, giving `maca_cat("", …)` where a
  literal brace belongs. 341 of 341 functions across the eight files are emitted.
* The self-hosted compiler reads every one of its own eight files with nothing to
  report. Three things stood between it and that. A record field declared as an
  array (`tys: Ty[]`) threw the field walk off, because it stepped three tokens
  for `name : Type` and an array type is five, so every record with a list field
  desynchronised the rest of the file. A string literal inside an interpolation
  (`"{show_joined(xs, " ", 0)}"`) ended the outer string at its first quote; the
  scan skips a `{…}` region now, and `{{`/`}}` still mean a literal brace. And the
  block-or-record test did not look at the token after a closing bracket, so the
  `if` in `at = index_of(x)` followed by `if …` read as a record field rather than
  as two statements. Eight files, 0 scan errors, 0 parse errors, 451 lines of C.
* The self-hosted parser cannot read past the end of its token list. Every list
  it walks stopped only at its own closing bracket, so a call, a list literal, a
  record body, a parameter list, a match arm, a payload or a block that ran to the
  end of input kept reading into memory that was not the list. On its own source
  that reached a byte pattern that looked like an integer token holding a null
  pointer, and `atoll(NULL)` took the process down: three of the compiler's own
  eight files crashed it. All nine loops stop at `Eof` now, `parse_variants`
  checks that a name follows a `|`, and the scanner ends the stream with a run of
  `Eof` tokens so a three-token lookahead cannot leave the list either. The
  compiler now reads all eight of its own files without crashing, emitting 428
  lines of C and reporting 31 constructs it cannot yet parse.
* An `if` branch may hold more than one statement. `Expr` carries a `stmts` list
  now, so a branch that binds a local is a block node rather than something the
  parser could not read: C lowers it to a statement expression
  (`({ int d = …; e; })`) and Rust to a block that ends in its value. Both compile
  and run to the same answer. This is what the compiler's own source is written
  in, 39 branches of it, and it is the alternative to rewriting all 39 to fit a
  parser that could only read one expression.
* A record literal needs a comma at the brace's own depth, as the spec always
  said. `opens_record_lit` asked only for a `name =`, so the condition of
  `if c { d = 1 d }` was read as the record literal `c { d = 1, d }`. Requiring the
  comma is what separates a record from a block.
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
