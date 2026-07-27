# A Rust source backend for Maca (`--target rust`)

**Status (2026-07-27):** R1, R2, and R3 are implemented.

- **R1/R2 functional core** — `crates/backend_rust` emits Rust source and
  `maca build --target rust` builds a native binary. Covered: `main` (exits with
  the returned code), functions, recursion, `int`/`float`/`str`/`bool`, records →
  structs, **sums → enums including payload variants** (`Circle(int) | Rect(int,
  int)` → `enum { Circle(i64), Rect(i64, i64) }`; construction and match arms
  qualified `Enum::Variant`), `match`, lists → `Vec`, `if`/ternary/`while`/`for`,
  arithmetic/comparison, string interpolation → `format!`, and declare-then-
  reassign mutability. A local passed by value to a user call is `.clone()`d so
  Maca's value semantics survive Rust's move checker.
- **R3 crate dependencies** — `import rust "gpui::div"` → `use gpui::div;`;
  `[rust-dependencies]` in `maca.toml` generates a throwaway Cargo project and
  builds through `cargo` (a program with no external crates still takes the fast
  single-file `rustc` path). An import that names an **undeclared crate**, or a
  non-Rust foreign import (`import c`/`import py`) on the Rust target, is a **hard
  error** — no more silent drops.

Gated by `crates/backend_rust/tests/emit.rs` (hermetic), `rust_target_tests` in
the driver (import validation + manifest generation), and
`crates/driver/tests/rust_backend.rs` (compile + run via `rustc`, plus a
best-effort cargo-dependency build).

**Known gaps before gpui:** recursive sum types need `Box<T>` insertion (e.g.
`tree.maca`); and there is no `::`-path / associated-function surface syntax yet
(`Type::new()`), so *calling into* a crate beyond free functions and method
chains waits on R5's foreign-type ergonomics. The remaining phases (R4 escaping
closures, R5 foreign trait impls + the `&mut` rule, R6 gpql) are specified below.

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

## 2. Language gaps, ordered by how blocking they are

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

## 3. What you explicitly do NOT need

A borrow checker or lifetimes (§2.2 replaces it); trait *definitions* (only
impls of foreign traits); generic bounds / associated types / HRTBs in v1; proc
macros, `unsafe`, or a Rust parser (you emit Rust, `rustc` reads it).

## 4. Acceptance program

`examples/gpui_counter.maca` — foreign trait impl, `&mut` threading, an escaping
mutating closure, a generic foreign type, `impl IntoElement` return, a builder
chain, and RC'd record mutation, in one file. If the button increments, gpql is
mechanical from there.

## 5. Phases (one per commit, `cargo test` green before advancing)

| phase | deliverable | acceptance |
|---|---|---|
| **R1** ✅ | `crates/backend_rust`; `--target rust`; `main() -> int` → a Rust binary | `examples/hello.maca` runs via the Rust backend |
| **R2** ✅ (core) | records → structs, sums → enums, lists → `Vec`, `str` → `String` | functional examples run on the Rust backend |
| **R3** ✅ | `import rust`, `[rust-dependencies]` → generated `Cargo.toml` + `cargo` build, unresolved imports are hard errors, method-chain passthrough | a Maca program links a real crates.io crate |
| **R4** | closures → `move` closures with `Rc` capture | a callback-taking crate API works |
| **R5** | `Name : Trait = { … }` → `impl Trait for Name`; `&mut` non-escaping rule + diagnostic | `examples/gpui_counter.maca` compiles and runs |
| **R6** | `spawn`/`await` → gpui executors; `@derive`; `[rust-patch]` | gpql's backend layer ports over |

R1–R4 are language-agnostic and worth having regardless of gpui. R5 is the one
that actually unblocks gpql. §2.9 (RC output that satisfies the borrow checker)
is where the backend gets hard; `Rc<RefCell<T>>` everywhere is the escape hatch.
