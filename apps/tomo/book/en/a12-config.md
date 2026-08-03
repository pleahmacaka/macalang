# Config Mode

The rules for the Nix target: what a configuration is, what it may not do, and
what the compiler checks before anything reaches a host. The introduction is
[One Language for Configuration](14-config-mode.md).

## Selecting the mode

Mode follows the target rather than a keyword in the file.

| How | Mode |
|---|---|
| `maca build host.maca --target nix` | config |
| `[hosts.X]` in `maca.toml` | config |
| `[[bin]]` in `maca.toml`, or no target | program |
| `maca dev` over `dev.maca` | config, with the flake emitter |

## A configuration is a value

A config module is **top-level assignments to option paths**, not a record:

```maca
import nixpkgs

networking.hostName = "rigel"
system.stateVersion = "24.11"

system.packages = git, curl, htop, ripgrep
```

`import nixpkgs` brings in the package set, so `nixpkgs.fish` names a package.
A nested option set is written as a record on the right of the `=`:

```maca
services.openssh = {
    passwordAuthentication = false
}
```

and a dotted path on the left goes as deep as the option does. The output is an
ordinary NixOS module:

```nix
{ config, pkgs, lib, ... }:
{
  networking.hostName = "rigel";
  system.stateVersion = "24.11";
  environment.systemPackages = [ pkgs.git pkgs.curl pkgs.htop pkgs.ripgrep ];
  services.openssh = {
    enable = true;
    passwordAuthentication = false;
  };
}
```

Nix evaluates that to build the system. Nothing about the pipeline is special.
The difference is that the file went through the type and effect checker first.

### The two rewrites

Most option paths come out the way they went in. Two do not, and both are
visible above:

| Written | Emitted |
|---|---|
| `system.packages = a, b` | `environment.systemPackages = [ pkgs.a pkgs.b ]` |
| `services.X = { … }` | the same block with `enable = true;` added |

The first is a shorter name for the option people reach for most. The second is
implicit enable: configuring a service is how you ask for it, so a block that
sets anything gets switched on.

The injection is unconditional. Writing `enable` in the block yourself emits it
twice (`enable = true; enable = false;`), and a repeated attribute is not a
thing Nix accepts. Leave it out and let the block mean what it says.

## What config mode forbids

Config mode is *pure*: a configuration describes state, it does not perform
actions. Every effect that is fine in a program is an error here, and **all** of
them, not a selected list:

```
EffectInConfig: config must be pure but this uses effect(s): async
```

| Reaching for | Row | Result |
|---|---|---|
| `await`, `spawn`, `sleep_ms` | `async` | compile error |
| `info`, `print`, and the console family | `io` | compile error |
| `x.read(…)`, `x.write(…)` and the file methods | `io` | compile error |
| a `net`/`http`/`socket` call | `net` | compile error |
| an `os`/`process` call | `os` | compile error |
| `fail` | `exn` | compile error |

The message names every row it found, so a configuration that prints *and*
sleeps reports `io, async`. There is no escape hatch, which is the point: a
machine definition that reads a file to decide what it declares is a machine
definition whose meaning depends on when you ran it.

The left column is a list of shapes, not of ideas, and the difference shows at
one edge: the free builtins `read_file`, `capture` and `exec` are in none of
those rows, so a config that calls one compiles. Read that as the reach of a
check rather than as permission. The intent is the whole of the left column,
and a config reaching for the disk is outside it either way.

The check is the effect system from [Effects and Async](a7-effects.md), pointed
at a whole module instead of a function.

## Options are checked by namespace

The compiler knows the NixOS option roots: `networking`, `services`, `system`,
`users`, `environment`, `programs`, `boot`, `hardware`, `security`, `nix`,
`fonts` and their siblings. An assignment whose root is none of them and is not
a local binding is a diagnostic:

```
UnknownOption: unknown NixOS option namespace `servicez`
```

Be precise about the reach of this check, because the difference matters when
you are debugging: it is the **namespace** that is verified, not the leaf. A
typo in `services.nginx.enabl` goes through to Nix, which rejects it at
evaluation time with its own message. What the compiler stops is a whole
namespace that does not exist.

`maca dev` is the one caller that suppresses the diagnostic, because `dev.*` is
not a NixOS namespace at all.

## Dev shells

`maca dev` reads `dev.maca` in config mode and emits a self-contained
`flake.nix` devShell:

```maca
dev.name = "myproject"
dev.packages = zig, nix, ripgrep
dev.env = { RUST_LOG = "debug" }
dev.shellHook = "echo ready"
```

The repository's own `flake.nix` is generated this way.

Windows has no Nix, so a config that declares `scoop.*`, `choco.*` or
`winget.*` packages also gets `.maca/dev/{setup,activate}.ps1`, a portable,
project-local toolchain under `.maca\dev\`. The flake ignores those namespaces,
so a Nix host is unaffected by their presence.

## Why share the language

Because programs and config are the same language, they share types, share
tooling, and can share values. A port number defined once is the same constant
your server binds and your firewall opens, with no drift between "the app" and
"the box it runs on".

The check that makes this safe rather than merely convenient is the effect row.
Without it, "configuration is code" means a configuration can do anything a
program can, which is how a deploy becomes a program nobody reviewed.
