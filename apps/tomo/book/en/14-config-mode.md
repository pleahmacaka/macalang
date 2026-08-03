# One Language for Configuration

The same Maca you write for programs also describes infrastructure. In **config
mode** a program compiles to Nix instead of a binary, so a machine's definition
is type-checked code, not a stringly-typed YAML file.

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

This is ordinary Maca: assignments, a bracketless comma list, a record. Build
it with `--target nix` and you get a NixOS module:

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

Nix evaluates that to build the system.

Two lines came out different from how they went in. `system.packages` is written
where NixOS spells it `environment.systemPackages`, and a `services.X` block you
configured at all gets `enable = true` without your writing it. You do not
switch a service on and then configure it separately.
[Config Mode](a12-config.md) lists both rewrites.

## What config mode forbids

Config mode is *pure*: a configuration describes state, it does not perform
actions. So the effects that are fine in a program become errors here.

- `await`/`spawn`/`sleep_ms`: async is impure → **compile error**.
- Reaching for I/O, the network, or the process table: impure → **compile
  error**.

The compiler enforces this with the effect system from
[the previous chapter](13-colorblind-async.md):

```
EffectInConfig: config must be pure but this uses effect(s): async
```

You cannot accidentally smuggle a side effect into a machine definition.

## Option names are checked

The option namespaces a NixOS module may assign to are known to the compiler, so
a whole namespace that doesn't exist is caught at your desk:

```
UnknownOption: unknown NixOS option namespace `servicez`
```

`UnknownOption` and `EffectInConfig` are ordinary compile errors, on the same
footing as a type mismatch.

## Try it

The repository has a real one at `examples/system.maca`. Build it and read the
Nix that comes out:

```
maca build examples/system.maca --target nix -o system.nix
```

Then add `delay = sleep_ms(10)` to a copy of it and build again. The error is
the point of this chapter.

## Why share the language

Because programs and config are the same language, they share types, share
tooling, and can share values. A port number defined once is the same constant
your server binds and your firewall opens, with no drift between "the app" and
"the box it runs on".

## Where the full answer is

[Config Mode](a12-config.md) in the reference has how the mode is selected, the
full effect table, exactly how far the option check reaches (further than you
would guess in one direction and less far in another), and `maca dev`, the
same machinery pointed at a development shell instead of a host.
