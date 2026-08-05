# Installing

Maca is one binary and a language server. Installing it takes a line.

## Download the installer, run it

The installer is a binary in the release, one per platform. Pick yours from
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

The last thing it does is compile and run a small program that imports the
standard library. So the installer does not report success until the compiler
it installed has actually compiled something.

## In GitHub Actions

There is an action, and it is two lines:

```yaml
- uses: pleahmacaka/macalang@main
- run: maca build
```

It fetches the installer for the runner, then runs `maca install`, which
fetches what your `maca.toml` names at the versions `maca.lock` pinned.

## What else you need

**A C compiler.** This is the one real requirement. Maca compiles through C, so
`maca build` and `maca run` invoke `cc` or `clang` at the end of the pipeline.
On macOS the Xcode command line tools provide it; on Debian or Ubuntu it is
`build-essential`; on Fedora, `gcc`. Everything else in the toolchain works
without one, but the two commands you will use most do not.

**Nix, only for two things.** `maca dev` and the Nix build target want
[Nix](https://nixos.org). Nothing else does, and the installer does not ask
about it: if you never run those two commands you never need it. Nix has no
native Windows build, so on Windows `maca dev` runs under WSL.

**Rust, only to build the compiler yourself.** The installer downloads a
binary. Building from a checkout is `cargo build`, and that is the only path
that wants a Rust toolchain.

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

For **Zed**, the repository ships an extension under `apps/editor/zed-maca` with a
tree-sitter grammar, highlighting, an outline and the server wired up. Install
it from a checkout: *Extensions → Install Dev Extension*, and choose that
directory.

For anything else that speaks LSP, point it at the `maca-lsp` binary for files
matching `*.maca`. It provides diagnostics, hover, go to definition, find
references, document symbols, signature help, completion, rename and formatting.

## Without installing anything

The [playground](../play/) runs the compiler in your browser (it is the same
compiler, built for WebAssembly), so you can work through the next few chapters
before deciding to install a toolchain at all.

## Keeping it current

```sh
maca upgrade
```

fetches the newest release and replaces the binaries in place.
