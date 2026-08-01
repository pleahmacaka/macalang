// macalang — compile & import Maca from JavaScript.
//
// The whole Maca front-end (lexer → parser → type/effect checker → JS emitter)
// runs here as WebAssembly. `compile()` returns diagnostics + emitted code;
// `toJS()`/`loadModule()` turn a `.maca` source into callable JS.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dir = dirname(fileURLToPath(import.meta.url));

// What to run when the wasm is missing or older than the exports asked of it.
// It is a build artifact and the repository does not carry one, so "no such
// file" is the state of a fresh checkout rather than a broken install.
const BUILD = "maca run packages/macalang/build.maca";

let _ex = null;
function wasm() {
  if (_ex) return _ex;
  const at = join(__dir, "maca_wasm.wasm");
  let bytes;
  try {
    bytes = readFileSync(at);
  } catch {
    throw new Error(`maca: no compiler at ${at}. Build one with \`${BUILD}\`.`);
  }
  const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {});
  _ex = instance.exports;
  return _ex;
}

// One export taking a source buffer, called over linear memory: copy the source
// in, read the `(ptr << 32) | len` answer back out, free both.
//
// A wasm built before the export existed is the failure this names, because
// nothing else can: `maca_wasm.wasm` is not in the repository and an old one
// left over from a previous build answers every older call perfectly.
function callWithSource(name, src, arg) {
  const ex = wasm();
  if (typeof ex[name] !== "function") {
    throw new Error(
      `maca: this maca_wasm.wasm has no "${name}" export, so it is older than` +
        ` the compiler it came from. Rebuild it with \`${BUILD}\`.`,
    );
  }
  const mem = () => new Uint8Array(ex.memory.buffer);
  const enc = new TextEncoder().encode(src);
  const p = ex.alloc(enc.length);
  mem().set(enc, p);
  const packed = ex[name](p, enc.length, arg);
  const ptr = Number(packed >> 32n);
  const len = Number(packed & 0xffffffffn);
  const out = new TextDecoder().decode(mem().slice(ptr, ptr + len));
  ex.dealloc(ptr, len);
  ex.dealloc(p, enc.length);
  return out;
}

function run(src, mode) {
  return JSON.parse(callWithSource("run", src, mode));
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

/**
 * Every language-server answer for one caret, as
 * `{ hover, signature, definition, references }` (see `crates/wasm`'s
 * `lsp_json`). Positions are 1-based lines and UTF-16 columns, which is what an
 * editor wants; `signature` and `definition` are `null` when there is nothing
 * to say. One call rather than four because an editor asks all four questions
 * about the same caret and each of them would otherwise re-parse the file.
 * @param {string} src  Maca source
 * @param {number} [offset]  byte offset of the caret
 */
export function lsp(src, offset = 0) {
  return JSON.parse(callWithSource("lsp", src, offset));
}

export default { version, compile, toJS, toESM, loadModule, loadFile, lsp };
