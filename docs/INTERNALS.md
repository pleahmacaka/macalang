# Internals

What the compiler does, and the invariants that are easy to break
from inside it. The back ends have a page of their own, [BACKENDS.md](BACKENDS.md).

The compiler is complete and `cargo test` is green across the workspace. Whole
programs (non-`main` functions, records→structs, sum types→tagged enums, lists,
string interpolation, `match` lowering incl. list patterns (bracketless `x, ..rest` or bracketed
`[]`/`[x]`/`[x, ..rest]`), UFCS) compile and
run end-to-end (parse → check → C → `cc`/`zig cc` → execute).

**UI syntax on every target.** A tag name called as a function is an element:
named args → attributes, positional args → children (comma-separated *or*
juxtaposed). The JS backend builds a reactive DOM; the **C backend renders the
same call to an HTML string** (`maca_concat` chain; `maca_attr` escapes
attribute values, children are not re-escaped; void elements self-close;
`on:click=` is a clean error pointing at `--target js`). A user definition or
local **always shadows a tag**, so `label`/`code`/`main`/`p` are tags and
ordinary names. A **hyphenated attribute needs no workaround**: an attached
`-` is part of an identifier and a spaced one is the operator (the same
attached-vs-spaced
rule as `x?`/`c ? x : y`), so `nav(data-tomo="toc", span("{a - b}"))` is one
attribute and one subtraction. Two forms an identifier alone can't express: a
**bool** attribute controls the attribute's
*presence* (`open=true` → `<details open>`, `hidden=false` → nothing, via
`maca_flag`); and `element(tag, …)` takes the **tag as a value**
(`element("h" ++ n, …)`, `th`/`td` per row, and `<main>`, which no call can
name), and voidness is decided in `maca_element` at run time. `styles()` is the
generated stylesheet for exactly the Tailwind utilities the module's `class=`
strings mention (collected module-wide, *not* inside raw `"""…"""` strings, so
markup in a raw block silently gets no rules). Variants:
`hover/focus/active/first/last/open/before/after/marker/placeholder/details-marker`
(selector suffix) and `dark/sm/md/lg/xl/max-sm/max-md/max-lg` (`@media`),
combinable; arbitrary values `[…]` with `_`→space; selectors are `css_escape`d
(an unescaped `.max-w-[42rem]` is dropped by browsers *silently*) and rules are
emitted in `order()` so a variant beats the utility it modifies. Documented in
handbook ch. 15 (`apps/tomo/book/{en,ko}/15-ui.md`); gated by
`crates/driver/tests/native_ui.rs` + `crates/backend_js/tests/tailwind.rs`.

Backends: native C (default, SIMD included), Nix (config mode), JS
(reactive UI), JVM (Java source / Minecraft-Fabric interop), and embedded
(freestanding C for Cortex-M / RISC-V). Driver: `build` (`--target
nix|js|jvm|rust|embedded|tauri`, `--mcu`, `--cp`), `run`, `-m` (run a function
out of a module, as in `maca -m http.serve`; the spec reads as either
module + function or a whole path run under its own name, the exit status comes
from the entry point's declared return type, and a `str[]` parameter receives
the leftover command line), `dev` (dev-shell flake),
`watch`, `fmt`, `lint`, `test` (runs every `test_…` function in a file),
`profile`, `init`, `bindgen` (C header → Maca FFI
declarations: `char*`→`str`, other pointers→opaque `int`, float family→`float`).
The native `build`/`run` path
resolves local imports: `import a/b` (and single-word `import a`) inlines a
sibling `<a/b>.maca` / `<a>.maca` module, transitively, in dependency order, so
a program can span files (`maca build apps/maca1/main.maca` builds the whole
self-hosted front-end from its imports). **Selective import**,
`import { foo, bar } from a/b`, inlines only the named top-level definitions
plus the transitive closure of same-module definitions they reference (dead-code
elimination at the module boundary); a name the module doesn't define is a clean
error, not a dangling reference (`crates/parser/src/imports.rs`).

**A page's identity and its assets (JS/Tauri targets).** `[page]` in the
`maca.toml` nearest the source gives the page its `title` (falling back to the
source file's stem, which is all it used to have), plus `lang` and
`description`; an unknown key there is an error, not a default. A quoted
`"…"` names a file and takes no language word, because its extension already
says what it is; a raw `"""…"""` block keeps one, because a block of source has
no name to read. The quoted path is resolved against the importing file and
read at build time, so `import "vendor/x.css"` and `import "vendor/x.js"`
inline that file's bytes into the page the way `import wasm` already did (a
missing path is a build error naming it). The parser records which form was
written in the language word, because `Import::Foreign` has no room for a flag
and every backend pattern-matches its fields: the file forms are `stylesheet`
and `script`, which `print.rs` writes back as `css`/`js`, so `maca fmt` and the
module inliner do not turn an inline block into a path. Gated by
`crates/driver/tests/page.rs`.

**Processes, no shell:** `exec(cmd, args) -> int` (the exit code) and
`capture(cmd, args) -> str` (its stdout) are `fork` + `execvp`. `args` is a
`str[]` and each element is one argument however it is spelled, so
`exec("cp", ["my notes.txt", dest])` copies one file and `exec("echo",
["$HOME"])` prints `$HOME`. With `env`/`cwd`/`chdir` and `copy_bytes(src, dst)`
(a byte copy, because `write_file(read_file(…))` stops at the first NUL, so it
truncates any binary), that is what lets every script in the repository be a
Maca program. `std/proc` is the layer above: `run`, `try_run`, `run_in`,
`output`, `which`/`have`, `env_or`.

`maca dev` also emits `.maca/dev/{setup,activate}.ps1` when the config declares
`scoop.*`/`choco.*`/`winget.*` packages (see `dev.maca`): Windows hosts (no nix)
get a portable, project-local toolchain under `.maca\dev\`, and the flake
ignores those namespaces so Nix/Linux hosts are unaffected (`emit_windows_dev`
in `maca-backend-nix`).

Bindings (no `let` keyword): a bare lowercase `x = e` binds a **mutable**
variable; `const x = e`, `x = e as const`, or a **Capitalized** name binds a
**constant** (`is_const`). Reassigning a constant is a compile error
(`DiagKind::Immutable`), caught by the compiler and the LSP. A Capitalized
constant works but `maca lint` nudges toward explicit `const`. (Runtime: a bare
`x = e` that first introduces a name declares it; a later `x = e` reassigns.)

Type checker (`maca-core`): gradual unification with an `any` escape hatch for
unknown stdlib, strict on the acceptance diagnostics (`DiagKind`:
TypeMismatch / NonExhaustive / EffectInConfig / UnknownOption / Immutable /
UndefinedName). The last flags a direct call to a name defined nowhere (no
local/user-fn/import/builtin), so a typo surfaces as a clean diagnostic
instead of a broken-C link error; UI element tags and embedded intrinsics are
exempt via `maca_parser::is_backend_intrinsic`. `UndefinedName` also covers
**phantom keywords** (`return`/`let`/`type`/`null`, each answered with what
Maca does instead) and a **misspelt UFCS method on a known receiver**: the
method sets of `str` and `T[]` are closed (`maca_core::STR_METHODS` /
`LIST_METHODS`), so `s.slice(…)` is a diagnostic with a `did you mean` rather
than an `undefined reference` from the linker. `any` receivers stay gradual.
The two lists are executed, not trusted: `crates/driver/tests/method_sets.rs`
compiles and runs every name in them. Function
signatures generalize into `Scheme`s (lowercase names are type vars) and
instantiate per call; the C backend monomorphizes generics (one specialized fn
per concrete instantiation). Call **arity** and disagreeing **if/ternary
branch** types also surface as `TypeMismatch`.

**Colorblind async (no `async` keyword):** async-ness is an inferred effect,
not a function color. `spawn f(x)` runs `f` concurrently → `Future a`; `await
fut : a` suspends until it resolves; `sleep_ms(ms)` is a suspension point. `eff`
in `maca-core` adds the `ASYNC` effect for `await`/`spawn`/`sleep_ms` (so config
mode rejects them as impure). C backend lowers to `maca_spawn`/`maca_await`/
`maca_sleep_ms` (pthread-backed futures in `maca-runtime`'s `ASYNC_C`; an async
fn is an ordinary fn, with no ABI change). The playground interpreter runs it
eagerly.
`await`/`spawn` are unary-precedence prefix operators (`await a + await b` =
`(await a) + (await b)`). Example: `apps/examples/async.maca`.

Language surface beyond the original cheatsheet: operator overloading;
`while`/`break`/`continue` + reassignment; half-open integer ranges `lo..hi`
(counts lo … hi; an `int[]`; `for i in 1..n` lowers to a counting loop in C); `%`, `<<`, `>>` operators;
hex/binary/octal integer literals with `_` separators; list/string subscripting
`xs[i]` with lvalue assignment (`xs[i] = v`, `p.f = v`); functional record update
`base with { f = v }`; `len(x)`; recursive sum types (`Tree`, `List`) via boxed
payloads and **recursive record types** (`Expr { children: Expr[] }`, forward-
declared in C to break the struct/array definition cycle); a bracketless comma
list as an arrow-fn body (`f() -> int[] => 1, 2`);
C-keyword-safe identifiers (a Maca `double`/`new`/`class` compiles); a string
stdlib as UFCS methods on `str` (`split`→`str[]`, `trim`, `upper`/`lower`,
`contains`, `starts_with`/`ends_with`, `replace`, `substr`, `index_of`; byte
semantics, implemented in the runtime, C backend, and playground interpreter);
**closures / first-class functions** (a lambda `v => …` captures its enclosing
scope; lowered to a `maca_closure` = code pointer + heap env, one uniform ABI
for capturing and non-capturing lambdas; args/results box through `int64_t`,
str via `intptr_t`, float bit-preserved via `maca_box_f64`); **higher-order
parameters**, where a top-level function referenced by name is a function value
(wrapped in a `maca_closure` via a hoisted boxing thunk), and an unannotated
parameter that is *called* in the body is typed as a closure, so
`run_end(cs, i, is_alpha)` / `pred(cs.get(i))` work with no function-type
syntax (native + interpreter); a **list stdlib**
as UFCS methods on any `T[]` (`map`/`filter`/`reduce`/`fold` take closures typed
by the element; `sort`/`reverse`/`push`/`pop`/`contains`/`index_of`/`sum`/`min`/
`max`/`first`/`last`/`length`/`get`/`slice`; native + interpreter; see
`apps/examples/collections.maca`); plus the `str` scan primitives
`chars`/`length`/`at` and the char classes `is_whitespace`/`is_ascii_digit`/
`is_alpha` (what `modules/maca/lexer.maca` scans with);
and raw triple-quoted strings (`"""…"""`) with `import js`/`import css` foreign
blocks that let a `.maca` UI carry its own host glue and styles inline (see
`apps/playground/playground.maca`). Examples:
`apps/examples/{indexing,record_update,tree,sum_record,keywords,strings}.maca`.

**A function can be kept in a record field**, declared `(T, U) -> R`. The
parens are required, and this is the only place a function type is written
down, because a field is declared before anything calls it. A function *passed*
still needs no annotation: an unannotated parameter that is called in the body
is one. That is what makes a route table, a reducer, or a builder expressible
(`crates/driver/tests/programs/function_fields.maca`). The `rust` and `jvm`
emitters reject a function field with a clean diagnostic rather than emitting
something that will not compile.

**A generic can name its own element type**: `first(xs: a[]) -> a`,
`sort_by(xs: a[], key: (a) -> str) -> a[]`. A call binds `a` by looking *into*
the argument's type, not only at a parameter written as a bare variable, and
the body is lowered knowing what `a` turned out to be, so a local declared
`a[]` inside a generic gets the concrete element type instead of the fallback
array (`crates/driver/tests/programs/generics.maca`).

`is_tty()` answers whether stdout is a terminal, which is how `cli/style`
decides to emit colour.

**Strings:** `{` opens an interpolation, so a literal brace is `\{`/`\}` or
`{{`/`}}`. A `"…"` string may not span a line (write `\n`, or use `"""…"""`,
which spans lines and interpolates nothing). Without that rule a stray `"{"`
opened an interpolation the quote never closed and the file silently
mis-compiled. An interpolation may carry a **format spec**:
`{x:.2}`, `{x:>8}`, `{x:<8}`, `{x:^8}`, `{x:08}`, `{x:>10.3}`, which is
`[align][0][width][.precision]`, all parts optional. It is pure sugar,
desugared in the parser (`apply_fmt_spec`) to `x.fixed(n)` /
`str(x).pad_start(w, p)` / `pad_end` / `pad_center`, so every back end gets it
for free. A spec's `:` is *attached* and a ternary's is *spaced*, which is how
the lexer tells `{x:>8}` from `{c ? a : b}` (`Tok::FmtSpec`, `fmt_spec_here`):
the same attached-vs-spaced rule as `x?` vs `c ? x : y`. New primitives behind
it: `float.fixed(n) -> str` (int receiver widened) and `str.pad_center(w, p)`.

**Memory (Perceus RC, C backend).** Two invariants hold the string and list
handling together, and both are easy to break from inside `maca-runtime` or
`crates/backend_c/src/ownership.rs`.

*Every `maca_str`-returning runtime function returns a fresh block or a static
literal, never one of its arguments.* `maca_str_copy` exists for the cases that
would otherwise hand an argument back: `maca_replace` with nothing to replace,
`maca_split` on an empty separator, `maca_pad` already wide enough. A borrowed
return is a double free that only shows up under a load the tests don't reach.

*`xs = xs.push(v)` appends in place; `ys = xs.push(v)` copies.* A list is a
value, so the copy is the rule and assigning back to the same name is the one
case where the old value is unreachable the moment the new one exists. Written
as a copy it is quadratic. Eight thousand elements took half a second and left
every intermediate buffer behind. `ownership::appendable_names` decides this
per *function*, not per block, and excludes parameters (a parameter is a second
handle by construction, and appending in place reallocates a list the caller
still holds), `for` pattern variables, and anything aliased.
`emit_specialization` and
`emit_closure` save and restore it, or a specialization bypasses the analysis
entirely. Every one of those exclusions was a wrong answer before it was a rule;
`crates/driver/tests/programs/accumulate.maca` is one test per shape.

**A test that asserts only answers cannot detect this.** An answer is identical
whether the list was copied or appended to, and `assert_eq(str(xs.length()), …)`
marks the list aliased, which switches the optimisation off inside its own test.
Assert on `alloc_count()`/`reuse_count()`, read elements through interpolations,
and read them *after* enough rounds to force a reallocation. `MACA_POISON=1`
fills released blocks with `0xDD` so a use-after-free is a wrong answer rather
than a lucky one.

**Codegen note (C backend):** control-flow expressions (`if`/`match`/block)
work in value position via a `Sink` (Discard/Return/Assign) threaded through
`block`/`stmt_expr`/`match_stmt`; nullary enum-variant patterns lower to tag
tests (mirroring the checker's `is_variant`). `maca-runtime` holds the C
sources (`RUNTIME_H`/`RUNTIME_C`). The native driver compiles via `wsl nix
shell nixpkgs#zig -c zig cc … -target x86_64-linux-musl -static -s` when WSL is
present, else the host `cc`. Both paths cache the invariant runtime as a compiled
object (`build_cache::object`, keyed on runtime source + compiler + target), so a
*changed* program relinks against the cached `maca_runtime.o` instead of
recompiling the whole runtime. The zig path falls back to the original
all-in-one invocation if the cached-object link fails, so it can't regress.

**Codegen note (JS backend):** the same `Sink` (Discard/Return/Assign), for the
same reason. `if`, `match` and a block are expressions in Maca and *statements*
in JS, and lowering them as values means an IIFE, which is a function boundary:
`break`/`continue` cannot cross it (a SyntaxError, which is how
`maca build --target js apps/site/home.maca` came to emit an `app.js` node
would not parse), and a `var` written in a branch declared a fresh local rather
than assigning the enclosing one, which was a wrong answer with nothing to say
so. So `jstmt` lowers them as real statements wherever there is statement
context, including a binding (`x = if c { … }` assigns from inside the
branches); the IIFE remains only where the expression really is nested inside
another. Two consequences to keep: a `for` loop variable and every `match`
pattern binding are declared **`var`**, because a body that reassigns the name
emits `var x = …` and a lexical binding of that name in an enclosing block
makes it a SyntaxError; and the statement form's scrutinee temporary is `_s$`,
which no Maca identifier can spell. `node --check` over every app that builds
to JS is `crates/driver/tests/js_target.rs`, and what those constructs
*compute* is `crates/driver/tests/programs/js_control_flow.maca`, run natively
there and under node by `crates/backend_js/tests/control_flow_run.rs`. Still
divergent, and left: `int / int` truncates natively and does not in JS, and a
`break` buried inside a *nested* value expression (`total + (if c { break }
else { 1 })`) still needs the IIFE, so it is a build-time failure the
`node --check` guard catches rather than a silent one.

Grammar decisions worth knowing (in `parser.rs`): `no_brace` mode in control
headers so `for x in xs {` isn't a ctor; fn-def detected by lookahead for
`-> | { | =>` after `)`; call args separated by comma **or** juxtaposition (UI);
lambda-body assign (`v => age = int(v)`).
