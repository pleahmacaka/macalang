# Targets

Which target you get is a flag, not a dialect: there is no `#ifdef`, no
per-target subset of the language, and no separate standard library. The tour is
[Where a Program Lands](15-targets.md).

## The flags

| Command | Produces |
|---|---|
| `maca build app.maca -o app` | a static native binary, through C |
| `maca build app.maca --target js -o out` | a self-contained page |
| `maca build app.maca --target jvm --cp …` | Java source |
| `maca build app.maca --target rust` | Rust source |
| `maca build app.maca --target embedded --mcu cortex-m0` | freestanding C |
| `maca build app.maca --target nix` | a `.nix` expression |
| `maca build app.maca --target tauri` | a desktop application scaffold |

`--mcu` belongs to `embedded` and `--cp` to `jvm`. With no `--target` you get a
native binary.

## Native, by way of C

```
maca build app.maca -o app
./app
```

No runtime to ship, no interpreter to start, no collector to pause. The
generated C goes to whatever compiler your system has.

## JavaScript

```
maca build app.maca --target js -o out
```

produces a self-contained page. The JavaScript backend understands Maca's
reactive UI syntax and generates the CSS for the Tailwind utility classes it
finds, so an interface needs no bundler and no `package.json`. The same syntax
works natively, where an element renders to an HTML string. See
[the UI syntax](a11-ui.md).

### What the page says it is

The page is named after its source file unless `maca.toml` says otherwise:

```toml
[page]
title = "tabpane"
lang = "ko"
description = "a browser start page"
```

`title` fills the `<title>`, and titles the window under `--target tauri` as
well. `lang` becomes the `<html lang>` attribute and `description` a meta tag;
leave either out and neither is emitted. A key that is none of the three is an
error rather than a default.

### What the page carries

A stylesheet or a script that is not Maca is an import:

```maca
import "vendor/daisyui.css"       // a file, read at build time
import "vendor/iconify-icon.js"    // a file, read at build time

import css """
.card { border-radius: 8px }
"""                                   // the source itself, written inline
```

A quoted string names a file and says what it is by its extension, so it takes
no language word; a `"""…"""` block is the source itself and keeps one. The file
is resolved against the source that imports it, read at build time and
**inlined**: a stylesheet lands in a `<style>` ahead of the generated one, so the
app's own utilities win over a vendor sheet; a script lands in a `<script>` after
the element the app mounts into. Nothing is linked, because `index.html` is the
whole deployable. `import "x.wasm"` is the same idea for a binary, embedded as
base64.

A path that resolves to no file fails the build and names the file.

## The JVM and Rust

`--target jvm` emits Java source, which makes JVM interop ordinary. Minecraft
mods through Fabric are the worked example in the repository. `--target rust`
emits Rust source and takes dependencies from `[rust-dependencies]` in
`maca.toml`, so a crates.io library is a line of configuration away.

### Implementing a foreign trait

`Type : Trait = { … }` declares a Rust trait implementation, one field per
method. This is the one place a lambda's return type has to be written down: the
trait lives in a crate the compiler does not read.

```maca
Counter : Render = {
    render = (self, window, cx) -> AnyElement =>
        div().child("Count: {self.count}").into_any_element()
}
```

## Embedded

```
maca build blink.maca --target embedded --mcu cortex-m0
```

emits freestanding C for a bare-metal microcontroller: no libc, no allocator, no
operating system. Cortex-M and RISC-V are supported. Memory-mapped registers are
ordinary Maca values, and a field write lowers to a read-modify-write of the
right width.

There is no console, so `info` and its siblings are not available (drive a UART
with `mmio_write`), and `main` returns nothing, because there is no process to
hand an exit code to. The reset handler calls it and halts when it returns.

`int` is a 32-bit word here rather than 64. The MMIO vocabulary is
`mmio_write`/`mmio_read`, `set_bits`/`clear_bits`/`toggle_bits`, `bit`,
`shl`/`shr`/`bit_or`/`bit_and`, `delay` and `nop`, and `for _ in forever()` is
the super-loop.

## Nix

`--target nix` is the config-mode output covered by
[Config Mode](a12-config.md).

## Tauri

`--target tauri` scaffolds a desktop application: the JavaScript backend for the
interface, a native binary underneath it.

## What each target refuses

A target refuses what it cannot honour, at compile time and by name.

| Target | Refuses |
|---|---|
| native | `on:click=` and its siblings; an event handler has nowhere to attach in a string |
| `rust` | a bodyless (FFI) function; `import c` / `import py`; an `import rust` naming an undeclared crate; a borrowed foreign parameter that is returned or stored; a function defined inside another |
| `jvm` | a function defined inside another |
| `embedded` | `info` and the other console builtins; a `main` with a return type; a function defined inside another |
| `nix` | any non-empty effect row; see [Effects and Async](a7-effects.md); `return`, since config mode has no function to leave |

A function defined inside another is refused by three targets for three reasons,
all about the *write*. Rust will not let two closures hold a mutable borrow of
one local at once. A Java lambda captures an effectively final variable, so
there is nowhere for the write to go. Freestanding C has no allocator, and a
shared local needs a heap cell. Native C and the JS backend both lower it; the
playground interpreter refuses it, because it copies what a closure captures and
would quietly lose the write.

## ABI and linking

Everything native converges on the **C ABI**. The C backend and the LLVM SIMD
span emit objects that link together; an FFI declaration is an ordinary extern;
and an async function is an ordinary function: `spawn`/`await` change what the
body does, never how it is called.

## Choosing

## Where the list stops

Every target here earns its place by reaching something. A BEAM target would be
the first backend added for elegance rather than reach.
[Colorblind async](a7-effects.md) already runs on pthreads in the C runtime.
