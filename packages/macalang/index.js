// macalang — compile & import Maca from JavaScript.
//
// The whole Maca front-end (lexer → parser → type/effect checker → JS emitter)
// runs here as WebAssembly. `compile()` returns diagnostics + emitted code;
// `toJS()`/`loadModule()` turn a `.maca` source into callable JS.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dir = dirname(fileURLToPath(import.meta.url));

let _ex = null;
function wasm() {
  if (_ex) return _ex;
  const bytes = readFileSync(join(__dir, "maca_wasm.wasm"));
  const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {});
  _ex = instance.exports;
  return _ex;
}

function run(src, mode) {
  const ex = wasm();
  const mem = () => new Uint8Array(ex.memory.buffer);
  const enc = new TextEncoder().encode(src);
  const p = ex.alloc(enc.length);
  mem().set(enc, p);
  const packed = ex.run(p, enc.length, mode);
  const ptr = Number(packed >> 32n);
  const len = Number(packed & 0xffffffffn);
  const out = new TextDecoder().decode(mem().slice(ptr, ptr + len));
  ex.dealloc(ptr, len);
  ex.dealloc(p, enc.length);
  return JSON.parse(out);
}

/** Compiler version string. */
export function version() {
  const ex = wasm();
  const packed = ex.version();
  const ptr = Number(packed >> 32n);
  const len = Number(packed & 0xffffffffn);
  const s = new TextDecoder().decode(new Uint8Array(ex.memory.buffer).slice(ptr, ptr + len));
  ex.dealloc(ptr, len);
  return s;
}

/**
 * Parse + type/effect check + emit. Returns
 * `{ parseErrors, diagnostics, outputs, jsExports }`.
 * @param {string} src  Maca source
 * @param {{mode?: 0|1}} [opts]  0 = program (default), 1 = config
 */
export function compile(src, { mode = 0 } = {}) {
  const r = run(src, mode);
  if (r.parseErrors.length) {
    throw new SyntaxError("maca: " + r.parseErrors.join("; "));
  }
  return r;
}

/** Maca source → emitted JavaScript (program mode). Throws on parse errors. */
export function toJS(src, opts) {
  return compile(src, opts).outputs.JS || "";
}

/** Emitted JS, re-exported as ESM so a bundler/loader can `export` the fns. */
export function toESM(src, opts) {
  const r = compile(src, opts);
  const js = r.outputs.JS || "";
  const names = r.jsExports || [];
  const exportLine = names.length ? `\nexport { ${names.join(", ")} };\n` : "";
  return js + exportLine;
}

/**
 * Compile `src` and return its top-level functions as a live module object.
 * @returns {Record<string, Function>}
 */
export function loadModule(src, opts) {
  const js = toJS(src, opts);
  const module = { exports: {} };
  // eslint-disable-next-line no-new-func
  new Function("module", "exports", "document", js)(module, module.exports, undefined);
  return module.exports;
}

/** Read a `.maca` file and load its functions. */
export function loadFile(path, opts) {
  return loadModule(readFileSync(path, "utf8"), opts);
}

export default { version, compile, toJS, toESM, loadModule, loadFile };
