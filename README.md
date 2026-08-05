# Maca

One typed language for **programs and infrastructure config**.

Programs compile to a native binary, JavaScript, the JVM, Rust, or bare-metal
firmware. Config compiles to Nix. Everything you write is `.maca` or
`maca.toml`.

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
| [`crates/`](crates/) | the Rust bootstrap compiler |

## Build it

```sh
cargo build
cargo test              # add -- --test-threads=1 for the full native suite
cargo run -p maca-driver -- --version
```

## Licence

MIT.
