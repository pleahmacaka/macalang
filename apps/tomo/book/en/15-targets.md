# Where a Program Lands

You have been building native binaries this whole book. That was a default, not
a limit: the same source compiles to a browser page, to the JVM, to Rust, to a
microcontroller, and to a machine's configuration. Which one you get is a flag.

There is no `#ifdef`, no per-target subset of the language and no separate
standard library. That is the claim worth testing, so test it.

## Six flags

```
maca build app.maca -o app                 # a native binary (the default)
maca build app.maca --target js -o out     # a page
maca build app.maca --target jvm           # Java source
maca build app.maca --target rust          # Rust source
maca build app.maca --target embedded --mcu cortex-m0
maca build app.maca --target nix           # a machine's configuration
```

## Native is the one to reach for

With no flag, Maca compiles through C and links a static binary: no runtime to
ship, no interpreter to start, no collector to pause. The generated C goes to
whatever compiler your system has, so the optimiser you benefit from is one that
has had thirty years of work put into it.

Most programs want this. The question only comes up when something outside the
program dictates the answer: a browser, a chip, a Java API, a library that
would take a month to port.

## The others exist to reach something

| Target | Reaches |
|---|---|
| `js` | the browser, with the UI syntax as a live DOM |
| `jvm` | Java's libraries; Minecraft mods are the worked example |
| `rust` | crates.io, through emitted Rust source |
| `embedded` | a bare-metal Cortex-M or RISC-V, with no libc under it |
| `nix` | the machine the binary runs on |

Each is on the list because it reaches an ecosystem Maca would otherwise have to
reimplement. That is the whole rule, and it is why the list is the length it is.

The one people ask for and don't get is BEAM. Maca's concurrency model looks
like a fit, but it would be the first backend added for elegance rather than
reach; [colorblind async](13-colorblind-async.md) already runs on real threads
in the C runtime. Reach is a reason; symmetry is not.

## Try it

Take any program you have written so far and build it twice:

```
maca build hello.maca -o hello
maca build hello.maca --target rust -o hello.rs
```

The second is Rust source you can read. Nothing in `hello.maca` changed, and
nothing had to.

## Where the full answer is

[Targets](a10-targets.md) in the reference has every flag, the MMIO vocabulary
the embedded target adds, the C ABI the native halves agree on, and the exact
list of what each target **refuses**, which is the part worth reading before
you pick one. A target that cannot honour something says so at compile time
rather than emitting code that quietly does nothing.

The UI syntax the `js` target brings to life is
[the previous chapter](15-ui.md); the Nix output is
[config mode](14-config-mode.md), two chapters back.
