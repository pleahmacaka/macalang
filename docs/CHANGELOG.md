# Changelog

Newest first. Versions are bare semver; the tag is the version.

## 0.6.3

- `modules/i18n`: one word looked up by key in whichever tongue was picked,
  with prefix locale matching and a readable fallback for missing keys. Quipu
  speaks through it: every label has an English and a Korean word, switched
  from Settings > Language.
- Quipu draws its own title bar: the app name, the menus, a drag stretch and
  the window buttons in one row, handed to the window system through a
  `zone=` attribute lowering to gpui's `WindowControlArea`; the OS title bar
  is gone. File > Quit and Ctrl+Q leave through the host.

## 0.6.2

- Quipu grows the furniture: a File / Edit / Help menu bar with dropdowns,
  tabs that preview on a click and pin on an edit (each with a close cross,
  unsaved buffers stashed and restored across switches), and a Settings pane
  (Edit > Settings) with a ligature toggle and a block-or-bar cursor.
- JetBrains Mono Nerd Font rides inside the binary, so the editor looks the
  same on a desktop that never installed it; the ligature toggle swaps to the
  NL cut. `assets/` beside a program is carried into its Cargo project for
  `include_bytes!`.
- The cursor line keeps its syntax colours, with the block cut into the spans
  rather than flattening them.
- The panel computes its text when a tab or file is clicked, never in a
  render; the toolchain no longer runs once a frame under the pointer.

## 0.6.1

- Quipu dresses like an editor: a hairline border between every region
  (`edge=` lowers to gpui's `.border_color`), an EXPLORER header, a real tab
  bar, a right-aligned gutter whose active line is lit, a full-width active
  line, the panel's twelve views as rounded pills, and a segmented status
  bar. The buffer text grew a size, and the main row clips so the footer
  stays on screen.

## 0.6.0

- Selection: a shifted arrow drops an anchor and stretches a shaded selection,
  Ctrl+C and Ctrl+X carry it to the system clipboard, Ctrl+V pastes over it
  (however many lines the clipboard brought), typing replaces it, and Home and
  End jump the line. The stretch logic is model functions a suite runs.
- The window keeps its keyboard: clicking a row used to steal focus and every
  key after it went nowhere; the host refocuses itself each frame.
- Everything clickable says so: a pointer cursor and a hover shade on file
  rows, panel tabs and quick-menu hits, via a `hover_bg=` attribute lowering
  to gpui's `.hover`.

## 0.5.9

- Quipu edits: the cursor is a block you steer with the arrows, typing inserts
  at it, Enter splits, Backspace deletes and joins, Tab indents, and Ctrl+S
  writes the file back; the tab and the status bar say when the buffer is
  unsaved. All of it is `Ide -> Ide` functions a suite runs without a window.
- Quipu highlights: a small scanner cuts each line into coloured spans: keywords,
  strings, numbers, comments, calls, capitalised types, under a
  One-Dark-ish palette, with the open file, the picked hit and the active
  panel tab marked by their backgrounds, and a status bar underneath.

## 0.5.8

- `await` on a foreign future blocks on the future itself, through a
  dependency-free parked waker in the runtime; `await` on a `spawn` still
  joins the thread it started. `sqlx`- and `reqwest`-shaped crates are
  reachable without a shim around every call.
- `click=` applies through `maca_apply`, a name any host's raw block defines,
  rather than through a function named after one editor.
- A raw block is carried verbatim: `"	"` written for the foreign language
  reaches it as the two characters written, instead of being cooked by the
  string lexer's escape order.
- The one gpui example is gated on every run of the examples suite: it must
  emit Rust that `rustc` reads whole, unresolved gpui imports aside, so it can
  no longer rot between releases.

## 0.5.7

- Quipu is an editor to use, not to look at: clicking a file opens it,
  clicking a tab switches the panel, the columns scroll, and Ctrl+P opens a
  quick menu that filters as you type and opens on Enter.
- `click=` on an element lowers to gpui's `.on_click`, closing over clones of
  what it reads and applying its `Ide -> Ide` function through the window's
  entity. A function-valued attribute never reaches a text target.
- The Rust back end grew up about mutation and closures: a parameter the body
  writes is declared `mut`, an assignment's left side is a place and never a
  clone, and a lambda stays where it was written because Rust has closures of
  its own; nothing is lifted to a name nobody wrote.
- A written string handed to a foreign method is lent as the `&str` it is, so
  `div().id("inc")` satisfies gpui's own conversions.
- `Element` erases to `gpui::AnyElement` at every element literal, so a row
  with a handler and a row without are the same type to a list.
- The unlowered-expression fallback in the Rust back end is a `compile_error!`
  rather than a silent `0`.
- `apps/examples/gpui_counter.maca` compiles again, is written in gpui's own
  listener idiom, and is gated by the editor workflow.
- Main CI carries rustc, so the rust suites' compile gates run rather than
  passing vacuously.

## 0.5.6

- The editor's window no longer opens frozen: the workspace scan skips VCS and
  build trees (`.git` alone cost twenty seconds), and the workspace is read
  once rather than twice.
- The Rust backend braces a `then` branch that is itself an `if`; bare, it
  emitted `if a if b`, which Rust reads as a missing block.

## 0.5.5

- Quipu launches without a console window: the emitted crate opens with
  `#![windows_subsystem = "windows"]`, hoisted from the raw boot block to the
  crate's first line.
- A Start Menu launch no longer dies walking a home directory: a folder with
  no `maca.toml` is not a workspace, so the editor opens on it without
  crawling it.
- The editor opens on the first file of the workspace, and the view is built
  from real elements: one per file in the tree, one per line in the buffer. A
  `xs.map(x => element)` child now lowers to gpui's `.children(...)`, built
  inside the render pass.

## 0.5.4

* **The installer installs the editor too, and Windows search finds it.** Where a Quipu
  build exists for the desktop the installer runs on, it rides along into the same bin
  directory, and on Windows a Start Menu shortcut is written through the shell object
  powershell carries, which is what makes typing `qui` into the search box find Quipu.
  A desktop with no editor build, or a download that fails, is said out loud and never
  fails the toolchain underneath it. Verified by running the installer on a Windows
  desktop and reading the shortcut back.

## 0.5.3

* **The installer installs on Windows, verified by running it there.** Two faults, both
  found by executing the shipped binary rather than reading it: `uname` under a git-bash
  answers `MINGW64_NT`, which is still Windows, so the platform asks `OS` first; and the
  extraction now names `System32	ar.exe` by full path, because the GNU tar a git-bash
  puts first on PATH cannot read zip and reads the colon of a drive-letter path as a
  remote host. Extraction runs inside the directory on bare names, so no path carries a
  colon. On a machine with no C compiler the installer still installs and says plainly
  what `maca run` will need; `maca check` works as installed.

## 0.5.2

* **Quipu is verified on a desktop, not just a linker.** The 0.5.1 window opened
  transparent and untitled: nothing painted the root, so the desktop showed through the
  editor. The gpui lowering gained `bg=` and `fg=` attributes (`.bg(gpui::rgb(...))` /
  `.text_color(...)`), the root paints its paper once and the colours inherit, and the
  window says Quipu in its title bar. Confirmed by launching the shipped Windows binary
  and reading its own pixels: three columns, dark paper, the tab row and the file count.

## 0.5.1

* **Quipu opens its window.** 0.5.0's editor binaries compiled the view against gpui but
  `main` printed the workspace state and exited; a raw Rust block now boots
  `gpui::Application`, opens a centered 1200x800 window and hands it `Shell` as the root
  view, so the binary on this page is an editor and not a report. An ordinary comment also
  settled its shape: `//` for one line, `/* ... */` past that, with the doc form `/** ... */`
  untouched, and MacaDoc, the spec index and the editor's reader all read the block blurb.

## 0.5.0

* **Joining is `+`, and `++` is only the bump.** `"a" + b` and `xs + ys` concatenate;
  either side already being text or a list is enough, and the other side is unified toward
  it, which is what types a lambda's bare parameter. A number beside a string stays a
  mistake, written `str(n)` or `"{n}"`, so `+` never silently stringifies the way the old
  `++` did. Binary `++` is refused by a diagnostic naming `+`, and the statements `n++` and
  `++n` are the `++` that remains, each `n = n + 1` written small. Every back end and the
  playground interpreter carry the same rule, and the embedded target refuses a join by
  name, because joining needs an allocator it does not have.

* **A doc comment is a `/** … */` block, and three slashes are just a comment.** All 1,364
  `///` lines across 115 files became blocks; MacaDoc, the spec index, the highlighter and
  the editor's reader all read the block form, and `///` now means what TypeScript would
  take it for: an ordinary comment that attaches nothing.

* **The editor ships on three desktops.** What it took is the measure of what was broken
  underneath: a Maca `int` is now C `long long`, because Windows makes `long` 32 bits and
  every pointer handle was being truncated on that desktop; a library is linked for a real
  `#include` line, never for the string literal of it the compiler carries; a captured
  command survives `cmd`'s quote stripping; `where` answers for `sh` where there is no sh;
  and the binary cargo made is copied out by bytes on Windows, with `cp` kept on POSIX
  because the execute bit lives there.

* **Quipu knows Ctrl+P, its keymap and its own age.** The quick menu reaches every file,
  panel and command, prefix matches first; the keymap is data the editor rewrites, refusing
  a taken chord by naming what holds it; the about page asks the toolchain for its version
  and the releases for the latest, comparing by number so 0.10.0 is later than 0.9.0.

## 0.4.0

* **`modules/chaski` is the client half of HTTP.** `modules/http` serves; chaski asks.
  A base address, headers that ride on every request, a timeout, and retries that fire on a
  transport failure or a 5xx and never on a 4xx, because asking twice does not change an
  answer. The reply is read out of what curl wrote, so TLS is not a library this tree
  maintains: `Expect:` is cleared so a large body cannot turn one header block into two,
  redirects are not followed, and `HTTP/2 200` and `HTTP/1.1 200 OK` both put the number in
  the same place. Query values are escaped a byte at a time, which is what UTF-8 needs and
  what `chars()` already gives. Named for the relay runners of the Inca roads, which is the
  road `modules/tambo`, the waystation, is already named after.

* **The BEAM is a target, and it is named `elixir`.** `modules/maca/emit_elixir.maca`,
  481 lines. The spec had said there would be no BEAM back end because it would be the
  first one added for elegance rather than reach; what changed is that a program holding
  thousands of slow network calls open at once wants a process each rather than a thread
  each, and no other target gives that. Most of the lowering is not translation, because
  both languages bind once and match on shape: a record is a struct, a nullary variant is
  an atom, `match` is `case`. What differs is spelled out instead of faked (`++` on text
  is `<>`, `int / int` is `div`, a `#` in a literal is escaped so Elixir does not invent an
  interpolation), and what the BEAM has no shape for is refused by name: `while`, because
  nothing there rebinds, and `return`, because a function's last expression is its value.
  The gate is `apps/examples/elixir_capstone.maca` built twice and run twice: the two back
  ends print the same text and leave the same exit status, or there is no language.

* **An import is named by its string, and no import carries a language word.** `import c
  "sqlite3.h"` is now `import "sqlite3.h"`, which is the rule assets already followed. The
  string classifies itself: an extension (`.h` `.c` `.rs` `.js` `.css` `.nix` `.py` `.java`
  `.ex`), then a `::` path or a bare crate name for Rust, then a dotted class for the JVM. A
  raw block is named the same way, `import "scanner.c" """…"""`, which is what removed the
  last reason to keep the word: the spec had said a block "keeps its word, because a block of
  source has no name to read a kind off", and now it has one. The old form is a parse error
  pointing at the string the word displaced, never a quiet no-op.

* **A workspace may hold a workspace.** A `[workspace]` inside a `[workspace]` was refused
  outright; now every workspace on the manifest chain answers for its own members, and the
  chain still runs to the outermost manifest so `[format]` and `[lint]` inherit as before. A
  monorepo can therefore live inside a monorepo, which is what lets `modules/ai/openai` be
  used on its own without leaving the tree that builds it.

* **The Rust workspace is deleted.** `crates/` held the frozen stage-0 compiler; every
  guarantee it stood for is now a `.maca` suite, and `bootstrap/maca.c` is what produces
  the first binary on a machine with neither Maca nor Rust. The `cargo test` and
  `fmt + clippy` CI jobs went with it, and the release is cut by `zig cc` from that seed
  for all five targets from one Linux runner. Getting there needed the compiler to be
  honest about things it had been quiet about: a formatter that rewrote `(a * b).sum()`
  into a different program, a `return` nobody type-checked, a nested definition whose
  writes went to a global, a handler rendered dead into markup, a self-build that
  overflowed the default 8 MB stack with no diagnostic. The emitted C also learned to
  run where there is no `fork`, which is what lets the Windows binary come from the seed.

* **The LLVM back end is deleted**, `crates/backend_llvm`, and nothing was ported in its place. The
  C back end already had the vector type and was declaring the kernel `extern` for the IR file to
  define; now it defines it, because `a * b` on an `ext_vector_type` is the operator and `.sum()` is
  a loop over the lanes, which is exactly what `llvm.vector.reduce.fadd` with a zero start means.
  Built with `-mavx2`, `dot8` disassembles to the instructions the IR path produced and in the same
  order, `vmulps %ymm1,%ymm0,%ymm0` and a sequential `vaddss` reduction. The kernel keeps external
  linkage: a `static` one is inlined into a caller that splats constants, and the vectors are folded
  away with it. 342 lines out, fifteen in.
* **`spec`, `profile` and `dev`** are the driver commands `apps/maca1` gained this round.
* **The JVM back end is written in Maca**, `modules/maca/emit_jvm.maca`: a class per module, a
  static method per function, records as classes and sums as enums, and a **refusal channel** beside
  the emitter, so a construct it cannot lower is named rather than emitted as Java that pretends.
  Eight refusals are listed by name today, `apps/mcmod` among them. What it does emit was compiled
  by `javac` 21 and run, and answers what the Maca source computes. With it, every back end stage-0
  carries has a Maca twin except the LLVM path, which exists only for the SIMD span.
* **`crates/wasm` is not a back end**, which is worth writing down because the name says otherwise:
  it is the compiler itself built for the browser playground. Its Maca answer is building
  `apps/maca1` for a wasm target, not writing an emitter, so no emitter was written.
* **`maca -m` runs a function in a module** through the same search roots an `import` walks, which
  is the last of the driver's small commands. `[scripts]` aliases from `maca.toml` come with it.
* **Config mode is read by the compiler written in Maca.** `emit_nix.maca` was ported with nothing
  feeding it: the parser could not read a dotted binding, so `apps/examples/system.maca` was
  eighteen parse errors. A top-level `a.b.c = value` is one binding whose name is the whole path,
  which is exactly what the Nix emitter splits on; bare braces are a value, so `{` is an atom; a
  record declaration is told from a record literal by whether its fields carry a `:` or an `=`; and
  `alias x = y` renames rather than declaring, which had been leaving a stray option in the emitted
  module. One lexer line came with it: an attached `-` belongs to the name, so `noto-fonts` is a
  package and not a subtraction, which is what stage-0 and the handbook both say. `system.maca`
  goes from 18 errors to none, and the Nix it emits carries every line the Rust back end's own test
  asserts.
* **The embedded back end is written in Maca**, `modules/maca/emit_embedded.maca`, the freestanding
  C target that `apps/blink` exists for. Four of the five back ends stage-0 carries now have a Maca
  twin: C, Rust, JavaScript, Nix and embedded, leaving the JVM, wasm and LLVM paths.
* **A task is a thread on the Rust target too.** `spawn` and `await` landed in the C back end and
  nowhere else, so the same program compiled one way and not the other, which is the divergence the
  differential gate exists to catch. Rust already owns the primitive: `spawn f(x)` is
  `std::thread::spawn(move || f(x))` and `await` is `join().unwrap()`, with a future the one value
  the emitter must not copy, because a join handle is used once. The emitted Rust compiles under
  `rustc` and answers what the C does.
* **The Nix back end is written in Maca.** `modules/maca/emit_nix.maca` is `crates/backend_nix`
  ported: the `{ config, pkgs, lib, ... }:` module shape, the split between what the system owns and
  what rides under home-manager, the path table that turns `system.packages` into
  `environment.systemPackages`, the `enable = true` a service implies by being written down, and a
  value printer over the tree. `maca build --target nix` reaches it. The parser does not read
  config-mode surface syntax yet, so nothing feeds it from a real file; the suite drives it from a
  tree built by hand, which is honest about what does and does not work.
* **MQTT is a builtin with a body again.** `mqtt_connect` and `mqtt_broker_run` were emitted as
  calls to symbols nothing defined, so both programs compiled and then failed to link. Stage-0
  keeps them in a runtime it compiles beside the output; stage-1 emits one translation unit, so
  they live in its preamble now, at the width a 64-bit `int` requires. `sqlite_open` and `py_call`
  are still open, and deliberately not faked: they need `-lsqlite3` and `-lpython3`, and nothing
  passes a link flag yet.
* **`maca build` with no path reads the manifest.** `maca.toml` names the entry through `[[bin]]`,
  which is what the Rust driver has always done and stage-1 could not: one binary is the default,
  more than one without `--bin` is an error that names them, and no manifest and no path is an error
  that says so.
* **A Maca `int` is 64 bits, as it always was in stage-0.** Stage-1 emitted it as a C `int`, which
  is not a narrowing but a divergence between the two compilers: `state * 1103515245` wrapped at
  2^32, so `modules/bench` answered `-51042` where it should answer `32606`, and the compiler itself
  wrote `2147483648` into the C it emits as `-2147483648`, because `int(str)` was `atoi`. A list
  cell, a map value, a guessed local and every emitted signature carry the full width now, and the
  foreign declarations the preamble makes (`http_listen`, `http_fetch`, `http_accept_loop`) were
  widened with them, because a prototype that disagrees with its definition is a C error rather than
  a rounding. `main` still answers a C `int` where it declares one, and a `main() -> Element`
  answers the text it renders. Thirty test expectations that spelled the old width were rewritten,
  which is what makes this one change rather than thirty.
* **Generics are monomorphised, the way stage-0 does it.** A type variable was erased before any
  back end saw it, so `id(x: a) -> a` was emitted `int id(int x)` and every call handed C the wrong
  type. A pass over the annotated module now clones a generic function once per concrete argument
  tuple, mangles the name, rewrites the calls, and drops the un-specialised original, so no back end
  ever meets a type variable and all three get generics at once.
* **An import that renames a definition tells the files that import it.** The driver renames a
  function when two packages define one name and then forgot the rename, so a file importing the
  renamed definition still called the old name and bound to whichever definition won. Three of the
  four files stage-1 refused were that, and the fourth went with them.
* **The C prelude answers what the frozen runtime answers.** Four divergences, found by running the
  standard library's own suites rather than by reading: `read_file` had no `S_ISREG` guard, so
  reading a directory succeeded, reported a nonsense length and **segfaulted**; `remove_dir` was
  `rmdir`, which fails on a non-empty directory, so a fixture survived between runs and seven
  assertions in `modules/std/tests/fs.maca` failed on counts that grew; `keys()` came back in
  insertion order where stage-0 sorts, which is what "iteration order is deterministic" rests on;
  and the epoch-millisecond builtins were 32-bit, so `modified_ms` was negative. `{x:>6}`, `{x:.1}`
  and `{x:^8}` are desugared in the parser as stage-0 desugars them, instead of the format spec
  being parsed and thrown away.
* **A lambda handed to a function is a function, not a null pointer.** `emit_expr` had no
  `ELambda` case, so a lambda anywhere except inlined into `map`/`filter` fell through to the `_`
  arm and emitted `0`: `any_of(xs, v => v > 8)` compiled without a word of complaint and called
  through a null function pointer. That is why `modules/std/tests/list.maca`, the most-used module
  in the repository, **segfaulted**. A lambda that reads nothing from around it is now lifted to a
  function of its own, which every back end already knows how to pass by name, and the module is
  annotated a second time so the checker types the lifted function from the call site rather than
  the pass having to guess. A lambda that does capture is left where it is, because a bare function
  has nowhere to put an environment; that one waits for a closure. `map` keeps its own lambda
  because it reads the body, and so does an attribute, which holds a handler rather than a name.
  Module suites run by stage-1: 18 green, now 19, and `modules/bench` moved from a segfault to an
  ordinary failing assertion.
* **The annotate pass hands every child the env it is written against.** It handed each child the
  *parent's* env, so every binder a construct owns (a match arm's payload, a comma pattern's
  cells, a lambda's parameter) was an unknown name while its own body was annotated, typed `any`,
  and reached C as `maca_int_to_str(<a pointer>)`. The checker already had `bound_arm`; the annotate
  pass just never called it.
* **A branch that raises borrows the other branch's type.** `c ? fail "x" : <a record>` put an `int`
  beside a struct, because `maca_fail` returns `int`; stage-0 threads an expected type and emits
  `(maca_fail(msg), <zero>)`, which stage-1 can now do by reading the sibling arm's type. A `match`
  run for its effect is emitted as C `if`/`else` statements, the escape `if` already had, so its
  arms need not agree once they stop being ternary arms.
* **`x?`, `a / b` over paths, and a block guessed from the wrong end.** The postfix `?` is a token
  the scanner tells apart by the whitespace before it; `/` between two strings is the path join
  `maca_path_join`, not a division the checker rejects; and a `{` opens a block by the backward
  line-head scan stage-0 uses rather than a forward guess. Sized numbers (`f32`, `i32`, `u8`) reach
  C as the widths they name.
* **`spawn` and `await`, with no `async` keyword, in the compiler written in Maca.** `spawn f(x)`
  yields a future and `await` joins it, lowered exactly as stage-0 does it: `maca_spawn` over a
  POSIX thread, one arity for one argument and one for two, with the future runtime in the C
  preamble beside `maca_cat` because stage-1 emits a single self-contained translation unit. Both
  words bind at unary precedence, which is where stage-0 parses them, so `await a + b` is
  `(await a) + b`. `xs.parallel(f)` is the same type rule as `xs.map(f)` and goes through the loop
  `map` already emits. `apps/examples/async.maca` now prints `20 + 40 = 60` in 52ms wall against two
  50ms sleeps, so the two really do overlap.
* **A branch run for its effect need not agree, and a local shadows a function of its name.** Two
  checker bugs, eight files. The statement-position escape that lets `if c { … }` be run for its
  effect was one level deep and `if`-only, so a nested `if`, an `else if` (child 2 is itself an
  `if`) and any `match` fell back to joining their branches: `{ placed = true }` answers `bool` and
  `{ at = at + 1 }` answers `int`, hence 4 files reading `expected bool, found int`. The escape now
  walks the whole tree it is skipping. Separately `call_type` resolved a name against the module's
  functions before its own locals, so `span(r, name, body)` calling its `body` parameter got the
  four-argument `body` from another package. Locals win, which is the rule `tag_wins` already wrote
  down; a local that is not callable still leaves the function alone.
* **A pattern matches what it says.** A record-typed payload was stored in a `long` slot, so
  `At(p) => p.x` read a struct through an integer; the slot and the constructor take the declared
  type now. Records and tagged sums are emitted in dependency order, so a field holding another
  record by value never names a type C has not seen. And a lowercase name in a pattern is a
  **binder**, not a value to compare against, which is what `match n { x if x < 0 => … }` needs:
  Maca capitalises variants (`docs/SPEC.md`), so the case of the first letter is the whole rule, and
  it holds on an unannotated tree as well.
* **The gaps a repository-wide sweep found.** `a + b` on a record is `add(a, b)`, which the spec
  asked for and only stage-0 did. `clamp` and `gcd` are lowered like `min` and `max` instead of
  reaching C as undeclared names. A method on a user record goes straight to the function of that
  name rather than being claimed by a list or map lowering. A record mapped back into a list is
  boxed rather than cast to `long`. And a lowercase top-level binding is module state a body may
  assign to, so it is emitted without `const`; a Capitalized one is a constant and keeps it, which
  is `docs/SPEC.md` on bindings, spelled in C.
* **A branch that ends in an assignment keeps it.** `if c { ys = ys.push(v) }` compiled to
  `maca_list_cat(ys, …)` with the `ys =` gone: the append ran and its answer was thrown away.
  `body_expr` took the last statement's *value* as the block's value, which is right for an
  expression and drops the target of an assignment, and `branch_value` was a second copy of the same
  four lines with the same hole. One function now, and a block whose last statement is a binding
  keeps that statement and answers with the name it set. This is the worst kind of bug the sweep can
  find, because the C compiled: nothing said the line had gone missing. Beside it, an `if` with no
  `else` in value position emitted a bare `?` for the branch it does not have, which is not C; it is
  a zero of the branch's own type now.
* **A method no receiver answers to is the function of that name.** `command("lint", …).rest("path",
  …)` is `rest(command(…), …)`, which the C back end already emitted correctly and the checker typed
  as `any`, so `Spec` was declared `static int` and every use of it was a type error. A method whose
  name is not a method and *is* a declared function is checked as the call it is, receiver first, so
  it gets that function's return type and its arguments are checked against what the function
  declared. A constant at file scope is annotated now as well, so it is emitted as the type its
  value has rather than the type its shape suggested.
* **A typed local binding holds its value to the type it declares.** `got: str = f(xs)` bound `got`
  to `str` and threw the value's type away, so an unannotated parameter that is called kept an
  unsolved return and reached C as `int (*f)(MacaList)` inside a function that assigns the result to
  a `const char*`. Unifying the declared type with the value fixes the signature and reports the
  clash the annotation was there to catch: `n: str = 1` is one error rather than none.
* **A map answers with the type it was declared over.** `Map str int` read a value back through a
  `(const char*)` cast, because the lowering knew the runtime call and not the map's value type. The
  cast follows the declaration now, `.get(k, fallback)` is a call of its own rather than a silently
  dropped second argument, and `.remove`/`.values` exist in C as they already did in Rust. The
  checker types all of them from the declaration, so `.keys()` is a list of the key type instead of
  `any`. `element(tag, …)` names its tag with its first argument, which is the one tag call the
  repository writes that is not spelled like the element it renders.
* **A named argument is an attribute, and a tag call is the HTML it renders.**
  `docs/SPEC.md` writes UI as functions (`div(class=…, …children)`), and stage-1 read
  the `=` as a token no production wanted: 17 files stopped at ``no expression starts
  at `=` ``. `parse_one_arg` now reads a name run closed by `=` as one `EAttr`
  argument. The run is hyphenated because the platform spells `aria-label` that way,
  and it names an attribute only when `=` closes it, so `f(a - b, c)` is still a
  subtraction. The checker types a tag call nobody declared as `str` and lowers it in
  the annotate pass, which is the one place that knows what the module declares:
  `div(class="wrap", body)` becomes `maca_element("div", "" ++ maca_attr("class",
  "wrap"), "" ++ body)`. Lowering there is what keeps `code(…)` a call when the module
  declares `code` and `input(…)` a read because the prelude owns the name, and it is
  why neither back end carries a tag list. The tag is a value rather than a static
  choice, so the void-element rule lives once inside `maca_element` instead of twice,
  and both back ends carry the three helpers, C in its preamble and Rust in a new one.
  Repo-wide, stage-1 accepts 137 files and now 147, and 107 emitted 109 that compile;
  `Map` is the largest class left, at 17.
* **The compiler needs a C compiler and nothing else.** `apps/maca1` follows its own
  `import` graph and builds an executable, so the loop that produces the compiler no
  longer passes through Rust. `unit_of` walks the graph depth first, resolving a path
  from the importing file's own directory (the written path, then `modules/`, then
  `src/`, then the same three one directory up, which is the order `docs/LAYOUT.md`
  sets out), and splices an imported file's *tokens* ahead of the importer's, so each
  file is read and lexed exactly once and a definition precedes its use. Probing a
  candidate needs no new builtin, because `read_file` already answers `""` for a path
  that does not open. Nine files resolve from `apps/maca1/main.maca` into 136152 bytes
  of C, byte-identical to a concatenation of those nine in the order the walk visits
  them, which is what pins both the set and the order. It is also four times faster
  than feeding the 96 KB concatenation, 3.0s against 13.7s, because lexing per file
  dodges the cost of `acc ++ [t]` growing one list to 27000 tokens.
* **`maca build in.maca -o bin` produces an executable**, through `exec(cmd, args)`.
  That builtin already existed in stage-0 and was missing from `docs/SPEC.md`; what
  was missing in Maca was its lowering, which `emit_c.maca` now has as `fork` +
  `execvp` + `waitpid`, mirroring stage-0's runtime rather than reaching for
  `system()`. A shell in one stage and not the other is exactly the divergence the
  differential gate exists to catch, and there being no shell is also why an argument
  holding a space or a `$` stays one argument.
* `docs/BOOTSTRAP.md` claimed `cmp maca1 maca2` closes the bootstrap. It cannot:
  `maca1` comes from stage-0's C back end through one C compiler and `maca2` from
  `emit_c.maca` through another, so they are different programs by construction. What
  does hold is the round after, `maca2` against `maca3`, and even there `cc` records
  the name of the file it was handed in the symbol table, so `-o maca2` and `-o maca3`
  differ in exactly one byte, the digit. Building both as `maca` in different
  directories makes them equal without stripping, and three rounds deep they stay
  equal at 257016 bytes.
* **The checker accepts the compiler's own source**, 220 errors down to none, and it is
  stronger afterwards rather than more permissive. All 220 were one bug with a lying
  message. `parse_one_lit_field` builds a field as a binary `=`, `binop_type` had no `=`
  case, so every `field = value` in the compiler fell through to `arith_type`, and
  `Ty { kind = k, name = "", ... }` alone was six of them. The complaint printed only the
  left type, which is why 220 of them read `any is not a number` and grouping by message
  gave thirteen useless buckets: the count per class is what said this was one bug, not
  thirty. Typing `=` as its value closes all 220 on its own, but on its own it would also
  make `Point { x = "s" }` pass silently, because `ERecord` and `EWith` used `walk_args`,
  which types a field's value and throws the type away. No record literal had ever been
  checked against its declared field types. `check_fields` does that now, and takes the
  record name for a `with` from the resolved type of its base, so `p with { x = "s" }` is
  caught too. 220 fake errors is why nobody noticed a whole construct was unchecked.
  `check_module` reported a count and no messages, so what any of the 220 said was
  invisible; `apps/maca1` now prints them the way it already printed scan and parse
  errors.
* **The bootstrap fixed point closes.** The compiler, compiled by itself, emits what
  it emitted before: `whole1.c`, `whole2.c` and `whole3.c` are one 129731-byte file,
  and stage-1, stage-2 and stage-3 print the same 244 lines and exit alike. Four
  one-line causes stood in the way, and the last one is the reason both halves are
  asserted: `cmp` was already silent while stage-1 and stage-2 were still different
  programs, stable at the wrong value. `plain_braces` collapsed `\{` but not `{{`, so
  a literal without an interpolation kept its doubling, which is every string in the C
  preamble, and a stray brace then made `opens_record_lit` fire and nest without end
  until stage-2 walked off the token array. `interp_step` read `}}` as an escaped
  brace even inside an interpolation, where the first one has to close it.
  `index_of` and `contains` on a `str[]` lowered to a helper that compares cells, so
  every `env.fns.index_of(name)` missed, stage-2's checker knew no names, and the
  untyped tree that came out of it was 1155 C errors. And `==` between two strings
  chose `strcmp` from the syntax rather than the type, so the very predicates that
  recognise `{{` compared pointers. `stage2_emits_the_c_it_was_built_from` runs the
  whole ladder and asserts both halves.
* The compiler's own source compiles as C, the last eight errors down to none, and
  `the_compilers_own_source_compiles_as_c` keeps it that way. The eight were four
  things. A `main` that takes arguments emits `int maca_main(MacaList)` beside a real
  `int main(int argc, char** argv)` that builds the list and calls it, while a `main`
  taking nothing still emits exactly `int main()`. `int(x)` lowers the way `str(x)`
  already did, a no-op on an int and `atoi` otherwise. `read_file` and `write_file`
  have helpers, each returning a fresh block. And the demo bound `bfn` and `ar` twice
  in one body, which Maca allows and C does not. The emitted C links, runs the demo it
  was built from, and compiles a small program of its own.
* Lexing a long source no longer exhausts the stack. The lexer was a continuation
  chain, `scan` to `step` to `lex_word` and back to `scan` for the next token, so depth
  grew with the input: 91 KB of compiler source is about 100k frames against an 8 MB
  stack. The emitted C is a genuine tail call and clang turns it into a jump, which is
  why the compiler built by `zig cc` survived and the one built by gcc took SIGSEGV on
  the same input, so the depth had to come out of the Maca instead of being wished onto
  the C compiler. `scan` halves a character range rather than walking it, and every
  `lex_*` returns with the cursor moved rather than calling `scan` itself. Depth is
  logarithmic, 34 frames for 91 KB, with the optimisation or without it, and token and
  error order are unchanged because the left half finishes before the right begins.
  `.chars()` types as `str[]` rather than `any`, which the restructure exposed: a local
  bound from it had `length()` lowered to `strlen` on a list.
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

* The type system is Maca. `modules/maca/ty.maca` is `modules/maca/ty.maca`
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
