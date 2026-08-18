# Maca: the specification

> One typed language for programs **and** infrastructure config, sharing one
> syntax and one type system. General programs compile to native (C-tier
> binary), JS, the JVM, Rust, Elixir, or freestanding C; infra config compiles
> to Nix.

This file is the authoritative description of the language. When a design
decision changes, this file and the code change together; the spec wins ties.

## Architecture

```
                          ┌─▶ C      ─▶ binary    CLI (C-tier), the default
.maca ─▶ maca-compiler ───┼─▶ js     ─▶ browser   web / UI
                          ├─▶ java   ─▶ JVM       Minecraft / Maven interop
                          ├─▶ rust   ─▶ crate     crates.io interop
                          ├─▶ elixir ─▶ BEAM      many small waits at once
                          ├─▶ C      ─▶ MCU       freestanding, Cortex-M / RISC-V
                          └─▶ nix    ─▶ .nix      config
```

- **Shared:** frontend, type checker, effect checker, core IR
- **Split:** codegen, runtime
- **Native = C:** `Maca → C → zig cc` (static musl) for every span, SIMD
  included: `f32x8` is an `ext_vector_type` the C backend lowers, built with
  `-mavx2`. There is no second native compiler to keep in step.

## Modes

| | General mode | Config mode |
|---|---|---|
| character | imperative/functional, effects | declarative, idempotent, pure `<>` |
| backend | native, JS, JVM, Rust, embedded | Nix |
| entry | `main` | root module (= configuration.nix) |
| run | runtime execution | Nix eval → derivation |

Mode is selected by target kind in `maca.toml`: `[[bin]]` = program,
`[hosts.X]` = config. See *The manifest* below for what else a manifest says
and which manifest answers.

## Language cheatsheet

- No `fn`, no `type`, no `Result`/`Ok` in surface syntax, no `<>` generics.
- Field syntax disambiguates: `:` field = type, `=` field = value,
  `Name { = }` = constructor. Single namespace for types and values.
- Bracketless comma lists (`xs = a, b, c`); significant newlines; records are
  newline-separated `{ }`.
- Functions: `f(x: T) -> R { body }` or `=> expr`. The two brace forms are one
  function: `f() -> R => { … }` *is* `f() -> R { … }`, and the parser keeps the
  block. See the arrow-brace rule below.
- **Variadic `...rest: T`** is the trailing arguments, collected. The written
  type is one argument's, because that is what a call site writes; inside the
  body `rest` is a `T[]`. Three dots and not two: `..` already means an
  half-open range and a list rest pattern, and the ellipsis is *prefix* so the
  mark is on the parameter rather than on the type. It must be the last
  parameter, must name its element type, and cannot be `main`. A call passes at
  least the fixed parameters and any number more; `f(1, 2, 3)` and `f([1, 2, 3])`
  against `f(xs: T[])` lower to the same list. A variadic is callable and
  nothing else: it has no arity as a value, so it cannot be passed to a
  higher-order parameter or stored in a `(T, U) -> R` field.
  (`tests/programs/variadic.maca`.)
- Errors are the inferred `exn` effect; propagate with `x?`, raise with `fail e`.
- Effects (Koka-style, inferred): `io`, `net`, `os`, `async`, `exn`. Config mode
  forces `<>`.
- **Colorblind async, no `async` keyword.** Any function can suspend; the
  `async` effect is inferred, never written. `spawn f(x)` runs `f` concurrently
  and yields a `Future`; `await fut` suspends until it resolves. `sleep_ms(ms)`
  is a suspension point. A task is an ordinary function: no coloring, no ABI
  change. (Native: POSIX-thread-backed, so a suspension point is a real thread
  boundary. The playground interpreter runs it eagerly.)
- Generics: lowercase type vars, applied by juxtaposition (`Map k v`), postfix
  `T[]` / `T?`. Nullable `T?` = `T | None`.
- **Closures / first-class functions.** A lambda `v => …` captures its enclosing
  scope; lowered to a `maca_closure` (code pointer + heap env), one ABI for
  capturing and non-capturing lambdas, callable as a value (`f = v => …; f(x)`).
- **`return`, for leaving a function early.** A function's last expression is
  still its value; `return` is the guard that gets out before reaching it, so
  the body after a guard need not be nested inside an `else`. `return` with no
  value leaves a function that declares no result; `return e` leaves one that
  declares `-> T`, and `e` is checked against `T`. `return` is a **statement**:
  it stands on a line of its own, or as the tail of an `if`/`match`/`for`/
  `while` branch. Anywhere the value would have nowhere to go (a ternary arm, a
  call argument, an operand, a lambda body, an `=> e` function body) it is a
  `TypeMismatch` naming the function, before any backend sees it. Lowered to the
  host language's own `return` on native C, JS, Rust, JVM and embedded C; config
  mode has no function to leave and says so.
  (`tests/programs/early_return.maca`.)
- **A named function nested in a function.** A block may define a function, and
  that function reads and **writes** the scope enclosing it, so a view can own
  its state and define its handlers beside it. It is a closure value, so it is
  passed, stored in a `(T, U) -> R` field, and may outlive the call that made
  it. Scope is the block's one rule: a nested definition is in scope **from
  where it is written**, not before, because a closure captures when it is
  made. That rules out mutual recursion and self-recursion between nested
  definitions, each with its own diagnostic pointing at the top level. A local
  that any nested definition **assigns** is shared by all of them (the C backend
  gives it a heap cell both reach); one that none assigns is copied, as a
  lambda's captures always were. Native C and JS lower it; `rust`, `jvm`,
  `embedded` and the playground interpreter refuse it by name.
  (`tests/programs/nested_fns.maca`.)
- **List stdlib (UFCS on `T[]`):** `map`/`filter`/`reduce`/`fold` (closures
  typed by the element), `sort`/`reverse`/`push`/`pop`/`contains`/`index_of`/
  `sum`/`min`/`max`/`first`/`last`, plus the editing and searching pair:
  `set(i, x)`/`insert(i, x)`/`remove(i)` (a new list, edited at `i`; an index
  the list does not have leaves it alone, and `insert` clamps, which is the
  rule `get` and `slice` already keep), `sort_by(key)` (stable, on an `int`,
  `float` or `str` key), `index_of_by(pred)` (the first index the predicate
  accepts, else `-1`) and `enumerate()` (a `{index, value}[]`). `set`, `insert`
  and `remove` obey the same ownership rule as `push`: `xs = xs.insert(0, v)`
  edits in place, `ys = xs.insert(0, v)` copies. String stdlib on `str`
  (`split`/`trim`/`upper`/…). Math prelude (`sqrt`/`pow`/`abs`/`min`/`max`/
  `clamp`/`gcd`/…), always available. (`apps/examples/{collections,strings,math}.maca`,
  `tests/programs/list_edits.maca`.)
- **Running another program is `exec(cmd, args)`**, a prelude builtin whose
  result is the child's exit code (`127` when it could not be started, `128 +
  signal` when it was killed, `-1` when the fork itself failed);
  `capture(cmd, args)` is the same call for its stdout, and
  `capture_err(cmd, args)` for its stderr. Each reads one stream and leaves the
  other alone, so a failing build's diagnostics are readable on their own,
  which is what a test that asserts on a diagnostic needs. `args` is a
  `str[]` and there is **no shell in between**, so an argument holding a space, a
  quote or a `$` is one argument rather than three, while `execvp` still searches
  `PATH`, which is the one shell behaviour a build step wants. `import std/proc`
  is the layer above it: `run` stops the program when the child fails, `try_run`
  reports it, `run_in` sets the working directory first. The C back end written
  in Maca lowers `exec` itself, which is what lets the self-hosted compiler
  drive a C compiler and hand back an executable rather than a `.c` file.
  `capture_err` is that back end's alone, because the frozen Rust bootstrap
  predates it. (`modules/std/proc.maca`, `apps/maca1/main.maca`,
  `modules/maca/tests/proc.maca`.)
- **Typed JSON (`import std/json`).** `encode(value)` and `decode(text)` are
  written by the compiler from the record and sum types the program declares:
  a record is an object with one member per field in declaration order, a list
  is an array, and the primitives are themselves. `decode` reads into whatever
  the binding says (`c: Config = decode(text)`), so a bare `decode(text)` with
  no destination is a build error. **A sum is its variant name in lower case**
  (`Layout = List | Grid` is stored `"list"`/`"grid"`), which is total in both
  directions because Maca capitalises variants; a variant with a payload has no
  JSON form beyond its name. Text that does not match the type `fail`s with a
  message naming the field (`field `columns`: expected a number, got a
  string`), so `try` catches it like any other failure. The rest of `std/json`
  stays the untyped text layer. (`tests/programs/json_typed.maca`.)
- Ternary is spaced `c ? x : y`; error-propagation is attached `x?`.
- Operator overloading (no new syntax): on a user type, an operator resolves to
  a same-named function: `a + b` → `add(a, b)`, `==` → `eq`, `<` → `lt`, etc.
  Primitives keep the native operator. (`apps/examples/operators.maca`.)
- Pattern & codegen completeness: record patterns (`match p { { x, y } => … }`),
  the `!` (logical-not) prefix operator, and `+` as the one join: it
  concatenates when *both* sides are strings or both are lists, while a number
  beside a string stays a mistake, which is what keeps `+` from silently
  stringifying. Binary `++` is refused by name (joining is `+`), and the only
  `++` left is the statement `n++` (or `++n`), which is `n = n + 1` written
  small,
  and record fields that reference a record declared later in the file all work
  natively. The parser no longer hangs on malformed input (a stalled `ident()`
  now advances). (`apps/examples/record_pattern.maca`.)
- Error model: `fail msg` raises (prints + `exit(1)` if unhandled); `try e` /
  `reify e` catches a failure via runtime setjmp/longjmp and yields the caught
  message (`str`, `""` on success), discharging the `exn` effect; execution
  continues past a caught failure. (`apps/examples/catch.maca`.)
- Sum types with payloads: `Shape = Circle(int) | Rect(int, int)`. Constructors
  are typed as functions (`Circle(10)`), payloads bind in patterns
  (`Circle(r) => r * r`). Native lowering is a tagged struct/union with a
  per-variant constructor; plain (nullary-only) enums are unchanged. Parses with
  no stage-0 front-end change. (`apps/examples/payload_sum.maca`.) A payload may be a
  record, in either declaration order. The C backend emits records and tagged
  sums in one combined dependency order, so the record struct is defined before a
  sum that carries it (and vice-versa). (`apps/examples/sum_record.maca`.) That
  order follows a field held **by value**; a `T[]` field is a heap pointer the
  forward declaration already covers, so two records may name each other as long
  as one side reaches the other only through an array
  (`tests/programs/mutual_records.maca`). Sums may be
  **recursive** (`Tree = Leaf(int) | Node(Tree, Tree)`, `List = Nil | Cons(int,
  List)`): a self-referential payload is boxed (a heap pointer) so the tagged
  union stays finite; the native backend emits a named, forward-declared struct,
  allocates the box in the constructor, and dereferences it when a match binds it.
  (`apps/examples/tree.maca`.)
- Arithmetic operators: `%` (modulo) and `<<` / `>>` (shifts) join
  `+ - * /`; all integer-only, checked and lowered on every backend.
  (`apps/examples/fizzbuzz.maca`.)
- Bindings (no `let`): a bare lowercase `x = e` is a mutable variable;
  `const x = e`, `x = e as const`, or a Capitalized name is a constant.
  Reassigning a constant is rejected (`DiagKind::Immutable`); a Capitalized
  constant works but `maca lint` warns. (`apps/examples/bad/reassign_const.maca`.)
- Imperative loops: `while cond { … }` with `break`/`continue`, plus
  reassignment of a mutable binding (`i = i + 1`). The `while` condition must be
  `bool`. Lowered to native C, embedded C, and JS.
  (`apps/examples/loops.maca`; `apps/examples/bad/while_cond.maca` is rejected.)
- Half-open integer ranges `lo..hi` (counts `lo … hi - 1`, so `5..5` is empty),
  an `int[]` value. `for i in lo..hi` lowers to a counting loop in C (no array
  materialized) in the frozen stage-0 backend; the Maca-written backend
  materializes it through `maca_range` in both positions, which `xs = 1..n`
  needs anyway. Endpoints must be `int`. (`apps/examples/range.maca`;
  `apps/examples/bad/range_end.maca` is rejected.)
- Dev environments in Maca (`maca dev`): `dev.maca` (config mode) → a self-
  contained `flake.nix` devShell via the Nix backend's `emit_flake`. `dev.name`,
  `dev.packages = a, b`, `dev.env = { K = "v" }`, `dev.shellHook`. Replaces a
  hand-written flake; the repo's own `flake.nix` is generated from `dev.maca`.
  See `docs/DEVENV.md`.
- Embedded (`maca build --target embedded --mcu cortex-m4`): Maca → freestanding
  C + a Cortex-M/RISC-V startup (vector table, reset handler) + linker script,
  cross-compiled with clang/lld to `firmware.elf`/`.bin`. `int` = 32-bit word;
  MMIO intrinsics `mmio_write/read`, `set_bits/clear_bits/toggle_bits`, `bit`,
  `shl/shr/bit_or/bit_and`, `delay`, `nop`; `for _ in forever()` = super-loop.
  Hex/binary/octal literals with `_` separators (`0x4002_0C00`). `apps/blink`.
- JVM interop (`maca build --target jvm`): Maca → Java source. `import java
  "pkg.Class"` → a Java import; `Name : Iface = { m = () => … }` → a class
  implementing `Iface` (a Fabric `ModInitializer`); a capitalized call
  `Pos(x,y,z)` → `new Pos(...)`; `obj.m(a)`/`Blocks.STONE` pass through. An
  unknown capitalized annotation is a foreign type → gradual `any`. Enables
  Minecraft (Fabric) modding in Maca, in `apps/mcmod`.
- Subscripting: `xs[i]` reads a list element or a one-character `str` from a
  string; `xs[i] = v` and `p.field = v` assign through the lvalue. Arrays lower
  to the runtime buffer (`arr.data[i]`), strings to `maca_str_at`; JS/JVM/embedded
  use their native subscript. (`apps/examples/indexing.maca`.)
- **A string is indexed by byte.** `length`, `at`, `get`, `slice` and `chars`
  all count bytes, and `at(i)` is therefore the one-byte `str` at offset `i`,
  never a code point. That is what `maca_str_at` does and what `chr`/`ord`
  assume, so a caller assembling UTF-8 does it a byte at a time on purpose.
- Functional record update: `base with { field = value }` yields a copy of a
  record with the named fields overwritten, leaving the original binding
  unchanged (C: struct copy; JS: object spread). (`apps/examples/record_update.maca`.)
- **A record literal is the record type it is written into.** `Point = { x: int,
  y: int }` is nominal, `{ x = 5, y = 6 }` is structural, and the two are one
  type: a literal *becomes* the named record wherever an annotation, a return
  type, a parameter, a field, or a list element names one, so the record's own
  struct is what is built and no second one is synthesized. Meeting the
  declaration is also what checks the literal against it: a record literal is
  otherwise open (field access is row-polymorphic), and a field nobody wrote
  would be silently zero, so a literal written into a named record has to name
  every field the record declares and no field it doesn't. With no such context a
  literal stays structural, and the native and Rust back ends synthesize one
  struct per distinct shape. (`tests/programs/arrow_records.maca`.)
- **The arrow-brace rule.** `=>` then `{` is a record literal and a block at the
  same time, and the shape of the body decides which, because a record literal's
  fields are `name = value` with commas between them and a block's statements are
  newline-separated with the last one the value:
  - some entry is not `name = value`, or a name is bound twice, or the braces are
    empty, and it is a **block**, parsed as `FnBody::Block` because
    `f() -> R => { … }` is `f() -> R { … }`; the pretty-printer behind the LSP's
    format command and `maca.fmt` prints back the spelling without the spare
    `=>` (`maca fmt` itself only re-indents, so it keeps either);
  - every entry is a distinct `name = value` and a comma sits at the brace's own
    depth, which no block has, and it is a **record literal** (a trailing comma
    counts, so `{ x = 1, }` is one). An entry may be written as the bare field
    name, which is that name as its own value: `Point { x, y }` is
    `Point { x = x, y = y }`;
  - every entry is a distinct `name = value` with only newlines between them, and
    both readings hold, so **neither is taken**: refused by name, showing the
    record spelling (`Name { … }`) and the block spelling (drop the `=>`).
  A *lambda*'s `=> {` is always a block, as is a match arm's, and parenthesising
  is how a record is written there (`(n) => ({ x = n })`).
- Paths are literals: `/tmp` `./x` `../x` `~/x`, joined with `p / "seg"`.
- SIMD vectors are first-class value types: `f32x8`, `i32x4`, … (native only).
- UI: elements are functions (`div(class=..., ...children)`); compile-time
  reactivity; the platform's own attribute names (`onclick=`, `oninput=`,
  `ondrop=`, and `value=` on a state name, which binds two ways); Tailwind
  first-class. The older `on:` / `bind:` directives still parse and mean the
  same thing.
  **Assignment is the update**: writing a declared state name repaints the
  nodes that read it, so a handler needs no `update()` call, and everything one
  handler assigns is repainted once, when the handler returns. `update()` (and
  `maca.refresh()`) remain, for when something outside Maca moved.
  A text/attribute/`class` child that reads
  state (or calls a function) re-renders; a text-returning call used as a child
  (`span(fmt(x))`) is a text node, not an element (only known HTML tags build
  elements); `html=expr` sets `innerHTML`; transpiled functions resolve state
  names to `state.x` so handlers can read and mutate state.
  **A child may be a list of elements.** `Element` is the type of a rendered
  element (a `str` natively, a DOM node on `js`) and `Element[]` a list of
  them; each element of the list is a child in its place, `+` joins two such
  lists, and `[]` contributes *nothing at all* on either target, which is what
  replaces a `class="hidden"` ternary. A function says so in its signature
  (`-> Element` / `-> Element[]`), which is what tells the compiler the call
  hands back nodes rather than text.
  (`tests/programs/ui.maca`,
  `modules/maca/tests/js.maca`.) The browser
  playground itself is a single Maca file (`apps/playground/playground.maca`)
  compiled by this backend, carrying its styles and the WebAssembly-bridge
  runtime inline via named raw-string blocks. An **asset**
  is named by its extension rather than by a keyword: `import "theme.css"`,
  `import "vendor/x.js"`, `import "m.wasm"`. Naming what you want out of a
  package is `import { pick_text } from "npm:pkg"`, and a snake_case name binds
  a camelCase or kebab-case export, so Maca spells it Maca's way.
- **A page's assets and its identity.** A quoted `"…"` *names* something and
  carries no language word: the extension says what a file is, and a package
  says what it is itself. A raw `"""…"""` block is named the same way, by the
  string in front of it, so no import anywhere carries a language word. So
  `import "page.css" """…"""` carries inline CSS while
  `import "vendor/daisyui.css"` embeds that file's bytes in a `<style>`
  ahead of the generated one, `import "vendor/x.js"` embeds a `<script>`
  ahead of the app, and `import "x.wasm"` embeds a base64 blob. Every asset
  is inlined, never linked: `index.html` is the whole deployable. A path that
  resolves to no file is a build error naming it.
  **An asset import may name a package instead of a path.** `npm:` is the same
  prefix `maca.toml` writes for a dependency, and it means the same thing here,
  so `import "npm:daisyui/dist/full.css"` reaches whatever `maca add npm:daisyui`
  installed and the directory it installed into never appears in the source.
  A package named without an extension, `import "npm:daisyui"`, says what it is
  in its own manifest: the first of `style`, `browser`, `module`, `main` that
  names a file a page can carry (`.css` a stylesheet, `.js`/`.mjs`/`.cjs` a
  script, `.wasm` WebAssembly) is both the kind and the entry. Several stated
  entries are not an ambiguity, because that list is ordered and a page is a
  browser. This is why an asset import needs no language word even when there is
  no extension to read. A package that states none is an error naming it, and
  `import "npm:daisyui/dist/full.css"` is how a file inside a package is named
  when the entry it leads with is not the one wanted, which is also the only way
  to ask for a kind the package does not lead with. A package that is not installed is an error naming it and saying to
  `maca add` it, never a silent fall-through, and a scoped `npm:@scope/pkg` is
  reached under the bare name `maca add` installed it as
  (`modules/maca/tests/page.maca`).
  The page's title is `[page] title` in the `maca.toml` nearest the source
  (`lang` and `description` too), falling back to the source file's stem, so a
  page is named by what it is rather than by what its file is called; the same
  title names the window under `--target tauri`. An unknown key under `[page]`
  is an error.
- The **JS bridge** is what a JavaScript block and the program say to each
  other, and the only thing either may assume about the other. `maca.get(name)`
  reads declared state, `maca.set(name, value)` (or `maca.set({…})`) writes it
  and refreshes the view, `maca.refresh()` re-syncs the bound nodes, and
  `maca.provide({ f })` hands back a function the Maca side *declared without a
  body*, which is how a signature like `cfg_write(title: str) -> bool` gets an
  implementation. A name the program never declared throws rather than becoming
  a field, and so does writing a constant. The emitted file is bridge, then
  block, then app, so a block's top level runs with the bridge available and
  before the first render. The generated `state`/`update()` remain, and remain
  the generator's own names rather than an interface. Reference:
  `apps/tomo/book/{en,ko}/a13-ffi.md`.

The standard library surface is *The Standard Library* in the handbook's
reference (`apps/tomo/book/en/a3-stdlib.md`), and the examples are
`apps/examples/*.maca`. The handbook is two volumes over one chapter list: *Learning
Maca* (`00-`…`18-`) teaches and the *Reference* (`a1-`…`a16-`) answers, and
`apps/tomo/book.toml` is the order they are read in.

## The manifest

`maca.toml` says what a directory is. A repository that holds more than one
thing writes one manifest per thing, plus a root that gathers them:

```toml
# the root
[package]
name = "maca"
version = "0.2.1"

[workspace]
members = ["modules/std", "apps/tomo"]

[format]
indent_size = 4
```

```toml
# modules/std/maca.toml
[package]
name = "std"
description = "The layer above the prelude builtins."
```

**Precedence is one rule: the nearest manifest that states a key answers for
it.** The chain covering a file runs from the manifest in its own directory up
to the workspace root, and a manifest that says nothing about a key inherits
the answer from the one above. That is why `modules/std` states its name and
not its version: the version it releases under is the workspace's, and a second
copy is a copy that goes stale.

Three tables are not settings and so are not on the chain, because each is the
answer to "which directory is this" rather than "how is this built":
`[workspace]`, which may appear only at the root and is what makes that
directory the root; `[package]`, which every member must state a `name` in; and
the `[[bin]]` blocks, which say what *this* package builds. Every path a
manifest writes is relative to the directory that manifest sits in.

**Members are listed, and the list is checked against the tree in both
directions.** A listed member with no `maca.toml` is an error naming it, and a
directory beside a member that holds a `maca.toml` and is not listed is an
error naming it too. A convention would silently adopt a stray directory; a
list on its own would silently drift from the tree; the list plus the check
does neither. A directory becomes a package by writing a `maca.toml`, and by
nothing else, so a scratch directory is never one.

**A per-package manifest changes nothing about which directories are import
search roots, nor the order they are tried in.** It changes only where the
search stops: the walk up from the importing file used to end at the first
`maca.toml` it met, and now ends at the workspace root, because a member's
manifest is not the edge of the world. `modules/std/text.maca` still reaches
`modules/` and so still resolves `import std/list`.

With no file named, `maca build`, `maca run` and `maca test` are about the
package the working directory holds: its `[[bin]]` (`--bin <name>` when it
declares more than one) and the `.maca` suites under its `tests` directory
(`[package] tests` renames it). A library that declares no `[[bin]]` is told
so, by name, rather than quietly building something else.

**Building is declared, not flagged.** `[build]` carries the five things
`maca build` would otherwise learn only from a flag, each of them a property of
the project rather than of the invocation: `target` (`--target`), `out` (`-o`),
`mcu` (`--mcu`), `classpath` (`--cp`) and `bin` (`--bin`, which `[[bin]]` a
bare `maca build` or `maca run` means).

```toml
[build]
target = "js"
out = "build"
```

is a project that builds by saying `maca build`. A flag on the command line
wins over the manifest, because the flag is one invocation and the manifest is
the project; a declared target also wins over the one the compiler would have
guessed from the source. `out` is a path like any other a manifest writes, so
it is relative to the directory that manifest sits in. An unknown key under
`[build]` is an error naming it, as under `[page]`.

`maca init` writes exactly the two files a project cannot do without: a
`maca.toml` stating its `[package] name` and the `[[bin]]` it builds, and that
`main.maca`. No commentary, and no table the project has not yet needed.

## The standard library travels with the compiler

**`maca` carries its own standard library.** All nine packages under
`modules/` (`std`, `cli`, `http`, `bench`, `profile`, `signal`, `tambo`, `web`)
are compiled into the binary, the way `maca-runtime` compiles the C runtime
into it, so `import std/json` means the same thing in a downloaded release as
it does in this repository. A release is still two files, `maca` and
`maca-lsp`, and there is nothing beside them to install, to find, or to keep in
step with the version that reads it.

Embedded, rather than installed beside the binary, for the reason the C runtime
already is: a single downloaded file should be the whole compiler. A directory
of `.maca` files next to the executable is a second thing to package, a second
thing an installer can drop, and a second thing that can be a different version
from the binary reading it. Nothing that can go missing can go missing.

The source has to be on disk for the compiler to resolve, read and inline it,
so the first import that reaches the carried copy unpacks it, once, under the
cache directory (`MACA_CACHE`, else `XDG_CACHE_HOME`, else `~/.cache/maca`) in
a directory named for the compiler's version and a digest of the files. A
different compiler unpacks somewhere of its own, so two versions on one machine
never read each other's `std`. The unpack is whole or not at all: files are
written to a temporary directory and renamed into place, and a `maca.toml`
declaring `[workspace]` is written at its root so the carried packages resolve
their own imports without walking out into whatever holds the cache.

**Precedence: the carried copy is the last thing asked.** Resolution is
unchanged, and the reordering of the search roots that this repository tried
once and reverted is still not the answer. The walk from the importing file up
to the workspace root runs first, over the written path and then the search
roots (`modules`, `src`, `maca_modules`) at each step; only when that whole
walk has found nothing does the compiler offer its own copy. So a project's own
`modules/std/text.maca` wins, and so does a `maca_modules/std/` that `maca add`
installed, without any rule being added for them.

**A replacement is a replacement for the packages that read it, too.** A file
the compiler carries resolves its imports against the project that asked for
it, not against the directory it was unpacked into: a project that writes its
own `modules/std/text.maca` gets that file both where it writes `import
std/text` and inside the carried `std/json` that reads `std/text`. Without that
the two copies would be two modules and a program reaching both would be told
`lines` is defined twice, which is a confusing way to say a vendored fix did
not take.

`MACA_STDLIB=<dir>` replaces the carried copy outright: imports resolve in that
directory instead, and nothing is unpacked. It is how a checkout of this
repository can be put in front of an installed compiler.

Inside this repository nothing changes, because `modules/` is a search root
under the workspace root and the walk finds it long before the fallback: the
tree stays the source of truth and is what the suites exercise. The carried
copy cannot drift from it either, since it is read out of `modules/` when the
compiler is built and
`modules/maca/tests/imports.maca` compares the two file by file.

## Status

The compiler is complete and every `.maca` suite is green.
Front-end (lexer → parser → gradual type/effect checker → core IR) plus
backends: native **C** (default, SIMD included), **Nix** (config mode),
**JS** (reactive UI + Tailwind), **JVM** (Java source), **Rust** source, and
**embedded** (freestanding C for Cortex-M / RISC-V). Driver: `init` / `build`
(`--target nix|js|jvm|rust|embedded|tauri`) / `run` / `dev` / `watch` / `fmt` /
`lint` / `test` / `profile` / `add` / `update` / `upgrade` / `bindgen`. Tooling:
LSP, MCP server, and a browser playground authored in Maca itself
(`apps/playground/playground.maca`, compiled by the JS backend) plus the wasm
front-end it loads (`apps/npm/wasm.maca`, compiled by the C backend and linked
by `zig cc -target wasm32-wasi`).

Every script in the repository is a Maca program: the site builder, the
benchmark harness, the linter, `bindgen`, and the npm package's wasm build.
There are no shell scripts left. Installing is a binary in the release,
`maca-install-<os>-<arch>`, built from `apps/install` by the same matrix that
builds the toolchain, because it runs where there is no `maca` yet and on a
Windows runner that has no C compiler to build one with.

The Rust workspace is the frozen **stage-0 bootstrap**; compiler work is
written in Maca under `modules/maca/` and gated by the stage-0 front-end (see
`docs/BOOTSTRAP.md`). Prefer adding to `modules/maca/*.maca` over growing the Rust
crates.

## Golden examples (regression set)

Verbatim from the spec, under `apps/examples/`:
`hello.maca`, `taskr.maca` (CLI), `system.maca` (config), `counter.maca` (UI),
`dot.maca` (SIMD), plus `apps/examples/bad/*.maca` for diagnostics, and the
language-surface goldens (`indexing`, `record_update`, `tree`, `sum_record`,
`keywords`, `generic`). Changing a design updates this file and the affected
example together; the spec wins ties.

`apps/examples/` is that set and only that set: a file is there because a test, this
document, or a handbook chapter names it. A runnable program built on a package
is an application and lives under `apps/` in a directory of its own
(`apps/cli_tool`, `apps/bench_demo`, `apps/profile_demo`, `apps/signal_demo`,
`apps/tambo_demo`). `taskr.maca` is the one runnable program that stays, because
it is also the lexer's golden token dump, the parser round-trip and the
`maca fmt --check` golden.
