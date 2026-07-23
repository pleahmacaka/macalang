"use strict";
// Host shim for the Maca-authored playground (playground.maca → app.js).
//
// The Maca app owns the UI, state, and control flow; this file provides the
// browser-API glue Maca can't express itself: instantiating the WebAssembly
// compiler, reading its packed return value out of linear memory, and holding
// the last result so the app's `mc*` getters can read it. Think of it as the
// playground's runtime, the way `maca-runtime` is the native runtime.
(function () {
  let ex = null;       // wasm exports
  let last = null;     // last compile result (parsed JSON)
  let lastMs = 0;      // wall-clock of the last compile+run
  let version = "";
  let statusText = "loading…";

  // ---- example programs (source lives here, not in the Maca file) ----------
  const EXAMPLES = {
    hello:    "main() -> int {\n    info(\"Hello from Maca\")\n    0\n}\n",
    tree:     "// Recursive sum types — self-referential payloads are boxed.\nTree = Leaf(int) | Node(Tree, Tree)\n\ntotal(t: Tree) -> int {\n    match t {\n        Leaf(n) => n\n        Node(l, r) => total(l) + total(r)\n    }\n}\n\nmain() -> int {\n    let t = Node(Leaf(1), Node(Leaf(2), Leaf(3)))\n    info(\"{total(t)}\")\n    0\n}\n",
    indexing: "// Subscripting, lvalue assignment, and functional record update.\nConfig = {\n    host: str\n    port: int\n}\n\nmain() -> int {\n    let xs = 10, 20, 30\n    info(\"{xs[1]} of {len(xs)}\")\n    xs[0] = 99\n    info(\"{xs[0]}\")\n    let base = Config { host = \"localhost\", port = 80 }\n    let secure = base with { port = 443 }\n    info(\"{base.port} -> {secure.port}\")\n    0\n}\n",
    config:   "networking.hostName = \"rigel\"\nsystem.stateVersion = \"24.11\"\n\nservices.openssh = {\n    passwordAuthentication = false\n}\n",
  };

  // ---- wasm bridge ---------------------------------------------------------
  function b64ToBytes(b64) {
    const bin = atob(b64.trim());
    const arr = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
    return arr;
  }
  function mem() { return new Uint8Array(ex.memory.buffer); }
  function readPacked(packed) {
    const ptr = Number(packed >> 32n), len = Number(packed & 0xffffffffn);
    const out = new TextDecoder().decode(mem().slice(ptr, ptr + len));
    ex.dealloc(ptr, len);
    return out;
  }
  function wasmRun(src, mode) {
    const bytes = new TextEncoder().encode(src);
    const p = ex.alloc(bytes.length);
    mem().set(bytes, p);
    const json = readPacked(ex.run(p, bytes.length, mode));
    ex.dealloc(p, bytes.length);
    return JSON.parse(json);
  }

  // ---- content helpers -----------------------------------------------------
  function diagText(r) {
    const pe = r.parseErrors || [], di = r.diagnostics || [];
    if (!pe.length && !di.length) return "✓ parsed and type-checked clean";
    const lines = [];
    for (const e of pe) lines.push("parse: " + e);
    for (const d of di) lines.push(d.kind + ": " + d.msg);
    return lines.join("\n");
  }
  function fmtN(n) {
    if (n >= 1e9) return (n / 1e9).toFixed(2) + "B";
    if (n >= 1e6) return (n / 1e6).toFixed(2) + "M";
    if (n >= 1e3) return (n / 1e3).toFixed(1) + "k";
    return String(n);
  }

  // ---- API consumed by the Maca app (global `mc*` functions) ---------------
  window.mcCompile = function (src, mode) {
    if (!ex) { statusText = "loading…"; return; }
    try {
      const t0 = performance.now();
      last = wasmRun(src, mode | 0);
      lastMs = performance.now() - t0;
      const n = (last.parseErrors || []).length + (last.diagnostics || []).length;
      statusText = n ? (n + " diagnostic" + (n > 1 ? "s" : "")) : "compiled clean";
    } catch (e) {
      last = { parseErrors: ["internal: " + e.message], diagnostics: [], outputs: {} };
      statusText = "error";
    }
  };
  window.mcTab = function (tab) {
    if (!last) return "";
    if (tab === "Diagnostics") return diagText(last);
    if (tab === "Console") {
      const run = last.run;
      if (!run) return "(config mode — nothing to run)";
      let body = run.output || "";
      if (run.error) body += (body && !body.endsWith("\n") ? "\n" : "") + "⚠ " + run.error;
      return body || "(no output)";
    }
    return (last.outputs && last.outputs[tab]) || "(nothing emitted)";
  };
  window.mcSummary = function () {
    const p = last && last.run && last.run.profile;
    if (!p) return "run a program to profile it";
    return `${lastMs.toFixed(lastMs < 10 ? 2 : 0)} ms · ${fmtN(p.steps)} steps · ${fmtN(p.totalCalls)} calls · depth ${fmtN(p.maxDepth)}`;
  };
  window.mcFlame = function () {
    const p = last && last.run && last.run.profile;
    return (p && p.flameSvg) || "";
  };
  window.mcStatus = function () { return statusText; };
  window.mcVersion = function () { return version ? "maca " + version + " · wasm" : ""; };
  window.mcExample = function (name) { return EXAMPLES[name] || ""; };

  // ---- boot: load the wasm compiler, then let the Maca app render ----------
  (async function boot() {
    try {
      const bytes = b64ToBytes(document.getElementById("wasm-b64").textContent);
      const { instance } = await WebAssembly.instantiate(bytes, {});
      ex = instance.exports;
      version = readPacked(ex.version());
      statusText = "ready";
      // run() and update() are globals defined by the Maca-compiled app.js.
      if (typeof run === "function") run();
      else if (typeof update === "function") update();
    } catch (e) {
      statusText = "wasm failed: " + e.message;
      if (typeof update === "function") update();
    }
  })();
})();
