# Where a Program Lands

The same source compiles to a native binary, a browser page, the JVM, Rust, a
microcontroller, and a machine's configuration. Which one you get is a flag: no
`#ifdef`, no per-target subset, no separate standard library.

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

## The others exist to reach something

| Target | Reaches |
|---|---|
| `js` | the browser, with the UI syntax as a live DOM |
| `jvm` | Java's libraries; Minecraft mods are the worked example |
| `rust` | crates.io, through emitted Rust source |
| `elixir` | the BEAM, where ten thousand small waits cost ten thousand processes |
| `embedded` | a bare-metal Cortex-M or RISC-V, with no libc under it |
| `nix` | the machine the binary runs on |

Each is on the list because it reaches an ecosystem Maca would otherwise have to
reimplement. `elixir` was the one held back longest, on the grounds that
[colorblind async](13-colorblind-async.md) already runs on real threads and a
BEAM backend would be the first added for elegance rather than reach. What
changed is the workload: a program holding thousands of slow network calls open
at once wants a process each, not a thread each, and no other target on this
list gives that. You name the language, never the machine under it, so the flag
is `--target elixir` and `--target beam` is a usage error.

## Try it

```
maca build hello.maca -o hello
maca build hello.maca --target rust -o hello.rs
```

## Where the full answer is

[Targets](a10-targets.md) has every flag, the MMIO vocabulary the embedded
target adds, the C ABI the native halves agree on, and the exact list of what
each target **refuses** at compile time.

The UI syntax the `js` target brings to life is
[the previous chapter](15-ui.md); the Nix output is
[config mode](14-config-mode.md).
