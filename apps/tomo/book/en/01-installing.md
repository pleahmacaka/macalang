# Installing

Maca is one binary and a language server.

## Download the installer, run it

Pick your platform from
[the latest release](https://github.com/pleahmacaka/macalang/releases/latest):

```sh
curl -fsSL -O https://github.com/pleahmacaka/macalang/releases/latest/download/maca-install-linux-x86_64
chmod +x maca-install-linux-x86_64
./maca-install-linux-x86_64
```

Substitute `macos-aarch64` on an Apple silicon Mac, `linux-aarch64` on an ARM
server, and on Windows download `maca-install-windows-x86_64.exe` and run it.

It puts `maca` and `maca-lsp` in `~/.local/bin`, and if that is not on your
`PATH` it prints the line to add. Two flags change its mind:

```sh
./maca-install-linux-x86_64 --prefix /usr/local   # somewhere else
./maca-install-linux-x86_64 --version 0.3.0       # an older release
```

`PREFIX` and `MACA_VERSION` in the environment do the same two things.

It finishes by compiling and running a small program that imports the standard
library, so it does not report success until the compiler it installed has
compiled something.

## In GitHub Actions

```yaml
- uses: pleahmacaka/macalang@main
- run: maca build
```

It fetches the installer for the runner, then runs `maca install`, which fetches
what your `maca.toml` names at the versions `maca.lock` pinned.

## What else you need

**A C compiler.** Maca compiles through C, so `maca build` and `maca run` invoke
`cc` or `clang` at the end of the pipeline. On macOS the Xcode command line
tools provide it; on Debian or Ubuntu it is `build-essential`; on Fedora, `gcc`.

**Nix, only for two things.** `maca dev` and the Nix build target want
[Nix](https://nixos.org). Nix has no native Windows build, so on Windows
`maca dev` runs under WSL.

**Rust, only to build the compiler yourself.** Building from a checkout is
`cargo build`.

## Checking it worked

```sh
maca --version
```

Then the shortest real program there is:

```sh
echo 'main() -> int {
    info("Hello, World")
    0
}' > hello.maca
maca run hello.maca
```

If that prints `Hello, World`, the compiler, the C toolchain and the runtime are
all in place. `maca run` exercises the whole pipeline, so it is a better smoke
test than `--version`.

## Editor support

The language server is already installed; the editor side is a separate step.

For **Zed**, the repository ships an extension under `apps/editor/zed-maca` with
a tree-sitter grammar, highlighting, an outline and the server wired up. Install
it from a checkout: *Extensions → Install Dev Extension*, and choose that
directory.

For anything else that speaks LSP, point it at the `maca-lsp` binary for files
matching `*.maca`. It provides diagnostics, hover, go to definition, find
references, document symbols, signature help, completion, rename and formatting.

## Without installing anything

The [playground](../play/) runs the same compiler in your browser, built for
WebAssembly.

## Keeping it current

```sh
maca upgrade
```

fetches the newest release and replaces the binaries in place.
