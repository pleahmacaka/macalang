# Maca → Minecraft (Fabric) mod

Write Minecraft mod logic in **Maca**, transpile it to Java with `maca build
--target jvm`, and load it like any Fabric mod. The Maca compiler emits a
`.java` file that `javac`/Gradle compiles against the Minecraft + Fabric API.

## The Maca source

`mod.maca` declares a Fabric `ModInitializer` and some plain Maca logic:

```maca
import java "net.fabricmc.api.ModInitializer"

greeting(name: str) -> str =>
    "Hello from a Maca-authored Fabric mod, {name}!"

ExampleMod : ModInitializer = {
    onInitialize = () => info(greeting("world"))
}
```

- `import java "…"` → a Java `import`.
- `Name : Iface = { m = () => … }` → `class Name implements Iface` (each lambda
  field is a method). This is how you implement a Fabric entrypoint.
- top-level functions → `static` helpers; `info(x)` → `System.out.println(x)`.
- Interop: `obj.method(a)` → `obj.method(a)`, `Blocks.STONE` → `Blocks.STONE`,
  a capitalized call `BlockPos(x, y, z)` → `new BlockPos(x, y, z)`.

## Transpile

```sh
# emit Java (add --cp <fabric-api-jar> so javac resolves Minecraft types)
maca build --target jvm mod.maca -o build --cp path/to/fabric-api.jar
```

This produces `build/Mod.java` with a nested `Mod$ExampleMod` implementing
`ModInitializer`, which is the class named in
`src/main/resources/fabric.mod.json`'s `entrypoints.main`.

## Into a real Fabric project

1. Scaffold a Fabric mod (the [official template](https://fabricmc.net/develop/)).
2. Point the build at the generated Java, e.g. run
   `maca build --target jvm mod.maca -o src/main/java/com/example` in a
   `prebuild` step (add `package com.example;` by placing it under that path and
   passing a package, or wrap the emit), and let Gradle compile it with the
   Minecraft + Fabric API on the classpath.
3. `gradlew runClient`: the mod loads and `onInitialize()` runs.

## Verified here

`onInitialize()` was compiled against a stub `ModInitializer` and invoked
through the interface on the JVM. It prints the greeting, proving the
Maca-authored class is a valid Fabric entrypoint. Full in-game runs need the
Minecraft/Fabric jars (fetched by Gradle), which aren't vendored in this repo.
