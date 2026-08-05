# The development shell

The environment this project is built in is described in Maca, not in
hand-written Nix. [`dev.maca`](../dev.maca) is the source; `flake.nix` is
generated from it and is not edited by hand.

```sh
maca dev        # reads ./dev.maca, writes ./flake.nix
nix develop     # enter the shell
```

## What you write

```maca
import nixpkgs

dev.name      = "macalang"
dev.packages  = rustc, cargo, clang, lld, jdk21
dev.env       = { RUST_BACKTRACE = "1" }
dev.shellHook = "echo ready"
```

| binding | becomes |
|---|---|
| `dev.name` | the flake description and the shell name |
| `dev.packages = a, b` | `mkShell { packages = [ pkgs.a pkgs.b ]; }` |
| `dev.env = { K = "v" }` | environment variables in the shell |
| `dev.shellHook` | `shellHook` |

The generated flake takes one input, `nixpkgs`, and covers `x86_64` and
`aarch64` on Linux and Darwin through `nixpkgs.lib.genAttrs`. So `nix develop`
works with nothing else to add.

## On a machine with no Nix

Declaring `scoop.*`, `choco.*` or `winget.*` packages also writes
`.maca/dev/{setup,activate}.ps1`, a toolchain local to the project under
`.maca\dev\`. The flake ignores those namespaces, so a Nix host is unaffected
by their presence and a Windows host does not need Nix.

This is the config-mode Nix back end doing its ordinary job: the same compiler
that turns a `system.maca` into a NixOS module turns `dev.maca` into a shell.
