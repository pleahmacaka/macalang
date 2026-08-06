# Installing

Maca is one binary and a language server. Pick your platform from
[the latest release](https://github.com/pleahmacaka/macalang/releases/latest).

## Download the installer, run it

```sh
curl -fsSL -O https://github.com/pleahmacaka/macalang/releases/latest/download/maca-install-linux-x86_64
chmod +x maca-install-linux-x86_64
./maca-install-linux-x86_64
```

Substitute `macos-aarch64` on an Apple silicon Mac, `linux-aarch64` on an ARM
server, and on Windows download `maca-install-windows-x86_64.exe`.

It puts `maca` and `maca-lsp` in `~/.local/bin`, and if that is not on your
`PATH` it prints the line to add. Two flags change its mind:

```sh
./maca-install-linux-x86_64 --prefix /usr/local   # somewhere else
./maca-install-linux-x86_64 --version 0.3.0       # an older release
```

## In GitHub Actions

```yaml
- uses: pleahmacaka/macalang@main
- run: maca build
```

## What else you need

**A C compiler.** `maca build` and `maca run` invoke `cc` or `clang` at the end
of the pipeline. On macOS the Xcode command line tools provide it; on Debian or
Ubuntu it is `build-essential`; on Fedora, `gcc`.

**Nix, only for `maca dev` and the Nix target.** [Nix](https://nixos.org) has no
native Windows build, so on Windows `maca dev` runs under WSL.

**Rust, only to build the compiler yourself**, with `cargo build`.

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

## Editor support

For **Zed**, the repository ships an extension under `apps/editor/zed-maca` with
a tree-sitter grammar, highlighting, an outline and the server wired up:
*Extensions → Install Dev Extension*, then choose that directory.

For anything else that speaks LSP, point it at `maca-lsp` for `*.maca`. It
provides diagnostics, hover, go to definition, find references, document
symbols, signature help, completion, rename and formatting.

## Without installing anything

The [playground](../play/) runs the same compiler in your browser, built for
WebAssembly.

## Keeping it current

```sh
maca upgrade
```

fetches the newest release and replaces the binaries in place.
