// Smoke test for the macalang JS package (run: node test.mjs).
//
// One of the few files here that is not Maca, and it cannot be: what it tests
// is that a *JavaScript* program can import this package and call it. Rewritten
// in Maca it would exercise the compiler rather than the package, which is
// what every other suite in the repository already does. `index.js` and
// `bun-plugin.js` are not Maca for the same reason twice over: they are what a
// consumer imports, and they run in Node and Bun, where no Maca toolchain
// exists.
//
// It needs a `maca_wasm.wasm` beside it, which is a build artifact and is not in
// the repository. Build one first:
//
//     maca run apps/npm/build.maca
import assert from "node:assert";
import { readFileSync, writeFileSync } from "node:fs";
import { compile, toJS, loadModule, loadFile, lsp, version } from "./index.js";

const BUILD = "maca run apps/npm/build.maca";

// 0. the wasm is there, is a wasm, and is the shape this package calls.
//
// A stale wasm is the failure worth naming here, because every other assertion
// in this file passes against one: `maca_wasm.wasm` is not in the repository
// and one left over from a previous build answers every older call perfectly.
// The import list is the cheapest thing that dates it, and it is short because
// the compiler is a wasi command rather than a library of exports.
let bytes;
try {
  bytes = readFileSync(new URL("./maca_wasm.wasm", import.meta.url));
} catch {
  assert.fail(`no maca_wasm.wasm beside test.mjs: build one with \`${BUILD}\``);
}
assert.deepEqual(
  [...bytes.subarray(0, 4)],
  [0x00, 0x61, 0x73, 0x6d],
  "maca_wasm.wasm does not start with the wasm magic bytes",
);
const mod = new WebAssembly.Module(bytes);
const surface = WebAssembly.Module.exports(mod).map((e) => e.name);
for (const name of ["memory", "_start"]) {
  assert.ok(
    surface.includes(name),
    `maca_wasm.wasm has no "${name}" export, so it is not the wasi command this ` +
      `package calls: rebuild it with \`${BUILD}\``,
  );
}
const wanted = new Set([
  "args_get",
  "args_sizes_get",
  "fd_close",
  "fd_fdstat_get",
  "fd_seek",
  "fd_write",
  "proc_exit",
]);
for (const i of WebAssembly.Module.imports(mod)) {
  assert.equal(i.module, "wasi_snapshot_preview1", `unexpected import module ${i.module}`);
  assert.ok(wanted.has(i.name), `the shim in index.js does not answer ${i.name}`);
}

console.log("maca version:", version());

// 1. compile returns diagnostics
const bad = compile("main() -> int => 0"); // ok program
assert.equal(bad.diagnostics.length, 0);

// 2. type errors surface as diagnostics
const typed = compile('bad() -> int => "nope"');
assert.ok(
  typed.diagnostics.some((d) => d.msg.includes("expected int, found str")),
  "expected a type mismatch",
);
assert.equal(typed.markers.length, 1, "and one marker for the editor to draw");

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

// 6. the language server answers for a caret
const src =
  "fib(n: int) -> int => n < 2 ? n : fib(n - 1) + fib(n - 2)\n" +
  "fibs() -> int => 0\n" +
  "main() -> int => fib(3)\n";
const at = lsp(src, src.indexOf("fib(3)"));
assert.equal(at.hover, "fib(n: int) -> int", "hover at a call site is the signature");
assert.equal(at.definition.line, 1, "the definition is the line fib is defined on");
assert.equal(
  at.references.length,
  4,
  "the definition and its three call sites, and not the fibs that starts with it",
);
// A caret on an operator names no binding, and the answer is empty rather than
// absent: an editor asks about every caret it sees, including the ones between
// two things.
const nowhere = lsp("main() -> int => 0", "main() ".length);
assert.equal(nowhere.hover, "", "no identifier under the caret");
assert.equal(nowhere.definition, null, "and nothing to point at");

console.log("all macalang package tests passed ✓");
