# Installing

Maca is one binary and a language server. Installing it takes a line.

## macOS and Linux

```sh
curl -fsSL https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.sh | bash
```

That downloads the prebuilt `maca` and `maca-lsp` for your platform and puts
them in `~/.local/bin`. If that directory is not already on your `PATH`, the
installer says so and tells you the line to add.

From a checkout, the same script works, and `PREFIX` chooses where things land:

```sh
./install.sh                     # ~/.local/bin
PREFIX=/usr/local ./install.sh   # /usr/local/bin, which may want sudo
```

## Windows

In PowerShell:

```powershell
irm https://raw.githubusercontent.com/pleahmacaka/macalang/main/install.ps1 | iex
```

Binaries go to `%USERPROFILE%\.local\bin`, and `$env:PREFIX` moves them
elsewhere.

## What else you need

**A C compiler.** This is the one real requirement. Maca compiles through C, so
`maca build` and `maca run` invoke `cc` or `clang` at the end of the pipeline.
On macOS the Xcode command line tools provide it; on Debian or Ubuntu it is
`build-essential`; on Fedora, `gcc`. Everything else in the toolchain works
without one, but the two commands you will use most do not.

**Rust, only sometimes.** The installer downloads a prebuilt binary when there
is one for your platform, and falls back to building from source when there is
not. Only that fallback needs `cargo`.

**Nix, only for two things.** `maca dev` and the Nix build target want
[Nix](https://nixos.org). If it is missing, the installer offers to fetch it
through the Determinate Systems installer; decline and everything except those
two keeps working. For an unattended run, set `MACA_INSTALL_NIX=1` to accept or
`MACA_INSTALL_NIX=0` to skip.

Nix has no native Windows build, so on Windows `maca dev` runs under WSL. The
Windows installer knows this and does not ask.

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
