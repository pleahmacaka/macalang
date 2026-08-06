# One Language for Configuration

In **config mode** a program compiles to Nix instead of a binary, so a machine's
definition is type-checked code, not a stringly-typed YAML file.

## A configuration is a value

```maca
import nixpkgs

networking.hostName = "rigel"
system.stateVersion = "24.11"

system.packages = git, curl, htop, ripgrep

services.openssh = {
    passwordAuthentication = false
}
```

Ordinary Maca: assignments, a bracketless comma list, a record. Built with
`--target nix` it is a NixOS module:

```
maca build host.maca --target nix -o host.nix
```

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

Two lines came out different: `system.packages` is written where NixOS spells it
`environment.systemPackages`, and a configured `services.X` block gets
`enable = true`. [Config Mode](a12-config.md) lists both rewrites.

## What config mode forbids

Config mode is *pure*: a configuration describes state, it does not act.

- `await`/`spawn`/`sleep_ms`: async is impure → **compile error**.
- Reaching for I/O, the network, or the process table: impure → **compile
  error**.

The effect system from [the previous chapter](13-colorblind-async.md) enforces
it:

```
EffectInConfig: config must be pure but this uses effect(s): async
```

## Option names are checked

The option namespaces a NixOS module may assign to are known to the compiler:

```
UnknownOption: unknown NixOS option namespace `servicez`
```

`UnknownOption` and `EffectInConfig` are ordinary compile errors.

## Try it

```
maca build apps/examples/system.maca --target nix -o system.nix
```

Then add `delay = sleep_ms(10)` to a copy and build again. The error is the
point of this chapter.

## Why share the language

Programs and config share types, tooling and values. A port number defined once
is the same constant your server binds and your firewall opens.

## Where the full answer is

[Config Mode](a12-config.md) has how the mode is selected, the full effect
table, how far the option check reaches, and `maca dev`.
