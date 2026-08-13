const key = "article:en/01-one";
const node = { innerHTML: "", textContent: "", value: "",
  getAttribute: k => k === "data-signal-as" ? "html" : null,
  setAttribute: () => {} };
global.document = { querySelectorAll: s =>
  s === "[data-signal=\"" + key + "\"]" ? [node] : [] };
global.window = global;
__RUNTIME__
const ops = [{ key, value: "<p>patched</p>" }];
window.macaSignal(ops);
console.log(JSON.stringify({ ok: node.innerHTML === ops[0].value,
                             text: node.textContent }));
