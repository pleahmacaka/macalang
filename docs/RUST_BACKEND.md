# A Rust source backend for Maca (`--target rust`)

`--target rust` emits Rust source that `rustc` or `cargo` compiles.

Rust is a substrate for an ecosystem, the way the JVM backend is for Maven: the
point is that a crates.io library costs a line of configuration rather than a
port. That matters because a Rust library's public API has no C ABI to bind to,
so `import c` cannot reach it. A C shim can be hand-written, which is what
`dbbrowser` does over libpq's 23 flat functions, but it does not scale to a
library with a thousand functions and fifty traits, and every generic collapses
to `void*` on the way through.

The JVM backend's three tricks transfer verbatim:

| JVM backend | Rust backend |
|---|---|
| `import java "pkg.Class"` | `import rust "gpui::div"` |
| `Name : Iface = { m = () => … }` → a class implementing an interface | → `impl Trait for Name` |
| an unknown capitalized type is foreign → gradual `any` | identical |

## What it lowers

**The functional core.** `main` (exits with the returned code), functions,
recursion, `int`/`float`/`str`/`bool`, records → `#[derive(Clone, Debug,
PartialEq)] struct`s, sums → enums **including payload variants** (`Circle(int)
| Rect(int, int)` → `enum { Circle(i64), Rect(i64, i64) }`; construction and
match arms qualified `Enum::Variant`), `match`, lists → `Vec`,
`if`/ternary/`while`/`for`, arithmetic and comparison, string interpolation →
`format!`, anonymous records, and declare-then-reassign mutability.

**Value semantics.** A local passed by value to a user call is `.clone()`d, so
Maca's value semantics survive Rust's move checker. That is the memory model:
clone at the boundary, not a reference-counted cell per record. It is a real
cost in a hot loop, and the native C target exists for that case. Moving
between the two is a flag.

**Crate dependencies.** `import rust "gpui::div"` → `use gpui::div;`.
`[rust-dependencies]` in `maca.toml` generates a throwaway Cargo project and
builds through `cargo`; a program with no external crates takes the fast
single-file `rustc` path. `[rust-patch]` becomes Cargo's `[patch.crates-io]`,
which is how a local checkout or a fork stands in for a published crate. An
import naming an **undeclared crate**, or a non-Rust foreign import
(`import c`/`import py`) on the Rust target, is a hard error rather than a
silent drop.

**Foreign-type calls without `::` surface syntax.** Maca has no `::`, so a call
on a foreign (capitalized, non-local) type is its constructor (`Buffer()` →
`Buffer::new()`), and `Type.assoc(a)` is an associated function
(`Duration.from_secs(5)` → `Duration::from_secs(5)`); an instance receiver stays
`recv.method(a)`. Integer literals in a foreign-call argument drop the `i64`
suffix so `rustc` infers the parameter type (`u64`/`usize`/…). A real std type
compiles and runs end-to-end this way.

**Closures.** A lambda lowers to a `move` closure (`n => n + 1` →
`move |n| { (n + 1) }`) with inferred parameter types, so it can escape into a
foreign callback API and outlive its frame.

**Foreign trait impls.** `Counter : Render = { render = (self, …) => … }` →
`impl Render for Counter { fn render(&mut self, …) { … } }`. A leading `self`
becomes `&mut self`. A parameter whose type this module does not declare is
**foreign**, and Maca never owns a value from a crate it does not read, so it
lowers to `&mut T`, which is how a Rust trait method takes its arguments
(`w: &mut Window`, `cx: &mut Context<Self>`). Call sites pass `&mut`, and a
borrow is passed on rather than cloned. It may be read and handed to another
call, but not returned or stored: that would outlive the call, and the lifetime
it would need has no spelling in Maca, so it is a clean error naming the
parameter rather than one from `rustc` about a type nobody wrote.

The return type is inferred from the body
(`()`/`bool`/`String`/`i64`), which covers event handlers and getters, or
**declared** as `render = (self, w, cx) -> AnyElement => …`, for a method whose
signature the backend cannot see. gpui's `Render::render` returns
`impl IntoElement`, and Rust lets an impl name a concrete type where the trait
wrote `impl Trait`, so the annotation is the whole answer. The
checker treats unknown lowercase calls as gradual foreign items once the program
has an `import rust`, and an `import rust """…"""` raw block is emitted verbatim,
so a trait or `fn` can be supplied inline.

**Method chains on foreign values.** UFCS plus JVM-style `obj.m(a)` passthrough
covers `div().flex().gap_2().child(x)` once the receiver is gradual.

**Colorblind async.** `spawn e` becomes a `std::thread::spawn` handle and
`await h` joins it, the same shape the C runtime gives it with pthreads. There
is still no `async` keyword and no function colour.

## What it does not do, and why that is the remit

No borrow checker, no lifetimes, no trait *definitions* (only impls of foreign
traits), no generic bounds, associated types or HRTBs, no proc macros, no
`unsafe`, and no Rust parser. The non-escaping rule on foreign parameters is
one syntactic check, not an analysis: it does not track what a call does with
what it was handed. The backend emits Rust and lets `rustc` be the
authority on Rust.

A foreign call's argument types stay gradual beyond bare integer literals. Maca
does not read the crate's signatures, so a wrong type is `rustc`'s error to
report rather than a second type checker's. That is the same bargain `import c`
makes, and the reason a foreign call needs no declaration in the first place.

Reaching a Rust library through its own type system, rather than reimplementing
that type system, is the whole design. A second Rust would be a second thing to
keep correct.

## Gates

- `crates/backend_rust/tests/emit.rs`: hermetic, over the emitted source.
- `rust_target_tests` in the driver: import validation and manifest generation.
- `crates/driver/tests/rust_backend.rs`: compile and run through `rustc`, plus
  a best-effort cargo-dependency build, a closure passed to a callback API, a
  foreign trait impl against a local stand-in trait, `spawn`/`await`, and a real
  `std` type.

`apps/examples/gpui_counter.maca` is the shape a gpui program takes on this backend:
a foreign trait impl with a declared return type, `&mut self` threading, a
mutating closure in an event handler, a generic foreign type, and a builder
chain.
