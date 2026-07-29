# One typed language for programs and the machines they run on

Maca is **statically typed**, **compiles to a native binary**, and has **no
garbage collector**. The same language that writes your service describes the
machine it is deployed to.

```maca
Shape = Circle(float) | Rect(float, float)

area(s: Shape) -> float =>
    match s {
        Circle(r)  => 3.14159 * r * r
        Rect(w, h) => w * h
    }

main() -> int {
    floor = [Circle(1.0), Rect(3.0, 4.0)]
    info("floor plan: {floor.map(area).sum():.2} m²")
    0
}
```

Try it in the playground — the compiler itself is compiled to WebAssembly, so
it runs in the page, with no server and nothing to install.

## Is this the right tool for you?

The decisions that put a language in a box, stated plainly:

| Question | Answer |
|---|---|
| Typed? | **Statically**, with inference — you rarely write a type that isn't a signature. `any` exists, but as an explicit escape hatch for foreign code, not a default. |
| Garbage collected? | **No tracing collector.** Reference counts are inserted by the compiler (Perceus), so there is no runtime and no pause. |
| Compiled or interpreted? | **Compiled**, through C, with `cc` doing the optimizing. `maca run` compiles and executes in one step, so it feels like a script. |
| How fast? | Within noise of C on recursion and float loops. The [benchmarks](https://github.com/pleahmacaka/macalang/blob/main/bench/results.md) are in the repository, with the one case where it loses and why. |
| Memory safety? | No manual `free`, no use-after-free in safe code, no borrow checker to satisfy. Values are values; the compiler works out when to release them. |
| Concurrency? | Threads, with `spawn`/`await` — and **no function colouring**. There is no `async` keyword to spread through your call graph. |
| Runtime? | A small C runtime, statically linked. A hello-world is a single binary that depends on nothing. |
| Maturity? | Young. The compiler is complete and test-gated, the standard library is small on purpose, and it is bootstrapping itself. |

## What sets it apart

### The same language configures the machine

Point the compiler at a config file and it emits Nix instead of a binary. Same
syntax, same type checker, same editor tooling — and the checker refuses I/O in
config mode, so a machine description cannot quietly do something.

```maca
import nixpkgs

networking.hostName = "rigel"
system.packages     = git, curl, htop, ripgrep

services.openssh = {
    passwordAuthentication = false
}
```

`maca build --target nix system.maca` turns that into a NixOS module. If you
have ever kept a service in one language and the machine it runs on in another,
this is the part to look at.

### Async has no colour

`spawn f(x)` runs `f` concurrently and `await` waits for it. That is the whole
surface. Async-ness is an inferred effect, not a property of a function's type,
so no function is dyed a colour that spreads to everything that calls it.

### One source, six targets

Native C is the default. The same source also compiles to JavaScript with a
reactive DOM, to Java source for JVM interop, to Rust source so a crates.io
library costs a line of configuration, to Nix for config, and to freestanding C
for a Cortex-M or RISC-V microcontroller — no libc, no allocator.

### Markup is syntax, not a string

A tag name called as a function is an element. On the JS target it builds a
reactive DOM; on native it renders to an HTML string, and the compiler
generates a stylesheet for exactly the utility classes you used.

```maca
page(title: str) -> str =>
    article(class="max-w-2xl mx-auto",
        h1(class="font-bold", title)
        p("Rendered on the server, or in the browser, from this line.")
    )
```

**This website is that feature.** It is a Maca program that reads Markdown and
writes the pages you are reading, with no hand-written markup and one line of
hand-written CSS.

## Not for you if

- You want a large ecosystem today. Maca reaches other ecosystems — C, Python,
  crates.io, Maven — rather than having grown its own.
- You want a tracing GC and the freedom from thinking about cycles that comes
  with it. Reference counting does not collect a cycle.
- You want a proven language. This one is young, and says so.

## Getting started

```sh
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
maca init hello && cd hello
maca run main.maca
```

Then read the handbook — twenty-seven chapters and an appendix — or open the
playground and change something.

The source, the issue tracker, and the benchmarks are
[on GitHub](https://github.com/pleahmacaka/macalang). Criticism is welcome —
particularly the kind that says which of the boxes above is the wrong one.
