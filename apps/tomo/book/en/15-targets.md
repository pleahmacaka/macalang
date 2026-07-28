# Targets

The same source compiles to very different things. Which one you get is a flag,
not a dialect — there is no `#ifdef`, no per-target subset of the language, and
no separate standard library. You write Maca and choose where it lands.

## Native, by way of C

With no `--target`, Maca compiles through C and links a static binary. This is
the path everything else is measured against: no runtime to ship, no interpreter
to start, no collector to pause. The generated C goes to whatever compiler your
system has, so the optimiser you benefit from is the one that has had thirty
years of work put into it.

```
maca build app.maca -o app
./app
```

A numerically hot span can lower through LLVM instead, for vectorisation. That
path exists only for SIMD, and it links over the C ABI alongside everything
else, so it stays a local decision rather than a whole-program one.

## JavaScript

```
maca build app.maca --target js -o out
```

produces a self-contained page. The JavaScript backend understands Maca's
reactive UI syntax and generates the CSS for the Tailwind utility classes it
finds, so an interface written in Maca needs no bundler, no `package.json` and
no build step beyond that command. The playground that ships with this book is
one `.maca` file compiled exactly this way.

The same syntax works on the native target, where an element renders to an HTML
string instead of a DOM node — that is what a static site generator needs, and
it is [the previous chapter's subject](15-ui.md).

## The JVM and Rust

These two are less about deployment than about *reach*. They exist so that Maca
can use an ecosystem instead of reimplementing it.

`--target jvm` emits Java source, which makes JVM interop ordinary — Minecraft
mods through Fabric are the worked example in the repository. `--target rust`
emits Rust source and takes dependencies from `[rust-dependencies]` in
`maca.toml`, so a crates.io library is a line of configuration away.

The trade is that you inherit the target's start-up cost and its runtime. Reach
for these when the library you need lives there, and for native when it does not.

## Embedded

```
maca build blink.maca --target embedded --mcu cortex-m0
```

emits freestanding C for a bare-metal microcontroller: no libc, no allocator, no
operating system. Cortex-M and RISC-V are supported. Memory-mapped registers are
ordinary Maca values, and a field write lowers to a read-modify-write of the
right width.

This is the target that most justifies compiling through C. The C toolchain for
a given chip already exists, and Maca gets to use it rather than carry a code
generator per microcontroller family.

## Nix

`--target nix` is the config-mode output covered in chapter 14. It belongs in
this list because it is the target that makes Maca unusual: the language that
produced the binary can also describe the machine the binary runs on, and both
halves are type-checked before anything is deployed.

## Tauri

`--target tauri` scaffolds a desktop application — the JavaScript backend for
the interface, a native binary underneath it.

## Choosing

Most programs want native, and the question only arises when something outside
the program dictates the answer: a browser, a microcontroller, a Java API, a
crate that would take a month to port. The useful property is not that Maca has
many backends but that moving between them costs a flag rather than a rewrite.

## Where the list stops

Every target here earns its place by reaching something: a browser, a
microcontroller, the JVM's libraries, crates.io. That is the whole rule, and it
is why the list is the length it is.

A BEAM target is the one people ask about, because Maca's concurrency model
looks like a fit. It isn't on the list, because it would be the first backend
added for elegance rather than reach — colorblind async (chapter 13) already
runs on pthreads in the C runtime, and an Erlang-style lowering would be a
second, genuinely different implementation of something that works. Reach is a
reason. Symmetry is not.
