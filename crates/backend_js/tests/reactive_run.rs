use std::io::Write;
use std::process::Command;

/// A DOM that can be typed into and clicked, and that counts every repaint of a text node.
const DOM: &str = r#"let paints = 0;
function makeNode(tag) {
  return { tagName: String(tag).toUpperCase(), className: "", value: "", _a: {}, _on: {},
    children: [],
    setAttribute(k, v) { this._a[k] = v; }, removeAttribute(k) { delete this._a[k]; },
    addEventListener(k, f) { (this._on[k] = this._on[k] || []).push(f); },
    appendChild(c) { this.children.push(c); return c; } };
}
function makeText(t) {
  const n = { nodeType: 3, _t: String(t) };
  Object.defineProperty(n, "textContent", {
    get() { return this._t; },
    set(v) { paints += 1; this._t = String(v); },
  });
  return n;
}
const app = makeNode("div");
global.document = { activeElement: null, createElement: makeNode, createTextNode: makeText,
  getElementById: (id) => (id === "app" ? app : null) };
function view(n) {
  n = n || app;
  return n.nodeType === 3 ? String(n.textContent) : n.children.map(view).join("|");
}
function nodes(name, n) {
  n = n || app;
  const here = n.tagName === name.toUpperCase() ? [n] : [];
  return here.concat((n.children || []).flatMap((c) => nodes(name, c)));
}
function el(name, i) { return nodes(name)[i || 0]; }
function fire(node, event, value) {
  if (value !== undefined) node.value = value;
  for (const f of node._on[event] || []) f({ target: node, type: event });
  return "";
}
function counted(f) { paints = 0; f(); return paints; }
function err(f) { try { f(); return "no throw"; } catch (e) { return String(e.message); } }
"#;

/// Emit `src`, mount it into the DOM stub under Node, then evaluate each expression in `calls` and return one output line per expression.
fn run(src: &str, calls: &[&str]) -> Vec<String> {
    let (out, js) = node(src, calls);
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

/// The same, for a program that is supposed to fail: the message it died with.
fn fails(src: &str) -> String {
    let (out, js) = node(src, &[]);
    assert!(
        !out.status.success(),
        "the program was expected to fail\n--- js ---\n{js}"
    );
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn node(src: &str, calls: &[&str]) -> (std::process::Output, String) {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let js = maca_backend_js::emit(&p.module).js;

    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(src, calls), &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-js-react-{}-{key:x}", std::process::id()));
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
    (out, js)
}

/// One handler, one assignment, and a view that never asks to be repainted.
const ONE: &str = "\
count = 0

bump() { count = count + 1 }

main() -> Element =>
    div(
        button(onclick=bump, \"+\")
        span(\"{count}\")
    )
";

#[test]
fn a_handler_that_assigns_repaints_with_no_refresh_call() {
    let out = run(
        ONE,
        &[
            "view()",
            "(fire(el(\"button\"), \"click\"), view())",
            "(fire(el(\"button\"), \"click\"), view())",
            "state.count",
        ],
    );
    assert_eq!(out, ["+|0", "+|1", "+|2", "2"]);
}

/// Three names written in one handler, and a fourth node whose value comes from a call, so it repaints on any change at all.
const THREE: &str = "\
a = 0
b = 0
c = 0

tick() -> str => \"tick\"

go() {
    a = 1
    b = 2
    c = 3
}

main() -> Element =>
    div(
        button(onclick=go, \"go\")
        span(\"{a}\")
        span(\"{b}\")
        span(\"{c}\")
        span(tick())
    )
";

#[test]
fn a_handler_assigning_three_names_repaints_once() {
    let out = run(
        THREE,
        &[
            "counted(() => fire(el(\"button\"), \"click\"))",
            "view()",
            "counted(() => fire(el(\"button\"), \"click\"))",
            "counted(() => maca.set({ a: 5, b: 6, c: 7 }))",
            "view()",
        ],
    );
    assert_eq!(out[0], "4", "one pass paints each of the four nodes once");
    assert_eq!(out[1], "go|1|2|3|tick");
    assert_eq!(
        out[2], "0",
        "a second click writes the same three values, which is not a change"
    );
    assert_eq!(out[3], "4", "a batch from the bridge is one turn as well");
    assert_eq!(out[4], "go|5|6|7|tick");
}

#[test]
fn only_the_nodes_that_read_the_assigned_name_repaint() {
    let src = "\
a = 0
b = 0

tick() -> str => \"tick\"

only_a() { a = 7 }

main() -> Element =>
    div(
        button(onclick=only_a, \"go\")
        span(\"{a}\")
        span(\"{b}\")
        span(tick())
    )
";
    let out = run(
        src,
        &[
            "counted(() => fire(el(\"button\"), \"click\"))",
            "view()",
            "view(el(\"span\", 1))",
        ],
    );
    assert_eq!(out[0], "2", "`a` and the call, not `b`");
    assert_eq!(out[1], "go|7|0|tick");
    assert_eq!(out[2], "0");
}

/// A loop that assigns the same name on every iteration.
const LOOP: &str = "\
count = 0

tick() -> str => \"tick\"

fill() {
    i = 0
    while i < 5 {
        count = count + 1
        i = i + 1
    }
}

main() -> Element =>
    div(
        button(onclick=fill, \"go\")
        span(\"{count}\")
        span(tick())
    )
";

#[test]
fn an_assignment_inside_a_loop_repaints_once_when_the_loop_is_done() {
    let out = run(
        LOOP,
        &["counted(() => fire(el(\"button\"), \"click\"))", "view()"],
    );
    assert_eq!(out[0], "2", "five iterations are still one repaint");
    assert_eq!(out[1], "go|5|tick");
}

/// Every event name the platform spells without a colon.
const EVENTS: &str = "\
log = \"\"

main() -> Element =>
    div(
        button(onclick=(e => log = log ++ \"click \")
               oninput=(e => log = log ++ \"input \")
               ondragstart=(e => log = log ++ \"dragstart \")
               ondragover=(e => log = log ++ \"dragover \")
               ondragend=(e => log = log ++ \"dragend \")
               ondrop=(e => log = log ++ \"drop \")
               \"target\")
        span(\"{log}\")
    )
";

#[test]
fn each_vanilla_event_name_fires_and_the_view_follows() {
    let mut calls = Vec::new();
    for ev in ["click", "input", "dragstart", "dragover", "dragend", "drop"] {
        calls.push(format!("(fire(el(\"button\"), \"{ev}\"), view())"));
    }
    let refs: Vec<&str> = calls.iter().map(|s| s.as_str()).collect();
    let out = run(EVENTS, &refs);
    assert_eq!(
        out,
        [
            "target|click ",
            "target|click input ",
            "target|click input dragstart ",
            "target|click input dragstart dragover ",
            "target|click input dragstart dragover dragend ",
            "target|click input dragstart dragover dragend drop ",
        ]
    );
}

/// `value=` on a state name, and on a lambda that computes what to store.
const VALUE: &str = "\
who = \"world\"
age = 0

main() -> Element =>
    div(
        input(value=who)
        input(value=(v => age = int(v) * 2))
        span(\"{who}\")
        span(\"{age}\")
    )
";

#[test]
fn a_value_attribute_on_a_state_name_writes_back_and_repaints() {
    let out = run(
        VALUE,
        &[
            "view()",
            "(fire(el(\"input\"), \"input\", \"maca\"), view())",
            "state.who",
            "(fire(el(\"input\", 1), \"input\", \"21\"), view())",
            "state.age",
            "el(\"input\").value",
        ],
    );
    assert_eq!(out[0], "||world|0");
    assert_eq!(out[1], "||maca|0");
    assert_eq!(out[2], "maca");
    assert_eq!(out[3], "||maca|42");
    assert_eq!(out[4], "42");
    assert_eq!(out[5], "maca", "the bound property follows the state");
}

#[test]
fn a_value_that_names_nothing_writable_is_an_ordinary_attribute() {
    let src = "\
const Fixed = \"fixed\"

main() -> Element => div(input(value=\"literal\") input(value=Fixed))
";
    let out = run(
        src,
        &[
            "JSON.stringify(el(\"input\")._a)",
            "JSON.stringify(el(\"input\", 1)._a)",
            "el(\"input\")._on.input === undefined",
            "el(\"input\", 1)._on.input === undefined",
        ],
    );
    assert_eq!(out[0], "{\"value\":\"literal\"}");
    assert_eq!(
        out[1], "{\"value\":\"fixed\"}",
        "a constant is read, not bound"
    );
    assert_eq!(out[2], "true", "an attribute must not listen for input");
    assert_eq!(out[3], "true", "a constant must not listen for input");
}

#[test]
fn a_write_through_a_state_object_repaints_what_reads_it() {
    let src = "\
p = 0
n = 0

import js \"\"\"
maca.set(\"p\", { x: 1 })
\"\"\"

grow() { p.x = 9 }

main() -> Element =>
    div(
        button(onclick=grow, \"go\")
        span(\"{p.x}\")
        span(\"{n}\")
    )
";
    let out = run(
        src,
        &[
            "view()",
            "counted(() => fire(el(\"button\"), \"click\"))",
            "view()",
        ],
    );
    assert_eq!(out[0], "go|1|0");
    assert_eq!(out[1], "1", "the node reading p.x, and nothing else");
    assert_eq!(out[2], "go|9|0");
}

#[test]
fn the_old_directive_spelling_still_binds_and_handles() {
    let src = "\
who = \"world\"
count = 0

bump() { count = count + 1 }

main() -> Element =>
    div(
        button(on:click=bump, \"+\")
        input(bind:value=who)
        span(\"{who}\")
        span(\"{count}\")
    )
";
    let out = run(
        src,
        &[
            "(fire(el(\"button\"), \"click\"), view())",
            "(fire(el(\"input\"), \"input\", \"maca\"), view())",
        ],
    );
    assert_eq!(out[0], "+||world|1");
    assert_eq!(out[1], "+||maca|1");
}

/// The case assignment cannot see: a value that lives outside Maca entirely.
const OUTSIDE: &str = "\
tick() -> str

main() -> Element => div(span(tick()))

import js \"\"\"
let hidden = 0;
maca.provide({ tick: () => \"n\" + hidden });
globalThis.moveIt = () => { hidden += 1; };
\"\"\"
";

#[test]
fn refresh_is_still_how_something_outside_maca_reaches_the_view() {
    let out = run(
        OUTSIDE,
        &[
            "view()",
            "(moveIt(), view())",
            "(maca.refresh(), view())",
            "(moveIt(), update(), view())",
        ],
    );
    assert_eq!(out[0], "n0");
    assert_eq!(
        out[1], "n0",
        "nothing in Maca changed, so nothing repainted"
    );
    assert_eq!(out[2], "n1");
    assert_eq!(out[3], "n2");
}

/// Every shape of top-level binding a page actually writes: a call, a constructor, a record, a list, and a name that reads another binding.
const INITIALISERS: &str = "\
Mode = Dark | Light

Panel = {
    title: str
    open: bool
}

const Home = \"home\"

blank() -> Panel => Panel { title = Home, open = true }

theme = Dark
panel = blank()
tabs = [Home, \"work\"]
greeting = \"hi {Home}\"
width = 2 * 3

label() -> str =>
    match theme {
        Dark => \"dark\"
        Light => \"light\"
    }

main() -> Element =>
    div(
        span(label())
        span(panel.title)
        span(\"{panel.open}\")
        span(\"{tabs.length()}\")
        span(greeting)
        span(\"{width}\")
    )
";

#[test]
fn a_top_level_binding_computed_by_a_call_is_not_null() {
    let out = run(
        INITIALISERS,
        &[
            "view()",
            "state.panel.title",
            "JSON.stringify(state.tabs)",
            "state.theme.$",
        ],
    );
    assert_eq!(out[0], "dark|home|true|2|hi home|6");
    assert_eq!(out[1], "home");
    assert_eq!(out[2], "[\"home\",\"work\"]");
    assert_eq!(out[3], "Dark", "a constructor is the value, not null");
}

#[test]
fn a_computed_binding_stays_reactive_after_the_page_starts() {
    let out = run(
        INITIALISERS,
        &[
            "(maca.set(\"panel\", { title: \"work\", open: false }), view())",
            "(maca.set(\"width\", 9), view())",
        ],
    );
    assert_eq!(out[0], "dark|work|false|2|hi home|6");
    assert_eq!(out[1], "dark|work|false|2|hi home|9");
}

#[test]
fn a_view_that_assigns_state_says_so_instead_of_looping_forever() {
    let src = "\
count = 0

grow() -> str {
    count = count + 1
    \"{count}\"
}

main() -> Element => div(span(grow()))
";
    let out = fails(src);
    assert!(
        out.contains("keeps writing state"),
        "a bind that writes state should name itself: {out}"
    );
}
