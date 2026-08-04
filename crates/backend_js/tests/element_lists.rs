use std::io::Write;
use std::process::Command;

/// A DOM small enough to read, printing itself as `tag[class]{children}` so a missing node is visible.
const DOM: &str = r#"function makeNode(tag) {
  return { nodeType: 1, tagName: String(tag).toUpperCase(), className: "", _a: {}, _on: {},
    children: [], innerHTML: "",
    setAttribute(k, v) { this._a[k] = v; }, removeAttribute(k) { delete this._a[k]; },
    addEventListener(k, f) { (this._on[k] = this._on[k] || []).push(f); },
    appendChild(c) { this.children.push(c); return c; },
    insertBefore(c, before) {
      const at = this.children.indexOf(before);
      this.children.splice(at < 0 ? this.children.length : at, 0, c);
      return c;
    },
    removeChild(c) {
      const at = this.children.indexOf(c);
      if (at >= 0) this.children.splice(at, 1);
      return c;
    } };
}
function makeText(t) { return { nodeType: 3, textContent: String(t) }; }
const app = makeNode("div");
global.document = { activeElement: null, createElement: makeNode, createTextNode: makeText,
  getElementById: (id) => (id === "app" ? app : null) };
function show(n) {
  if (n.nodeType === 3) return n.textContent;
  const cls = n.className ? "." + n.className : "";
  return n.tagName.toLowerCase() + cls + "{" + n.children.map(show).join("") + "}";
}
function view() { return app.children.map(show).join(""); }
globalThis.assert_eq = function (got, want, m) {
  if (String(got) !== String(want)) {
    throw new Error(m + "\n  got:  " + String(got) + "\n  want: " + String(want));
  }
};
function count(name, n) {
  n = n || app;
  const here = n.tagName === name.toUpperCase() ? 1 : 0;
  return here + (n.children || []).reduce((a, c) => a + count(name, c), 0);
}
"#;

/// Emit `src`, mount it under the DOM stub, and return one line per expression in `calls`.
fn run(src: &str, calls: &[&str]) -> Vec<String> {
    let p = maca_parser::parse(src);
    assert!(p.errors.is_empty(), "parse: {:?}", p.errors);
    let js = maca_backend_js::emit(&p.module).js;

    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&(src, calls), &mut h);
    let key = std::hash::Hasher::finish(&h);
    let dir = std::env::temp_dir().join(format!("maca-js-el-{}-{key:x}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::File::create(dir.join("app.js"))
        .unwrap()
        .write_all(js.as_bytes())
        .unwrap();

    let checked = Command::new("node")
        .arg("--check")
        .arg(dir.join("app.js"))
        .output()
        .expect("node is required for the JS backend tests");
    assert!(
        checked.status.success(),
        "node --check rejected the emitted program\n{}\n--- js ---\n{js}",
        String::from_utf8_lossy(&checked.stderr)
    );

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
        .expect("node runs the emitted program");
    assert!(
        out.status.success(),
        "node failed\n{}\n--- js ---\n{js}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect()
}

/// The shape the start page is written in: a view that renders nothing returns the empty list.
const TOOLBAR: &str = "\
locked = true

toolbar() -> Element[] {
    if locked {
        return []
    }

    [div(class=\"bar\", \"edit\")]
}

main() -> Element =>
    section(
        span(\"title\")
        toolbar()
    )
";

#[test]
fn a_view_that_returns_the_empty_list_adds_no_node() {
    let out = run(
        TOOLBAR,
        &[
            "view()",
            "count(\"div\")",
            "(state.locked = false, view())",
            "(state.locked = true, view())",
        ],
    );
    assert_eq!(
        out,
        [
            "section{span{title}}",
            "1",
            "section{span{title}div.bar{edit}}",
            "section{span{title}}"
        ],
        "the empty list contributes nothing, and the view follows the state it reads"
    );
}

/// One list of children, gathered from two views with `++`.
const JOINED: &str = "\
Link = { title: str, icon: str }

pencil(on: bool) -> Element[] {
    if !on {
        return []
    }

    [button(class=\"pencil\", \"edit\")]
}

row(link: Link, on: bool) -> Element =>
    li(class=\"row\",
       [i(class=\"icon\", link.icon), span(class=\"title\", link.title)] ++ pencil(on))

main() -> Element =>
    ul(
        row(Link { title = \"Docs\", icon = \"book\" }, false)
        row(Link { title = \"Mail\", icon = \"at\" }, true)
    )
";

#[test]
fn a_list_of_children_is_spliced_into_the_parent() {
    let out = run(JOINED, &["view()", "count(\"button\")"]);
    assert_eq!(
        out,
        [
            "ul{li.row{i.icon{book}span.title{Docs}}\
             li.row{i.icon{at}span.title{Mail}button.pencil{edit}}}",
            "1"
        ],
        "each element of the list is its own node, and the locked row has no pencil"
    );
}

/// `.map` over data, which is the other way a child list arrives.
const MAPPED: &str = "\
items() -> str[] => [\"a\", \"b\", \"c\"]

main() -> Element =>
    ul(items().map(t => li(t)))
";

#[test]
fn a_mapped_list_becomes_one_node_per_element() {
    let out = run(MAPPED, &["view()", "count(\"li\")"]);
    assert_eq!(out, ["ul{li{a}li{b}li{c}}", "3"]);
}

/// Two views joined with `++` where the parent is the root view, not a transpiled function.
const ROOT_JOIN: &str = "\
head() -> Element[] => [li(\"a\")]

tail() -> Element[] => []

main() -> Element => ul(head() ++ tail() ++ [li(\"b\")])
";

#[test]
fn two_element_lists_joined_at_the_root_are_both_children() {
    assert_eq!(
        run(ROOT_JOIN, &["view()", "count(\"li\")"]),
        ["ul{li{a}li{b}}", "2"],
        "the empty one in the middle contributes nothing"
    );
}

/// A list literal written straight into the view, beside an ordinary child.
const LITERAL: &str = "\
main() -> Element =>
    div([span(\"a\"), span(\"b\")], \"tail\")
";

#[test]
fn a_list_literal_is_a_child_where_it_is_written() {
    assert_eq!(
        run(LITERAL, &["view()", "count(\"span\")"]),
        ["div{span{a}span{b}tail}", "2"],
        "in the order the arguments are in"
    );
}

/// The tag/definition rule the native back end already keeps: a definition wins.
const SHADOW: &str = "\
label(pos: bool) -> str => pos ? \"right\" : \"left\"

main() -> Element => div(label(true))
";

#[test]
fn a_definition_still_shadows_the_tag_of_the_same_name() {
    assert_eq!(run(SHADOW, &["view()"]), ["div{right}"]);
}

/// The list methods the start page uses, run under node so the JS lowering is executed, not just parsed.
const METHODS: &str = "\
Link = { title: str, rank: int }

test_set_replaces_one_element() {
    xs = [1, 2, 3]
    assert_eq(xs.set(1, 9).join(\",\"), \"1,9,3\", \"the middle one\")
    assert_eq(xs.set(7, 9).join(\",\"), \"1,2,3\", \"an index it does not have\")
    assert_eq(xs.join(\",\"), \"1,2,3\", \"and the original is untouched\")
}

test_insert_and_remove_shift_the_rest() {
    xs = [1, 2, 3]
    assert_eq(xs.insert(0, 9).join(\",\"), \"9,1,2,3\", \"at the front\")
    assert_eq(xs.insert(99, 9).join(\",\"), \"1,2,3,9\", \"past the end\")
    assert_eq(xs.insert(-4, 9).join(\",\"), \"9,1,2,3\", \"before the start\")
    assert_eq(xs.insert(-1, 9).join(\",\"), \"9,1,2,3\", \"and just before it\")
    assert_eq(xs.set(-1, 9).join(\",\"), \"1,2,3\", \"a negative index changes nothing\")
    assert_eq(xs.remove(1).join(\",\"), \"1,3\", \"the middle one\")
    assert_eq(xs.remove(9).join(\",\"), \"1,2,3\", \"an index it does not have\")
}

test_index_of_by_finds_the_first_match() {
    xs = [4, 7, 9, 7]
    assert_eq(str(xs.index_of_by(v => v == 7)), \"1\", \"the first\")
    assert_eq(str(xs.index_of_by(v => v > 100)), \"-1\", \"none\")
}

test_enumerate_pairs_an_index_with_a_value() {
    out = [\"a\", \"b\"].enumerate().map(e => \"{e.index}:{e.value}\")
    assert_eq(out.join(\",\"), \"0:a,1:b\", \"index and value\")
}

test_sort_by_orders_on_the_key() {
    ls = [Link { title = \"c\", rank = 2 }, Link { title = \"a\", rank = 3 }]
    assert_eq(ls.sort_by(l => l.title).map(l => l.title).join(\",\"), \"a,c\",
              \"alphabetical\")
    assert_eq(ls.sort_by(l => l.rank).map(l => l.title).join(\",\"), \"c,a\",
              \"numeric\")
}

main() -> Element => div(\"methods\")
";

#[test]
fn the_new_list_methods_compute_the_same_answers_under_node() {
    let out = run(
        METHODS,
        &[
            "(test_set_replaces_one_element(), \"set ok\")",
            "(test_insert_and_remove_shift_the_rest(), \"insert ok\")",
            "(test_index_of_by_finds_the_first_match(), \"index_of_by ok\")",
            "(test_enumerate_pairs_an_index_with_a_value(), \"enumerate ok\")",
            "(test_sort_by_orders_on_the_key(), \"sort_by ok\")",
        ],
    );
    assert_eq!(
        out,
        [
            "set ok",
            "insert ok",
            "index_of_by ok",
            "enumerate ok",
            "sort_by ok"
        ]
    );
}
