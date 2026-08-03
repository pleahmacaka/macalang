//! The JS bridge, executed rather than read.
//!
//! An `import js` block used to talk to Maca through two names nothing had
//! promised it: `state`, this generator's own local, and `update()`. It also had
//! to write `window.f = …` for every function the Maca side declared, because a
//! Maca call lowers to a bare call that resolves on the global object. The
//! `maca` object is what replaces both, so what these tests assert is what the
//! program computes with it: a block writing state and the view seeing it, Maca
//! calling a function the block supplied, and a name nobody declared being
//! rejected instead of quietly doing nothing.
//!
//! An emitted-text assertion cannot see any of that. `state.form_titel = x` is
//! plausible text and a silent no-op at run time, which is the whole bug.

use std::io::Write;
use std::process::Command;

/// A DOM small enough to mount into and read back: elements, text nodes, and
/// the `textContent` a reactive bind writes.
const DOM: &str = "\
function makeNode(tag) {\n\
  return { tagName: String(tag).toUpperCase(), className: \"\", _a: {}, children: [],\n\
    setAttribute(k, v) { this._a[k] = v; }, removeAttribute(k) { delete this._a[k]; },\n\
    addEventListener() {}, appendChild(c) { this.children.push(c); return c; } };\n\
}\n\
const app = makeNode(\"div\");\n\
global.document = { createElement: makeNode,\n\
  createTextNode: (t) => ({ nodeType: 3, textContent: t }),\n\
  getElementById: (id) => (id === \"app\" ? app : null) };\n\
// The text of the mounted view, flattened, so a test can ask what the reader\n\
// would see rather than which node holds it.\n\
function view(n) {\n\
  n = n || app;\n\
  return n.nodeType === 3 ? String(n.textContent) : n.children.map(view).join(\"|\");\n\
}\n\
// A throw is an answer here, so report the message the same way as a value.\n\
function err(f) { try { f(); return \"no throw\"; } catch (e) { return String(e.message); } }\n";

/// Emit `src`, mount it into the DOM stub under Node, then evaluate each
/// expression in `calls` and return one output line per expression.
fn run(src: &str, calls: &[&str]) -> Vec<String> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let js = maca_backend_js::emit(&p.module).js;

    // Tests in one binary share a process id, so the directory is keyed on the
    // program itself; otherwise concurrent cases overwrite each other's module.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(src, calls), &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-js-bridge-{}-{key:x}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::File::create(dir.join("app.js"))
        .unwrap()
        .write_all(js.as_bytes())
        .unwrap();

    let mut d = String::from(DOM);
    d.push_str("const m = require(\"./app.js\");\nwith (m) {\n");
    for c in calls {
        d.push_str(&format!("  console.log(String({c}));\n"));
    }
    d.push_str("}\n");
    std::fs::File::create(dir.join("run.js"))
        .unwrap()
        .write_all(d.as_bytes())
        .unwrap();

    let out = Command::new("node")
        .arg(dir.join("run.js"))
        .output()
        .expect("node is required for the JS backend tests");
    assert!(
        out.status.success(),
        "node failed\n--- stderr ---\n{}\n--- js ---\n{js}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect()
}

/// A view over one piece of state, plus a block that writes it two ways: once
/// while the module loads, and once from a function the test calls later.
const TITLE: &str = "\
title = \"none\"

import js \"\"\"
maca.set(\"title\", \"boot\")
globalThis.later = (t) => maca.set(\"title\", t)
globalThis.read = () => maca.get(\"title\")
\"\"\"

main() -> Element => div(span(title))
";

#[test]
fn a_block_writes_declared_state_and_the_view_sees_it() {
    let out = run(
        TITLE,
        &[
            // written before the app mounted: the first render already has it
            "view()",
            "read()",
            // written afterwards: `set` refreshes, so the bound node follows
            "(later(\"open\"), view())",
            "read()",
        ],
    );
    assert_eq!(out, ["boot", "boot", "open", "open"]);
}

/// A function declared in Maca and implemented by the block, called from both
/// sides: by the view as it mounts, and by the test through the Maca function
/// that wraps it.
const HOST: &str = "\
ask(who: str) -> str

greet(who: str) -> str => \"hi \" ++ ask(who)

import js \"\"\"
maca.provide({ ask: (w) => w.toUpperCase() })
\"\"\"

main() -> Element => div(greet(\"you\"))
";

#[test]
fn maca_calls_a_function_the_block_supplied() {
    let out = run(HOST, &["greet(\"bob\")", "view()"]);
    // The view is built by `mount()` at load, which is after the block ran, so
    // the implementation is in place by the time the first render calls it.
    assert_eq!(out, ["hi BOB", "hi YOU"]);
}

#[test]
fn an_unimplemented_host_function_names_itself() {
    // The same declarations with nothing provided, and a view that does not
    // call it (an unimplemented call during `mount()` fails the whole load,
    // which is a different report). Without the stub this was a bare call to a
    // global nobody assigned: `ask is not defined`, thrown from inside `greet`,
    // naming neither the boundary nor what to do about it.
    let src = "\
ask(who: str) -> str

greet(who: str) -> str => \"hi \" ++ ask(who)

main() -> Element => div(span(\"idle\"))
";
    let out = run(src, &["err(() => greet(\"bob\"))"]);
    assert!(
        out[0].contains("`ask` is declared in Maca") && out[0].contains("maca.provide"),
        "unhelpful message: {out:?}"
    );
}

/// Two names and one of them a constant, so every rejection has a near miss
/// beside it that must still work.
const DECLARED: &str = "\
title = \"none\"
Limit = 3

import js \"\"\"
globalThis.probe = {
  setTypo: () => maca.set(\"form_titel\", \"x\"),
  getTypo: () => maca.get(\"nope\"),
  setConst: () => maca.set(\"Limit\", 9),
  provideTypo: () => maca.provide({ nope: () => 1 }),
  setBatch: (o) => maca.set(o),
};
\"\"\"

main() -> Element => div(span(title))
";

#[test]
fn a_name_the_program_never_declared_is_rejected() {
    let out = run(
        DECLARED,
        &[
            "err(probe.setTypo)",
            "err(probe.getTypo)",
            "err(probe.setConst)",
            "err(probe.provideTypo)",
            // the typo must not have written anything on its way to throwing
            "JSON.stringify(state)",
        ],
    );
    assert!(
        out[0].contains("form_titel") && out[0].contains("title"),
        "set of an undeclared name: {out:?}"
    );
    assert!(
        out[1].contains("nope"),
        "get of an undeclared name: {out:?}"
    );
    assert!(
        out[2].contains("`Limit` is a constant"),
        "set of a constant: {out:?}"
    );
    assert!(
        out[3].contains("nope") && out[3].contains("not declared"),
        "provide of an undeclared function: {out:?}"
    );
    assert_eq!(out[4], "{\"title\":\"none\",\"Limit\":3}");
}

#[test]
fn a_batch_set_checks_every_name_before_writing_any() {
    // Half-applying it would be worse than refusing: the view would show a
    // state no program ever produced, and the throw would look unrelated to it.
    let out = run(
        DECLARED,
        &[
            "err(() => probe.setBatch({ title: \"ok\", typo: 1 }))",
            "view()",
            "(probe.setBatch({ title: \"both\" }), view())",
        ],
    );
    assert!(out[0].contains("typo"), "batch set: {out:?}");
    assert_eq!(out[1], "none", "a rejected batch wrote anyway: {out:?}");
    assert_eq!(out[2], "both");
}

#[test]
fn the_old_state_and_update_names_still_work() {
    // Kept working on purpose: `apps/playground/playground.maca` calls
    // `update()` from its own block, and the driver's harness reads `state`.
    // The bridge is what a new program should reach for, not a wall the
    // existing ones hit.
    let out = run(
        TITLE,
        &[
            "(state.title = \"raw\", update(), view())",
            "maca.get(\"title\")",
        ],
    );
    assert_eq!(out, ["raw", "raw"]);
}
