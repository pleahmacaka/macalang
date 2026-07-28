# A Rust source backend for Maca (`--target rust`)

`--target rust` emits Rust source that `rustc` or `cargo` compiles. Rust is a
substrate for an ecosystem, the way the JVM backend is for Maven: the point is
that a crates.io library costs a line of configuration rather than a port.

- **The functional core** — `crates/backend_rust` emits Rust source and
  `maca build --target rust` builds a native binary. Covered: `main` (exits with
  the returned code), functions, recursion, `int`/`float`/`str`/`bool`, records →
  structs, **sums → enums including payload variants** (`Circle(int) | Rect(int,
  int)` → `enum { Circle(i64), Rect(i64, i64) }`; construction and match arms
  qualified `Enum::Variant`), `match`, lists → `Vec`, `if`/ternary/`while`/`for`,
  arithmetic/comparison, string interpolation → `format!`, and declare-then-
  reassign mutability. A local passed by value to a user call is `.clone()`d so
  Maca's value semantics survive Rust's move checker.
- **Crate dependencies** — `import rust "gpui::div"` → `use gpui::div;`;
  `[rust-dependencies]` in `maca.toml` generates a throwaway Cargo project and
  builds through `cargo` (a program with no external crates still takes the fast
  single-file `rustc` path). An import that names an **undeclared crate**, or a
  non-Rust foreign import (`import c`/`import py`) on the Rust target, is a **hard
  error** — no more silent drops.
- **Foreign-type calls without `::` surface syntax** — Maca has no `::`, so a
  call on a foreign (capitalized, non-local) type is its constructor
  (`Buffer()` → `Buffer::new()`) and `Type.assoc(a)` is an associated function
  (`Duration.from_secs(5)` → `Duration::from_secs(5)`); an instance receiver
  stays `recv.method(a)`. Integer literals in a foreign-call argument drop the
  `i64` suffix so `rustc` infers the parameter type (`u64`/`usize`/…). A real std
  type (`std::time::Duration`) compiles and runs end-to-end this way.

Gated by `crates/backend_rust/tests/emit.rs` (hermetic), `rust_target_tests` in
the driver (import validation + manifest generation), and
`crates/driver/tests/rust_backend.rs` (compile + run via `rustc`, plus a
best-effort cargo-dependency build).

- **Closures** — a lambda lowers to a `move` closure (`n => n + 1` →
  `move |n| { (n + 1) }`) with inferred parameter types, so it can escape into a
  foreign callback API and outlive its frame. Verified end-to-end passing a
  closure to a raw-block `fn apply(f: impl Fn(i64)->i64, …)`.
- **Foreign trait impls** — `Counter : Render = { render = (self, …) => … }` →
  `impl Render for Counter { fn render(&mut self, …) { … } }`. A leading `self`
  becomes `&mut self` (§2.1); the return type is inferred from the body
  (`()`/`bool`/`String`/`i64`), which covers event handlers and getters. The
  checker treats unknown lowercase calls as gradual foreign items once the
  program has an `import rust`, and an `import rust """…"""` raw block is emitted
  verbatim (a trait/impl/`fn` supplied inline). Verified compiling + running
  against a local stand-in trait.

- **Colorblind async** — `spawn e` becomes a `std::thread::spawn` handle and
  `await h` joins it, the same shape the C runtime gives it with pthreads. There
  is still no `async` keyword and no function colour.
- **Crate overrides** — `[rust-patch]` in `maca.toml` becomes Cargo's
  `[patch.crates-io]`, which is how a local checkout or a fork stands in for a
  published crate.

What stays gradual, deliberately: a foreign call's argument types beyond bare
integer literals. Maca does not read the crate's signatures, so a wrong type is
`rustc`'s error to report rather than a second type checker's — which is the
same bargain `import c` makes, and the reason a foreign call needs no
declaration in the first place.

---

## 0. The problem in one line

gpui-ce is a **Rust library**, and Maca's native path emits **C**. The gap is
not effort — it is that gpui's public API has no C ABI to bind to. A Rust source
backend (like `backend_jvm` for Java) closes it: Maca → Rust source → `cargo`.

## 1. Two paths

- **Path A — C shim** (`import c`, the `dbbrowser` model): a Rust `staticlib`
  re-exports the library behind opaque handles. No compiler work, but someone
  hand-writes ~1800 wrappers and every generic collapses to `void*`. Right for
  libpq (23 flat C functions); wrong for gpui (1026 fns, 54 traits, generics).
- **Path B — a Rust source backend** ← recommended, and what this crate is. The
  JVM backend's three tricks transfer verbatim:

  | JVM backend | Rust backend |
  |---|---|
  | `import java "pkg.Class"` | `import rust "gpui::div"` |
  | `Name : Iface = { m = () => … }` → class implementing an interface | → `impl Trait for Name` |
  | unknown capitalized type → foreign → gradual `any` | identical |

  Payoff isn't gpui-specific: it also unlocks `sqlx`, `keyring`, `ureq`,
  `lsp-types`, … — Maca as a crates.io citizen the way `--target jvm` did Maven.

## 2. How each Rust-shaped problem is handled

- **2.1 Foreign trait impl (biggest).** `impl Render for Workspace { fn render(&mut self, …) -> impl IntoElement }`. Generalize the JVM backend's `Name : Iface = { m = () => … }`: `self` first param → `&mut self`, multiple methods per block, generic trait arg (`EventEmitter ConnectEvent`). `TableDelegate` (6 methods) is the stress case.
- **2.2 `&mut` parameters (a restriction, not a borrow checker).** One rule: *a parameter whose type is a foreign type is a non-escaping borrow* — may be passed on, may not be stored/returned/captured. Emit `&mut T`, diagnose violations. Local, syntactic.
- **2.3 Escaping closures.** Closures already lower to `maca_closure`. When one escapes into a foreign call, emit a Rust `move` closure capturing an `Rc` of the env (so it's `'static`); RC-clone captured values.
- **2.4 Generic foreign types.** `Entity Workspace` → `Entity<Workspace>` from juxtaposition; skip trait bounds in v1 (gradual `any`, let `rustc` error).
- **2.5 `impl Trait` return.** Emit `-> AnyElement` + `.into_any_element()`.
- **2.6 Method chains on foreign values.** UFCS + JVM-style `obj.m(a)` passthrough already covers `div().flex().gap_2().child(x)` once the receiver is gradual.
- **2.7 Attributes / derives.** `@derive(Clone, Serialize)` passthrough; macros via `import rust` raw blocks (like `import js """…"""`).
- **2.8 Async → executor mapping.** `spawn` → `cx.background_spawn(async move …)`, `await` → `.await`; choose executor via annotation/intrinsic.
- **2.9 Memory model (the real risk).** Lower every record to `Rc<RefCell<T>>`, scalars to plain values. gpui is already `Rc`-shaped (`Entity<T>`), so idiomatic here.
- **2.10 Deps.** `maca.toml` `[rust-dependencies]` + `[rust-patch]` → the emitted `Cargo.toml`.

## 3. What the backend deliberately does not do

A borrow checker or lifetimes (§2.2 replaces it); trait *definitions* (only
impls of foreign traits); generic bounds / associated types / HRTBs in v1; proc
macros, `unsafe`, or a Rust parser (you emit Rust, `rustc` reads it).

## 4. The program that exercises all of it

`examples/gpui_counter.maca` — foreign trait impl, `&mut` threading, an escaping
mutating closure, a generic foreign type, `impl IntoElement` return, a builder
chain, and RC'd record mutation, in one file. If the button increments, gpql is
mechanical from there.

## 5. Where the ambition stops

Everything above is about reaching Rust's ecosystem, and that is the whole
remit. The backend does not try to be a second Rust: no borrow checker (§2.2's
non-escaping rule stands in for one), no lifetimes, no trait definitions, no
generic bounds. It emits Rust and lets `rustc` be the authority on Rust.

The one place that authority costs something is §2.9, the memory model:
lowering every record to `Rc<RefCell<T>>` is what makes Maca's value semantics
survive the borrow checker, and it is a real cost in a hot loop. The native C
target exists for that case, and moving between the two is a flag.
