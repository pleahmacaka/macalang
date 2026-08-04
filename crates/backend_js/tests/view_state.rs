use std::io::Write;
use std::process::Command;

/// A DOM that counts every repaint, both in total and per text node, so two views can be told apart by which one moved.
const DOM: &str = r#"let paints = 0;
function makeNode(tag) {
  return { nodeType: 1, tagName: String(tag).toUpperCase(), className: "", value: "",
    _a: {}, _on: {}, children: [],
    setAttribute(k, v) { this._a[k] = v; }, removeAttribute(k) { delete this._a[k]; },
    addEventListener(k, f) { (this._on[k] = this._on[k] || []).push(f); },
    appendChild(c) { this.children.push(c); return c; } };
}
function makeText(t) {
  const n = { nodeType: 3, _t: String(t), _p: 0 };
  Object.defineProperty(n, "textContent", {
    get() { return this._t; },
    set(v) { paints += 1; this._p += 1; this._t = String(v); },
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
function texts(n) {
  n = n || app;
  return n.nodeType === 3 ? [n] : (n.children || []).flatMap(texts);
}
function fire(node, event, value) {
  if (value !== undefined) node.value = value;
  for (const f of node._on[event] || []) f({ target: node, type: event });
  return "";
}
function counted(f) { paints = 0; f(); return paints; }
function countedIn(node, f) { const was = node._p; f(); return node._p - was; }
"#;

/// Emit `src`, mount it into the DOM stub under Node, then evaluate each expression in `calls` and return one output line per expression.
fn run(src: &str, calls: &[&str]) -> Vec<String> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let js = maca_backend_js::emit(&p.module).js;

    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(src, calls), &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-js-view-{}-{key:x}", std::process::id()));
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

/// The shape this is all about: the state belongs to the view, and the handler that writes it is nested inside it.
const BOARD: &str = "\
board() -> Element {
    grip = 0

    grab() {
        grip = grip + 1
    }

    div(button(onclick=grab, \"grab\") span(\"{grip}\"))
}

main() -> Element => div(board())
";

#[test]
fn a_local_a_nested_handler_writes_repaints_the_view_that_owns_it() {
    let out = run(
        BOARD,
        &[
            "view()",
            "(fire(el(\"button\"), \"click\"), view())",
            "counted(() => fire(el(\"button\"), \"click\"))",
            "view()",
        ],
    );
    assert_eq!(out[0], "grab|0");
    assert_eq!(out[1], "grab|1", "the handler's write reached the view");
    assert_eq!(out[2], "1", "one node reads it, so one node repaints");
    assert_eq!(out[3], "grab|2");
}

/// The same view twice: each call owns its own state.
const TWO: &str = "\
counter(tag: str) -> Element {
    n = 0

    bump() {
        n = n + 1
    }

    div(button(onclick=bump, tag) span(\"{n}\"))
}

main() -> Element => div(counter(\"a\") counter(\"b\"))
";

#[test]
fn two_instances_of_a_view_do_not_share_their_state() {
    let out = run(
        TWO,
        &[
            "view()",
            "(fire(el(\"button\"), \"click\"), fire(el(\"button\"), \"click\"), view())",
            "(fire(el(\"button\", 1), \"click\"), view())",
            "counted(() => fire(el(\"button\"), \"click\"))",
            "countedIn(texts()[3], () => fire(el(\"button\"), \"click\"))",
        ],
    );
    assert_eq!(out[0], "a|0|b|0");
    assert_eq!(out[1], "a|2|b|0", "the second instance did not move");
    assert_eq!(out[2], "a|2|b|1");
    assert_eq!(out[3], "1", "one write, one node");
    assert_eq!(out[4], "0", "and it is never the other instance's node");
}

/// One local a handler writes, one it never touches.
const CHEAP: &str = "\
card() -> Element {
    title = \"card\"
    n = 0

    bump() {
        n = n + 1
    }

    div(button(onclick=bump, \"go\") span(title) span(\"{n}\"))
}

main() -> Element => div(card())
";

#[test]
fn a_view_local_nothing_writes_costs_no_binding() {
    let out = run(
        CHEAP,
        &[
            "view()",
            "counted(() => update())",
            "counted(() => fire(el(\"button\"), \"click\"))",
            "view()",
        ],
    );
    assert_eq!(out[0], "go|card|0");
    assert_eq!(
        out[1], "1",
        "only the local a handler writes is worth watching"
    );
    assert_eq!(out[2], "1");
    assert_eq!(out[3], "go|card|1");
}

/// Three locals written in one handler, and a fourth node whose value comes from a call.
const BATCH: &str = "\
panel() -> Element {
    a = 0
    b = 0
    c = 0

    go() {
        a = 1
        b = 2
        c = 3
    }

    div(button(onclick=go, \"go\") span(\"{a}\") span(\"{b}\") span(\"{c}\") span(tick()))
}

tick() -> str => \"tick\"

main() -> Element => div(panel())
";

#[test]
fn a_handler_writing_three_locals_repaints_once() {
    let out = run(
        BATCH,
        &[
            "counted(() => fire(el(\"button\"), \"click\"))",
            "view()",
            "counted(() => fire(el(\"button\"), \"click\"))",
        ],
    );
    assert_eq!(out[0], "4", "one pass, each of the four nodes once");
    assert_eq!(out[1], "go|1|2|3|tick");
    assert_eq!(out[2], "0", "the same three values are not a change");
}

/// The owner's shape: a sum type held by the view, read through a `match`.
const GRIP: &str = "\
Grip = Idle | Holding(int)

board() -> Element {
    grip = Idle

    grab(index: int) {
        grip = Holding(index)
    }

    div(
        button(ondragstart=(e => grab(2)) ondragend=(e => grab(0)) \"cell\")
        span(
            match grip {
                Idle => \"idle\"
                Holding(i) => \"held {i}\"
            }
        )
    )
}

main() -> Element => div(board())
";

#[test]
fn a_view_local_read_through_a_match_follows_the_handler() {
    let out = run(
        GRIP,
        &[
            "view()",
            "(fire(el(\"button\"), \"dragstart\"), view())",
            "(fire(el(\"button\"), \"dragend\"), view())",
        ],
    );
    assert_eq!(out[0], "cell|idle");
    assert_eq!(out[1], "cell|held 2");
    assert_eq!(out[2], "cell|held 0");
}

/// The same view, with the `match` given a name first.
const DERIVED: &str = "\
Grip = Idle | Holding(int)

board() -> Element {
    grip = Idle

    grab(index: int) {
        grip = Holding(index)
    }

    label = match grip {
        Idle => \"idle\"
        Holding(i) => \"held {i}\"
    }

    div(button(ondragstart=(e => grab(2)) \"cell\") span(label))
}

main() -> Element => div(board())
";

#[test]
fn a_local_computed_from_a_view_local_is_recomputed_with_it() {
    let out = run(
        DERIVED,
        &["view()", "(fire(el(\"button\"), \"dragstart\"), view())"],
    );
    assert_eq!(out[0], "cell|idle");
    assert_eq!(
        out[1], "cell|held 2",
        "naming the match must not freeze the node reading it"
    );
}

#[test]
fn a_local_a_handler_rewrites_is_not_recomputed_behind_its_back() {
    let src = "\
row() -> Element {
    n = 1
    doubled = n * 2

    bump() {
        n = n + 1
        doubled = doubled + 100
    }

    div(button(onclick=bump, \"go\") span(\"{doubled}\"))
}

main() -> Element => div(row())
";
    let out = run(
        src,
        &[
            "view()",
            "(fire(el(\"button\"), \"click\"), view())",
            "(fire(el(\"button\"), \"click\"), view())",
        ],
    );
    assert_eq!(out[0], "go|2");
    assert_eq!(out[1], "go|102", "the handler's value stands");
    assert_eq!(out[2], "go|202");
}

#[test]
fn a_view_local_reaches_an_attribute_and_a_class() {
    let src = "\
row() -> Element {
    on = false

    flip() {
        on = !on
    }

    div(button(onclick=flip, \"flip\") span(class=\"{on}\", hidden=on, \"x\"))
}

main() -> Element => div(row())
";
    let out = run(
        src,
        &[
            "el(\"span\").className",
            "JSON.stringify(el(\"span\")._a)",
            "(fire(el(\"button\"), \"click\"), el(\"span\").className)",
            "JSON.stringify(el(\"span\")._a)",
        ],
    );
    assert_eq!(out[0], "false");
    assert_eq!(out[1], "{}", "a false flag is an absent attribute");
    assert_eq!(out[2], "true");
    assert_eq!(out[3], "{\"hidden\":\"\"}");
}

#[test]
fn a_value_on_a_view_local_writes_it_back_and_repaints() {
    let src = "\
field() -> Element {
    who = \"world\"

    div(input(value=who) span(\"{who}\"))
}

main() -> Element => div(field() field())
";
    let out = run(
        src,
        &[
            "view()",
            "(fire(el(\"input\"), \"input\", \"maca\"), view())",
            "el(\"input\").value",
            "el(\"input\", 1).value",
        ],
    );
    assert_eq!(out[0], "|world||world");
    assert_eq!(out[1], "|maca||world", "the other instance keeps its own");
    assert_eq!(out[2], "maca");
    assert_eq!(out[3], "world");
}

/// Module state, read from inside a view function rather than from `main`.
const SHARED: &str = "\
count = 0

bump() { count = count + 1 }

panel() -> Element => span(\"{count}\")

main() -> Element => div(button(onclick=bump, \"+\") panel())
";

#[test]
fn module_state_read_inside_a_view_function_still_repaints() {
    let out = run(
        SHARED,
        &[
            "view()",
            "(fire(el(\"button\"), \"click\"), view())",
            "counted(() => maca.set(\"count\", 7))",
            "view()",
        ],
    );
    assert_eq!(out[0], "+|0");
    assert_eq!(out[1], "+|1");
    assert_eq!(out[2], "1");
    assert_eq!(out[3], "+|7");
}

#[test]
fn main_owns_state_the_same_way_a_view_does() {
    let src = "\
main() -> Element {
    n = 0

    bump() {
        n = n + 1
    }

    div(button(onclick=bump, \"+\") span(\"{n}\"))
}
";
    let out = run(
        src,
        &[
            "view()",
            "(fire(el(\"button\"), \"click\"), view())",
            "counted(() => update())",
        ],
    );
    assert_eq!(out[0], "+|0");
    assert_eq!(out[1], "+|1");
    assert_eq!(out[2], "1");
}

#[test]
fn a_calculation_that_writes_its_own_local_is_not_state() {
    let src = "\
total(a: int, b: int) -> int {
    sum = 0

    add(x: int) {
        sum = sum + x
    }

    add(a)
    add(b)
    sum
}

n = 1

bump() { n = n + 1 }

main() -> Element => div(button(onclick=bump, \"+\") span(\"{total(n, 10)}\"))
";
    let out = run(
        src,
        &[
            "view()",
            "(fire(el(\"button\"), \"click\"), view())",
            "(fire(el(\"button\"), \"click\"), view())",
        ],
    );
    assert_eq!(out[0], "+|11");
    assert_eq!(
        out[1], "+|12",
        "a helper's scratch local is not an update, and does not loop"
    );
    assert_eq!(out[2], "+|13");
}
