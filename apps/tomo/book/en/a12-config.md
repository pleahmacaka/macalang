# Config Mode

The rules for the Nix target: what a configuration is, what it may not do, and
what the compiler checks. The introduction is
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

A nested option set is a record on the right of the `=`, and a dotted path on
the left goes as deep as the option does:

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

### The two rewrites

| Written | Emitted |
|---|---|
| `system.packages = a, b` | `environment.systemPackages = [ pkgs.a pkgs.b ]` |
| `services.X = { … }` | the same block with `enable = true;` added |

The injection is unconditional. Writing `enable` in the block yourself emits it
twice (`enable = true; enable = false;`), and a repeated attribute is not a thing
Nix accepts.

## What config mode forbids

Config mode is *pure*. Every effect that is fine in a program is an error here,
and **all** of them:

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

The check is the effect system from [Effects and Async](a7-effects.md) pointed
at a whole module. The left column is a list of shapes, not of ideas: the free
builtins `read_file`, `capture` and `exec` are in none of those rows, so a
config that calls one compiles.

## Options are checked by namespace

The compiler knows the NixOS option roots: `networking`, `services`, `system`,
`users`, `environment`, `programs`, `boot`, `hardware`, `security`, `nix`,
`fonts` and their siblings. An assignment whose root is none of them and is not
a local binding is a diagnostic:

```
UnknownOption: unknown NixOS option namespace `servicez`
```

It is the **namespace** that is verified, not the leaf. A typo in
`services.nginx.enabl` goes through to Nix, which rejects it at evaluation time
with its own message.

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

Windows has no Nix, so a config that declares `scoop.*`, `choco.*` or `winget.*`
packages also gets `.maca/dev/{setup,activate}.ps1`, a portable, project-local
toolchain under `.maca\dev\`. The flake ignores those namespaces.

## Why share the language

Programs and config share types, tooling and values, and the effect row is what
makes that safe rather than merely convenient.

