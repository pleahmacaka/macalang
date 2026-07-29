// Smoke test for the macalang JS package (run: node test.mjs).
//
// One of the few files here that is not Maca, and it cannot be: what it tests
// is that a *JavaScript* program can import this package and call it. Rewritten
// in Maca it would exercise the compiler rather than the package, which is
// what every other suite in the repository already does. `index.js` and
// `bun-plugin.js` are not Maca for the same reason twice over — they are what a
// consumer imports, and they run in Node and Bun, where no Maca toolchain
// exists.
import assert from "node:assert";
import { writeFileSync } from "node:fs";
import { compile, toJS, loadModule, loadFile, version } from "./index.js";

console.log("maca version:", version());

// 1. compile returns diagnostics
const bad = compile("main() -> int => 0"); // ok program
assert.equal(bad.diagnostics.length, 0);

// 2. type errors surface as diagnostics
const typed = compile('bad() -> int => "nope"');
assert.ok(typed.diagnostics.some((d) => d.kind === "TypeMismatch"), "expected TypeMismatch");

// 3. functions become callable
const m = loadModule(
  "add(a: int, b: int) -> int => a + b\n" +
    "double(n: int) -> int => n * 2\n" +
    'greet(name: str) -> str => "hi {name}!"\n',
);
assert.equal(m.add(2, 3), 5);
assert.equal(m.double(21), 42);
assert.equal(m.greet("maca"), "hi maca!");

// 4. loadFile
writeFileSync("/tmp/_maca_pkg_test.maca", "sq(n: int) -> int => n * n\n");
const f = loadFile("/tmp/_maca_pkg_test.maca");
assert.equal(f.sq(9), 81);

// 5. toJS produces a string
assert.ok(toJS("id(x: int) -> int => x").includes("function id"));

console.log("all macalang package tests passed ✓");
