/* Maca playground: Monaco editor + the compiler front-end compiled to wasm.
 * Monaco and `maca_wasm.wasm` are both served locally (no CDN at runtime) —
 * run `./build.sh` to vendor Monaco and build the wasm, then serve this folder. */

'use strict';

// Monaco is vendored locally by build.sh (playground/vendor/vs); no CDN at runtime.
const VS = './vendor/vs';

// ---- embedded examples ----------------------------------------------------
const EXAMPLES = {
  'hello.maca (program)': {
    mode: 0,
    src: `main() -> int {\n    info("Hello, World")\n    0\n}\n`,
  },
  'generic.maca (program)': {
    mode: 0,
    src: `// Polymorphic identity: each use instantiates fresh type variables.\nid(x: a) -> a => x\n\npick(cond: bool, x: a, y: a) -> a =>\n    cond ? x : y\n\nmain() -> int {\n    let n: int = id(5)\n    let s: str = id("hello")\n    let m: int = pick(true, n, 7)\n    info(s)\n    m\n}\n`,
  },
  'taskr.maca (program)': {
    mode: 0,
    src: `Status = Todo | Doing | Done\n\nTask = {\n    id:     int\n    title:  str\n    status: Status\n}\n\nrender(t: Task) -> str {\n    let box = t.status == Done ? "[x]" : "[ ]"\n    "{box} #{t.id}  {t.title}"\n}\n\nmain() -> int {\n    let t = Task { id = 1, title = "ship", status = Todo }\n    info(render(t))\n    0\n}\n`,
  },
  'bad: type mismatch': {
    mode: 0,
    src: `// The declared return type is int, but the body is a string.\nbad() -> int => "not an int"\n`,
  },
  'bad: arity': {
    mode: 0,
    src: `greet(name: str) -> str => "hi {name}"\n\nmain() -> int {\n    greet("ada", "extra")\n    0\n}\n`,
  },
  'system.maca (config)': {
    mode: 1,
    src: `networking.hostName = "rigel"\nsystem.stateVersion = "24.11"\n\nservices.openssh = {\n    passwordAuthentication = false\n}\n`,
  },
  'bad: effect in config': {
    mode: 1,
    src: `// Config must be pure; logging is an io effect.\ngreeting = info("hi")\n`,
  },
};

// ---- wasm bridge ----------------------------------------------------------
class MacaWasm {
  constructor(exports) {
    this.ex = exports;
    this.dec = new TextDecoder();
    this.enc = new TextEncoder();
  }
  static async load(url) {
    const resp = await fetch(url);
    if (!resp.ok) throw new Error(`fetch ${url}: ${resp.status}`);
    const bytes = await resp.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new MacaWasm(instance.exports);
  }
  _mem() { return new Uint8Array(this.ex.memory.buffer); }
  _readPacked(packed) {
    const ptr = Number(packed >> 32n);
    const len = Number(packed & 0xffffffffn);
    const out = this.dec.decode(this._mem().slice(ptr, ptr + len));
    this.ex.dealloc(ptr, len);
    return out;
  }
  version() { return this._readPacked(this.ex.version()); }
  run(src, mode) {
    const bytes = this.enc.encode(src);
    const p = this.ex.alloc(bytes.length);
    this._mem().set(bytes, p);
    const packed = this.ex.run(p, bytes.length, mode);
    const json = this._readPacked(packed);
    this.ex.dealloc(p, bytes.length);
    return JSON.parse(json);
  }
}

// ---- UI --------------------------------------------------------------------
const $ = (id) => document.getElementById(id);
let editor = null;
let wasm = null;
let currentTab = 'Diagnostics';
let lastResult = null;

function setStatus(msg) { $('status').textContent = msg; }

function renderTabs(result) {
  const tabsEl = $('tabs');
  tabsEl.innerHTML = '';
  const diagCount = (result?.parseErrors?.length || 0) + (result?.diagnostics?.length || 0);
  const names = ['Diagnostics'];
  if (result?.outputs) names.push(...Object.keys(result.outputs));

  for (const name of names) {
    const tab = document.createElement('div');
    tab.className = 'tab' + (name === currentTab ? ' active' : '');
    tab.textContent = name;
    if (name === 'Diagnostics') {
      const dot = document.createElement('span');
      dot.className = 'dot ' + (diagCount ? 'err' : 'ok');
      tab.appendChild(dot);
    }
    tab.onclick = () => { currentTab = name; showTab(result); };
    tabsEl.appendChild(tab);
  }
  if (!names.includes(currentTab)) currentTab = 'Diagnostics';
}

function showTab(result) {
  renderTabs(result);
  const diagPanel = $('diagnostics');
  const outPanel = $('output');
  diagPanel.classList.toggle('active', currentTab === 'Diagnostics');
  outPanel.classList.toggle('active', currentTab !== 'Diagnostics');

  if (currentTab === 'Diagnostics') {
    renderDiagnostics(result);
  } else {
    outPanel.textContent = result?.outputs?.[currentTab] ?? '';
  }
}

function renderDiagnostics(result) {
  const el = $('diagnostics');
  el.innerHTML = '';
  const parseErrors = result?.parseErrors || [];
  const diags = result?.diagnostics || [];

  if (!parseErrors.length && !diags.length) {
    const ok = document.createElement('div');
    ok.className = 'empty ok';
    ok.textContent = '✓ no errors — parsed and type-checked clean';
    el.appendChild(ok);
    return;
  }
  for (const e of parseErrors) el.appendChild(diagRow('parse', 'ParseError', e));
  for (const d of diags) el.appendChild(diagRow('error', d.kind, d.msg));
}

function diagRow(cls, kind, msg) {
  const row = document.createElement('div');
  row.className = 'diag ' + cls;
  const k = document.createElement('span');
  k.className = 'kind';
  k.textContent = kind;
  const m = document.createElement('span');
  m.className = 'msg';
  m.textContent = msg;
  row.append(k, m);
  return row;
}

function run() {
  if (!wasm || !editor) return;
  const src = editor.getValue();
  const mode = parseInt($('mode').value, 10);
  try {
    lastResult = wasm.run(src, mode);
    setStatus('compiled');
  } catch (err) {
    setStatus('error: ' + err.message);
    lastResult = { parseErrors: ['internal: ' + err.message], diagnostics: [], outputs: {} };
  }
  showTab(lastResult);
}

let debounce = null;
function scheduleRun() {
  clearTimeout(debounce);
  debounce = setTimeout(run, 300);
}

function loadExample(name) {
  const ex = EXAMPLES[name];
  if (!ex) return;
  $('mode').value = String(ex.mode);
  editor.setValue(ex.src);
  run();
}

// ---- boot ------------------------------------------------------------------
require.config({ paths: { vs: VS } });
const VS_ABS = new URL(VS + '/', document.baseURI).href;
self.MonacoEnvironment = {
  getWorkerUrl: () => URL.createObjectURL(new Blob(
    [`self.MonacoEnvironment={baseUrl:'${VS_ABS}'};importScripts('${VS_ABS}base/worker/workerMain.js');`],
    { type: 'text/javascript' },
  )),
};

require(['vs/editor/editor.main'], async () => {
  const { registerMaca } = await import('./maca-lang.js');
  registerMaca(monaco);

  // example dropdown
  const sel = $('example');
  for (const name of Object.keys(EXAMPLES)) {
    const opt = document.createElement('option');
    opt.value = name; opt.textContent = name;
    sel.appendChild(opt);
  }

  editor = monaco.editor.create($('editor'), {
    value: EXAMPLES['hello.maca (program)'].src,
    language: 'maca',
    theme: 'maca-dark',
    fontSize: 14,
    minimap: { enabled: false },
    scrollBeyondLastLine: false,
    automaticLayout: true,
    tabSize: 4,
  });

  editor.onDidChangeModelContent(scheduleRun);
  editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, run);

  sel.onchange = () => loadExample(sel.value);
  $('mode').onchange = run;
  $('run').onclick = run;
  $('theme').onchange = () => monaco.editor.setTheme($('theme').value);

  try {
    wasm = await MacaWasm.load('./maca_wasm.wasm');
    $('version').textContent = 'maca ' + wasm.version() + ' · wasm';
    setStatus('ready');
    run();
  } catch (err) {
    setStatus('wasm not loaded — run ./build.sh first (' + err.message + ')');
  }
});
