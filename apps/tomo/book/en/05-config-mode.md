# One Language for Configuration

The same Maca you write for programs also describes infrastructure. In **config
mode** a program compiles to Nix instead of a binary — so a machine's definition
is type-checked code, not a stringly-typed YAML file.

## A configuration is a value

```
import options

host = {
    networking.hostName = "web-01"
    services.nginx.enable = true
    services.nginx.virtualHosts."example.com".root = "/srv/www"
}
```

This is ordinary Maca — a record of settings. It compiles to a `.nix` expression
that Nix evaluates to build the system.

## What config mode forbids

Config mode is *pure*: a configuration describes state, it does not perform
actions. So the effects that are fine in a program become errors here.

- `await`/`spawn`/`sleep_ms` — async is impure → **compile error**.
- Reaching for I/O, randomness, or the clock — impure → **compile error**.

The compiler enforces this with the effect system from the previous chapter. You
cannot accidentally smuggle a side effect into a machine definition.

## Options are typed

The options a target exposes (`services.nginx.enable`, and so on) are typed. Set
one that doesn't exist, or give it the wrong type, and you get a clear diagnostic
before anything reaches a host:

```
services.nginx.enabled = true   // error: unknown option (did you mean `enable`?)
```

`UnknownOption` and `EffectInConfig` are ordinary compile errors, caught at your
desk rather than on the machine.

## Why share the language

Because programs and config are the same language, they share types, share
tooling, and can share values. A port number defined once is the same constant
your server binds and your firewall opens — no drift between "the app" and "the
box it runs on".

Next: the targets and the toolchain.
