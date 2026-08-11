// macalang: compile & import Maca from JavaScript.
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
const BUILD = "maca run apps/npm/build.maca";

let _mod = null;
function wasm() {
  if (_mod) return _mod;
  const at = join(__dir, "maca_wasm.wasm");
  let bytes;
  try {
    bytes = readFileSync(at);
  } catch {
    throw new Error(`maca: no compiler at ${at}. Build one with \`${BUILD}\`.`);
  }
  _mod = new WebAssembly.Module(bytes);
  return _mod;
}

// The compiler is a wasi command: `_start`, an argv, and a JSON line on stdout.
// So a call is one instance, and the seven imports below are the whole of the
// host wasi-libc reaches for. Nothing survives a call, which is the point: the
// old pointer ABI leaked a buffer per keystroke.
const done = {};
function ask(argv) {
  let out = "";
  let memory = null;
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  // argv[0] is the program name, which `args()` inside the wasm drops.
  const args = ["maca"].concat(argv).map((a) => {
    const b = enc.encode(a);
    const z = new Uint8Array(b.length + 1);
    z.set(b);
    return z;
  });
  const view = () => new DataView(memory.buffer);
  const wasi = {
    args_sizes_get(np, sp) {
      let n = 0;
      for (const a of args) n += a.length;
      view().setUint32(np, args.length, true);
      view().setUint32(sp, n, true);
      return 0;
    },
    args_get(pp, bp) {
      let at = bp;
      for (let i = 0; i < args.length; i++) {
        view().setUint32(pp + i * 4, at, true);
        new Uint8Array(memory.buffer).set(args[i], at);
        at += args[i].length;
      }
      return 0;
    },
    fd_write(fd, iov, n, wrote) {
      let sent = 0;
      for (let i = 0; i < n; i++) {
        const at = view().getUint32(iov + i * 8, true);
        const len = view().getUint32(iov + i * 8 + 4, true);
        if (fd === 1) out += dec.decode(new Uint8Array(memory.buffer, at, len));
        sent += len;
      }
      view().setUint32(wrote, sent, true);
      return 0;
    },
    fd_close: () => 0,
    fd_seek: () => 0,
    fd_fdstat_get: () => 0,
    proc_exit() {
      throw done;
    },
  };
  const at = new WebAssembly.Instance(wasm(), { wasi_snapshot_preview1: wasi });
  memory = at.exports.memory;
  try {
    at.exports._start();
  } catch (e) {
    if (e !== done) throw e;
  }
  return out;
}

function run(src, mode) {
  return JSON.parse(ask(["compile", String(mode | 0), src]));
}

/** Compiler version string. */
export function version() {
  return JSON.parse(ask(["version"]));
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
 * `{ hover, signature, definition, references }` (see `apps/npm/wasm.maca`'s
 * `pg_lsp`). Positions are 1-based lines and UTF-16 columns, which is what an
 * editor wants; `signature` and `definition` are `null` when there is nothing
 * to say. One call rather than four because an editor asks all four questions
 * about the same caret and each of them would otherwise re-parse the file.
 * @param {string} src  Maca source
 * @param {number} [offset]  byte offset of the caret
 */
export function lsp(src, offset = 0) {
  return JSON.parse(ask(["lsp", String(offset | 0), src]));
}

export default { version, compile, toJS, toESM, loadModule, loadFile, lsp };
