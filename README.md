# Maca

One typed language that becomes the others. Maca is a **universal
transpiler**: what you write is translated into C, JavaScript, Java, Rust,
Elixir, freestanding C or Nix, and that language's own toolchain takes it from
there. Maca emits no machine code of its own, so every target is a language
already at home on its platform.

Programs become a binary through C, a page through JavaScript, a JAR through
Java, a crate through Rust, firmware through freestanding C. Infrastructure
config becomes Nix. Everything you write is `.maca` or `maca.toml`.

```maca
import { lines } from std/text

Task = { title: str, done: bool }

read(text: str) -> Task[] =>
    lines(text).map(l => Task { title = l.trim(), done = l.starts_with("x ") })

main() -> int {
    tasks = read("x ship it\nwrite the README")
    info("{tasks.filter(t => !t.done).length()} left")
    0
}
```

**[Read the handbook](https://pleahmacaka.github.io/macalang/)** ([한국어](https://pleahmacaka.github.io/macalang/ko/)),
or [try it in the browser](https://pleahmacaka.github.io/macalang/playground/).

## Install

Download the installer for your platform from
[the latest release](https://github.com/pleahmacaka/macalang/releases/latest)
and run it:

```sh
curl -fsSL -O https://github.com/pleahmacaka/macalang/releases/latest/download/maca-install-linux-x86_64
chmod +x maca-install-linux-x86_64 && ./maca-install-linux-x86_64
```

It puts `maca` and `maca-lsp` in `~/.local/bin`, then compiles and runs a
program that imports the standard library to prove the install works.
`--prefix` chooses somewhere else and `--version` chooses a release.

In GitHub Actions, the whole of it is one step:

```yaml
- uses: pleahmacaka/macalang@main
- run: maca build
```

`maca build` and `maca run` need a C compiler on PATH. `maca dev` and Nix
builds need [Nix](https://nixos.org), and nothing else does.

## Where things are

| | |
|---|---|
| [Handbook](https://pleahmacaka.github.io/macalang/) | learn the language, then keep it open as the reference |
| [`docs/SPEC.md`](docs/SPEC.md) | the authority on what the language is |
| [`docs/`](docs/) | for working on Maca: bootstrap, back ends, releasing |
| [`modules/`](modules/) | the standard library, which rides inside the binary |
| [`apps/`](apps/) | everything this repository builds, including the compiler written in Maca |
| [`bootstrap/`](bootstrap/) | `maca.c`, the compiler as it emitted itself: how a machine with no Maca gets one |

## Build it

Maca is written in Maca. `bootstrap/maca.c` is the compiler as the compiler
emitted it, so any C compiler turns it back into a working one:

```sh
cc -O1 -o bootstrap/maca bootstrap/maca.c
MACA=$PWD/bootstrap/maca ./bootstrap/maca build apps/maca1/main.maca -o bin/maca
MACA=$PWD/bin/maca ./bin/maca --version
```

## Licence

MIT.
