# Dev environment in Maca (`dev.maca` → `flake.nix`)

The project's development environment is described in **Maca**, not hand-written
Nix. [`dev.maca`](../dev.maca) is the source of truth; `flake.nix` is generated
from it:

```sh
maca dev            # reads ./dev.maca, writes ./flake.nix
maca dev env.maca -o flake.nix   # explicit paths
nix develop         # enter the shell
```

## Surface

```maca
import nixpkgs

dev.name     = "macalang"
dev.packages = rustc, cargo, clang, lld, jdk21
dev.env      = { RUST_BACKTRACE = "1" }
dev.shellHook = "echo ready"
```

| binding | becomes |
|---|---|
| `dev.name` | the flake description / shell name |
| `dev.packages = a, b, …` | `mkShell { packages = [ pkgs.a pkgs.b … ]; }` |
| `dev.env = { K = "v" }` | shell environment variables |
| `dev.shellHook = "…"` | `shellHook` |

The generated flake is self-contained: only a `nixpkgs` input, multi-system via
`nixpkgs.lib.genAttrs` (`x86_64`/`aarch64`, Linux + Darwin). So `nix develop`
works with no extra flake inputs.

This reuses the config-mode Nix backend (`maca-backend-nix`): the same compiler
that turns `system.maca` into a NixOS module turns `dev.maca` into a dev-shell
flake. Regenerate `flake.nix` whenever you edit `dev.maca`; don't hand-edit the
flake.
